#!/usr/bin/env python3
"""Identify the supplied arcade ROM and extract bounded mechanical evidence.

No ROM is downloaded or copied into the repository. The optional derived image
is for REA/Ghidra analysis only; the Rust game never loads it.
"""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

CHIPS = [
    ("35820-01.h1", "5dcb58276c08d75d36baadb6cefe30d4916de9b0"),
    ("35821-01.jk1", "6f25af7133d3ec79029117162649f94e93f36e0e"),
    ("35822-01.kl1", "eb52b11c6757c8dc3be88b276ea4dc7dfebf7cf7"),
    ("35823-01.ln1", "f0daa5d2835a856e2038612e755dc7ded28fc923"),
    ("35824-01.np1", "7f69ee990fd4fa1f2fceca7fc66fcaa02e4d2314"),
    ("35825-01.r1", "03cabbef50c33852fbbf38dd3eecaf70a82df82f"),
]

def inspect(archive: Path):
    images = []
    identities = []
    with zipfile.ZipFile(archive) as package:
        for name, expected in CHIPS:
            # MAME names the fourth chip lm1; this supplied archive uses ln1.
            if name not in package.namelist() and name.endswith("ln1"):
                name = name.replace("ln1", "lm1")
            image = package.read(name)
            digest = hashlib.sha1(image).hexdigest()
            if len(image) != 2048 or digest != expected:
                raise ValueError(f"Unexpected revision 1 chip identity: {name}")
            images.append(image)
            identities.append({"name": name, "size": len(image), "sha1": digest})
    rom = b"".join(images)
    def table(address, length):
        return list(rom[address - 0x5000:address - 0x5000 + length])
    evidence = {
        "format": "MissileCommand reference evidence v1",
        "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
        "rom_sha256": hashlib.sha256(rom).hexdigest(),
        "load_address": "0x5000", "length": len(rom), "chips": identities,
        "tables": {
            "icbm_counts": {"address": "0x60c6", "values": table(0x60c6, 19)},
            "missile_delay_low": {"address": "0x60d9", "values": table(0x60d9, 15)},
            "missile_delay_high": {"address": "0x60e8", "values": table(0x60e8, 15)},
            "smart_bombs": {"address": "0x60f7", "values": table(0x60f7, 19)},
            "explosion_radii": {"address": "0x54c6", "values": table(0x54c6, 28)},
        },
        "limitations": ["Static table extraction establishes these bytes, not full behavioral equivalence.",
                         "Timing, scheduler, rendering, sound and random sequences need separate runtime evidence."],
    }
    return evidence, rom


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rom_zip", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--image", type=Path, help="Write derived raw image outside the repository for REA")
    args = parser.parse_args()
    try:
        evidence, rom = inspect(args.rom_zip)
        rendered = json.dumps(evidence, indent=2) + "\n"
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(rendered)
        else:
            print(rendered, end="")
        if args.image:
            args.image.write_bytes(rom)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        parser.exit(1, f"Reference inspection failed: {error}\n")


if __name__ == "__main__":
    main()
