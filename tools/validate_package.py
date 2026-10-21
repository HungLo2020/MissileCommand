#!/usr/bin/env python3
"""Check a release ZIP against the Basalt standard without extracting it."""

from __future__ import annotations

import argparse
from pathlib import Path, PurePosixPath
import stat
import zipfile


def validate_archive(archive: Path, executable: str = "app/bin/missile-command") -> None:
    """Raise ValueError for an unsafe or incomplete Basalt release."""
    with zipfile.ZipFile(archive) as package:
        entries = package.infolist()
        if not entries:
            raise ValueError("archive is empty")
        seen: set[str] = set()
        files: set[PurePosixPath] = set()
        paths: set[PurePosixPath] = set()
        for entry in entries:
            path = PurePosixPath(entry.filename)
            if (
                path.is_absolute()
                or ".." in path.parts
                or "\\" in entry.filename
                or not path.parts
                or path.parts[0] != "app"
                or entry.filename != path.as_posix() + ("/" if entry.is_dir() else "")
            ):
                raise ValueError(f"unsafe or non-app archive path: {entry.filename!r}")
            if entry.filename in seen:
                raise ValueError(f"duplicate archive path: {entry.filename!r}")
            seen.add(entry.filename)
            paths.add(path)
            if not entry.is_dir():
                files.add(path)
            mode = entry.external_attr >> 16
            if stat.S_ISLNK(mode):
                raise ValueError(f"symlinks are not allowed: {entry.filename!r}")
            if entry.flag_bits & 1:
                raise ValueError("encrypted entries are not supported")
        for path in paths:
            if any(parent in files for parent in path.parents):
                raise ValueError(f"archive file also used as a directory: {path}")
        if executable not in seen:
            raise ValueError(f"missing executable: {executable}")
        binary = package.getinfo(executable)
        if binary.is_dir() or binary.file_size == 0:
            raise ValueError("executable is empty or is a directory")
        if not (binary.external_attr >> 16) & stat.S_IXUSR:
            raise ValueError("executable does not have its execute bit set")
        if "app/README.md" not in seen:
            raise ValueError("missing app/README.md")
        corrupt = package.testzip()
        if corrupt is not None:
            raise ValueError(f"corrupt ZIP member: {corrupt}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("--executable", default="app/bin/missile-command")
    args = parser.parse_args()
    try:
        validate_archive(args.archive, args.executable)
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Invalid release: {error}\n")
    print(f"Validated {args.archive}")


if __name__ == "__main__":
    main()
