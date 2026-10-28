"""Ruin scenery: lamp posts, ivy, waystones, the summit gate, oaks, the broken
arch, hanging roots, undergrowth, crystals, the waterfall and spikes; M76's
props for six areas and their water and lava clips.

Props share the courier's language: flat palette fills shaded from their own
silhouettes and a single dark outline. Composite pieces (gate, oak, arch,
roots) are drawn whole and cut into 64-pixel tiles.
"""
import math

from PIL import Image

from art import PALETTE, rng_for
from pixels import (Part, compose, despeckle, dither, fbm, grade, outline, packed_stones, ramp, shade, to_image,
                    unit)
from terrain import (BREAK_LIPS, COPING_TOP, DECK, FROND, LEAF, TILE, coursed, darker, ellipse, from_image,
                     line, rect, tinted)

STONE = dict(base='~', light=':', shadow='-', back='=')
PALE_STONE = dict(base=':', light=';', shadow='=', back='~')
BARK = dict(base='ω', light='"', shadow='/', back='/')
IRON = dict(base='s', light='S', shadow='B', back='B')
BRASS = dict(base='a', light='A', shadow='(', back='(')
LEAF_RAMP = 'tTgGm'


def cut(images, name, canvas, columns, rows):
    im = to_image(canvas, (columns * 64, rows * 64))
    for row in range(rows):
        for col in range(columns):
            images[f'{name}_big_{row}_{col}'] = im.crop((col * 64, row * 64, (col + 1) * 64, (row + 1) * 64))


def blocks(inside, course=10, seed='blocks', offset=0):
    """Dress a stone mask as coursed masonry with shaded blocks."""
    colors = {}
    for x, y in inside:
        row = (y - offset) // course
        width = 14 + round(unit(seed, 'width', row) * 10)
        stagger = round(unit(seed, 'stagger', row) * width)
        u, v = (x + stagger) % width, (y - offset) % course
        if u == 0 or v == 0:
            symbol = '<'
        elif v == 1 or (x, y - 1) not in inside:
            symbol = ':'
        elif v == course - 1 or (x, y + 1) not in inside:
            symbol = '-'
        elif u == 1 or (x - 1, y) not in inside:
            symbol = '='
        elif u == width - 1 or (x + 1, y) not in inside:
            symbol = ':'
        else:
            symbol = '~' if unit(seed, 'tone', row, (x + stagger) // width) > 0.35 else '='
        colors[(x, y)] = symbol
    return colors


def leaves(region, seed, frame=0, sway=0.0, size=(64, 64)):
    """Fill a mask with overlapping leaves, lit from above and the right."""
    top = min(y for _, y in region)
    bottom = max(y for _, y in region)
    colors = {}
    for x, y in sorted(region, key=lambda p: (p[1], p[0])):
        if unit(seed, 'leaf', x // 3, y // 3) < 0.55 and (x + y) % 3 == 0:
            height = (y - top) / max(1, bottom - top)
            light = 1 - height * 0.8 + (0.15 if (x + 1, y) not in region else 0)
            dx = round(sway * math.sin(math.tau * frame / 4 + y / 11))
            for lx, ly in ((0, 0), (1, 0), (-1, 0), (0, -1), (0, 1)):
                p = (x + lx + dx, y + ly)
                if 0 <= p[0] < size[0] and 0 <= p[1] < size[1]:
                    tone = light + (0.12 if ly < 0 else -0.1 if ly > 0 else 0)
                    colors[p] = LEAF_RAMP[min(4, max(0, int(tone * 4.2)))]
    for p in region:
        colors.setdefault(p, 't')
    return colors


def lamp_post(frame):
    plinth = Part()
    plinth.polygon([(23, 59), (23, 55), (26, 52), (37, 52), (40, 55), (40, 59)])
    pole = Part()
    pole.rectangle((30, 40, 33, 52))
    collar = Part()
    collar.rectangle((28, 47, 35, 49))
    roof = Part()
    roof.polygon([(24, 24), (31, 17), (32, 17), (39, 24)])
    roof.rectangle((31, 13, 32, 17))
    cage = Part()
    cage.rectangle((25, 24, 38, 41))
    flicker = [0, 1, -1, 1, 0, -1][frame]
    sway = [0, 1, 0, -1, 0, 1][frame]
    glass = {}
    for x in range(27, 37):
        for y in range(26, 40):
            glass[(x, y)] = 'y'
    flame = Part()
    tip = 29 - flicker
    flame.polygon([(30, 38), (29, 34), (31 + sway, tip), (34, 34), (33, 38)])
    for p in flame.pixels():
        if p in glass:
            glass[p] = 'Y'
    for y in range(26, 40):
        glass[(31, y)] = glass[(31, y)] if glass[(31, y)] == 'Y' else 'A'  # the centre mullion
    return to_image(despeckle(outline(compose([
        (shade(plinth.pixels(), **STONE), None),
        (shade(pole.pixels(), **IRON), 'o'),
        (shade(collar.pixels(), **BRASS), 'o'),
        (shade(cage.pixels(), **IRON), 'o'),
        (glass, None),
        (shade(roof.pixels(), **BRASS), 'o'),
    ]))))


def ivy(frame):
    """Branching ivy: clings to a cliff face, or creeps up from the ground."""
    rng = rng_for('ivy-stems')
    stems = Part()
    tips = []
    # Stems fan out and up from a root near the bottom centre.
    for angle in (-150, -125, -100, -80, -55, -30):
        a = math.radians(angle + rng.randrange(-8, 9))
        x, y = 32.0, 58.0
        points = [(x, y)]
        for step in range(rng.randrange(4, 7)):
            a += math.radians(rng.randrange(-22, 23))
            x = min(58.0, max(5.0, x + math.cos(a) * 8))
            y = min(58.0, max(6.0, y + math.sin(a) * 8))
            points.append((x, y))
        stems.limb(points, 1)
        tips.extend(points[1:])
    colors = {p: 't' for p in stems.pixels()}
    # Leaves cluster along the stems; lower leaves are older and darker.
    for i, (x, y) in enumerate(tips):
        dx = round(math.sin(math.tau * frame / 4 + y / 9 + i) * (58 - y) / 40)
        for lx, ly, tone in ((0, 0, 'g'), (-1, 0, 'T'), (1, 0, 'g'), (0, -1, 'G'), (-1, -1, 'g'),
                             (1, -1, 'G'), (0, 1, 'T'), (2, -2, 'm' if y < 30 else 'g')):
            colors[(round(x) + lx + dx, round(y) + ly)] = tone
        for k in range(2):
            ox, oy = round(x) + rng.randrange(-4, 5) + dx, round(y) + rng.randrange(-4, 5)
            colors[(ox, oy)] = 'g'
            colors[(ox + 1, oy)] = 'T'
            colors[(ox, oy - 1)] = 'G'
    # Swaying leaves must keep the transparent border the extractor relies on.
    colors = {p: s for p, s in colors.items() if 2 <= p[0] <= 61 and 2 <= p[1] <= 60}
    return to_image(despeckle(outline(colors)))


def waystone():
    slab = Part()
    slab.polygon([(20, 59), (20, 24), (23, 17), (29, 13), (35, 13), (41, 17), (44, 24), (44, 59)])
    inside = slab.pixels()
    colors = shade(inside, '~', light=':', shadow='-', back='=')
    for y in range(20, 58):
        if unit('waystone-weather', y) < 0.25:
            colors[(22 + round(unit('waystone-x', y) * 18), y)] = '='
    # Carved rune: a ring crossed by a staff.
    for angle in range(0, 360, 12):
        x = round(32 + 6 * math.cos(math.radians(angle)))
        y = round(34 + 6 * math.sin(math.radians(angle)))
        colors[(x, y)] = '<'
    for y in range(24, 46):
        colors[(32, y)] = '<'
    for x, y in ((29, 27), (35, 27), (28, 42), (36, 42)):
        colors[(x, y)] = '<'
    for x, y in ((40, 20), (41, 21), (40, 22), (41, 23), (39, 24)):
        colors[(x, y)] = '<'  # a crack from the shoulder
    for x in range(20, 45):
        for y in range(54, 60):
            if dither(x, y, (y - 53) / 7 + unit('waystone-moss', x) * 0.3) and (x, y) in colors:
                colors[(x, y)] = 'T' if y > 56 else 'g'
    colors[(24, 20)] = 'G'
    colors[(25, 19)] = 'g'
    return to_image(outline(colors))


def summit_gate():
    size = (192, 192)
    stone = Part(size)
    for x0 in (34, 134):
        stone.rectangle((x0, 62, x0 + 23, 175))  # pillars
        stone.rectangle((x0 - 4, 54, x0 + 27, 61))  # capitals
        stone.rectangle((x0 - 6, 176, x0 + 29, 190))  # plinths
    cx, cy = 96.0, 62.0
    arch = Part(size)
    arch.draw.pieslice((cx - 62, cy - 58, cx + 62, cy + 58), 180, 360, fill=255)
    hollow = Part(size)
    hollow.draw.pieslice((cx - 38, cy - 38, cx + 38, cy + 38), 180, 360, fill=255)
    ring = arch.pixels() - hollow.pixels()
    # A bite is missing from the right haunch.
    ring = {(x, y) for x, y in ring if not (x > 140 and y < 38 and x + y < 176 + (x % 5))}
    colors = blocks(stone.pixels(), course=12, seed='gate-pillars', offset=6)
    for x, y in ring:
        angle = math.degrees(math.atan2(cy - y, x - cx))
        r = math.hypot(x - cx, y - cy)
        joint = abs((angle + 3.75) % 15 - 7.5) < 0.9 * 60 / max(r, 1)
        if joint or r < 39.5 or r > 57.5:
            colors[(x, y)] = '<' if joint else '-' if r > 57.5 else '='
        else:
            colors[(x, y)] = ':' if r > 54 else '~' if (angle // 15) % 2 else '='
    # Keystone with a glowing rune.
    for x in range(86, 107):
        for y in range(2, 26):
            if abs(x - 96) <= 10 - (y - 2) * 0.12:
                colors[(x, y)] = '<' if abs(x - 96) > 9 - (y - 2) * 0.12 else ':' if y < 5 else '~'
    for dx, dy in ((0, -4), (-3, -2), (3, -2), (-3, 2), (3, 2), (0, 4), (0, 0), (-1, 0), (1, 0), (0, -1), (0, 1)):
        colors[(96 + dx, 14 + dy)] = 'C' if (dx, dy) == (0, 0) else 'c'
    # A half-raised portcullis hangs in the opening.
    portcullis = {}
    for x in range(62, 131):
        for y in range(28, 92):
            if (x, y) in hollow.pixels() or y >= 62:
                bar = (x - 62) % 9 == 0
                rail = y in (44, 64, 84)
                tooth = y > 86 and (x - 62) % 9 == 0
                if (bar and y <= 90) or (rail and 62 <= x <= 130) or tooth:
                    portcullis[(x, y)] = 'S' if bar and (x - 62) % 18 == 0 else 's' if bar else 'B'
    colors.update({p: s for p, s in portcullis.items() if p not in colors})
    # Rubble from the broken haunch lies against the right plinth.
    rubble = Part(size)
    for x, y, r in ((170, 185, 7), (180, 187, 5), (160, 188, 4)):
        rubble.ellipse((x, y), r, r * 0.7)
    ivy_mass = Part(size)
    for x, y, r in ((36, 60, 9), (32, 80, 7), (38, 100, 6), (30, 122, 5), (44, 52, 6)):
        ivy_mass.ellipse((x, y), r, r * 1.3)
    canvas = compose([
        (colors, None),
        (shade(rubble.pixels(), **STONE), 'o'),
        (leaves(ivy_mass.pixels(), 'gate-ivy', size=size), None),
    ])
    return outline(canvas, size=size)


def oak():
    size = (192, 192)
    rng = rng_for('oak-shape')
    trunk = Part(size)
    trunk.polygon([(70, 191), (80, 176), (86, 140), (84, 104), (92, 86), (100, 86), (108, 104),
                   (106, 140), (112, 176), (124, 191)])
    for points, width in (([(92, 100), (70, 74), (48, 64)], 9), ([(100, 96), (124, 72), (150, 64)], 9),
                          ([(96, 92), (98, 60)], 8), ([(80, 186), (64, 191)], 5),
                          ([(114, 186), (130, 191)], 5)):
        trunk.limb(points, width)
    bark = shade(trunk.pixels(), **BARK)
    for x, y in trunk.pixels():
        if unit('bark', x, y // 4) < 0.18 and bark[(x, y)] == 'ω':
            bark[(x, y)] = '/'
        if y > 176 and unit('bark-moss', x) < 0.4 and (x, y - 1) not in trunk.pixels():
            bark[(x, y)] = 'T'
    crown = Part(size)
    clumps = [(96, 44, 34), (58, 60, 28), (134, 60, 28), (40, 84, 20), (152, 84, 20),
              (76, 30, 24), (118, 30, 24), (96, 78, 26)]
    for cx, cy, r in clumps:
        crown.ellipse((cx, cy), r, r * 0.86)
    region = crown.pixels()
    # Scalloped edge: bite small gaps into the outline.
    region = {p for p in region if not (unit('oak-edge', p[0] // 4, p[1] // 4) < 0.18
                                        and any((p[0] + dx, p[1] + dy) not in region
                                                for dx, dy in ((4, 0), (-4, 0), (0, 4), (0, -4))))}
    foliage = leaves(region, 'oak-leaves', size=size)
    for _ in range(6):
        hx, hy = rng.randrange(50, 142), rng.randrange(30, 90)
        for p in [(hx + dx, hy + dy) for dx in range(-2, 3) for dy in range(-1, 2)]:
            foliage.pop(p, None)  # sky glimpsed through the crown
    return outline(compose([(bark, None), (foliage, 'o')]), size=size)


def ruined_arch():
    size = (256, 192)
    stone = Part(size)
    stone.rectangle((26, 44, 62, 184))  # tall pillar
    stone.rectangle((20, 36, 68, 44))  # its capital
    stone.polygon([(194, 184), (194, 112), (202, 104), (210, 110), (218, 98), (228, 106), (228, 184)])
    stone.rectangle((20, 184, 234, 191))  # buried footing
    colors = blocks(stone.pixels(), course=12, seed='arch-pillars', offset=8)
    cx, cy = 128.0, 104.0
    for x in range(20, 236):
        for y in range(0, 104):
            r = math.hypot(x - cx, y - cy)
            angle = math.degrees(math.atan2(cy - y, x - cx))
            broken = angle < 62 + 10 * unit('arch-break', round(r) // 3)
            if 70 <= r <= 96 and angle <= 180 and not broken:
                joint = abs((angle + 5) % 12 - 6) < 0.8 * 70 / r
                colors[(x, y)] = ('<' if joint else ':' if r > 93 else '-' if r < 72.5
                                  else '~' if (angle // 12) % 2 else '=')
    rubble = Part(size)
    for x, y, r in ((150, 186, 9), (168, 187, 7), (132, 189, 6), (180, 183, 5), (98, 188, 5)):
        rubble.ellipse((x, y), r, r * 0.7)
    ivy_mass = Part(size)
    for x, y, r in ((30, 46, 10), (60, 60, 8), (40, 90, 7), (66, 118, 6), (28, 140, 7)):
        ivy_mass.ellipse((x, y), r, r * 1.2)
    return outline(compose([
        (colors, None),
        (shade(rubble.pixels(), **STONE), 'o'),
        (leaves(ivy_mass.pixels(), 'arch-ivy', size=size), None),
    ]), size=size)


def root_curtain():
    size = (192, 128)
    rng = rng_for('roots')
    roots = Part(size)
    for x0 in (26, 62, 98, 138, 168):
        x, y, width = float(x0), 0.0, 6
        points = [(x, y)]
        while y < 118 and width > 0:
            y += rng.randrange(8, 16)
            x += rng.randrange(-7, 8)
            points.append((x, y))
            if rng.random() < 0.35:
                roots.limb([points[-1], (x + rng.choice((-1, 1)) * rng.randrange(8, 16),
                                         y + rng.randrange(10, 24))], max(1, width - 3))
            roots.limb(points[-2:], width)
            width -= 1
    colors = shade(roots.pixels(), **BARK)
    moss = Part(size)
    for x in range(8, 186, 10):
        moss.ellipse((x + rng.randrange(-3, 4), 2), rng.randrange(5, 9), rng.randrange(3, 6))
    return outline(compose([(colors, None), (leaves(moss.pixels(), 'root-moss', size=size), 'o')]),
                   size=size)


def fern():
    colors = {}
    for i, (angle, length) in enumerate(((-152, 28), (-122, 38), (-96, 44), (-70, 38), (-36, 28),
                                         (-164, 18), (-18, 19))):
        a = math.radians(angle)
        for step in range(length):
            t = step / length
            bend = 0.9 * t * t  # fronds arch and droop at the tip
            x = 32 + math.cos(a) * step
            y = 58 + math.sin(a) * step + bend * length * 0.5
            p = (round(x), round(y))
            colors[p] = 'T'
            if step % 3 == 1 and step < length - 1:
                leaflet = max(1, round((1 - t) * 5))
                for side in (-1, 1):
                    for k in range(1, leaflet + 1):
                        q = (round(x - math.sin(a) * k * side), round(y + math.cos(a) * k * side * 0.6 + k * 0.4))
                        colors.setdefault(q, 'G' if k == leaflet else 'g')
    return to_image(outline(colors))


def grass():
    colors = {}
    for i in range(16):
        x0 = 10 + i * 2.8 + unit('grass-x', i) * 2
        height = 12 + unit('grass-h', i) * 22
        lean = (unit('grass-lean', i) - 0.5) * 10
        for step in range(round(height)):
            t = step / height
            p = (round(x0 + lean * t * t), 59 - step)
            colors[p] = 'T' if t < 0.35 else 'g' if t < 0.8 else 'G'
        if i % 5 == 2:
            colors[(round(x0 + lean), 59 - round(height))] = 'm'
    return to_image(outline(colors))


def flowers():
    colors = {}
    petals = ('V', 'I', 'R', 'V', 'I', 'R')
    for i, (x0, height) in enumerate(((13, 26), (21, 36), (30, 22), (38, 32), (46, 27), (53, 19))):
        for step in range(height):
            colors[(x0 + round(math.sin(step / 7 + i) * 1.2), 59 - step)] = 'T'
        tx, ty = x0 + round(math.sin(height / 7 + i) * 1.2), 59 - height
        for dx, dy in ((0, -2), (-2, 0), (2, 0), (0, 2), (-1, -1), (1, -1), (-1, 1), (1, 1)):
            colors[(tx + dx, ty + dy)] = petals[i]
        colors[(tx, ty)] = 'y'
        colors[(x0 + 1, 59 - height // 2)] = 'g'
        colors[(x0 + 2, 58 - height // 2)] = 'g'
    return to_image(outline(colors))


def mushrooms():
    parts = []
    for x, height, r in ((20, 12, 7), (32, 18, 9), (44, 10, 6), (51, 6, 4)):
        stalk = Part()
        stalk.rectangle((x - 1, 59 - height, x + 1, 59))
        cap = Part()
        cap.draw.pieslice((x - r, 59 - height - r, x + r, 59 - height + r), 180, 360, fill=255)
        parts.append((shade(stalk.pixels(), 'I', shadow='σ', back='σ'), 'o'))
        cap_colors = shade(cap.pixels(), ',', light='`', shadow=')')
        for sx, sy in ((x - r // 2, 59 - height - r // 2), (x + r // 3, 60 - height - r // 3 * 2)):
            if (sx, sy) in cap_colors:
                cap_colors[(sx, sy)] = '`'
        parts.append((cap_colors, 'o'))
    return to_image(outline(compose(parts)))


def rubble():
    parts = []
    for points in ([(6, 59), (8, 50), (15, 46), (22, 49), (24, 59)],
                   [(20, 59), (24, 44), (32, 39), (41, 43), (44, 59)],
                   [(40, 59), (43, 51), (51, 48), (58, 52), (59, 59)],
                   [(28, 59), (30, 55), (36, 54), (38, 59)]):
        block = Part()
        block.polygon(points)
        colors = shade(block.pixels(), **STONE)
        for p in list(colors):
            if (p[0], p[1] - 1) not in colors and unit('rubble-moss', p) < 0.5:
                colors[p] = 'g'
        parts.append((colors, 'o'))
    return to_image(outline(compose(parts)))


def crystal():
    parts = []
    for base, tip, width in (((22, 59), (15, 26), 5), ((34, 59), (35, 16), 7), ((44, 59), (50, 32), 5)):
        body = Part()
        bx, by = base
        tx, ty = tip
        body.polygon([(bx - width, by), (bx - width, by - 6), (tx - 1, ty + 5), (tx, ty),
                      (tx + 1, ty + 5), (bx + width, by - 6), (bx + width, by)])
        inside = body.pixels()
        colors = {}
        for x, y in inside:
            # The left facet faces away from the moon; the right glows.
            left = x < bx + (tx - bx) * (by - y) / max(1, by - ty)
            colors[(x, y)] = ')' if left else ','
            if (x + 1, y) not in inside or (x, y - 1) not in inside:
                colors[(x, y)] = '`'
        parts.append((colors, 'o'))
    rocks = Part()
    rocks.polygon([(12, 59), (16, 54), (28, 53), (40, 55), (52, 54), (56, 59)])
    parts.insert(0, (shade(rocks.pixels(), **STONE), None))
    return to_image(outline(compose(parts)))


def hanging_moss():
    colors = {}
    for i, x in enumerate(range(6, 59, 4)):
        length = 8 + round(unit('moss-len', i) * 30)
        for y in range(1, length):
            sway = round(math.sin(y / 6 + i) * (y / length) * 2)
            colors[(x + sway, y)] = 't' if y < 4 else 'T' if y % 5 else 'g'
            if y % 7 == 3:
                colors[(x + sway + 1, y)] = 'g'
        colors[(x + round(math.sin(length / 6 + i) * 2), length)] = 'G'
    for x in range(4, 61):
        colors[(x, 1)] = 't'
        colors[(x, 2)] = 'T' if x % 3 else 'g'
    return to_image(outline(colors))


def waterfall_frames():
    """A 32-row loop: each frame scrolls it 8 rows and stacks it twice."""
    loop = {}
    for x in range(8, 56):
        phase = round(unit('falls', x // 3) * 32)
        for y in range(32):
            stripe = (y + phase) % 32
            if x in (8, 55):
                symbol = '|'
            elif x in (9, 54):
                symbol = '|' if stripe % 2 else '{'
            elif stripe < 3 and x % 3 == 1:
                symbol = '_'
            elif stripe < 11 and x % 3 != 0:
                symbol = '}'
            else:
                symbol = '{' if x % 5 else '|'
            loop[(x, y)] = symbol
    for x in (6, 7, 56, 57):
        for y in range(32):
            if unit('spray', x, y) < 0.2:
                loop[(x, y)] = '}'
    frames = []
    for i in range(4):
        canvas = {(x, y): loop[(x, (y - 8 * i) % 32)] for x in range(64) for y in range(64)
                  if (x, (y - 8 * i) % 32) in loop}
        frames.append(to_image(canvas))
    return frames


def spikes():
    parts = []
    beam = Part()
    beam.rectangle((5, 56, 58, 60))
    parts.append((shade(beam.pixels(), **BARK), None))
    for x in (10, 21, 32, 43, 54):
        spike = Part()
        spike.polygon([(x - 5, 56), (x, 37), (x + 5, 56)])
        inside = spike.pixels()
        colors = {}
        for px, py in inside:
            colors[(px, py)] = 'S' if px > x else 's' if px == x else 'B'
        colors[(x, 37)] = 'w'
        colors[(x, 38)] = 'S'
        for py in range(50, 56):
            if unit('rust', x, py) < 0.3 and (x - 2, py) in colors:
                colors[(x - 2, py)] = '('
        parts.append((colors, 'o'))
    return to_image(outline(compose(parts)))


def build_scenery(images, lit, animations):
    frames = [lamp_post(i) for i in range(6)]
    for i, im in enumerate(frames):
        images['lantern' if i == 0 else f'lantern_{i}'] = im
    animations['torch_flicker'] = {'looping': True, 'frames': [
        {'sprite': 'ex10_' + ('lantern' if i == 0 else f'lantern_{i}'), 'duration_ms': 100} for i in range(6)]}
    for i in range(4):
        images['vines' if i == 0 else f'vines_{i}'] = ivy(i)
    animations['vines_sway'] = {'looping': True, 'frames': [
        {'sprite': 'ex10_' + ('vines' if i == 0 else f'vines_{i}'), 'duration_ms': 200} for i in (0, 1, 2, 3)]}
    images['marker'] = waystone()
    cut(images, 'gate', summit_gate(), 3, 3)
    cut(images, 'oak', oak(), 3, 3)
    cut(images, 'arch', ruined_arch(), 4, 3)
    cut(images, 'roots', root_curtain(), 3, 2)
    for name, draw in (('fern', fern), ('grass', grass), ('flowers', flowers), ('mushrooms', mushrooms),
                       ('rubble', rubble), ('crystal', crystal), ('hanging_moss', hanging_moss),
                       ('spikes', spikes)):
        images[name] = draw()
    for i, im in enumerate(waterfall_frames()):
        images[f'waterfall_flow_{i}'] = im
    animations['waterfall_flow'] = {'looping': True, 'frames': [
        {'sprite': f'ex10_waterfall_flow_{i}', 'duration_ms': 100} for i in range(4)]}
    for name in images:
        if name.startswith(('lantern', 'vines', 'marker', 'gate_big', 'oak_big', 'arch_big', 'roots_big',
                            'fern', 'grass', 'flowers', 'rubble', 'hanging_moss', 'spikes')):
            lit.add(name)
    build_level_scenery(images, lit, animations)


# ---------------------------------------------------------------------------
# M76 (01 (d)): the new level's scenery and its water and lava, drawn as the
# art review's sixth draw drew them (`perf-runs/20261021-e01d-art-review/`).
# A prop wider or taller than a tile is cut `<name>_big_<row>_<col>`; one
# that fits sits in a tile, on its foot (or hung from its top). Water and
# lava are looping clips whose frames repeat every 64 pixels across and
# down, so tiles of them join.


def shaded(part, palette, **extra):
    return shade(part.pixels(), **{**palette, **extra})


def finish(colors, size=(TILE, TILE)):
    return to_image(outline(colors, size=size), size)


BASALT_SHADE = dict(base='β', light='ξ', shadow='ν', back='γ', rim='α')


BONE = dict(base='w', light='I', shadow='τ', back='σ')


MAROON = dict(base='X', light='E', shadow='M', back='Θ', rim='E')


STEEL = dict(base=':', light=';', shadow='-', back='=')


DUST = dict(base='Φ', light='B', shadow='i', back='b', rim='Ω')


ORE = dict(base='(', light='a', shadow='>', back='/', rim='A')


WHITE_MIST = ('%', '^', '&', '*', 'p', 'q')


def falls(rows, frame=0):
    """The ravine's falls whole: the channel's water leaving its end at
    x = 64 as a glassy lip, arcing out and spreading over two rows, then a
    curtain two tiles wide: a dark back sheet, streaks of two lengths and
    speeds (each a 64-px loop at 16 px a frame), bright heads, surges, thin
    translucent edges; then the plunge, a heap of foam wider than the
    curtain with spray thrown in arcs and rings on the pool. Three tiles
    wide, `rows` rows down to the pool's surface."""
    width, pool_y = 3 * TILE, rows * TILE
    c = {}

    def edges(y):
        wob = 1.5 * math.sin(math.tau * y / TILE)
        right = TILE + 8 + 90 * (1 - math.exp(-max(0, y - 2) / 34)) + wob
        left = TILE - 2 + 16 * (1 - math.exp(-max(0, y - 11) / 46)) - wob
        if y > pool_y - 40:
            spread = (y - (pool_y - 40)) * 0.45
            left, right = left - spread, right + spread
        return left, right

    for y in range(2, pool_y + 2):
        left, right = edges(y)
        for x in range(math.floor(left) - 1, math.ceil(right) + 2):
            r = (x - left) / max(1.0, right - left)
            if r < -0.03 or r > 1.03:
                continue
            yy = y - 16 * frame
            l1, b1 = 8 + unit('fall-l', x) * 30, unit('fall-b', x)
            s1 = (yy + unit('fall-p', x) * TILE) % TILE
            s2 = (2 * yy + unit('fall-p2', x) * TILE) % TILE
            surge = 0.5 + 0.5 * math.sin(math.tau * yy / TILE + x * 0.07)
            edge = min(r, 1 - r)
            if y < 24:
                along = (y + (x - TILE) * 0.35) % 6
                sym = '`' if r > 0.9 else '_' if (y < 6 and r < 0.35) else '}' if along < 2 else ')'
            elif edge < 0.08:
                if not dither(x, y, 0.35 + edge * 6):
                    continue
                sym = 'ϰ' if s1 < l1 else 'ς'
            elif s1 < 2 and b1 > 0.45:
                sym = '`'
            elif s1 < l1:
                sym = '_' if b1 > 0.85 and s1 < 7 else ',' if b1 > 0.55 else '}'
            elif s2 < 3 + unit('fall-l2', x) * 10 and surge > 0.55:
                sym = '}'
            else:
                sym = ')' if b1 > 0.3 or surge > 0.75 else '{'
            c[(x, y)] = sym
    for y in range(2, 32):
        _, right = edges(y)
        c[(round(right), y)] = '`'
        if y < 16:
            c[(round(right) - 1, y)] = '_'
    left, right = edges(pool_y)
    mid, half = (left + right) / 2, (right - left) / 2
    for y in range(pool_y - 4, pool_y + 6):
        for x in range(round(mid - half - 44), round(mid + half + 44)):
            v = max(0.0, 1 - abs(x - mid) / (half + 44)) * (1 - abs(y - pool_y) / 7)
            if v > 0 and dither(x, y, v * 1.5):
                c[(x, y)] = '_' if dither(x, y + 3, 0.6) else '}'
    for k in range(80):
        spread = (0.25 + 0.75 * unit('foam-r', k)) * (half + 30)
        bx = mid + (1 if k % 2 else -1) * spread * unit('foam-x', k)
        by = pool_y - 2 - unit('foam-y', k) ** 1.7 * 26 * (1 - abs(bx - mid) / (half + 40))
        rad = 1 + int(unit('foam-size', k) * 4 * max(0.3, 1 - abs(bx - mid) / (half + 40)))
        for dy in range(-rad, rad + 1):
            for dx in range(-rad, rad + 1):
                if dx * dx + dy * dy <= rad * rad:
                    c[(round(bx + dx), round(by + dy))] = '`' if dx + dy < -rad // 2 else '_' if dx + dy < rad // 2 + 1 else ','
    for k in range(60):
        side, speed, t = (1 if k % 2 else -1), 0.6 + unit('spray-v', k) * 1.4, unit('spray-t', k)
        x = mid + side * (half * 0.5 + speed * t * 56)
        y = pool_y - 10 - (speed * 64 * t - 52 * t * t)
        c[(round(x), round(y))] = '`' if k % 3 else ','
    for ring in range(3):
        y = pool_y + 8 + ring * 5
        for x in range(round(mid - half - 30 - ring * 26), round(mid + half + 30 + ring * 26)):
            if dither(x, y, 0.5 - ring * 0.12):
                c[(x, y)] = '_' if ring == 0 and dither(x, y + 1, 0.5) else '}'
    return to_image(c, (width, pool_y + 24))


def falls_tiles(frame=0):
    """The falls' pieces as the run cuts them: the lip and its spread (top
    two rows), the curtain's left and right halves (a 64-px loop) and the
    plunge."""
    im = falls(4, frame)
    return [(name, im.crop((col * TILE, row * TILE, (col + 1) * TILE, (row + 1) * TILE)))
            for name, col, row in (('falls_lip', 1, 0), ('falls_spread_left', 1, 1), ('falls_spread_right', 2, 1),
                                   ('falls_left', 1, 2), ('falls_right', 2, 2), ('falls_foot_left', 1, 3),
                                   ('falls_foot_right', 2, 3))]


def plume(width, height, seed, strength=1.0, phase=0.0):
    """A mist cloud in the palette's translucent whites (alpha 8 to 68),
    thickest low in its middle, billowing slowly, its levels dithered so it
    has no bands or holes: the falls' plunge, steam. `phase` (0 to 1) turns
    its billows through one rising cycle, so frames at even phases loop."""
    c = {}
    turn = math.tau * phase
    for y in range(height):
        for x in range(width):
            dx = (x - width / 2) / (width / 2)
            dy = (y - height * 0.72) / (height * (0.72 if y < height * 0.72 else 0.28))
            fall = max(0.0, 1 - dx * dx - dy * dy) ** 1.4
            billow = (0.82 + 0.18 * math.sin(x / 11 + seed + turn) * math.cos(y / 9 - x / 29 + seed + 2 * turn)
                      + 0.06 * math.sin(y / 23 + 2 * turn) * math.cos(x / 17 - turn))
            v = fall * billow * strength * len(WHITE_MIST)
            level = int(v) + (1 if dither(x, y, v - int(v)) else 0)
            if level > 0:
                c[(x, y)] = WHITE_MIST[min(level, len(WHITE_MIST)) - 1]
    return to_image(c, (width, height))


def fissure(height, seed=0):
    """A glowing gash in the back wall, where the lavafall pours out
    (e01hollow): an almond slot 30 px wide, white-hot in its throat, its
    lips crusted dark and cracked red, its lower lip open where the stream
    leaves."""
    width = 32
    c = {}
    for y in range(height):
        t = y / max(1, height - 1)
        half = 2 + round(10 * math.sin(math.pi * min(1.0, t * 0.85 + 0.08)) + unit('fissure', seed, y // 3) * 2)
        if t > 0.75:
            half = max(half, 9)
        cx = width // 2 + round(math.sin(y * 0.35 + seed) * 1.2)
        for x in range(cx - half - 3, cx + half + 4):
            d = abs(x + 0.5 - cx) / max(1, half)
            if d < 0.35:
                sym = 'Z' if t > 0.3 and d < 0.15 else 'Y'
            elif d < 0.65:
                sym = 'y'
            elif d < 0.9:
                sym = 'Ъ'
            elif d <= 1.0:
                sym = 'L'
            elif d <= 1.0 + 1.5 / max(1, half):
                sym = 'M' if unit('lip', seed, x, y // 2) < 0.6 else 'X'
            else:
                sym = 'θ' if unit('crust', seed, x, y // 2) < 0.7 else 'ν'
            c[(x, y)] = sym
    return to_image(c, (width, height))


# The x the gargoyle's water leaves its jaws at, where its pour stands.
GARGOYLE_MOUTH = 32


def limestone(pixels, base='σ', light='w', mid='τ', dark='S'):
    """Pale limestone shading for a mask lit from the upper right: tops and
    right-facing edges catch the moon, bottoms and left-facing edges fall
    into shade, the body a dither between `base` and `mid` by height."""
    c = {}
    if not pixels:
        return c
    top, bottom = min(y for _, y in pixels), max(y for _, y in pixels)
    for x, y in pixels:
        v = (y - top) / max(1, bottom - top)
        sym = base if not dither(x, y, max(0.0, v - 0.45) * 1.6) else mid
        if (x, y - 1) not in pixels or (x + 1, y) not in pixels:
            sym = light
        if (x, y + 1) not in pixels:
            sym = dark
        elif (x - 1, y) not in pixels:
            sym = mid
        c[(x, y)] = sym
    return c


def gargoyle(frame):
    """The channel's gargoyle (01 (k)), frame `frame` of 4: a horned beast
    crouched on a stepped corbel and leaning out of the channel's face
    toward the viewer, seen from a little below, in pale weathered
    limestone apart from the blue-grey masonry and the arcade: its crown
    seen from above under moss, a heavy brow shading deep eye hollows with
    a faint green glint, a foreshortened snout, jaws wide with fangs over
    the jaw's shadowed underside, forelegs whose claws grip the corbel,
    lichen and water stains, a shadow cast on the wall, and the channel's
    water pouring over its tongue."""
    def mask(draw):
        part = Part()
        draw(part)
        return part.pixels()
    corbel = mask(lambda q: q.polygon([(15, 47), (49, 47), (47, 52), (42, 52), (40, 57), (24, 57), (22, 52), (17, 52)]))
    arm_l = mask(lambda q: q.limb([(13, 30), (9, 40), (14, 48)], 6))
    arm_r = mask(lambda q: q.limb([(51, 30), (55, 40), (50, 48)], 6))
    horn_l = mask(lambda q: (q.limb([(19, 10), (13, 5), (9, 4)], 4), q.limb([(9, 4), (6, 7)], 2)))
    horn_r = mask(lambda q: (q.limb([(45, 10), (51, 5), (55, 4)], 4), q.limb([(55, 4), (58, 7)], 2)))
    ear_l = mask(lambda q: q.polygon([(13, 16), (4, 13), (9, 18), (13, 24)]))
    ear_r = mask(lambda q: q.polygon([(51, 16), (60, 13), (55, 18), (51, 24)]))
    head = mask(lambda q: q.polygon([(16, 9), (32, 6), (48, 9), (53, 18), (52, 31), (45, 38), (19, 38), (12, 31), (11, 18)]))
    jaw = mask(lambda q: q.polygon([(20, 40), (44, 40), (41, 47), (23, 47)]))
    c = compose([
        (limestone(corbel, 'τ', 'σ', 'S', 's'), 'o'),
        (limestone(arm_l, 'τ', 'σ', 'S', 's'), 'o'),
        (limestone(arm_r), 'o'),
        (limestone(horn_l, 'τ', 'σ', 'S', 's'), 'o'),
        (limestone(horn_r), 'o'),
        (limestone(ear_l, 'τ', 'σ', 'S', 's'), 'o'),
        (limestone(ear_r), 'o'),
        (limestone(jaw, 'S', 'τ', 's', 's'), 'o'),
        (limestone(head), 'o'),
    ])
    # Claws gripping the corbel's corners.
    for x0, sign in ((15, 1), (49, -1)):
        for k in range(3):
            x = x0 + sign * (k * 3)
            c[(x, 48)], c[(x, 49)], c[(x + sign, 50)] = 'w', 'σ', 'o'
    # The crown seen from above, a lit plane; the brow a shelf casting
    # shadow into the eye hollows.
    for x in range(17, 48):
        for y in range(8, 12):
            if (x, y) in c and c[(x, y)] not in ('o',):
                c[(x, y)] = 'w' if dither(x, y, 0.5) else 'σ'
    for x in range(14, 51):
        if (x, 15) in c:
            c[(x, 14)], c[(x, 15)] = 'I' if x > 32 else 'w', 'S'
            if (x, 16) in c:
                c[(x, 16)] = 's'
    for ex in (24, 40):
        for dy in range(0, 6):
            for dx in range(-4, 5):
                if (dx / 4.5) ** 2 + ((dy - 2.5) / 3) ** 2 <= 1:
                    c[(ex + dx, 17 + dy)] = '■' if abs(dx) < 3 and 1 <= dy <= 4 else 'o'
        c[(ex + 1, 19)] = 'c'
    # The snout, foreshortened toward us, its nostrils on its front.
    snout = {(x, y) for x in range(25, 40) for y in range(24, 33) if ((x - 32) / 7.5) ** 2 + ((y - 28) / 4.8) ** 2 <= 1}
    for (x, y), sym in limestone(snout, 'σ', 'I', 'τ', 'S').items():
        c[(x, y)] = sym
    for nx in (29, 35):
        c[(nx, 29)], c[(nx, 30)], c[(nx + (1 if nx < 32 else -1), 30)] = 's', 'o', 'o'
    # Jaws wide under the snout: the dark throat, fangs, a tongue lip.
    for y in range(34, 43):
        for x in range(18, 47):
            if ((x - 32) / 13) ** 2 + ((y - 38) / 4) ** 2 <= 1:
                c[(x, y)] = '■' if abs(x - 32) < 9 and 35 <= y <= 41 else 'o'
    for x, length in ((21, 4), (43, 4), (26, 2), (30, 1), (34, 1), (38, 2)):
        for d in range(length):
            c[(x, 35 + d)] = 'I' if d < length - 1 else 'w'
    for x, length in ((24, 2), (40, 2)):
        for d in range(length):
            c[(x, 41 - d)] = 'w'
    for x in range(28, 37):
        c[(x, 41)] = 'τ' if 29 <= x <= 35 else 'S'
        c[(x, 42)] = 'S'
    # Moss on the crown and the horns' roots; lichen; stains; a crack.
    for x in range(16, 49):
        if unit('gm4', x // 2) < 0.7:
            ys = sorted(y for (xx, y) in c if xx == x and y < 12 and c[(xx, y)] != 'o')
            if ys:
                for y in ys[:1 + int(unit('gm4l', x) * 3)]:
                    c[(x, y)] = 'T' if y == ys[0] else 'g'
    for n in range(14):
        q = (14 + int(unit('lich4', n) * 36), 18 + int(unit('lich4y', n) * 16))
        if c.get(q) in ('σ', 'τ', 'w'):
            c[q] = 'G' if n % 3 else 'm'
    for x0, y0, n in ((22, 23, 6), (42, 23, 6), (19, 43, 4), (45, 43, 4)):
        for d in range(n):
            q = (x0, y0 + d)
            if c.get(q) in ('σ', 'τ', 'w', 'S'):
                c[q] = 's' if d < n - 2 else 'S'
    for d in range(7):
        q = (47 - d // 2, 19 + d)
        if c.get(q) in ('σ', 'w', 'τ', 'I'):
            c[q] = 's'
    outline(c, 'o')
    # The shadow it casts down and to its left on the channel's face.
    for (x, y) in list(c):
        q = (x - 3, y + 4)
        if q not in c and 0 <= q[0] < TILE and 0 <= q[1] < TILE:
            c[q] = 'λ'
    # The channel's water pouring over its tongue, to the tile's foot.
    for y in range(41, TILE):
        half = 3 + (y - 41) / 10
        for x in range(round(GARGOYLE_MOUTH - half) - 1, round(GARGOYLE_MOUTH + half) + 2):
            if y < 48 and c.get((x, y)) not in (None, 'λ', 'S', 'τ', 'o', 's'):
                continue
            d = abs(x - GARGOYLE_MOUTH)
            v = (y - 16 * frame + x * 5) % 32
            if d > half:
                if dither(x, y + 4 * frame, 0.35):
                    c[(x, y)] = 'ϰ'
            elif d > half - 1:
                c[(x, y)] = 'ς'
            else:
                c[(x, y)] = '`' if v < 2 else '_' if (v + x) % 7 == 0 else '}' if (x + y) % 3 else ')'
    return to_image(c)


# The x the second gargoyle's water falls at, where its pour stands.
DRAKE_MOUTH = 56


def gargoyle_drake(frame):
    """The channel's second gargoyle (e01hollow), frame `frame` of 4: apart
    from the first in pose, a winged drake in profile crouched on a stepped
    corbel and leaning out to the right, its haunch and tail curled over
    the corbel's side, a foreleg's claws on its edge, a bat wing raised
    behind (its tip broken off), its neck stretched forward to a horned
    head whose jaws gape and spill the channel's water in an arc that falls
    to the tile's foot; the same pale weathered limestone as the first,
    mossed on its upper surfaces, lichened, cracked, its shadow cast on the
    wall."""
    def mask(draw):
        part = Part()
        draw(part)
        return part.pixels()
    corbel = mask(lambda q: q.polygon([(3, 43), (32, 43), (30, 47), (26, 47), (24, 52), (11, 52), (9, 47), (3, 47)]))
    # The wing raised up and back: its arm from the shoulder to the wrist,
    # three fingers fanning back and down, the membrane scalloped between.
    WRIST, FINGERS = (19, 3), ((4, 9), (4, 19), (11, 25))
    arm = mask(lambda q: q.limb([(26, 22), (22, 12), WRIST], 4))
    web = Part()
    web.polygon([(26, 22), WRIST, *FINGERS, (20, 25)])
    for a, b in zip((WRIST, *FINGERS), FINGERS):
        mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2
        web.draw.ellipse((round(mx - 5), round(my - 5), round(mx + 4), round(my + 4)), fill=0)
    wing = web.pixels() | arm
    tail = mask(lambda q: (q.limb([(8, 38), (3, 44), (4, 51), (9, 56)], 4),
                           q.polygon([(8, 55), (14, 54), (12, 60)])))
    haunch = mask(lambda q: q.ellipse((14, 35), 8, 7))
    body = mask(lambda q: q.polygon([(8, 30), (16, 23), (27, 21), (34, 25), (35, 31), (29, 37), (19, 41), (10, 41)]))
    neck = mask(lambda q: q.limb([(29, 27), (35, 23), (40, 23)], 7))
    leg = mask(lambda q: q.limb([(29, 32), (31, 38), (28, 42)], 5))
    horn_far = mask(lambda q: (q.limb([(46, 18), (45, 12), (42, 8)], 3), q.limb([(42, 8), (40, 7)], 2)))
    horn = mask(lambda q: (q.limb([(42, 18), (38, 12), (33, 10)], 4), q.limb([(33, 10), (30, 11)], 2)))
    head = mask(lambda q: q.polygon([(38, 18), (44, 15), (50, 17), (55, 22), (56, 26), (52, 27), (45, 28), (40, 27),
                                     (37, 23)]))
    jaw = mask(lambda q: q.polygon([(41, 30), (51, 30), (54, 32), (48, 35), (41, 34)]))
    c = compose([
        (limestone(corbel, 'τ', 'σ', 'S', 's'), 'o'),
        ({p: 's' if dither(*p, 0.75) else 'S' for p in wing - arm}, 'o'),
        (limestone(arm, 'τ', 'σ', 'S', 's'), None),
        (limestone(horn_far, 'τ', 'σ', 'S', 's'), 'o'),
        (limestone(tail, 'τ', 'σ', 'S', 's'), 'o'),
        (limestone(body), 'o'),
        (limestone(haunch), None),
        (limestone(neck), 'o'),
        (limestone(leg), 'o'),
        (limestone(jaw, 'S', 'τ', 's', 's'), 'o'),
        (limestone(head), 'o'),
        (limestone(horn), 'o'),
    ])
    # The wing's fingers, lit bones over the shadowed membrane, the last
    # one's tip broken off; a claw at the wrist.
    for tip in FINGERS:
        for t in range(1, 14):
            q = (round(WRIST[0] + (tip[0] - WRIST[0]) * t / 14), round(WRIST[1] + (tip[1] - WRIST[1]) * t / 14))
            if q in c and c[q] != 'o':
                c[q] = 'w' if t < 5 else 'σ' if t < 10 else 'τ'
    for q in ((11, 25), (12, 25), (11, 24), (12, 24), (13, 25)):
        c.pop(q, None)
    c[(WRIST[0], WRIST[1] - 1)], c[(WRIST[0] + 1, WRIST[1] - 2)] = 'w', 'I'
    # Claws on the corbel's edge and the hind foot on its top.
    for x0 in (27, 30, 33):
        c[(x0, 43)], c[(x0, 44)], c[(x0 + 1, 45)] = 'w', 'σ', 'o'
    for x0 in (9, 12, 15, 18):
        c[(x0, 42)], c[(x0 + 1, 43)] = 'w', 'o'
    # The head: a heavy brow over a deep socket with a faint green glint,
    # a nostril at the snout's tip, fangs over the gaping jaw, the throat
    # dark.
    for x in range(44, 52):
        if (x, 19) in c:
            c[(x, 19)] = 'I' if x > 47 else 'w'
            c[(x, 20)] = 'S'
    for dx, dy in ((0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1), (1, 2)):
        c[(47 + dx, 21 + dy)] = '■'
    c[(48, 21)] = 'c'
    c[(54, 23)], c[(54, 24)] = 's', 'o'
    for y in range(28, 31):
        for x in range(41, 55):
            if (x, y) not in c or c[(x, y)] in ('S', 's', 'o'):
                c[(x, y)] = '■' if 43 <= x <= 52 and y >= 28 else 'o'
    for x, length in ((44, 2), (47, 1), (50, 2), (53, 2)):
        for d in range(length):
            c[(x, 28 + d)] = 'I' if d == 0 else 'w'
    for x in (45, 49):
        c[(x, 30)] = 'w'
    for x in range(41, 52):
        c[(x, 31)] = 'τ' if 43 <= x <= 50 else c.get((x, 31), 'S')
    # Moss on the upper surfaces, lichen, a crack down the haunch.
    for x in range(8, 52):
        ys = sorted(y for (xx, y) in c if xx == x and c[(xx, y)] not in ('o', '■'))
        if ys and unit('dm', x // 2) < 0.55:
            for y in ys[:1 + int(unit('dml', x) * 2)]:
                c[(x, y)] = 'T' if y == ys[0] else 'g'
    for n in range(12):
        q = (6 + int(unit('dl', n) * 48), 14 + int(unit('dly', n) * 30))
        if c.get(q) in ('σ', 'τ', 'w'):
            c[q] = 'G' if n % 3 else 'm'
    for d in range(8):
        q = (15 + d // 3, 30 + d)
        if c.get(q) in ('σ', 'w', 'τ', 'I'):
            c[q] = 's'
    # The haunch's crease where it folds against the body.
    for a in range(0, 14):
        q = (round(14 + 8 * math.cos(math.radians(200 + a * 9))), round(35 + 7 * math.sin(math.radians(200 + a * 9))))
        if c.get(q) in ('σ', 'w', 'τ', 'I'):
            c[q] = 'S'
    outline(c, 'o')
    # Its shadow on the wall, down and to the left.
    for (x, y) in list(c):
        q = (x - 3, y + 4)
        if q not in c and 0 <= q[0] < TILE and 0 <= q[1] < TILE:
            c[q] = 'λ'
    # The water: out of the jaws in an arc, then falling plumb to the foot.
    for y in range(29, TILE):
        t = min(1.0, (y - 29) / 9)
        centre = 52 + (DRAKE_MOUTH - 52) * math.sin(t * math.pi / 2)
        half = 1.5 + 1.5 * t + max(0, y - 38) / 12
        for x in range(round(centre - half) - 1, round(centre + half) + 2):
            if not 0 <= x < TILE or (y < 36 and c.get((x, y)) not in (None, 'λ', '■', 'o', 's', 'S', 'τ')):
                continue
            d = abs(x - centre)
            v = (y - 16 * frame + x * 5) % 32
            if d > half:
                if dither(x, y + 4 * frame, 0.35):
                    c[(x, y)] = 'ϰ'
            elif d > half - 1:
                c[(x, y)] = 'ς'
            else:
                c[(x, y)] = '`' if v < 2 else '_' if (v + x) % 7 == 0 else '}' if (x + y) % 3 else ')'
    return to_image(c)


def splash_rocks(i):
    """Boulders at the falls' foot in the ravine's wet stone, glistening on
    top, mossed, foam lapping their feet."""
    size = (2 * TILE, TILE)
    layers = []
    for cx, cy, rx, ry in ((34, 52, 22, 14), (72, 56, 15, 10), (100, 58, 10, 6)):
        rock = Part(size)
        rock.ellipse((cx + (i % 2) * 6, cy), rx, ry)
        colors = shade(rock.pixels(), 'η', light=';', shadow='n', back='d', rim=':')
        for (x, y) in list(colors):
            if y < cy - ry * 0.35 and dither(x, y, 0.3):
                colors[(x, y)] = ',' if (x + y) % 3 else '`'
            elif y > cy + ry * 0.3:
                colors[(x, y)] = 'd' if dither(x, y, 0.5) else 'n'
            if y < cy and unit('rock-moss', i, x // 3, y // 2) < 0.14:
                colors[(x, y)] = 'Q'
        layers.append((colors, 'o'))
    c = compose(layers)
    cols = sorted({x for (x, y) in c if y >= 50})
    for x in range(2 * TILE):
        near = min((abs(x - col) for col in cols), default=99)
        for y in range(57, TILE):
            if dither(x, y, max(0.0, 1 - near / 8) * (0.9 - (y - 57) * 0.08)) and (x, y) not in c:
                c[(x, y)] = '_' if unit('lap', i, x, y) < 0.55 else '`' if unit('lap', i, x, y) < 0.8 else '}'
    for k in range(24):
        c[(4 + int(unit('splash-dot', i, k) * 120), 14 + int(unit('splash-dot-y', i, k) * 26))] = '`' if k % 2 else ','
    return to_image(c, size)


def vine(length, seed=0, flowers=False, width=26):
    """A hanging vine: a thin stem swaying down, leaves paired on it and
    smaller toward its tip, moonflowers on some."""
    size = (width, length)

    def at(y):
        return width / 2 + 3.5 * math.sin(y / 21 + seed) + 1.5 * math.sin(y / 8 + 2 * seed)
    stem, leaves, blooms = Part(size), Part(size), Part(size)
    stem.limb([(at(y), y) for y in range(0, length, 3)] + [(at(length - 1), length - 1)], 1)
    for k, y in enumerate(range(3 + int(unit('vine', seed) * 4), length - 3, 5)):
        x, side = at(y), (1 if k % 2 else -1)
        n = (1 - y / length * 0.45) * (3 + unit('vine-leaf', seed, k) * 2.5)
        leaves.polygon([(x, y), (x + side * n, y - 1.5), (x + side * (n + 2.5), y + n * 0.6), (x + side * 1.5, y + n * 0.9)])
        if flowers and unit('vine-bloom', seed, k) < 0.2:
            blooms.ellipse((x - side * 3, y + 2), 2, 2)
    c = compose([
        (shade(stem.pixels(), 't', light='T', shadow='N'), None),
        (shaded(leaves, LEAF), None),
        (shade(blooms.pixels(), 'w', light='I', shadow='σ'), None),
    ])
    return finish(c, size)


def vines(lengths, seed=0, gap=8, flowers=True):
    """A curtain of vines side by side, of `lengths`."""
    width = 26 + gap * (len(lengths) - 1)
    im = Image.new('RGBA', (width, max(lengths)))
    for k, length in enumerate(lengths):
        im.alpha_composite(vine(length, seed * 7 + k, flowers and k % 2 == 0), (k * gap, 0))
    return im


def fern_clump(i, size=(80, 48)):
    """A fern clump: seven arching fronds from one crown, pinnae paired along
    each, tips drooping."""
    w, h = size
    layers = []
    for f in range(7):
        a = math.pi * (0.16 + 0.68 * f / 6) + (unit('clump-a', i, f) - 0.5) * 0.18
        length = h * (0.78 + 0.22 * unit('clump-l', i, f))
        pts = []
        for d in range(0, int(length), 2):
            t = d / length
            pts.append((w / 2 + math.cos(a) * d * 1.05, h - 2 - math.sin(a) * d * 1.15 + t * t * length * 0.32))
        rachis, pinnae = Part(size), Part(size)
        rachis.limb(pts, 1)
        for j, (x, y) in enumerate(pts[1:]):
            n = (1 - j / len(pts)) * 5 + 1
            for side in (-1, 1):
                pinnae.limb([(x, y), (x + side * n * math.sin(a) + math.cos(a) * 1.5, y + side * n * math.cos(a) - math.sin(a) * 1.5 + 1)], 1)
        layers += [(shaded(pinnae, FROND), None), (shade(rachis.pixels(), 'P', light='U'), None)]
    return finish(compose(layers), size)


def grass_tuft(i, width=40, height=16):
    """Blades in a clump, leaning, lit at their tips."""
    size = (width, height)
    blades = Part(size)
    for k in range(9):
        x = 4 + unit('tuft', i, k) * (width - 8)
        hgt = 6 + unit('tuft-h', i, k) * (height - 8)
        lean = (unit('tuft-lean', i, k) - 0.5) * 8
        blades.limb([(x, height - 1), (x + lean * 0.5, height - 1 - hgt * 0.6), (x + lean, height - 1 - hgt)], 1)
    return finish(shade(blades.pixels(), 'U', light='W', shadow='P', rim='G'), size)


def cliff_tree(i):
    """A gnarled tree leaning out of the ravine's east rim: roots gripping
    the stone, a trunk bent toward the gorge, leaf clusters lit moonward
    and dangling strands."""
    size = (160, 128)
    trunk, upper, boughs, roots, crown = (Part(size) for _ in range(5))
    trunk.limb([(150, 124), (138, 114), (118, 106), (98, 100)], 7)
    upper.limb([(98, 100), (80, 94), (64, 90)], 5)
    boughs.limb([(122, 108), (128, 88), (124, 72)], 3)
    boughs.limb([(90, 98), (64, 86), (42, 86)], 3)
    boughs.limb([(74, 94), (58, 104), (44, 112)], 2)
    roots.limb([(150, 124), (159, 126)], 3)
    roots.limb([(146, 120), (134, 127)], 2)
    roots.limb([(152, 118), (158, 108)], 2)
    for cx, cy, rx, ry in ((58, 74, 26, 15), (94, 66, 23, 14), (126, 76, 17, 11), (36, 94, 16, 10), (76, 88, 18, 10)):
        crown.ellipse((cx, cy), rx, ry)
    leaves = {}
    for (x, y) in crown.pixels():
        if unit('leaf-hole', i, x // 3, y // 3) < 0.12:
            continue
        n = unit('leaf', i, x // 2, y // 2)
        lit = (x - y * 0.6) > 10 and n < 0.6
        leaves[(x, y)] = 'G' if lit and n < 0.18 else 'g' if lit else 't' if n > 0.82 else 'T'
    c = compose([
        (shaded(boughs, BARK, light="'"), None),
        (shaded(roots, BARK, light="'"), None),
        (shaded(trunk, BARK, light="'", rim='"'), None),
        (shaded(upper, BARK, light="'", rim='"'), None),
        (leaves, 'o'),
    ])
    for k in range(4):
        x0 = 30 + int(unit('dangle', i, k) * 90)
        y0 = max((y for (x, y) in leaves if x == x0), default=None)
        if y0 is None:
            continue
        for d in range(1, 10 + int(unit('dangle-l', i, k) * 14)):
            c[(x0 + round(math.sin(d * 0.4 + k)), y0 + d)] = 'T' if d % 3 else 'g'
    return finish(c, size)


def glowcaps(i):
    """Pale mushrooms on a wet ledge, their caps in the two greens that glow
    (`c`, `C`), stems pale, spores drifting up."""
    size = (48, 28)
    layers = []
    for k in range(5):
        cx = 6 + int(unit('cap', i, k) * 36)
        hgt = 5 + int(unit('cap-h', i, k) * 10)
        rad = 3 + int(unit('cap-r', i, k) * 3)
        lean = (unit('cap-lean', i, k) - 0.5) * 4
        stem, cap = Part(size), Part(size)
        stem.limb([(cx, 27), (cx + lean, 27 - hgt)], 2)
        cap.ellipse((cx + lean, 27 - hgt), rad, max(2, rad * 2 // 3))
        dome = {(x, y) for (x, y) in cap.pixels() if y <= 27 - hgt}
        layers += [(shade(stem.pixels(), 'σ', light='w', shadow='τ'), None),
                   (shade(dome, 'c', light='C', shadow='D', rim='C'), 'o')]
    c = compose(layers)
    for k in range(6):
        c[(4 + int(unit('spore', i, k) * 40), 2 + int(unit('spore-y', i, k) * 8))] = 'c'
    return to_image(outline(c, size=size), size)


def lily_pads(i):
    """Pads afloat on the pool's calm end, each notched, one in flower."""
    size = (128, 14)
    layers = []
    for k in range(4):
        cx, rx, ry = 12 + k * 30 + int(unit('pad', i, k) * 10), 7 + int(unit('pad-r', i, k) * 5), 2 + k % 2
        pad = Part(size)
        pad.ellipse((cx, 9), rx, ry)
        notch = unit('pad-notch', i, k) * math.tau
        inside = {(x, y) for (x, y) in pad.pixels()
                  if abs((math.atan2(y - 9, x - cx) - notch + math.pi) % math.tau - math.pi) > 0.35 or math.hypot(x - cx, y - 9) < 2}
        layers.append((shade(inside, 'Q', light='U', shadow='P', rim='g'), 'o'))
    bloom = Part(size)
    bloom.ellipse((44, 6), 3, 2)
    layers.append((shade(bloom.pixels(), 'w', light='I', shadow='σ'), 'o'))
    return finish(compose(layers), size)


def boulder(i, size=(64, 40)):
    """A broken boulder of the cut's rock, cracked across, ore flecked."""
    w, h = size
    pts = []
    for k in range(11):
        a = math.pi * k / 10
        r = 0.72 + 0.28 * unit('boulder', i, k)
        pts.append((w / 2 + math.cos(a) * (w / 2 - 2) * r, h - 2 - math.sin(a) * (h - 4) * r))
    body = Part(size)
    body.polygon(pts + [(w / 2 - (w / 2 - 2) * 0.9, h - 2)])
    c = shade(body.pixels(), **DUST)
    for (x, y) in list(c):
        if c[(x, y)] == 'Φ' and (x - y * 0.5 + unit('facet', i) * 20) % 17 < 1.2:
            c[(x, y)] = 'b'
        if unit('boulder-ore', i, x // 2, y // 2) < 0.035:
            c[(x, y)] = 'a'
    for k in range(2):
        x, y = int(w * (0.3 + 0.4 * unit('boulder-crack', i, k))), int(h * 0.3)
        for d in range(int(h * 0.55)):
            x += 1 if unit('boulder-cx', i, k, d) > 0.62 else -1 if unit('boulder-cx', i, k, d) < 0.25 else 0
            if (x, y + d) in c:
                c[(x, y + d)] = 'o'
            if (x + 1, y + d) in c:
                c[(x + 1, y + d)] = 'B'
    return finish(c, size)


def rubble_pile(i, size=(96, 40), palette=None):
    """A heap of broken stones: a mound of grit with stones bedded in it and
    on it, bigger toward its foot, a few rusted lumps."""
    w, h = size
    pal = palette or DUST

    def crown(x):
        t = min(1.0, max(0.0, (x - 2) / (w - 4)))
        return h - 2 - (h - 12) * math.sin(math.pi * t) ** 0.8 * (0.85 + 0.15 * unit('mound', i, int(x) // 8))
    mound = Part(size)
    mound.polygon([(2, h - 1)] + [(x, crown(x)) for x in range(4, w - 3, 4)] + [(w - 2, h - 1)])
    c = shade(mound.pixels(), pal['shadow'], light=pal['base'], shadow=pal['back'])
    for (x, y) in list(c):
        if unit('grit', i, x // 2, y // 2) < 0.18:
            c[(x, y)] = pal['base']
    for k in range(34):
        x = 6 + unit('rubble', i, k) * (w - 12)
        top = crown(x)
        y = top + 2 + unit('rubble-y', i, k) * max(1.0, h - 4 - top)
        r = 2 + int(unit('rubble-r', i, k) * 3 * (0.6 + 0.4 * (y - top) / max(1.0, h - top)))
        stone = Part(size)
        stone.ellipse((x, y), r + 1, r)
        c.update(shade(stone.pixels(), **(ORE if palette is None and unit('rubble-ore', i, k) < 0.12 else pal)))
    return finish(c, size)


def ore_heap(i):
    """Ore heaped where the carts tip it: rusted lumps and grey stone, a
    shovel driven into its side."""
    size = (96, 48)
    c = from_image(rubble_pile(i + 50, size, ORE))
    shovel, blade = Part(size), Part(size)
    shovel.limb([(70, 6), (62, 34)], 2)
    blade.polygon([(57, 32), (67, 34), (65, 42), (58, 41)])
    c.update(compose([(shaded(shovel, BARK, light="'"), None), (shaded(blade, IRON, rim='S'), None)]))
    return finish({p: s for p, s in c.items() if s != 'o'}, size)


def wheelbarrow(i):
    """A wooden barrow on one iron wheel, stones in its tray."""
    size = (64, 40)
    tray, frame, wheel = Part(size), Part(size), Part(size)
    tray.polygon([(12, 12), (52, 10), (47, 26), (18, 27)])
    frame.limb([(2, 17), (18, 26), (46, 28)], 2)
    frame.limb([(24, 27), (21, 38)], 2)
    wheel.ellipse((48, 31), 6, 6)
    c = compose([
        (shaded(frame, BARK, light="'"), None),
        (shaded(tray, BARK, light="'", rim='"'), 'o'),
        (shade(wheel.pixels(), 'Φ', light='ψ', shadow='b', rim='Ω'), 'o'),
    ])
    c[(48, 31)] = 'χ'
    for k in range(5):
        ellipse(c, 20 + k * 6, 11 - (k % 2), 3, 2, lambda dx, dy: 'B' if dy < 0 else 'Φ')
    return finish(c, size)


def tool_rack(i):
    """A rack of two posts and a bar: two shovels and a pick hung on it, a
    coil of rope."""
    size = (64, 64)
    posts, bar, handles, irons, rope = (Part(size) for _ in range(5))
    posts.limb([(8, 61), (8, 16)], 3)
    posts.limb([(56, 61), (56, 16)], 3)
    bar.limb([(5, 18), (59, 18)], 3)
    handles.limb([(19, 18), (17, 48)], 2)
    handles.limb([(32, 18), (34, 48)], 2)
    handles.limb([(46, 18), (46, 44)], 2)
    irons.polygon([(13, 48), (22, 48), (22, 56), (17, 60), (13, 56)])
    irons.polygon([(30, 48), (39, 48), (39, 56), (34, 60), (30, 56)])
    irons.polygon([(37, 42), (55, 42), (58, 45), (46, 44), (35, 45)])
    rope.ellipse((27, 31), 5, 5)
    c = compose([
        (shaded(posts, BARK, light="'"), None),
        (shade({p for p in rope.pixels() if math.hypot(p[0] - 27, p[1] - 31) > 2.5}, '"', light="'", shadow='ω'), None),
        (shaded(handles, BARK, light="'"), None),
        (shaded(irons, IRON, rim='S'), 'o'),
        (shaded(bar, BARK, light="'"), 'o'),
    ])
    return finish(c, size)


def timber_stack(i):
    """Squared beams stacked three high, lashed with rope."""
    size = (96, 40)
    layers = []
    for x0, x1, y0, y1 in ((4, 92, 29, 38), (9, 86, 20, 28), (16, 79, 11, 19)):
        beam = Part(size)
        beam.rectangle((x0, y0, x1, y1))
        colors = shaded(beam, BARK, light="'", rim='"')
        for (x, y) in list(colors):
            if colors[(x, y)] == BARK['base'] and unit('beam-grain', i, x // 7, y) < 0.15:
                colors[(x, y)] = '/'
        layers.append((colors, 'o'))
    c = compose(layers)
    for x in (24, 70):
        for y in range(11, 39):
            if (x, y) in c:
                c[(x, y)] = '"' if y % 3 else "'"
    return finish(c, size)


def rail_bumper(i):
    """A buffer stop at the rails' end: raked struts and a beam faced in
    iron."""
    size = (48, 40)
    struts, beam, face = Part(size), Part(size), Part(size)
    struts.limb([(6, 39), (22, 14)], 4)
    struts.limb([(40, 39), (30, 14)], 3)
    beam.rectangle((14, 10, 40, 18))
    face.rectangle((12, 11, 16, 17))
    c = compose([(shaded(struts, BARK, light="'"), None), (shaded(beam, BARK, light="'", rim='"'), 'o'),
                 (shade(face.pixels(), 'Φ', light='ψ', shadow='b'), None)])
    c[(20, 13)], c[(34, 13)] = 'χ', 'χ'
    return finish(c, size)


def pebbles(i, width=64):
    """Loose stones scattered on a tread."""
    c = {}
    for k in range(7):
        x, r = 3 + int(unit('pebble', i, k) * (width - 6)), 1 + int(unit('pebble-r', i, k) * 3)
        ellipse(c, x, 9 - r, r + 1, r, lambda dx, dy: 'Ω' if dx + dy < -1 else 'B' if dy < 1 else 'b')
    return finish(c, (width, 12))


def hanging_lantern(i):
    """A miner's lantern hung on a short chain, its glass in the glowing
    yellows."""
    size = (16, 40)
    c = {}
    for y in range(0, 14):
        c[(8, y)] = 'Ω' if y % 4 < 2 else 'Φ'
    rect(c, 4, 14, 13, 16, lambda x, y: 'ψ' if y == 14 else 'Ω')
    rect(c, 4, 16, 13, 30, lambda x, y: 'Φ' if x in (4, 12) else 'Y' if (x in (7, 8) and 19 < y < 26) else 'y' if 5 < x < 11 else 'J')
    rect(c, 3, 30, 14, 33, lambda x, y: 'Ω' if y == 30 else 'Φ')
    return finish(c, size)


def rope_bucket(length):
    """A rope paid out over an edge, a bucket at its end."""
    size = (24, length)
    rope, bucket, bail = Part(size), Part(size), Part(size)
    rope.limb([(12, 0), (12, length - 18)], 1)
    bucket.polygon([(4, length - 14), (20, length - 14), (18, length - 1), (6, length - 1)])
    bail.limb([(5, length - 14), (12, length - 20), (19, length - 14)], 1)
    c = compose([(shade(rope.pixels(), '"', light="'", shadow='ω'), None), (shade(bail.pixels(), 'Φ', light='ψ'), None),
                 (shaded(bucket, BARK, light="'", rim='"'), 'o')])
    for x in range(5, 20):
        if (x, length - 9) in c:
            c[(x, length - 9)] = 'Ω'
    return finish(c, size)


def fire_crystal(i, hanging=False):
    """A cluster of fire crystals grown from a lump of basalt (e01hollow),
    glowing from within: each a long prism, white-hot down its core, its
    faces orange, its edges dark red, a glint at its tip; the rock about
    their feet lit by them."""
    layers = []
    rock = Part()
    rock.ellipse((32, 59), 15 + i % 2 * 3, 5)
    rock.ellipse((24 - i, 60), 7, 4)
    base = grade(rock.pixels(), 'ενγβα', light=(-0.5, -0.9, 0.6), spread=0.3)
    layers.append((base, None))
    for k, (dx, h, w, lean) in enumerate(((0, 32, 5.5, 0.08), (-9, 22, 4.5, -0.38), (9, 25, 4.5, 0.42),
                                          (-16, 13, 3.5, -0.7), (16, 15, 3.5, 0.75))):
        if k >= 3 + i % 3:
            break
        h += (i * 7 + k * 3) % 5 - 2
        cx = 32 + dx
        tipx, tipy = cx + math.sin(lean) * h, 58 - math.cos(lean) * h
        side = (math.cos(lean), math.sin(lean))
        crystal = Part()
        crystal.polygon([(cx - w, 59), (cx + w, 59), (tipx + side[0] * w * 0.8, tipy + h * 0.22),
                         (tipx, tipy), (tipx - side[0] * w * 0.8, tipy + h * 0.22)])
        pts = crystal.pixels()
        colors = {}
        for (x, y) in pts:
            along = max(0.0, min(1.0, (59 - y) / max(1.0, 59 - tipy)))
            axis = cx + (tipx - cx) * along
            off = (x + 0.5 - axis) / max(1.0, w * (1 - along * 0.75))
            if abs(off) < 0.22:
                colors[(x, y)] = 'y' if along > 0.55 else 'Ъ'
            elif off < -0.22:
                colors[(x, y)] = 'Ъ' if off > -0.7 else 'J'
            else:
                colors[(x, y)] = 'K' if off < 0.75 else 'L'
        for (x, y) in pts:
            if (x + 1, y) not in pts or (x - 1, y) not in pts:
                colors[(x, y)] = 'L' if (x + 1, y) not in pts else 'J'
        tip = min(pts, key=lambda p: p[1])
        colors[tip] = 'Y'
        layers.append((colors, 'M'))
    c = compose(layers)
    # The crystals' glow on the rock about their feet.
    for (x, y) in list(c):
        if c[(x, y)] in 'ενγβα' and y < 62 and any(c.get((x + dx, y - 1)) in ('Ъ', 'K', 'L', 'J', 'y', 'Y')
                                                   for dx in (-1, 0, 1)):
            c[(x, y)] = 'Ъ' if dither(x, y, 0.5) else 'K'
    im = finish(c)
    return im.transpose(Image.FLIP_TOP_BOTTOM) if hanging else im


def toll_boom(i):
    """The troll's toll: a forked post, a log boom across the way at hip
    height, a net of stones for its counterweight, a skull hung from its
    tip."""
    size = (2 * TILE, 2 * TILE)
    post, boom, net, rope, skull, jaw = (Part(size) for _ in range(6))
    post.limb([(24, 127), (24, 64)], 7)
    post.limb([(24, 66), (19, 56)], 3)
    post.limb([(24, 66), (30, 56)], 3)
    boom.limb([(6, 66), (122, 76)], 6)
    net.ellipse((12, 84), 8, 9)
    rope.limb([(10, 68), (12, 76)], 1)
    rope.limb([(118, 79), (118, 92)], 1)
    skull.ellipse((118, 98), 6, 6)
    jaw.ellipse((118, 104), 4, 2)
    c = compose([
        (shaded(post, BARK, light="'", rim='"'), None),
        (shade(rope.pixels(), '"', light="'", shadow='ω'), None),
        (shade(net.pixels(), '~', light=':', shadow='-'), 'o'),
        (shaded(boom, BARK, light="'", rim='"'), 'o'),
        (shade(jaw.pixels(), **BONE), 'o'),
        (shade(skull.pixels(), **BONE), 'o'),
    ])
    for (x, y), s in list(c.items()):
        if s == BARK['base'] and (x + 2 * y) % 11 == 0:
            c[(x, y)] = '/'
    for x in range(6, 19, 3):
        for y in range(77, 92, 3):
            if (x, y) in c:
                c[(x, y)] = '"'
    for x, y in ((116, 97), (117, 97), (120, 97), (121, 97), (118, 100)):
        c[(x, y)] = '0'
    for x in range(40, 120, 14):
        c[(x, 70 + (x - 6) * 10 // 116)] = 'M'
        c[(x + 1, 70 + (x - 6) * 10 // 116)] = 'X'
    return finish(c, size)


def lean_to(i):
    """The troll's shelter: poles leaned on a ridge pole, hides lashed over
    them, a bone rack and a pot."""
    size = (2 * TILE, 2 * TILE)
    poles, hides, bonesp, pot = (Part(size) for _ in range(4))
    poles.limb([(10, 127), (70, 40)], 4)
    poles.limb([(126, 127), (66, 40)], 4)
    poles.limb([(60, 42), (100, 46)], 3)
    hides.polygon([(18, 124), (66, 50), (98, 54), (118, 124)])
    for k in range(4):
        bonesp.limb([(26 + k * 9, 127), (28 + k * 9, 100)], 2)
    pot.ellipse((96, 118), 11, 8)
    c = compose([
        (shaded(poles, BARK, light="'", rim='"'), None),
        (shade(hides.pixels(), '(', light='a', shadow='/', back='>'), 'o'),
        (shade(bonesp.pixels(), **BONE), 'o'),
        (shaded(pot, IRON, rim='S'), 'o'),
    ])
    for (x, y), s in list(c.items()):
        if s == '(' and (x * 3 + y) % 13 == 0:
            c[(x, y)] = '>'
        if s in ('(', 'a') and y % 18 == 0 and (x // 3) % 2:
            c[(x, y)] = '"'
    for x in range(50, 90, 6):
        c[(x, 70 + (x - 50) // 4)] = 'w'
    return finish(c, size)


def toll_sign(i):
    """A crude board on a stake: a painted skull and three coin marks."""
    size = (TILE, 2 * TILE)
    stake, board = Part(size), Part(size)
    stake.limb([(32, 127), (32, 50)], 4)
    board.polygon([(8, 44), (56, 40), (58, 74), (10, 78)])
    c = compose([(shaded(stake, BARK, light="'"), None), (shaded(board, BARK, light="'", rim='"'), 'o')])
    ellipse(c, 25, 56, 7, 6, lambda dx, dy: 'w' if dy < 3 else None)
    for x, y in ((22, 55), (23, 55), (27, 55), (28, 55), (25, 58)):
        c[(x, y)] = '0'
    for x in range(21, 30, 2):
        c[(x, 62)] = 'w'
    for k, (x, y) in enumerate(((42, 52), (47, 59), (42, 66))):
        ellipse(c, x, y, 3, 3, lambda dx, dy: 'A' if dx + dy < 0 else 'a')
    return finish(c, size)


def spoil_heap(i):
    """Spoil from the cut: a mound of broken rock, ore lumps rusting in it."""
    size = (2 * TILE, TILE)
    mound = Part(size)
    mound.polygon([(2, 63), (30, 34), (52, 26), (70, 30), (96, 42), (126, 63)])
    c = shaded(mound, STONE)
    for k in range(60):
        x, y = 8 + int(unit('spoil', i, k) * 112), 30 + int(unit('spoily', i, k) * 32)
        if (x, y) in c:
            r = 1 + int(unit('spr', i, k) * 3)
            ellipse(c, x, y, r, r, lambda dx, dy: ':' if dx + dy < 0 else '-' if unit('sp', dx, dy) < 0.5 else '~')
            if unit('ore', i, k) < 0.2:
                c[(x, y)] = 'a'
    return finish(c, size)


def trestle(height):
    """A timber trestle under a scaffold's deck: two posts, a cap, a sill and
    X-braces."""
    size = (TILE, height)
    posts, braces = Part(size), Part(size)
    posts.rectangle((8, 0, 13, height - 1))
    posts.rectangle((50, 0, 55, height - 1))
    posts.rectangle((4, 0, 59, 5))
    posts.rectangle((4, height - 6, 59, height - 1))
    braces.limb([(12, 6), (52, height - 7)], 3)
    braces.limb([(52, 6), (12, height - 7)], 3)
    c = compose([(shaded(braces, BARK, light='"'), None), (shaded(posts, BARK, light="'", rim='"'), 'o')])
    for x, y in ((10, 3), (52, 3), (10, height - 4), (52, height - 4)):
        c[(x, y)] = 'Ω'
    return finish(c, size)


def rails():
    c = {}
    for x in range(TILE):
        c[(x, 0)], c[(x, 1)], c[(x, 2)], c[(x, 3)] = 'χ', 'ψ', 'Ω', 'B'
        if x % 16 < 11:
            for y in range(4, 10):
                c[(x, y)] = '"' if y == 4 else '/' if y == 9 or x % 16 in (0, 10) else 'ω'
    return to_image(c, (TILE, 10))


def cart():
    c = {}
    for y in range(10, 50):
        for x in range(8, 104):
            plank = (y - 10) // 8
            v = (y - 10) % 8
            sym = '"' if v == 0 else '/' if v == 7 else 'ω'
            if unit('cart', plank, x // 9) < 0.12 and v in (2, 3, 4):
                sym = '/'
            c[(x, y)] = sym
        for x in (8, 9, 102, 103, 54, 55):
            c[(x, y)] = 'Ω' if x % 2 == 0 else 'Φ'
    for x in range(6, 106):
        c[(x, 10)], c[(x, 11)], c[(x, 49)] = 'χ', 'Ω', 'Φ'
    for x, y in ((12, 14), (12, 44), (58, 14), (58, 44), (98, 14), (98, 44), (50, 14), (50, 44)):
        c[(x, y)], c[(x + 1, y)] = 'φ', 'Φ'
    for k in range(26):
        cx = 14 + int(unit('ore', k) * 84)
        cy = 9 - int(unit('orey', k) * 7)
        r = 3 + int(unit('orer', k) * 4)
        for dy in range(-r, r + 1):
            for dx in range(-r, r + 1):
                if dx * dx + dy * dy <= r * r and cy + dy <= 10:
                    c[(cx + dx, cy + dy)] = 'χ' if dx + dy < -r // 2 else 'Ω' if dx + dy < r // 2 else 'Φ'
        if unit('rusty', k) < 0.4:
            c[(cx, cy)] = 'a'
    for wx in (28, 84):
        for dy in range(-13, 14):
            for dx in range(-13, 14):
                d = math.hypot(dx, dy)
                if d <= 13:
                    sym = 'Φ' if d > 11 else 'Ω' if d > 9 else 'ψ' if abs(dx) < 2 or abs(dy) < 2 else 'b'
                    c[(wx + dx, 60 + dy)] = 'χ' if d < 3 else sym
    return finish(c, (112, 76))


def broken_cart():
    """The cart at the bridge's head: tipped on its axle, a wheel off and
    lying beside it, its ore spilled."""
    im = cart()
    c = from_image(im)
    for (x, y) in list(c):
        if math.hypot(x - 84, y - 60) <= 15:
            del c[(x, y)]
    tipped = to_image(c, im.size).rotate(-8, resample=Image.NEAREST, expand=True, center=(28, 60))
    out = Image.new('RGBA', (tipped.width + 40, tipped.height + 4))
    out.alpha_composite(tipped, (0, 0))
    wheel = {}
    ellipse(wheel, 14, 5, 13, 4, lambda dx, dy: 'Φ' if abs(dx) > 10 or abs(dy) > 2 else 'ψ' if abs(dx) < 2 else 'b')
    out.alpha_composite(finish(wheel, (30, 12)), (tipped.width - 14, out.height - 12))
    return out


def bones(i):
    c = {}
    def oval(cx, cy, rx, ry, sym, shade_='τ'):
        ellipse(c, cx, cy, rx, ry, lambda dx, dy: shade_ if dy > ry // 2 or dx > rx // 2 else sym)
    if i == 0:
        oval(22, 20, 9, 8, 'w')
        oval(22, 28, 6, 3, 'σ')
        for x, y in ((18, 19), (19, 19), (25, 19), (26, 19), (18, 20), (26, 20)):
            c[(x, y)] = '0'
        for x in range(18, 27, 2):
            c[(x, 29)] = '0'
        c[(22, 23)] = '0'
        for x in range(34, 58):
            c[(x, 28)], c[(x, 29)] = 'σ', 'τ'
        oval(34, 28, 2, 2, 'w')
        oval(57, 28, 2, 2, 'w')
    elif i == 1:
        for k in range(5):
            cx = 10 + k * 9
            for a in range(20):
                t = math.pi * a / 19
                c[(round(cx + 6 * math.cos(t)), round(28 - 12 * math.sin(t)))] = 'σ' if a < 10 else 'τ'
        for x in range(6, 56):
            c[(x, 29)], c[(x, 30)] = 'τ', 'S'
    else:
        oval(16, 26, 7, 5, 'σ')
        oval(44, 24, 10, 6, 'w')
        for x, y in ((40, 23), (41, 23), (48, 23), (47, 24)):
            c[(x, y)] = '0'
        for x in range(4, 60):
            if unit('chip', x) < 0.2:
                c[(x, 31)] = 'τ'
    return finish(c, (TILE, 32))


def fence(i):
    posts, rails_, broken = Part(), Part(), Part()
    for px in (8, 50):
        posts.polygon([(px, 61), (px, 30), (px + 3, 26), (px + 6, 30), (px + 6, 61)])
    for ry in (36, 47):
        end = 62 if (i % 2 == 0 or ry == 36) else 40
        rails_.polygon([(2, ry), (end / 2, ry + 1.5), (end, ry), (end, ry + 4), (end / 2, ry + 5.5), (2, ry + 4)])
    if i % 2:
        broken.polygon([(40, 47), (44, 47), (52, 59), (48, 60)])
    c = compose([
        (shaded(rails_, BARK, light='"', rim='"'), None),
        (shaded(broken, BARK, light='"'), 'o'),
        (shaded(posts, BARK, light="'", rim='"'), 'o'),
    ])
    for (x, y), s in list(c.items()):
        if s == BARK['base'] and unit('grain', i, x // 6, y // 2) < 0.12:
            c[(x, y)] = '/'
    for x, y in ((11, 38), (11, 49), (53, 38), (53, 49)):
        c[(x, y)] = 'B'
    for px in (8, 50):
        for x in range(px - 3, px + 10):
            if unit('tuft', i, x) < 0.6:
                c[(x, 61)], c[(x, 60)] = 'T', 'g' if unit('t2', i, x) < 0.5 else 'G'
    return finish(c)


def signpost(i):
    post, board = Part(), Part()
    post.rectangle((29, 20, 34, 61))
    board.polygon([(8, 11), (49, 11), (57, 18), (49, 25), (8, 25)])
    c = compose([(shaded(post, BARK, light="'", rim='"'), None), (shaded(board, BARK, light="'", rim='"'), 'o')])
    for x in range(9, 49):
        if unit('bgrain', i, x // 4) < 0.5:
            c[(x, 18)] = '/'
    line(c, 15, 18, 40, 18, '>')
    line(c, 36, 15, 40, 18, '>')
    line(c, 36, 21, 40, 18, '>')
    c[(31, 13)], c[(32, 23)] = 'B', 'B'
    for x in range(8, 32):
        if unit('smoss', i, x) < 0.5:
            c[(x, 10)] = 'T'
            if unit('sm2', i, x) < 0.4:
                c[(x, 9)] = 'g'
    return finish(c)


def stump(i):
    body, top = Part(), Part()
    body.polygon([(10, 61), (15, 54), (16, 43), (48, 43), (49, 54), (54, 61)])
    top.ellipse((32, 43), 16, 5)
    c = compose([(shaded(body, BARK, light='"', back='>'), None)])
    for (x, y) in top.pixels():
        r = math.hypot((x - 32) / 16, (y - 43) / 5)
        c[(x, y)] = "'" if round(r * 7) % 2 else '"'
    c[(32, 43)] = 'ω'
    for (x, y), s in list(c.items()):
        if s == BARK['base'] and (x * 3 + y) % 9 == 0:
            c[(x, y)] = '/'
    for mx, my in ((20, 41), (45, 42)):
        cap, stem = Part(), Part()
        cap.ellipse((mx, my - 4), 4, 3)
        stem.rectangle((mx - 1, my - 2, mx + 1, my + 1))
        c.update(shade(stem.pixels(), 'σ', light='w', shadow='τ'))
        caps = shade(cap.pixels(), 'z', light='F', shadow='X')
        for (x, y) in list(caps):
            if (x + y) % 3 == 0 and caps[(x, y)] == 'z':
                caps[(x, y)] = 'w'
        c.update(caps)
    return finish(c)


def log_pile(i):
    layers = []
    for cx, cy in ((19, 50), (43, 50), (31, 34)):
        log = Part()
        log.ellipse((cx, cy), 10, 10)
        colors = {}
        for (x, y) in log.pixels():
            r = math.hypot(x - cx, y - cy)
            colors[(x, y)] = '/' if r > 8.5 else "'" if round(r) % 3 == 0 else '"'
            if r > 8.5 and y < cy and x > cx:
                colors[(x, y)] = 'ω'
        colors[(cx, cy)] = 'ω'
        layers.append((colors, 'o'))
    return finish(compose(layers))


def pillar_stump(i):
    column, plinth = Part(), Part()
    top = [(18 + k * 4, 18 + round(6 * unit('ptop', i, k) + 3 * math.sin(k + i))) for k in range(8)]
    column.polygon([(18, 56)] + top + [(46, top[-1][1]), (46, 56)])
    plinth.rectangle((12, 55, 52, 61))
    c = compose([(blocks(column.pixels(), course=9, seed=('pillar', i)), None),
                 (blocks(plinth.pixels(), course=7, seed=('plinth', i)), 'o')])
    for x, y in top:
        for dx in range(4):
            if unit('pmoss', i, x + dx) < 0.55:
                c[(x + dx, y)] = 'T'
                c[(x + dx, y - 1)] = 'g' if unit('pm', i, x + dx) < 0.5 else 'G'
    for k in range(3):
        rock = Part()
        rock.ellipse((6 + int(unit('chunk', i, k) * 52), 59), 3, 2)
        c.update(shaded(rock, STONE))
    return finish(c)


def reeds(i):
    stems, heads, blades = Part(), Part(), Part()
    for k in range(10):
        x = 6 + int(unit('reed', i, k) * 52)
        top = 18 + int(unit('reedh', i, k) * 18)
        lean = (unit('lean', i, k) - 0.5) * 8
        stems.limb([(x, 61), (x + lean * 0.5, 40), (x + lean, top)], 2)
        if unit('cat', i, k) < 0.6:
            heads.limb([(x + lean, top - 7), (x + lean, top)], 3)
        if unit('blade', i, k) < 0.5:
            d = 1 if k % 2 else -1
            blades.limb([(x, 61), (x + 5 * d, 50), (x + 9 * d, 44)], 2)
    c = compose([
        (shade(blades.pixels(), 'U', light='W', shadow='P'), None),
        (shade(stems.pixels(), 'T', light='g', shadow='t', rim='G'), None),
        (shade(heads.pixels(), 'ω', light='"', shadow='/', rim="'"), None),
    ])
    return finish(c)


# The statue's pale stone, dark to light: its shading ramp (e01hollow).
MARBLE = ('s', 'S', 'τ', 'σ', 'w')


def statue(i):
    """The aqueduct's statue (e01hollow, fully redrawn), 2 x 3 tiles: a
    water-bearer in pale weathered stone on a moulded pedestal. She stands
    in contrapposto, her weight on her right leg, her left knee pressing
    through the drapery; a chiton girdled under the bust falls in an
    overfold, a himation wraps her hips and flares to a scalloped hem, one
    sandalled foot showing. Her left arm is raised to steady an urn on her
    shoulder, its mouth tipped; her right arm is broken off below the
    elbow, its forearm lying at the pedestal's foot. Her head, bun behind,
    is bowed away from the urn, the nose worn off. The fountain she was
    left lime streaks from the urn down her shoulder and folds and onto
    the pedestal: a cornice, a die with a carved tablet, a torus base and a
    coursed plinth; moss on every upper surface, lichen, a crack, ivy."""
    w, h = 2 * TILE, 3 * TILE
    size = (w, h)

    def mask(draw):
        part = Part(size)
        draw(part)
        return part.pixels()

    def lit(pts, **kw):
        return grade(pts, MARBLE, light=(0.5, -0.8, 0.55), spread=0.35, **kw)

    # The pedestal, in the arcade's grey stone.
    plinth = mask(lambda q: q.rectangle((30, 177, 96, 191)))
    torus = mask(lambda q: q.rectangle((33, 171, 93, 176)))
    die = mask(lambda q: q.rectangle((38, 137, 88, 170)))
    cornice = mask(lambda q: q.polygon([(34, 128), (92, 128), (92, 131), (90, 134), (88, 136), (38, 136),
                                        (36, 134), (34, 131)]))
    ped = compose([
        (blocks(plinth, course=7, seed='statue-plinth', offset=177), None),
        ({p: ';' if p[1] == 171 else ':' if p[1] == 172 else '~' if p[1] < 175 else '-' for p in torus}, 'o'),
        ({p: (';' if p[0] > 84 else ':') if p[1] < 139 else '-' if p[0] < 41 else
          '~' if unit('die', p[0] // 6, p[1] // 5) < 0.7 else '=' for p in die}, 'o'),
        ({p: ';' if p[1] < 130 else ':' if p[1] < 132 else '~' if p[1] < 134 else '-' for p in cornice}, 'o'),
    ])
    # The tablet, sunk in the die, its lines of worn letters.
    for y in range(143, 163):
        for x in range(45, 82):
            edge = y in (143, 162) or x in (45, 81)
            ped[(x, y)] = '<' if edge and (y == 143 or x == 45) else ':' if edge else '=' if dither(x, y, 0.4) else '~'
    for row, (x0, x1) in enumerate(((49, 77), (51, 75), (54, 72))):
        for x in range(x0, x1):
            if unit('glyph', row, x // 3) < 0.72 and x % 3 != 2:
                ped[(x, 147 + row * 5)] = '<'
                ped[(x, 148 + row * 5)] = '-' if unit('glyph2', row, x) < 0.5 else '<'
    # The figure, on a skeleton about 7.5 heads tall (a head 15 px): the
    # pit of her neck over the weight foot, the hips tilted up on the
    # weight side and the shoulders the other way, elbows at the waist,
    # wrists at the hip; limbs taper from shoulder to wrist.
    def tapered(a, b, wa, wb):
        """A limb from joint `a` to `b`, `wa` px wide at `a`, `wb` at `b`."""
        dx, dy = b[0] - a[0], b[1] - a[1]
        n = math.hypot(dx, dy) or 1
        px, py = -dy / n, dx / n
        return mask(lambda q: (q.polygon([(a[0] + px * wa / 2, a[1] + py * wa / 2),
                                          (b[0] + px * wb / 2, b[1] + py * wb / 2),
                                          (b[0] - px * wb / 2, b[1] - py * wb / 2),
                                          (a[0] - px * wa / 2, a[1] - py * wa / 2)]),
                               q.ellipse(b, wb / 2, wb / 2)))

    SHOULDER_R, SHOULDER_L = (48, 41), (74, 39)
    ELBOW_R, ELBOW_L = (43, 60), (81, 57)
    WRIST_L = (77, 71)
    KNEE_FREE = (73, 98)
    # Her head, bowed toward the urn at her left hip: the skull, the jaw
    # and chin turned to the right, the nose on the silhouette.
    skull = mask(lambda q: q.ellipse((60, 21), 7, 8))
    face = mask(lambda q: q.polygon([(62, 20), (67, 19), (69, 22), (70, 24), (68, 25), (68, 27), (66, 29),
                                     (62, 30), (59, 28)]))
    head = skull | face
    hair = {(x, y) for (x, y) in skull if y < 17 + max(0, 64 - x) * 0.85 and y < 26} | mask(
        lambda q: q.ellipse((53, 16), 4, 4))
    # The neck: as wide as most of the head, its back running down from the
    # skull's base into the shoulders' slope, its front from under the jaw
    # to the pit of the neck, leaning forward as she bows.
    neck = mask(lambda q: q.polygon([(55, 24), (61, 27), (64, 30), (64, 33), (68, 37), (51, 39), (54, 34),
                                     (54, 28)]))
    torso = mask(lambda q: q.polygon([(47, 45), (49, 41), (54, 38), (60, 37), (66, 36), (71, 37), (75, 40),
                                      (77, 44), (75, 49), (72, 57), (73, 66), (49, 66), (50, 57), (48, 50)]))
    delt_r = mask(lambda q: q.ellipse((SHOULDER_R[0], SHOULDER_R[1] + 1), 3.5, 4))
    delt_l = mask(lambda q: q.ellipse((SHOULDER_L[0], SHOULDER_L[1] + 1), 3.5, 4))
    arm_r = tapered(SHOULDER_R, ELBOW_R, 7, 5) | tapered(ELBOW_R, (44, 65), 5, 4.5)
    arm_l = tapered(SHOULDER_L, ELBOW_L, 7, 5) | tapered(ELBOW_L, WRIST_L, 5, 4)
    hand = mask(lambda q: q.polygon([(74, 71), (79, 70), (82, 73), (80, 76), (75, 75)]))
    # The mantle wrapped from her hips to her ankles, falling straight down
    # the weight leg and spreading round the bent knee; the roll of it
    # slanting across her hips.
    skirt = mask(lambda q: q.polygon([(48, 64), (74, 64), (77, 72), (79, 84), (81, 98), (83, 110), (85, 121),
                                      (82, 124), (47, 124), (46, 112), (48, 98), (47, 84), (46, 72)]))
    knee = mask(lambda q: q.ellipse(KNEE_FREE, 6, 9))
    roll = mask(lambda q: q.polygon([(46, 63), (51, 61), (79, 70), (78, 75), (49, 68), (45, 67)]))
    foot_w = mask(lambda q: q.polygon([(52, 122), (61, 122), (62, 127), (50, 127)]))
    foot_f = mask(lambda q: q.polygon([(74, 123), (80, 123), (82, 127), (73, 127)]))
    # The urn held against her left hip, tipped down to the right as if
    # pouring, her forearm over it and her hand under its shoulder.
    tilt = math.radians(40)
    u, v = (math.sin(tilt), math.cos(tilt)), (math.cos(tilt), -math.sin(tilt))
    ucx, ucy = 86, 74

    def along(a, b):
        return ucx + u[0] * a + v[0] * b, ucy + u[1] * a + v[1] * b

    urn = mask(lambda q: (q.polygon([along(10 * math.cos(t / 12 * math.tau), 7 * math.sin(t / 12 * math.tau))
                                     for t in range(12)]),
                          q.polygon([along(7, -2.5), along(7, 2.5), along(14, 2), along(14, -2)]),
                          q.polygon([along(14, -3.5), along(14, 3.5), along(16, 3.5), along(16, -3.5)])))
    handle = mask(lambda q: (q.limb([along(4, 7), along(8, 9), along(11, 2.5)], 2),
                             q.limb([along(4, -7), along(8, -9), along(11, -2.5)], 2)))
    figure = compose([
        (lit(foot_w), 'o'),
        (lit(foot_f), 'o'),
        (lit(skirt, fall=0.3), 'o'),
        (lit(knee, bias=0.14), None),
        (lit(arm_r, depth=3), 'o'),
        (lit(delt_r, bias=0.05), None),
        (lit(torso, fall=0.12), 'o'),
        (lit(roll, depth=2, bias=0.05), 'o'),
        (lit(neck, depth=3, bias=-0.05), 'o'),
        (lit(head, depth=3, bias=0.08), 'o'),
        (grade(hair, ('s', 'S', 'S', 'τ', 'σ'), light=(0.5, -0.8, 0.55), depth=3, spread=0.35), None),
        (lit(handle, depth=1), None),
        (lit(urn, bias=-0.05), 'o'),
        (lit(arm_l, depth=3), 'o'),
        (lit(delt_l, bias=0.08), None),
        (lit(hand, depth=2, bias=0.05), 'o'),
    ])

    def stroke(points, sym, ridge=None, skip=()):
        """A fold's trough along `points` in `sym`, its lit ridge beside it
        to the right in `ridge`; never over an outline or `skip`."""
        for (x0, y0), (x1, y1) in zip(points, points[1:]):
            n = max(round(abs(x1 - x0)), round(abs(y1 - y0)), 1)
            for t in range(n + 1):
                q = (round(x0 + (x1 - x0) * t / n), round(y0 + (y1 - y0) * t / n))
                if figure.get(q) not in (None, 'o') and q not in skip:
                    figure[q] = sym
                    r = (q[0] + 1, q[1])
                    if ridge and figure.get(r) not in (None, 'o') and r not in skip:
                        figure[r] = ridge

    front = urn | arm_l | hand | handle
    # The chiton's folds from its shoulder pins to the cord under the bust,
    # the cord, and the fall of it to the mantle's roll.
    for pts in (((53, 38), (56, 46), (57, 51)), ((55, 37), (60, 45), (61, 51)), ((69, 37), (66, 45), (65, 51)),
                ((71, 38), (70, 46), (69, 51)), ((58, 52), (57, 62)), ((63, 52), (64, 62)), ((68, 52), (69, 62))):
        stroke(pts, 'S', 'σ', front)
    for x in range(50, 74):
        q = (x, 51 + round(0.6 * math.sin(x * 0.4)))
        if figure.get(q) not in (None, 'o') and q not in front and dither(x, q[1], 0.7):
            figure[q] = 'S'
            if figure.get((x, q[1] + 1)) not in (None, 'o') and (x, q[1] + 1) not in front:
                figure[(x, q[1] + 1)] = 'σ'
    for (cx0, cy0) in ((55, 47), (67, 46)):
        for (x, y) in list(figure):
            if ((x - cx0) / 4.5) ** 2 + ((y - cy0) / 3.5) ** 2 <= 1 and figure[(x, y)] in MARBLE and (x, y) not in front:
                k = MARBLE.index(figure[(x, y)])
                figure[(x, y)] = MARBLE[min(4, k + 1)] if y < cy0 else MARBLE[max(0, k - 1)] if y > cy0 + 1 else figure[(x, y)]
    # The mantle's folds: straight down the weight leg, fanning from the
    # roll, curving round the bent knee and swinging between the legs; its
    # hem scalloped over the feet.
    for pts in (((50, 70), (51, 84), (49, 100), (50, 121)), ((55, 70), (54, 88), (56, 106), (55, 121)),
                ((74, 74), (66, 88), (60, 104), (58, 121)), ((62, 71), (64, 84)), ((69, 74), (76, 86)),
                ((80, 106), (81, 121)), ((75, 109), (76, 121)), ((68, 110), (69, 121))):
        stroke(pts, 'S', 'w', front)
    for k in range(3):
        stroke([(59, 99 + k * 5), (64, 102 + k * 5), (69, 104 + k * 4)], 's', None, front)
    for x in range(46, 86):
        q = (x, 121 + round(1.2 * math.sin(x * 0.7)))
        if figure.get(q) not in (None, 'o'):
            figure[q] = 'S'
    # The roll's twist, its folds slanting across it.
    for k in range(5):
        x0 = 50 + k * 6
        stroke([(x0, 62 + k * 1.6), (x0 + 3, 68 + k * 1.4)], 'S', None, front)
    # Her face: the brow lit over the blank eye's shadowed slot, the nose,
    # the lips, the chin, the jaw's line under the ear; the hair's waves,
    # parted and drawn back to the bun, its band.
    for x, y, sym in ((65, 19, 'w'), (66, 19, 'w'), (67, 20, 'σ'), (65, 20, 's'), (66, 20, 's'), (69, 22, 'σ'),
                      (70, 23, 'τ'), (68, 26, 'S'), (67, 26, 'S'), (67, 27, 'σ'), (65, 29, 'S'), (64, 29, 'S'),
                      (61, 27, 'S'), (60, 26, 'S')):
        if figure.get((x, y)) not in (None, 'o'):
            figure[(x, y)] = sym
    # The ear, under the hair at the jaw's hinge.
    for q, sym in (((58, 22), 'σ'), ((57, 23), 'S'), ((57, 24), 'S'), ((58, 25), 'τ'), ((59, 24), 's')):
        figure[q] = sym
    for k in range(3):
        for (x, y) in hair:
            if y == 14 + k * 3 + round(abs(x - 60) * 0.25) and (x + k) % 3 and figure.get((x, y)) not in (None, 'o'):
                figure[(x, y)] = 'S' if k % 2 else 'τ'
    for x in range(55, 67):
        q = (x, 16 + round(max(0, 64 - x) * 0.85))
        if q in hair and figure.get(q) not in (None, 'o'):
            figure[q] = 'σ' if x % 2 else 'S'
    figure[(53, 16)], figure[(54, 15)] = 's', 'σ'
    # The neck's shadow under the jaw and the pit between the collarbones;
    # the trapezius lit on the shoulders' slope.
    for x in range(58, 64):
        if figure.get((x, 31)) not in (None, 'o'):
            figure[(x, 31)] = 'S'
    for q in ((61, 36), (62, 36), (60, 37), (63, 37)):
        figure[q] = 'S'
    for x in range(50, 56):
        figure[(x, 40 - (x - 50) * 2 // 5)] = 'w'
    for x in range(66, 73):
        figure[(x, 36 + (x - 66) // 2)] = 'w'
    # The right forearm broken off below the elbow, its break pale and
    # rough, its shadow on the mantle.
    for x in range(41, 48):
        y = 64 + (unit('stump', x) < 0.5)
        if (x, y) in arm_r:
            figure[(x, y)] = 'w' if dither(x, y, 0.6) else 'σ'
            figure[(x, y + 1)] = 'o'
    # The urn's ridges and dark mouth; her fingers along its shoulder.
    for a0 in (-4, 3):
        for b0 in range(-7, 8):
            q = tuple(round(c0) for c0 in along(a0, b0))
            if q in urn and q not in hand | arm_l and figure.get(q) not in (None, 'o'):
                figure[q] = 'S'
    for b0 in range(-2, 3):
        figure[tuple(round(c0) for c0 in along(16, b0))] = 's'
    for q in ((75, 74), (77, 75), (79, 75)):
        figure[q] = 'S'
    # The lime the fountain left: streaks down the urn from its mouth, and a
    # crust on the cornice and down the die where its water fell.
    mx, my = (round(c0) for c0 in along(16, 0))
    for k in range(3):
        for t in range(4, 13):
            q = tuple(round(c0) for c0 in along(16 - t, (k - 1) * 3))
            if q in urn and q not in hand | arm_l and figure.get(q) not in (None, 'o') and dither(*q, 0.6):
                figure[q] = 'S'
    for k, x0 in enumerate((mx - 2, mx, mx + 1)):
        for y in range(128, 176):
            q = (x0, y)
            if q in ped and dither(x0, y, 0.85 - (y - 128) / 60):
                ped[q] = 'σ' if y < 132 else 'τ' if k != 1 else 'S'
    c = compose([(ped, None), (figure, 'o')])
    # The fallen forearm at the foot: thick at its snapped elbow, tapering
    # to the wrist, the hand curled with its fingers in.
    arm = mask(lambda q: (q.polygon([(99, 184), (103, 183), (112, 186), (112, 189), (103, 191), (99, 191)]),
                          q.ellipse((115, 187), 3, 3), q.limb([(116, 190), (119, 190)], 2)))
    c = compose([(c, None), (lit(arm), 'o')])
    for y in range(184, 191):
        c[(99, y)] = 'w' if y % 2 else 'σ'
        c[(100, y)] = 'τ' if y % 3 else 'σ'
    for x in (114, 116, 118):
        c[(x, 189)] = 'S'
    # A crack across her skirt and down the die; moss on every upper
    # surface; lichen; ivy up the pedestal's shaded corner.
    x = 47
    for d in range(30):
        q = (x + d, 84 + round(math.sin(d * 0.6) * 1.5) + d // 10)
        if c.get(q) not in (None, 'o'):
            c[q] = 's'
    for d in range(22):
        q = (60 + round(math.sin(d * 0.5)), 137 + d)
        if c.get(q) not in (None, 'o'):
            c[q] = '<'
    for x in range(30, 97):
        col = sorted(y for (xx, y) in c if xx == x and c[(xx, y)] != 'o')
        for y in col:
            if y > 34 and ((x, y - 1) not in c or c[(x, y - 1)] == 'o'):
                if unit('smoss', x // 2, y // 3) < (0.55 if y > 120 else 0.35):
                    c[(x, y)] = 'T' if unit('smoss2', x, y) < 0.6 else 'g'
                    if unit('smoss3', x, y) < 0.4 and (x, y + 1) in c:
                        c[(x, y + 1)] = 't'
    for n in range(16):
        q = (40 + int(unit('slich', n) * 48), 20 + int(unit('slichy', n) * 150))
        if c.get(q) in ('σ', 'τ', 'w', '~', ':'):
            c[q] = 'm' if n % 3 else 'G'
    for d in range(44):
        y = 190 - d
        x = 39 + round(1.5 * math.sin(d * 0.3))
        if (x, y) in c:
            c[(x, y)] = 't'
            if d % 5 == 0:
                for k in range(3):
                    c[(x - 1 - k, y - k // 2)] = 'G' if k == 0 else 'g'
    return finish(c, size)


def broken_pillar():
    """The aqueduct's broken pillar (e01hollow), 2 x 4 tiles, standing at
    the area's start: a fluted column of four drums and the stump of a
    fifth, snapped on a slant, its break rough and mossed, on an Attic
    base (two tori round a scotia) over a coursed plinth; its fluting lit
    toward the moon, the drums' joints curving with the shaft, a crack
    down one drum, ivy climbing its shadowed side and the fallen drum
    lying on its end beside the plinth, flutes scalloping its rim."""
    w, h = 2 * TILE, 4 * TILE
    cx, foot = 62, h - 1
    ramp = ('<', '-', '=', '~', ':', ';')
    plinth, mould, shaft = Part((w, h)), {}, Part((w, h))
    plinth.polygon([(30, foot), (94, foot), (94, foot - 23), (91, foot - 26), (33, foot - 26), (30, foot - 23)])

    def radius(y):
        """The shaft's half width at row `y`, swelling a little low down."""
        return 21 + 1.6 * math.sin(math.pi * min(1.0, max(0.0, (y - 30) / 190))) - (219 - y) * 0.012

    def broken(x):
        """The break's top at column `x`: low on the left, high on the right,
        jagged."""
        u = (x - (cx - 22)) / 44
        return 70 - 46 * u ** 1.3 + (unit('pbreak', x // 3) - 0.5) * 7 + 5 * math.sin(u * 9)

    top = foot - 26
    # The Attic base: a broad torus, the scotia, a narrower torus.
    for y, half, sym in ((top - 1, 31, None), (top - 2, 31, None), (top - 3, 30, None), (top - 4, 30, None),
                         (top - 5, 29, None), (top - 6, 27, '<'), (top - 7, 27, '<'), (top - 8, 27, None),
                         (top - 9, 26, None), (top - 10, 26, None), (top - 11, 25, None)):
        for x in range(cx - half, cx + half + 1):
            u = (x - cx) / half
            if sym:
                mould[(x, y)] = sym
                continue
            band = (top - 1, top - 2, top - 3, top - 4, top - 5) if y >= top - 5 else (top - 8, top - 9, top - 10, top - 11)
            v = band.index(y) / (len(band) - 1)
            mould[(x, y)] = ramp_at(ramp, 0.3 + 0.35 * u + 0.45 * (v if y >= top - 5 else v) - 0.1, x, y)
    shaft_foot = top - 12
    for y in range(0, shaft_foot + 1):
        r = radius(y)
        for x in range(round(cx - r), round(cx + r) + 1):
            if y >= broken(x):
                shaft.draw.point((x, y), fill=255)
    pts = shaft.pixels()
    c = {}
    for (x, y) in pts:
        r = radius(y)
        u = (x - cx) / r
        # Nine flutes across the front, each a groove: its left wall lit,
        # its right in shade, a fillet between them.
        phase = (math.asin(max(-1.0, min(1.0, u))) / math.pi + 0.5) * 12
        f = phase % 1
        fillet = f < 0.14
        groove = -0.25 * math.cos(f * math.pi) if not fillet else 0.18
        value = 0.4 + 0.5 * u + groove - 0.3 * (1 - math.sqrt(max(0.0, 1 - u * u)))
        c[(x, y)] = ramp_at(ramp, value, x, y)
    # The drums' joints, curving with the shaft, and the break's rough top.
    for j in range(1, 6):
        y0 = shaft_foot - j * 38
        for x in range(round(cx - 23), round(cx + 24)):
            u = (x - cx) / radius(y0)
            if abs(u) < 1:
                q = (x, y0 + round(2 * math.sqrt(1 - u * u)))
                if q in c:
                    c[q] = '<'
                    if (x, q[1] + 1) in c:
                        c[(x, q[1] + 1)] = ramp_at(ramp, 0.75 + 0.3 * u, x, q[1] + 1)
    rough = {(x, y) for (x, y) in pts if y < broken(x) + 5}
    c.update(packed_stones(rough, ('<', '=', '~', ':', ';', ';'), size=3, seed='pillar-break'))
    for (x, y) in rough:
        if (x, y - 1) not in pts and unit('pbm', x // 2) < 0.5:
            c[(x, y)] = 'T' if unit('pbm2', x) < 0.6 else 'g'
            if (x, y + 1) in c and unit('pbm3', x) < 0.5:
                c[(x, y + 1)] = 't'
    for d in range(30):
        q = (cx + 9 + round(math.sin(d * 0.5) * 1.5) + d // 12, shaft_foot - 120 + d)
        if q in c:
            c[q] = '<'
    # The plinth: coursed blocks, lit along the top.
    c = compose([(blocks(plinth.pixels(), course=9, seed='pillar-plinth', offset=top), None), (c, 'o'),
                 (mould, 'o')])
    for x in range(30, 95):
        if unit('plm', x // 3) < 0.45:
            c[(x, top)] = 'T'
            c[(x, top - 1)] = 'g' if unit('plm2', x) < 0.6 else 'G'
    # Ivy up the column's shadowed side.
    stem = cx - 18
    for d in range(150):
        y = shaft_foot - d
        x = stem + round(3 * math.sin(d * 0.13)) - (1 if d > 90 else 0)
        if (x, y) in c and y > broken(x) + 6:
            c[(x, y)] = 't'
            if d % 6 == 0:
                side = 1 if (d // 6) % 2 else -1
                for k in range(3):
                    c[(x + side * (1 + k), y - k // 2)] = 'G' if k == 0 else 'g' if k == 1 else 'T'
    # The fallen drum on its side along the wall: its fluted body lit
    # along the top, its cut end an ellipse with the dowel's socket, its
    # far end snapped; moss along its back.
    drum, x0, x1, dr = {}, 101, 126, 11
    dcy = foot - dr
    for y in range(dcy - dr, foot + 1):
        v = (y - dcy) / dr
        for x in range(x0, x1 + 1):
            if x > x1 - 3 - round(3 * unit('dsnap', y // 2)):
                continue
            f = ((math.asin(max(-1.0, min(1.0, v))) / math.pi + 0.5) * 9) % 1
            groove = -0.22 * math.cos(f * math.pi) if f >= 0.15 else 0.15
            drum[(x, y)] = ramp_at(ramp, 0.55 - 0.45 * v + groove, x, y)
    for y in range(dcy - dr, foot + 1):
        for x in range(x0 - 5, x0 + 4):
            if ((x - x0) / 4.5) ** 2 + ((y - dcy) / dr) ** 2 <= 1:
                drum[(x, y)] = '<' if abs(y - dcy) < 2 and abs(x - x0) < 2 else ramp_at(
                    ramp, 0.62 - 0.25 * (y - dcy) / dr, x, y)
    for x in range(x0 + 2, x1 - 4):
        if unit('dmoss', x // 2) < 0.55:
            drum[(x, dcy - dr)] = 'T'
            if unit('dmoss2', x) < 0.4:
                drum[(x, dcy - dr - 1)] = 'g'
    c = compose([(c, None), (drum, 'o')])
    return finish(c, (w, h))


# The aqueduct floor's paving (e01hollow): its pieces lie in the floor's
# turf band, their foot `PAVING_SINK` px into the floor's top, the most the
# floating check lets a standing piece sink.
PAVING_SINK = 6


def aq_paving(k):
    """The aqueduct floor's own family (e01hollow), variant `k` of 4, laid
    in the turf band under the arcade where the old road ran: worn flags,
    their moonlit tops crossing the band, joints slanting with the view and
    mossed, one cracked (0); a flag heaved up on its edge over a gap of
    gravel and a tuft (1); a carved gutter stone crossing the band with
    water in its groove (2); gravel and pebbles with clover where the flags
    are gone (3). Each is a tile wide and fades into the turf at its ends."""
    c = {}
    band = range(TILE - PAVING_SINK, TILE)
    top = TILE - PAVING_SINK
    if k in (0, 1):
        joints = (4, 25, 46, 62) if k == 0 else (6, 30, 58)
        for y in band:
            v = y - top
            for x in range(joints[0], joints[-1]):
                shift = round(v * 0.6)
                joint = any(x == j + shift for j in joints)
                n = sum(1 for j in joints if j + shift <= x)
                if joint:
                    sym = 'o' if v > 0 else '<'
                elif v == 0:
                    sym = ';'
                elif v == PAVING_SINK - 1:
                    sym = '<'
                else:
                    t = unit('flag', k, n, x // 3, v)
                    sym = ':' if t < 0.55 else ';' if t < 0.7 else '~'
                c[(x, y)] = sym
        if k == 0:
            x = 33
            for y in band:
                x += 1 if unit('fcr', y) > 0.5 else 0
                c[(x, y)] = '<'
        else:
            # The middle flag heaved up on its far edge, gravel in the gap.
            for x in range(31, 56):
                c[(x, top - 1)] = ';'
                c[(x, top - 2)] = ':' if x < 54 else 'o'
                c[(x, top)] = ':'
            c.update(packed_stones({(x, y) for x in range(26, 32) for y in band}, ('o', '<', '-', '=', ':'), size=2,
                                   seed='pave-gravel'))
            for t in range(5):
                c[(28 + t % 3, top - 1 - t // 2)] = 'g' if t % 2 else 'G'
        for j in joints:
            for t in range(2 + int(unit('fjm', k, j) * 3)):
                q = (j + round((t % PAVING_SINK) * 0.6), top + t % PAVING_SINK)
                if q in c:
                    c[q] = 'T' if t % 2 else 't'
    elif k == 2:
        for y in band:
            v = y - top
            for x in range(2, 62):
                shift = round(v * 0.6)
                u = x - 20 - shift
                if 0 <= u < 24:
                    sym = '<' if u in (0, 23) else (':' if u < 4 or u > 19 else
                                                     '}' if v < 2 and dither(x, y, 0.6) else '{' if v < 4 else '|')
                    if 4 <= u <= 19 and v == 0:
                        sym = '_' if dither(x, y, 0.5) else '}'
                else:
                    sym = ';' if v == 0 else '<' if v == PAVING_SINK - 1 else ':' if unit('gut', x // 4, v) < 0.6 else '~'
                c[(x, y)] = sym
        for x in range(2, 62):
            if unit('gutm', x // 2) < 0.3:
                c[(x, top)] = 'T'
    else:
        inside = {(x, y) for x in range(3, 61) for y in band}
        c.update(packed_stones(inside, ('<', '-', '=', '~', ':', ';'), size=3, seed='pave-pebbles'))
        for x in range(3, 61):
            if unit('clover', x // 3) < 0.45:
                for t in range(1 + int(unit('clh', x) * 3)):
                    c[(x, top - 1 - t + (0 if t else 1))] = 'G' if t == 0 else 'g'
    # The turf creeping over the paving's ends, a few blades standing up.
    for x in list(range(0, 6)) + list(range(58, TILE)):
        for y in band:
            if (x, y) in c and dither(x, y, 0.75 - min(x, TILE - 1 - x) * 0.12):
                c[(x, y)] = 'g' if y - top < 2 else 'T'
    for n in range(4):
        x = int(unit('pblade', k, n) * 60) + 2
        for t in range(2 + n % 3):
            c[(x + (t % 2), top - 1 - t)] = 'G' if t == 0 else 'g'
    return to_image(c)


def fallen_capital():
    """A capital fallen from the arcade's lost span (e01hollow), lying
    tilted on the floor: an abacus slab over a moulded echinus, a
    fragment of the shaft still socketed under it, chipped corners, moss
    along its top."""
    slab, echinus, stub = Part(), Part(), Part()
    slab.polygon([(8, 47), (52, 42), (54, 50), (10, 55)])
    echinus.polygon([(14, 55), (49, 51), (46, 58), (18, 61)])
    stub.polygon([(22, 61), (43, 58), (44, 63), (22, 63)])
    c = compose([
        (blocks(stub.pixels(), course=20, seed='cap-stub'), None),
        (shade(echinus.pixels(), '~', light=':', shadow='-', back='='), 'o'),
        (shade(slab.pixels(), ':', light=';', shadow='-', back='~'), 'o'),
    ])
    for x in range(10, 53):
        y = 47 - round((x - 8) * 5 / 44)
        if unit('capm', x // 3) < 0.5:
            c[(x, y)] = 'T' if unit('capm2', x) < 0.6 else 'g'
    for q in ((51, 43), (52, 43), (52, 44), (9, 54), (10, 54)):
        c.pop(q, None)
    for x in range(16, 47, 4):
        y = 56 + (x - 16) // 12
        if (x, y) in c:
            c[(x, y)] = '<'
    return finish(c)


# Terracotta, dark to light (e01hollow's amphora).
TERRACOTTA = ('а', 'п', 'ω', '(', '"', "'", 'a')


def amphora_radius(t):
    """The amphora's half width `t` px along its axis from the rim: a
    collared rim, the long neck, a carinated shoulder, the body tapering to
    a solid spike toe."""
    if t < 2.5:
        return 3.5
    if t < 11:
        return 2.5
    if t < 16:
        return 2.5 + (t - 11) * 0.95
    if t < 40:
        return 7.3 - (t - 16) * 0.11
    if t < 48:
        return 4.6 - (t - 40) * 0.38
    return 1.6


def amphora():
    """A Roman amphora lying broken on the floor (e01hollow, fully
    redrawn): long and slender, its collared rim and neck raised off the
    ground on a handle, two handles looping from the carinated shoulder to
    the neck, its body tapering to a spike toe in the dirt. A piece of its
    upper belly is broken out, dark within, a pale broken lip below it and
    its sherds lying round it. Wheel ridges ring the body, a weed grows out
    of the break and moss creeps up from the ground."""
    tilt = math.radians(5)
    ax, ay = math.cos(tilt), math.sin(tilt)
    nx, ny = -math.sin(tilt), math.cos(tilt)
    ox, oy, length = 5.0, 54.0, 52
    body = {}
    for y in range(TILE):
        for x in range(TILE):
            dx, dy = x - ox, y - oy
            t, v = dx * ax + dy * ay, dx * nx + dy * ny
            if 0 <= t <= length:
                r = amphora_radius(t)
                if abs(v) <= r:
                    body[(x, y)] = (t, v / r)
    # Settle it so the belly's lowest pixel lies on the tile's floor.
    drop = TILE - 1 - max(y for _, y in body)
    body = {(x, y + drop): tv for (x, y), tv in body.items()}
    c = {}
    for (x, y), (t, v) in body.items():
        value = 0.6 - 0.5 * v - (0.25 if t < 11 else 0)
        if -0.75 < v < -0.45 and 14 < t < 40:
            value += 0.25
        if any(abs(t - ring) < 0.6 for ring in (18, 23, 29, 34)):
            value -= 0.2
        c[(x, y)] = ramp_at(TERRACOTTA, value, x, y, 0.3)

    def at(t, v):
        return round(ox + t * ax + v * nx), round(oy + t * ay + v * ny) + drop

    # The smashed hole on the upper belly: dark within, the far wall lit
    # through it, a pale broken lip along its lower edge.
    hole = {p for p, (t, v) in body.items()
            if 23 < t < 34 and v < -0.4 + 0.3 * math.sin((t - 23) / 11 * math.pi)
            + (unit('ahole', round(t) // 2) - 0.5) * 0.25}
    for p in hole:
        t, v = body[p]
        c[p] = '■' if v < -0.55 else 'а' if v < -0.3 else 'п'
    for p in hole:
        x, y = p
        if (x, y + 1) in body and (x, y + 1) not in hole:
            c[(x, y + 1)] = "'"
    for p in [p for p in body if p not in hole]:
        x, y = p
        if (x, y + 1) in hole or (x + 1, y) in hole or (x - 1, y) in hole:
            c[p] = 'ω'
    # The handles: loops from the shoulder up to the neck's top.
    handles = Part()
    for side in (-1, 1):
        handles.limb([at(13.5, side * 7.5), at(9, side * 9.5), at(4.5, side * 4.5)], 2)
    for p in handles.pixels():
        if p not in body or body[p][1] * (1 if p[1] > oy + drop else -1) < 0:
            c.setdefault(p, ramp_at(TERRACOTTA, 0.55 if p[1] < oy + drop else 0.25, *p, 0.3))
    # The rim's collar and mouth, the toe's knob.
    for v in range(-3, 4):
        c[at(0.5, v)] = "'" if v < 0 else '"'
    c[at(0.5, 0)] = '■'
    c[at(0.5, 1)] = 'а'
    outline(c, 'o')
    # Sherds round it, one leaning on its side, their broken edges lit.
    for k, (sx, w0, h0) in enumerate(((22, 5, 2), (33, 4, 2), (47, 6, 3), (55, 3, 2), (60, 4, 2))):
        sy = TILE - 1
        for dx in range(w0):
            for dy in range(h0 - (dx in (0, w0 - 1))):
                q = (sx + dx, sy - dy)
                if q not in c:
                    c[q] = "'" if dy == h0 - 1 - (dx in (0, w0 - 1)) else '(' if k % 2 else '"'
    # A weed out of the break; moss and dirt where it lies.
    wx, wy = at(28.5, -4)
    for k, (dx, hh) in enumerate(((0, 6), (2, 4), (-2, 3))):
        for d in range(hh):
            c[(wx + dx + (d // 4) * (1 if dx >= 0 else -1), wy - d)] = 'G' if d == hh - 1 else 'g' if d > 1 else 'T'
    for x in range(8, 62):
        if (x, TILE - 1) in c and unit('amoss', x // 2) < 0.45:
            c[(x, TILE - 1)] = 'T' if unit('amoss2', x) < 0.6 else '>'
            if unit('amoss3', x) < 0.3 and (x, TILE - 2) in c:
                c[(x, TILE - 2)] = 't'
    return to_image(c)


def ramp_at(ramp, value, x, y, spread=0.35):
    """`value` (0 to 1) through `ramp`, dark to light, by the ordered
    dither."""
    from pixels import ramp as pick
    return pick(max(0.0, min(0.999, value)), ramp, x, y, spread)


def pool_rocks(i):
    layers = []
    for cx, cy, rx, ry in ((22, 54, 14, 8), (44, 57, 10, 5)):
        rock = Part()
        rock.ellipse((cx, cy), rx, ry)
        colors = shaded(rock, STONE)
        for x in range(cx - rx + 2, cx + rx - 1):
            colors[(x, cy + ry - 1)] = '}' if dither(x, cy, 0.5) else ')'
        for x in range(cx - rx // 2, cx + 2):
            if unit('rmoss', i, x) < 0.5:
                colors[(x, cy - ry)] = 'T'
        layers.append((colors, 'o'))
    return finish(compose(layers))


def rope_ladder(i):
    size = (32, 2 * TILE)
    ropes, rungs = Part(size), Part(size)
    for x in (6, 25):
        ropes.limb([(x + round(math.sin(k * 0.9)), k * 10) for k in range(13)], 2)
    for y in range(8, 120, 14):
        rungs.limb([(7, y), (25, y + 1)], 3)
    c = compose([(shade(ropes.pixels(), '"', light="'", shadow='ω'), None),
                 (shade(rungs.pixels(), '"', light="'", shadow='/', back='ω'), 'o')])
    return finish(c, size)


# The hollow's stalactites (e01hollow, round 2), eight of them, each a
# list of points hanging from the roof: x, length, half width at its root,
# how it tapers (above 1 long and slender, below 1 stubby), a lean, and
# its tip: `drip` a glowing drop, `snap` broken off, `fork` split in two.
STALACTITES = (
    ((32, 44, 12, 1.3, 0.0, 'drip'), (17, 24, 7, 1.2, -0.1, None), (46, 18, 6, 1.1, 0.1, None)),
    ((14, 22, 2, 1.0, 0.0, None), (21, 38, 3, 1.0, 0.0, 'drip'), (27, 16, 2, 1.0, 0.0, None),
     (33, 46, 3, 1.0, 0.05, None), (40, 28, 2, 1.0, 0.0, None), (47, 12, 2, 1.0, 0.0, None)),
    ((32, 24, 16, 0.6, 0.0, 'drip'), (15, 10, 6, 0.7, 0.0, None)),
    ((32, 40, 21, 0.0, 0.0, 'curtain'),),
    ((31, 60, 9, 1.4, 0.03, 'drip'),),
    ((32, 26, 14, 0.8, 0.0, 'snap'), (49, 14, 5, 1.0, 0.1, None)),
    ((30, 40, 11, 1.1, 0.0, 'fork'), (12, 14, 4, 1.0, -0.1, None)),
    ((18, 9, 4, 0.9, 0.0, None), (27, 14, 4, 1.0, 0.0, 'drip'), (37, 7, 3, 0.9, 0.0, None), (45, 11, 4, 1.0, 0.0, None)),
)


def stalactite(i):
    """A stalactite of the hollow's basalt hanging from the roof (e01hollow,
    round 2), `STALACTITES[i]`: a big point with lesser ones, a cluster of
    needles, a fat stub, a fluted curtain, a long spear, one snapped off, a
    forked pair, a row of nubs; each ridged down its length, lit along its
    west face, a glowing drop at some tips."""
    layers = []
    for x0, length, half, taper, lean, tip in STALACTITES[i]:
        p = Part()
        if tip == 'curtain':
            # A fluted curtain: a sheet with a scalloped hem, folds down it.
            pts = [(x0 - half, 0), (x0 + half, 0)]
            for k in range(13):
                x = x0 + half - k * 2 * half / 12
                pts.append((x, length * (0.55 + 0.45 * abs(math.sin(k * 1.3 + 0.4)))))
            p.polygon(pts)
        else:
            end = length * (0.62 if tip == 'snap' else 1.0)
            pts = []
            for k in range(9):
                t = k / 8
                w = half * (1 - t) ** taper if tip != 'snap' else half * (1 - 0.5 * t)
                pts.append((x0 - w + lean * end * t, end * t))
            for k in range(8, -1, -1):
                t = k / 8
                w = half * (1 - t) ** taper if tip != 'snap' else half * (1 - 0.5 * t)
                pts.append((x0 + w + lean * end * t, end * t))
            if tip == 'snap':
                pts[8:10] = [(x0 - half * 0.5, end), (x0 - 2, end + 3), (x0 + 1, end - 1), (x0 + half * 0.5, end + 2)]
            p.polygon(pts)
        colors = shaded(p, BASALT_SHADE)
        for (x, y) in list(colors):
            ridge = (x - x0 - round(y * lean) + round(y * 0.07)) % (3 if tip == 'curtain' else 5)
            if colors[(x, y)] == BASALT_SHADE['base'] and ridge == 0:
                colors[(x, y)] = 'ν'
            elif colors[(x, y)] == BASALT_SHADE['base'] and ridge == 2 and y < length * 0.5:
                colors[(x, y)] = 'α'
        if tip == 'snap':
            # The break: a pale fresh face across its end.
            bottom = max(y for _, y in colors)
            for (x, y) in list(colors):
                if y >= bottom - 2:
                    colors[(x, y)] = 'ξ' if y == bottom - 2 else 'α'
        if tip == 'fork':
            twin = Part()
            tx = x0 + 6
            twin.polygon([(tx - 3, length * 0.4), (tx + 3, length * 0.38), (tx + 9, length * 0.95), (tx + 7, length * 0.98)])
            colors.update(shaded(twin, BASALT_SHADE))
        if tip == 'drip':
            tx = round(x0 + lean * length)
            colors[(tx, round(length) + 1)] = 'Ъ'
            colors[(tx, round(length) + 3)] = 'y'
        layers.append((colors, 'o'))
    return finish(compose(layers))


def stalagmite(i):
    """A cluster of basalt cones rising from the floor, ridged, ash-capped,
    a cooled drip-crust around their feet."""
    layers = []
    for k, (dx, height, width) in enumerate(((-12, 20 + (i % 2) * 5, 8), (12, 16 + (i % 3) * 4, 7), (0, 34 + (i % 3) * 8, 14))):
        cx = 32 + dx
        p = Part()
        p.polygon([(cx - width, 61), (cx - width * 0.35, 61 - height * 0.55), (cx, 61 - height), (cx + width * 0.3, 61 - height * 0.6), (cx + width, 61)])
        colors = shaded(p, BASALT_SHADE)
        for (x, y) in list(colors):
            ridge = (x - cx - round((61 - y) * 0.06)) % 5
            if colors[(x, y)] == BASALT_SHADE['base'] and ridge == 0:
                colors[(x, y)] = 'ν'
            if y < 61 - height + 5 and dither(x, y, 0.65):
                colors[(x, y)] = 'Φ'
        layers.append((colors, 'o'))
    c = compose(layers)
    for x in range(14, 51):
        if unit('crust', i, x) < 0.5:
            c[(x, 61)] = 'γ'
            if unit('crust2', i, x) < 0.15:
                c[(x, 60)] = 'L'
    return finish(c)


# The hollow's small lava pools (e01hollow, round 3): each one's half
# widths and its bubble's place, and its clip's frames and pace.
EMBER_POOLS = ((21, 4.6, 0.3), (16, 4.0, -0.25))
POOL_FRAMES, POOL_MS = 4, 180


def ember_pool(i, frame=0):
    """A small pool of lava in a lip of cooled basalt (e01hollow, round 3),
    frame `frame` of `POOL_FRAMES`: its skin orange under meandering yellow
    veins that drift, dark crust rafts on it, a bubble swelling, bursting
    in a spray of drops and settling; the lip's far face lit from below by
    it, the near lip's crest catching its glow, embers rising off it."""
    rx, ry, spot = EMBER_POOLS[i]
    cx, cy = 32, 54
    drift = frame * 2.5
    lip = ellipse_pixels((TILE, TILE), cx, cy + 0.5, rx + 5, ry + 4)
    pool = ellipse_pixels((TILE, TILE), cx, cy, rx, ry)
    c = grade(lip - pool, 'ενγβα', light=(-0.5, -0.9, 0.6), spread=0.3, depth=3, fall=0.2)
    for (x, y) in lip - pool:
        inner = any((x + dx, y + dy) in pool for dx, dy in ((0, 1), (0, -1), (1, 0), (-1, 0)))
        if inner and y < cy:
            c[(x, y)] = 'Ъ' if abs(x - cx) < rx * 0.75 else 'K'
        elif inner:
            c[(x, y)] = 'L'
        elif (x, y - 1) not in lip and y > cy:
            c[(x, y)] = 'M'
    for (x, y) in pool:
        r = math.hypot((x + 0.5 - cx) / rx, (y + 0.5 - cy) / ry)
        vein = abs(fbm((x - drift) / 6, y / 2.2, ('pool-vein', i)) - 0.5)
        raft = fbm((x - drift * 0.6) / 7 + 9, y / 3, ('pool-raft', i))
        heat = 1 - r * 0.6
        if raft > 0.66 and r > 0.25:
            sym = 'θ' if raft > 0.74 else 'M'
        elif vein < 0.05:
            sym = 'Y' if heat > 0.75 else 'y'
        elif vein < 0.11:
            sym = 'y' if heat > 0.6 else 'Ъ'
        else:
            sym = 'Ъ' if heat > 0.55 else 'K'
        c[(x, y)] = sym
    # The bubble: swelling, bursting, its ring settling.
    bx, by = round(cx + spot * rx), round(cy)
    if frame == 0:
        c[(bx, by)], c[(bx + 1, by)] = 'y', 'Y'
    elif frame == 1:
        for dx in range(-2, 3):
            c[(bx + dx, by - (1 if abs(dx) < 2 else 0))] = 'Y' if abs(dx) < 2 else 'y'
        c[(bx - 1, by - 2)], c[(bx, by - 2)] = 'Z', 'Y'
    elif frame == 2:
        for dx, dy in ((-3, 0), (3, 0), (-2, -1), (2, -1)):
            c[(bx + dx, by + dy)] = 'Ъ'
        for dx, dy in ((-2, -4), (1, -6), (3, -3), (-1, -8)):
            c[(bx + dx, by + dy)] = 'y'
    else:
        for dx in (-4, -3, 3, 4):
            c[(bx + dx, by)] = 'K'
        c[(bx, by - 10)] = 'Ъ'
    # Embers rising off it.
    for k in range(3 + i):
        x = round(cx + (unit('mote', i, k) - 0.5) * rx * 1.4)
        y = round(cy - 7 - ((unit('motey', i, k) * 14 + frame * 3.5) % 18))
        c[(x, y)] = 'Y' if k % 2 else 'y'
    bottom = max(y for _, y in c)
    return finish({(x, y + 62 - bottom): sym for (x, y), sym in c.items()})


def obsidian(i):
    layers = []
    for k in range(4):
        bx = 8 + int(unit('shard', i, k) * 44)
        hgt = 12 + int(unit('shh', i, k) * 20)
        lean = (unit('shl', i, k) - 0.5) * 10
        shard = Part()
        shard.polygon([(bx - 6, 61), (bx + lean, 61 - hgt), (bx + 6, 61)])
        colors = shade(shard.pixels(), '2', light='δ', shadow='0', back='0', rim='ξ')
        for (x, y) in list(colors):
            if colors[(x, y)] == '2' and unit('glint', i, k, y) < 0.05:
                colors[(x, y)] = 'π'
        layers.append((colors, 'o'))
    return finish(compose(layers))


def chunk(points, seed, glass=False, light=0.0):
    """An angular chunk of the hollow's basalt (e01hollow's rubble, of the
    stones' material): `points` its outline, split into planes from a
    ridge point, each lit by how it faces the upper left, crisp at the
    ridges; `glass` turns it to black obsidian with a hard sheen."""
    xs, ys = [p[0] for p in points], [p[1] for p in points]
    ridge = (min(xs) + (max(xs) - min(xs)) * (0.38 + 0.2 * unit('ridge', seed)), min(ys) + (max(ys) - min(ys)) * 0.45)
    colors = {}
    for k in range(len(points)):
        a, b = points[k], points[(k + 1) % len(points)]
        plane = Part()
        plane.polygon([a, b, ridge])
        mx, my = (a[0] + b[0]) / 2 - ridge[0], (a[1] + b[1]) / 2 - ridge[1]
        norm = math.hypot(mx, my) or 1
        level = 0.5 + 0.38 * (-0.6 * mx / norm - 0.8 * my / norm) + light + (unit('plane', seed, k) - 0.5) * 0.12
        for (x, y) in plane.pixels():
            if glass:
                colors[(x, y)] = '0' if level < 0.55 else '2'
            else:
                colors[(x, y)] = ramp(level, 'ενγβαξ', x, y, 0.1)
    # The ridges: a lit edge on the upper planes' side.
    for (x, y), sym in list(colors.items()):
        if (x, y - 1) not in colors:
            colors[(x, y)] = ('δ' if glass else 'ξ') if colors[(x, y)] not in 'εν0' else ('2' if glass else 'γ')
    if glass:
        for k, (x, y) in enumerate(sorted(colors, key=lambda p: (p[1], p[0]))[:3]):
            colors[(x, y)] = 'π'
    return colors


def heap(pieces, seed, size=(TILE, TILE), foot=61):
    """Chunks piled back to front, each `(cx, width, height, lift, glass)`,
    their feet on row `foot` less `lift`; outlined."""
    layers = []
    for k, (cx, w, h, lift, glass) in enumerate(pieces):
        y0 = foot - lift
        jag = [unit('jag', seed, k, n) for n in range(6)]
        points = [(cx - w * 0.5, y0), (cx - w * (0.42 + 0.1 * jag[0]), y0 - h * (0.45 + 0.2 * jag[1])),
                  (cx - w * (0.1 + 0.15 * jag[2]), y0 - h), (cx + w * (0.15 + 0.2 * jag[3]), y0 - h * (0.85 + 0.15 * jag[4])),
                  (cx + w * 0.5, y0 - h * (0.3 + 0.25 * jag[5])), (cx + w * 0.47, y0)]
        layers.append((chunk(points, (seed, k), glass, -0.06 * (k % 2)), 'o'))
    return finish(compose(layers), size)


# The hollow's rubble heaps (e01hollow): chunks `(cx, width, height, lift,
# glass)`, back to front.
RUBBLE_HEAPS = (
    ((20, 14, 9, 0, False), (40, 18, 12, 0, False), (30, 22, 16, 0, False), (47, 9, 6, 0, True),
     (14, 8, 5, 0, False)),
    ((24, 26, 19, 0, False), (42, 14, 10, 0, False), (36, 10, 7, 0, True), (16, 11, 7, 0, False),
     (50, 7, 5, 0, False)),
    ((32, 12, 8, 0, False), (22, 9, 6, 0, True), (42, 10, 7, 0, False)),
)
OBSIDIAN_HEAP = ((26, 16, 14, 0, True), (40, 12, 10, 0, True), (33, 8, 6, 0, True), (17, 7, 5, 0, False),
                 (48, 6, 4, 0, True))


def scree(side, seed):
    """Scree heaped against a wall on its `side`: a slope of small basalt
    chips packed under a few larger chunks, tallest at the wall."""
    slope = set()
    for x in range(1, 62):
        u = x / 61 if side == 'left' else 1 - x / 61
        # A concave slope, steep at the wall, its surface lumpy with stones.
        top = 22 + 39 * (1 - (1 - u) ** 1.8) + 2.2 * math.sin(x * 0.9 + seed) + 1.5 * math.sin(x * 0.37)
        for y in range(round(top), 62):
            slope.add((x, y))
    pebbles = packed_stones(slope, 'θνγβαξ', size=6, seed=('scree', side, seed), mortar='ε')
    for (x, y), sym in list(pebbles.items()):
        if (x, y - 1) not in pebbles and sym not in 'εθ':
            pebbles[(x, y)] = 'ξ' if dither(x, y, 0.6) else 'α'
    layers = [(pebbles, None)]
    for k, (fx, w, h) in enumerate(((12, 12, 8), (26, 9, 6), (44, 8, 5)) if side == 'left'
                                   else ((52, 12, 8), (38, 9, 6), (20, 8, 5))):
        foot = 61 - round((61 - 22) * (1 - fx / 62 if side == 'left' else fx / 62) * 0.55)
        points = [(fx - w / 2, foot), (fx - w / 2 + 1, foot - h * 0.6), (fx - 1, foot - h), (fx + w / 2 - 1, foot - h * 0.7),
                  (fx + w / 2, foot)]
        layers.append((chunk(points, ('scree-chunk', side, seed, k)), 'o'))
    return finish(compose(layers))


def fallen_slab():
    """A slab of basalt fallen from the roof: flat, broken across, its two
    halves tilted where they lie, its underside dark, its top crazed."""
    layers = []
    for k, pts in enumerate((((3, 61), (5, 50), (33, 46), (35, 57), (34, 61)),
                             ((32, 61), (36, 52), (60, 55), (61, 61)))):
        piece = Part()
        piece.polygon(pts)
        px = piece.pixels()
        colors = {}
        for (x, y) in px:
            top = (x, y - 3) not in px
            colors[(x, y)] = ('ξ' if (x, y - 1) not in px else 'α') if top else ramp(
                0.35 - 0.2 * k + (unit('slab', k, x // 4, y // 2) - 0.5) * 0.15, 'ενγβ', x, y, 0.1)
        for (x, y) in px:
            if (x * 3 + y * 7 + k) % 23 == 0 and colors[(x, y)] in 'γβ':
                colors[(x, y)] = 'ν'
        layers.append((colors, 'o'))
    return finish(compose(layers))


def crate(i):
    box, brace = Part(), Part()
    box.rectangle((10, 20, 54, 61))
    brace.limb([(13, 58), (51, 23)], 4)
    c = compose([(shaded(box, BARK, light="'", rim='"'), None), (shaded(brace, BARK, light='"'), 'o')])
    for y in range(28, 61, 8):
        for x in range(11, 54):
            if (x, y) in c and c[(x, y)] not in ('"', "'"):
                c[(x, y)] = '/'
    for x, y in ((10, 20), (50, 20), (10, 57), (50, 57)):
        rect(c, x, y, x + 5, y + 5, lambda xx, yy: 'χ' if (xx, yy) == (x + 1, y + 1) else 'Ω' if (xx + yy) % 2 else 'ψ')
    if i % 2:
        ellipse(c, 33, 36, 5, 3, lambda dx, dy: '(' if dither(dx + 33, dy + 36, 0.6) else None)
    return finish(c)


def barrel(i):
    body = Part()
    body.polygon([(19, 22)] + [(16 - round(2 * math.sin(math.pi * k / 10)), 22 + k * 4) for k in range(11)]
                 + [(48 + round(2 * math.sin(math.pi * k / 10)), 62 - k * 4) for k in range(11)] + [(45, 22)])
    c = shaded(body, BARK, light="'", rim='"')
    for (x, y) in list(c):
        if (x - 16) % 5 == 0 and c[(x, y)] == BARK['base']:
            c[(x, y)] = '/'
    for hy in (27, 41, 55):
        for x in range(10, 54):
            if (x, hy) in c:
                c[(x, hy)], c[(x, hy + 1)] = 'ψ', 'Ω'
                if (x, hy + 2) in c:
                    c[(x, hy + 2)] = 'Φ'
                if unit('hoop', i, x // 3, hy) < 0.2:
                    c[(x, hy + 1)] = 'a'
    ellipse(c, 32, 22, 13, 3, lambda dx, dy: '"' if abs(dx) > 10 or dy < -1 else '>')
    return finish(c)


def pickaxe(i):
    handle, head = Part(), Part()
    handle.limb([(22, 61), (36, 22)], 3)
    head.polygon([(18, 24), (30, 16), (42, 16), (55, 26), (52, 28), (40, 21), (32, 21), (21, 28)])
    c = compose([(shaded(handle, BARK, light="'"), None), (shaded(head, IRON, rim='S'), 'o')])
    for (x, y) in list(c):
        if c[(x, y)] == IRON['base'] and unit('prust', i, x // 2, y) < 0.25:
            c[(x, y)] = 'a'
    return finish(c)


def winch(i):
    legs, drum, crank, rope = Part(), Part(), Part(), Part()
    legs.limb([(8, 61), (26, 22)], 4)
    legs.limb([(56, 61), (38, 22)], 4)
    legs.limb([(14, 46), (50, 46)], 3)
    drum.ellipse((32, 28), 13, 7)
    crank.limb([(44, 28), (52, 18), (56, 18)], 2)
    rope.limb([(30, 34), (29, 63)], 2)
    drum_px = drum.pixels()
    c = compose([
        (shaded(legs, BARK, light="'", rim='"'), None),
        (shade(rope.pixels(), '"', light="'", shadow='ω'), None),
        (shade(drum_px, BARK['base'], light="'", shadow=BARK['shadow'], back=BARK['back']), 'o'),
        (shaded(crank, IRON, rim='S'), 'o'),
    ])
    for (x, y) in drum_px:
        if (x + 30) % 4 < 2 and c.get((x, y)) == BARK['base']:
            c[(x, y)] = '"'
    return finish(c)


def chain(i, links=15):
    """A hanging chain of `links` links, a hook at its foot."""
    c = {}
    size = (16, 2 * TILE)
    for k in range(links):
        y = 4 + k * 8
        link = Part(size)
        if k % 2 == 0:
            link.ellipse((8, y), 3, 5)
            hole = {(x, yy) for x in range(6, 11) for yy in range(y - 3, y + 4) if ((x - 8) / 1.5) ** 2 + ((yy - y) / 3) ** 2 <= 1}
            c.update({p: s for p, s in shade(link.pixels(), 'Ω', light='ψ', shadow='B', back='Φ').items() if p not in hole})
        else:
            link.rectangle((7, y - 4, 9, y + 4))
            c.update(shade(link.pixels(), 'Ω', light='ψ', shadow='B'))
    foot = 4 + links * 8
    hook = Part(size)
    hook.limb([(8, foot - 3), (8, foot + 6), (6, foot + 9), (3, foot + 7), (3, foot + 4)], 2)
    c.update(shade(hook.pixels(), 'Ω', light='ψ', shadow='B'))
    return finish(c, size)


def skull_pike(i):
    size = (TILE, 2 * TILE)
    pole, skull, jaw, rags = Part(size), Part(size), Part(size), Part(size)
    pole.limb([(32, 125), (32, 28)], 4)
    skull.ellipse((32, 20), 9, 8)
    jaw.ellipse((32, 28), 6, 3)
    for k in range(4):
        x0 = 27 + k * 3
        rags.limb([(x0, 33), (x0 + round(math.sin(k) * 2), 42 + k * 3), (x0 + round(math.cos(k) * 3), 50 + k * 4)], 2)
    c = compose([
        (shaded(pole, BARK, light="'"), None),
        (shade(rags.pixels(), 'X', light='E', shadow='M'), 'o'),
        (shade(jaw.pixels(), **BONE), 'o'),
        (shade(skull.pixels(), **BONE), 'o'),
    ])
    for x, y in ((28, 20), (29, 20), (35, 20), (36, 20), (28, 21), (29, 21), (35, 21), (36, 21), (32, 24)):
        c[(x, y)] = '0'
    for x in range(28, 37, 2):
        c[(x, 29)] = '0'
    return finish(c, size)


def campfire(i):
    """Stones in a ring, crossed logs and tongues of flame (drawn unlit), a
    spit across with a haunch, sparks rising."""
    layers = []
    for k in range(7):
        a = math.pi * (k / 6)
        stone = Part()
        stone.ellipse((round(32 + 19 * math.cos(a)), round(58 - 2 * math.sin(a))), 4, 3)
        layers.append((shaded(stone, STONE), 'o'))
    logs = Part()
    logs.limb([(16, 57), (46, 49)], 5)
    logs.limb([(16, 49), (46, 57)], 5)
    layers.append((shaded(logs, BARK, light="'"), 'o'))
    for ring, sym in ((1.0, 'L'), (0.78, 'K'), (0.52, 'J'), (0.28, 'Y')):
        flame = Part()
        pts = [(32 - 14 * ring, 52)]
        for k in range(7):
            x = 32 - 14 * ring + k * 28 * ring / 6
            tip = 52 - (26 + 8 * math.sin(k * 1.7 + i)) * ring * (0.55 + 0.45 * math.sin(math.pi * k / 6))
            pts += [(x, tip), (x + 2 * ring, 52 - 10 * ring)]
        pts.append((32 + 14 * ring, 52))
        flame.polygon(pts)
        layers.append(({p: sym for p in flame.pixels()}, None))
    spit, haunch = Part(), Part()
    spit.limb([(8, 61), (9, 26)], 2)
    spit.limb([(56, 61), (55, 26)], 2)
    spit.limb([(6, 27), (58, 27)], 2)
    haunch.ellipse((33, 27), 7, 4)
    layers.append((shaded(spit, BARK, light="'"), None))
    layers.append((shade(haunch.pixels(), 'R', light='F', shadow='r', back='r'), 'o'))
    c = compose(layers)
    for k in range(6):
        c[(24 + int(unit('spark', i, k) * 16), 6 + int(unit('sparky', i, k) * 12))] = 'J' if k % 2 else 'K'
    return to_image(c)


def banner(i):
    size = (TILE, 2 * TILE)
    pole, bar, cloth = Part(size), Part(size), Part(size)
    pole.limb([(10, 125), (10, 6)], 4)
    bar.limb([(8, 11), (44, 11)], 3)
    pts = [(14, 13), (40, 13)]
    for k in range(9, -1, -1):
        x = 14 + k * 26 / 9
        y = 68 + round(6 * math.sin(k * 1.9 + i)) + (6 if k % 2 else 0)
        pts.append((x + round(2 * math.sin(k)), y))
    cloth.polygon(pts)
    c = compose([(shaded(pole, BARK, light="'"), None), (shaded(cloth, MAROON), 'o'), (shaded(bar, BARK, light="'"), 'o')])
    for (x, y) in list(c):
        if c[(x, y)] == MAROON['base'] and (x * 2 + y // 9) % 7 == 0:
            c[(x, y)] = 'M'
        if c[(x, y)] in ('X', 'E', 'M') and unit('hole', i, x // 3, y // 4) < 0.03:
            del c[(x, y)]
    for k in range(3):
        line(c, 21 + k * 5, 30, 25 + k * 5, 48, 'w' if k != 1 else 'σ')
    return finish(c, size)


def guardian(i):
    """A weathered stone guardian at the bridge's end: helm, shield and a
    sword point-down, two tiles tall, cracked and mossed."""
    size = (TILE, 2 * TILE)
    plinth, body, helm, shield, blade, guard = (Part(size) for _ in range(6))
    plinth.rectangle((9, 107, 55, 124))
    body.polygon([(19, 107), (21, 76), (23, 52), (26, 42), (38, 42), (41, 52), (43, 76), (45, 107)])
    helm.ellipse((32, 31), 8, 10)
    helm.polygon([(30, 20), (32, 14), (34, 20)])
    shield.polygon([(10, 58), (26, 56), (27, 74), (18, 88), (9, 74)])
    blade.polygon([(42, 54), (46, 54), (46, 98), (44, 104), (42, 98)])
    guard.rectangle((38, 50, 50, 53))
    c = compose([
        (blocks(plinth.pixels(), course=9, seed='guard-plinth', offset=107), None),
        (shaded(body, PALE_STONE), 'o'),
        (shaded(blade, STEEL), 'o'),
        (shaded(guard, STONE), 'o'),
        (shaded(shield, STONE), 'o'),
        (shaded(helm, PALE_STONE), 'o'),
    ])
    line(c, 26, 31, 38, 31, '<')
    for k in range(5):
        x, y = 22 + int(unit('gcrk', i, k) * 18), 46 + int(unit('gcy', i, k) * 54)
        for d in range(6):
            if (x + d // 2, y + d) in c:
                c[(x + d // 2, y + d)] = '<'
    for y in range(44, 106):
        x = 22 + round(math.sin(y * 0.4))
        if unit('gmoss', i, y // 2) < 0.4 and (x, y) in c:
            c[(x, y)] = 'T'
    ellipse(c, 17, 70, 3, 4, lambda dx, dy: '=' if (17 + dx, 70 + dy) in c else None)
    return finish(c, size)


def headframe(i):
    """A timber headframe over the mine's shaft: two raked legs, braces, a
    sheave wheel on top and its rope, 2 x 3 tiles."""
    size = (2 * TILE, 3 * TILE)
    legs, braces, wheel, rope = Part(size), Part(size), Part(size), Part(size)
    legs.limb([(18, 191), (58, 34)], 6)
    legs.limb([(110, 191), (70, 34)], 6)
    legs.limb([(52, 34), (76, 34)], 5)
    for y in (70, 110, 150):
        f = (y - 34) / 157
        braces.limb([(58 - 40 * f, y), (70 + 40 * f, y)], 4)
    braces.limb([(30, 150), (64, 70)], 3)
    braces.limb([(98, 150), (64, 70)], 3)
    wheel.ellipse((64, 24), 17, 17)
    rope.limb([(80, 24), (82, 191)], 2)
    c = compose([
        (shade(rope.pixels(), '"', light="'", shadow='ω'), None),
        (shaded(braces, BARK, light="'", rim='"'), None),
        (shaded(legs, BARK, light="'", rim='"'), 'o'),
    ])
    for (x, y) in wheel.pixels():
        r = math.hypot(x - 64, y - 24)
        spoke = any(abs((x - 64) * math.sin(a) - (y - 24) * math.cos(a)) < 1.2 for a in (0, math.pi / 3, 2 * math.pi / 3))
        if r > 14 or r < 3 or spoke:
            c[(x, y)] = 'ψ' if (y < 24 and x > 64) else 'Ω' if r > 14 else 'Φ'
    return finish(c, size)



def lift_gantry(i):
    """The mine's elevator gantry (M79), 3 x 3 tiles: a timber post on the
    upper level's deck, braced, carrying a jib out over the shaft with the
    sheave at its end and the rope back to a winch drum at the post's
    foot. Only the post and its brace reach the deck, so the gantry stands
    on its support while the jib overhangs the shaft."""
    size = (3 * TILE, 3 * TILE)
    post, brace, jib, wheel, rope, drum, bolts = (Part(size) for _ in range(7))
    post.limb([(146, 191), (146, 10)], 9)
    brace.limb([(182, 191), (152, 128)], 5)
    brace.limb([(146, 62), (84, 14)], 5)
    jib.limb([(16, 12), (156, 12)], 8)
    jib.limb([(150, 4), (150, 20)], 3)
    wheel.ellipse((32, 24), 14, 14)
    rope.limb([(44, 20), (168, 150)], 2)
    drum.ellipse((172, 162), 15, 10)
    bolts.limb([(140, 12), (152, 12)], 3)
    drum_px = drum.pixels()
    c = compose([
        (shaded(brace, BARK, light="'", rim='"'), None),
        (shaded(post, BARK, light="'", rim='"'), 'o'),
        (shaded(jib, BARK, light="'", rim='"'), 'o'),
        (shade(rope.pixels(), '"', light="'", shadow='ω'), None),
        (shade(drum_px, BARK['base'], light="'", shadow=BARK['shadow'], back=BARK['back']), 'o'),
        (shaded(bolts, IRON, rim='S'), None),
    ])
    for (x, y) in drum_px:
        if (x + 3) % 4 < 2 and c.get((x, y)) == BARK['base']:
            c[(x, y)] = '"'
    for (x, y) in wheel.pixels():
        r = math.hypot(x - 32, y - 24)
        spoke = any(abs((x - 32) * math.sin(a) - (y - 24) * math.cos(a)) < 1.2 for a in (0, math.pi / 3, 2 * math.pi / 3))
        if r > 11 or r < 3 or spoke:
            c[(x, y)] = 'ψ' if (y < 24 and x > 32) else 'Ω' if r > 11 else 'Φ'
    return finish(c, size)


def elevator_cage():
    """The elevator's cage (M79), 3 tiles across and a tile tall, a wide
    deck's size: an iron-shod plank floor its bottom `DECK` rows, iron
    corner posts and a crossbar, a ring at the bar's middle where the cable
    takes it, and a rail between the posts. The game stands its floor's
    body under it."""
    width, height = 3 * TILE, TILE
    floor = height - DECK
    size = (width, height)
    planks, posts, bar, rail, ring = (Part(size) for _ in range(5))
    planks.rectangle((2, floor, width - 3, height - 6))
    for x in (4, width - 5):
        posts.limb([(x, floor + 2), (x, 8)], 5)
    bar.limb([(4, 8), (width - 5, 8)], 5)
    rail.limb([(6, floor - 18), (width - 7, floor - 18)], 2)
    ring.ellipse((width // 2, 4), 4, 4)
    c = compose([
        (shaded(rail, IRON, rim='S'), None),
        (shade(planks.pixels(), BARK['base'], light="'", shadow=BARK['shadow'], back=BARK['back']), 'o'),
        (shaded(posts, IRON, rim='S'), 'o'),
        (shaded(bar, IRON, rim='S'), 'o'),
        (shaded(ring, BRASS), None),
    ])
    for (x, y) in planks.pixels():
        if x % 24 == 0:
            c[(x, y)] = '/'
        if y == floor:
            c[(x, y)] = 'A'
    for x in range(2, width - 3):
        for y in range(height - 6, height - 1):
            c[(x, y)] = 'B' if y == height - 6 else 's' if (x // 3) % 2 else 'S'
    return finish(c, size)


def lift_cable():
    """A stretch of the elevator's cable down a tile's middle columns: a dark
    rope with a lit strand, which the game stretches from the sheave to the
    cage."""
    c = {}
    for y in range(TILE):
        c[(31, y)] = "'" if (y // 3) % 2 else '"'
        c[(32, y)] = 'ω'
    return to_image(c)


def water_frame(frame):
    """The channel's water riding in its trough behind the coping's near
    capstones (`terrain.coping`), frame `frame` of 8: the surface wave and
    its glints move 8 px a frame; the capstones' top face hides the rest."""
    c = {}
    for x in range(TILE):
        u = (x - 8 * frame) % TILE
        surface = 2 + round(math.sin(math.tau * u / 32) * 0.8)
        for y in range(surface, COPING_TOP):
            if y == surface:
                c[(x, y)] = '`' if unit('glint', u // 3) < 0.3 else '_'
            elif y < COPING_TOP - 1:
                c[(x, y)] = '}' if dither(x, y, 0.55) else ')'
            else:
                c[(x, y)] = '{' if dither(x, y, 0.5) else '|'
    return to_image(c)


def river_frame(frame, kind='calm', deep=False):
    """Deep night water, frame `frame` of 8: mostly the darkest blues, a lit
    skin along the top (none under it, `deep`), rare foam drifting 8 px a
    frame; rapids streak white, a rock breaks the surface."""
    phase = math.tau * frame / 8
    c = {}
    for y in range(TILE):
        yy = y + (TILE if deep else 0)
        for x in range(TILE):
            u = (x - 8 * frame) % TILE
            flow = math.sin(math.tau * (x / 32 + yy / 32) + phase) + 0.6 * math.sin(math.tau * (x / 16 - yy / 16) + 2 * phase)
            if yy == 0:
                sym = '}' if dither(x, y, 0.35) else ')'
            elif yy < 4:
                sym = ')' if flow > 0.9 else '{'
            else:
                sym = '{' if flow > 1.1 else '|'
            if unit('foam', kind, u // 2, yy // 2) < (0.04 if kind == 'rapids' else 0.004):
                sym = '}'
            if kind == 'rapids' and (u + 2 * yy) % 32 < 2 and yy < 20:
                sym = '_' if dither(x, y, 0.45) else '}'
            c[(x, y)] = sym
    if kind == 'rock':
        ellipse(c, 32, 6, 16, 9, lambda dx, dy: None if dy > 3 else ';' if dx < -6 and dy < 0 else ':' if dx < 6 else '~')
        for x in range(14, 51):
            if dither(x, 7, 0.6):
                c[(x, 7)] = '_'
            c[(x, 8)] = '}' if (x + frame) % 3 else '`'
    return to_image(c)


def pool_frame(frame, kind='calm'):
    """The ravine's pool, frame `frame` of 4: a lit skin, ripples catching the
    moon near the top, blue going deep, stones and weed on its bed; foam
    drifting where the falls churn it, a foam line where it meets the west
    wall."""
    phase = math.tau * frame / 4
    c = {}
    for y in range(TILE):
        for x in range(TILE):
            ripple = math.sin(math.tau * x / 32 + phase + y * 0.9) + 0.55 * math.sin(math.tau * x / 16 + y * 1.7 + 2 * phase)
            if y == 0:
                sym = '}' if dither(x, y, 0.5) else ')'
            elif y == 1:
                sym = ')' if dither(x, y, 0.55) else '{'
            elif y < 14:
                sym = ')' if ripple > 1.2 and y < 9 else '{' if dither(x, y, 1 - (y - 2) / 13) or ripple > 1.25 else '|'
            elif y < 40:
                sym = '{' if ripple > 1.4 else '|'
            else:
                sym = '|' if dither(x, y, 1 - (y - 40) / 28) else 'O'
            c[(x, y)] = sym
    for k in range(2):
        cx, cy = 10 + int(unit('pool-stone', kind, k) * 44), 57 + int(unit('pool-stone-y', kind, k) * 5)
        ellipse(c, cx, cy, 6 + 3 * k, 3, lambda dx, dy: 'd' if dy < -1 else 'n')
    for k in range(3):
        x = 4 + int(unit('weed', kind, k) * 56)
        for d in range(8 + int(unit('weed-l', kind, k) * 16)):
            c[(x + round(math.sin(d * 0.4 + k + phase) * 1.3), TILE - 1 - d)] = 'P' if d % 3 else 'D'
    if kind == 'foam':
        for k in range(16):
            fx, fy = int(unit('pool-foam', k) * 60) + 16 * frame, int(unit('pool-foam-y', k) ** 2 * 7)
            for d in range(2 + int(unit('pool-foam-l', k) * 10)):
                c[((fx + d) % TILE, fy)] = '_' if d % 3 else '`'
                if d % 2 == 0:
                    c[((fx + d) % TILE, fy + 1)] = '}'
    elif kind == 'edge_left':
        for x in range(0, 5):
            for y in range(3):
                if dither(x + frame, y, 0.8 - y * 0.2):
                    c[(x, y)] = '_' if (x + y + frame) % 3 else '`'
    return to_image(c)


# The stones damming the pool's east end, back to front: middle, width and
# height, each standing on the cave floor at the foot of the shore's canvas
# (3 x 2 tiles, the water's surface on its second row's top).
SHORE_STONES = ((158, 46, 50), (182, 28, 18), (122, 66, 82), (95, 32, 20))
# The falls' wet stone, dark to light.
WET_RAMP = ('n', 'd', 'η', ':', ';')


def wet_stone(size, middle, width, height, seed):
    """A rounded stone of the falls' wet rock standing on the foot of a
    canvas of `size`, lit from the moon at the upper left, cracked, mossed
    along its top."""
    w, h = size
    pts = []
    for k in range(13):
        a = math.pi * k / 12
        r = 0.8 + 0.2 * unit('shore-stone', seed, k)
        pts.append((middle + math.cos(a) * width / 2 * r, h - 1 - math.sin(a) * height * r))
    body = Part(size)
    body.polygon(pts)
    inside = body.pixels()
    crown = min(y for _, y in inside)
    c = {}
    for x, y in inside:
        u = (x - middle) / (width / 2)
        v = (y - crown) / max(1, h - crown)
        light = 0.62 - 0.42 * u - 0.75 * v + 0.12 * (unit('shore-grain', seed, x // 2, y // 2) - 0.5)
        level = max(0.0, min(len(WET_RAMP) - 1.001, light * (len(WET_RAMP) - 1)))
        k = int(level) + (1 if dither(x, y, level - int(level)) else 0)
        c[(x, y)] = WET_RAMP[min(k, len(WET_RAMP) - 1)]
        if (x, y - 1) not in inside:
            c[(x, y)] = ';' if u < 0.45 else ':'
    for k in range(2 if width > 40 else 1):
        x, y = int(middle + (unit('shore-crack', seed, k) - 0.5) * width * 0.5), crown + 6 + 4 * k
        for d in range(int(height * 0.45)):
            x += 1 if unit('shore-cx', seed, k, d) > 0.6 else -1 if unit('shore-cx', seed, k, d) < 0.3 else 0
            if (x, y + d) in c:
                c[(x, y + d)] = 'o'
    for x, y in inside:
        rise = next((d for d in range(1, 6) if (x, y - d) not in inside), 9)
        if rise <= 3 and unit('shore-moss', seed, x // 4) < 0.7 and unit('shore-moss-y', seed, x, y) < 1.05 - 0.25 * rise:
            c[(x, y)] = 'G' if rise == 1 and unit('shore-moss-l', seed, x) < 0.2 else 'g' if rise == 1 else 'Q' if rise < 3 else 'T'
    return c


def pool_shore(frame):
    """The ravine's pool running out against fallen stones at its east end
    (M79), frame `frame` of 4, 3 x 2 tiles: the calm water over a bed
    shelving up to them, foam lapping them at the waterline, and the stones
    standing out of it in the falls' wet rock, mossed, with rubble at their
    far foot down to the cave floor."""
    w, h, top = 3 * TILE, 2 * TILE, TILE
    calm = from_image(pool_frame(frame, 'calm'))

    def bed(x):
        t = min(1.0, max(0.0, (x - 16) / 80))
        return h - 26 * t * t * (3 - 2 * t)

    c = {}
    for x in range(112):
        for y in range(top, h):
            if y < bed(x):
                s = calm.get((x % TILE, y - top))
                if s:
                    c[(x, y)] = s
            else:
                c[(x, y)] = '|' if dither(x, y, 0.45) or unit('shore-bed', x // 3, y // 2) < 0.25 else 'O'
    for k in range(10):
        cx = 22 + int(unit('shore-pebble', k) * 76)
        cy = round(bed(cx))
        for dx in range(-3, 4):
            for dy in range(-1, 2):
                if abs(dx) + abs(dy) < 4 and (cx + dx, cy + dy) in c:
                    c[(cx + dx, cy + dy)] = '{' if dy < 0 else '|'
    stones = compose([(wet_stone((w, h), m, sw, sh, k), 'o') for k, (m, sw, sh) in enumerate(SHORE_STONES)])
    for k in range(16):
        cx = 134 + int(unit('shore-rubble', k) * 54)
        cy = h - 2 - int(unit('shore-rubble-y', k) * 3)
        rx = 2 + int(unit('shore-rubble-r', k) * 3)
        for dx in range(-rx, rx + 1):
            for dy in (-1, 0, 1):
                p = (cx + dx, cy + dy)
                if 0 <= p[0] < w and p[1] < h and p not in stones:
                    c[p] = ';' if dy == -1 and abs(dx) < rx else ':' if dy == 0 else '~'
    c.update(stones)
    # The water's mark on the stones, and the foam lapping them.
    for (x, y), s in stones.items():
        if top - 1 <= y <= top + 2 and x < 130 and s != 'o':
            c[(x, y)] = 'd' if (x + y + frame) % 4 else ','
    edge = min(x for (x, y) in stones if top <= y <= top + 2)
    for x in range(edge - 24, edge + 2):
        for y in range(top, top + 4):
            if (x, y) not in stones and dither(x + frame * 2, y, (0.95 - (y - top) * 0.24) * max(0.0, 1 - (edge - x) / 24)):
                c[(x, y)] = '_' if unit('shore-lap', x, y, frame) < 0.6 else '`'
    return to_image(c, (w, h))


# The aqueduct's pool (e01hollow): its canvas, 3 x 1 tiles, its water's
# ends and its depth at the middle, front to back, in pixels.
BASIN = (3 * TILE, TILE)
BASIN_ENDS, BASIN_DEPTH = (10, 182), 10


def basin_top(x):
    """The far edge of the pool's water at column `x`, or None outside it:
    a flattened half ellipse standing on the canvas's foot."""
    left, right = BASIN_ENDS
    u = (x - (left + right) / 2) / ((right - left) / 2)
    if abs(u) >= 1:
        return None
    return BASIN[1] - 2 - round(BASIN_DEPTH * math.sqrt(1 - u * u) + 1.5 * math.sin(x * 0.11))


def pool_basin(frame):
    """A small pool on the aqueduct's floor (e01hollow), frame `frame` of
    4, 3 x 1 tiles: the still water of a sunken flag's hollow seen from a
    little above, lying behind the floor's turf, deep blue with the moon's
    glints drifting on it and ripples spreading from the reeds; a muddy far
    bank with pebbles and moss; reeds in clumps at both ends and a few
    stems out of the water, their reflections dark under them; lily pads,
    one flowering."""
    w, h = BASIN
    phase = math.tau * frame / 4
    c = {}
    for x in range(w):
        t = basin_top(x)
        if t is None:
            continue
        for y in range(t, h - 1):
            depth = (y - t) / max(1, h - 2 - t)
            ripple = math.sin(x * 0.35 + phase + y * 1.3) + 0.6 * math.sin(x * 0.13 - 2 * phase)
            if y == t:
                sym = '}' if dither(x, y, 0.6) else ')'
            elif ripple > 1.25 and depth < 0.6:
                sym = ')'
            else:
                sym = '{' if dither(x, y, 0.55 - 0.4 * depth) or ripple > 1.1 else '|'
            c[(x, y)] = sym
        c[(x, h - 1)] = 'O' if dither(x, h - 1, 0.6) else 'n'
        # The far bank: wet mud, pebbles, moss over its lip.
        for y in range(t - 3, t):
            r = unit('bank', x // 2, y)
            c[(x, y)] = ('T' if r < 0.3 else 'g' if r < 0.38 else 'd') if y == t - 3 else 'n' if r < 0.55 else 'd'
        if unit('pebble', x // 3) < 0.25:
            c[(x, t - 2)], c[(x, t - 3)] = ';', ':'
    # The moon's glints, drifting a pixel a frame, and the ripple rings.
    for k in range(9):
        gx = int(unit('glint', k) * (BASIN_ENDS[1] - BASIN_ENDS[0] - 20)) + BASIN_ENDS[0] + 10 + frame * (1 + k % 2)
        t = basin_top(gx)
        if t is None:
            continue
        gy = t + 2 + int(unit('glinty', k) * max(1, h - 5 - t))
        for d in range(1 + k % 3):
            if (gx + d, gy) in c:
                c[(gx + d, gy)] = '`' if d == 0 and (frame + k) % 3 else '_'
    for cx0, k in ((52, 0), (130, 1)):
        r = 3 + ((frame + 2 * k) % 4) * 3
        cy = h - 6
        for a in range(0, 360, 12):
            q = (round(cx0 + math.cos(math.radians(a)) * r), round(cy + math.sin(math.radians(a)) * r * 0.25))
            if q in c and c[q] in ('{', '|', ')') and dither(*q, 0.9 - r / 16):
                c[q] = '}'
    # Lily pads, one flowering.
    for px, pw, flower in ((70, 7, False), (84, 5, True), (112, 6, False), (145, 4, False)):
        t = basin_top(px)
        py = t + 3 + (px % 3)
        for dx in range(-pw, pw + 1):
            for dy in (-1, 0):
                if abs(dx) < pw - (1 if dy else 0) and (px + dx, py + dy) in c and not (dx in (1, 2) and dy == 0):
                    c[(px + dx, py + dy)] = 'W' if dy < 0 and dx > 0 else 'U' if dy < 0 else 'Q'
        if flower:
            c[(px - 1, py - 2)], c[(px, py - 2)], c[(px + 1, py - 2)], c[(px, py - 3)] = 'w', 'I', 'w', 'I'
    # Reeds: clumps at both ends, a few stems out of the water, their
    # reflections dark in it.
    stems, heads, blades = Part(BASIN), Part(BASIN), Part(BASIN)
    for k, (x, base) in enumerate([(14 + 3 * j + int(unit('rl', j) * 3), 60) for j in range(6)]
                                  + [(162 + 3 * j + int(unit('rr', j) * 3), 60) for j in range(6)]
                                  + [(44, 58), (101, 57), (122, 58)]):
        top = 6 + int(unit('rh', k) * 22)
        lean = (unit('rlean', k) - 0.5) * 10 + math.sin(phase + k) * 0.6
        stems.limb([(x, base), (x + lean * 0.4, (base + top) / 2), (x + lean, top)], 2)
        if unit('rcat', k) < 0.55:
            heads.limb([(x + lean, top - 6), (x + lean, top)], 3)
        if unit('rblade', k) < 0.6:
            d = 1 if k % 2 else -1
            blades.limb([(x, base), (x + 5 * d, base - 10), (x + 9 * d + lean * 0.3, base - 17)], 2)
        for y in range(base + 1, h - 1):
            q = (round(x + lean * 0.1), y)
            if q in c and dither(q[0], y, 0.7):
                c[q] = 'O'
    reeds_c = compose([
        (shade(blades.pixels(), 'U', light='W', shadow='P'), None),
        (shade(stems.pixels(), 'T', light='g', shadow='t', rim='G'), None),
        (shade(heads.pixels(), 'ω', light='"', shadow='/', rim="'"), None),
    ])
    outline(reeds_c, 'o', size=BASIN)
    c.update(reeds_c)
    return to_image(c, BASIN)


def lava_frame(frame, part='surface', i=0):
    """Molten rock, frame `frame` of 4: a glowing skin, then meandering veins
    of yellow through orange that cools to red with depth; dark crust rafts
    drifting 16 px a frame on the skin with glowing seams; a bubble or two
    ringed in yellow. The yellows emit, as the ember cracks' do. `surface`
    lies 6 px under its tile's top; `deep` fills its tile and carries on
    from it."""
    phase = math.tau * frame / 4
    c = {}
    top = 6 if part == 'surface' else 6 - TILE
    for y in range(max(0, top), TILE):
        for x in range(TILE):
            depth = y - top
            flow = math.sin(y * 0.26 + 1.4 * math.sin(math.tau * x / TILE * 2 + phase)
                            + 0.7 * math.sin(math.tau * x / TILE * 3 - y * 0.13 + 2 * phase) + phase)
            vein = abs(flow) < 0.14 and dither(x, y, 0.85 - depth / 200)
            if depth <= 1:
                sym = 'Y' if depth == 0 and dither(x, y, 0.45) else 'y'
            elif depth < 5:
                sym = 'y' if vein or dither(x, y, 0.35) else 'J'
            elif depth < 20:
                sym = ('y' if depth < 10 else 'J') if vein else 'J' if dither(x, y, 1 - (depth - 5) / 18) else 'K'
            elif depth < 44:
                sym = 'J' if vein else 'K' if dither(x, y, 1 - (depth - 20) / 30) else 'L'
            else:
                sym = 'K' if vein else 'L' if dither(x, y, 1 - (depth - 44) / 34) else 'M'
            c[(x, y)] = sym
    if part == 'surface':
        for k in range(2 + i % 2):
            cx, w = int(unit('raft', i, k) * 50) + 16 * frame, 10 + int(unit('raft-w', i, k) * 14)
            for x in range(cx, cx + w):
                h = 1 + round(2.5 * math.sin(math.pi * (x - cx) / w) * (0.7 + 0.3 * unit('raft-h', i, k, (x - cx) // 3)))
                for y in range(top, top + 1 + h):
                    edge = x in (cx, cx + w - 1) or y == top + h
                    c[(x % TILE, y)] = 'K' if edge else ('J' if unit('raft-seam', i, k, (x - cx) // 4) < 0.12 else 'M' if (x + y) % 4 else 'Θ')
        bx, by, r = 8 + int(unit('bubble', i) * 48), top + 8 + int(unit('bubble-y', i) * 8), 1 + frame % 3
        ellipse(c, bx, by, r, r, lambda dx, dy, r=r: 'Y' if dx * dx + dy * dy >= (r - 1) ** 2 else 'K')
    return to_image(c)


def lava_crust(stage, part='top', i=0):
    """The ice spray's crust over lava (M79), stage `stage` of 4: cooled
    basalt plates split by jagged seams under a lit rim, over cooled rock
    that hides the melt (`top`, the surface row), or that rock alone
    (`deep`). Stage 0 has just set, its seams dark; stages 1 to 3 warm the
    seams and open cracks to a glow, the last before it melts. The yellows
    emit, as the lava's do."""
    c = {}
    seam_sym, seam_hot = ('θ', 'M', 'L', 'K')[stage], (None, 'L', 'J', 'y')[stage]
    vein_sym = ('M', 'M', 'L', 'K')[stage]
    top = 3 if part == 'top' else -1
    lip = {}
    for x in range(TILE):
        lip[x] = 24 + round(2.0 * math.sin(math.tau * x / TILE * 2 + i * 1.7) + 1.2 * math.sin(x * 0.61 + i))
        rise = top + round(0.7 * math.sin(x * 0.37 + i) + 0.6 * math.sin(x * 0.11 + 2 * i)) if part == 'top' else 0
        for y in range(max(0, rise), TILE):
            n = unit('crust', part, i, x // 2, y // 2)
            if part == 'top' and y == rise:
                sym = 'α' if n < 0.75 else 'ξ'
            elif part == 'top' and y == rise + 1:
                sym = 'ξ' if n < 0.6 else 'β'
            elif part == 'top' and y < lip[x]:
                v = (y - rise) / max(1, lip[x] - rise)
                sym = ('β' if n < 0.55 else 'γ') if v < 0.55 else ('γ' if n < 0.5 else 'ν')
            elif part == 'top' and y == lip[x]:
                sym = 'θ'
            else:
                # The cooled melt under the plates: dark rock, a dull red where
                # it is warmest.
                sym = 'ν' if n < 0.55 else 'ζ' if n < 0.85 else 'ε'
                if dither(x, y, 0.06 + 0.04 * stage) and unit('warm', part, i, x // 3, y // 3) < 0.35:
                    sym = 'M'
            c[(x, y)] = sym
    if part == 'top':
        x = int(unit('seam0', i) * 8) + 4
        seams = []
        while x < TILE:
            seams.append(x)
            x += 14 + int(unit('seamw', i, x) * 9)
        for s0 in seams:
            x = s0
            for y in range(3, 28):
                r = unit('seam', i, s0, y)
                x += -1 if r < 0.28 else 1 if r > 0.72 else 0
                xx = x % TILE
                if (xx, y) in c and y < lip[xx]:
                    hot = seam_hot and unit('seamh', i, s0, y) < 0.18 + 0.15 * stage
                    c[(xx, y)] = seam_hot if hot else seam_sym
        for k in range(stage * 3):
            x, y = int(unit('ckx', i, k) * 60) + 2, 6 + int(unit('cky', i, k) * 10)
            for d in range(4 + stage * 3):
                r = unit('ckd', i, k, d)
                x += 1 if r > 0.2 else 0
                y += -1 if r < 0.25 else 1 if r > 0.8 else 0
                xx = x % TILE
                if (xx, y) in c and 5 <= y < lip[xx] - 1:
                    c[(xx, y)] = seam_hot if unit('ckh', i, k, d) < 0.3 + 0.15 * stage else seam_sym
    # Veins of the melt through the cooled rock, warming with the stage.
    start = 30 if part == 'top' else 0
    for k in range(2 + stage):
        x0, y0 = int(unit('vein', part, i, k) * 56) + 4, start + int(unit('veiny', part, i, k) * 20)
        for d in range(12 + 6 * stage):
            r = unit('vd', part, i, k, d)
            x0 += -1 if r < 0.3 else 1 if r > 0.7 else 0
            y0 += 1
            if 0 <= y0 < TILE:
                hot = seam_hot and unit('vh', part, i, k, d) < 0.1 + 0.12 * stage
                c[(x0 % TILE, y0)] = seam_hot if hot else vein_sym
    return to_image(c)


# The crust's look (01 (k)): its tables by crack stage, read at a
# fractional stage.
SEAM = ('θ', 'M', 'L', 'K')
SEAM_HOT = (None, 'L', 'J', 'y')
VEIN = ('M', 'M', 'L', 'K')
# The px between a row's partial frames as a front comes down it.
FRONT_STEP = 8


def value_noise(key, x, y, cell):
    """Smooth value noise in [0, 1): a grid of `cell` px, bilinear between
    its corners, wrapping every tile across so tiles join."""
    gx, gy = x / cell, y / cell
    x0, y0 = math.floor(gx), math.floor(gy)
    fx, fy = gx - x0, gy - y0
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    n = TILE // cell

    def v(a, b):
        return unit(key, a % n, b % n)
    top = v(x0, y0) * (1 - fx) + v(x0 + 1, y0) * fx
    bottom = v(x0, y0 + 1) * (1 - fx) + v(x0 + 1, y0 + 1) * fx
    return top * (1 - fy) + bottom * fy


def blobs(key, x, y):
    return 0.6 * value_noise(key, x, y, 16) + 0.4 * value_noise(key + '2', x, y, 8)


def wavy(key, x, base, amp=4.0):
    """A wavy front's row at column `x`, repeating every tile across so
    tiles join: `base` plus two sines of the tile's period."""
    a, b = unit(key, 'a') * math.tau, unit(key, 'b') * math.tau
    return base + amp * (0.65 * math.sin(math.tau * x / TILE + a) + 0.35 * math.sin(math.tau * 2 * x / TILE + b))


def crust_at(stage, part, i, hot_seams=None, rime=0.0, warm=0.0, veins=None):
    """`lava_crust` at a fractional crack stage: the tables of
    `floor(stage)`, the cracks' length and heat by the fraction;
    `hot_seams` forces the seams' symbols (the set's bright seams), `rime`
    frosts the plates' tops, `warm` dithers the cooled melt warm and `veins`
    forces the veins' symbols."""
    s = int(stage)
    f = stage - s
    c = {}
    seam_sym, seam_hot = (SEAM[s], SEAM_HOT[s]) if not hot_seams else hot_seams
    vein_sym, vein_hot = (VEIN[s], seam_hot) if not veins else veins
    lip = {x: 24 + round(2.0 * math.sin(math.tau * x / TILE * 2 + i * 1.7) + 1.2 * math.sin(x * 0.61 + i))
           for x in range(TILE)}
    for x in range(TILE):
        rise = 3 + round(0.7 * math.sin(x * 0.37 + i) + 0.6 * math.sin(x * 0.11 + 2 * i)) if part == 'top' else 0
        for y in range(max(0, rise), TILE):
            n = unit('crust', part, i, x // 2, y // 2)
            if part == 'top' and y == rise:
                sym = 'α' if n < 0.75 else 'ξ'
            elif part == 'top' and y == rise + 1:
                sym = 'ξ' if n < 0.6 else 'β'
            elif part == 'top' and y < lip[x]:
                v = (y - rise) / max(1, lip[x] - rise)
                sym = ('β' if n < 0.55 else 'γ') if v < 0.55 else ('γ' if n < 0.5 else 'ν')
            elif part == 'top' and y == lip[x]:
                sym = 'θ'
            else:
                sym = 'ν' if n < 0.55 else 'ζ' if n < 0.85 else 'ε'
                if dither(x, y, 0.06 + 0.04 * stage) and unit('warm', part, i, x // 3, y // 3) < 0.35:
                    sym = 'M'
                if warm and blobs('setwarm%s%d' % (part, i), x, y) < warm * (1.1 - (y / TILE if part == 'top' else 0.4)):
                    sym = 'M' if dither(x, y, 0.6) else sym
            c[(x, y)] = sym
    if part == 'top':
        x = int(unit('seam0', i) * 8) + 4
        seams = []
        while x < TILE:
            seams.append(x)
            x += 14 + int(unit('seamw', i, x) * 9)
        for s0 in seams:
            x = s0
            for y in range(3, 28):
                r = unit('seam', i, s0, y)
                x += -1 if r < 0.28 else 1 if r > 0.72 else 0
                xx = x % TILE
                if (xx, y) in c and y < lip[xx]:
                    hot = seam_hot and unit('seamh', i, s0, y) < 0.18 + 0.15 * stage
                    c[(xx, y)] = seam_hot if hot else seam_sym
        cracks = round(stage * 3)
        for k in range(cracks):
            x, y = int(unit('ckx', i, k) * 60) + 2, 6 + int(unit('cky', i, k) * 10)
            length = 4 + round(stage * 3) + (round(f * 4) if k >= cracks - 3 else 0)
            for d in range(length):
                r = unit('ckd', i, k, d)
                x += 1 if r > 0.2 else 0
                y += -1 if r < 0.25 else 1 if r > 0.8 else 0
                xx = x % TILE
                if (xx, y) in c and 5 <= y < lip[xx] - 1 and seam_hot:
                    c[(xx, y)] = seam_hot if unit('ckh', i, k, d) < 0.3 + 0.15 * stage else seam_sym
        if stage >= 2.5:
            # The plates' edges lift: a glowing rim under a lifted edge.
            for s0 in seams:
                for y in range(4, 22):
                    xx = (s0 + round(math.sin(y * 0.5 + i) * 1.5)) % TILE
                    if (xx + 1, y) in c and unit('lift', i, s0, y) < 0.25 * (stage - 2):
                        c[((xx + 1) % TILE, y)] = 'J'
        if rime:
            for (x, y), sym in list(c.items()):
                if sym in ('α', 'ξ') and dither(x, y, rime):
                    c[(x, y)] = 'π' if unit('rime2', i, x, y) < 0.55 else '`'
                elif sym == 'β' and y <= 8 and dither(x, y, rime * 0.4):
                    c[(x, y)] = 'ξ'
    start = 30 if part == 'top' else 0
    for k in range(2 + s):
        x0, y0 = int(unit('vein', part, i, k) * 56) + 4, start + int(unit('veiny', part, i, k) * 20)
        for d in range(12 + round(6 * stage)):
            r = unit('vd', part, i, k, d)
            x0 += -1 if r < 0.3 else 1 if r > 0.7 else 0
            y0 += 1
            if 0 <= y0 < TILE:
                hot = vein_hot and unit('vh', part, i, k, d) < 0.1 + 0.12 * max(stage, 1.5 if veins else 0)
                c[(x0 % TILE, y0)] = vein_hot if hot else vein_sym
    return to_image(c)


def lava_crust_frame(n, part, i):
    """The crust's frame `n` of 9: 0 just set (bright seams, rime on the
    plates), 1 dulling, 2 the hold (`lava_crust`'s stage 0), 3 to 8 two
    for each of the three crack stages, its cracks lengthening and warming
    to yellow, the plates' edges lifting."""
    if n == 0:
        return crust_at(0, part, i, hot_seams=('K', 'J'), rime=0.75, veins=('K', 'J'))
    if n == 1:
        return crust_at(0, part, i, hot_seams=('L', 'K'), rime=0.4, veins=('L', 'K'))
    if n == 2:
        return lava_crust(0, part, i)
    return crust_at(1 + (n - 3) / 2, part, i)


def front_row(x, j, key):
    """A front's row at column `x` for partial `j` (0 to 7): `j` steps of
    `FRONT_STEP` px down, waving with the tile's period so columns join, the
    wave easing to nothing at a row's edges so rows join too."""
    amp = 3.0 * math.sin(math.pi * min(j, TILE // FRONT_STEP - j) / (TILE // FRONT_STEP))
    return wavy(key, x, j * FRONT_STEP + (1 if j == 0 else 0), amp)


def lava_freeze(j, part, i):
    """A row the freeze's front has reached `j` steps down, over the molten
    clip: the fresh crust's pixels above the front (the set's bright seams
    and rime), a cooling edge that glows at the front and a band of
    darkening melt just under it; clear below, where the melt still
    shows."""
    from art import PALETTE
    back = {tuple(v): s for s, v in PALETTE.items()}
    crust = lava_crust_frame(0, part, i).load()
    c = {}
    for x in range(TILE):
        edge = front_row(x, j, 'freeze-front')
        for y in range(TILE):
            if y < edge - 1:
                if crust[x, y][3]:
                    c[(x, y)] = back.get(tuple(crust[x, y]), 'β')
            elif y < edge + 1:
                if crust[x, min(TILE - 1, y)][3]:
                    c[(x, y)] = 'K' if dither(x, y, 0.6) else 'L'
            elif y < edge + 5:
                if dither(x, y, 0.75 - (y - edge) * 0.15):
                    c[(x, y)] = 'M' if dither(x, y, 0.5) else 'ϑ'
    return to_image(c)


def lava_melt(j, part, i):
    """A row the melt's front has reached `j` steps down, over the molten
    clip: clear above the front, where the melt is back; at the front a hot
    glowing edge and dark rafts breaking off and sinking; below it the
    crust as it was at its last crack, still hot."""
    from art import PALETTE
    back = {tuple(v): s for s, v in PALETTE.items()}
    crust = lava_crust_frame(8, part, i).load()
    c = {}
    for x in range(TILE):
        edge = front_row(x, j, 'melt-front')
        for y in range(TILE):
            if crust[x, y][3] and y > edge + 1:
                c[(x, y)] = back.get(tuple(crust[x, y]), 'ν')
            elif crust[x, y][3] and edge - 1 <= y <= edge + 1:
                c[(x, y)] = 'y' if dither(x, y, 0.5) else 'J'
    for k in range(5):
        cx = int(unit('raft', part, i, j, k) * 60) + 2
        w = 5 + int(unit('raftw', part, i, k) * 7)
        lift = 3 + int(unit('rafth', part, i, j, k) * 5)
        for x in range(cx, cx + w):
            xx = x % TILE
            top = round(front_row(xx, j, 'melt-front')) - lift
            for y in range(top, top + 2 + (x - cx) % 2):
                if 0 <= y < TILE:
                    c[(xx, y)] = 'K' if x in (cx, cx + w - 1) else ('γ' if y == top else 'ν')
    return to_image(c)


def pour_wide(frame):
    """A thin fall of water from a broken end, a spout or the gargoyle
    (01 (k)), frame `frame` of 4: a 10 px column, its core of streaks moving
    16 px a frame between translucent edges; segments stack every 64 px."""
    c = {}
    for y in range(TILE):
        sway = round(math.sin(math.tau * y / TILE) * 1.2)
        x0 = TILE // 2 - 5 + sway
        for x in range(x0 - 1, x0 + 11):
            v = (y - 16 * frame) % TILE
            if x in (x0 - 1, x0 + 10):
                if dither(x, y + frame * 4, 0.45):
                    c[(x, y)] = 'ϰ'
                continue
            if x in (x0, x0 + 9):
                c[(x, y)] = 'ς' if dither(x, y, 0.7) else '}'
                continue
            c[(x, y)] = '`' if (v + x * 7) % 32 < 2 else '_' if (v + x * 5) % 29 == 0 else '}' if (v + x) % 4 else ')'
    return to_image(c)


def pour_foot(frame):
    """A pour's last segment over the floor, frame `frame` of 4: the stream
    to its foot, a crown of drops thrown up and out, foam and a shallow pool
    on the floor ringing out to its sides."""
    c = from_image(pour_wide(frame))
    for (x, y) in list(c):
        if y > 52:
            del c[(x, y)]
    for x in range(4, 60):
        d = abs(x - 31.5)
        depth = 3 if d < 20 else 2 if d < 26 else 1
        for y in range(TILE - depth, TILE):
            ring = (d - 3 * frame) % 12 < 2
            c[(x, y)] = '`' if ring and y == TILE - depth else '_' if y == TILE - depth else '}' if dither(x, y, 0.6) else ')'
    for x in range(22, 42):
        for y in range(53, TILE - 2):
            if dither(x + frame, y, 0.75 - abs(x - 31.5) / 16):
                c[(x, y)] = '`' if unit('foam', frame, x, y) < 0.5 else '_'
    for k in range(10):
        a = (k / 10) * math.pi
        speed = 0.6 + unit('crown', k) * 0.6
        t = ((frame + unit('ph', k) * 4) % 4) / 4
        x = 31.5 + math.cos(a) * 20 * speed * t
        y = 56 - math.sin(a) * 18 * speed * t * (1 - t) * 4 / 2
        for dx, dy in ((0, 0), (1, 0)) if t < 0.6 else ((0, 0),):
            q = (round(x) + dx, round(y) + dy)
            if 0 <= q[1] < TILE:
                c[q] = '`' if t < 0.4 else '_'
    return to_image(c)


def spill(frame, side, k):
    """The trough's water over a broken end's lip (`terrain.BREAK_LIPS`),
    frame `frame` of 4: a sheet bulging out past the lip with foam and
    spray, falling into a column 4.5 px past it, the wide pour's width."""
    sign = 1 if side == 'right' else -1
    lip = BREAK_LIPS[side][k]
    col = lip + sign * 4.5
    c = {}
    for y in range(2, TILE):
        t = min(1.0, (y - 2) / 14)
        centre = lip + sign * (1.5 + 3 * math.sin(t * math.pi * 0.5))
        centre = centre * (1 - t) + col * t
        half = 5.5 - 0.5 * t + 0.6 * math.sin(y * 0.45 + frame * 1.6)
        for x in range(round(centre - half) - 2, round(centre + half) + 3):
            if sign * (x - lip) < -1 and y < 12:
                continue
            d = abs(x - centre)
            if d > half + 0.5:
                if d < half + 2 and dither(x, y + frame * 4, 0.3):
                    c[(x, y)] = 'ϰ'
                continue
            v = (y - 16 * frame + x * 3) % 32
            if y < 6 and d < half - 1:
                sym = '`' if (x + frame) % 3 else '_'
            elif d > half - 1:
                sym = 'ς' if dither(x, y, 0.6) else '}'
            elif v < 2:
                sym = '`'
            elif (v + x) % 7 == 0:
                sym = '_'
            else:
                sym = '}' if (x + y) % 3 else ')'
            c[(x, y)] = sym
    for n in range(6):
        t = ((frame + unit('sp2', side, k, n) * 4) % 4) / 4
        x = lip + sign * (3 + 12 * t * (0.6 + unit('sp2x', k, n) * 0.6))
        y = 4 - 5 * t + 16 * t * t
        if 0 <= round(y) < TILE:
            c[(round(x), round(y))] = '`' if t < 0.5 else 'ϰ'
    return to_image(c)


def channel_drips(frame):
    """Lime drips under the channel's coping, frame `frame` of 6: three
    stubby stalactites, and a drop that swells at the middle one, falls and
    thins to a streak."""
    c = {}
    for k, x in enumerate((14, 31, 50)):
        n = 3 + int(unit('stal', k) * 5)
        for y in range(0, n):
            for dx in range(-(n - y) // 3, (n - y) // 3 + 1):
                c[(x + dx, y)] = 'w' if dx < 0 else 'σ' if dx == 0 else 'τ'
    x = 31
    if frame < 2:
        for dy in range(frame + 2):
            c[(x, 7 + dy)] = '`' if dy == 0 else '_'
        c[(x - 1, 8)] = '}'
    else:
        y = 8 + (frame - 1) * 11
        for d in range(4):
            if y - d < TILE:
                c[(x, y - d)] = '`' if d == 0 else '_' if d == 1 else 'ϰ'
    return to_image(c)


def fallen_voussoir(k):
    """A fallen arch stone, `k` of 3: a wedge on its side or end, chipped,
    its face moss-flecked; its foot on the tile's floor."""
    p = Part()
    ww, hh = ((26, 18), (30, 14), (20, 22))[k]
    base = TILE - 1
    slope = (6, 4, 8)[k]
    p.polygon([(32 - ww // 2, base), (32 + ww // 2, base), (32 + ww // 2 - slope, base - hh),
               (32 - ww // 2 + slope // 2, base - hh)])
    pts = p.pixels()
    c = {}
    for (x, y) in pts:
        if (x, y - 1) not in pts:
            c[(x, y)] = ';'
        elif (x, y + 1) not in pts:
            c[(x, y)] = '<'
        elif (x - 1, y) not in pts:
            c[(x, y)] = '-'
        else:
            c[(x, y)] = coursed(x * 3, y * 3, ('fv', k), course=40, block=60)
            if unit('fvmoss', k, x // 3, y // 2) < 0.12:
                c[(x, y)] = 'T'
    for d in range(4):
        c.pop((32 + ww // 2 - slope - d, base - hh + d // 2), None)
    outline(c, 'o')
    return to_image(c)


def lavafall_frame(frame):
    """A fall of lava from a crack, frame `frame` of 4 (e01hollow): a stream
    23 px wide, yellow-hot at its core in streaks that run down 16 px a
    frame, orange through its body, a crust darkening its edges in plates
    that slide down with it, glowing where they part; segments stack every
    64 px."""
    c = {}
    cx, half = TILE // 2, 11.5
    for y in range(TILE):
        # Its pattern repeats every 64 px, so stacked segments join.
        flow = (y - 16 * frame) % TILE
        x0 = cx + 1.2 * math.sin(math.tau * y / TILE)
        width = half + 0.8 * math.sin(math.tau * 2 * y / TILE + 1.0)
        for x in range(round(x0 - width - 1), round(x0 + width + 2)):
            r = abs(x + 0.5 - x0) / width
            if r > 1.08:
                continue
            streak = math.sin(math.tau * (x - x0) / 7 + math.tau * flow / TILE * 3) * 0.5 + 0.5
            # The crust: plates on each edge, a plate every 16 px of flow.
            plate = (flow + (0 if x < x0 else 8)) % 16
            crust = r > 0.62 - 0.12 * math.sin(math.tau * flow / 16 + (0 if x < x0 else 2))
            if r > 0.98:
                c[(x, y)] = 'θ' if plate else 'L'
            elif crust:
                # Plates of crust, the hot flow glowing in the seams between.
                c[(x, y)] = 'Ъ' if plate in (0, 1) else ('ν' if r > 0.85 else 'X' if plate % 5 == 0 else 'M')
            elif r < 0.08 + 0.1 * streak:
                c[(x, y)] = 'Y' if streak > 0.85 else 'y'
            elif r < 0.3:
                c[(x, y)] = 'y' if streak > 0.7 else 'Ъ'
            else:
                c[(x, y)] = 'Ъ' if streak > 0.35 else 'K'
    return to_image(c)


def lava_splash(frame):
    """The lavafall's foot in the basin, frame `frame` of 4 (e01hollow): a
    crown of molten lava thrown up where the stream meets the surface, a
    glowing ring spreading on it, spatter flung out and falling back,
    dark crust bobbing; its bottom row on the lava's surface."""
    c = {}
    base = TILE - 6
    turn = math.tau * frame / 4
    # The ring on the surface, spreading and cooling.
    for x in range(4, TILE - 4):
        d = abs(x + 0.5 - 32) / 28
        for y in range(base - 1, base + 4):
            wave = 0.5 + 0.5 * math.sin(d * 9 - turn * 1.5)
            if d < 1 and (y - base + 1) / 5 < 1 - d * 0.8:
                c[(x, y)] = 'Y' if d < 0.18 else 'y' if d < 0.35 or wave > 0.8 else 'Ъ' if wave > 0.4 else 'K'
    # The crown: tongues of lava thrown up about the stream's foot.
    for k in range(5):
        dx = (k - 2) * 6 + 2 * math.sin(turn + k)
        height = 7 + 6 * (0.5 + 0.5 * math.sin(turn * 1 + k * 1.9))
        for d in range(round(height)):
            w = max(0, round(2.2 * (1 - d / height)))
            for e in range(-w, w + 1):
                x = round(32 + dx + e + d * (dx / 14))
                c[(x, base - 1 - d)] = 'y' if abs(e) < w * 0.5 or d < 2 else 'Ъ' if d < height * 0.7 else 'L'
    # Spatter: droplets flung out on arcs, cooling as they fall.
    for k in range(10):
        side = -1 if k % 2 else 1
        t = (unit('spatter', k) + frame / 4) % 1.0
        speed = 10 + unit('spv', k) * 14
        x = round(32 + side * (4 + speed * t * 1.4))
        y = round(base - 3 - speed * t * 1.8 + 30 * t * t)
        if 0 <= x < TILE and 0 <= y < base:
            c[(x, y)] = 'Y' if t < 0.3 else 'y' if t < 0.6 else 'K'
            if t < 0.5:
                c[(x - side, y + 1)] = 'Ъ'
    # Crust bobbing on the surface either side.
    for k in range(4):
        x0 = round(10 + k * 13 + 2 * math.sin(turn + k * 2))
        if abs(x0 - 32) < 8:
            continue
        for x in range(x0, x0 + 4 + k % 2):
            c[(x, base - 1)] = 'M'
            c[(x, base)] = 'θ'
    return to_image(c)


# e01hollow's geysers: the vent's frame and its mouth (the contract's (32,
# 20)), the plume's and the sputter's frames, each with its bottom row on
# the floor; the crater's opening about the mouth, the throat's glow low in
# it; cooled basalt dark to light, and the flame's ramp from its dark red
# edge in to its white core.
GEYSER_VENT, GEYSER_MOUTH = (64, 32), (32, 20)
GEYSER_PLUME, GEYSER_SPUTTER = (64, 176), (64, 64)
CRATER, THROAT = (12.5, 4.0), (7.5, 2.2)
VENT_RAMP = 'ενγβαξ'
FLAME = 'XMEzL+KЪJyYZ'
# The vent's cone: a low dome, a shoulder each side, chunks fallen at its
# foot; its long cracks, a start and the steps each takes outward from the
# collar, their first `HOT_STEPS` glowing while it runs.
CONE = (((32, 24.5), 26, 9.5), ((14, 28), 11, 4.5), ((50, 28.5), 11, 4.2), ((32, 19.5), 15.5, 5.5))
FALLEN = (((5, 30), 3.5, 2), ((59, 30.5), 3, 1.6), ((10, 30.5), 2.5, 1.5))
VENT_CRACKS = ((17, 19, ((-1, 0), (-1, 1), (-1, 0), (-1, 1), (-1, 1), (0, 1), (-1, 1))),
               (46, 19, ((1, 0), (1, 1), (1, 0), (1, 1), (0, 1), (1, 1), (1, 1))),
               (22, 16, ((-1, 0), (-1, 0), (-1, 1), (-1, 0))),
               (41, 16, ((1, 0), (1, 1), (1, 0), (1, 0))),
               (12, 25, ((-1, 1), (-1, 0), (-1, 1), (0, 1))),
               (52, 25, ((1, 1), (1, 0), (1, 1))))
HOT_STEPS = 3


def ellipse_pixels(size, cx, cy, rx, ry):
    """The pixels of `size` inside the ellipse about (`cx`, `cy`)."""
    return {(x, y) for y in range(size[1]) for x in range(size[0])
            if ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 <= 1}


def near_lip(x):
    """The crater's near lip at column `x`, in the vent's rows: a flame's
    root stays behind it."""
    u = (x + 0.5 - GEYSER_MOUTH[0]) / CRATER[0]
    return GEYSER_MOUTH[1] + CRATER[1] * math.sqrt(max(0.0, 1 - u * u)) if abs(u) < 1 else GEYSER_MOUTH[1]


def collar_top(x):
    """The top edge of the vent's glazed collar at column `x`."""
    u = (x + 0.5 - GEYSER_MOUTH[0]) / (CRATER[0] + 3.5)
    return math.ceil(GEYSER_MOUTH[1] - 0.5 - (CRATER[1] + 2.3) * math.sqrt(max(0.0, 1 - u * u)))


def geyser_vent(choked=False):
    """A vent of cracked, heat-glazed basalt round a glowing throat (64 x
    32, its bottom row on the floor, its mouth's middle at (32, 20)): a low
    dome of cooled basalt crazed by long cracks, a collar of black glaze
    round the crater, the crater's far wall lit from below by the throat's
    slit of glow, the cracks glowing near the collar; choked, a crust of
    grey-white rime over the crater, frost on the collar, the cracks cold."""
    mx, my = GEYSER_MOUTH
    body = set()
    for (cx, cy), rx, ry in CONE + FALLEN:
        body |= {(x, y) for x, y in ellipse_pixels(GEYSER_VENT, cx, cy, rx, ry) if y < GEYSER_VENT[1]}
    c = grade(body, VENT_RAMP, light=(-0.55, -0.85, 0.6), spread=0.3, fall=0.25)
    for (cx, cy), rx, ry in FALLEN:
        chunk = {(x, y) for x, y in ellipse_pixels(GEYSER_VENT, cx, cy, rx, ry) if y < GEYSER_VENT[1]}
        c.update(grade(chunk, VENT_RAMP, light=(-0.55, -0.85, 0.6), spread=0.3))
        for (x, y) in chunk:
            if (x - 1, y) not in chunk and (x - 1, y) in body:
                c[(x - 1, y)] = 'θ'
    # Ropy wrinkles slanting down the flanks, a cooled flow's skin.
    for (x, y) in body:
        u = x - mx
        ripple = math.sin(y * 1.9 - abs(u) * 0.55 + (0.8 if u < 0 else 2.1)) + 0.35 * math.sin(u * 0.3)
        if ripple > 1.05 and c[(x, y)] in 'γβα' and abs(u) > 6:
            c[(x, y)] = 'ν' if c[(x, y)] == 'γ' else 'γ'
    # The long cracks: a dark seam, the crust's lit edge under it.
    seams = []
    for x0, y0, steps in VENT_CRACKS:
        x, y = x0, y0
        seam = []
        for dx, dy in steps:
            seam.append((x, y))
            x, y = x + dx, y + dy
        for p in seam:
            if p in body:
                c[p] = 'θ'
                below = (p[0], p[1] + 1)
                if below in body and below not in seam and c[below] in 'βγ':
                    c[below] = 'α'
        seams.append(seam)
    # The collar: black glaze round the crater, a hard sheen on its far
    # side where the light catches it.
    collar = ellipse_pixels(GEYSER_VENT, mx, my - 0.5, CRATER[0] + 3.5, CRATER[1] + 2.3)
    crater = ellipse_pixels(GEYSER_VENT, mx, my, *CRATER)
    for (x, y) in collar - crater:
        u = (x + 0.5 - mx) / (CRATER[0] + 3.5)
        c[(x, y)] = '2' if dither(x, y, 0.45 + 0.3 * u) else '0'
        if (x, y - 1) not in collar and u < 0.4:
            c[(x, y)] = 'ш' if u > -0.2 else 'α'
    for x in range(19, 31):
        if (x, collar_top(x)) in c and x % 3:
            c[(x, collar_top(x))] = 'π' if x in (22, 23) else 'δ'
    if choked:
        vent_rime(c, body)
    else:
        vent_throat(c, crater, body, seams)
    return to_image(outline(c, 'o', GEYSER_VENT), GEYSER_VENT)


def vent_throat(c, crater, body, seams):
    """The crater's far wall lit from below in ragged tongues, the throat's
    slit of glow, the near lip's edge catching it and the cracks hot near
    the collar."""
    mx, my = GEYSER_MOUTH
    for (x, y) in crater:
        r = math.hypot((x + 0.5 - mx) / THROAT[0], (y + 0.5 - my - 1.2) / THROAT[1])
        if r < 1:
            c[(x, y)] = 'Y' if r < 0.45 else 'y' if r < 0.78 else 'Ъ'
            continue
        below = (my + 1.2 - (y + 0.5)) / (CRATER[1] + 1.2)
        side = abs(x + 0.5 - mx) / CRATER[0]
        heat = 1.05 - below * 1.15 - side * 0.5 + (fbm(x / 2.5, y / 1.5, 'crater') - 0.5) * 0.55
        c[(x, y)] = 'Ъ' if heat > 0.78 else 'K' if heat > 0.56 else 'L' if heat > 0.36 else 'M' if heat > 0.2 else 'θ'
    for x in range(mx - 9, mx + 10):
        y = math.floor(near_lip(x))
        if (x, y) in c and (x, y) not in crater:
            c[(x, y)] = 'Ъ' if abs(x - mx) < 6 else 'K'
    for seam in seams:
        for d, p in enumerate(seam[:HOT_STEPS]):
            if p in body and p not in crater:
                c[p] = 'Ъ' if d == 0 else 'K' if d == 1 else 'L'


def vent_rime(c, body):
    """A crust of grey-white rime bulging over the crater, lit from the
    upper left, feathered, crazed between its plates, frost along the
    collar's top; the cracks stay cold."""
    mx, my = GEYSER_MOUTH
    lid = ellipse_pixels(GEYSER_VENT, mx, my - 0.6, CRATER[0] + 1.4, CRATER[1] + 1.1)
    for (x, y) in lid:
        u = (x + 0.5 - mx) / (CRATER[0] + 1.4)
        v = (y + 0.5 - my + 0.6) / (CRATER[1] + 1.1)
        lit = 0.72 - 0.42 * v - 0.3 * u - 0.25 * (u * u + v * v) + (fbm(x / 2, y / 1.4, 'rime') - 0.5) * 0.5
        c[(x, y)] = ramp(lit, 'ψχφѴ', x, y, 0.2)
    for x0, y0, steps in ((22, 19, ((1, 0), (1, 1), (1, 0), (1, 0), (1, -1), (1, 0))),
                          (32, 16, ((1, 1), (0, 1), (1, 1), (1, 0))),
                          (37, 21, ((1, 0), (1, -1), (1, 0), (1, 0)))):
        x, y = x0, y0
        for dx, dy in steps:
            if (x, y) in lid:
                c[(x, y)] = 'ψ'
            x, y = x + dx, y + dy
    for p in ((25, 17), (29, 16), (35, 17), (40, 18)):
        if p in lid:
            c[p] = 'Ψ'
    # Feathery rime standing up off the crust's far edge.
    for x in range(mx - 11, mx + 12, 2):
        top = min(y for (px, y) in lid if px == x)
        if unit('feather', x) < 0.7:
            c[(x, top - 1)] = 'χ'
            if unit('feather2', x) < 0.4:
                c[(x + 1, top - 2)] = 'φ'
    for x in range(mx - 15, mx + 16):
        y = collar_top(x)
        if (x, y) in c and (x, y) not in lid and dither(x, y, 0.6):
            c[(x, y)] = 'χ' if dither(x, y, 0.5) else 'ψ'


def flame(c, size, mouth_y, height, phase, seed, width0, width1, power=1.0, smoke=0.0):
    """A column of flame from (32, `mouth_y`) up `height` px into canvas
    `c`: its heat a widening column times noise that climbs with `phase`
    (px), so tongues lick off its edges and break from its head; banded
    from a dark red edge to a white core; smoke where it cools over its
    upper half. Its root stays behind the vent's near lip."""
    w, h = size
    for y in range(h):
        rise = (mouth_y - y) / max(1.0, height)
        if rise < -0.02 or rise > 1.6:
            continue
        hh = max(0.0, rise)
        cx = 32 + 2.4 * math.sin(y / 17 + phase / 23 + seed) * hh + 0.9 * math.sin(y / 6 + phase / 9)
        half = width0 + (width1 - width0) * min(1.0, hh) ** 0.6 + 3.0 * max(0.0, 1 - hh * 9)
        for x in range(w):
            if y > mouth_y - 3 and y > mouth_y - GEYSER_MOUTH[1] + near_lip(x) - 0.5:
                continue
            dx = (x + 0.5 - cx) / half
            n = fbm((x - cx) / 7.5 + seed, (y + phase) / 9.0, ('flame', seed))
            m = fbm((x - cx) / 3.5 - seed, (y + phase * 1.4) / 4.0, ('lick', seed))
            body = max(0.0, 1 - dx * dx)
            fade = 1 - max(0.0, min(1.0, (hh - 0.7) / 0.45))
            heat = power * body ** 0.8 * fade * (0.28 + 1.0 * n + 0.3 * m) - 0.2 * hh
            if heat > 0.16:
                c[(x, y)] = ramp(min(0.999, (heat - 0.16) ** 1.3), FLAME, x, y, 0.12)
            elif smoke and hh > 0.35 and abs(dx) < 1.5:
                s = fbm((x - cx) / 6 - seed, (y + phase * 0.6) / 7, ('smoke', seed)) * smoke
                s *= min(1.0, (hh - 0.35) * 2.5) * max(0.0, 1 - abs(dx) / 1.5) * 1.6
                if s > 0.5:
                    c.setdefault((x, y), 'ѐ' if s > 0.62 else 'ё')


def embers(c, size, mouth_y, height, phase, seed, count, width):
    """Embers flung out of a flame into canvas `c`: bright specks with a
    cooling tail, climbing and spreading with `phase`."""
    w, h = size
    for k in range(count):
        side = -1 if k % 2 else 1
        life = (unit('ember', seed, k) + phase / 40) % 1.0
        y = round(mouth_y - height * (0.15 + 0.85 * life))
        x = round(32 + side * (width * (0.6 + 0.5 * life) + 2 + unit('emx', seed, k) * 6 + life * 10))
        if 0 <= x < w and 1 <= y < h - 1 and (x, y) not in c:
            c[(x, y)] = 'Y' if life < 0.5 else 'y'
            c.setdefault((x - side, y + 1), 'K' if life < 0.6 else 'L')


# The eruption's frames: its height, the root's and the head's half widths,
# its power and its smoke.
PLUME_SHAPES = ((118, 6.5, 13, 1.35, 0.0), (168, 7.5, 17.5, 1.35, 0.8), (174, 7.5, 18.5, 1.32, 1.0),
                (174, 7.5, 18.5, 1.28, 1.1), (166, 7.0, 17.5, 1.15, 1.2), (150, 4.5, 17, 0.8, 1.4))


def geyser_plume(frame):
    """The eruption, frame `frame` of 6 over its 0.8 s (64 x 176, its bottom
    row on the floor, over the vent's mouth): a roaring column of flame,
    white-yellow at its core, red and smoky at its edges, embers flung out;
    it bursts up, roars and gutters in its last frame."""
    c = {}
    mouth_y = GEYSER_PLUME[1] - GEYSER_VENT[1] + GEYSER_MOUTH[1]
    height, w0, w1, power, smoke = PLUME_SHAPES[frame]
    flame(c, GEYSER_PLUME, mouth_y, height, frame * 26.0, 3.1, w0, w1, power, smoke)
    embers(c, GEYSER_PLUME, mouth_y, height, frame * 26.0, 5, 10 if frame else 5, w1 * 0.7)
    return to_image(c, GEYSER_PLUME)


def geyser_sputter(frame):
    """The sputter, frame `frame` of 4 over its 0.6 s (64 x 64, its bottom
    row on the floor, over the vent's mouth): licks of flame and puffs of
    smoke, each higher than the last."""
    c = {}
    mouth_y = GEYSER_SPUTTER[1] - GEYSER_VENT[1] + GEYSER_MOUTH[1]
    height = (12, 19, 27, 38)[frame]
    flame(c, GEYSER_SPUTTER, mouth_y, height, frame * 13.0 + 40, 8.3 + frame, 3.5 + frame * 0.6, 2.5 + frame,
          0.95 + 0.1 * frame, smoke=1.6)
    embers(c, GEYSER_SPUTTER, mouth_y, height, frame * 13.0, 9, 1 + frame, 3)
    return to_image(c, GEYSER_SPUTTER)


def rails_worn():
    """A run of rails with a sleeper gone."""
    c = from_image(rails())
    for x in range(32, 43):
        for y in range(4, 10):
            c.pop((x, y), None)
    return to_image(c, (TILE, 10))


def placed(images, name, im, hang=False):
    """`im` as tiles: in one when it fits, its foot on the tile's floor (or
    its top on the tile's ceiling, `hang`), centred; else padded the same
    way to whole tiles and cut `<name>_big_<row>_<col>`."""
    cols, rows = -(-im.width // TILE), -(-im.height // TILE)
    canvas = Image.new('RGBA', (cols * TILE, rows * TILE))
    canvas.alpha_composite(im, ((cols * TILE - im.width) // 2, 0 if hang else rows * TILE - im.height))
    if (cols, rows) == (1, 1):
        images[name] = canvas
        return [name]
    names = []
    for row in range(rows):
        for col in range(cols):
            piece = f'{name}_big_{row}_{col}'
            images[piece] = canvas.crop((col * TILE, row * TILE, (col + 1) * TILE, (row + 1) * TILE))
            names.append(piece)
    return names


def clip(images, animations, name, frames, duration):
    """Frames `<name>_<i>` and their looping animation `<name>`."""
    names = []
    for i, im in enumerate(frames):
        images[f'{name}_{i}'] = im
        names.append(f'{name}_{i}')
    animations[name] = {'looping': True, 'frames': [{'sprite': 'ex10_' + n, 'duration_ms': duration} for n in names]}
    return names


# Standing props: name and drawing; then the hung ones.
LEVEL_PROPS = (
    ('fence_0', lambda: fence(0)), ('fence_1', lambda: fence(1)), ('signpost', lambda: signpost(0)),
    ('stump', lambda: stump(0)), ('log_pile', lambda: log_pile(0)),
    ('pillar_stump_0', lambda: pillar_stump(0)),
    ('pillar_stump_1', lambda: pillar_stump(1)), *((f'reeds_{i}', lambda i=i: reeds(i)) for i in range(4)),
    ('statue', lambda: statue(0)),
    ('fern_clump_0', lambda: fern_clump(0)), ('fern_clump_1', lambda: fern_clump(1, (64, 40))),
    ('grass_tuft_0', lambda: grass_tuft(0)), ('grass_tuft_1', lambda: grass_tuft(1)),
    *((f'glowcaps_{i}', lambda i=i: glowcaps(i)) for i in range(3)), ('cliff_tree', lambda: cliff_tree(0)),
    ('lily_pads', lambda: lily_pads(0)), ('splash_rocks_0', lambda: splash_rocks(0)),
    ('splash_rocks_1', lambda: splash_rocks(1)), ('pool_rocks_0', lambda: pool_rocks(0)),
    ('pool_rocks_1', lambda: pool_rocks(1)),
    *((f'stalagmite_{i}', lambda i=i: stalagmite(i)) for i in range(3)),
    ('obsidian', lambda: obsidian(0)),
    *((f'fire_crystal_{i}', lambda i=i: fire_crystal(i)) for i in range(3)),
    ('headframe', lambda: headframe(0)), ('trestle', lambda: trestle(105)), ('rail_0', rails), ('rail_1', rails_worn),
    ('rail_bumper', lambda: rail_bumper(0)), ('cart', cart), ('crate_0', lambda: crate(0)), ('crate_1', lambda: crate(1)),
    ('barrel_0', lambda: barrel(0)), ('barrel_1', lambda: barrel(1)),
    ('boulder_0', lambda: boulder(0)), ('boulder_1', lambda: boulder(7, (48, 30))), ('boulder_2', lambda: boulder(8, (56, 34))),
    ('rubble_pile_0', lambda: rubble_pile(0)), ('rubble_pile_1', lambda: rubble_pile(9, (80, 30))),
    ('ore_heap', lambda: ore_heap(0)), *((f'pebbles_{i}', lambda i=i: pebbles(i * 9)) for i in range(3)),
    ('tool_rack', lambda: tool_rack(0)), ('timber_stack', lambda: timber_stack(0)),
    ('wheelbarrow', lambda: wheelbarrow(0)), ('pickaxe', lambda: pickaxe(0)), ('winch', lambda: winch(0)),
    *((f'bones_{i}', lambda i=i: bones(i)) for i in range(3)), ('guardian', lambda: guardian(0)),
    ('banner_0', lambda: banner(0)), ('banner_1', lambda: banner(1)), ('skull_pike', lambda: skull_pike(0)),
    ('toll_boom', lambda: toll_boom(0)), ('toll_sign', lambda: toll_sign(0)), ('lean_to', lambda: lean_to(0)),
    ('spoil_heap', lambda: spoil_heap(0)), ('cart_tipped', broken_cart),
    ('lift_gantry', lambda: lift_gantry(0)),
    *((f'fallen_voussoir_{k}', lambda k=k: fallen_voussoir(k)) for k in range(3)),
    # e01hollow's aqueduct: the broken pillar at its start, its floor's
    # paving, a fallen capital and a broken amphora.
    ('broken_pillar', broken_pillar), *((f'aq_paving_{k}', lambda k=k: aq_paving(k)) for k in range(4)),
    ('fallen_capital', fallen_capital), ('amphora', amphora),
    # e01hollow's rubble: basalt chunks in heaps, glassy obsidian, scree
    # against a wall either side and a slab fallen from the roof.
    *((f'basalt_rubble_{k}', lambda k=k: heap(RUBBLE_HEAPS[k], ('rubble', k))) for k in range(3)),
    ('obsidian_rubble', lambda: heap(OBSIDIAN_HEAP, 'obsidian-rubble')),
    ('scree_left', lambda: scree('left', 0)), ('scree_right', lambda: scree('right', 0)),
    ('fallen_slab', fallen_slab),
)
HUNG_PROPS = (
    *((f'vine_{i}', lambda i=i, n=n: vine(n, 2 + i, i != 1)) for i, n in enumerate((110, 150, 84))),
    *((f'vine_curtain_{i}', lambda i=i, n=n: vines(n, 20 + i, gap=9)) for i, n in enumerate(
        ((150, 96, 180), (70, 124, 92, 56), (100, 62, 136, 84), (52, 34)))),
    ('rope_ladder', lambda: rope_ladder(0)), ('rope_bucket', lambda: rope_bucket(100)),
    ('hanging_lantern', lambda: hanging_lantern(0)), ('chain', lambda: chain(0, links=10)),
    *((f'stalactite_{i}', lambda i=i: stalactite(i)) for i in range(len(STALACTITES))),
    *((f'fire_crystal_hanging_{i}', lambda i=i: fire_crystal(i + 3, hanging=True)) for i in range(2)),
    ('fissure', lambda: fissure(34, 2)),
)
# The heat haze over the lava: the plume's translucent whites warmed, at
# the same alphas.
HAZE = {'j': 'э', '^': 'ю', '&': 'я', '*': 'Б', 'p': 'Г', 'q': 'Д'}


def haze(width, height, seed, strength, phase=0.0):
    """Heat rising off the lava (e01hollow, round 3), at `phase` (0 to 1)
    through one rising loop: thin wisps in the palette's warm translucents,
    thickest just over the surface and fading as they climb, curling as
    they go, the ends of the lava thinning them; frames at even phases
    loop."""
    c = {}
    ramp_symbols = 'эюяБ'
    for y in range(height):
        rise = 1 - y / height
        for x in range(width):
            u = x / width
            ends = min(1.0, u / 0.15, (1 - u) / 0.15)
            # Each wisp climbs its own height, so the top edge is ragged.
            reach = 0.45 + 0.55 * fbm(x / 46, 0.5, ('reach', seed))
            base = ends * max(0.0, 1 - rise / reach) ** 1.6 * strength
            if base < 0.02:
                continue
            curl = 5 * math.sin(y / 13 + x / 37 + seed)

            def wisp(t):
                n = fbm((x + curl) / 15, (y + t * 90) / 17, ('heat', seed))
                return 1 - abs(2 * n - 1)

            w = (1 - phase) * wisp(phase) + phase * wisp(phase - 1)
            v = base * w ** 3.2 * len(ramp_symbols) * 1.7
            level = int(v) + (1 if dither(x, y, v - int(v)) else 0)
            if level > 0:
                c[(x, y)] = ramp_symbols[min(level, len(ramp_symbols)) - 1]
    return to_image(c, (width, height))


# Unlit: mist and steam, which the night shows through, the lava's haze
# (subtle since e01hollow), and the campfire's flames, which carry their own
# light.
MISTS = ()
# The rising mist, unlit and animated: the falls' plume, drawn at a phase,
# `MIST_FRAMES` frames through one cycle, each tile of its cut a clip of its
# own.
RISING = (
    ('plume', lambda t: plume(300, 170, 1.3, 1.5, t)),
    # The heat off the lava lake and the shelf's lava (e01hollow, round 3).
    ('haze', lambda t: haze(620, 150, 4.2, 0.55, t)),
    ('haze_small', lambda t: haze(180, 100, 5.1, 0.5, t)),
)
MIST_FRAMES, MIST_MS = 6, 150


def rising_clips(images, animations, name, frames, duration):
    """`frames` padded as `placed` pads one, foot on the floor, and cut into
    tiles: each tile's frames `<name>_big_<row>_<col>_<i>` and its looping
    animation `<name>_big_<row>_<col>`."""
    cols, rows = -(-frames[0].width // TILE), -(-frames[0].height // TILE)
    pieces = {}
    for im in frames:
        canvas = Image.new('RGBA', (cols * TILE, rows * TILE))
        canvas.alpha_composite(im, ((cols * TILE - im.width) // 2, rows * TILE - im.height))
        for row in range(rows):
            for col in range(cols):
                pieces.setdefault((row, col), []).append(
                    canvas.crop((col * TILE, row * TILE, (col + 1) * TILE, (row + 1) * TILE)))
    for (row, col), tiles in pieces.items():
        clip(images, animations, f'{name}_big_{row}_{col}', tiles, duration)


def far_lantern():
    """A lit lantern on the far scaffold: its glass a speck of the glowing
    yellows in a dark frame."""
    return to_image({(1, 0): 'K', (0, 1): 'K', (1, 1): 'y', (2, 1): 'K', (1, 2): 'Y', (1, 3): 'K'}, (3, 4))


# Unlit, under the far tint as the review drew them (`terrain.tinted`): the
# cut's far scaffold, a step darker, and the boulders on its far terraces.
FAR_PROPS = (
    ('far_trestle', lambda: to_image(tinted(darker(from_image(trestle(60)))), (TILE, 60))),
    ('far_boulder_0', lambda: to_image(tinted(from_image(boulder(7, (48, 30)))), (48, 30))),
    ('far_boulder_1', lambda: to_image(tinted(from_image(boulder(8, (56, 34)))), (56, 34))),
    ('far_rubble', lambda: to_image(tinted(from_image(rubble_pile(9, (80, 30)))), (80, 30))),
    ('far_lantern', far_lantern),
)


def build_level_scenery(images, lit, animations):
    """M76's props and its water and lava clips."""
    for name, draw in LEVEL_PROPS:
        lit.update(placed(images, name, draw()))
    for name, draw in HUNG_PROPS:
        lit.update(placed(images, name, draw(), hang=True))
    for name, draw in MISTS:
        placed(images, name, draw())
    for name, draw in RISING:
        rising_clips(images, animations, name, [draw(k / MIST_FRAMES) for k in range(MIST_FRAMES)], MIST_MS)
    for name, draw in FAR_PROPS:
        placed(images, name, draw())
    clip(images, animations, 'campfire', [campfire(f * 1.6) for f in range(4)], 120)
    lit.update(clip(images, animations, 'water', [water_frame(f) for f in range(8)], 90))
    for kind, name in (('calm', 'river'), ('rapids', 'river_rapids'), ('rock', 'river_rock')):
        lit.update(clip(images, animations, name, [river_frame(f, kind) for f in range(8)], 110))
    lit.update(clip(images, animations, 'river_deep', [river_frame(f, deep=True) for f in range(8)], 110))
    # The river's depths below its first rows, darker going down.
    for k in (1, 2):
        images[f'river_dark_{k}'] = to_image(darker(from_image(images['river_deep_0']), k * 2))
        lit.add(f'river_dark_{k}')
    for kind, name in (('calm', 'pool'), ('foam', 'pool_foam'), ('edge_left', 'pool_edge_left')):
        lit.update(clip(images, animations, name, [pool_frame(f, kind) for f in range(4)], 160))
    # The pool's east shore, each tile with any of its art a clip.
    shore = [pool_shore(f) for f in range(4)]
    for row in range(2):
        for col in range(3):
            tiles = [im.crop((col * TILE, row * TILE, (col + 1) * TILE, (row + 1) * TILE)) for im in shore]
            if any(tile.getbbox() for tile in tiles):
                lit.update(clip(images, animations, f'pool_shore_big_{row}_{col}', tiles, 160))
    # The aqueduct's small pool (e01hollow), each tile a clip.
    basin = [pool_basin(f) for f in range(4)]
    for col in range(BASIN[0] // TILE):
        tiles = [im.crop((col * TILE, 0, (col + 1) * TILE, TILE)) for im in basin]
        lit.update(clip(images, animations, f'pool_basin_big_0_{col}', tiles, 160))
    falls = [dict(falls_tiles(f)) for f in range(4)]
    for name in falls[0]:
        lit.update(clip(images, animations, name, [frames[name] for frames in falls], 80))
    # The aqueduct's water (01 (k)): its pours and their splash feet, the
    # spill over each broken end's lip, the drips under the coping and the
    # gargoyle with the water over its tongue; e01hollow's second gargoyle,
    # the drake, with the water out of its jaws.
    lit.update(clip(images, animations, 'pour_wide', [pour_wide(f) for f in range(4)], 80))
    lit.update(clip(images, animations, 'pour_foot', [pour_foot(f) for f in range(4)], 80))
    for side in ('left', 'right'):
        for k in range(3):
            lit.update(clip(images, animations, f'spill_{side}_{k}', [spill(f, side, k) for f in range(4)], 80))
    lit.update(clip(images, animations, 'channel_drips', [channel_drips(f) for f in range(6)], 120))
    lit.update(clip(images, animations, 'gargoyle', [gargoyle(f) for f in range(4)], 80))
    lit.update(clip(images, animations, 'gargoyle_drake', [gargoyle_drake(f) for f in range(4)], 80))
    lit.update(clip(images, animations, 'lava_surface', [lava_frame(f) for f in range(4)], 220))
    lit.update(clip(images, animations, 'lava_deep', [lava_frame(f, 'deep') for f in range(4)], 220))
    lit.update(clip(images, animations, 'lavafall', [lavafall_frame(f) for f in range(4)], 100))
    # e01hollow: the lavafall's foot in the basin; the geysers' vent, lit,
    # and its choked twin; the sputter's four frames and the eruption's six,
    # unlit, which the game picks by the phase's time.
    lit.update(clip(images, animations, 'lava_splash', [lava_splash(f) for f in range(4)], 100))
    for i in range(len(EMBER_POOLS)):
        lit.update(clip(images, animations, f'ember_pool_{i}', [ember_pool(i, f) for f in range(POOL_FRAMES)],
                        POOL_MS))
    images['geyser_vent'] = geyser_vent()
    images['geyser_vent_choked'] = geyser_vent(choked=True)
    lit.update({'geyser_vent', 'geyser_vent_choked'})
    for f in range(4):
        images[f'geyser_sputter_{f}'] = geyser_sputter(f)
    for f in range(6):
        images[f'geyser_plume_{f}'] = geyser_plume(f)
    # The elevator's cage and cable, which the game draws (M79).
    images['lift_deck_elevator'] = elevator_cage()
    images['lift_cable'] = lift_cable()
    lit.update({'lift_deck_elevator', 'lift_cable'})
    # The crust's look (01 (k)), drawn by the game over a basin's lava: its
    # nine frames, and each row's partials as the freeze's front and the
    # melt's come down through it.
    for i in range(2):
        for part in ('top', 'deep'):
            for n in range(9):
                name = f'lava_crust_{part}_{n}_{i}'
                images[name] = lava_crust_frame(n, part, i)
                lit.add(name)
            for j in range(TILE // FRONT_STEP):
                for kind, draw in (('freeze', lava_freeze), ('melt', lava_melt)):
                    name = f'lava_{kind}_{part}_{j}_{i}'
                    images[name] = draw(j, part, i)
                    lit.add(name)
