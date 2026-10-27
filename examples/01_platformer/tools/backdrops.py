"""Midnight sky, moon, cloud banks, ruined ridges and woodland silhouettes; M76's
background-layer tiles, far strips, black night sky and mist.

Strips are 1024 pixels wide and repeat seamlessly: every profile is a sum of
whole-period sines and every shape is drawn at x and x ± 1024. The last row of
the opaque strips is solid, because the runtime stretches it below the art.
"""
import math

from PIL import Image, ImageDraw

from art import rng_for
from pixels import Part, dither, packed_stones, shade, to_image, unit, wave
from terrain import (BACK_DARK, BREAK_LIPS, IRON_MASKS, MASKS, TILE, coursed, darker, ellipse, from_image,
                     iron_clad, line, mine_rock, rect, rock_tiles, tinted, wet_rock)

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


def build_backdrops(images, lit):
    images['sky'] = sky()
    images['moon'] = moon()
    images['clouds_far'] = clouds('clouds_far')
    images['clouds_near'] = clouds('clouds_near')
    images['distant_ridges'] = distant_ridges()
    images['near_woodland'] = near_woodland()
    images['sky_black'] = sky_black()
    for name in FAR_STRIPS:
        images[name] = globals()[name]()
    for i, (height, seed, dense) in enumerate(MISTS):
        images[f'mist_{i}'] = mist_band(height, seed, dense)
    build_level_backs(images, lit)


def validate_backdrops(images):
    for name in ('clouds_far', 'clouds_near', 'distant_ridges', 'near_woodland', *FAR_STRIPS):
        im = images[name]
        left = [im.getpixel((0, y))[3] > 0 for y in range(im.height)]
        right = [im.getpixel((im.width - 1, y))[3] > 0 for y in range(im.height)]
        # Opaque strips must continue across the wrap within one pixel of height.
        if name in ('distant_ridges', 'near_woodland', *FAR_STRIPS):
            if abs(sum(left) - sum(right)) > 3 or not all(im.getpixel((x, im.height - 1))[3] for x in range(im.width)):
                raise ValueError(f'{name}: strip must wrap and keep an opaque last row')


# ---------------------------------------------------------------------------
# M76 (01 (d)): the background layer's tiles, the areas' far strips, the
# black night sky and the ravine's mist, as the art review's sixth draw drew
# them (`perf-runs/20261021-e01d-art-review/`). Background tiles stand one
# step darker than the play layer (`terrain.darker`), the gorge's far wall
# two; the far strips draw unlit, a step darker than the ridges.

# The palette's true black, the night sky's.
NIGHT = '\u25a0'


def ivy_into(c, seed, x0, y0, length, down=True, spread=7):
    """A vine with paired leaves, hanging (`down`) or climbing from
    `(x0, y0)`."""
    for d in range(length):
        y = y0 + d if down else y0 - d
        x = x0 + round(2.5 * math.sin(d * 0.21 + unit(seed) * 6))
        c[(x, y)] = 't'
        if d % 5 == 0:
            side = 1 if (d // 5) % 2 else -1
            for k in range(int(2 + unit(seed, 'leaf', d) * (spread - 2))):
                lx, ly = x + side * (1 + k // 2), y + (k % 2) - (0 if down else 1)
                c[(lx, ly)] = 'G' if k == 0 and unit(seed, 'lit', d) < 0.5 else 'g' if k % 3 == 0 else 'T'


BAY = 4 * TILE          # a pier tile and a three-tile opening


ARCH_R, ARCH_RING, SPRING = 88, 14, 102


VOUSSOIRS = 17


def arcade(bays=3, seed='arcade', stained=True, back=True):
    """The aqueduct's arcade over two tile rows: piers a tile wide, a
    semicircular arch of voussoirs (a proud keystone) over each three-tile
    opening, coursed spandrels above, a moulded impost where each arch
    springs, lime and wet streaks where the channel leaks. Drawn whole and
    cut into tiles (`arcade_tiles`), so its pieces join; the bridge reuses
    it in front, the aqueduct puts it behind, one step darker, its streaks
    thinned (e01hollow): fewer, shorter and sparser."""
    width, height = bays * BAY + TILE, 2 * TILE
    c = {}
    for y in range(height):
        for x in range(width):
            b, u = x // BAY, x % BAY
            cx = b * BAY + TILE + 96 if u >= TILE // 2 else b * BAY - 96
            dx, dy = x - cx, SPRING - y
            if dy > 0:
                d = math.hypot(dx, dy)
                if d < ARCH_R:
                    continue
                theta = math.atan2(dy, dx)
                step = math.pi / VOUSSOIRS
                k = int(theta / step)
                key = k == VOUSSOIRS // 2
                if d < ARCH_R + ARCH_RING or (key and d < ARCH_R + ARCH_RING + 5):
                    if (theta - k * step) * d < 1.1:
                        sym = '<'
                    elif d - ARCH_R < 1.5:
                        sym = '-'
                    elif d > ARCH_R + ARCH_RING - 2 + (5 if key else 0):
                        sym = ';' if dy > 24 else ':'
                    else:
                        n = unit(seed, 'vous', b, k, round(d) // 4)
                        sym = ';' if key and n < 0.4 else ':' if n < 0.62 else '~'
                    c[(x, y)] = sym
                    continue
                c[(x, y)] = coursed(x, y, (seed, 'spandrel'))
            elif y < SPRING + 10 and (u < TILE + 10 or u >= BAY - 10):
                k = y - SPRING
                end = (u == TILE + 9) or (u == BAY - 10)
                sym = ';' if k == 0 else ':' if k == 1 else '~' if k < 4 else '=' if k == 4 else '-' if k == 5 else ':' if k < 9 else '<'
                if end:
                    sym = '<'
                c[(x, y)] = sym
            elif 6 <= u < 58:
                c[(x, y)] = '<' if u == 6 else '-' if u == 7 else ';' if u == 56 else '<' if u == 57 else coursed(x, y, (seed, 'shaft'))
    if stained:
        streaks, reach, start = (4, 34, 0.7) if back else (9, 60, 1.0)
        for k in range(bays * streaks):
            x = int(unit(seed, 'lime', k) * width)
            length = 10 + int(unit(seed, 'limel', k) * reach)
            wet = unit(seed, 'wet', k) < 0.4
            for d in range(length):
                if (x, d) in c and dither(x, d, start - start * d / (length + 4)):
                    c[(x, d)] = ('<' if d % 4 else '-') if wet else ('w' if d < 3 else 'σ' if d % 3 else 'τ')
        for k in range(bays * 2):
            ivy_into(c, (seed, 'ivy', k), int(unit(seed, 'ivx', k) * width), 0, 18 + int(unit(seed, 'ivl', k) * 40))
    return darker(c) if back else c


ARCADE_PIECES = (('pier_top', 0, 0), ('capital', 0, 1), ('arch_left', 1, 0), ('arch_key', 2, 0),
                 ('arch_right', 3, 0), ('spring_left', 1, 1), ('spring_right', 3, 1))


def arcade_tiles(back=True):
    """The arcade's seven pieces, cut from the middle bay of a drawing."""
    im = to_image(arcade(back=back), (3 * BAY + TILE, 2 * TILE))
    return [(f'arcade_{name}', im.crop((BAY + col * TILE, row * TILE, BAY + (col + 1) * TILE, (row + 1) * TILE)))
            for name, col, row in ARCADE_PIECES]


def pier(i, part='shaft', back=True):
    """An arcade pier's shaft (plain, lime runs, ivy, a carved plaque), its
    base (a chamfered plinth, moss and a wet line) and, for the bridge, its
    cutwater (a starling with a pointed cap, weed and foam at the water)."""
    c = {}
    for y in range(TILE):
        for x in range(6, 58):
            c[(x, y)] = coursed(x + 7 * TILE, y, ('pier', i))
        c[(6, y)], c[(7, y)], c[(56, y)], c[(57, y)] = '<', '-', ';', '<'
    kind = i % 4
    if part == 'shaft' and kind == 1:
        for k in range(6):
            x, y0 = 10 + int(unit('plime', i, k) * 42), 16 * int(unit('plimey', i, k) * 4)
            for d in range(4 + int(unit('plimel', i, k) * 22)):
                if y0 + d < TILE and dither(x, y0 + d, 1 - d / 30):
                    c[(x, y0 + d)] = 'w' if d < 2 else 'σ' if d % 3 else 'τ'
    elif part == 'shaft' and kind == 2:
        ivy_into(c, ('pivy', i), 16 + int(unit('pivx', i) * 30), 63, 60, down=False, spread=8)
    elif part == 'shaft' and kind == 3:
        rect(c, 16, 18, 48, 40, lambda x, y: '<' if x in (16, 47) or y in (18, 39) else ';' if x == 46 or y == 38 else '-' if x == 17 or y == 19 else ':')
        for row, (x0, x1) in enumerate(((21, 43), (20, 44), (24, 40))):
            for x in range(x0, x1):
                if unit('glyph', i, row, x // 3) < 0.7 and x % 3 != 2:
                    c[(x, 24 + row * 5)] = '<'
                    c[(x, 25 + row * 5)] = '-'
        for k in range(5):
            c[(14, 22 + k * 4)], c[(49, 22 + k * 4)] = 'T', 'g'
    if part == 'base':
        for y in range(38, TILE):
            w = min(6, y - 37)
            for x in range(6 - w, 58 + w):
                if y == 38 + (6 - w) and w < 6:
                    sym = ';'
                elif x in (6 - w, 57 + w):
                    sym = '<'
                elif y >= 56 and dither(x, y, (y - 54) / 10):
                    sym = '-' if dither(x, y, 0.5) else '<'
                else:
                    sym = coursed(x + 7 * TILE, y + 4, ('base', i), course=12)
                c[(x, y)] = sym
        for x in range(0, TILE):
            if unit('bmoss', i, x // 2) < 0.65:
                for y in range(60 - int(unit('bml', i, x) * 7), TILE):
                    c[(x, y)] = 'T' if y > 61 else 'g' if dither(x, y, 0.5) else 'T'
    elif part == 'cutwater':
        for y in range(20, TILE):
            for x in range(0, TILE):
                cap = 20 + abs(x - 32) * 0.55
                if y < cap:
                    continue
                if y < cap + 2:
                    sym = ';' if x >= 32 else '='
                elif y < 34:
                    sym = (':' if x >= 32 else '-') if (x + y) % 7 else '<'
                elif x in (0, 63):
                    sym = '<'
                else:
                    sym = coursed(x + 9 * TILE, y, ('cut', i), course=12)
                if y >= 50 and dither(x, y, (y - 48) / 14):
                    sym = 'P' if unit('weed', i, x // 2, y // 3) < 0.25 else '-' if dither(x, y, 0.5) else '<'
                c[(x, y)] = sym
        for x in range(TILE):
            for y in range(59, TILE):
                if dither(x, y, 0.55 + 0.4 * abs(x - 32) / 32):
                    c[(x, y)] = '_' if (x + y) % 3 else '`'
    return to_image(darker(c) if back else c)


def pier_wet(k):
    """A pier's shaft under a leak (01 (k)), `k` of 2: the back shaft with a
    damp run down its middle, lime crusting its edges and moss pads on the
    courses. The run darkens the stone a step at about half the old dither
    (e01hollow), so the courses show through it."""
    c = from_image(pier(k * 2 + 1, 'shaft', back=True))
    x0 = 18 + int(unit('wetx', k) * 16)
    w = 10 + int(unit('wetw', k) * 8)
    for y in range(TILE):
        sway = round(math.sin(math.tau * y / TILE + k) * 2)
        for x in range(x0 + sway, x0 + sway + w):
            if (x, y) not in c:
                continue
            if x in (x0 + sway, x0 + sway + w - 1):
                if dither(x, y, 0.3):
                    c[(x, y)] = 'σ' if dither(x, y, 0.5) else 'τ'
            elif dither(x, y, 0.4):
                c[(x, y)] = BACK_DARK.get(c[(x, y)], c[(x, y)])
    for j in range(6):
        mx, my = 8 + int(unit('pmoss', k, j) * 46), 4 + j * 10 + int(unit('pmy', k, j) * 4)
        for dx in range(-3 - j % 2, 4):
            for dy in range(0, 2 + (dx + 4) % 2):
                if (mx + dx, my + dy) in c and dither(mx + dx, my + dy, 0.85):
                    c[(mx + dx, my + dy)] = 't' if dy else 'T'
    return to_image(c)


# The second pier top's opening (e01hollow): a round relieving oculus
# through the spandrel over a pier, its centre, its opening's radius, its
# ring's depth and its voussoirs.
OCULUS, OCULUS_R, OCULUS_RING, OCULUS_STONES = (32, 27), 11, 6, 11


def pier_top_oculus():
    """The aqueduct's second pier top (e01hollow), over every other pier:
    the plain top's spandrel pierced by a round relieving oculus, a ring of
    voussoirs with a proud keystone, the soffit's lower curve catching the
    moon inside it, lime weeping from its foot and moss in its lower
    joints; the night shows through. Only its middle changes, so its sides
    meet the arches as the plain top's do."""
    c = dict(arcade_tiles_canvas()['pier_top'])
    cx, cy = OCULUS
    step = math.tau / OCULUS_STONES
    ring = {}
    for y in range(TILE):
        for x in range(TILE):
            dx, dy = x - cx, y - cy
            d = math.hypot(dx, dy)
            theta = math.atan2(dx, -dy) % math.tau        # 0 at the top, clockwise
            k = int((theta + step / 2) // step) % OCULUS_STONES
            key = k == 0
            outer = OCULUS_R + OCULUS_RING + (3 if key else 0)
            if d < OCULUS_R:
                c.pop((x, y), None)
                # The soffit's lower curve, seen from a little above.
                if dy > 0 and d > OCULUS_R - 3 + abs(dx) / OCULUS_R * 2.5:
                    ring[(x, y)] = ';'
                continue
            if d >= outer:
                continue
            off = abs(((theta + step / 2) % step) - step / 2) * d
            if off < 1.0 and not (key and d > OCULUS_R + OCULUS_RING):
                sym = '<'
            elif d - OCULUS_R < 1.5:
                sym = '<' if dy < 0 else '-'
            elif d > outer - 1.5:
                sym = ';' if dx > 0 or dy < -6 else ':' if dy < 6 else '-'
            else:
                n = unit('oculus', k, round(d) // 3)
                sym = ';' if key and n < 0.4 else ':' if n < 0.6 else '~'
            ring[(x, y)] = sym
    # The spandrel's courses close round the ring with a dark joint.
    for (x, y) in list(ring):
        for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if (nx, ny) in c and (nx, ny) not in ring and math.hypot(nx - cx, ny - cy) >= OCULUS_R:
                c[(nx, ny)] = 'o'
    c.update(darker(ring))
    # Lime weeping from the ring's foot, and moss in its lower joints.
    for n in range(3):
        x = cx - 6 + n * 6 + int(unit('ocl', n) * 3)
        y0 = cy + OCULUS_R + OCULUS_RING
        for d in range(6 + int(unit('ocll', n) * 14)):
            p = (x, y0 + d)
            if p in c and dither(x, y0 + d, 0.75 - d / 24):
                c[p] = 'σ' if d < 2 else 'τ' if d % 3 else 'S'
    for k in (4, 5, 6, 7):
        a = (k + 0.5) * step
        for r in range(OCULUS_R + 1, OCULUS_R + OCULUS_RING):
            p = (round(cx + math.sin(a) * r), round(cy - math.cos(a) * r))
            if p in c and unit('ocm', k, r) < 0.6:
                c[p] = 't' if r % 2 else 'T'
    return to_image(c)


def arcade_tiles_canvas():
    """The aqueduct's arcade pieces (`arcade_tiles(back=True)`) as canvases,
    by name without the prefix."""
    return {name.removeprefix('arcade_'): from_image(im) for name, im in arcade_tiles(back=True)}


def plinth_top(x):
    """The base's chamfered top at column `x`: row 38 under the shaft,
    falling a row a pixel out to row 43 at the tile's sides."""
    return 37 + max(1, 6 - x, x - 57)


def pier_base(k):
    """A pier's base, variant `k` of 4 (e01hollow): today's chamfered plinth
    (0); its left corner broken away, the rubble core showing in the break,
    the chunk lying at its foot and a fern in the joint above it (1); a
    rounded torus over the plinth, a lime tide-mark, and a drain arched
    through its foot weeping a trickle onto wet stone (2); a block split by
    a root, ivy climbing from the ground, a mason's mark and moss cushions
    on the chamfer's shoulders (3). Each keeps the shaft's joints at its top
    and its foot on the tile's floor, so it joins as the plain base does."""
    if k == 0:
        return pier(4, 'base')
    c = from_image(pier(4 + k, 'base', back=False))
    if k == 1:
        # The facing stones of its left corner fallen: a notch out of its
        # silhouette, the rubble core showing round it, in shadow under the
        # stones still standing, and the largest stone lying at its foot.
        notch, core = Part(), Part()
        notch.polygon([(-1, 42), (-1, 54), (3, 55), (7, 51), (11, 49), (13, 45), (10, 42), (5, 40)])
        core.polygon([(-1, 40), (-1, 57), (6, 57), (10, 54), (16, 51), (20, 46), (25, 43), (23, 39), (13, 38), (5, 39)])
        hole, recess = notch.pixels(), core.pixels() - notch.pixels()
        for p in hole:
            c.pop(p, None)
        recess = {p for p in recess if p in c}
        c.update(packed_stones(recess, ('o', '<', '-', '=', '~'), size=4, seed='base-core'))
        for (x, y) in recess:
            if (x, y - 1) not in recess and (x, y - 1) in c or (x + 1, y) not in recess and (x + 1, y) in c:
                c[(x, y)] = 'o'
            elif (x - 1, y) in hole or (x, y + 1) in hole:
                c[(x, y)] = ':'
        chunk = Part()
        chunk.polygon([(0, 63), (15, 63), (16, 58), (11, 55), (4, 56), (0, 58)])
        pts = chunk.pixels()
        c.update(shade(pts, '~', light=';', shadow='<', back='-', rim=':'))
        for (x, y) in pts:
            if unit('chunkm', x // 2) < 0.4 and (x, y - 1) not in pts:
                c[(x, y)] = 'T'
        for (x, y) in list(pts):
            for nx, ny in ((x + 1, y), (x, y - 1)):
                if (nx, ny) not in pts and 0 <= nx < TILE:
                    c[(nx, ny)] = 'o'
        for t in range(10):
            fx, fy = 27 + round(math.sin(t * 0.6) * 1.5), 38 - t
            c[(fx, fy)] = 'G' if t % 3 == 0 else 'g'
            if t % 2 and t < 9:
                for d in range(1, 3 + t // 3):
                    c[(fx - d, fy + d // 2)] = 'T' if d > 1 else 'g'
                    c[(fx + d, fy + d // 2)] = 'g' if d > 1 else 'G'
    elif k == 2:
        for x in range(0, TILE):
            t = plinth_top(x)
            for y, sym in ((44, ';'), (45, ':'), (46, '-'), (47, '<')):
                if y > t and (x, y) in c:
                    c[(x, y)] = sym
            mark = 51 + round(math.sin(x * 0.4) * 0.6)
            if unit('tide', x // 2) < 0.55 and (x, mark) in c:
                c[(x, mark)] = 'τ' if unit('tidel', x) < 0.4 else 'S'
        # The drain, arched through the plinth's foot, weeping onto the
        # stone below its sill.
        for y in range(53, TILE):
            for x in range(25, 40):
                dx, dy = x - 32, y - 57
                inside = (dy >= 0 and abs(dx) <= 4) or (dy < 0 and dx * dx + dy * dy <= 16)
                ring = (dy >= -1 and abs(dx) <= 6) or (dy < -1 and dx * dx + dy * dy <= 40)
                if inside:
                    c[(x, y)] = '■' if y < 62 else '<'
                elif ring:
                    c[(x, y)] = ';' if dy < -3 else ':' if dx > 0 else '-'
                    if abs(abs(dx) - 5) < 1 or (dy < -1 and abs(dx * dx + dy * dy - 32) < 5 and dx % 3 == 0):
                        c[(x, y)] = '<'
        for y in range(59, TILE):
            for x in range(29, 36):
                if dither(x, y + 1, 0.85 - abs(x - 32) * 0.2):
                    c[(x, y)] = '}' if (x + y) % 3 else '_'
        for x in range(24, 41):
            if (x, 63) in c and abs(x - 32) > 3 and dither(x, 63, 0.7):
                c[(x, 63)] = '|' if dither(x, 63, 0.5) else '{'
        for mx, h in ((49, 4), (53, 3), (56, 5)):
            for y in range(TILE - h, TILE):
                c[(mx, y)] = 'τ'
            for dx in (-2, -1, 0, 1, 2):
                c[(mx + dx, TILE - h)] = 'w' if dx < 1 else 'σ'
            c[(mx - 1, TILE - 1 - h)], c[(mx, TILE - 1 - h)] = 'w', 'I'
    elif k == 3:
        x = 30
        for y in range(39, TILE):
            r = unit('root', y)
            x += 1 if r > 0.72 else -1 if r < 0.24 else 0
            c[(x, y)] = 'o'
            c[(x + 1, y)] = '<' if unit('rootw', y) < 0.6 else c.get((x + 1, y), '<')
        for t in range(7):
            c[(31 + t // 2, 46 - t)] = 'G' if t % 2 else 'g'
            c[(30 + t // 2, 46 - t)] = 'T'
        ivy_into(c, ('bivy', 3), 52, 63, 30, down=False, spread=6)
        for mx, my in ((11, 49), (12, 50), (13, 51), (14, 52), (15, 53), (15, 49), (14, 50), (12, 52), (11, 53)):
            c[(mx, my)] = '<'
        for x in list(range(0, 7)) + list(range(57, TILE)):
            t = plinth_top(x)
            if unit('cush', x) < 0.85:
                c[(x, t - 1)] = 'G' if unit('cushg', x) < 0.3 else 'g'
                c[(x, t)] = 'g'
                for y in range(t + 1, t + 2 + int(unit('cushl', x) * 4)):
                    c[(x, y)] = 'T'
    return to_image(darker(c))


def gap_strip(g):
    """Gap `g`'s arcade tops (01 (k)), rows 29-30 from the right break's pier
    to the left break's column, six tiles: the pier under the channel's
    end, the broken bay, the pier standing in the gap and the next bay's
    haunch under the channel's other end, cut along one broken profile.
    Under the right lip the masonry drops near-plumb to a ragged ledge about
    40 px down row 29; under the left lip it goes on down behind the pour
    onto the capital of the pier beside it, whose broken top rises in a
    ragged shoulder to the pour's outer edge, 14-18 px under the channel, so
    the pier carries the channel's end; from each pier the spandrel falls
    away steeply to the bay's broken ring, which keeps its springing
    voussoirs. Slopes, ledges and the voussoirs kept differ by gap and side.
    Returns the twelve tiles by (row, column)."""
    full = arcade(back=True)
    lip_r = BREAK_LIPS['right'][g]
    face_l = 5 * TILE + BREAK_LIPS['left'][(g + 1) % 3] - 8
    ledge_r = 36 + int(unit('ledger', g) * 10)
    stump = 40 + int(unit('stump', g) * 12)
    steep_r, steep_l = (1.5, 2.4, 1.9)[g], (2.2, 1.6, 2.7)[g]
    step = math.pi / VOUSSOIRS
    keep_r = 5 + int(unit('stubr', g) * 2)
    keep_l = 5 + int(unit('stubl', g) * 2)
    cx = TILE + 96
    peak = (16, 14, 18)[g]
    fall = (1.2, 1.0, 1.4)[g]

    def rag(x, key, amp=6):
        return (unit('rag', g, key, x // 3) - 0.5) * amp

    def face(y):
        """The left face's x at strip row `y`: plumb, ragged every 3 px."""
        return face_l + rag(y, 'fl', 3)

    def shoulder(x):
        return peak + max(0, face_l - x) * fall + rag(x, 'sh', 5)

    def top(x):
        """The masonry's broken top at strip column `x`, in row 29's px."""
        if x <= lip_r + rag(0, 'lr', 4) or x >= face_l + 2:
            return -1e9
        if x < TILE:
            return ledge_r + rag(x, 'r')
        if x < 4 * TILE:
            if x < cx:
                return ledge_r + (x - TILE) * steep_r + rag(x, 'sr', 8)
            return stump + (4 * TILE - x) * steep_l + rag(x, 'sl', 8)
        if x < 5 * TILE:
            return min(stump + rag(x, 'p'), shoulder(x))
        return min(stump - (x - 5 * TILE) * 0.5 + rag(x, 'h'), shoulder(x))

    c = {}
    for (x, y), sym in full.items():
        if not 0 <= x < 6 * TILE:
            continue
        dx, dy = x - cx, SPRING - y
        if TILE <= x < 4 * TILE and dy > 0:
            d = math.hypot(dx, dy)
            theta = math.atan2(dy, dx)
            kk = int(theta / step)
            kept = kk < keep_r or kk >= VOUSSOIRS - keep_l
            if d < ARCH_R + ARCH_RING + 5:
                if kept:
                    c[(x, y)] = sym
                continue
            # A ragged band of backing courses rides the kept voussoirs.
            backing = 9 + rag(x, 'bk', 10) + 4 * math.sin(theta * 5 + g)
            edge = keep_r * step if dx > 0 else math.pi - keep_l * step
            near = theta < edge if dx > 0 else theta > edge
            if kept and near and d < ARCH_R + ARCH_RING + 5 + backing:
                c[(x, y)] = sym
                continue
        if y < SPRING - 4 and x < face(y) and y < top(x):
            continue
        c[(x, y)] = sym
    # Broken faces: lit where a top was torn, dark at the sides; rubble.
    for (x, y) in list(c):
        if y >= SPRING - 4:
            continue
        if (x, y - 1) not in c and y > 0:
            c[(x, y)] = ';' if dither(x, y, 0.4) else ':'
        elif ((x + 1, y) not in c or (x - 1, y) not in c) and c[(x, y)] != '<':
            c[(x, y)] = '<' if dither(x, y, 0.6) else '-'
    for n in range(18):
        x0 = int(unit('rub', g, n) * 6 * TILE)
        t = top(x0)
        if t < 0 or t > SPRING - 12:
            continue
        r = 1 + int(unit('rubr', g, n) * 2)
        for dy in range(-r, r + 1):
            for dx in range(-r, r + 1):
                q = (x0 + dx, round(t) - r + dy)
                if dx * dx + dy * dy <= r * r and q not in c:
                    c[q] = ';' if dy < 0 else '-'
    im = to_image(c, (6 * TILE, 2 * TILE))
    return {(row, col): im.crop((col * TILE, row * TILE, (col + 1) * TILE, (row + 1) * TILE))
            for row in range(2) for col in range(6)}


def parapet(i, kind='plain', back=True):
    """The deck's far parapet, behind the player: a low coursed wall under a
    coping (`plain`, `mossy`), a `post` over each pier, a `broken` stretch
    whose stones have fallen."""
    c = {}
    post = kind == 'post'
    broken = kind == 'broken'
    for y in range(TILE):
        for x in range(TILE):
            top = 22 if post and 4 <= x < 22 else 34
            if broken:
                top = max(top, 34 + round(18 * max(0.0, math.sin(math.pi * (x - 10) / 44))) if 10 <= x < 54 else top)
            if y < top:
                continue
            k = y - top
            if not broken or not (10 <= x < 54):
                sym = ';' if k == 0 else ':' if k < 3 else '-' if k == 3 else coursed(x, y, ('parapet', i), course=12, block=24)
            else:
                sym = '<' if k == 0 else coursed(x, y, ('parapet', i), course=12, block=24)
            if post and x in (4, 21) and y >= 22:
                sym = '<'
            c[(x, y)] = sym
    for x in range(TILE):
        if unit('pmoss', i, x // 4) < (0.75 if kind == 'mossy' else 0.2) and not (broken and 10 <= x < 54):
            top = 34 if not (post and 4 <= x < 22) else 22
            c[(x, top)] = 'T'
            if kind == 'mossy' and unit('pdrape', i, x) < 0.4:
                for d in range(1, 3 + int(unit('pdl', i, x) * 8)):
                    c[(x, top + d)] = 't' if d > 2 else 'T'
    return to_image(darker(c) if back else c)


def cave_back(kind, i):
    """The cave's back wall: dark, low-contrast rock repeating every 64 px,
    and its kinds (ember, timber, pillar, stalactites, xbrace, ladder, ore)."""
    c = {}
    for y in range(TILE):
        for x in range(TILE):
            v = (math.sin(math.tau * (x / TILE + 0.13 * i)) + math.sin(math.tau * (2 * x / TILE + y / TILE + 0.3 * i))
                 + math.sin(math.tau * (y / TILE * 2 + 0.7)) * 0.8) / 2.8
            n = unit('back', kind, i, x // 3, y // 3)
            sym = 'ζ' if v > 0.35 else 'ε' if v > -0.25 else 'θ'
            if n < 0.06:
                sym = 'θ'
            elif n > 0.97 and v > 0.2:
                sym = 'ν'
            c[(x, y)] = sym
    for k in range(3):
        x0 = int(unit('fissure', kind, i, k) * 64)
        y0 = int(unit('fissure-y', kind, i, k) * 64)
        for d in range(10 + int(unit('fl', kind, i, k) * 16)):
            x0 += 1 if unit('fx', kind, i, k, d) > 0.5 else 0
            c[(x0 % TILE, (y0 + d) % TILE)] = 'θ'
    if kind == 'ember' and unit('vein?', i) < 0.16:
        x0, y0 = int(unit('vx', i) * 60), int(unit('vy', i) * 30)
        for d in range(14 + int(unit('vlen', i) * 18)):
            x0 += -1 if unit('vxs', i, d) < 0.3 else 1 if unit('vxs', i, d) > 0.7 else 0
            c[(x0 % TILE, (y0 + d) % TILE)] = 'L' if unit('vh', i, d) > 0.15 else 'K'
            if unit('vrim', i, d) < 0.5:
                c[((x0 + 1) % TILE, (y0 + d) % TILE)] = 'M'
    elif kind == 'timber':
        for y in range(TILE):
            for x in range(24, 40):
                u = (x - 24) / 15
                sym = '/' if u < 0.1 or u > 0.9 else '"' if 0.25 < u < 0.35 else 'ω'
                if unit('grain', i, x, y // 5) < 0.12:
                    sym = '/'
                c[(x, y)] = sym
        if i % 2 == 0:
            for y in range(6, 18):
                for x in range(TILE):
                    if 24 <= x < 40:
                        continue
                    v = (y - 6) / 11
                    c[(x, y)] = '"' if v < 0.12 else '/' if v > 0.88 else 'ω'
            for x, y in ((22, 9), (41, 9), (22, 14), (41, 14)):
                c[(x, y)], c[(x + 1, y)] = 'χ', 'Ω'
    elif kind == 'pillar':
        for y in range(TILE):
            for x in range(16, 48):
                u = (x - 16) / 31
                sym = 'γ' if u < 0.18 else 'ν' if u < 0.7 else 'ζ'
                if (y + int(unit('pj', i) * 20)) % 26 == 0:
                    sym = 'ε'
                c[(x, y)] = sym
            c[(15, y)], c[(48, y)] = 'θ', 'θ'
    elif kind == 'stalactites':
        for k in range(3):
            cx = 8 + int(unit('bst', i, k) * 48)
            length = 18 + int(unit('bstl', i, k) * 26)
            for d in range(length):
                w = max(0, round((length - d) / 5))
                for dx in range(-w, w + 1):
                    c[(cx + dx, d)] = 'ν' if dx < 0 else 'θ'
    elif kind == 'xbrace':
        for x0, x1 in ((0, 63), (63, 0)):
            line(c, x0, 0, x1, 63, 'ω', 3)
            line(c, x0, 1, x1, 64, '/', 1)
        for y in range(TILE):
            for x in (0, 1, 62, 63):
                c[(x, y)] = 'ω' if x in (1, 62) else '/'
    elif kind == 'ladder':
        for y in range(TILE):
            for x in (20, 21, 42, 43):
                c[(x, y)] = '"' if x in (20, 42) else 'ω'
        for y in range(4, TILE, 14):
            for x in range(22, 42):
                c[(x, y)], c[(x, y + 1)] = '"', '/'
    elif kind == 'ore':
        for k in range(4):
            x0, y0 = int(unit('ore', i, k) * 60), int(unit('orey', i, k) * 60)
            for d in range(8 + int(unit('orel', i, k) * 10)):
                x0 += 1
                y0 += -1 if unit('oy', i, k, d) < 0.4 else 1 if unit('oy', i, k, d) > 0.7 else 0
                c[(x0 % TILE, y0 % TILE)] = 'Φ'
                if d % 4 == 0:
                    c[(x0 % TILE, (y0 - 1) % TILE)] = 'ψ'
                if d % 7 == 3:
                    c[(x0 % TILE, (y0 + 1) % TILE)] = 'a'
    return to_image(c)


def cave_tunnel(kind):
    base = Image.new('RGBA', (128, 128))
    for k, (x, y) in enumerate(((0, 0), (64, 0), (0, 64), (64, 64))):
        base.alpha_composite(cave_back('rock', 40 + k), (x, y))
    c = from_image(base)
    for y in range(24, 128):
        for x in range(28, 100):
            dx = (x - 64) / 36
            top = 24 + round(40 * (1 - math.sqrt(max(0.0, 1 - dx * dx))))
            if y >= top and abs(dx) <= 1:
                depth = (y - top) / 100
                c[(x, y)] = '0' if depth > 0.15 or abs(dx) < 0.7 else 'θ'
    for y in range(22, 128):
        for x in range(26, 102):
            dx = (x - 64) / 36
            if abs(dx) > 1.06:
                continue
            top = 24 + round(40 * (1 - math.sqrt(max(0.0, 1 - min(1.0, dx * dx)))))
            if top - 3 <= y < top or abs(dx) > 1.0 and y >= top - 3:
                c[(x, y)] = 'γ' if (x < 64) else 'ν'
    if kind == 'ember':
        for y in range(56, 128):
            for x in range(36, 92):
                d = math.hypot((x - 64) / 28, (y - 124) / 60)
                if d < 1 and dither(x, y, (1 - d) * 1.2):
                    c[(x, y)] = 'M' if d > 0.6 else 'L' if d > 0.3 else 'K'
        for x in range(30, 98):
            if dither(x, 127, 0.8):
                c[(x, 127)] = 'K'
    else:
        for x0 in (22, 98):
            rect(c, x0, 20, x0 + 8, 128, lambda x, y: '"' if x == x0 + 1 else '/' if x == x0 + 7 else 'ω')
        rect(c, 22, 16, 106, 24, lambda x, y: '"' if y == 16 else '/' if y == 23 else 'ω')
        ellipse(c, 64, 92, 3, 3, lambda dx, dy: 'J' if dx * dx + dy * dy < 3 else 'K')
        for x in range(50, 79):
            c[(x, 124)] = 'Φ'
    return to_image(c, (128, 128))


def cave_mouth(columns=8, rows=7):
    """The Ember hollow's mouth (M79): the tunnel's back under the ravine's
    east wall, `columns` x `rows` tiles of the hollow's own cave rock, in
    shadow at the opening and lit by embers toward the hollow; a row of
    stalactites under the lintel, ember veins growing deeper in and a floor
    of glowing grit at its far end. Opaque, so no backdrop shows through,
    and lit, so it dims with the hollow's backs beside it."""
    canvas = Image.new('RGBA', (columns * TILE, rows * TILE))
    for r in range(rows):
        for c in range(columns):
            depth = c / max(1, columns - 1)
            if r == 0:
                kind = 'stalactites'
            elif unit('mouth-kind', r, c) < 0.15 + 0.5 * depth ** 1.5:
                kind = 'ember'
            else:
                kind = 'rock'
            tile = from_image(cave_back(kind, 80 + r * columns + c))
            shadow = 2 if c < 2 else 1 if c < 4 else 0
            if shadow:
                tile = darker(tile, shadow)
            canvas.alpha_composite(to_image(tile), (c * TILE, r * TILE))
    c = from_image(canvas)
    w, h = columns * TILE, rows * TILE
    # The opening's lip: the ravine's far wall, as its wet back draws it,
    # reaching a jagged way in at the mouth's first column, its edge lit by
    # the night outside.
    for r in range(rows):
        wet = from_image(to_image(tinted(from_image(wet_rock(500 + r, (False, False, False, False), None, back=2)))))
        for y in range(TILE):
            edge = 10 + round(6 * math.sin((r * TILE + y) * 0.09) + 5 * math.sin((r * TILE + y) * 0.23 + 1.3)
                              + 4 * unit('lip', (r * TILE + y) // 5))
            for x in range(edge):
                if (x, y) in wet:
                    c[(x, r * TILE + y)] = wet[(x, y)]
            c[(edge, r * TILE + y)] = 'α'
            c[(edge + 1, r * TILE + y)] = 'ν'
    # Glowing grit along the floor and in the joints near the hollow.
    for x in range(w):
        heat = (x / w) ** 2
        for y in range(h - 6, h):
            if dither(x, y, heat * (0.35 + (y - (h - 6)) / 10)):
                c[(x, y)] = 'K' if unit('grit', x // 2, y) < 0.3 else 'L'
    return to_image(c, (w, h))


def strip_canvas(layers, edge='α'):
    """Ridge layers like `distant_ridges`: each a symbol and a height, the
    slopes toward the moon lit `edge`."""
    canvas = {}
    for symbol, height, lit in layers:
        for x in range(STRIP[0]):
            top = round(height(x))
            for y in range(max(0, top), STRIP[1]):
                canvas[(x, y)] = symbol
            if lit and height(x + 1) > height(x - 1) and 0 <= top < STRIP[1]:
                canvas[(x, top)] = edge
    return canvas


def silhouette(canvas, draw_fn, symbol, edge='α'):
    mask = Image.new('L', STRIP, 0)
    extra = draw_fn(ImageDraw.Draw(mask)) or []
    data = mask.load()
    for y in range(STRIP[1]):
        for x in range(STRIP[0]):
            if data[x, y]:
                lit = y > 0 and not data[x, y - 1] and x + 1 < STRIP[0] and data[x + 1, y]
                canvas[(x, y)] = edge if lit else symbol
    return extra


def far_tower():
    """The clearing's horizon, over today's sky: far hills and, on a near
    rise, the old ruined tower of 01's first map with a broken crown, one
    window lit. A step darker than the third draw."""
    far = ridge_profile(170, 'tower-far', (3, 50), (7, 20), (17, 6))
    near = ridge_profile(214, 'tower-near', (2, 26), (5, 12), (13, 4))
    canvas = strip_canvas([('β', far, True), ('γ', near, False)])

    def tower(draw):
        g = round(near(620)) + 4
        draw.rectangle((606, g - 82, 626, g + 6), fill=255)
        for c in range(606, 627, 5):
            if unit('crown', c) < 0.65:
                draw.rectangle((c, g - 88, c + 3, g - 82), fill=255)
        draw.polygon([(626, g - 60), (640, g - 52), (640, g + 6), (626, g + 6)], fill=255)
        draw.polygon([(596, g - 22), (606, g - 40), (606, g + 6), (596, g + 6)], fill=255)
        draw.polygon([(618, g - 88), (626, g - 72), (626, g - 88)], fill=0)
        return [(612, g - 66), (616, g - 44), (632, g - 36)]
    windows = silhouette(canvas, tower, 'γ')
    for k, (x, y) in enumerate(windows):
        canvas[(x, y)], canvas[(x, y + 1)] = ('A', 'a') if k == 1 else ('ν', 'ν')
    return to_image(canvas, STRIP)


def far_arcade():
    """The aqueduct's horizon (01 (k)): a far ridge, then a two-tier ruin
    marching over it on taller piers, its lower arches deep, its upper tier
    lighter; broken spans end in ragged stubs, two with a thread of water
    glinting down from the break; dark against the sky."""
    far = ridge_profile(200, 'arcade-far2', (2, 40), (6, 16), (15, 5))
    ground = ridge_profile(238, 'arcade-ground2', (3, 10))
    canvas = strip_canvas([('β', far, True)])
    glints = []

    def arcade_(draw):
        # Sixteen bays across the strip, each drawn at x and x + 1024, the
        # wrap through an opening's middle, so the arcade runs on across it.
        for k in range(16):
            g = round(ground(k * 64)) + 8
            broken = unit('span2', k) < 0.24
            for x0 in (k * 64 - 38, k * 64 - 38 + WIDTH):
                draw.rectangle((x0, 108, x0 + 10, g), fill=255)
                draw.rectangle((x0 - 2, 106, x0 + 12, 110), fill=255)
                if not broken:
                    # The lower tier's deck and deep arch; the upper tier's
                    # band and two small arches.
                    draw.rectangle((x0, 132, x0 + 64, 146), fill=255)
                    draw.pieslice((x0 + 10, 140, x0 + 64, 194), 180, 360, fill=0)
                    draw.rectangle((x0 + 10, 167, x0 + 64, g), fill=0)
                    draw.rectangle((x0, 100, x0 + 64, 110), fill=255)
                    draw.rectangle((x0, 110, x0 + 64, 132), fill=255)
                    for a in (0, 1):
                        ax = x0 + 14 + a * 26
                        draw.pieslice((ax, 114, ax + 20, 134), 180, 360, fill=0)
                        draw.rectangle((ax, 124, ax + 20, 132), fill=0)
                    draw.rectangle((x0 + 2, 96, x0 + 62, 100), fill=255)
                else:
                    # A ragged stub off each side, and the gap between.
                    draw.polygon([(x0 + 10, 110), (x0 + 24, 104), (x0 + 30, 120), (x0 + 18, 136), (x0 + 10, 150)],
                                 fill=255)
                    draw.polygon([(x0 + 64, 100), (x0 + 56, 98), (x0 + 48, 116), (x0 + 58, 132), (x0 + 64, 140)],
                                 fill=255)
                    if unit('glint', k) < 0.6 and x0 < WIDTH:
                        glints.append((x0 + 27, 108, g))
        return []
    silhouette(canvas, arcade_, 'γ')
    for x, top, g in glints:
        for y in range(top, g - 6):
            if dither(x, y, 0.5):
                canvas[(x % WIDTH, y)] = 'ξ' if (y // 3) % 2 else 'δ'
    return to_image({p: s for p, s in canvas.items() if 0 <= p[0] < WIDTH}, STRIP)


def far_cliffs():
    """The ravine's far side on the black sky: a jagged wall with ledges and
    pines, a thread of water down it, moonlit along its crest."""
    crest = ridge_profile(110, 'cliffs', (5, 60), (11, 30), (23, 12), (47, 5))
    canvas = strip_canvas([('ν', crest, True)])
    for k in range(10):
        lx = int(unit('ledge', k) * STRIP[0])
        ly = 150 + int(unit('ledgey', k) * 90)
        for x in range(lx, lx + 30 + int(unit('ll', k) * 50)):
            canvas[(x % STRIP[0], ly)] = 'β'
            canvas[(x % STRIP[0], ly + 1)] = 'ε'
        if unit('pine', k) < 0.75:
            for d in range(16):
                for dx in range(-(d // 3), d // 3 + 1):
                    canvas[((lx + 12 + dx) % STRIP[0], ly - 16 + d)] = 'ε' if dx < 0 else 'ζ'
    for y in range(round(crest(512)) + 2, STRIP[1]):
        if dither(512, y, 0.55):
            canvas[(512 + round(math.sin(y / 20)), y)] = '|'
            canvas[(513 + round(math.sin(y / 20)), y)] = '{'
    for k in range(16):
        bx, bw = int(unit('rib', k) * STRIP[0]), 14 + int(unit('rib-w', k) * 26)
        for y in range(round(crest(bx + bw // 2)) + 10, STRIP[1]):
            x0 = bx + round(3 * math.sin(y / 19 + k))
            for x in range(x0, x0 + bw):
                if canvas.get((x % STRIP[0], y)) == 'ν':
                    canvas[(x % STRIP[0], y)] = 'β' if x >= x0 + bw - 2 else 'ε' if x == x0 else 'γ'
    return to_image(canvas, STRIP)


def far_mine():
    """The open cut's horizon on the black sky: spoil heaps, headframes and a
    chimney with a lit window over the far hills."""
    far = ridge_profile(176, 'mine-far', (3, 44), (8, 18), (19, 6))

    def heaps(x):
        # Whole periods across the strip, so the heaps run on across the wrap.
        return (214 - 34 * max(0.0, math.sin(math.tau * 3 * x / WIDTH)) ** 2
                - 22 * max(0.0, math.sin(math.tau * 5 * x / WIDTH + 1)) ** 2)
    canvas = strip_canvas([('γ', far, True), ('ν', heaps, True)])

    def frames(draw):
        for x0 in (180, 690):
            g = round(heaps(x0)) + 4
            draw.polygon([(x0 - 22, g), (x0 - 4, g - 70), (x0 + 4, g - 70), (x0 + 22, g)], fill=255)
            draw.polygon([(x0 - 14, g), (x0 - 2, g - 60), (x0 + 2, g - 60), (x0 + 14, g)], fill=0)
            draw.ellipse((x0 - 10, g - 84, x0 + 10, g - 64), fill=255)
            draw.ellipse((x0 - 6, g - 80, x0 + 6, g - 68), fill=0)
        draw.rectangle((424, round(heaps(430)) - 46, 434, 256), fill=255)
        draw.rectangle((422, round(heaps(430)) - 50, 436, round(heaps(430)) - 46), fill=255)
        return [(428, round(heaps(430)) - 30)]
    windows = silhouette(canvas, frames, 'ν', edge='β')
    for x, y in windows:
        canvas[(x, y)], canvas[(x, y + 1)] = 'A', 'a'
    return to_image(canvas, STRIP)


def far_valley():
    """The valley beyond the troll's bridge on the black sky: far peaks, the
    river's ribbon with the moon in it, and a long bridge on small arches,
    broken in its middle."""
    peaks = ridge_profile(196, 'valley-far', (3, 40), (7, 18), (17, 7), (37, 3))

    def banks(x):
        return 226 - 14 * math.sin(math.tau * x / 1024 * 2 + 0.6)
    canvas = strip_canvas([('γ', peaks, True), ('ν', banks, False)])
    for x in range(STRIP[0]):
        river_y = round(banks(x)) + 8
        for y in range(river_y, river_y + 5):
            canvas[(x, y)] = '|' if y > river_y else '{'
        if unit('glint', x // 6) < 0.18:
            canvas[(x, river_y)] = '}'

    def bridge_(draw):
        deck = 192
        for x0 in range(170, 850, 24):
            if 470 < x0 < 560:
                continue
            foot = round(banks(x0)) + 9
            draw.rectangle((x0, deck, x0 + 24, deck + 11), fill=255)
            draw.rectangle((x0 + 20, deck, x0 + 24, foot), fill=255)
            draw.pieslice((x0 + 3, deck + 5, x0 + 19, deck + 21), 180, 360, fill=0)
            draw.rectangle((x0 + 3, deck + 13, x0 + 19, foot), fill=0)
        draw.polygon([(470, deck), (476, deck), (473, deck + 6)], fill=255)
        draw.polygon([(560, deck), (564, deck), (564, deck + 26), (561, deck + 8)], fill=255)
        return []
    silhouette(canvas, bridge_, 'ε', edge='β')
    return to_image(canvas, STRIP)



def sky_black():
    """A true-black night sky, its stars fewer and dimmer than `sky`'s: the
    sky of the three areas whose far strips sit on black."""
    width, height = SKY
    canvas = {(x, y): NIGHT for y in range(height) for x in range(width)}
    rng = rng_for('sky-black-stars')
    for _ in range(650):
        x, y = rng.randrange(width), rng.randrange(400)
        canvas[(x, y)] = 'ν' if rng.random() < 0.5 else 'ξ' if rng.random() < 0.85 else '9'
    for _ in range(36):
        canvas[(rng.randrange(width), rng.randrange(360))] = 'π'
    for _ in range(6):
        x, y = rng.randrange(8, width - 8), rng.randrange(12, 300)
        canvas[(x, y)] = 'w'
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            canvas[(x + dx, y + dy)] = 'ν'
    return to_image(canvas, SKY)


def mist_band(height, seed, dense=0.5):
    """A mist band across the strip's width, thinning to nothing at its top
    and bottom; its billows repeat every 1024 pixels, so it wraps."""
    canvas = {}
    for y in range(height):
        fall = 1 - abs(y / height * 2 - 1)
        for x in range(WIDTH):
            billow = math.sin(math.tau * 3 * x / WIDTH + seed) * math.sin(y / 9 + math.tau * 7 * x / WIDTH + seed)
            v = fall * dense * (0.6 + 0.4 * billow)
            if dither(x, y, v):
                canvas[(x, y)] = 'κ' if v < 0.25 else 'ι' if v < 0.45 else 'μ'
    return to_image(canvas, (WIDTH, height))


# The far strips, each outdoor area's; three sit on `sky_black`.
FAR_STRIPS = ('far_tower', 'far_arcade', 'far_cliffs', 'far_mine', 'far_valley')
# The ravine's mist band over its far strip: height, seed and density.
MISTS = ((120, 1.0, 0.42),)
# The mist over the far walls, in the decorations layer between them and
# the terrain: the gorge's three bands, then the cut's two dust bands.
FAR_MISTS = ((150, 2.5, 0.55), (120, 3.3, 0.5), (110, 4.0, 0.5), (120, 1.7, 0.45), (90, 3.1, 0.35))
# The backdrop's images by name: none is a tile, so the tileset leaves them
# out (`level.py`), and none is lit.
BACKDROPS = ('sky', 'distant_ridges', 'near_woodland', 'clouds_far', 'clouds_near', 'sky_black',
             *FAR_STRIPS, *(f'mist_{i}' for i in range(len(MISTS))))
CAVE_BACKS = (('rock', 4), ('ember', 4), ('pillar', 2), ('stalactites', 2), ('timber', 2), ('xbrace', 1),
              ('ore', 2))


def cut_tiles(images, name, im):
    """`im` cut into tiles named `<name>_big_<row>_<col>`."""
    for row in range(im.height // TILE):
        for col in range(im.width // TILE):
            images[f'{name}_big_{row}_{col}'] = im.crop((col * TILE, row * TILE, (col + 1) * TILE, (row + 1) * TILE))


def build_level_backs(images, lit):
    """The background layer's tiles: the aqueduct's arcade and piers behind,
    the bridge's in front at full light, the parapet, the gorge's far wall,
    the cut's far terraces, the cave backs and tunnel mouths."""
    first = set(images)
    for name, im in arcade_tiles(back=True):
        images[name] = im
    for name, im in arcade_tiles(back=False):
        images[name.replace('arcade_', 'arcade_front_')] = im
    for i in range(4):
        images[f'pier_{i}'] = pier(i)
        images[f'pier_front_{i}'] = pier(i, back=False)
    # The piers' four bases and the second pier top, alternating over the
    # aqueduct's piers (e01hollow).
    for k in range(4):
        images['pier_base' + (f'_{k}' if k else '')] = pier_base(k)
    images['arcade_pier_top_1'] = pier_top_oculus()
    # The aqueduct's broken spans (01 (k)): the wet shafts beside each gap
    # and each gap's arcade tops.
    for k in range(2):
        images[f'pier_wet_{k}'] = pier_wet(k)
    for g in range(3):
        for (row, col), im in gap_strip(g).items():
            images[f'arcade_gap_{g}_{row}_{col}'] = im
    images['pier_cutwater'] = pier(0, 'cutwater', back=False)
    images['parapet_post'] = parapet(0, 'post')
    images['parapet_0'] = parapet(1)
    images['parapet_mossy'] = parapet(2, 'mossy')
    images['parapet_broken'] = parapet(3, 'broken')
    kinds = (None, None, 'sheen')
    # The gorge's far wall and the cut's far terraces under the far tint,
    # unlit, as the review drew them behind their mist.
    far = set(images)
    rock_tiles(images, 'wet_back', lambda i, open_: to_image(tinted(from_image(
        wet_rock(i + 500, open_, kinds[i % 3], back=2)))), MASKS, 3, 0)
    rock_tiles(images, 'cut_back', lambda i, open_: to_image(tinted(darker(from_image(mine_rock(i + 300, open_))))),
               MASKS, 2, 0)
    rock_tiles(images, 'cut_back_iron', lambda i, open_: to_image(tinted(darker(from_image(iron_clad(i + 300, open_))))),
               IRON_MASKS, 2, 0)
    for k, (height, seed, dense) in enumerate(FAR_MISTS):
        band = Image.new('RGBA', (WIDTH, -(-height // TILE) * TILE))
        band.alpha_composite(mist_band(height, seed, dense), (0, (band.height - height) // 2))
        cut_tiles(images, f'far_mist_{k}', band)
    far = set(images) - far
    for kind, count in CAVE_BACKS:
        for i in range(count):
            images[f'cave_back_{kind}_{i}'] = cave_back(kind, i)
    images['cave_back_ladder'] = cave_back('ladder', 2)
    cut_tiles(images, 'cave_mouth', cave_mouth())
    cut_tiles(images, 'cave_tunnel_ember', cave_tunnel('ember'))
    cut_tiles(images, 'cave_tunnel_mine', cave_tunnel('timber'))
    lit.update(set(images) - first - far)
