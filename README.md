# Missile Command

A native Rust reconstruction of **Atari's 1980 arcade game**, using the supplied revision 1 ROM as the reference. The game runs its own typed Rust simulation; it does not emulate a CPU or load a ROM. Keyboard and mouse controls, software pixel rendering, host ALSA audio, deterministic input replays, and Basalt-compatible releases are included.

The original request changed from Atari 2600 to arcade after identifying the supplied `missile1.zip`. This project targets the arcade version. See [the reconstruction record](docs/reconstruction.md) for evidence, measured rules, deliberate changes, and remaining fidelity limits. This is a playable reconstruction, not a claim of complete ROM equivalence.

## Play

On Linux, unpack the release ZIP and run `app/bin/missile-command`. The app needs a desktop session and the host's ALSA library (`libasound.so.2`); there is no bundled audio server or updater. A normal launch opens a window without terminal input or network access.

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

Linux uses X11 (including XWayland on Wayland desktops). The resizable window preserves a 4:3 display with nearest-neighbor sampling and opaque black letterboxing. Losing focus pauses play. Audio uses ALSA's `default` device; select another with `--audio-device NAME`. Audio device failure leaves the game playable.

## Build and verify

Requires stable Rust with edition 2024 support (Rust 1.85+) and a C compiler. Linux build packages: `pkg-config libasound2-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev`. The release workflow builds on Ubuntu 22.04 to retain its glibc baseline. Linux x64 is the verified release target.

```sh
cargo run --release
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
# Optional native-window checks always create their own private Xvfb display:
python3 tools/window_smoke.py
python3 -m unittest discover -s tools -p 'test_*.py'
```

The simulation has no platform or I/O dependencies. A fixed 61.03515625 Hz step follows the arcade video timing. Missile motion uses 8.8 fixed point, and a seeded generator makes replays reproducible within the same implementation version.

```sh
cargo run --release -- --headless --seed 42 --ticks 3600 --autoplay --record /tmp/flight.json --snapshot /tmp/flight.ppm
cargo run --release -- --headless --ticks 3600 --replay /tmp/flight.json --snapshot /tmp/replay.ppm
cmp /tmp/flight.ppm /tmp/replay.ppm
```

`--autoplay` is a test driver, not a gameplay mode enabled by default. `--record FILE` also records real keyboard/mouse play, including every simulation input. `--snapshot FILE` writes a native 256×231 PPM. `--smoke-frames N` bounds an actual window run. `--no-audio` disables playback for unattended checks.

## Releases and launcher integration

```sh
cargo build --release --locked
python3 tools/package.py
python3 tools/validate_package.py dist/MissileCommand-0.1.0-linux-x64.zip
```

Archives contain only `app/`, with the executable at `app/bin/missile-command` and a 3:4 launcher cover at `app/basalt/cover.png`. A `v<version>` tag runs the release workflow. See [Basalt integration](docs/basalt.md) and the [proposed catalog entry](packaging/MissileCommand.json). No Basalt repository changes are necessary to build or run this game.

High scores live in `data/save/records.json`; mute preferences live in `data/config/shared/preferences.json`. Writes are atomic. All Basalt directory variables are honored, with install-relative paths outside Basalt. Runtime never writes under `app/`. Corrupt or unsupported saves are preserved rather than replaced.

## Code map

- `src/game.rs`, `src/waves.rs`: deterministic simulation and revision 1 rules.
- `src/render.rs`: palette and bitmap software rendering.
- `src/audio.rs`: procedural effects and host ALSA worker.
- `src/storage.rs`: versioned persistence and install paths.
- `src/main.rs`, `src/desktop.rs`, `src/input.rs`: window identity, input edges, fixed-step loop, recording/replay.
- `tools/reference.py`: inspect a locally supplied ROM and reproduce static evidence.

The project includes no ROM. Original Rust code and cover art are MIT licensed; historical Atari program and audiovisual material remain their owners' property.
