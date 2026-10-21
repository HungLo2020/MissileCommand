# Basalt release integration

The packaging target is [Basalt Game Standard version 1](https://github.com/HungLo2020/Basalt/blob/516209fc8489779cb322cc9b641a1487beff4ec3/STANDARDS.md),
with catalog schema version 2, reviewed at Basalt revision
`516209fc8489779cb322cc9b641a1487beff4ec3`. No changes to the Basalt repository are required or made here.

Build and package on Linux x64 using Rust and Python 3.11 or newer:

```sh
cargo build --locked --release
python3 tools/package.py
python3 tools/validate_package.py dist/MissileCommand-0.1.0-linux-x64.zip
```

The archive name uses `package.version` from `Cargo.toml`. Its only root entry is `app/`:

```text
app/
  bin/missile-command
  README.md
  LICENSE
  basalt/cover.png
```

The packager records Unix executable permissions explicitly. It includes no ROMs, user data,
development files, or updater. The validator checks safe paths, duplicate entries, symlinks,
executable presence and permissions, the README, and ZIP integrity before an archive becomes final.
`SOURCE_DATE_EPOCH` optionally controls ZIP timestamps; the default is fixed for reproducibility.

`packaging/MissileCommand.json` is the proposed Linux catalog entry. It uses Basalt's default
layout and maintenance actions. It is deliberately kept in this repository; adding it to a local
Basalt catalog or the Basalt repository is a separate action. To try it locally:

```sh
mkdir -p ~/.config/basalt/games
cp packaging/MissileCommand.json ~/.config/basalt/games/
basalt catalog
basalt install MissileCommand --library default
basalt actions MissileCommand
```

The installed Basalt CLI successfully loaded this entry from an isolated temporary XDG config
and exposed all seven standard maintenance actions. Full download/install validation follows a
published matching GitHub Release. Basalt launches `app/bin/missile-command`
from the install root without arguments. The runtime honors `BASALT_GAME_DIR`, `BASALT_SAVE_DIR`,
`BASALT_CONFIG_DIR`, `BASALT_SHARED_CONFIG_DIR`, and `BASALT_CACHE_DIR`. Without those variables it
derives the install root from the executable's `app/bin` location, independently of the working
directory. It creates missing data directories, stores versioned high-score records in
`data/save/records.json`, and mute preferences in `data/config/shared/preferences.json`. Writes use
a temporary file in the same directory followed by atomic replacement. Missing fields from older
records receive defaults; invalid or newer-version records are left untouched. The runtime never
writes into `app/` and contains no updater.

The Linux executable uses the host's X11 desktop libraries and ALSA (`libasound.so.2`) as requested;
it does not bundle an audio server. These are normal desktop runtime dependencies, and development
headers are needed only to build. ALSA device failure leaves the game playable without audio.
The release starts without a ROM, arguments, terminal input, or network access. Controller support
is outside the current scope.

The CI workflow runs formatting, Clippy, Rust and archive tests, a release build, and native-window
smoke checks under Xvfb, then uploads the validated Linux archive as a workflow artifact.
A pushed `v<version>` tag runs those checks again, verifies
that the tag matches `Cargo.toml`, and publishes the ZIP as a GitHub Release asset. A manual run
of the release workflow creates an artifact without publishing a release. Builds use Ubuntu 22.04
to keep the Linux glibc baseline conservative; compatibility still requires testing on each supported
desktop distribution. Windows and macOS are not currently built or claimed as release targets.

The original `assets/cover.png` is included as `app/basalt/cover.png`: 900×1200, opaque PNG,
3:4 portrait, under 2 MB. The catalog's `artwork.cover_url` points to that same image on the main
branch for pre-install display. Optional square `assets/icon.png` or `assets/icon.svg` files are
also copied into `app/basalt/` when present. No ROM artwork is sourced automatically.
