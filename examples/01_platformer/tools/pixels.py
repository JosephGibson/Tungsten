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
