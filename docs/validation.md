# Release validation

Validation performed on 2026-10-23 after the Quasar desktop migration.

| Check | Result |
|---|---|
| Formatting and Clippy | Passed with warnings treated as errors on Rust 1.94. |
| Rust tests | 37 passed: game rules, 40-wave configuration sweep, full-state deterministic replay, replay exhaustion, original-ROM movement samples, input-edge mapping and aim clamping, 4:3 viewport mapping, Quasar storage migration/atomic replacement, procedural audio and offline Quasar mixing, and RGBA8 presentation output. |
| Headless build | `cargo test --locked --no-default-features` passes without winit, Vulkan, or audio dependencies. |
| Python package tests | Six passed, including traversal/permissions/archive layout rejection. |
| Quasar pin | `DevUtils/verify_engine.py` confirms six direct Quasar crates and all transitive engine crates resolve to `v0.2.5` at one Git commit. |
| Native window | Quasar's winit/Vulkan path presented frames on a desktop with an RTX 2070. Quasar 0.2.5 set `WM_CLASS=missile-command/MissileCommand`, `_NET_WM_PID`, `_NET_WM_WINDOW_TYPE_NORMAL`, full opacity, and a `1024×768` opaque region. |
| Native window in CI | Configured for a private Xvfb server with Mesa lavapipe: application class, PID, opacity metadata, resizing, controls, pause, normal keyboard quit and saved records. |
| Original game runtime | MAME 0.285 ran the supplied revision 1 ROM, started a player, fired two measured shots, and produced the recorded movement fixture. |
| Packaged executable | The migrated package ran from a read-only `app/` tree with an unrelated working directory and wrote records/preferences under `BASALT_GAME_DIR`. The earlier pre-Quasar package also passed a relocated path containing spaces; the archive contract is unchanged. |
| Recorded/replayed headless run | A 3,600-tick `--no-default-features` autoplay recording replayed to an identical PPM and state summary: wave 3, score 1985, three cities, playing. This driver is a test script, not a proof of full ROM equivalence. |
| Host audio | Quasar's CPAL backend opened the default output and played procedural effects. The offline Quasar mixer test renders the same source without a device. No bundled audio runtime. |
| Basalt catalog | The proposed catalog entry and packaging contract are unchanged. The recorded v0.1.0 install/backup/reinstall evidence predates the Quasar migration and still applies to the archive layout and Basalt actions. [Result](evidence/basalt-install.json). |
| Basalt package contract | ZIP contains only app paths, executable Unix permission, launcher cover, no ROM/user data, no updater. |

The window regression is explained in [the reconstruction record](reconstruction.md).
All automated window interaction requires the harness's own private Xvfb and a
Vulkan 1.3 driver, including in CI. It does not connect to the user's desktop.

This verification supports the bounded behaviors above. Remaining game fidelity
limits are listed in [mechanics](mechanics.md); no universal cycle/pixel equivalence
claim is made.
