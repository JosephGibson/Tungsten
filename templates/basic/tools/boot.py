"""The boot splash's sprite: a white disc, written as an RGBA PNG.

`src/shell/boot.rs` draws everything on the boot screen but its text from
this one texture, the boot group's logo: a glow or an electron from the
whole disc, scaled and tinted, and a dot, a tail, the tile's card or the
load's hairline from its solid middle. The disc is 32 × 32 px, opaque to a
radius of 12 px and clear past 16, its alpha falling evenly between, so the
linear filter keeps its edge smooth drawn at a quarter of its size and soft
drawn larger; every pixel is white, so the filter blends no black into the
edge. `--check` compares decoded pixels, since deflate's bytes differ
between zlib builds.

    python3 -B tools/boot.py            # write assets/sprites/boot_dot.png
    python3 -B tools/boot.py --out DIR  # write DIR instead
    python3 -B tools/boot.py --check    # compare it pixel for pixel
"""

import argparse
import math
import pathlib
import struct
import sys
import zlib

sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import sprites  # noqa: E402

NAME = 'boot_dot.png'
SIZE = 32
# Opaque to SOLID px from the middle and clear from CLEAR; `boot.rs`'s
# `DISC` is the half size over the radius between them, 16 / 14.
SOLID, CLEAR = 12.0, 16.0


def disc():
    """The disc's rows: white, each pixel's alpha by its middle's distance."""
    rows = []
    for y in range(SIZE):
        row = bytearray()
        for x in range(SIZE):
            distance = math.hypot(x + 0.5 - SIZE / 2, y + 0.5 - SIZE / 2)
            alpha = min(1.0, max(0.0, (CLEAR - distance) / (CLEAR - SOLID)))
            row += bytes((255, 255, 255, round(alpha * 255)))
        rows.append(bytes(row))
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true',
                        help=f'compare assets/sprites/{NAME} with a fresh render; write nothing')
    parser.add_argument('--out', type=pathlib.Path, default=sprites.SPRITES,
                        help='the folder to write, assets/sprites/ by default')
    args = parser.parse_args()
    image = (SIZE, SIZE, disc())
    if args.check:
        path = sprites.SPRITES / NAME
        try:
            matches = path.is_file() and sprites.decode(path.read_bytes()) == image
        except (ValueError, zlib.error, struct.error) as error:
            print(f'{NAME}: unreadable: {error}', file=sys.stderr)
            return 1
        if not matches:
            print(f'{NAME}: differs from a fresh render', file=sys.stderr)
            return 1
        print(f'{NAME}: matches')
        return 0
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / NAME).write_bytes(sprites.encode(*image))
    print(f'wrote {NAME} to {args.out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
