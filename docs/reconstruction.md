# Reconstruction record

The reference is the user's arcade `missile1.zip`, revision 1, selected after the
user approved switching from the originally requested Atari 2600 version. The
Rust executable implements game rules directly and needs no original ROM.

## Evidence and investigation

[Reverse Engineer Anything](https://github.com/morluto/rea) supplied the artifact
inventory and Ghidra analysis. The locally installed REA skill is version 42;
the analysis provider reported Ghidra 12.1.4 with language `6502:LE:16:default`.
The six revision 1 program chips were concatenated in address order, mapped to
`$5000–$7fff`, and inspected as a raw NMOS 6502 image. This mapping does not model
the arcade board, reset sequence, memory-mapped hardware, or runtime execution.

| Evidence | Finding and scope |
|---|---|
| [Archive inventory](evidence/rom-inventory.json) | REA inventory of the supplied ZIP; SHA-256 `48793c0ac27d20c21181ee15a33184c83e3d9c38bb1dffd0638fce76d9932593`. Contains revision 1 and later program chips. |
| [Program table extraction](evidence/rev1-tables.json) | Selected revision 1 image SHA-256 `4cfa3b0ea969bc721788a0e00f5c22466cfa02458de26599303de06100e4602c`; 12,288 bytes. Individual chip hashes match MAME's revision 1 definitions. The archive spells chip four `ln1`; MAME uses `lm1`. |
| [REA native evidence](evidence/rea-rev1.json) | Observed ICBM-kill assembly and bounded original data reads. The earlier overview reported 123 defined procedures; that count does not establish exhaustive semantic recovery. |
| REA `ev_ee54d1b844587f7f78f6fd0dc7df5402a603d473af987960cc68418516743730` | Exact 28-byte explosion-radius table at `$54c6`, complete read. |
| REA `ev_1b5a6bf3397d304bb8d7e9d981f667e0e8a6dcce874b5ab1147fe7696e6dfcc9` | Exact wave-count/delay/bomb table bytes starting at `$60c6`, complete 68-byte read. |
| REA `ev_6aba90487beb08f13a2298be91eb17f9ccafba07be0ec78c4adaf4fe5bc51e14` | Revision 1 ICBM kill assembly at `$555f`: target cleanup, trail erasure, altitude exclusion and score dispatch. |
| [MAME movement observation](evidence/mame-movement.json) | Actual revision 1 game under MAME 0.285. A vertical side shot advanced native Y 27,30,33,36,39,42; a center shot advanced 31,38,45,52,59,66. One round was deducted from each corresponding battery. The integration test compares Rust positions against these recorded values. |

Primary explanatory sources:

- [Atari's surviving source, pinned commit bab468c](https://github.com/historicalsource/missile-command/tree/bab468c2277e37bf9bc0ddc83786a58e6b609cde): drawing tables, motion, offense selection, explosion scheduling, scoring and game transitions.
- [MAME board implementation](https://github.com/mamedev/mame/blob/master/src/mame/atari/missile.cpp): chip identity, address mapping, video timing, and visible raster.
- [Annotated later-revision disassembly](https://6502disassembly.com/va-missile-command/MissileCommand.html) and [wave guide](https://6502disassembly.com/va-missile-command/wave-guide.html): explanatory cross-reference, checked against revision 1 bytes where revisions differ.
- [Basalt standard, pinned commit 516209f](https://github.com/HungLo2020/Basalt/blob/516209fc8489779cb322cc9b641a1487beff4ec3/STANDARDS.md): release and user-data contract. The Basalt repository was not modified.

The selected ROM corrects a misleading later-revision assumption: waves 17–19
use 16/18/20 ICBMs. Source inspection also resolves side/center interceptor loops
as 3/7 integration steps, and the wave-9 smart-bomb branch permits upward evasion
starting at wave 9. The flier firing interval counts horizontal movements, not
video frames. Explosion updates visit four of twenty slots per frame in five
batches; testing collisions every frame would make defenses too generous.

## Reproduce the reference probes

Static extraction requires Python only and a locally supplied archive:

```sh
python3 DevUtils/reference.py /path/to/missile1.zip --output /tmp/rev1-tables.json --image /tmp/rev1.bin
```

`DevUtils/mame_probe.lua` is a bounded, explicit original-machine input/RAM probe.
Run it only inside an isolated emulator/virtual display environment; it opens no
browser and uses no desktop-wide input tools. Set `MISSILE_REFERENCE_OUT` to an
existing absolute output directory and use MAME's `-autoboot_script` option. It
inserts two coins, starts one player, sets only cursor coordinates for vertical
measurement, fires one side and one center shot, and records per-frame missile
positions/ammunition. The original ROM is never included in a release.

## Verification and remaining limits

The [mechanics contract](mechanics.md) maps implemented behavior to individual
source routines and describes remaining differences. Tests cover complete state
replay, a forty-wave configuration sweep, unattended game over/restart, source
rules and numeric tables, and the recorded original-ROM movement probe.

This is a playable rule-informed reconstruction. It does **not** establish
complete pixel, random-stream or cycle equivalence. The seeded RNG replaces
POKEY randomness; some scheduling ties and fractional-coordinate boundaries can
differ. Smart-bomb probes use geometry rather than the mutable original video
memory. Bonus presentation, attract screens and single-player start/quit flows
are adapted for desktop use; the original two-player/cocktail and DIP-switch
variants are outside this release. Bomber/satellite silhouettes are reconstructed.
Audio effects are synthesized independently and are not a cycle-exact POKEY model.

Font, coastline, city bit patterns, ammo placement and palette tables were
recovered from the surviving drawing source. Software video redraws the frame
instead of reproducing hardware pixel-write side effects. The cover is original
procedural artwork generated by `DevUtils/artwork.py`.

## Windowing regression

The desktop check identified KWin's empty-application-class opacity rule as the
reason an otherwise opaque window appeared transparent. Quasar 0.2.5 now owns the
native identity and compositor hints: `WM_CLASS=missile-command/MissileCommand`, the
process ID, normal-window type, full opacity, and an opaque region that follows
resizing. Quasar applies the metadata before graphics initialization and reconciles
it after resizes, DPI changes, and fullscreen transitions. The game no longer carries
private Xlib/XCB code. The captured composited playfield was true RGB black after
the identity fix. Framebuffer and letterbox pixels also carry a fully opaque alpha
byte before Quasar's framebuffer presenter uploads them.

`DevUtils/window_smoke.py` always starts its own private Xvfb server, never uses the
caller's DISPLAY, scopes every X client to that server, and addresses keyboard
events only to the spawned game's verified PID. It needs a Vulkan 1.3 driver; CI
installs Mesa's lavapipe software driver on the smoke runner. CI exercises resize,
opaque metadata, controls, pause, normal quit and persistence there, including a
read-only app tree and launch from an unrelated working directory. No desktop
settings are changed by the game or these tests.
