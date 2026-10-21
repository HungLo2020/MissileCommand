# Simulation contract and evidence

The executable is a native Rust reconstruction. `src/game.rs` contains gameplay,
`src/waves.rs` contains wave configuration and palette data, and the renderer and
host services consume that state. There is no CPU emulator, runtime disassembler,
ROM loader, assembly interpreter, or ROM dependency in the finished game.

The target used here is the user's **arcade `missile1` revision 1 archive**.
The byte extraction and REA analysis are recorded in
[the ROM inventory](evidence/rom-inventory.json) and
[the revision 1 table extraction](evidence/rev1-tables.json).
The primary explanatory source is Atari's surviving program listing,
[historicalsource/missile-command at bab468c](https://github.com/historicalsource/missile-command/tree/bab468c2277e37bf9bc0ddc83786a58e6b609cde).
The [annotated disassembly](https://6502disassembly.com/va-missile-command/MissileCommand.html)
is useful for naming and understanding routines, but includes later-revision
addresses and patches; the user's program bytes take precedence.

## Rules implemented

| Rule | Implementation and evidence |
|---|---|
| Frame cadence | One deterministic update per video frame, `5,000,000 / 320 / 256 = 61.03515625 Hz`. Rendering has independent timing. |
| Coordinates | Native 256 × 231 visible raster, top-left origin. Source bottom-up coordinates map with `display_y = 230 - native_y`. |
| Cursor bounds | Horizontal 8–247, native vertical 45–206, hence display vertical 24–185. `DOCURS`, `HMIN/HMAX`, and `W3COMN.XX` constants. |
| Targets | Six cities and three batteries at original target-table locations. Batteries start each wave alive with ten interceptors. `HTARGS`, `VTARGS`, `MAXMIS`, `NEWWV1`. |
| Interceptors | Eight simultaneous interceptors, with defensive explosions plus airborne interceptors limited to sixteen. Side batteries move three integration steps per frame; the center moves seven. `UPABMS` loop includes zero, despite its comments saying two/six. |
| Missile paths | Signed integer 8.8 velocity. Path distance is `max(dx,dy) + floor(3*min(dx,dy)/8)`, capped at 255; integer-operation ordering follows `GENVEL`. |
| Enemy cadence | Original unsigned 8.8 frame delays, with the fractional remainder carried between movements. Delays reach zero on wave 15. `WICSPL`, `WICSPH`, `UPICBM`. |
| Enemy totals | Nineteen-entry ICBM and smart-bomb tables, clamped after wave 19. The supplied revision 1 ROM has **16, 18, 20** ICBMs at waves 17–19. This was checked directly at program address `$60c6`; online guide labels/counts disagree. |
| Target policy | Prefer living untargeted cities until three cities are destroyed or targeted; then prefer untargeted living batteries, followed by dead or already-targeted cities and batteries. `GUICBM`, `NOTLCI`. Repeated attacks can concentrate on the same three cities. |
| MIRVs | Incoming missiles in native altitude 128–159 may spawn up to three additional heads, subject to offense limits. Each new head consumes the current wave's remaining ICBM budget. `ICPOSI`, `MIRVER`. |
| Aircraft | One bomber or satellite at a time beginning on wave 2. Bombers move one pixel every three frames; satellites every two. Cooldown, altitude and fire-distance tables vary by wave. `ACTPLA`, `PROPLA`, `PLARAT`, `WSPLAU`, `WSPFIR`. The firing table is checked against pixels moved, rather than elapsed frames. |
| Smart bombs | Begin on wave 6. Eight probes at cardinal/diagonal offsets 8/6 pixels detect explosions; sixteen opening masks and original 8.8 evasion vectors select a route. `ANDANG`, `BIGHOL`, `FINDBL`, `FINSIN`, `CMDEVI`. Upward evasion is blocked **before wave 9**, matching the actual conditional branch rather than the wave guide's reversed prose. |
| Explosions | Twenty circularly allocated slots, four visited each frame in five descending batches. Radius animation and damage run only when that slot's batch is selected. The original integer octagonal distance is used for collisions. Peak radius is thirteen pixels. Missiles, smart bombs and aircraft add 1, 3 and 6 pixels respectively to the collision radius. `PREXPL`, `OLDRAD`, `NEWRAD`, `DAMAGE`, `DELTA`. |
| Chain reactions | Destroyed hostiles spawn explosions, which can destroy other hostiles. A child allocated to a later slot in the current batch can be processed during that same frame. Ground bursts below native altitude 33 cannot damage hostiles. `DAMAGE`, `DETONA`. |
| Scoring | ICBM 25, smart bomb 125, aircraft 100; all multiplied by 1× on waves 1–2, 2× on 3–4, through 6× at wave 11+. `SETICS`, `ICKILL`, `CMKILL`, `SPUTKI`. |
| Wave bonus | Five points per unused interceptor and 100 per surviving city, multiplied by the wave multiplier. Batteries repair and refill before the next wave. `ABMADD`, `ENDWV4`, `NEWWV1`. |
| Bonus cities | Default award every 10,000 points. Cities bank when all six are present, then restore randomly selected destroyed cities between waves. A banked city can continue play after all visible cities are lost. `BONLVL`, `CHEKBO`, `REGEN`. |
| Cleanup | Once ammo, airborne interceptors and explosions are exhausted, remaining hostiles and aircraft move at maximum cadence until the wave clears. This mode stays latched across subsequent ground bursts. Pending attacks stop after three cities are lost and all batteries are empty. `TALIVE`, `ALIVE`. |

## Reproducibility and limits

`Game::new(seed)`, `start()` and `update(Input)` have no filesystem, audio, window,
threading, wall-clock or operating-system RNG dependencies. Inputs specify an aim
point and three fire-button press edges. Complete state can be cloned and compared,
and the executable supports input recording/replay for reproducible investigations.

This is **rule- and table-informed reconstruction, not cycle-exact emulation**.
The original POKEY random stream is replaced with a seeded integer RNG. Smart-bomb
probes query explosion geometry instead of the original mutable framebuffer;
overlapping drawn/erased pixels can therefore produce different choices. The source
uses fixed hardware missile slots and snapshots of missile heights; compact Rust
entity vectors and current state can change tie order or a launch boundary by a
frame. Missile endpoints snap to their target rather than preserving the original
one-step overshoot. Collision coordinates use display integer pixels, so the
conversion of fractional native Y coordinates can differ at a one-pixel boundary.
Wave bonuses use a fixed four-second presentation and award their tally together,
rather than reproducing the cabinet's multi-stage count-up timings. Scores and
wave indices remain modern wide integers rather than reproducing overflow bugs.
The machine's coin accounting, DIP-switch variants, cocktail/two-player mode,
trackball electrical behavior and controller support are outside this release.

The test suite checks the selected ROM's numeric wave tables, ammunition and
capacity rules, center/side speeds, batch-delayed explosion damage, chain scoring,
ground-burst exclusion, blast growth/shrink, target allocation, bonus banking,
random city restoration, wave bonuses/rearming, cleanup latching, MIRV budget
accounting, aircraft cadence, smart-bomb evasion, and complete unattended game
over/restart. A forty-wave sweep exercises all offense configurations, and full
state replay checks deterministic behavior. These tests establish those rules;
they do not establish universal pixel or cycle equivalence with the ROM.
