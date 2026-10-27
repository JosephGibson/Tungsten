"""01 (g)'s props: pine crates that burn down and red powder kegs on a fuse.

Drawn as the review sheet (M81) proposed, with the generator's own toolkit:
two pine crates in their own pale ramp (`Л П У`), apart from the mine's dark
decor; a powder keg in red-painted staves with riveted hoops, a skull and a
fuse, and its four lit frames, the cord shorter in each; a burning crate's
char overlay in three stages; and the particles of a crate falling in, a keg
blowing and a rock breaking. The crates and the kegs are lit; the overlay and
the particles draw unlit over the lit world.
"""
import math

from actors import SOLE, lerp
from pixels import Part, compose, outline, point, shade, to_image, unit

PINE = dict(base='П', light='Л', shadow='У', back='У', rim='Л')
PINE_DARK = dict(base='У', light='П', shadow='"', back='"', rim='П')
KEG_RED = dict(base='z', light='F', shadow='ϑ', back='ϑ', rim='F')
# A crate's box: 56 px, its prefab's half 28 about the frame's centre.
LO, HI = 4, 59
# The keg's fuse in its frames, by length left: lit at 10, burnt to 2.
FUSE_FRAMES = (10, 7, 4, 2)


def grain(c, inside, seed, along_x, density=0.18):
    """Darker grain lines along each board."""
    for (x, y) in sorted(inside):
        if c.get((x, y)) != 'П':
            continue
        lane = y if along_x else x
        phase = x if along_x else y
        if unit(seed, 'lane', lane) < density and unit(seed, 'run', lane, phase // 7) < 0.7:
            c[(x, y)] = 'У'


def nail(c, x, y):
    c[(x, y)] = 'ψ'
    c[(x + 1, y)] = 'Φ'
    c[(x, y + 1)] = 'Φ'
    c[(x + 1, y + 1)] = 'B'


def knot(c, x, y):
    for dx, dy in ((0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)):
        if c.get((x + dx, y + dy)) in ('П', 'У', 'Л'):
            c[(x + dx, y + dy)] = 'У'
    if (x, y) in c:
        c[(x, y)] = '"'


def crate_slatted():
    """Pale pine: two stiles and three rails, vertical slats between with
    straw showing through their gaps, nails at every crossing, grain and
    knots, a stencilled pair of arrows on the middle rail."""
    stiles, rails, slats, gaps = Part(), Part(), Part(), Part()
    stiles.rectangle((LO, LO, LO + 6, HI))
    stiles.rectangle((HI - 6, LO, HI, HI))
    for y0 in (LO, 29, HI - 6):
        rails.rectangle((LO + 7, y0, HI - 7, y0 + 6))
    xs = [(11, 19), (22, 30), (33, 41), (44, 52)]
    for x0, x1 in xs:
        slats.rectangle((x0, LO + 7, x1, HI - 7))
    for x0, x1 in ((20, 21), (31, 32), (42, 43)):
        gaps.rectangle((x0, LO + 7, x1, HI - 7))
    c = compose([
        ({p: 'ь' for p in gaps.pixels()}, None),
        (shade(slats.pixels(), **PINE), '"'),
        (shade(rails.pixels(), **PINE), '"'),
        (shade(stiles.pixels(), **PINE), '"'),
    ])
    for y0 in (LO + 7, 36, HI - 7):  # the rails and stiles shade the recessed slats
        for x in range(LO + 7, HI - 6):
            if c.get((x, y0)) == 'П':
                c[(x, y0)] = 'У'
    for y in range(LO + 7, HI - 6):
        if c.get((LO + 7, y)) in ('П', 'Л'):
            c[(LO + 7, y)] = 'У'
    grain(c, slats.pixels(), 'slat', False)
    grain(c, stiles.pixels(), 'stile', False, 0.22)
    grain(c, rails.pixels(), 'rail', True, 0.3)
    # Straw through the gaps: golden strands over the dark inside.
    for gx in (20, 31, 42):
        for y in range(LO + 8, HI - 7):
            if unit('straw', gx, y // 3) < 0.45:
                c[(gx + (y % 2), y)] = 'A' if unit('straw-tone', gx, y) < 0.6 else 'Л'
        for k in range(3):
            y = 14 + round(unit('tuft', gx, k) * 34)
            for t in range(3):
                c[(gx - 1 + t, y - t)] = 'A' if t % 2 else 'Л'
    for x0, x1 in xs:
        for y0 in (LO, 29, HI - 6):
            nail(c, x0 + 3, y0 + 2)
            nail(c, x1 - 4, y0 + 2)
    for y0 in (LO, 29, HI - 6):
        nail(c, LO + 2, y0 + 2)
        nail(c, HI - 4, y0 + 2)
    for x, y in ((15, 44), (37, 17), (56, 41)):
        knot(c, x, y)
    for ax in (25, 37):  # stencilled arrows on the middle rail: this way up
        for t in range(3):
            for dx in range(-t, t + 1):
                if c.get((ax + dx, 30 + t)) in ('П', 'У'):
                    c[(ax + dx, 30 + t)] = '"'
        c[(ax, 33)] = '"'
        c[(ax, 34)] = '"'
    return outline(c)


def crate_planked():
    """Pale pine: five planks across, two battens nailed over them, a split
    plank end and a chipped corner, grain and knots, a trader's brand."""
    planks, battens = Part(), Part()
    rows = [(LO, 13), (15, 24), (26, 35), (37, 46), (48, HI)]
    for y0, y1 in rows:
        planks.rectangle((LO, y0, HI, y1))
    for x0 in (11, 46):
        battens.rectangle((x0, LO, x0 + 6, HI))
    inside = planks.pixels()
    inside -= {(x, y) for x in range(LO, LO + 4) for y in range(LO, LO + 4) if x + y < LO * 2 + 4}
    c = compose([
        ({(x, y): '"' for x in range(LO, HI + 1) for y in range(LO, HI + 1)}, None),
        (shade(inside, **PINE), None),
        (shade(battens.pixels(), **PINE_DARK), '"'),
    ])
    for y0, y1 in rows:  # each plank shaded as its own board
        for x in range(LO, HI + 1):
            if c.get((x, y0)) == 'П':
                c[(x, y0)] = 'Л'
            if c.get((x, y1)) == 'П':
                c[(x, y1)] = 'У'
    grain(c, inside, 'plank', True, 0.26)
    for x in range(LO, HI + 1):  # the grooves between the planks
        for y in (14, 25, 36, 47):
            if c.get((x, y)) not in ('У', 'П', 'Л'):
                continue
            c[(x, y)] = '"'
    for x0 in (11, 46):
        for y0, y1 in rows:
            nail(c, x0 + 2, y0 + 3)
    for t in range(6):  # a split along the third plank's end
        c[(HI - 1 - t, 30 + (t > 2))] = '"'
    for x, y in ((28, 9), (40, 41), (22, 52)):
        knot(c, x, y)
    for y in range(24, 39):  # a trader's brand burnt in: a ring round a star
        for x in range(25, 40):
            r = math.hypot(x - 32, y - 31)
            star = abs(x - 32) + abs(y - 31) <= 3 or (abs(x - 32) <= 0 and abs(y - 31) <= 4) \
                or (abs(y - 31) <= 0 and abs(x - 32) <= 4)
            if (5.2 <= r <= 6.4 or star) and c.get((x, y)) in ('П', 'Л', 'У'):
                c[(x, y)] = '"' if unit('brand', x, y) < 0.8 else 'У'
    return outline(c)


def crate_char(stage):
    """A burning crate's overlay in three stages, which 01 picks by thirds of
    the burn: soot climbing from the foot and the edges, the boards' edges
    eaten black, ember cracks along the grain, and at the last stage holes
    burnt through. Drawn over the crate's charring tint."""
    c = {}
    cut = (0.62, 0.42, 0.24)[stage]
    for y in range(LO, HI + 1):
        for x in range(LO, HI + 1):
            edge = min(x - LO, HI - x, y - LO, HI - y) / 28
            v = (unit('soot', x // 3, y // 3) * 0.55 + unit('soot2', x // 2, y // 2) * 0.25
                 + (y - LO) / 56 * 0.45 - edge * 0.5)
            if v > cut:
                deep = v - cut
                c[(x, y)] = 'ь' if deep > 0.22 else ('б' if deep > 0.1 else 'а')
                if stage == 2 and deep > 0.42:
                    c[(x, y)] = 'щ'
    for k in range(4 + 5 * stage):  # ember cracks along the grain
        x = LO + 4 + round(unit('crack-x', k) * 47)
        y = LO + 14 + round(unit('crack-y', k) * 38)
        length = 4 + round(unit('crack-l', k) * (5 + 3 * stage))
        for t in range(length):
            p = (x + round(math.sin(t * 0.9 + k) * 1.2), y - t)
            if p in c:
                heat = unit('heat', k, t, stage)
                c[p] = 'J' if heat > 0.75 else ('K' if heat > 0.35 else 'L')
    for k in range(6 + 6 * stage):  # glowing specks in the soot
        p = (LO + round(unit('speck-x', k, stage) * 55), LO + 20 + round(unit('speck-y', k, stage) * 35))
        if p in c and c[p] != 'щ':
            c[p] = 'K' if k % 3 else 'J'
    return to_image(c)


def keg_body():
    """The keg's staves: its profile bulging from 36 px at the ends to 44."""
    body = Part()
    top, bottom = 11, SOLE
    left, right = [], []
    for k in range(13):
        y = top + (bottom - top) * k / 12
        half = 18 + 4 * math.sin(math.pi * k / 12)
        left.append((32 - half, y))
        right.append((31 + half, y))
    body.polygon(left + right[::-1])
    return body, top, bottom


def keg_canvas(fuse=FUSE_FRAMES[0], spark=None):
    """A powder keg in red-painted staves: two pairs of iron hoops with
    rivets, a white skull and crossbones stencilled on its belly, the paint
    worn to bare wood at the rims, a lid with a bung and a twisted fuse
    curling up from it. `fuse` is the cord's length left; `spark` a frame
    of its burning tip."""
    body, top, bottom = keg_body()
    inside = body.pixels()
    c = shade(inside, **KEG_RED)

    def half_at(y):
        return 18 + 4 * math.sin(math.pi * (y - top) / (bottom - top))
    for (x, y) in sorted(inside):  # stave seams following the bulge
        h = half_at(y)
        for u in (-0.72, -0.4, -0.12, 0.16, 0.44, 0.72):
            if round(32 + u * h) == x and c[(x, y)] in ('z', 'E'):
                c[(x, y)] = 'M'
        stave = int((x - 32) / (h * 0.28) + 10)
        if c[(x, y)] == 'z' and stave % 2:
            c[(x, y)] = 'E'
    for y0, y1 in ((15, 17), (21, 22), (48, 49), (53, 55)):  # the hoops
        for y in range(y0, y1 + 1):
            for x in range(64):
                if (x, y) in inside:
                    c[(x, y)] = 'ψ' if y == y0 else ('B' if y == y1 else 'Φ')
        for x in range(12, 52, 9):
            if (x, y0) in inside:
                c[(x, y0)] = 'χ'
    for (x, y) in sorted(inside):  # paint worn to bare wood by the rims
        if c[(x, y)] in ('z', 'E', 'F', 'ϑ') and (y < 15 or y > 55 or abs(x - 32) > half_at(y) - 2):
            if unit('wear', x // 2, y // 2) < 0.32:
                c[(x, y)] = "'" if unit('wear-t', x, y) < 0.5 else '"'
    skull = Part()  # the stencil: a skull over crossed bones
    skull.ellipse((32, 32), 5, 4.5)
    skull.rectangle((29, 34, 35, 38))
    bones = Part()
    bones.limb([(23, 44), (41, 33)], 2)
    bones.limb([(23, 33), (41, 44)], 2)
    for p in bones.pixels() | skull.pixels():
        if p in c and c[p] not in ('ψ', 'Φ', 'B', 'χ'):
            c[p] = 'w' if p in skull.pixels() or unit('bone', *p) < 0.8 else 'σ'
    for p in ((30, 32), (30, 33), (31, 32), (34, 32), (34, 33), (33, 32), (32, 35)):
        c[p] = 'Θ'  # eye sockets, the nose
    for x in (30, 32, 34):
        c[(x, 37)] = 'Θ'  # teeth
    lid = Part()
    lid.ellipse((32, 11), 17, 3)
    c.update(shade(lid.pixels(), '"', light="'", shadow='ω'))
    for x in range(18, 47, 4):
        if (x, 11) in c:
            c[(x, 11)] = 'ω'  # the lid's boards
    c[(38, 10)], c[(39, 10)], c[(38, 11)], c[(39, 11)] = '(', 'a', 'ω', '('
    canvas = outline(c)
    if fuse > 0:  # a twisted cord curling up and back from the bung
        path = [(38.5, 9), (39.5, 7), (41, 5), (40.5, 3), (39, 2), (37.5, 1)]
        pts = []
        for i in range(len(path) - 1):
            a, b = path[i], path[i + 1]
            for t in range(4):
                pts.append(point(lerp(a, b, t / 4)))
        pts = pts[:fuse * 2]
        for i, p in enumerate(pts):
            canvas[p] = 'σ' if i % 2 else 'τ'
        ring = {(x + dx, y + dy) for x, y in pts for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))}
        for p in ring:
            if p not in canvas and 0 <= p[0] < 64 and 0 <= p[1] < 64:
                canvas[p] = 'o'
        if spark is not None and pts:
            tx, ty = pts[-1]
            canvas[(tx, ty)] = 'Ψ'
            arms = ((1, 0), (-1, 0), (0, -1), (0, 1)) if spark % 2 == 0 else ((1, -1), (-1, -1), (1, 1), (-1, 1))
            for dx, dy in arms:
                canvas[(tx + dx, ty + dy)] = 'Y'
                if spark % 2 == 0:
                    canvas[(tx + 2 * dx, ty + 2 * dy)] = 'J'
    return canvas


def char_splinter():
    """`crate_collapse`'s particle: a charred plank sliver, its broken end
    still glowing."""
    c = {}
    for t in range(34):
        x, y = 15 + t, 40 - round(t * 0.45)
        for w in range(5):
            symbol = 'У' if w == 0 else ('"' if w < 3 else 'ь')
            if t > 24:
                symbol = 'K' if w < 2 else ('L' if w < 4 else 'ь')
            if t < 4 and w > 2:
                symbol = 'б'
            c[(x, y + w)] = symbol
    return to_image(outline(c))


def keg_stave():
    """`keg_staves`' particle: a red stave's piece with a hoop's stub."""
    c = {}
    for t in range(30):
        x, y = 18 + t, 38 - round(t * 0.3)
        for w in range(6):
            c[(x, y + w)] = 'F' if w == 0 else ('z' if w < 4 else 'ϑ')
    for w in range(7):
        c[(30, 33 + w)] = 'ψ'
        c[(31, 33 + w)] = 'Φ'
    for t in range(3):
        c[(46 + t, 28 + t * 2)] = 'K'
    return to_image(outline(c))


def rock_chip():
    """`rock_chips`' particle: a grey chip of the goblin's rock."""
    c = shade({(x, y) for x in range(26, 38) for y in range(27, 37) if abs(x - 32) + abs(y - 32) < 7},
              'Ω', light='ψ', shadow='Φ', back='B', rim='χ')
    return to_image(outline(c))


# e01sfx: a broken crate's five planks, each a board of 28 x 8 about the
# middle of its 32 x 16 frame, its body's box.
PLANKS = 5
PLANK_FRAME = (32, 16)
PLANK_BOARD = (2, 4, 30, 12)


def build_planks(images, lit):
    """The planks, lit as the crates are. Until e01sfx's asset session draws
    them, each is a crate shrunk to the board, its two variants in turn."""
    from PIL import Image

    from art import canvas
    x0, y0, x1, y1 = PLANK_BOARD
    for i in range(PLANKS):
        crate = images[f'pine_crate_{i % 2}'].crop((LO, LO, HI + 1, HI + 1))
        im, _ = canvas(PLANK_FRAME)
        im.paste(crate.resize((x1 - x0, y1 - y0), Image.Resampling.NEAREST), (x0, y0))
        images[f'plank_{i}'] = im
        lit.add(f'plank_{i}')


def build_props(images, lit):
    """The crates and the keg, lit; the keg's lit frames by quarter of its
    fuse; the char overlay's three stages and the particles, unlit; and the
    crates' planks (`build_planks`), lit."""
    for name, canvas in (('pine_crate_0', crate_slatted()), ('pine_crate_1', crate_planked()),
                         ('powder_keg', keg_canvas())):
        images[name] = to_image(canvas)
        lit.add(name)
    for i, fuse in enumerate(FUSE_FRAMES):
        name = f'powder_keg_lit_{i}'
        images[name] = to_image(keg_canvas(fuse, spark=i))
        lit.add(name)
    for stage in range(3):
        images[f'pine_crate_char_{stage}'] = crate_char(stage)
    build_planks(images, lit)
    images['char_splinter'] = char_splinter()
    images['keg_stave'] = keg_stave()
    images['rock_chip'] = rock_chip()
