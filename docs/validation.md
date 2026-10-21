# Release validation

Validation performed on 2026-10-21 for the initial Linux x64 release.

| Check | Result |
|---|---|
| Formatting and Clippy | Passed with warnings treated as errors. |
| Rust tests | 32 passed: game rules, 40-wave configuration sweep, full-state deterministic replay, original-ROM movement samples, saves, synthesis and window/input regression checks. |
| Python package tests | Six passed, including traversal/permissions/archive layout rejection. |
| Native window in CI | Passed on its own private Xvfb: application class, PID, opacity metadata, resizing, controls, pause, normal keyboard quit and saved records. |
| Original game runtime | MAME 0.285 ran the supplied revision 1 ROM, started a player, fired two measured shots, and produced the recorded movement fixture. |
| Packaged executable | Ran from a relocated path containing spaces, with read-only app files and unrelated working directory. App hashes unchanged. |
| Recorded/replayed packaged run | 12,000 frames produced identical final framebuffer and state summary: wave 3, score 1985, game over. This driver is a test script, not a proof of full ROM equivalence. |
| Host audio | Built and launched through host `libasound.so.2`; sound synthesis bounded/decay test passed. No bundled audio runtime. |
| Basalt catalog | Installed Basalt CLI loaded the proposed entry in isolated XDG directories, downloaded the published v0.1.0 ZIP, installed it in a path containing spaces, backed up a save, and reinstalled without changing that save. Installed headless playthrough passed. [Result](evidence/basalt-install.json). Basalt repository untouched. |
| Basalt package contract | ZIP contains only app paths, executable Unix permission, launcher cover, no ROM/user data, no updater. |

The window regression is explained in [the reconstruction record](reconstruction.md).
All automated window interaction requires the harness's own private Xvfb,
including in CI. It does not connect to the user's desktop.

This verification supports the bounded behaviors above. Remaining game fidelity
limits are listed in [mechanics](mechanics.md); no universal cycle/pixel equivalence
claim is made.
