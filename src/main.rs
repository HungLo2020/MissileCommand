mod desktop;
mod input;
use minifb::{Key, MouseButton, MouseMode, ScaleMode, Window, WindowOptions};
use missile_command::{
    audio::{Audio, Sound},
    game::{Game, GameEvent, Input, TICK_HZ, Vec2},
    render::Renderer,
    storage::{self, Paths, Preferences, Records},
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Default)]
struct Options {
    headless: bool,
    autoplay: bool,
    ticks: u64,
    smoke_frames: Option<u64>,
    seed: Option<u64>,
    snapshot: Option<PathBuf>,
    record: Option<PathBuf>,
    replay: Option<PathBuf>,
    no_audio: bool,
    audio_device: String,
}

impl Options {
    fn parse() -> Result<Option<Self>> {
        let mut args = std::env::args().skip(1);
        let mut options = Self {
            ticks: 3600,
            audio_device: "default".into(),
            ..Self::default()
        };
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} requires a value"));
            match arg.as_str() {
                "--headless" => options.headless = true,
                "--autoplay" => options.autoplay = true,
                "--no-audio" => options.no_audio = true,
                "--ticks" => options.ticks = value()?.parse()?,
                "--smoke-frames" => options.smoke_frames = Some(value()?.parse()?),
                "--seed" => options.seed = Some(value()?.parse()?),
                "--snapshot" => options.snapshot = Some(value()?.into()),
                "--record" => options.record = Some(value()?.into()),
                "--replay" => options.replay = Some(value()?.into()),
                "--audio-device" => options.audio_device = value()?,
                "--version" => {
                    println!("Missile Command {}", env!("CARGO_PKG_VERSION"));
                    return Ok(None);
                }
                "--help" | "-h" => {
                    println!(
                        "Missile Command\n\nMouse / arrows: aim    A S D or mouse buttons: left/center/right silo\nEnter: start    P / Escape: pause    F1: help    M: mute    Q: quit while paused\n\n--headless --ticks N --seed N --autoplay\n--record FILE / --replay FILE   deterministic JSON input replay\n--snapshot FILE                final framebuffer as PPM\n--smoke-frames N               run native window for N rendered frames\n--no-audio / --audio-device NAME  host ALSA playback (default: default)"
                    );
                    return Ok(None);
                }
                _ => return Err(format!("unknown option {arg}; see --help").into()),
            }
        }
        if options.record.is_some() && options.replay.is_some() {
            return Err("choose either --record or --replay".into());
        }
        Ok(Some(options))
    }
}

#[derive(Serialize, Deserialize)]
struct Replay {
    version: u32,
    seed: u64,
    frames: Vec<ReplayInput>,
}
#[derive(Serialize, Deserialize)]
struct ReplayInput {
    aim: [f32; 2],
    fire: [bool; 3],
    start: bool,
}
impl From<Input> for ReplayInput {
    fn from(input: Input) -> Self {
        Self {
            aim: [input.aim.x, input.aim.y],
            fire: input.fire,
            start: input.start,
        }
    }
}
impl From<&ReplayInput> for Input {
    fn from(input: &ReplayInput) -> Self {
        Self {
            aim: Vec2::new(input.aim[0], input.aim[1]),
            fire: input.fire,
            start: input.start,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Missile Command: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let Some(options) = Options::parse()? else {
        return Ok(());
    };
    let replay: Option<Replay> = options
        .replay
        .as_ref()
        .map(|path| -> Result<_> {
            let replay: Replay = serde_json::from_slice(&fs::read(path)?)?;
            if replay.version != 1 {
                return Err("unsupported replay version".into());
            }
            Ok(replay)
        })
        .transpose()?;
    let seed = replay
        .as_ref()
        .map(|r| r.seed)
        .or(options.seed)
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64
        });
    let mut recorded = Replay {
        version: 1,
        seed,
        frames: Vec::new(),
    };
    let mut game = Game::new(seed);
    let mut renderer = Renderer::new();
    if options.headless {
        for tick in 0..options.ticks {
            let input = replay
                .as_ref()
                .and_then(|r| r.frames.get(tick as usize))
                .map(Input::from)
                .unwrap_or_else(|| {
                    if replay.is_some() {
                        Input::default()
                    } else {
                        automatic_input(&game, options.autoplay, tick == 0)
                    }
                });
            if options.record.is_some() {
                recorded.frames.push(input.into());
            }
            game.update(input);
        }
        renderer.render(&game, game.aim, game.score);
        finish_outputs(&options, &recorded, &renderer, &game)?;
        return Ok(());
    }
    let paths = Paths::discover()?;
    paths.create()?;
    let records_path = paths.save.join("records.json");
    let preferences_path = paths.shared.join("preferences.json");
    let (mut records, records_writable) = read_or_default::<Records>(&records_path);
    let (mut preferences, preferences_writable) = read_or_default::<Preferences>(&preferences_path);
    let records_writable = records_writable && records.version <= 1;
    let preferences_writable = preferences_writable && preferences.version <= 1;
    let audio = if options.no_audio {
        None
    } else {
        match Audio::open(&options.audio_device, preferences.muted) {
            Ok(audio) => Some(audio),
            Err(error) => {
                eprintln!("Audio unavailable: {error}");
                None
            }
        }
    };
    let mut window = Window::new(
        "Missile Command — Enter to start · F1 help",
        1024,
        768,
        WindowOptions {
            transparency: false,
            borderless: false,
            resize: true,
            scale_mode: ScaleMode::UpperLeft,
            ..WindowOptions::default()
        },
    )?;
    let keyboard = input::Keyboard::attach(&mut window);
    let desktop = desktop::Desktop::attach(&window)?;
    let mut previous_size = (0, 0);
    let mut previous_title = String::new();
    window.set_target_fps(120);
    let mut frame = Vec::new();
    let mut paused = false;
    let mut help = false;
    let mut aim = Input::default().aim;
    let mut pending_fire = [false; 3];
    let mut previous_mouse = [false; 3];
    let mut last_mouse_position = None;
    let mut had_focus = false;
    let mut pending_start = false;
    let mut elapsed = 0.0_f64;
    let mut last = Instant::now();
    let mut rendered = 0_u64;
    let mut simulation_ticks = 0_usize;
    let step = 1.0 / TICK_HZ;
    let mut last_score_saved = records.high_score;
    while window.is_open() {
        let pressed = keyboard.take_pressed();
        let now = Instant::now();
        let dt = (now - last).as_secs_f64().min(0.1);
        last = now;
        if pressed.contains(&Key::F1) {
            help = !help;
        }
        if pressed.contains(&Key::P) || pressed.contains(&Key::Escape) {
            if help {
                help = false;
            } else {
                paused = !paused;
            }
        }
        let focused = window.is_active();
        if had_focus && !focused {
            keyboard.release_all();
            paused = true;
        }
        had_focus = focused;
        if paused && pressed.contains(&Key::Q) {
            break;
        }
        if pressed.contains(&Key::M) {
            preferences.muted = !preferences.muted;
            if preferences_writable {
                storage::atomic_save(&preferences_path, &preferences)?;
            }
        }
        let stopped = paused || help;
        if let Some(audio) = &audio {
            audio.set_muted(preferences.muted || stopped);
        }
        let (width, height) = window.get_size();
        if width == 0 || height == 0 {
            window.update();
            continue;
        }
        if previous_size != (width, height) {
            desktop.resize(width, height)?;
            previous_size = (width, height);
        }
        let viewport = Viewport::new(width, height);
        if let Some((x, y)) = window.get_mouse_pos(MouseMode::Pass) {
            let moved = last_mouse_position != Some((x, y));
            last_mouse_position = Some((x, y));
            if moved
                && x >= viewport.x as f32
                && y >= viewport.y as f32
                && x < (viewport.x + viewport.width) as f32
                && y < (viewport.y + viewport.height) as f32
            {
                aim = Vec2::new(
                    (x - viewport.x as f32) * 256.0 / viewport.width as f32,
                    (y - viewport.y as f32) * 231.0 / viewport.height as f32,
                );
            }
        }
        let dx = i32::from(keyboard.held(Key::Right)) - i32::from(keyboard.held(Key::Left));
        let dy = i32::from(keyboard.held(Key::Down)) - i32::from(keyboard.held(Key::Up));
        aim.x = (aim.x + dx as f32 * dt as f32 * 160.0).clamp(8.0, 247.0);
        aim.y = (aim.y + dy as f32 * dt as f32 * 160.0).clamp(24.0, 185.0);
        for (i, (key, mouse)) in [
            (Key::A, MouseButton::Left),
            (Key::S, MouseButton::Middle),
            (Key::D, MouseButton::Right),
        ]
        .into_iter()
        .enumerate()
        {
            let down = window.get_mouse_down(mouse);
            if !stopped {
                pending_fire[i] |= pressed.contains(&key) || (down && !previous_mouse[i]);
            }
            previous_mouse[i] = down;
        }
        if pressed.contains(&Key::Enter) {
            if stopped {
                paused = false;
                help = false;
            } else {
                pending_start = true;
            }
        }
        if stopped {
            elapsed = 0.0;
            pending_fire = [false; 3];
            pending_start = false;
        } else {
            elapsed += dt;
            while elapsed >= step {
                let input = if let Some(replay) = &replay {
                    replay
                        .frames
                        .get(simulation_ticks)
                        .map(Input::from)
                        .unwrap_or_default()
                } else if options.autoplay {
                    automatic_input(&game, true, simulation_ticks == 0)
                } else {
                    Input {
                        aim,
                        fire: pending_fire,
                        start: pending_start,
                    }
                };
                if options.record.is_some() {
                    recorded.frames.push(input.into());
                }
                game.update(input);
                if let Some(audio) = &audio {
                    for event in &game.events {
                        let sound = match event {
                            GameEvent::Launch { .. } => Some(Sound::Launch),
                            GameEvent::Empty { .. } => Some(Sound::Empty),
                            GameEvent::Explosion => Some(Sound::Explosion),
                            GameEvent::CityDestroyed { .. } | GameEvent::BaseDestroyed { .. } => {
                                Some(Sound::CityLost)
                            }
                            GameEvent::BonusCity | GameEvent::WaveCompleted => Some(Sound::Bonus),
                            GameEvent::GameOver => Some(Sound::GameOver),
                            _ => None,
                        };
                        if let Some(sound) = sound {
                            audio.play(sound);
                        }
                    }
                }
                records.high_score = records.high_score.max(game.score);
                records.best_wave = records.best_wave.max(game.wave);
                if records_writable
                    && last_score_saved != records.high_score
                    && game
                        .events
                        .iter()
                        .any(|e| matches!(e, GameEvent::WaveCompleted | GameEvent::GameOver))
                {
                    storage::atomic_save(&records_path, &records)?;
                    last_score_saved = records.high_score;
                }
                pending_fire = [false; 3];
                pending_start = false;
                simulation_ticks += 1;
                elapsed -= step;
            }
        }
        renderer.render(
            &game,
            if options.autoplay || replay.is_some() {
                game.aim
            } else {
                aim
            },
            records.high_score,
        );
        if help {
            renderer.overlay(
                "HOW TO PLAY",
                &[
                    "MOUSE / ARROWS : AIM",
                    "A S D : LEFT CENTER RIGHT",
                    "MOUSE BUTTONS ALSO FIRE",
                    "CENTER MISSILES ARE FASTER",
                    "CHAIN EXPLOSIONS TO DEFEND",
                    "10000 POINTS : BONUS CITY",
                    "M : MUTE   ENTER : RETURN",
                ],
            );
        } else if paused {
            renderer.overlay(
                "PAUSED",
                &[
                    "ENTER / P : RESUME",
                    "F1 : HOW TO PLAY",
                    "M : MUTE",
                    "Q : QUIT",
                ],
            );
        }
        let title = if help {
            "Missile Command — Controls"
        } else if paused {
            "Missile Command — Paused"
        } else {
            "Missile Command — F1 help"
        };
        if title != previous_title {
            window.set_title(title);
            previous_title = title.into();
        }
        viewport.present(&renderer.pixels, &mut frame, width, height);
        window.update_with_buffer(&frame, width, height)?;
        rendered += 1;
        if options.smoke_frames.is_some_and(|limit| rendered >= limit) {
            break;
        }
    }
    if records_writable {
        storage::atomic_save(&records_path, &records)?;
    }
    finish_outputs(&options, &recorded, &renderer, &game)?;
    Ok(())
}

fn read_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> (T, bool) {
    match storage::load(path) {
        Ok(value) => (value, true),
        Err(error) => {
            eprintln!(
                "Cannot read {}: {error}; preserving original file",
                path.display()
            );
            (T::default(), false)
        }
    }
}

/// A test driver, deliberately outside the game rules and unavailable during normal play.
fn automatic_input(game: &Game, autoplay: bool, start: bool) -> Input {
    let mut input = Input {
        start,
        ..Input::default()
    };
    if !autoplay {
        return input;
    }
    if let Some(enemy) = game
        .missiles
        .iter()
        .filter(|m| {
            !matches!(
                m.kind,
                missile_command::game::MissileKind::Interceptor { .. }
            )
        })
        .max_by(|a, b| a.position.y.total_cmp(&b.position.y))
    {
        input.aim = Vec2::new(enemy.position.x, (enemy.position.y + 8.0).min(190.0));
    } else {
        input.aim = Vec2::new(128.0, 90.0);
    }
    if game.tick % 24 == 0 {
        let preferred = if input.aim.x < 80.0 {
            0
        } else if input.aim.x > 176.0 {
            2
        } else {
            1
        };
        let base = [preferred, 1, 0, 2]
            .into_iter()
            .find(|&i| game.bases[i].alive && game.bases[i].ammo > 0);
        if let Some(base) = base {
            input.fire[base] = true;
        }
    }
    input
}

fn finish_outputs(
    options: &Options,
    recorded: &Replay,
    renderer: &Renderer,
    game: &Game,
) -> Result<()> {
    if let Some(path) = &options.record {
        storage::atomic_save(path, recorded)?;
    }
    if let Some(path) = &options.snapshot {
        write_ppm(path, &renderer.pixels)?;
    }
    println!(
        "ticks={} wave={} score={} cities={} phase={:?}",
        game.tick,
        game.wave,
        game.score,
        game.alive_cities(),
        game.phase
    );
    Ok(())
}

fn write_ppm(path: &Path, pixels: &[u32]) -> io::Result<()> {
    let mut file = io::BufWriter::new(fs::File::create(path)?);
    file.write_all(b"P6\n256 231\n255\n")?;
    for pixel in pixels {
        file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])?;
    }
    file.flush()
}

struct Viewport {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}
impl Viewport {
    fn new(width: usize, height: usize) -> Self {
        let view_width = width.min(height.saturating_mul(4) / 3).max(1);
        let view_height = (view_width * 3 / 4).max(1).min(height);
        Self {
            x: (width - view_width) / 2,
            y: (height - view_height) / 2,
            width: view_width,
            height: view_height,
        }
    }
    fn present(&self, pixels: &[u32], output: &mut Vec<u32>, width: usize, height: usize) {
        output.resize(width * height, 0xff000000);
        output.fill(0xff000000);
        for y in 0..self.height {
            let source_y = y * 231 / self.height;
            for x in 0..self.width {
                output[(self.y + y) * width + self.x + x] =
                    0xff000000 | pixels[source_y * 256 + x * 256 / self.width];
            }
        }
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;
    #[test]
    fn composed_frame_and_letterbox_are_always_opaque() {
        let viewport = Viewport::new(1000, 500);
        let mut output = Vec::new();
        let input = vec![0x00010203; 256 * 231];
        viewport.present(&input, &mut output, 1000, 500);
        assert!(output.iter().all(|pixel| pixel >> 24 == 0xff));
        assert_eq!(output[0], 0xff000000);
        assert_eq!(output[viewport.y * 1000 + viewport.x], 0xff010203);
    }
    #[test]
    fn resizing_keeps_four_three_viewport_inside_window() {
        for (w, h) in [(1, 1), (640, 480), (1920, 1080), (400, 1000)] {
            let viewport = Viewport::new(w, h);
            assert!(viewport.width + viewport.x <= w);
            assert!(viewport.height + viewport.y <= h);
            let mut output = Vec::new();
            viewport.present(&vec![0; 256 * 231], &mut output, w, h);
            assert_eq!(output.len(), w * h);
        }
    }
}
