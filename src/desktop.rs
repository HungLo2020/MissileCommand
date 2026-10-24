use crate::{Options, Session, read_or_default};
use missile_command::{
    audio::{Audio, Sound},
    game::{GameEvent, HEIGHT, Input, TICK_HZ, Vec2, WIDTH},
    render::Renderer,
    storage::{self, Preferences, Records},
};
use quasar_core::{DisplayOptions, Filter, FixedStep, Scaling};
use quasar_input::{InputState, Key, MouseButton};
use quasar_render_2d::{FramebufferPresenter, PresentOutcome};
use quasar_storage::GamePaths;
use quasar_window::{
    DesktopConfig, GameWindow, WindowConfig, X11Identity,
    winit::{
        application::ApplicationHandler,
        event::{DeviceEvent, DeviceId, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::WindowId,
    },
};
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, Default, PartialEq, Eq)]
struct GameplayEdges {
    fire: [bool; 3],
    start: bool,
}

fn gameplay_edges(input: &InputState, stopped: bool) -> GameplayEdges {
    if stopped {
        return GameplayEdges::default();
    }
    GameplayEdges {
        fire: [
            input.pressed(Key::KeyA) || input.mouse_pressed(MouseButton::Left),
            input.pressed(Key::KeyS) || input.mouse_pressed(MouseButton::Middle),
            input.pressed(Key::KeyD) || input.mouse_pressed(MouseButton::Right),
        ],
        start: input.pressed(Key::Enter),
    }
}

fn adjusted_aim(mut aim: Vec2, input: &InputState, dt: f64) -> Vec2 {
    let dx = i32::from(input.held(Key::ArrowRight)) - i32::from(input.held(Key::ArrowLeft));
    let dy = i32::from(input.held(Key::ArrowDown)) - i32::from(input.held(Key::ArrowUp));
    aim.x = (aim.x + dx as f32 * dt as f32 * 160.0).clamp(8.0, 247.0);
    aim.y = (aim.y + dy as f32 * dt as f32 * 160.0).clamp(24.0, 185.0);
    aim
}

pub fn run(options: Options, session: Session) -> Result {
    let paths = GamePaths::discover(Some(Path::new(env!("CARGO_MANIFEST_DIR"))))?;
    let records_path = paths.save.join("records.json");
    let preferences_path = paths.shared_config.join("preferences.json");
    let (records, records_readable) = read_or_default::<Records>(&records_path);
    let (preferences, preferences_readable) = read_or_default::<Preferences>(&preferences_path);
    let records_writable = records_readable && records.version <= 1;
    let preferences_writable = preferences_readable && preferences.version <= 1;
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
    let mut app = App {
        last_score_saved: records.high_score,
        records,
        preferences,
        records_writable,
        preferences_writable,
        records_path,
        preferences_path,
        options,
        session,
        window: None,
        presenter: None,
        renderer: Renderer::new(),
        rgba: Vec::new(),
        audio,
        paused: false,
        help: false,
        aim: Input::default().aim,
        pending_fire: [false; 3],
        pending_start: false,
        clock: FixedStep::new(TICK_HZ, 8)?,
        previous: Instant::now(),
        started: Instant::now(),
        frames: 0,
        window_size: (0, 0),
        title: String::new(),
        error: None,
    };
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    if app.records_writable {
        storage::atomic_save(&app.records_path, &app.records)?;
    }
    if app.preferences_writable {
        storage::atomic_save(&app.preferences_path, &app.preferences)?;
    }
    app.session.finish(&app.options, &app.renderer)?;
    if let Some(error) = app.error.take() {
        return Err(error.into());
    }
    Ok(())
}

struct App {
    options: Options,
    session: Session,
    records_path: PathBuf,
    preferences_path: PathBuf,
    records: Records,
    preferences: Preferences,
    records_writable: bool,
    preferences_writable: bool,
    // Quasar graphics must be dropped before the window on suspension/exit.
    presenter: Option<FramebufferPresenter>,
    window: Option<GameWindow>,
    renderer: Renderer,
    rgba: Vec<u8>,
    audio: Option<Audio>,
    paused: bool,
    help: bool,
    aim: Vec2,
    pending_fire: [bool; 3],
    pending_start: bool,
    clock: FixedStep,
    previous: Instant,
    started: Instant,
    frames: u64,
    window_size: (u32, u32),
    title: String,
    last_score_saved: u64,
    error: Option<String>,
}

impl App {
    fn automated(&self) -> bool {
        self.options.autoplay || self.session.is_replay() || self.options.smoke_frames.is_some()
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.error = Some(error.to_string());
        event_loop.exit();
    }

    fn apply_size(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width, height) == self.window_size {
            return;
        }
        self.window_size = (width, height);
        if let Some(presenter) = &mut self.presenter {
            presenter.resize(width, height);
        }
    }

    fn initialize(&mut self, event_loop: &ActiveEventLoop) -> Result {
        // Quasar applies the X11 identity and opacity policy before returning, so
        // Vulkan presenter setup cannot delay compositor metadata.
        let mut window = GameWindow::create_with_desktop(
            event_loop,
            &WindowConfig {
                title: "Missile Command — Enter to start · F1 help".into(),
                application_id: "missile-command".into(),
                size: (1024, 768),
                minimum_size: (320, 240),
                fullscreen: false,
                resizable: true,
            },
            &DesktopConfig {
                x11: Some(X11Identity::with_names("missile-command", "MissileCommand")),
                ..DesktopConfig::for_application("missile-command")
            },
        )?;
        let size = window.native().inner_size();
        self.window_size = (size.width, size.height);
        let presenter = FramebufferPresenter::new(
            window.shared_native(),
            (size.width.max(1), size.height.max(1)),
            (WIDTH, HEIGHT),
            DisplayOptions {
                aspect: Some((4, 3)),
                filter: Filter::Nearest,
                scaling: Scaling::Fit,
            },
        )?;
        // Reconcile a resize that landed while the presenter initialized.
        window.sync_desktop();
        window
            .native()
            .set_title("Missile Command — Enter to start · F1 help");
        self.window = Some(window);
        self.presenter = Some(presenter);
        self.previous = Instant::now();
        self.started = Instant::now();
        self.clock.reset();
        self.frames = 0;
        Ok(())
    }

    fn redraw(&mut self, event_loop: &ActiveEventLoop) -> Result {
        let Some(window) = &self.window else {
            return Ok(());
        };
        if !window.drawable() {
            return Ok(());
        }
        let aim = if self.options.autoplay || self.session.is_replay() {
            self.session.game.aim
        } else {
            self.aim
        };
        self.renderer
            .render(&self.session.game, aim, self.records.high_score);
        if self.help {
            self.renderer.overlay(
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
        } else if self.paused {
            self.renderer.overlay(
                "PAUSED",
                &[
                    "ENTER / P : RESUME",
                    "F1 : HOW TO PLAY",
                    "M : MUTE",
                    "Q : QUIT",
                ],
            );
        }
        self.renderer.write_rgba(&mut self.rgba);
        let presented = self
            .presenter
            .as_mut()
            .expect("presenter exists while the window is alive")
            .present(&self.rgba)?;
        if presented == PresentOutcome::Presented {
            self.frames += 1;
            if self
                .options
                .smoke_frames
                .is_some_and(|limit| self.frames >= limit)
            {
                event_loop.exit();
                return Ok(());
            }
        }
        let title = if self.help {
            "Missile Command — Controls"
        } else if self.paused {
            "Missile Command — Paused"
        } else {
            "Missile Command — F1 help"
        };
        if self.title != title {
            self.window
                .as_ref()
                .expect("window exists during redraw")
                .native()
                .set_title(title);
            self.title = title.into();
        }
        Ok(())
    }

    fn process_input(&mut self, event_loop: &ActiveEventLoop, now: Instant) {
        let Some(window) = &mut self.window else {
            return;
        };
        let frame_dt = (now - self.previous).as_secs_f64().min(0.1);
        let input = &mut window.input;

        if let Some((x, y)) = input.cursor_physical
            && let Some(viewport) = self
                .presenter
                .as_ref()
                .and_then(FramebufferPresenter::viewport)
            && let Some((source_x, source_y)) = viewport.to_source(x, y)
        {
            self.aim = Vec2::new(source_x as f32, source_y as f32);
        }
        self.aim = adjusted_aim(self.aim, input, frame_dt);

        if input.pressed(Key::F1) {
            self.help = !self.help;
        }
        if input.pressed(Key::KeyP) || input.pressed(Key::Escape) {
            if self.help {
                self.help = false;
            } else {
                self.paused = !self.paused;
            }
        }
        if self.paused && input.pressed(Key::KeyQ) {
            event_loop.exit();
            return;
        }
        if input.pressed(Key::KeyM) {
            self.preferences.muted = !self.preferences.muted;
            if self.preferences_writable {
                let _ = storage::atomic_save(&self.preferences_path, &self.preferences);
            }
        }
        let stopped = self.paused || self.help;
        if stopped && input.pressed(Key::Enter) {
            self.paused = false;
            self.help = false;
        }
        let edges = gameplay_edges(input, stopped);
        for (index, fire) in edges.fire.into_iter().enumerate() {
            self.pending_fire[index] |= fire;
        }
        self.pending_start |= edges.start;
        if stopped {
            self.pending_fire = [false; 3];
            self.pending_start = false;
        }
        input.consume_edges();
        if let Some(audio) = &self.audio {
            audio.set_muted(self.preferences.muted || stopped);
        }
    }

    fn sound_for(event: &GameEvent) -> Option<Sound> {
        match event {
            GameEvent::Launch { .. } => Some(Sound::Launch),
            GameEvent::Empty { .. } => Some(Sound::Empty),
            GameEvent::Explosion => Some(Sound::Explosion),
            GameEvent::CityDestroyed { .. } | GameEvent::BaseDestroyed { .. } => {
                Some(Sound::CityLost)
            }
            GameEvent::BonusCity | GameEvent::WaveCompleted => Some(Sound::Bonus),
            GameEvent::GameOver => Some(Sound::GameOver),
            _ => None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none()
            && let Err(error) = self.initialize(event_loop)
        {
            self.fail(event_loop, error);
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.presenter.take();
        self.window.take();
        self.pending_fire = [false; 3];
        self.pending_start = false;
        self.clock.reset();
        self.previous = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let update = {
            let Some(window) = &mut self.window else {
                return;
            };
            if window.native().id() != id {
                return;
            }
            window.process(&event)
        };
        if update.close_requested {
            event_loop.exit();
            return;
        }
        if let Some(size) = update.resized {
            self.apply_size(size.width, size.height);
        }
        if let Some(focused) = update.focus_changed {
            self.clock.reset();
            self.previous = Instant::now();
            if !focused {
                self.pending_fire = [false; 3];
                self.pending_start = false;
                if !self.automated() {
                    self.paused = true;
                }
            }
        }
        if update.redraw_requested
            && let Err(error) = self.redraw(event_loop)
        {
            self.fail(event_loop, error);
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let Some(window) = &mut self.window {
            window.process_device(&event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.options.smoke_frames.is_some() && self.started.elapsed() > Duration::from_secs(30) {
            self.fail(
                event_loop,
                "native smoke run timed out before presenting requested frames",
            );
            return;
        }
        let automated = self.automated();
        if let Some(window) = &mut self.window {
            window.sync_desktop();
        }
        let current_size = self.window.as_ref().map(|window| {
            let size = window.native().inner_size();
            (size.width, size.height)
        });
        if let Some((width, height)) = current_size {
            self.apply_size(width, height);
        }
        let now = Instant::now();
        self.process_input(event_loop, now);
        let focused = self.window.as_ref().is_some_and(|window| window.focused());
        if self.paused || self.help {
            self.clock.reset();
        } else if focused || automated {
            self.clock.advance(now - self.previous);
        } else {
            self.clock.reset();
        }
        self.previous = now;

        let mut replay_finished = false;
        while self.clock.take_tick() {
            let input = Input {
                aim: self.aim,
                fire: self.pending_fire,
                start: self.pending_start,
            };
            self.pending_fire = [false; 3];
            self.pending_start = false;
            if !self.session.tick(input, self.options.autoplay) {
                replay_finished = true;
                break;
            }
            if let Some(audio) = &self.audio {
                for event in &self.session.game.events {
                    if let Some(sound) = App::sound_for(event) {
                        audio.play(sound);
                    }
                }
            }
            self.records.high_score = self.records.high_score.max(self.session.game.score);
            self.records.best_wave = self.records.best_wave.max(self.session.game.wave);
            if self.records_writable
                && self.last_score_saved != self.records.high_score
                && self
                    .session
                    .game
                    .events
                    .iter()
                    .any(|event| matches!(event, GameEvent::WaveCompleted | GameEvent::GameOver))
            {
                if let Err(error) = storage::atomic_save(&self.records_path, &self.records) {
                    self.fail(event_loop, error);
                    return;
                }
                self.last_score_saved = self.records.high_score;
            }
        }
        if replay_finished {
            event_loop.exit();
            return;
        }
        if self.window.as_ref().is_some_and(|window| window.drawable()) {
            self.window
                .as_ref()
                .expect("window exists")
                .native()
                .request_redraw();
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            now + self.clock.until_next_tick().min(Duration::from_millis(16)),
        ));
    }

    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.presenter.take();
        self.window.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quasar_core::Viewport;

    #[test]
    fn captured_edges_survive_a_tap_and_ignore_auto_repeat() {
        let mut input = InputState::default();
        input.key(Key::KeyA, true);
        input.key(Key::KeyA, false);
        assert!(gameplay_edges(&input, false).fire[0]);
        input.consume_edges();
        assert!(!gameplay_edges(&input, false).fire[0]);

        input.key(Key::KeyA, true);
        assert!(gameplay_edges(&input, false).fire[0]);
        input.consume_edges();
        input.key(Key::KeyA, true);
        assert!(!gameplay_edges(&input, false).fire[0]);
    }

    #[test]
    fn mouse_edges_are_captured_and_focus_release_clears_them() {
        let mut input = InputState::default();
        input.mouse_button(MouseButton::Left, true);
        input.mouse_button(MouseButton::Left, false);
        assert!(gameplay_edges(&input, false).fire[0]);
        input.consume_edges();
        assert!(!gameplay_edges(&input, false).fire[0]);

        input.key(Key::KeyA, true);
        input.key(Key::Enter, true);
        input.release_all();
        assert_eq!(gameplay_edges(&input, false), GameplayEdges::default());
    }

    #[test]
    fn stopped_state_suppresses_gameplay_edges() {
        let mut input = InputState::default();
        input.key(Key::KeyA, true);
        input.key(Key::Enter, true);
        assert_eq!(gameplay_edges(&input, true), GameplayEdges::default());
    }

    #[test]
    fn arrow_aim_moves_and_clamps() {
        let mut input = InputState::default();
        input.key(Key::ArrowRight, true);
        assert_eq!(
            adjusted_aim(Vec2::new(128.0, 115.0), &input, 0.1),
            Vec2::new(144.0, 115.0)
        );
        input.key(Key::ArrowDown, true);
        assert_eq!(
            adjusted_aim(Vec2::new(128.0, 115.0), &input, 10.0),
            Vec2::new(247.0, 185.0)
        );

        let mut input = InputState::default();
        input.key(Key::ArrowLeft, true);
        input.key(Key::ArrowUp, true);
        assert_eq!(
            adjusted_aim(Vec2::new(128.0, 115.0), &input, 10.0),
            Vec2::new(8.0, 24.0)
        );
    }

    #[test]
    fn arcade_presentation_keeps_a_four_three_viewport() {
        let options = DisplayOptions {
            aspect: Some((4, 3)),
            filter: Filter::Nearest,
            scaling: Scaling::Fit,
        };
        let viewport = Viewport::calculate((WIDTH, HEIGHT), (1000, 500), options).unwrap();
        assert_eq!(
            (viewport.x, viewport.y, viewport.width, viewport.height),
            (167, 0, 666, 500)
        );
        assert_eq!(viewport.to_source(500.0, 250.0), Some((128.0, 115.5)));
        assert!(viewport.to_source(0.0, 0.0).is_none());
    }
}
