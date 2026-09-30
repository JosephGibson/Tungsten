"""Midnight sky, moon, cloud banks, ruined ridges and woodland silhouettes.

Strips are 1024 pixels wide and repeat seamlessly: every profile is a sum of
whole-period sines and every shape is drawn at x and x ± 1024. The last row of
the opaque strips is solid, because the runtime stretches it below the art.
"""
import math

from PIL import Image, ImageDraw

from art import rng_for
from pixels import dither, to_image, unit, wave

WIDTH = 1024
STRIP = (WIDTH, 256)
SKY = (WIDTH, 512)


def wrapped(draw_shape, x):
    for offset in (-WIDTH, 0, WIDTH):
        draw_shape(x + offset)


def mask_pixels(mask):
    data = mask.load()
    return {(x, y) for y in range(mask.height) for x in range(mask.width) if data[x, y]}


def sky():
    canvas = {}
    width, height = SKY
    ramp = '0123456789'
    for y in range(height):
        level = 8.9 * (y / (height - 1)) ** 1.35
        band = int(level)
        for x in range(width):
            index = min(9, band + dither(x, y, level - band))
            symbol = ramp[index]
            # Haze gathers above the horizon.
            if y > height - 90 and dither(x, y, (y - (height - 90)) / 110):
                symbol = 'ν'
            canvas[(x, y)] = symbol
    rng = rng_for('sky-stars')
    # A faint milky band crosses from lower left to upper right.
    def in_band(x, y):
        return abs(y - (190 - x * 0.17 + 18 * math.sin(x / 90))) < 34
    for _ in range(900):
        x, y = rng.randrange(width), rng.randrange(360)
        if in_band(x, y) or rng.random() < 0.35:
            canvas[(x, y)] = 'ξ' if rng.random() < 0.85 else '9'
    for _ in range(70):
        x, y = rng.randrange(width), rng.randrange(320)
        canvas[(x, y)] = 'π'
    for _ in range(9):
        x, y = rng.randrange(8, width - 8), rng.randrange(12, 280)
        canvas[(x, y)] = 'w'
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            canvas[(x + dx, y + dy)] = 'ξ'
    return to_image(canvas, SKY)


def moon():
    """Full moon: darker limb, solid irregular maria and a few round craters.
    Features stay at least three pixels wide, so they hold up when scaled."""
    size = (64, 64)
    mask = Image.new('L', size, 0)
    ImageDraw.Draw(mask).ellipse((7, 7, 56, 56), fill=255)
    disc = mask_pixels(mask)
    maria = [(22, 24, 9), (41, 39, 7), (24, 43, 6), (44, 22, 4)]
    craters = [(17, 34, 3.2), (36, 50, 3.0), (48, 32, 3.0), (30, 15, 3.4)]
    canvas = {}
    for x, y in disc:
        r = math.hypot(x - 31.5, y - 31.5)
        symbol = 'w' if r > 23.6 or (r > 21.8 and x < 30) else 'I'
        for cx, cy, cr in maria:
            wobble = 0.82 + 0.18 * math.sin(math.atan2(y - cy, x - cx) * 3 + cx)
            d = math.hypot(x - cx, y - cy) / (cr * wobble)
            if d < 1:
                symbol = 'σ' if d < 0.5 and cr > 5 else 'w'
        canvas[(x, y)] = symbol
    for cx, cy, cr in craters:
        for x, y in disc:
            d = math.hypot(x - cx, y - cy)
            if d <= cr:
                # Shadowed upper-left wall, lit lower-right rim, dark floor.
                edge = d > cr - 1.1
                symbol = ('τ' if x + y < cx + cy else 'I') if edge else 'σ'
                canvas[(x, y)] = symbol
    canvas[(32, 32)] = 'I'
    return to_image(canvas, size)


def clouds(name):
    rng = rng_for(name)
    near = name.endswith('near')
    mask = Image.new('L', STRIP, 0)
    draw = ImageDraw.Draw(mask)
    count = 4 if near else 6
    for k in range(count):
        cx = (k + rng.random() * 0.6) * WIDTH / count
        base = rng.randrange(70, 130) if near else rng.randrange(50, 150)
        span = rng.randrange(150, 240) if near else rng.randrange(100, 190)
        lobes = rng.randrange(5, 8)
        for i in range(lobes):
            t = i / (lobes - 1)
            radius = (16 + 22 * math.sin(math.pi * t) ** 0.8) * (1.15 if near else 0.85)
            radius *= 0.8 + 0.4 * rng.random()
            lx = cx - span / 2 + span * t

            def lobe(x, r=radius, ly=base - radius * 0.55):
                draw.ellipse((x - r * 1.25, ly - r, x + r * 1.25, min(base + 6, ly + r)), fill=255)
            wrapped(lobe, lx)
        wrapped(lambda x: draw.rectangle((x - span / 2, base - 6, x + span / 2, base + 5), fill=255), cx)
    inside = mask_pixels(mask)
    canvas = {}
    for x, y in inside:
        above = (x, y - 1) not in inside or (x, y - 2) not in inside
        below = (x, y + 3) not in inside
        if above:
            symbol = 'μ'
        elif below or dither(x, y, 0.35 if (x, y + 8) not in inside else 0.0):
            symbol = 'λ'
        else:
            symbol = 'ι'
        canvas[(x, y)] = symbol
    # A soft veil fringes each bank.
    for x, y in list(inside):
        for dx, dy in ((0, -2), (-2, 0), (2, 0), (0, -3)):
            p = ((x + dx) % WIDTH, y + dy)
            if p not in canvas and 0 <= p[1] < STRIP[1] and dither(*p, 0.5):
                canvas[p] = 'κ'
    return to_image(canvas, STRIP)


def ridge_profile(base, seed, *peaks):
    """Cusped mountain line: (1 - |sin|) spikes at whole-period frequencies."""
    phases = [unit('ridge', seed, k) * math.pi for k, _ in peaks]
    def height(x):
        rise = sum(amp * (1 - abs(math.sin(math.pi * k * x / WIDTH + phase))) ** 1.4
                   for (k, amp), phase in zip(peaks, phases))
        return base - rise
    return height


def ruins(draw, ground):
    """Castle keep, a broken curtain wall with arches, and a lone spire, seated on
    the ridge line `ground(x)`; windows are returned so they can be lit."""
    windows = []
    def keep(x, g):
        draw.rectangle((x, g - 72, x + 24, g + 8), fill=255)
        for c in range(0, 25, 6):
            draw.rectangle((x + c, g - 78, x + c + 3, g - 72), fill=255)
        draw.polygon([(x + 24, g - 52), (x + 44, g - 56), (x + 44, g + 8), (x + 24, g + 8)], fill=255)
        draw.polygon([(x + 29, g - 56), (x + 35, g - 80), (x + 40, g - 56)], fill=255)
        draw.polygon([(x + 35, g - 80), (x + 38, g - 70), (x + 41, g - 76), (x + 40, g - 56)], fill=0)
        windows.extend([(x + 6, g - 56), (x + 13, g - 40), (x + 33, g - 38)])
    def wall(x, g):
        draw.rectangle((x, g - 34, x + 130, g + 8), fill=255)
        for c in range(x + 8, x + 120, 24):
            draw.pieslice((c, g - 24, c + 15, g - 4), 180, 360, fill=0)
            draw.rectangle((c, g - 14, c + 15, g + 8), fill=0)
        for c in range(x, x + 130, 7):
            if unit('crenel', c) < 0.6:
                draw.rectangle((c, g - 39, c + 3, g - 34), fill=255)
        draw.polygon([(x + 96, g - 34), (x + 104, g - 46), (x + 112, g - 34)], fill=0)
    def spire(x, g):
        draw.rectangle((x, g - 88, x + 10, g + 8), fill=255)
        draw.polygon([(x - 3, g - 88), (x + 5, g - 124), (x + 13, g - 88)], fill=255)
        draw.polygon([(x + 5, g - 124), (x + 8, g - 112), (x + 13, g - 118), (x + 13, g - 88)], fill=0)
        windows.append((x + 4, g - 70))
    for build, x, width in ((keep, 268, 44), (wall, 312, 130), (spire, 760, 10)):
        base = max(ground(x + dx) for dx in range(width)) + 2
        wrapped(lambda px, b=build, g=round(base): b(px, g), x)
    return windows


def distant_ridges():
    layers = [
        ('α', ridge_profile(150, 'far', (3, 62), (7, 26), (13, 12), (29, 5))),
        ('β', ridge_profile(182, 'mid', (2, 44), (5, 24), (11, 11), (23, 4))),
        ('γ', ridge_profile(214, 'near', (4, 26), (9, 12), (19, 6), (37, 3))),
    ]
    canvas = {}
    for symbol, height in layers:
        for x in range(WIDTH):
            top = round(height(x))
            lit = height(x + 1) > height(x - 1)  # slope falls toward the moon on the right
            for y in range(max(0, top), STRIP[1]):
                canvas[(x, y)] = symbol
            if lit and 0 <= top < STRIP[1] and symbol != 'γ':
                canvas[(x, top)] = 'δ'
        if symbol == 'β':
            mask = Image.new('L', STRIP, 0)
            windows = ruins(ImageDraw.Draw(mask), height)
            for p in mask_pixels(mask):
                canvas[p] = 'β'
            # A few windows still burn in the ruins.
            for i, (x, y) in enumerate(windows):
                if i % 2 == 0:
                    canvas[(x % WIDTH, y)] = 'A'
                    canvas[(x % WIDTH, y + 1)] = 'a'
    return to_image(canvas, STRIP)


def near_woodland():
    rng = rng_for('woodland-trees')
    rows = []
    for depth, count, symbol, rim in ((0, 30, 'ζ', 'η'), (1, 22, 'ε', 'ζ')):
        mask = Image.new('L', STRIP, 0)
        draw = ImageDraw.Draw(mask)
        for k in range(count):
            x = (k + rng.random() * 0.8) * WIDTH / count
            height = rng.randrange(40, 76) if depth == 0 else rng.randrange(55, 112)
            ground = 226 if depth == 0 else 238
            if rng.random() < 0.6:
                # Conifer: jagged tiers narrowing to a spire.
                tiers = rng.randrange(4, 7)
                for t in range(tiers):
                    f = t / tiers
                    half = (0.34 - 0.26 * f) * height * (0.8 + 0.3 * rng.random())
                    bottom = ground - f * height * 0.85
                    top = bottom - height * 0.38

                    def tier(cx, h=half, b=bottom, tp=top):
                        draw.polygon([(cx - h, b), (cx, tp), (cx + h, b)], fill=255)
                    wrapped(tier, x)
            else:
                # Broadleaf: a trunk under a clumped crown.
                wrapped(lambda cx, g=ground, h=height: draw.rectangle((cx - 3, g - h * 0.5, cx + 3, g), fill=255), x)
                for _ in range(6):
                    cx = x + rng.randrange(-24, 25)
                    cy = ground - height * (0.55 + 0.35 * rng.random())
                    r = rng.randrange(14, 26)
                    wrapped(lambda px, py=cy, pr=r: draw.ellipse((px - pr, py - pr, px + pr, py + pr), fill=255), cx)
        rows.append((mask_pixels(mask), symbol, rim))
    canvas = {}
    for inside, symbol, rim in rows:
        for x, y in inside:
            edge = (x, y - 1) not in inside and ((x - 1, y) in inside or (x + 1, y) not in inside)
            canvas[(x, y)] = rim if edge else symbol
    for x in range(WIDTH):
        top = round(232 + wave(x, WIDTH, (4, 3, 0.7), (11, 2, 1.9)))
        for y in range(top, STRIP[1]):
            canvas[(x, y)] = 'θ'
    return to_image(canvas, STRIP)


def build_backdrops(images):
    images['sky'] = sky()
    images['moon'] = moon()
    images['clouds_far'] = clouds('clouds_far')
    images['clouds_near'] = clouds('clouds_near')
    images['distant_ridges'] = distant_ridges()
    images['near_woodland'] = near_woodland()


def validate_backdrops(images):
    for name in ('clouds_far', 'clouds_near', 'distant_ridges', 'near_woodland'):
        im = images[name]
        left = [im.getpixel((0, y))[3] > 0 for y in range(im.height)]
        right = [im.getpixel((im.width - 1, y))[3] > 0 for y in range(im.height)]
        # Opaque strips must continue across the wrap within one pixel of height.
        if name in ('distant_ridges', 'near_woodland'):
            if abs(sum(left) - sum(right)) > 3 or not all(im.getpixel((x, im.height - 1))[3] for x in range(im.width)):
                raise ValueError(f'{name}: strip must wrap and keep an opaque last row')
