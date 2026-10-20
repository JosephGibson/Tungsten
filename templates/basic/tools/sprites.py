"""The game's sprites, drawn from filled shapes and written as RGBA PNG.

Each sprite is a list of shapes painted in order, each sampled 4 × 4 times
per pixel, so its edges are smooth; only arithmetic and comparisons decide
a pixel, so a render repeats pixel for pixel. `--check` compares decoded
pixels, since deflate's bytes differ between zlib builds. M36's
`player.png` is not drawn here.

    python3 -B tools/sprites.py            # write assets/sprites/
    python3 -B tools/sprites.py --out DIR  # write DIR instead
    python3 -B tools/sprites.py --check    # compare assets/sprites/ pixel for pixel
"""
import argparse
import math
import pathlib
import struct
import sys
import zlib

SPRITES = pathlib.Path(__file__).resolve().parent.parent / 'assets' / 'sprites'
GRID = 4
SIGNATURE = b'\x89PNG\r\n\x1a\n'


def circle(cx, cy, r):
    """A disc about (cx, cy)."""
    return lambda x, y: (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def box(x0, y0, x1, y1, radius=0.0):
    """A rectangle, its corners rounded by `radius`."""
    def inside(x, y):
        if not (x0 <= x <= x1 and y0 <= y <= y1):
            return False
        dx = max(x0 + radius - x, 0.0, x - (x1 - radius))
        dy = max(y0 + radius - y, 0.0, y - (y1 - radius))
        return dx * dx + dy * dy <= radius * radius
    return inside


def polygon(*points):
    """A polygon by its corners, even-odd."""
    edges = list(zip(points, points[1:] + points[:1]))

    def inside(x, y):
        hit = False
        for (ax, ay), (bx, by) in edges:
            if (ay > y) != (by > y) and x < ax + (y - ay) * (bx - ax) / (by - ay):
                hit = not hit
        return hit
    return inside


def render(width, height, layers):
    """`layers`, each (shape, (r, g, b, a)), painted in order over clear
    pixels; returns the RGBA rows."""
    rows = []
    for py in range(height):
        row = bytearray()
        for px in range(width):
            sums = [0.0, 0.0, 0.0, 0.0]
            for sy in range(GRID):
                for sx in range(GRID):
                    x, y = px + (sx + 0.5) / GRID, py + (sy + 0.5) / GRID
                    r = g = b = a = 0.0
                    for shape, (lr, lg, lb, la) in layers:
                        if shape(x, y):
                            alpha = la / 255
                            keep = 1 - alpha
                            r, g, b = lr * alpha + r * keep, lg * alpha + g * keep, lb * alpha + b * keep
                            a = alpha + a * keep
                    for i, value in enumerate((r, g, b, a)):
                        sums[i] += value
            count = GRID * GRID
            alpha = sums[3] / count
            if alpha <= 0:
                row += bytes(4)
                continue
            row += bytes(min(255, round(sums[i] / count / alpha)) for i in range(3))
            row.append(round(alpha * 255))
        rows.append(bytes(row))
    return rows


def encode(width, height, rows):
    """8-bit RGBA PNG bytes, every row unfiltered."""
    def chunk(kind, data):
        return (struct.pack('>I', len(data)) + kind + data
                + struct.pack('>I', zlib.crc32(kind + data)))
    header = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    raw = b''.join(b'\x00' + row for row in rows)
    return (SIGNATURE + chunk(b'IHDR', header) + chunk(b'IDAT', zlib.compress(raw, 9))
            + chunk(b'IEND', b''))


def decode(data):
    """An 8-bit RGBA, non-interlaced PNG's (width, height, rows), through any
    of the five row filters."""
    if data[:8] != SIGNATURE:
        raise ValueError('not a PNG')
    at, idat, header = 8, b'', None
    while at < len(data):
        length, kind = struct.unpack('>I4s', data[at:at + 8])
        body = data[at + 8:at + 8 + length]
        at += 12 + length
        if kind == b'IHDR':
            header = struct.unpack('>IIBBBBB', body)
        elif kind == b'IDAT':
            idat += body
    width, height, depth, color, _, _, interlace = header
    if (depth, color, interlace) != (8, 6, 0):
        raise ValueError('not 8-bit RGBA without interlace')
    raw, stride = zlib.decompress(idat), width * 4
    rows, above = [], bytes(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind, line = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = above[i]
            c = above[i - 4] if i >= 4 else 0
            p = a + b - c
            paeth = min((abs(p - a), 0, a), (abs(p - b), 1, b), (abs(p - c), 2, c))[2]
            line[i] = (line[i] + (0, a, b, (a + b) // 2, paeth)[kind]) & 255
        rows.append(bytes(line))
        above = line
    return width, height, rows


def bolt():
    """The player's shot: a pale core in an amber shell and a soft glow."""
    return 12, 12, [
        (circle(6, 6, 5.6), (255, 170, 60, 90)),
        (circle(6, 6, 4.2), (255, 196, 84, 255)),
        (circle(6, 6, 2.4), (255, 250, 220, 255)),
    ]


def dummy():
    """The target dummy: a ringed target, red on bone white."""
    rings = [(15.5, (58, 40, 34, 255))]
    for at, radius in enumerate((14.2, 11.6, 9.2, 6.8, 4.4, 2.2)):
        rings.append((radius, (206, 58, 58, 255) if at % 2 else (238, 232, 218, 255)))
    return 32, 32, [(circle(16, 16, r), color) for r, color in rings]


def gem():
    """A gold diamond, its crown lit and its right side shaded."""
    return 20, 20, [
        (polygon((10, 0.8), (18.6, 7.2), (10, 19.2), (1.4, 7.2)), (110, 72, 12, 255)),
        (polygon((10, 2.4), (16.8, 7.4), (10, 17.2), (3.2, 7.4)), (255, 196, 56, 255)),
        (polygon((10, 2.4), (16.8, 7.4), (3.2, 7.4)), (255, 226, 120, 255)),
        (polygon((10, 7.4), (16.8, 7.4), (10, 17.2)), (226, 150, 34, 255)),
        (circle(7.4, 5.6, 1.1), (255, 252, 230, 255)),
    ]


def heal_pickup():
    """A green kit with a white cross."""
    white = (250, 255, 250, 255)
    return 20, 20, [
        (box(0.8, 0.8, 19.2, 19.2, 5.0), (28, 92, 48, 255)),
        (box(2.2, 2.2, 17.8, 17.8, 3.8), (84, 200, 108, 255)),
        (box(3.4, 3.2, 16.6, 8.4, 2.6), (122, 222, 142, 255)),
        (box(8.2, 4.6, 11.8, 15.4, 0.8), white),
        (box(4.6, 8.2, 15.4, 11.8, 0.8), white),
    ]


def speed_pickup():
    """A blue badge with a lightning bolt."""
    flash = polygon((12.0, 3.0), (6.0, 11.0), (9.6, 11.0), (8.0, 17.2),
                    (14.4, 8.8), (10.6, 8.8), (12.6, 3.0))
    return 20, 20, [
        (circle(10, 10, 9.6), (24, 52, 128, 255)),
        (circle(10, 10, 8.4), (84, 140, 255, 255)),
        (circle(8.4, 7.6, 4.6), (122, 172, 255, 255)),
        (flash, (255, 248, 200, 255)),
    ]


def enemy():
    """A red spiked blob with angry eyes."""
    dark, ink = (120, 18, 26, 255), (70, 8, 14, 255)
    centre = (16.0, 16.5)

    def spike(turn):
        angle = turn * math.tau / 8 + math.tau / 16
        corner = lambda a, r: (centre[0] + r * math.cos(a), centre[1] + r * math.sin(a))
        return polygon(corner(angle - 0.33, 10.0), corner(angle, 15.2), corner(angle + 0.33, 10.0))
    layers = [(spike(turn), dark) for turn in range(8)]
    layers += [
        (circle(*centre, 12.0), dark),
        (circle(*centre, 10.8), (222, 52, 52, 255)),
        (circle(12.2, 11.8, 3.2), (246, 120, 110, 255)),
        (circle(12.2, 15.6, 3.0), (255, 246, 240, 255)),
        (circle(19.8, 15.6, 3.0), (255, 246, 240, 255)),
        (circle(12.9, 16.4, 1.5), (30, 10, 14, 255)),
        (circle(19.1, 16.4, 1.5), (30, 10, 14, 255)),
        (polygon((8.6, 11.6), (14.6, 13.4), (14.2, 14.6), (8.2, 12.8)), ink),
        (polygon((23.4, 11.6), (17.4, 13.4), (17.8, 14.6), (23.8, 12.8)), ink),
        (box(13.0, 21.0, 19.0, 22.6, 0.8), ink),
    ]
    return 32, 32, layers


DRAWN = {
    'bolt.png': bolt,
    'dummy.png': dummy,
    'gem.png': gem,
    'heal_pickup.png': heal_pickup,
    'speed_pickup.png': speed_pickup,
    'enemy.png': enemy,
}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true',
                        help='compare assets/sprites/ with a fresh render; write nothing')
    parser.add_argument('--out', type=pathlib.Path, default=SPRITES,
                        help='the folder to write, assets/sprites/ by default')
    args = parser.parse_args()
    drawn = {}
    for name, draw in DRAWN.items():
        width, height, layers = draw()
        drawn[name] = (width, height, render(width, height, layers))
    if args.check:
        failed = False
        for name, image in drawn.items():
            path = SPRITES / name
            try:
                matches = path.is_file() and decode(path.read_bytes()) == image
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
        (args.out / name).write_bytes(encode(width, height, rows))
    print(f'wrote {len(drawn)} sprites to {args.out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
