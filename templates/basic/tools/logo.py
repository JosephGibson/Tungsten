"""The boot splash's logo and bar, drawn from filled shapes and written as RGBA PNG.

`boot_logo.png` is 768 × 256 px, its art inside a 24 px margin and clear
around it; the boot view draws it at half its size × the boot's scale and
centres it, so it has no anchor. `boot_bar.png` is 16 × 16 px and opaque;
the view stretches it to the load's progress. Each is a list of shapes
painted in order and sampled 4 × 4 times per pixel by `sprites.py`'s
sampler, and `--check` compares decoded pixels, as `sprites.py`'s does.
Until the logo is drawn, both are placeholders: a flat accent box inside the
margin and the accent square.

    python3 -B tools/logo.py            # write assets/sprites/boot_logo.png and boot_bar.png
    python3 -B tools/logo.py --out DIR  # write DIR instead
    python3 -B tools/logo.py --check    # compare them pixel for pixel
"""
import argparse
import pathlib
import struct
import sys
import zlib

sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import sprites  # noqa: E402

# The logo's size and the margin its art keeps clear, and the bar's size,
# in pixels: what the boot view's code depends on.
LOGO = (768, 256)
MARGIN = 24
BAR = (16, 16)
# The shell theme's accent.
ACCENT = (0xF0, 0xA8, 0x30, 255)


def logo():
    """The logo: a placeholder, a flat accent box filling the margin's
    inside."""
    width, height = LOGO
    box = sprites.box(MARGIN, MARGIN, width - MARGIN, height - MARGIN)
    return width, height, [(box, ACCENT)]


def bar():
    """The bar: the accent square, opaque to its edges."""
    width, height = BAR
    return width, height, [(sprites.box(0, 0, width, height), ACCENT)]


DRAWN = {'boot_logo.png': logo, 'boot_bar.png': bar}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true',
                        help='compare the files in assets/sprites/ with a fresh render; write nothing')
    parser.add_argument('--out', type=pathlib.Path, default=sprites.SPRITES,
                        help='the folder to write, assets/sprites/ by default')
    args = parser.parse_args()
    drawn = {}
    for name, draw in DRAWN.items():
        width, height, layers = draw()
        drawn[name] = (width, height, sprites.render(width, height, layers))
    if args.check:
        failed = False
        for name, image in drawn.items():
            path = sprites.SPRITES / name
            try:
                matches = path.is_file() and sprites.decode(path.read_bytes()) == image
            except (ValueError, zlib.error, struct.error) as error:
                print(f'{name}: unreadable: {error}', file=sys.stderr)
                matches = None
            if matches:
                print(f'{name}: matches')
            else:
                if matches is not None:
                    print(f'{name}: differs from a fresh render', file=sys.stderr)
                failed = True
        if failed:
            return 1
        print(f'{len(drawn)} sprites match')
        return 0
    args.out.mkdir(parents=True, exist_ok=True)
    for name, (width, height, rows) in drawn.items():
        (args.out / name).write_bytes(sprites.encode(width, height, rows))
    print(f'wrote {len(drawn)} sprites to {args.out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
