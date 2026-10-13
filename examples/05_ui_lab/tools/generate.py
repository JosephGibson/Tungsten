#!/usr/bin/env python3
"""Deterministic offline authoring of example 05's two sprites; never invoked
by Cargo or the engine. Standard library only: the PNGs are written by hand
(zlib + the PNG chunk layout), so no Pillow is needed.

    python3 -B examples/05_ui_lab/tools/generate.py           # write the sprites
    python3 -B examples/05_ui_lab/tools/generate.py --check   # compare bytes
"""
import argparse
import struct
import sys
import zlib
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
SPRITES = TOOLS.parent / "assets" / "sprites"

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


OUTPUTS = {
    "frame.png": lambda: png_bytes(FRAME_SIZE, FRAME_SIZE, frame_pixels()),
    "icon.png": lambda: png_bytes(ICON_SIZE, ICON_SIZE, icon_pixels()),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="compare the files on disk with what would be written")
    args = parser.parse_args()
    SPRITES.mkdir(parents=True, exist_ok=True)
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
