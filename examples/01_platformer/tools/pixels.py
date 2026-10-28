"""Palette pixel toolkit shared by the actor, terrain, scenery and backdrop art.

Art is assembled as `{(x, y): symbol}` dictionaries: parts are drawn as masks,
shaded from their own silhouettes, layered with dark rims where they overlap and
finally converted into RGBA images through the palette.
"""
import hashlib
import math

from PIL import Image, ImageDraw

from art import PALETTE

NEIGHBOURS = ((1, 0), (-1, 0), (0, 1), (0, -1))
BAYER = ((0, 8, 2, 10), (12, 4, 14, 6), (3, 11, 1, 9), (15, 7, 13, 5))


def add(a, b, scale=1.0):
    return (a[0] + b[0] * scale, a[1] + b[1] * scale)


def point(p):
    return (round(p[0]), round(p[1]))


def smoothstep(u):
    u = min(1.0, max(0.0, u))
    return u * u * (3 - 2 * u)


def unit(*keys):
    """Deterministic value in [0, 1) for any tuple of names and integers."""
    digest = hashlib.sha256(repr(keys).encode()).digest()
    return int.from_bytes(digest[:8], 'big') / 2 ** 64


def dither(x, y, value, step=1.0):
    """Ordered-dither threshold: True when `value` (0..1) wins at this pixel."""
    return value * step > (BAYER[y % 4][x % 4] + 0.5) / 16


class Part:
    """A mask drawn with PIL, later shaded into palette symbols."""

    def __init__(self, size=(64, 64)):
        self.size = size
        self.mask = Image.new('L', size, 0)
        self.draw = ImageDraw.Draw(self.mask)

    def polygon(self, points):
        self.draw.polygon([point(p) for p in points], fill=255)

    def ellipse(self, center, rx, ry):
        x, y = center
        self.draw.ellipse((round(x - rx), round(y - ry), round(x + rx), round(y + ry)), fill=255)

    def rectangle(self, box):
        self.draw.rectangle(tuple(round(v) for v in box), fill=255)

    def limb(self, points, width):
        pts = [point(p) for p in points]
        self.draw.line(pts, fill=255, width=width, joint='curve')
        radius = (width - 1) / 2
        for x, y in pts:
            self.draw.ellipse((x - radius, y - radius, x + radius, y + radius), fill=255)

    def pixels(self):
        data = self.mask.load()
        width, height = self.size
        return {(x, y) for y in range(height) for x in range(width) if data[x, y]}


def shade(inside, base, light=None, shadow=None, back=None, trim=None, rim=None, trim_from=0):
    """Top edges catch the moon; bottom and back edges fall into shadow."""
    colors = {}
    for x, y in inside:
        symbol = base
        if shadow and (x, y + 1) not in inside:
            symbol = trim if trim and y >= trim_from else shadow
        if back and (x - 1, y) not in inside:
            symbol = back
        if rim and (x + 1, y) not in inside and y >= trim_from:
            symbol = rim
        if light and (x, y - 1) not in inside:
            symbol = light
        colors[(x, y)] = symbol
    return colors


def grade(inside, symbols, light=(-0.45, -0.8, 0.6), depth=None, spread=0.3, fall=0.0, bias=0.0):
    """Graded shading for a part, a few dithered steps from dark to light:
    its height rises as a rounded dome `depth` pixels in from its edge (by
    default all the way to its middle), so each pixel's slope turns it
    toward or away from `light` (screen x and y toward the light, then out
    of the picture), lit top-left to a shadowed underside; `fall` darkens it
    further toward its foot, `bias` lifts the whole. `symbols` run dark to
    light, picked by the ordered dither."""
    if not inside:
        return {}
    far = 10 ** 6
    order = sorted(inside, key=lambda p: (p[1], p[0]))
    dist = dict.fromkeys(inside, far)
    for pixels, steps in ((order, ((-1, 0, 3), (0, -1, 3), (-1, -1, 4), (1, -1, 4))),
                          (order[::-1], ((1, 0, 3), (0, 1, 3), (1, 1, 4), (-1, 1, 4)))):
        for x, y in pixels:  # a chamfer distance in from the edge, in thirds of a pixel
            dist[(x, y)] = min([dist[(x, y)]] + [dist.get((x + dx, y + dy), 0) + w for dx, dy, w in steps])
    depth = depth or max(1.0, max(dist.values()) / 3 - 0.5)

    def height(p):
        u = min(1.0, max(0.0, (dist.get(p, 0) / 3 - 0.5) / depth))
        return depth * math.sqrt(1 - (1 - u) ** 2)

    length = math.sqrt(sum(c * c for c in light))
    lx, ly, lz = (c / length for c in light)
    top = min(y for _, y in inside)
    tall = max(1, max(y for _, y in inside) - top)
    colors = {}
    for x, y in order:
        nx = (height((x - 1, y)) - height((x + 1, y))) / 2
        ny = (height((x, y - 1)) - height((x, y + 1))) / 2
        norm = math.sqrt(nx * nx + ny * ny + 1)
        facing = (nx * lx + ny * ly + lz) / norm
        value = 0.5 + 0.5 * (facing - lz) / (1 - lz) + bias - fall * (y - top) / tall
        colors[(x, y)] = ramp(value, symbols, x, y, spread)
    return colors


def packed_stones(inside, symbols, size=5, seed='stones', mortar=None):
    """`inside` packed with small stones about `size` px across (e01hollow):
    each pixel joins the nearest of a jittered grid of centres, each stone
    is lit toward the moon at its upper right and falls to shadow at its
    lower left, through `symbols` dark to light, with `mortar` (else the
    darkest symbol) in the joints between stones. A rubble core, gravel or
    scree."""
    def centre(gx, gy):
        return (gx + 0.2 + 0.6 * unit(seed, gx, gy, 'x')) * size, (gy + 0.2 + 0.6 * unit(seed, gx, gy, 'y')) * size

    colors = {}
    for x, y in inside:
        gx, gy = x // size, y // size
        near = sorted((math.hypot(x - cx, y - cy), cx, cy, i, j)
                      for i in (gx - 1, gx, gx + 1) for j in (gy - 1, gy, gy + 1) for cx, cy in (centre(i, j),))
        (d1, cx, cy, i, j), (d2, *_) = near[0], near[1]
        if d2 - d1 < 1.0:
            colors[(x, y)] = mortar or symbols[0]
            continue
        lean = ((x - cx) - (y - cy)) / size + (unit(seed, i, j, 'tone') - 0.5) * 0.6
        colors[(x, y)] = ramp(0.5 + lean * 0.7, symbols[1:] if mortar is None else symbols, x, y, 0.3)
    return colors


def fbm(x, y, seed, octaves=3, _lattice={}):
    """Smooth value noise in [0, 1) at (`x`, `y`), `octaves` of it summed at
    doubling frequencies and halving weights (e01hollow): a flame's
    tongues, a crater's glow, rock's soft masses."""
    def corner(o, i, j):
        key = (seed, o, i, j)
        if key not in _lattice:
            _lattice[key] = unit('lattice', seed, o, i, j)
        return _lattice[key]

    total = norm = 0.0
    for o in range(octaves):
        u, v = x * 2 ** o, y * 2 ** o
        i, j = math.floor(u), math.floor(v)
        fx, fy = u - i, v - j
        sx, sy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
        top = corner(o, i, j) + (corner(o, i + 1, j) - corner(o, i, j)) * sx
        low = corner(o, i, j + 1) + (corner(o, i + 1, j + 1) - corner(o, i, j + 1)) * sx
        total += (top + (low - top) * sy) / 2 ** o
        norm += 1 / 2 ** o
    return total / norm


def compose(layers):
    """Paint parts back to front; each part rims what it overlaps."""
    canvas = {}
    for colors, separator in layers:
        if separator:
            for x, y in colors:
                for dx, dy in NEIGHBOURS:
                    n = (x + dx, y + dy)
                    if n in canvas and n not in colors:
                        canvas[n] = separator
        canvas.update(colors)
    return canvas


def despeckle(canvas):
    """Drop pixels with no 4-connected neighbour."""
    lonely = [p for p in canvas if not any((p[0] + dx, p[1] + dy) in canvas for dx, dy in NEIGHBOURS)]
    for p in lonely:
        del canvas[p]
    return canvas


def outline(canvas, symbol='o', size=(64, 64)):
    ring = {
        (x + dx, y + dy)
        for x, y in canvas
        for dx, dy in NEIGHBOURS
        if (x + dx, y + dy) not in canvas
    }
    for x, y in ring:
        if 0 <= x < size[0] and 0 <= y < size[1]:
            canvas[(x, y)] = symbol
    return canvas


def to_image(canvas, size=(64, 64)):
    im = Image.new('RGBA', size, PALETTE['.'])
    pixels = im.load()
    for (x, y), symbol in canvas.items():
        if 0 <= x < size[0] and 0 <= y < size[1]:
            pixels[x, y] = PALETTE[symbol]
    return im


def ramp(value, symbols, x=0, y=0, spread=0.0):
    """Pick from dark-to-light `symbols` by value in [0, 1], optionally dithered."""
    value += (BAYER[y % 4][x % 4] / 16 - 0.5) * spread
    index = min(len(symbols) - 1, max(0, int(value * len(symbols))))
    return symbols[index]


def wave(x, period, *harmonics):
    """Sum of sines that repeats exactly every `period` pixels."""
    return sum(amp * math.sin(math.tau * (k * x / period) + phase) for k, amp, phase in harmonics)
