#[cfg(feature = "desktop")]
mod desktop;

use missile_command::{
    game::{Game, Input, Vec2},
    render::Renderer,
    storage,
};
use serde::{Deserialize, Serialize};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Default)]
pub(crate) struct Options {
    pub(crate) headless: bool,
    pub(crate) autoplay: bool,
    pub(crate) ticks: u64,
    pub(crate) smoke_frames: Option<u64>,
    pub(crate) seed: Option<u64>,
    pub(crate) snapshot: Option<PathBuf>,
    pub(crate) record: Option<PathBuf>,
    pub(crate) replay: Option<PathBuf>,
    pub(crate) no_audio: bool,
    pub(crate) audio_device: String,
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
                        "Missile Command\n\nMouse / arrows: aim    A S D or mouse buttons: left/center/right silo\nEnter: start    P / Escape: pause    F1: help    M: mute    Q: quit while paused\n\n--headless --ticks N --seed N --autoplay\n--record FILE / --replay FILE   deterministic JSON input replay\n--snapshot FILE                final framebuffer as PPM\n--smoke-frames N               run native window for N rendered frames\n--no-audio / --audio-device NAME  host audio through Quasar (default: default)"
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

pub(crate) struct Session {
    pub(crate) game: Game,
    replay: Option<Replay>,
    recording: Option<Replay>,
    ticks: usize,
}

impl Session {
    pub(crate) fn new(options: &Options) -> Result<Self> {
        let replay = options
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
            .map(|replay| replay.seed)
            .or(options.seed)
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as u64
            });
        Ok(Self {
            game: Game::new(seed),
            recording: options.record.as_ref().map(|_| Replay {
                version: 1,
                seed,
                frames: Vec::new(),
            }),
            replay,
            ticks: 0,
        })
    }

    pub(crate) fn tick(&mut self, live: Input, autoplay: bool) -> bool {
        let input = if let Some(replay) = &self.replay {
            let Some(frame) = replay.frames.get(self.ticks) else {
                return false;
            };
            Input::from(frame)
        } else if autoplay {
            automatic_input(&self.game, true, self.ticks == 0)
        } else {
            live
        };
        if let Some(recording) = &mut self.recording {
            recording.frames.push(input.into());
        }
        self.game.update(input);
        self.ticks += 1;
        true
    }

    #[cfg(feature = "desktop")]
    pub(crate) fn is_replay(&self) -> bool {
        self.replay.is_some()
    }

    pub(crate) fn finish(&self, options: &Options, renderer: &Renderer) -> Result<()> {
        if let Some(path) = &options.record {
            let Some(recording) = &self.recording else {
                return Err("--record requested without a recording".into());
            };
            storage::atomic_save(path, recording)?;
        }
        if let Some(path) = &options.snapshot {
            write_ppm(path, &renderer.pixels)?;
        }
        println!(
            "ticks={} wave={} score={} cities={} phase={:?}",
            self.game.tick,
            self.game.wave,
            self.game.score,
            self.game.alive_cities(),
            self.game.phase
        );
        Ok(())
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
    let mut session = Session::new(&options)?;
    if options.headless {
        for _ in 0..options.ticks {
            if !session.tick(Input::default(), options.autoplay) {
                break;
            }
        }
        let mut renderer = Renderer::new();
        renderer.render(&session.game, session.game.aim, session.game.score);
        session.finish(&options, &renderer)?;
        return Ok(());
    }

    #[cfg(feature = "desktop")]
    {
        desktop::run(options, session)
    }
    #[cfg(not(feature = "desktop"))]
    {
        let _ = session;
        Err("desktop support disabled; pass --headless or rebuild with default features".into())
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
        .filter(|missile| {
            !matches!(
                missile.kind,
                missile_command::game::MissileKind::Interceptor { .. }
            )
        })
        .max_by(|a, b| a.position.y.total_cmp(&b.position.y))
    {
        input.aim = Vec2::new(enemy.position.x, (enemy.position.y + 8.0).min(190.0));
    } else {
        input.aim = Vec2::new(128.0, 90.0);
    }
    if game.tick.is_multiple_of(24) {
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

#[cfg(feature = "desktop")]
pub(crate) fn read_or_default<T: for<'de> Deserialize<'de> + Default>(path: &Path) -> (T, bool) {
    match storage::load(path) {
        Ok(Some(value)) => (value, true),
        Ok(None) => (T::default(), true),
        Err(error) => {
            eprintln!(
                "Cannot read {}: {error}; preserving original file",
                path.display()
            );
            (T::default(), false)
        }
    }
}

fn write_ppm(path: &Path, pixels: &[u32]) -> io::Result<()> {
    let mut file = io::BufWriter::new(fs::File::create(path)?);
    file.write_all(b"P6\n256 231\n255\n")?;
    for pixel in pixels {
        file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8])?;
    }
    file.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replay_session() -> Session {
        Session {
            game: Game::new(7),
            replay: Some(Replay {
                version: 1,
                seed: 7,
                frames: vec![ReplayInput {
                    aim: [10.0, 20.0],
                    fire: [true, false, false],
                    start: true,
                }],
            }),
            recording: None,
            ticks: 0,
        }
    }

    #[test]
    fn replay_exhaustion_does_not_apply_an_extra_tick() {
        let mut session = replay_session();
        assert!(session.tick(Input::default(), false));
        assert_eq!(session.ticks, 1);
        let after_first_tick = session.game.tick;
        assert!(!session.tick(Input::default(), false));
        assert_eq!(session.ticks, 1);
        assert_eq!(session.game.tick, after_first_tick);
    }

    #[test]
    fn live_ticks_always_advance() {
        let mut session = Session {
            game: Game::new(7),
            replay: None,
            recording: None,
            ticks: 0,
        };
        assert!(session.tick(Input::default(), false));
        assert!(session.tick(Input::default(), false));
        assert_eq!(session.ticks, 2);
    }
}
