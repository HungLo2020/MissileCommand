#!/usr/bin/env python3
"""Create a Basalt-compatible release archive from an already built binary."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import stat
import time
import tomllib
import zipfile

from validate_package import validate_archive


ROOT = Path(__file__).resolve().parent.parent


def add_file(package: zipfile.ZipFile, source: Path, name: str, executable: bool) -> None:
    # A fixed timestamp and explicit Unix permissions make releases reproducible,
    # including archives produced by Windows runners.
    epoch = max(315532800, int(os.environ.get("SOURCE_DATE_EPOCH", "315532800")))
    entry = zipfile.ZipInfo(name, time.gmtime(epoch)[:6])
    entry.create_system = 3
    entry.external_attr = (stat.S_IFREG | (0o755 if executable else 0o644)) << 16
    entry.compress_type = zipfile.ZIP_DEFLATED
    package.writestr(entry, source.read_bytes())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/missile-command")
    parser.add_argument("--platform", choices=("linux-x64", "windows-x64", "macos-x64", "macos-arm64"), default="linux-x64")
    parser.add_argument("--output-dir", type=Path, default=ROOT / "dist")
    parser.add_argument("--version", help="Must match the package version in Cargo.toml")
    args = parser.parse_args()
    try:
        with (ROOT / "Cargo.toml").open("rb") as manifest:
            version = tomllib.load(manifest)["package"]["version"]
        if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?", version):
            raise ValueError("Cargo package.version is not a supported release version")
        if args.version is not None and args.version.removeprefix("v") != version:
            raise ValueError(f"release version {args.version!r} does not match Cargo version {version!r}")
        if not args.binary.is_file() or args.binary.stat().st_size == 0:
            raise ValueError(f"missing or empty binary: {args.binary}; run cargo build --release first")
        readme = ROOT / "README.md"
        if not readme.is_file():
            raise ValueError("README.md is required")
        cover = ROOT / "assets/cover.png"
        if not cover.is_file():
            raise ValueError("assets/cover.png is required")
        args.output_dir.mkdir(parents=True, exist_ok=True)
        archive = args.output_dir / f"MissileCommand-{version}-{args.platform}.zip"
        temporary = archive.with_suffix(".zip.tmp")
        executable = "app/bin/missile-command" + (".exe" if args.platform == "windows-x64" else "")
        try:
            with zipfile.ZipFile(temporary, "w") as package:
                add_file(package, args.binary, executable, True)
                add_file(package, readme, "app/README.md", False)
                for license_name in ("LICENSE", "LICENSE-MIT", "LICENSE-APACHE"):
                    source = ROOT / license_name
                    if source.is_file():
                        add_file(package, source, f"app/{license_name}", False)
                # Share the repository's original cover; ROMs are never packaged.
                add_file(package, cover, "app/basalt/cover.png", False)
                for filename in ("icon.png", "icon.svg"):
                    source = ROOT / "assets" / filename
                    if source.is_file():
                        add_file(package, source, f"app/basalt/{filename}", False)
            validate_archive(temporary, executable)
            temporary.replace(archive)
        finally:
            temporary.unlink(missing_ok=True)
    except (OSError, KeyError, TypeError, ValueError, tomllib.TOMLDecodeError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Packaging failed: {error}\n")
    print(archive)


if __name__ == "__main__":
    main()
