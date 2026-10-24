# Missile Command

A native Rust reconstruction of **Atari's 1980 arcade game**, using the supplied revision 1 ROM as the reference. The game runs its own typed Rust simulation; it does not emulate a CPU or load a ROM. The desktop layer is built on [Quasar](https://github.com/HungLo2020/Quasar) (`quasar-window`, `quasar-input`, `quasar-render-2d`, `quasar-storage`, and `quasar-audio`), while the game keeps its deterministic software renderer, input replays, and Basalt-compatible releases.

The original request changed from Atari 2600 to arcade after identifying the supplied `missile1.zip`. This project targets the arcade version. See [the reconstruction record](docs/reconstruction.md) for evidence, measured rules, deliberate changes, and remaining fidelity limits. This is a playable reconstruction, not a claim of complete ROM equivalence.

## Play

On Linux, unpack the release ZIP and run `app/bin/missile-command`. The app needs a desktop session and a Vulkan 1.3 driver with dynamic rendering, synchronization2, and timeline semaphores, plus the host's ALSA library (`libasound.so.2`) for Quasar's CPAL audio backend. There is no bundled audio server, GPU runtime, or updater. A normal launch opens a window without terminal input or network access.

| Control | Action |
|---|---|
| Mouse or arrow keys | Aim |
| A / S / D | Fire left / center / right silo |
| Left / middle / right mouse button | Fire corresponding silo |
| Enter | Start; resume when paused |
| P or Escape | Pause / resume |
| F1 | Help |
| M | Mute |
| Q while paused | Quit |

Each silo holds 10 missiles. The center silo fires faster. Chain explosions to intercept incoming missiles; surviving cities and unused ammunition score at each wave's end. Earn a replacement city every 10,000 points. Silos rearm between waves. Controller support is deferred.

Quasar 0.2.5 owns native X11/Wayland window identity and compositor hints, including the X11 window class, process ID, normal-window type, full opacity, and an opaque region that follows resizing. The resizable window preserves a 4:3 display with nearest-neighbor sampling and opaque black letterboxing. Losing focus pauses play and releases held input. Audio uses the default CPAL output; select another with `--audio-device NAME`. Audio device failure leaves the game playable.

## Build and verify

Requires stable Rust 1.94+ (the Quasar workspace MSRV), edition 2024, and a C++ compiler with CMake and Ninja/Make for `shaderc`. Linux build packages: `build-essential cmake ninja-build pkg-config libxkbcommon-dev libxkbcommon-x11-0 libasound2-dev libwayland-dev libx11-dev libxrandr-dev libxi-dev libxcursor-dev libvulkan-dev`. The release workflow builds on Ubuntu 22.04 to retain its glibc baseline and exercises the native window separately on a newer runner with Mesa's software Vulkan driver. Linux x64 is the verified release target.

```sh
cargo run --release
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo test --locked --no-default-features
python3 DevUtils/verify_engine.py
# Optional native-window checks always create their own private Xvfb display:
python3 DevUtils/window_smoke.py
python3 -m unittest discover -s DevUtils -p 'test_*.py'
```

The simulation has no platform or I/O dependencies. Building with `--no-default-features` produces a headless binary without winit, Vulkan, or audio dependencies. A fixed 61.03515625 Hz step follows the arcade video timing. Missile motion uses 8.8 fixed point, and a seeded generator makes replays reproducible within the same implementation version.

```sh
cargo run --release --no-default-features -- --headless --seed 42 --ticks 3600 --autoplay --record /tmp/flight.json --snapshot /tmp/flight.ppm
cargo run --release --no-default-features -- --headless --ticks 3600 --replay /tmp/flight.json --snapshot /tmp/replay.ppm
cmp /tmp/flight.ppm /tmp/replay.ppm
```

`--autoplay` is a test driver, not a gameplay mode enabled by default. `--record FILE` also records real keyboard/mouse play, including every simulation input. A replay runs for its recorded frame count; `--ticks` does not append default inputs after the replay ends. `--snapshot FILE` writes a native 256×231 PPM. `--smoke-frames N` bounds an actual window run. `--no-audio` disables playback for unattended checks.

## Releases and launcher integration

```sh
cargo build --release --locked
python3 DevUtils/package.py
python3 DevUtils/validate_package.py dist/MissileCommand-0.1.0-linux-x64.zip
```

Archives contain only `app/`, with the executable at `app/bin/missile-command` and a 3:4 launcher cover at `app/basalt/cover.png`. A `v<version>` tag runs the release workflow. See [Basalt integration](docs/basalt.md) and the [proposed catalog entry](packaging/MissileCommand.json). No Basalt repository changes are necessary to build or run this game.

High scores live in `data/save/records.json`; mute preferences live in `data/config/shared/preferences.json`. Quasar's storage helpers provide atomic writes and Basalt directory resolution, with install-relative paths outside Basalt. Runtime never writes under `app/`. Corrupt or unsupported saves are preserved rather than replaced. The Quasar crates are pinned to `v0.2.5`; `DevUtils/verify_engine.py` rejects a mixed tag or local path dependency.

## Code map

- `src/game.rs`, `src/waves.rs`: deterministic simulation and revision 1 rules.
- `src/render.rs`: palette and bitmap software rendering, plus RGBA8 presentation output.
- `src/audio.rs`: procedural effects rendered as Quasar procedural sources on the CPAL backend.
- `src/storage.rs`: versioned records and preferences persisted through `quasar-storage`.
- `src/main.rs`: CLI, deterministic headless runner, replay recording, and PPM output.
- `src/desktop.rs`: Quasar window/input/framebuffer-presenter loop, fixed-step scheduling, and pause/help handling.
- `DevUtils/`: development tools for ROM evidence extraction, packaging/validation, artwork, engine-tag verification, and native-window checks.

The project includes no ROM. Original Rust code and cover art are MIT licensed; historical Atari program and audiovisual material remain their owners' property.

## AGENTS

This project follows the [HungLo2020 coding standards](https://github.com/HungLo2020/LinuxScripts/blob/master/CODING-STANDARDS.md).
