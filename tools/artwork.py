#!/usr/bin/env python3
"""Rebuild the original procedural launcher cover; needs Pillow, no game ROM.

Run from any directory: python3 tools/artwork.py
The bundled PNG is the release asset. Pillow/fonts are build-time tools only.
"""
from pathlib import Path
import math
import random

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parents[1]
WIDTH, HEIGHT = 900, 1200
NAVY = (6, 13, 26)
CYAN = (79, 233, 239)
ORANGE = (255, 101, 54)


def font(size: int) -> ImageFont.FreeTypeFont:
    candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSansCondensed-Bold.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "C:/Windows/Fonts/arialbd.ttf",
    ]
    for candidate in candidates:
        if Path(candidate).is_file():
            return ImageFont.truetype(candidate, size)
    raise SystemExit("Install DejaVu Sans or edit the font candidates in artwork.py")


def centered(draw, y, text, size, fill):
    face = font(size)
    box = draw.textbbox((0, 0), text, font=face)
    draw.text(((WIDTH - (box[2] - box[0])) / 2, y - box[1]), text, font=face, fill=fill)


def main():
    image = Image.new("RGB", (WIDTH, HEIGHT), NAVY)
    pixels = image.load()
    # Bounded, deterministic illumination keeps the PNG small and reproducible.
    for y in range(HEIGHT):
        for x in range(WIDTH):
            blue = max(0, 1 - math.hypot((x - 490) / 620, (y - 650) / 560))
            warm = max(0, 1 - math.hypot((x - 490) / 330, (y - 690) / 330))
            pixels[x, y] = (6 + int(18 * warm), 13 + int(19 * blue), 26 + int(27 * blue))
    draw = ImageDraw.Draw(image)
    # Printed border and corner registration marks.
    draw.rectangle((31, 31, 868, 1168), outline=(41, 73, 89), width=2)
    for x, sx in [(31, 1), (868, -1)]:
        for y, sy in [(31, 1), (1168, -1)]:
            draw.line((x, y, x + sx * 35, y), fill=CYAN, width=3)
            draw.line((x, y, x, y + sy * 35), fill=CYAN, width=3)
    centered(draw, 69, "A NATIVE RUST RECONSTRUCTION", 19, CYAN)
    centered(draw, 121, "MISSILE", 133, (235, 246, 245))
    centered(draw, 252, "COMMAND", 111, (235, 246, 245))
    draw.line((93, 393, 807, 393), fill=ORANGE, width=4)
    centered(draw, 421, "SIX CITIES. THREE BATTERIES.", 22, (179, 204, 214))

    rng = random.Random(1980)
    for _ in range(65):
        x, y = rng.randint(63, 837), rng.randint(483, 933)
        draw.point((x, y), fill=(75, 103, 116))

    # A faceted defensive burst, drawn from geometric primitives.
    center = (495, 722)
    for radius, color in [(143, (48, 76, 86)), (118, (226, 91, 51)),
                          (91, (255, 170, 68)), (63, (255, 217, 126))]:
        points = [(center[0] + math.cos(math.pi / 8 + i * math.pi / 4) * radius,
                   center[1] + math.sin(math.pi / 8 + i * math.pi / 4) * radius)
                  for i in range(8)]
        draw.polygon(points, outline=color, width=3 if radius == 143 else 7)
    draw.ellipse((475, 702, 515, 742), fill=(255, 235, 178))

    # Threats converge on a stylised skyline; no cabinet or ROM artwork is used.
    for start, end in [((180, 490), (355, 869)), ((765, 476), (591, 758)),
                       ((628, 480), (505, 697)), ((72, 587), (155, 907))]:
        draw.line((*start, *end), fill=(102, 47, 38), width=8)
        draw.line((*start, *end), fill=ORANGE, width=2)
        x, y = end
        draw.rectangle((x - 3, y - 3, x + 3, y + 3), fill=(255, 226, 155))
    for origin, target in [((110, 1028), (436, 771)), ((450, 1028), (495, 722)),
                            ((790, 1028), (608, 650))]:
        draw.line((*origin, *target), fill=CYAN, width=3)
        x, y = target
        draw.line((x - 9, y, x + 9, y), fill=CYAN, width=2)
        draw.line((x, y - 9, x, y + 9), fill=CYAN, width=2)
        draw.ellipse((x - 13, y - 13, x + 13, y + 13), outline=CYAN, width=1)

    draw.line((62, 1030, 838, 1030), fill=CYAN, width=2)
    for i in range(6):
        cx = 202 + i * 98
        for j in range(6):
            x = cx + j * 9
            h = rng.randint(13, 61)
            draw.rectangle((x, 1028 - h, x + 6, 1028), fill=(18, 40, 51), outline=(62, 141, 155))
            for window_y in range(1035 - h, 1024, 9):
                draw.point((x + 3, window_y), fill=CYAN)
    for x in (110, 450, 790):
        draw.polygon([(x - 37, 1030), (x - 20, 1008), (x + 20, 1008), (x + 37, 1030)],
                     fill=(17, 51, 62), outline=CYAN, width=2)
        draw.rectangle((x - 5, 1001, x + 5, 1011), fill=(200, 241, 238))

    centered(draw, 1090, "DEFEND THE LAST LIGHTS", 27, (235, 246, 245))
    centered(draw, 1135, "ARCADE  /  KEYBOARD + MOUSE", 15, (105, 151, 165))
    output = ROOT / "assets" / "cover.png"
    output.parent.mkdir(parents=True, exist_ok=True)
    image.save(output, optimize=True)
    print(f"{output}: {WIDTH}x{HEIGHT}, {output.stat().st_size:,} bytes")


if __name__ == "__main__":
    main()
