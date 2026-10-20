#!/usr/bin/env python3
"""Deterministic offline authoring of the fixtures crate's UI sprites: the
window frame, the HUD icon and the inventory's ten item icons; never invoked
by Cargo or the engine. Standard library only: the PNGs are written by hand
(zlib + the PNG chunk layout), so no Pillow is needed.

    python3 -B examples/03_fixtures/tools/generate.py           # write the sprites
    python3 -B examples/03_fixtures/tools/generate.py --check   # compare bytes
"""
import argparse
import struct
import sys
import zlib
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
SPRITES = TOOLS.parent / "assets" / "sprites"
ITEMS = SPRITES / "items"

FRAME_SIZE = 48
FRAME_INSET = 12  # the nine-slice inset the manifest declares
ICON_SIZE = 32

# sRGB colours (r, g, b, a).
FRAME_EDGE = (124, 130, 160, 255)
FRAME_RIM = (62, 66, 90, 255)
FRAME_FILL = (34, 36, 48, 150)
FRAME_CORNER = (91, 141, 239, 255)
ICON_FILL = (91, 141, 239, 255)
ICON_EDGE = (240, 240, 245, 255)
CLEAR = (0, 0, 0, 0)


def png_bytes(width, height, pixels):
    """A PNG of `pixels`, a row-major list of (r, g, b, a), as bytes."""

    def chunk(kind, data):
        body = kind + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    raw = bytearray()
    for y in range(height):
        raw.append(0)  # filter: none
        for x in range(width):
            raw.extend(pixels[y * width + x])
    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def frame_pixels():
    """A 48×48 frame: a 2 px edge, a 1 px rim inside it, a translucent fill
    and accent marks in the four 12 px corners, so a stretched nine-slice
    shows whether the corners kept their size."""
    size = FRAME_SIZE
    out = []
    for y in range(size):
        for x in range(size):
            d = min(x, y, size - 1 - x, size - 1 - y)
            in_corner = (x < FRAME_INSET or x >= size - FRAME_INSET) and (
                y < FRAME_INSET or y >= size - FRAME_INSET
            )
            if d < 2:
                out.append(FRAME_EDGE)
            elif d == 2:
                out.append(FRAME_RIM)
            elif in_corner and 4 <= d <= 6:
                out.append(FRAME_CORNER)
            else:
                out.append(FRAME_FILL)
    return out


def icon_pixels():
    """A 32×32 diamond with a 2 px light edge on a transparent ground."""
    size = ICON_SIZE
    half = size / 2
    out = []
    for y in range(size):
        for x in range(size):
            d = abs(x + 0.5 - half) + abs(y + 0.5 - half)
            if d > half - 1:
                out.append(CLEAR)
            elif d > half - 3:
                out.append(ICON_EDGE)
            else:
                out.append(ICON_FILL)
    return out


# The item icons: 32×32, each a few flat shapes painted in order on a
# transparent ground. A shape is (kind, colour, *params) in pixel units.
def inside(shape, x, y):
    kind = shape[0]
    if kind == "circle":
        _, _, cx, cy, r = shape
        return (x - cx) ** 2 + (y - cy) ** 2 <= r * r
    if kind == "ellipse":
        _, _, cx, cy, rx, ry = shape
        return ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 <= 1.0
    if kind == "rect":
        _, _, x0, y0, x1, y1 = shape
        return x0 <= x < x1 and y0 <= y < y1
    if kind == "line":
        _, _, x0, y0, x1, y1, width = shape
        dx, dy = x1 - x0, y1 - y0
        t = max(0.0, min(1.0, ((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy)))
        px, py = x0 + t * dx, y0 + t * dy
        return (x - px) ** 2 + (y - py) ** 2 <= (width / 2) ** 2
    if kind == "ring":
        _, _, cx, cy, r, width = shape
        d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
        return abs(d - r) <= width / 2
    if kind == "arc":
        # The left half of a ring: a bow's limb.
        _, _, cx, cy, r, width = shape
        d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
        return abs(d - r) <= width / 2 and x <= cx
    if kind == "poly":
        _, _, points = shape
        hit = False
        for i in range(len(points)):
            (ax, ay), (bx, by) = points[i], points[i - 1]
            if (ay > y) != (by > y) and x < (bx - ax) * (y - ay) / (by - ay) + ax:
                hit = not hit
        return hit
    raise ValueError(kind)


def item_pixels(shapes):
    out = []
    for y in range(ICON_SIZE):
        for x in range(ICON_SIZE):
            color = CLEAR
            for shape in shapes:
                if inside(shape, x + 0.5, y + 0.5):
                    color = shape[1]
            out.append(color)
    return out


GLASS = (215, 225, 240, 255)
STEEL = (190, 196, 210, 255)
WOOD = (140, 92, 52, 255)
GOLD = (236, 190, 70, 255)

ITEM_SHAPES = {
    "draught": [
        ("rect", GLASS, 13, 4, 19, 12),
        ("rect", WOOD, 12, 2, 20, 5),
        ("circle", GLASS, 16, 20, 10),
        ("circle", (214, 64, 72, 255), 16, 21, 8),
    ],
    "sword": [
        ("line", STEEL, 9, 23, 27, 5, 4),
        ("line", (120, 126, 140, 255), 12, 20, 27, 5, 1.2),
        ("line", GOLD, 6, 15, 17, 26, 3),
        ("line", WOOD, 4, 28, 10, 22, 3),
    ],
    "cap": [
        ("ellipse", (150, 102, 60, 255), 16, 18, 11, 10),
        ("rect", CLEAR, 0, 18, 32, 32),
        ("rect", (110, 72, 40, 255), 3, 17, 29, 21),
    ],
    "lantern": [
        ("ring", (90, 90, 100, 255), 16, 7, 4, 2),
        ("rect", (70, 70, 80, 255), 9, 9, 23, 28),
        ("rect", (255, 214, 102, 255), 11, 11, 21, 26),
        ("circle", (255, 244, 200, 255), 16, 18, 3),
    ],
    "ore": [
        ("poly", (110, 110, 118, 255), [(5, 20), (10, 9), (20, 6), (28, 14), (26, 25), (13, 28)]),
        ("circle", (214, 132, 70, 255), 13, 15, 2.5),
        ("circle", (214, 132, 70, 255), 21, 19, 2),
        ("circle", (214, 132, 70, 255), 15, 23, 1.5),
    ],
    "herb": [
        ("line", (70, 120, 50, 255), 16, 29, 16, 12, 2),
        ("ellipse", (92, 176, 84, 255), 11, 13, 6, 9),
        ("ellipse", (120, 200, 100, 255), 21, 15, 6, 9),
    ],
    "elixir": [
        ("rect", GLASS, 13, 3, 19, 11),
        ("poly", GLASS, [(13, 10), (19, 10), (28, 28), (4, 28)]),
        ("poly", (80, 140, 255, 255), [(10, 17), (22, 17), (26, 26), (6, 26)]),
    ],
    "bow": [
        ("arc", WOOD, 24, 16, 13, 3),
        ("line", (230, 230, 230, 255), 24, 3.5, 24, 28.5, 1),
        ("line", STEEL, 7, 16, 27, 16, 1.5),
    ],
    "mail": [
        ("rect", (176, 196, 222, 255), 9, 8, 23, 28),
        ("poly", (176, 196, 222, 255), [(9, 8), (3, 14), (6, 18), (9, 15)]),
        ("poly", (176, 196, 222, 255), [(23, 8), (29, 14), (26, 18), (23, 15)]),
        ("rect", (120, 140, 170, 255), 13, 6, 19, 10),
        ("line", (140, 160, 190, 255), 9, 18, 23, 18, 1),
    ],
    "moonstone": [
        ("ring", (196, 170, 255, 255), 16, 16, 12, 2),
        ("circle", (228, 220, 255, 255), 16, 16, 8),
        ("circle", (255, 255, 255, 255), 13, 13, 2.5),
    ],
}

OUTPUTS = {
    "frame.png": lambda: png_bytes(FRAME_SIZE, FRAME_SIZE, frame_pixels()),
    "icon.png": lambda: png_bytes(ICON_SIZE, ICON_SIZE, icon_pixels()),
}
for _name, _shapes in ITEM_SHAPES.items():
    OUTPUTS[f"items/{_name}.png"] = (
        lambda shapes=_shapes: png_bytes(ICON_SIZE, ICON_SIZE, item_pixels(shapes))
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="compare the files on disk with what would be written")
    args = parser.parse_args()
    ITEMS.mkdir(parents=True, exist_ok=True)
    stale = []
    for name, make in OUTPUTS.items():
        path = SPRITES / name
        data = make()
        if args.check:
            if not path.exists() or path.read_bytes() != data:
                stale.append(name)
        else:
            path.write_bytes(data)
            print(f"wrote {path.relative_to(TOOLS.parent.parent.parent)} ({len(data)} bytes)")
    if stale:
        print("stale: " + ", ".join(stale), file=sys.stderr)
        return 1
    if args.check:
        print("up to date")
    return 0


if __name__ == "__main__":
    sys.exit(main())
