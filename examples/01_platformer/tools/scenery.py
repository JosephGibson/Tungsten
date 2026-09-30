"""Ruin scenery: lamp posts, ivy, waystones, the summit gate, oaks, the broken
arch, hanging roots, undergrowth, crystals, the waterfall and spikes.

Props share the courier's language: flat palette fills shaded from their own
silhouettes and a single dark outline. Composite pieces (gate, oak, arch,
roots) are drawn whole and cut into 64-pixel tiles.
"""
import math

from art import rng_for
from pixels import Part, compose, despeckle, dither, outline, shade, to_image, unit

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
