"""Ruin masonry, turf, platforms, trestles and the waterfall's wet cliff; M76's
undersides, pit lining, channel, basalt, wet and mine rock, iron cladding,
borders and bridge deck.

Every opaque terrain tile is cut from one masonry texture that repeats exactly
every 64 pixels. Variants only change stones that lie wholly inside a tile, so
any arrangement of variants meets without a seam; the turf lip, cliff edges and
platform decks are likewise identical wherever tiles touch.
"""
import functools
import math
from dataclasses import dataclass

from PIL import Image

from art import PALETTE, rng_for
from pixels import dither, to_image, unit, wave

TILE = 64
COURSES = ((0, 15), (15, 17), (32, 14), (46, 18))  # (top row, height); sums to 64
DECK = 23  # platform decks are exactly this many pixels deep (collision depth)
VARIANTS = 8
LIGHTEN = {'<': '-', '-': '=', '=': '~', '~': ':', ':': ';', ';': ';'}
DARKEN = {';': ':', ':': '~', '~': '=', '=': '-', '-': '<', '<': '<'}
WET = {';': '{', ':': '=', '~': '=', '=': '-', '-': '|', '<': 'o'}
RING = ((-1, -2), (0, -2), (1, -2), (-2, -1), (2, -1), (-2, 0), (2, 0), (-2, 1), (2, 1),
        (-1, 2), (0, 2), (1, 2))
RELIEF_PREFIXES = ('rock_', 'moss_rock_', 'ground', 'cliff_', 'slab_', 'bridge', 'lift_deck',
                   'timber_support', 'corner_', 'fill_', 'stone_', 'platform', 'ledge',
                   # M76's families, and the background layer's (`backdrops.py`).
                   'under_', 'pit_', 'channel_', 'basalt_', 'wet_rock_', 'mine_rock_', 'iron_clad_',
                   'border_', 'arcade_', 'pier_', 'parapet_', 'wet_back_', 'cut_back_', 'cave_back_',
                   'cave_tunnel_')


@dataclass(frozen=True)
class Stone:
    course: int
    index: int
    x0: int
    width: int
    y0: int
    height: int

    def local(self, x, y):
        return ((x - self.x0) % TILE, y - self.y0)

    @property
    def inland(self):
        """Wholly inside the tile and clear of its edges, so variants may alter it."""
        return 2 <= self.x0 and self.x0 + self.width <= TILE - 2


def stones():
    layout = []
    for course, (y0, height) in enumerate(COURSES):
        rng = rng_for(f'masonry-course-{course}')
        x = rng.randrange(TILE)
        widths = []
        while sum(widths) < TILE - 30:
            widths.append(rng.randrange(14, 29))
        rest = TILE - sum(widths)
        widths += [rest // 2, rest - rest // 2] if rest > 28 else [rest]
        for index, width in enumerate(widths):
            layout.append(Stone(course, index, x % TILE, width, y0, height))
            x += width
    return layout


STONES = stones()


def stone_at(x, y):
    for stone in STONES:
        u, v = stone.local(x, y)
        if 0 <= v < stone.height and u < stone.width:
            return stone
    raise AssertionError((x, y))


def stone_tone(stone):
    roll = unit('stone-tone', stone.course, stone.index)
    return ':' if roll < 0.07 else '=' if roll < 0.47 else '~'


def face(stone, u, v, tone, width=None):
    """Symbol for a stone pixel at local (u, v); u = 0 / v = 0 are mortar joints."""
    width = width or stone.width
    if u == 0 or v == 0:
        return '<'
    fu, fv, fw, fh = u - 1, v - 1, width - 1, stone.height - 1
    corner = min(fu, fw - 1 - fu) + min(fv, fh - 1 - fv)
    chip = unit('chip', stone.course, stone.index, fu < fw / 2, fv < fh / 2) < 0.45
    if corner == 0 or (chip and corner == 1):
        return '<'
    if fv == 0:
        return ';' if tone == ':' else ':'
    if fu == fw - 1:
        return ':'
    if fv == fh - 1:
        return '-'
    if fu == 0:
        return '='
    symbol = tone
    speck = unit('speck', stone.course, stone.index, fu, fv)
    if speck < 0.06:
        symbol = DARKEN[symbol]
    elif speck < 0.09:
        symbol = LIGHTEN[symbol]
    # Stones darken toward their base.
    if fv > fh * 0.62 and dither(fu, fv, (fv / fh - 0.62) * 2.2):
        symbol = DARKEN[symbol]
    return symbol


def masonry(seed=None):
    """The periodic wall; `seed` names a variant that reworks inland stones only."""
    canvas = {}
    for y in range(TILE):
        for x in range(TILE):
            stone = stone_at(x, y)
            u, v = stone.local(x, y)
            canvas[(x, y)] = face(stone, u, v, stone_tone(stone))
    if seed is not None:
        vary(canvas, seed)
    return canvas


def vary(canvas, seed):
    rng = rng_for(seed)
    inland = [s for s in STONES if s.inland]
    rng.shuffle(inland)
    split, cracked, stained, pale, shelf = inland[0], inland[1:3], inland[3], inland[4], inland[5]
    # Split one stone in two with a new joint.
    if split.width >= 20:
        cut = split.width // 2 + rng.randrange(-3, 4)
        tone = stone_tone(split)
        for v in range(split.height):
            for u in range(1, split.width):
                x, y = (split.x0 + u) % TILE, split.y0 + v
                if u < cut:
                    canvas[(x, y)] = face(split, u, v, tone, width=cut)
                else:
                    half = Stone(split.course, split.index + 100, split.x0 + cut, split.width - cut,
                                 split.y0, split.height)
                    canvas[(x, y)] = face(half, u - cut, v, tone)
    # A shallow block laid on its side: a second horizontal joint.
    row = shelf.height // 2 + rng.randrange(-1, 2)
    for u in range(1, shelf.width):
        x = (shelf.x0 + u) % TILE
        canvas[(x, shelf.y0 + row)] = '<'
        if canvas[(x, shelf.y0 + row + 1)] not in '<':
            canvas[(x, shelf.y0 + row + 1)] = ':'
        if canvas[(x, shelf.y0 + row - 1)] not in '<':
            canvas[(x, shelf.y0 + row - 1)] = '-'
    # Hairline cracks wander down a face.
    for stone in cracked:
        x = stone.x0 + 3 + rng.randrange(max(1, stone.width - 6))
        for y in range(stone.y0 + 2, stone.y0 + 2 + rng.randrange(4, stone.height - 3)):
            x += rng.choice((-1, 0, 0, 1))
            x = min(max(x, stone.x0 + 2), stone.x0 + stone.width - 2)
            canvas[(x, y)] = '<'
    # A water stain runs down from the top joint.
    x = stained.x0 + 2 + rng.randrange(max(1, stained.width - 4))
    for y in range(stained.y0 + 2, stained.y0 + stained.height - 2):
        if canvas[(x, y)] not in '<-':
            canvas[(x, y)] = DARKEN[canvas[(x, y)]]
    # One paler, freshly exposed block.
    for v in range(2, pale.height - 1):
        for u in range(2, pale.width - 1):
            p = ((pale.x0 + u) % TILE, pale.y0 + v)
            if canvas[p] not in '<':
                canvas[p] = LIGHTEN[canvas[p]]
    # Now and then a wall has lost a block; the cavity shows rubble in shadow.
    if rng.random() < 0.15:
        hole = cracked[0]
        for v in range(1, hole.height):
            for u in range(1, hole.width):
                p = ((hole.x0 + u) % TILE, hole.y0 + v)
                rubble = unit('cavity', seed, u, v)
                canvas[p] = '<' if v < 3 or rubble < 0.4 else '-' if rubble < 0.8 else '='
    # Moss creeps along a few inland joints.
    for _ in range(rng.randrange(3, 8)):
        x, y = rng.randrange(4, TILE - 4), rng.choice([c[0] for c in COURSES[1:]])
        for dx in range(rng.randrange(2, 6)):
            if 3 <= x + dx < TILE - 3 and canvas[(x + dx, y)] == '<':
                canvas[(x + dx, y)] = 't' if dx % 3 else 'T'


def deepen(canvas, fade_from=None):
    """Push the wall one step darker, so the play surface reads above the mass.
    With `fade_from`, darkening ramps in (by dither) from that row to the bottom."""
    for (x, y), symbol in canvas.items():
        if symbol not in ';:~=-':
            continue
        if fade_from is None or (y >= fade_from and dither(x, y, (y - fade_from) / (TILE - 1 - fade_from))):
            canvas[(x, y)] = DARKEN[symbol]
    return canvas


def turf(canvas, seed=None):
    """Shared turf lip on rows 0..5; `seed` adds variant moss curtains below it."""
    for x in range(TILE):
        tip = wave(x, TILE, (3, 0.6, 0.4), (7, 0.4, 1.9), (11, 0.3, 0.2))
        canvas[(x, 0)] = 'm' if tip > 0.55 else 'G'
        canvas[(x, 1)] = 'G' if tip > 0.1 else 'g'
        canvas[(x, 2)] = 'g'
        canvas[(x, 3)] = 'g' if dither(x, 3, 0.5) else 'T'
        canvas[(x, 4)] = 'T'
        canvas[(x, 5)] = 't' if dither(x, 5, 0.75) else '>'
        # The lip shades the stone just below it.
        if canvas[(x, 6)] not in '<o':
            canvas[(x, 6)] = DARKEN.get(canvas[(x, 6)], canvas[(x, 6)])
    if seed is None:
        return canvas
    rng = rng_for(seed)
    x = rng.randrange(3, 8)
    while x < TILE - 4:
        length = rng.randrange(2, 11)
        width = 2 if length > 6 and x < TILE - 5 else 1
        for dx in range(width):
            for y in range(6, 6 + length):
                canvas[(x + dx, y)] = 'T' if dx == 0 and y < 5 + length else 't'
            canvas[(x + dx, 6 + length)] = 'g'
        x += width + rng.randrange(2, 7)
    return canvas


def edge(canvas, side, top=0):
    """Finish a wall face: the left face sits in shadow, the right catches the moon."""
    columns = ((0, '-'), (1, '=')) if side == 'left' else ((TILE - 1, ';'), (TILE - 2, ':'))
    for y in range(top, TILE):
        for x, symbol in columns:
            if canvas.get((x, y)) not in (None, '<', 'o'):
                canvas[(x, y)] = symbol
    return canvas


def cap(side, seed):
    """A surface end: turf rolls over the corner and drapes down the face."""
    canvas = turf(deepen(masonry(), fade_from=18), seed)
    edge(canvas, side, top=6)
    rng = rng_for(seed + '-drape')
    outer = [0, 1, 2] if side == 'left' else [TILE - 1, TILE - 2, TILE - 3]
    for depth, x in enumerate(outer):
        for y in range(6, 6 + rng.randrange(4, 12) - depth * 3):
            canvas[(x, y)] = 'T' if depth else 't'
    for x, y in ((outer[0], 0), (outer[1], 0), (outer[0], 1)):
        canvas.pop((x, y))  # round the corner
    canvas[(outer[0], 2)] = 'g'
    canvas[(outer[1], 1)] = 'g'
    return canvas


def deck_ends(canvas):
    """Decks are exactly DECK pixels deep and span the full tile width."""
    rows = {y for _, y in canvas}
    xs = {x for x, _ in canvas}
    assert min(rows) == 0 and max(rows) == DECK - 1 and min(xs) == 0 and max(xs) == TILE - 1
    return canvas


def slab(seed, broken=False):
    canvas = {}
    rng = rng_for(seed)
    for x in range(TILE):
        block_u = x % 32
        for y in range(DECK - 4):
            if block_u == 0:
                symbol = '<'
            elif y == 0:
                symbol = ';'
            elif y == 1:
                symbol = ':'
            elif y in (7, 13):
                symbol = '<'  # carved frieze grooves
            elif y in (8, 14):
                symbol = '-'
            elif y >= DECK - 6:
                symbol = '=' if y == DECK - 6 else '-'
            else:
                symbol = '~' if block_u > 1 else '='
                if unit('slab-speck', seed, x, y) < 0.06:
                    symbol = DARKEN[symbol]
            canvas[(x, y)] = symbol
        # Dentils under the slab, one per 8 pixels.
        if x % 8 in (2, 3, 4, 5):
            for y in range(DECK - 4, DECK):
                canvas[(x, y)] = '=' if y < DECK - 1 else '-'
        elif x % 8 in (1, 6):
            canvas[(x, DECK - 4)] = '<'
    # A carved ring rune between the grooves; intact slabs keep a faint glow.
    for cx in (12, 44):
        for dx, dy in RING:
            canvas[(cx + dx, 10 + dy)] = '<'
        canvas[(cx, 10)] = 'c' if not broken else '='
    if broken:
        # Chipped lip, cracks and missing bites from the underside.
        for x in rng.sample(range(4, TILE - 4), 3):
            canvas[(x, 0)] = ':'
            canvas[(x, 1)] = '='
        for _ in range(2):
            x = rng.randrange(6, TILE - 6)
            for y in range(2, DECK - 5):
                x += rng.choice((-1, 0, 1))
                canvas[(x, y)] = '<'
        for x0, width, depth in ((rng.randrange(8, 20), 7, 6), (rng.randrange(36, 50), 9, 8)):
            for x in range(x0, x0 + width):
                for y in range(DECK - depth + abs(x - x0 - width // 2) // 2, DECK):
                    canvas.pop((x, y), None)
        for x in range(TILE):
            if (x, 0) in canvas and unit('slab-moss', seed, x) < 0.3:
                canvas[(x, 0)] = 'G' if unit('slab-moss-tip', seed, x) < 0.5 else 'g'
    return deck_ends(canvas)


def planks(seed, weathered=False):
    canvas = {}
    rng = rng_for(seed)
    tones = ('-', '=', '~', ':') if weathered else ('/', 'ω', '"', "'")
    dark, mid, light, high = tones
    # Plank ends, 8 pixels apart, some sitting a pixel low.
    for board in range(TILE // 8):
        drop = 1 if weathered and unit('plank-drop', seed, board) < 0.3 else 0
        for u in range(8):
            x = board * 8 + u
            for y in range(drop, 6):
                if u == 7:
                    symbol = dark
                elif y == drop:
                    symbol = high
                elif y == 5:
                    symbol = dark
                elif u == 0:
                    symbol = light
                else:
                    symbol = mid if unit('grain', seed, x, y) > 0.12 else dark
                canvas[(x, y)] = symbol
    # Stringer beam with bolts.
    for x in range(TILE):
        canvas[(x, 6)] = dark
        canvas[(x, 7)] = mid if weathered else 'ω'
        canvas[(x, 8)] = mid if weathered else 'ω'
        canvas[(x, 9)] = dark
        if x % 16 == 4:
            canvas[(x, 7)] = 's'
            canvas[(x, 8)] = 'S'
    if weathered:
        # Broken board end and hanging moss reach the deck's full depth.
        canvas.pop((rng.randrange(20, 44), 0), None)
        for x0 in (rng.randrange(6, 20), rng.randrange(40, 56)):
            for dx, length in enumerate((DECK - 10, DECK - 13, DECK - 15)):
                for y in range(10, 10 + length):
                    canvas[(x0 + dx, y)] = 't' if dx == 1 else 'T'
        for x in range(TILE):
            if x % 32 in (15, 16):
                for y in range(10, 14):
                    canvas[(x, y)] = '/'
    else:
        # A sagging rope, lashed to the beam every 16 pixels.
        for x in range(TILE):
            sag = 10 + round(12 * math.sin(math.pi * x / TILE) ** 2)
            canvas[(x, min(sag, DECK - 1))] = '"'
            canvas[(x, min(sag - 1, DECK - 1))] = "'" if x % 4 else '"'
            if x % 16 == 8:
                for y in range(10, sag - 1):
                    canvas[(x, y)] = '"' if y % 2 else "'"
    return deck_ends(canvas)


def lift_deck():
    canvas = planks('lift-deck')
    for x in range(TILE):
        canvas[(x, 0)] = 'A'  # brass trim marks the moving deck
        if x % 21 == 10:
            for y in range(1, 10):
                canvas[(x, y)] = 's' if y % 3 else 'S'
    for x in range(TILE):
        for y in range(10, DECK):
            canvas.pop((x, y), None)
    # A hanging rune plate between two chains.
    for y in range(10, 15):
        canvas[(26, y)] = 's'
        canvas[(37, y)] = 's'
    for x in range(24, 40):
        for y in range(15, DECK):
            border = x in (24, 39) or y in (15, DECK - 1)
            canvas[(x, y)] = 'o' if border else '/'
    for dx, dy in RING:
        canvas[(31 + dx, 19 + dy)] = 'c'
    canvas[(31, 19)] = 'C'
    return deck_ends(canvas)


def timber_support():
    """Twin posts with one X-brace per tile; stacked tiles read as a trestle."""
    canvas = {}
    for x0 in (16, 42):
        for y in range(TILE):
            for u in range(6):
                symbol = '/' if u == 0 else "'" if u == 5 else '"' if u == 4 else 'ω'
                if u in (2, 3) and unit('post-grain', x0, y // 5) < 0.3:
                    symbol = '/'
                canvas[(x0 + u, y)] = symbol
    for start, end in (((22, 3), (41, 60)), ((41, 3), (22, 60))):
        for step in range(61):
            t = step / 60
            x = round(start[0] + (end[0] - start[0]) * t)
            y = round(start[1] + (end[1] - start[1]) * t)
            for dx in range(-1, 2):
                if 22 <= x + dx <= 41:
                    canvas[(x + dx, y)] = '"' if dx == -1 else 'ω' if dx == 0 else '/'
    for y in (0, 1, 2):
        for x in range(16, 48):
            canvas[(x, y)] = '"' if y == 0 else 'ω' if y == 1 else '/'
    for x, y in ((18, 1), (45, 1), (31, 31), (32, 31)):
        canvas[(x, y)] = 's'
    return canvas


def cliff_back():
    """Wet, darker wall behind the waterfall; streaks repeat every 32 rows."""
    canvas = {p: WET[s] if s in WET else s for p, s in masonry().items()}
    for x in range(TILE):
        if unit('seep', x) < 0.3:
            for y in range(TILE):
                if (y + round(unit('seep-phase', x) * 32)) % 32 < 18 and canvas[(x, y)] not in 'o<':
                    canvas[(x, y)] = '|'
    return canvas


def build_terrain(images, lit):
    for i in range(VARIANTS):
        images[f'rock_{i}'] = to_image(deepen(masonry(f'rock-{i}')))
        images[f'moss_rock_{i}'] = to_image(turf(deepen(masonry(f'moss-stone-{i}'), fade_from=18), f'moss-{i}'))
    images['ground_left'] = to_image(cap('left', 'cap-left'))
    images['ground_right'] = to_image(cap('right', 'cap-right'))
    images['cliff_left'] = to_image(edge(deepen(masonry()), 'left'))
    images['cliff_right'] = to_image(edge(deepen(masonry()), 'right'))
    images['slab_carved'] = to_image(slab('slab-carved'))
    images['slab_broken'] = to_image(slab('slab-broken', broken=True))
    images['bridge_rope'] = to_image(planks('bridge-rope'))
    images['bridge_weathered'] = to_image(planks('bridge-weathered', weathered=True))
    images['lift_deck'] = to_image(lift_deck())
    images['timber_support'] = to_image(timber_support())
    images['cliff_back'] = to_image(cliff_back())
    images['cliff_back_top'] = to_image(turf(cliff_back(), 'cliff-back-top'))
    # The collision tile and the older, unplaced terrain names share the family.
    aliases = {
        'ground': 'moss_rock_0', 'ground_1': 'moss_rock_1', 'ground_2': 'moss_rock_2',
        'ground_3': 'moss_rock_3', 'corner_convex_left': 'ground_left',
        'corner_convex_right': 'ground_right', 'corner_concave_left': 'moss_rock_4',
        'corner_concave_right': 'moss_rock_5', 'stone_wall': 'rock_0', 'fill_1': 'rock_1',
        'fill_2': 'rock_2', 'platform': 'slab_carved', 'platform_1': 'slab_broken',
        'platform_2': 'bridge_weathered', 'stone_platform': 'slab_carved', 'ledge': 'slab_broken',
        'bridge': 'bridge_rope',
    }
    for alias, source in aliases.items():
        images[alias] = images[source].copy()
    build_level_terrain(images)
    for name in images:
        if name.startswith(RELIEF_PREFIXES):
            lit.add(name)


def validate_terrain(images):
    """Variants share every pixel on a side seam; the turf lip and deck depth are fixed."""
    def pixels(name, xs, ys):
        im = images[name]
        return [im.getpixel((x, y)) for y in ys for x in xs]
    seams = [((0, 1, TILE - 2, TILE - 1), range(TILE))]
    for family, lip in (('rock', ()), ('moss_rock', range(6))):
        for i in range(1, VARIANTS):
            for xs, ys in seams + [(range(TILE), lip)]:
                if pixels(f'{family}_{i}', xs, ys) != pixels(f'{family}_0', xs, ys):
                    raise ValueError(f'{family}_{i}: pixels on a tile seam differ from {family}_0')
    for name in ('slab_carved', 'slab_broken', 'bridge_rope', 'bridge_weathered', 'lift_deck'):
        if images[name].getbbox() != (0, 0, TILE, DECK):
            raise ValueError(f'{name}: deck must span the tile and be {DECK} pixels deep')
    # M76's families: a mask's variants share both side seams (the rocks'
    # outer facets, the cladding's crossing plate), so any two meet.
    groups = [[f'under_{i}' for i in range(VARIANTS)], [f'basalt_{i}' for i in range(BASALT_VARIANTS)],
              [f'basalt_top_{i}' for i in range(4)], [f'basalt_under_{i}' for i in range(4)]]
    for family, masks, count in (('wet_rock', MASKS, ROCK_VARIANTS), ('mine_rock', MASKS, ROCK_VARIANTS),
                                 ('iron_clad', IRON_MASKS, ROCK_VARIANTS), ('wet_back', MASKS, 3),
                                 ('cut_back', MASKS, 2), ('cut_back_iron', IRON_MASKS, 2)):
        groups += [[f'{family}_{mask}_{n}' for n in range(count)] for mask in masks]
    for names in groups:
        for name in names[1:]:
            if pixels(name, seams[0][0], range(TILE)) != pixels(names[0], seams[0][0], range(TILE)):
                raise ValueError(f'{name}: pixels on a tile seam differ from {names[0]}')


# ---------------------------------------------------------------------------
# M76 (01 (d)): the new level's terrain, drawn as the art review's sixth draw
# (`perf-runs/20261021-e01d-art-review/`) drew them: the masonry's underside,
# the pit's lining, the aqueduct's channel, columnar basalt, the ravine's wet
# rock, the mine's rock and its iron cladding by open sides, the borders where
# two families meet and the troll's bridge deck.

# Each mask-drawn family takes one tile per open-side mask (bit 1 north, 2
# east, 4 south, 8 west) and variant.
MASKS = range(16)
ROCK_VARIANTS = 4
BASALT_VARIANTS = 16
SYMBOL = {rgba: symbol for symbol, rgba in PALETTE.items()}


def from_image(im):
    """A palette image back into a canvas of symbols."""
    px = im.load()
    return {(x, y): SYMBOL[px[x, y]] for y in range(im.height) for x in range(im.width) if px[x, y][3]}


def sides(mask):
    """A mask's open sides as (north, east, south, west)."""
    return tuple(bool(mask & bit) for bit in (1, 2, 4, 8))


def taper(x, amount):
    """`amount` faded to nothing at a tile's two sides, so a variant's ragged
    edge meets every other variant's at the seam."""
    return amount * math.sin(math.pi * x / (TILE - 1))


def rock_canvas(i):
    """Today's masonry fill, `rock_<i>`, as a canvas."""
    return deepen(masonry(f'rock-{i % VARIANTS}'))


def moss_canvas(i):
    """Today's masonry surface, `moss_rock_<i>`, as a canvas."""
    return turf(deepen(masonry(f'moss-stone-{i % VARIANTS}'), fade_from=18), f'moss-{i % VARIANTS}')


def ellipse(c, cx, cy, rx, ry, pick):
    for dy in range(-ry, ry + 1):
        for dx in range(-rx, rx + 1):
            if rx > 0 and ry > 0 and (dx * dx) / (rx * rx) + (dy * dy) / (ry * ry) <= 1.0:
                s = pick(dx, dy)
                if s:
                    c[(cx + dx, cy + dy)] = s


def rect(c, x0, y0, x1, y1, pick):
    for y in range(y0, y1):
        for x in range(x0, x1):
            s = pick(x, y)
            if s:
                c[(x, y)] = s


def line(c, x0, y0, x1, y1, sym, w=1):
    n = max(abs(x1 - x0), abs(y1 - y0), 1)
    for k in range(n + 1):
        x = round(x0 + (x1 - x0) * k / n)
        y = round(y0 + (y1 - y0) * k / n)
        for d in range(w):
            c[(x + d, y)] = sym


def crack(canvas, seed, start, steps, down=True, hot=0.18, width=1):
    """A glowing ember crack: a jagged walk with hot cores and dark rims."""
    x, y = start
    for k in range(steps):
        r = unit(seed, k)
        x += (-1 if r < 0.33 else 1 if r > 0.66 else 0)
        y += 1 if down else (-1 if r > 0.5 else 1)
        x, y = x % TILE, y % TILE
        heat = unit(seed, 'heat', k)
        core = 'Y' if heat < hot * 0.35 else 'y' if heat < hot else 'J' if heat < 0.55 else 'K'
        for dx in range(width):
            canvas[((x + dx) % TILE, y)] = core
        for nx, ny in ((x - 1, y), (x + width, y)):
            if canvas.get((nx % TILE, ny)) not in ('Y', 'y', 'J', 'K'):
                canvas[(nx % TILE, ny)] = 'L' if unit(seed, 'rim', k) < 0.6 else 'M'


def coursed(x, y, seed, course=16, block=32):
    """Dressed stone in courses at absolute pixel `(x, y)`, so pieces cut from
    one drawing join: joints dark, each block lit along its top."""
    row = y // course
    u = (x + (0 if row % 2 == 0 else block // 2)) % block
    v = y % course
    if u == 0 or v == 0:
        return '<'
    if v == 1:
        return ';' if dither(x, y, 0.55) else ':'
    if u == block - 1 or v == course - 1:
        return '-'
    n = unit(seed, (x + (0 if row % 2 == 0 else block // 2)) // block, row, x // 5, y // 4)
    return ':' if n < 0.6 else '~' if n < 0.93 else ';'


BACK_DARK = {';': ':', ':': '~', '~': '=', '=': '-', '-': '<', '<': 'o', 'w': 'σ', 'σ': 'τ', 'τ': 'S',
             'Ω': 'B', 'B': 'Φ', 'Φ': 'b', 'b': 'i', 'i': 'o', 'χ': 'ψ', 'ψ': 'Ω', 'φ': 'χ', 'a': '(', '(': '>', 'A': 'a',
             'T': 't', 'g': 'T', 'G': 'g', 'U': 'P', 'W': 'U', ',': ')', '`': ',', '_': '}',
             'η': '=', 'd': 'n', 'n': 'o', 'Q': 'D', 'D': 'P', 'O': 'n', '|': 'O', ')': '{', '}': ')',
             'm': 'S', "'": '"', '"': 'ω', 'ω': '/'}


def darker(c, steps=1):
    """A background depth's piece: every symbol one step down its ramp."""
    for _ in range(steps):
        c = {p: BACK_DARK.get(s, s) for p, s in c.items()}
    return c


# A far wall's tint, as the review drew the gorge's far wall and the cut's far
# terraces (each channel out of 255), and the far scaffold's, a step deeper.
FAR_TINT = (104, 112, 136)
FAR_PROP_TINT = (90, 96, 120)


@functools.cache
def tint_map(tint):
    """Each opaque symbol to the opaque one nearest its colour times `tint`:
    a tint kept in palette pixels. True black and the emissive symbols are
    never chosen, so a tinted piece neither reads as the night sky nor
    glows."""
    targets = {s: v for s, v in PALETTE.items() if v[3] == 255 and s not in '■yYCc'}
    out = {}
    for s, v in PALETTE.items():
        if v[3] != 255:
            continue
        want = [c * t / 255 for c, t in zip(v[:3], tint)]
        out[s] = min(targets, key=lambda o: sum((a - b) ** 2 for a, b in zip(targets[o][:3], want)))
    return out


def tinted(c, tint=FAR_TINT):
    """Canvas `c` under `tint` (`tint_map`); translucent symbols stay."""
    table = tint_map(tint)
    return {p: table.get(s, s) for p, s in c.items()}


def under(i):
    """Masonry underside, four kinds: roots, a moss curtain, a wet seep and
    lime drips, each hanging from a shadowed lip."""
    c = rock_canvas(i)
    edges = {}
    for x in range(TILE):
        edge = 55 + round(2.2 * math.sin(math.tau * 2 * x / TILE) + taper(x, 1.4 * math.sin(math.tau * 5 * x / TILE + 2.1 * i)))
        edges[x] = edge
        for y in range(edge + 1, TILE):
            c.pop((x, y), None)
        c[(x, edge)] = 'o'
        c[(x, edge - 1)] = '<'
        for y in range(edge - 9, edge - 1):
            if (x, y) in c and dither(x, y, (y - (edge - 9)) / 9):
                c[(x, y)] = DARKEN.get(c[(x, y)], c[(x, y)])
    kind = i % 4
    if kind in (0, 1):
        for k in range(5 if kind == 0 else 9):
            x = int(unit('root', i, k) * 60) + 2
            length = (6 + int(unit('rootlen', i, k) * 12)) if kind == 0 else (8 + int(unit('mosslen', i, k) * 9))
            moss = kind == 1 or unit('moss', i, k) < 0.45
            top = edges[x] - 3
            for y in range(top, min(TILE, top + length)):
                sway = round(math.sin(y * 0.45 + k) * 0.8)
                if moss:
                    c[(x + sway, y)] = 'T' if y < top + length - 2 else 'G'
                    if y % 3 == 0:
                        c[(x + sway + 1, y)] = 't' if kind == 0 else 'g'
                else:
                    c[(x + sway, y)] = '/' if y % 2 else 'ω'
                    if y > top + length - 3:
                        c[(x + sway, y)] = '"'
    elif kind == 2:
        x0 = 18 + int(unit('seep', i) * 26)
        for y in range(20, TILE):
            for dx in range(3):
                if (x0 + dx, y) in c and dither(x0 + dx, y, 0.75):
                    c[(x0 + dx, y)] = 'o' if dx == 1 else '<'
        for k, y in enumerate(range(edges[x0 + 1] + 2, TILE, 3)):
            c[(x0 + 1, y)] = '`' if k % 2 else ','
    else:
        for k in range(4):
            x = 6 + int(unit('lime', i, k) * 52)
            length = 4 + int(unit('limelen', i, k) * 6)
            top = edges[x]
            for d in range(length):
                w = max(0, (length - d) // 3)
                for dx in range(-w, w + 1):
                    c[(x + dx, top + d)] = 'w' if dx < 0 else 'τ' if dx > 0 else 'σ'
    return to_image(c)


def ashlar(seed, course=16, lit_side=None):
    """Dressed stone in courses: the pit's lining and its floor."""
    c = {}
    for y in range(TILE):
        row = y // course
        off = 0 if row % 2 == 0 else 16
        for x in range(TILE):
            u = (x + off) % 32
            v = y % course
            if u == 0 or v == 0:
                c[(x, y)] = '<'
            elif u == 1 or v == 1:
                c[(x, y)] = ';' if dither(x, y, 0.7) else ':'
            elif u == 31 or v == course - 1:
                c[(x, y)] = '-'
            else:
                n = unit(seed, (x + off) // 32, row, x % 4, y % 3)
                c[(x, y)] = ':' if n < 0.55 else '~' if n < 0.9 else ';'
    if lit_side == 'right':
        for y in range(TILE):
            c[(TILE - 1, y)], c[(TILE - 2, y)] = ';', ':'
    elif lit_side == 'left':
        for y in range(TILE):
            c[(0, y)], c[(1, y)] = '-', '='
    return c


def pit_wall(side, kind='plain', seed=0):
    c = ashlar(f'pit-{side}-{seed}', lit_side='right' if side == 'left' else 'left')
    cx = 40 if side == 'left' else 24
    if kind == 'ring':
        for a in range(64):
            t = math.tau * a / 64
            x, y = round(cx + 7 * math.cos(t)), round(39 + 7 * math.sin(t))
            c[(x, y)] = 'ψ' if math.sin(t) < 0 else 'Ω'
        rect(c, cx - 2, 28, cx + 3, 33, lambda x, y: 'χ' if (x, y) == (cx - 1, 29) else 'Φ')
    elif kind == 'cracked':
        x, y = 10 + seed % 20, 6
        for k in range(40):
            x += 1 if unit('pc', seed, k) > 0.45 else 0
            y += 1
            c[(x % TILE, y % TILE)] = '<'
            c[((x + 1) % TILE, y % TILE)] = '-'
    elif kind == 'niche':
        for y in range(22, 42):
            for x in range(cx - 8, cx + 9):
                top = 22 + round(8 - math.sqrt(max(0, 64 - (x - cx) ** 2)))
                if y >= top:
                    c[(x, y)] = '<' if y > top + 1 else '-'
        for y in range(16, 22):
            for x in range(cx - 3, cx + 4):
                if dither(x, y, 0.4):
                    c[(x, y)] = '-'
        rect(c, cx - 2, 35, cx + 2, 41, lambda x, y: 'w' if x < cx else 'σ')
        c[(cx - 1, 31)], c[(cx, 31)] = 'J', 'K'
        c[(cx - 1, 32)], c[(cx, 32)] = 'Y', 'y'
        c[(cx - 1, 33)], c[(cx, 33)] = 'y', 'J'
        c[(cx - 1, 34)] = 'J'
    elif kind == 'mossy':
        for (x, y), s in list(c.items()):
            if s in '<-' and unit('pmoss', seed, x // 3, y // 2) < 0.4:
                c[(x, y)] = 'T' if unit('pm2', x, y) < 0.6 else 't'
    return to_image(c)


def pit_rim(side):
    c = ashlar(f'rim-{side}', lit_side='right' if side == 'left' else 'left')
    lip = moss_canvas(0)
    for (x, y), sym in lip.items():
        if y < 7:
            c[(x, y)] = sym
    outer = [TILE - 1, TILE - 2, TILE - 3] if side == 'left' else [0, 1, 2]
    for depth, x in enumerate(outer):
        for y in range(6, 6 + int(unit('drape', side, depth) * 10) + 4 - depth * 2):
            c[(x, y)] = 'T' if depth else 't'
    return to_image(c)


def pit_floor(i):
    c = ashlar(f'floor-{i}', course=22)
    for x in range(TILE):
        c[(x, 0)] = ';'
        c[(x, 1)] = ':' if dither(x, 1, 0.5) else ';'
    if i % 3 == 1:
        rect(c, 20, 6, 44, 16, lambda x, y: 'o' if (x - 20) % 4 < 2 else 'Ω' if y < 11 else 'Φ')
        rect(c, 19, 5, 45, 6, lambda x, y: 'ψ')
    return to_image(c)


def channel(i, broken=None, kind='plain'):
    c = rock_canvas(i + 3)
    for x in range(TILE):
        wave = math.sin(math.tau * (x / 32) + i) * 0.8
        surface = 2 + round(wave)
        for y in range(0, 16):
            if y < surface:
                c.pop((x, y), None)
            elif y == surface:
                c[(x, y)] = '`' if unit('glint', i, x // 3) < 0.3 else '_'
            elif y < 7:
                c[(x, y)] = '}' if dither(x, y, 0.55) else ')'
            elif y < 11:
                c[(x, y)] = '{' if dither(x, y, 0.5) else '|'
            elif y == 11:
                c[(x, y)] = ';'
            elif y == 12:
                c[(x, y)] = ':'
            else:
                c[(x, y)] = '<' if x % 32 == 0 else '-'
    if kind == 'mossy':
        for x in range(TILE):
            if unit('lipmoss', i, x // 3) < 0.55:
                for y in range(11, 13 + int(unit('lm', i, x) * 6)):
                    c[(x, y)] = 'T' if y < 14 else 't'
                if unit('fern', i, x // 7) < 0.2:
                    for y in range(5, 11):
                        c[(x, y)] = 'G' if y % 2 else 'g'
    elif kind == 'spout':
        for y in range(11, TILE):
            for x in range(26, 38):
                flow = math.sin((y * 0.7) + x * 0.9)
                c[(x, y)] = '_' if (x in (26, 37) and dither(x, y, 0.4)) else '}' if flow > 0.4 else ')' if flow > -0.3 else '{'
        for x in range(24, 40):
            c[(x, 11)] = '_'
    elif kind == 'cracked':
        for x in range(14, 30):
            for y in range(11, 14):
                c.pop((x, y), None)
        for x in range(14, 30):
            c[(x, 14)] = '<'
        c[(31, 18)], c[(32, 19)], c[(33, 21)] = '<', '<', '<'
    if broken:
        for y in range(0, TILE):
            for x in (range(48, TILE) if broken == 'right' else range(0, 16)):
                edge = 48 + round(6 * math.sin(y * 0.3)) if broken == 'right' else 16 - round(6 * math.sin(y * 0.3))
                if (broken == 'right' and x > edge) or (broken == 'left' and x < edge):
                    c.pop((x, y), None)
    return to_image(c)


def channel_break(i, side='right'):
    """A channel's broken end: the facing gone block by block along a stepped
    edge, the rubble core showing as rounded stones in dark mortar, the
    trough's water spilling over the break."""
    c = from_image(channel(i))
    sign = 1 if side == 'right' else -1

    def edge(y):
        step = int(unit('breakstep', i, y // 8) * 14)
        return 40 + step if side == 'right' else 23 - step
    for y in range(TILE):
        for x in range(TILE):
            if sign * (x - edge(y)) > 0:
                c.pop((x, y), None)
    for y in range(11, TILE):
        for d in range(11):
            x = edge(y) - sign * d
            if (x, y) in c:
                c[(x, y)] = 'o' if d == 0 else '<'
    for k in range(30):
        y0 = 13 + int(unit('cs', i, k) * 49)
        x0 = edge(y0) - sign * (3 + int(unit('cd', i, k) * 7))
        r = 1 + int(unit('cr', i, k) * 3)
        for dy in range(-r, r + 1):
            for dx in range(-r, r + 1):
                if dx * dx + dy * dy <= r * r + 1 and (x0 + dx, y0 + dy) in c and c[(x0 + dx, y0 + dy)] == '<':
                    c[(x0 + dx, y0 + dy)] = ';' if dx + dy <= -r else ':' if dx + dy < r // 2 + 1 else '-'
    e = edge(3)
    for y in range(2, 13):
        c[(e + sign, y)] = '`' if y % 4 == 0 else '}'
        c[(e + 2 * sign, y + 1)] = ')'
    return to_image(c)


WIDTHS = ((16, 16, 16, 16), (12, 20, 16, 16), (20, 12, 20, 12), (16, 12, 16, 20))


def basalt(i, part='body'):
    """Columnar basalt: columns 12-20 px wide, staggered joints; a variant in
    four: plain, an ember nodule, an obsidian streak, ash in the joints."""
    c = {}
    x0 = 0
    for col, width in enumerate(WIDTHS[i % 4]):
        joints = sorted((int(unit('joint', i, col, k) * 22) + k * 32) % TILE for k in range(2))
        for x in range(x0, x0 + width):
            u = (x - x0) / max(1, width - 1)
            for y in range(TILE):
                n = unit('bas', i, col, x // 2, y // 3)
                if x == x0:
                    sym = 'θ'
                elif u < 0.22:
                    sym = 'α' if n < 0.7 else 'ξ'
                elif u < 0.7:
                    sym = 'β' if n < 0.6 else 'γ'
                else:
                    sym = 'ν' if n < 0.7 else 'γ'
                if y in joints:
                    sym = 'ε'
                elif (y - 1) % TILE in joints and u < 0.7:
                    sym = 'ξ' if dither(x, y, 0.5) else 'α'
                if (i // 4) % 4 == 3 and (y in joints or (y + 1) % TILE in joints) and unit('ash', i, x // 2) < 0.6:
                    sym = 'Φ' if dither(x, y, 0.5) else 'Ω'
                c[(x, y)] = sym
        x0 += width
    v = (i // 4) % 4
    if v == 1:
        cx, cy = 14 + int(unit('nod', i) * 34), 14 + int(unit('nody', i) * 34)
        ellipse(c, cx, cy, 5, 4, lambda dx, dy: 'M' if abs(dx) > 3 or abs(dy) > 2 else 'L')
        ellipse(c, cx, cy, 3, 2, lambda dx, dy: 'K' if abs(dx) > 1 else 'J')
        c[(cx, cy)], c[(cx - 1, cy)] = 'Y', 'y'
    elif v == 2:
        sx = 10 + int(unit('obs', i) * 40)
        for y in range(8, 56):
            sx += 1 if unit('obsx', i, y) > 0.8 else -1 if unit('obsx', i, y) < 0.15 else 0
            c[(sx, y)], c[(sx + 1, y)] = '0', '2'
            if y % 9 == 0:
                c[(sx, y)] = 'π'
    if unit('crack?', i) < 0.16:
        crack(c, ('crack', i), (int(unit('cx', i) * 60), int(unit('cy', i) * 30)),
              10 + int(unit('cl', i) * 16), hot=0.22)
    if part == 'top':
        for x in range(TILE):
            depth = 5 + round(taper(x, 1.5 * math.sin(math.tau * 3 * x / TILE + i)))
            for y in range(depth):
                c[(x, y)] = 'Ω' if y == 0 else 'Φ' if dither(x, y, 0.6) else 'Ω'
            if i % 3 == 1:
                ripple = round(1.2 * math.sin(x * 0.55 + i))
                c[(x, depth + 1 + ripple)] = 'α'
            if unit('ember-pocket', i, x // 5) < 0.12:
                c[(x, depth)] = 'J'
                c[(x, depth - 1)] = 'K'
    elif part == 'under':
        for x in range(TILE):
            edge = 54 + round(taper(x, 2 * math.sin(math.tau * 3 * x / TILE + i)))
            for y in range(edge, TILE):
                c.pop((x, y), None)
            c[(x, edge - 1)] = 'θ'
        for k in range(4):
            x = int(unit('drip', i, k) * 56) + 4
            length = 5 + int(unit('dl', i, k) * 8)
            for y in range(52, 52 + length):
                w = max(0, (length - (y - 52)) // 4)
                for dx in range(-w, w + 1):
                    c[(x + dx, y)] = 'γ' if dx < 0 else 'ν'
            c[(x, 52 + length)] = 'K' if k % 2 else 'ν'
    elif part in ('cliff_left', 'cliff_right'):
        cols = ((0, 'θ'), (1, 'ν')) if part == 'cliff_left' else ((TILE - 1, 'ξ'), (TILE - 2, 'α'))
        for y in range(TILE):
            for x, sym in cols:
                c[(x, y)] = sym
    return to_image(c)


def border(left, right, seed):
    a, b = left.load(), right.load()
    im = Image.new('RGBA', (TILE, TILE))
    px = im.load()
    for y in range(TILE):
        split = 32 + round(7 * math.sin(y * 0.31 + unit(seed) * 6) + 4 * math.sin(y * 0.83 + 1))
        for x in range(TILE):
            px[x, y] = a[x, y] if x < split else b[x, y]
        px[split, y] = tuple(PALETTE['θ'])
        if unit(seed, y) < 0.3:
            px[split - 1, y] = tuple(PALETTE['ε'])
    return im


def bridge_deck(i):
    """The troll's bridge's deck: paving setts lit along their tops, a
    cornice with dentils, coursed spandrel below."""
    c = {}
    for y in range(TILE):
        for x in range(TILE):
            if y == 0:
                sym = ';'
            elif y < 6:
                sym = '<' if (x + (4 if y > 2 else 0)) % 9 == 0 or y == 5 else ':' if unit('sett', i, x // 9, y // 3) < 0.7 else '~'
            elif y == 6:
                sym = '<'
            elif y == 7:
                sym = ';'
            elif y < 11:
                sym = ':' if y < 10 else '~'
            elif y == 11:
                sym = '-'
            elif y < 14:
                sym = '-' if x % 6 < 3 else '<'
            else:
                sym = coursed(x, y, ('deck', i))
            c[(x, y)] = sym
    for x in range(TILE):
        if unit('dmoss', i, x // 3) < 0.25:
            c[(x, 5)] = 'T'
    if i % 3 == 1:
        for k in range(3):
            x = int(unit('dlime', i, k) * 60)
            for y in range(14, 14 + int(unit('dll', i, k) * 30)):
                if dither(x, y, 0.8):
                    c[(x, y)] = 'σ'
    return to_image(c)


LEAF = dict(base='T', light='g', shadow='t', back='N', rim='G')


FROND = dict(base='U', light='W', shadow='P', back='P', rim='G')


SOFT_BEDS = ((11, 21), (44, 54))


ROCK_KINDS = ((0.36, 'plain'), (0.50, 'sheen'), (0.76, 'moss'), (0.86, 'fern'), (0.92, 'roots'), (1.0, 'lichen'))


ROCK_RAMP = 'nd=η~:;'


GRID_X, GRID_Y, SQUASH = 3, 5, 1.9


LEDGES = (21, 47)


def ledge_y(k, x):
    """Where bedding ledge `k` crosses column `x`: a wave with the tile's
    period, so ledges run on from tile to tile."""
    return LEDGES[k] + round(1.6 * math.sin(math.tau * x / TILE * (k + 1) + 0.9 * k))


@functools.lru_cache(maxsize=None)
def rock_seeds(base):
    """The rock's facets: a jittered 3 x 4 grid of centres. The outer ring's
    jitter is the same in every variant and kept near the sides, so the
    pixels along each side belong to shared facets and any two variants
    join; the inner two move with `base`. Each facet leans its own way, which
    sets how much moonlight it takes."""
    grid = {}
    for gy in range(GRID_Y):
        for gx in range(GRID_X):
            outer = gx in (0, GRID_X - 1) or gy in (0, GRID_Y - 1)
            key = ('rock-edge', gx, gy) if outer else ('rock-in', base, gx, gy)
            spread = 3.0 if outer else 6.0
            x = (gx + 0.5) * TILE / GRID_X + (unit(key, 'x') - 0.5) * 2 * spread
            y = (gy + 0.5) * TILE / GRID_Y + (unit(key, 'y') - 0.5) * 2 * spread
            lean = unit(key, 'lean') * math.tau
            facing = 3.1 + 1.3 * (0.6 * math.cos(lean) - 0.8 * math.sin(lean))
            if any(a <= y < b for a, b in SOFT_BEDS):
                facing -= 0.8
            grid[(gx, gy)] = (x, y, facing, key)
    return grid


@functools.lru_cache(maxsize=None)
def rock_base(base):
    """The rock for one of six bases: flat facets, each lit by how it leans,
    crisp at their cracks (a lit lip on their upper and moonward rims, a
    shadow on the others), grained inside; two bedding ledges cross every
    tile, lit along their tops. Each pixel keeps its symbol, how near it
    lies to its facet's rim and which way it faces from the facet's
    centre."""
    grid = rock_seeds(base)
    out = {}
    for y in range(TILE):
        for x in range(TILE):
            gx, gy = int(x * GRID_X / TILE), int(y * GRID_Y / TILE)
            near = []
            for dgx in (-1, 0, 1):
                for dgy in (-1, 0, 1):
                    ngx, ngy = gx + dgx, gy + dgy
                    sx, sy, facing, key = grid[(ngx % GRID_X, ngy % GRID_Y)]
                    sx, sy = sx + (ngx // GRID_X) * TILE, sy + (ngy // GRID_Y) * TILE
                    near.append((math.hypot(x - sx, (y - sy) * SQUASH), sx, sy, facing, key))
            near.sort(key=lambda n: n[0])
            (d1, sx, sy, facing, key), d2 = near[0], near[1][0]
            gap = d2 - d1
            rim = max(0.0, 1 - gap / 3.5)
            vx, vy = x - sx, (y - sy) * SQUASH
            norm = abs(vx) + abs(vy) + 1e-6
            fx, fy = vx / norm, vy / norm
            noise = unit('rock-grain', key, x // 2, y // 3)
            if gap < 1.1:
                sym = 'n'
            elif rim > 0.35:
                sym = (';' if rim > 0.7 else ':') if (fy < -0.3 or fx > 0.45) else ('d' if rim > 0.7 else '=')
            else:
                level = facing + (noise - 0.5) * 0.9
                sym = ROCK_RAMP[max(1, min(5, round(level)))]
                if noise < 0.03:
                    sym = 'd'
            out[(x, y)] = (sym, rim, fy, key)
    for k in range(len(LEDGES)):
        for x in range(TILE):
            y = ledge_y(k, x)
            sym, rim, fy, key = out[(x, y)]
            out[(x, y - 1)] = ('d', 1.0, 1.0, key)
            out[(x, y)] = ('n', 1.0, 0.0, key)
            out[(x, y + 1)] = (';' if dither(x, y, 0.6) else ':', 1.0, -1.0, key)
            if (x, y + 2) in out and dither(x, y + 2, 0.5):
                out[(x, y + 2)] = (':', 0.8, -1.0, out[(x, y + 2)][3])
    return out


def rock_edges(open_):
    """Each pixel's depth into the rock from its open sides, and the side it
    faces; corners round where two open sides meet. Depth below 0 is air."""
    n, e, s, w = open_
    radius = 10
    out = {}
    for y in range(TILE):
        for x in range(TILE):
            d = {}
            if n:
                d['n'] = y - (5 + round(1.4 * math.sin(math.tau * 2 * x / TILE + 0.5) + 0.8 * math.sin(math.tau * 5 * x / TILE)))
            if s:
                d['s'] = 57 + round(2.0 * math.sin(math.tau * 2 * x / TILE + 1.3) + 1.2 * math.sin(math.tau * 5 * x / TILE + 2.0)) - y
            if w:
                d['w'] = x - (3 + round(2.0 * abs(math.sin(math.pi * y / 32 + 0.7)) + 1.2 * math.sin(math.tau * 3 * y / TILE)))
            if e:
                d['e'] = 60 - round(2.0 * abs(math.sin(math.pi * y / 32 + 1.9)) + 1.2 * math.sin(math.tau * 3 * y / TILE + 1.0)) - x
            if not d:
                continue
            facing = min(d, key=d.get)
            depth = d[facing]
            for a, b in (('n', 'e'), ('n', 'w'), ('s', 'e'), ('s', 'w')):
                if a in d and b in d and d[a] < radius and d[b] < radius:
                    corner = radius - math.hypot(radius - d[a], radius - d[b])
                    if corner < depth:
                        depth, facing = corner, (a if d[a] <= d[b] else b)
            out[(x, y)] = (depth, facing)
    return out


def fern_into(c, i, fx, fy, fronds=6, reach=11, up=True):
    """A fern's fronds from `(fx, fy)` into canvas `c`, pinnae on each frond,
    arching and drooping at their tips."""
    for f in range(fronds):
        a = math.pi * (0.1 + 0.8 * f / max(1, fronds - 1)) + (unit('fern-a', i, f) - 0.5) * 0.25
        if not up:
            a = -a
        length = reach * (0.6 + 0.4 * unit('fern-l', i, f))
        for d in range(int(length)):
            t = d / length
            x = fx + round(math.cos(a) * d * 1.15)
            y = fy - round(math.sin(a) * d) + round(t * t * length * 0.45)
            c[(x, y)] = 'P' if d < 2 else 'U'
            if d > 1 and d % 2 == 0:
                side = 1 if math.cos(a) >= 0 else -1
                c[(x + side, y + 1)] = 'W' if t < 0.5 and dither(x, y, 0.5) else 'U'
                c[(x - side, y - 1)] = 'U' if dither(x, y, 0.6) else 'P'


def wet_rock(i, open_=(False, False, False, False), kind=None, back=0):
    """The ravine's rock: beds of rounded blocks in wet teal-grey stone, each
    block lit along its top and moonward side and shadowed beneath, the
    soft beds recessed and laminated. Kinds: plain, a wet sheen, moss pads,
    a fern pocket, roots, lichen. Its edges follow its open sides
    (`open_` is north, east, south, west): a mossy turf on top with blades
    and sprigs, a moonlit face east, a shadowed face west, a dripping
    underside with moss strands, corners rounded where two meet. `back`
    darkens it for the gorge's far wall."""
    if kind is None:
        r = unit('rock-kind', i)
        kind = next(name for limit, name in ROCK_KINDS if r < limit)
    base = rock_base(i % 6 if isinstance(i, int) else unit('rock-base', i) * 6 // 1)
    c = {p: v[0] for p, v in base.items()}
    if kind == 'sheen':
        x0, wid = 12 + int(unit('sheen', i) * 34), 6 + int(unit('sheen-w', i) * 5)
        for y in range(TILE):
            fade = min(1.0, (y + 4) / 12, (TILE + 3 - y) / 12)
            for x in range(x0, x0 + wid):
                core = abs(x - (x0 + wid / 2)) < wid / 4
                s0 = c.get((x, y))
                if s0 in (';', ':'):
                    c[(x, y)] = '`' if core and dither(x, y, 0.6 * fade) else ','
                elif s0 == 'n':
                    c[(x, y)] = 'O'
                elif dither(x, y, 0.65 * fade):
                    c[(x, y)] = '|' if core else 'O'
    elif kind == 'moss':
        for (x, y), (sym, rim, fy, key) in base.items():
            if sym != 'n' and rim > 0.45 and fy < -0.45 and unit('moss', i, key) < 0.6:
                c[(x, y)] = 'U' if rim > 0.85 and dither(x, y, 0.5) else 'Q' if rim > 0.65 else 'D'
                if rim > 0.8 and unit('drape', i, x) < 0.12:
                    for d in range(1, 3 + int(unit('drape-l', i, x) * 5)):
                        c[(x, y + d)] = 'P' if d % 2 else 'D'
    elif kind == 'fern':
        fx, fy = 18 + int(unit('fern-x', i) * 26), 36
        for x in range(fx - 3, fx + 4):
            c[(x, fy + 1)], c[(x, fy + 2)] = 'n', 'o'
        fern_into(c, i, fx, fy)
    elif kind == 'roots':
        for k in range(2):
            x = 12 + int(unit('root', i, k) * 40)
            for y in range(TILE):
                x += -1 if unit('root-x', i, k, y) < 0.18 else 1 if unit('root-x', i, k, y) > 0.82 else 0
                c[(x, y)], c[(x + 1, y)] = '"' if y % 4 else "'", 'ω'
                c[(x + 2, y)] = '/'
                if unit('rootlet', i, k, y) < 0.08:
                    for d in range(1, 5):
                        c[(x + 2 + d, y + d // 2)] = 'ω'
    elif kind == 'lichen':
        for k in range(5):
            cx, cy = 6 + int(unit('lichen', i, k) * 52), 6 + int(unit('lichen-y', i, k) * 52)
            for d in range(9):
                px, py = cx + int(unit('lc', i, k, d) * 6) - 3, cy + int(unit('lcy', i, k, d) * 4) - 2
                if (px, py) in c and c[(px, py)] != 'n':
                    c[(px, py)] = 'm' if d % 3 == 0 else 'S' if d % 3 == 1 else 's'
    edges = rock_edges(open_)
    under_edge = {}
    for (x, y), (depth, facing) in edges.items():
        if facing == 's' and depth == 0:
            under_edge[x] = y
        if depth < 0:
            c.pop((x, y), None)
            continue
        if facing == 'n':
            if depth <= 6:
                c[(x, y)] = ('G' if dither(x, y, 0.4) else 'U') if depth == 0 else 'U' if depth == 1 and dither(x, y, 0.5) \
                    else 'Q' if depth <= 2 else 'D' if depth <= 4 else 'P' if depth == 5 else 'n'
        elif facing == 'e':
            if depth <= 2:
                c[(x, y)] = 'n' if depth == 0 else ';' if depth == 1 else (':' if dither(x, y, 0.6) else c[(x, y)])
        elif facing == 'w':
            if depth <= 3:
                c[(x, y)] = 'o' if depth == 0 else 'n' if depth == 1 else 'd' if depth == 2 else ('=' if dither(x, y, 0.5) else c[(x, y)])
        elif depth <= 3:
            c[(x, y)] = 'o' if depth == 0 else 'n' if depth == 1 else 'd' if depth == 2 else ('d' if dither(x, y, 0.5) else c[(x, y)])
    if open_[0]:
        for x in range(TILE):
            if unit('blade', i, x) < 0.3:
                top = next((y for y in range(TILE) if (x, y) in c), None)
                if top is not None and top < 12:
                    for d in range(1, 2 + int(unit('blade-h', i, x) * 4)):
                        c[(x, top - d)] = 'W' if d >= 3 else 'G' if d == 2 else 'U'
        for f in range(1 if unit('sprig', i) < 0.5 else 0):
            fern_into(c, ('top', i), 14 + int(unit('sprig-x', i) * 36), 6, fronds=5, reach=8)
    if open_[2]:
        for k in range(6):
            x = 4 + int(unit('strand', i, k) * 56)
            if x not in under_edge:
                continue
            y0 = under_edge[x]
            if unit('strand-kind', i, k) < 0.55:
                for d in range(1, 4 + int(unit('strand-l', i, k) * 9)):
                    c[(x + round(math.sin(d * 0.5 + k) * 0.6), y0 + d)] = 'Q' if d < 3 else 'P' if d % 2 else 'D'
            else:
                for d in range(2, 8, 2 + int(unit('drip', i, k) * 2)):
                    c[(x, y0 + d)] = ',' if d % 4 else '`'
    return to_image(darker(c, back) if back else c)


MINE_ROCK = {'n': 'o', 'd': 'i', '=': 'b', 'η': 'Φ', '~': 'Φ', ':': 'B', ';': 'Ω'}


def mine_rock(i, open_=(False, False, False, False), back=0):
    """The cut's rock: the ravine's facets in the mine's dry greys, an ore
    vein rusting through them with bright flecks, now and then a coal seam
    along a bedding ledge; on top a crust of spoil (gravel, broken stone,
    rusted ore) with loose stones sitting on it. Its faces follow `open_`
    as the ravine's rock does."""
    base = rock_base(i % 6 if isinstance(i, int) else 0)
    c = {p: MINE_ROCK.get(v[0], v[0]) for p, v in base.items()}
    if unit('vein', i) < 0.55:
        x, y = 4 + int(unit('vein-x', i) * 20), 14 + int(unit('vein-y', i) * 36)
        for d in range(24 + int(unit('vein-l', i) * 30)):
            x += 1 if unit('vein-dx', i, d) < 0.8 else 0
            y += -1 if unit('vein-dy', i, d) < 0.28 else 1 if unit('vein-dy', i, d) > 0.72 else 0
            if 0 <= x < TILE and 0 < y < TILE:
                c[(x, y)] = 'a' if d % 3 else '('
                if unit('fleck', i, d) < 0.16:
                    c[(x, y - 1)] = 'A'
    if unit('coal', i) < 0.3:
        k = int(unit('coal-k', i) * 2)
        for x in range(TILE):
            y = ledge_y(k, x)
            for d in range(2, 5):
                c[(x, y - d)] = '0' if d < 4 else 'θ'
            if unit('coal-glint', i, x) < 0.07:
                c[(x, y - 3)] = 'b'
    for (x, y), (depth, facing) in rock_edges(open_).items():
        if depth < 0:
            c.pop((x, y), None)
        elif facing == 'n' and depth <= 6:
            n = unit('gravel', i, x // 2, y // 2)
            c[(x, y)] = ('Ω' if dither(x, y, 0.5) else 'B') if depth == 0 else 'o' if depth == 6 else 'i' if depth == 5 \
                else 'B' if n < 0.25 else 'Φ' if n < 0.6 else 'b' if n < 0.85 else 'a' if n < 0.93 else '('
        elif facing == 'e' and depth <= 2:
            c[(x, y)] = 'o' if depth == 0 else 'Ω' if depth == 1 else ('B' if dither(x, y, 0.6) else c[(x, y)])
        elif facing in ('w', 's') and depth <= 2:
            c[(x, y)] = 'o' if depth == 0 else 'i' if depth == 1 else ('b' if dither(x, y, 0.5) else c[(x, y)])
    if open_[0]:
        for k in range(4):
            x = 4 + int(unit('stone', i, k) * 54)
            top = next((y for y in range(TILE) if (x, y) in c), None)
            if top is not None:
                r = 1 + int(unit('stone-r', i, k) * 2)
                ellipse(c, x, top - r + 1, r + 1, r, lambda dx, dy: 'Ω' if dx + dy < -1 else 'B' if dy < 1 else 'b')
    return to_image(darker(c, back) if back else c)


def rivet(c, x, y, key):
    """A domed rivet at `(x, y)`, now and then bleeding rust down the plate."""
    if not (0 <= x < TILE - 1 and 0 <= y < TILE - 1):
        return
    c[(x, y)], c[(x + 1, y)], c[(x, y + 1)], c[(x + 1, y + 1)] = 'φ', 'χ', 'ψ', 'Ω'
    if unit('rivet-rust', key, x, y) < 0.35:
        for d in range(2, 3 + int(unit('rivet-rust-l', key, x, y) * 9)):
            if y + d < TILE and dither(x, y + d, 1 - d / 12):
                c[(x + 1, y + d)] = '(' if d < 5 else 'a'


def iron_clad(i, open_=(False, False, False, False)):
    """Riveted plates hung on the cut's faces: two rows of plates 32 px tall
    in a staggered bond (the plate across the tile's side is the same in
    every variant, so any two join), each plate pillowed, lit along its top
    and moonward edge, shadowed below, rivets every 8 px along its seams
    bleeding rust; some plates replaced (a shade darker) or rusted through;
    on some tiles a vent, a hatch, a dent, a welded patch or a torn corner
    showing the rock behind. An open side gets an angle-iron edge."""
    c = {}
    inner = [14, 46] if unit('clad-a', i) < 0.5 else [14, 28 + int(unit('clad-a-j', i) * 6), 46]
    row_a = [(46 - TILE, 14, ('clad-cross',))] + [(a, b, ('clad', i, 0, a)) for a, b in zip(inner, inner[1:])] + [(46, 46 + 32, ('clad-cross',))]
    joints = [0, 22 + int(unit('clad-b', i) * 20), TILE] if unit('clad-b3', i) < 0.6 \
        else [0, 18 + int(unit('clad-b', i) * 6), 38 + int(unit('clad-b2', i) * 6), TILE]
    row_b = [(a, b, ('clad', i, 1, a)) for a, b in zip(joints, joints[1:])]
    plates = [(0, row_a), (32, row_b)]
    for top, row in plates:
        for a, b, key in row:
            w = b - a
            shared = key == ('clad-cross',)
            heavy = not shared and unit(key, 'rust') < 0.12
            worn = not shared and unit(key, 'tone') < 0.25
            for y in range(top, top + 32):
                v = y - top
                for x in range(max(0, a), min(TILE, b)):
                    u = x - a
                    if u == 0 or v == 0:
                        sym = '<'
                    elif v == 1:
                        sym = ';'
                    elif v == 2:
                        sym = ':' if dither(x, y, 0.5) else '~'
                    elif u == w - 1:
                        sym = ';' if v < 28 else ':'
                    elif u == 1:
                        sym = '='
                    elif v == 31:
                        sym = '-'
                    elif v == 30:
                        sym = '=' if dither(x, y, 0.5) else '~'
                    else:
                        n = unit('clad-grain', key, x // 3, y // 4)
                        sym = (':' if n < 0.78 else '~') if v < 12 else ('~' if n < 0.88 else ':') if v < 25 else ('=' if dither(x, y, 0.35) else '~')
                        if worn:
                            sym = {':': '~', '~': '=', '=': '-'}[sym]
                        if unit('clad-grime', key, x, y // 6) < 0.04:
                            sym = '-'
                    c[(x, y)] = sym
            if heavy:
                for k in range(2 + int(unit(key, 'blotches') * 2)):
                    bx = a + 6 + int(unit(key, 'blotch-x', k) * max(1, w - 12))
                    by = top + 6 + int(unit(key, 'blotch-y', k) * 18)
                    rx_, ry_ = 3 + int(unit(key, 'blotch-r', k) * 3), 2 + int(unit(key, 'blotch-r2', k) * 2)
                    ellipse(c, bx, by, rx_, ry_, lambda dx, dy: 'a' if dy < -ry_ // 2 else '(' if dither(bx + dx, by + dy, 0.75) else '>')
                    for d in range(ry_, ry_ + 4 + int(unit(key, 'blotch-run', k) * 8)):
                        if 0 <= bx < TILE and by + d < top + 30 and dither(bx, by + d, 1 - d / 14):
                            c[(bx, by + d)] = '('
            for rx in range(a + 4, b - 3, 8):
                for ry in (top + 3, top + 27):
                    rivet(c, rx, ry, key)
    feature = unit('clad-feature', i)
    fx0, fx1 = (inner[0] + 3, inner[1] - 3)
    if feature < 0.14:
        for y in range(8, 25, 4):
            for x in range(fx0 + 2, fx1 - 2):
                c[(x, y)], c[(x, y + 1)], c[(x, y - 1)] = '<', '-', ';'
    elif feature < 0.26:
        rect(c, fx0 + 2, 36, fx0 + 22, 58, lambda x, y: '<' if x in (fx0 + 2, fx0 + 21) or y in (36, 57) else ';' if y == 37 else '-' if y == 56 else '~')
        for y in (40, 52):
            rect(c, fx0, y, fx0 + 4, y + 3, lambda x, yy: 'ψ' if yy == y else 'Ω')
        c[(fx0 + 17, 47)], c[(fx0 + 18, 47)] = 'χ', 'ψ'
    elif feature < 0.38:
        ellipse(c, 30, 46, 8, 6, lambda dx, dy: '-' if (dx < -3 or dy < -3) else ';' if (dx > 3 or dy > 3) else '=')
    elif feature < 0.5:
        rect(c, 8, 38, 28, 56, lambda x, y: ';' if y == 38 or x == 8 else '-' if y == 55 or x == 27 else ':' if unit('patch', i, x // 3, y // 3) < 0.8 else 'a')
        for x in range(7, 29):
            c[(x, 37)] = 'ψ' if x % 2 else 'Ω'
        for y in range(37, 57):
            c[(7, y)] = 'ψ' if y % 2 else 'Ω'
    elif feature < 0.6:
        rect(c, 34, 40, 50, 50, lambda x, y: '<' if x in (34, 49) or y in (40, 49) else ';' if y == 41 else ':')
        for x, y in ((36, 42), (46, 42), (36, 46), (46, 46)):
            c[(x, y)], c[(x + 1, y)] = 'φ', 'Ω'
    n, e, s, w = open_
    if w:
        for y in range(TILE):
            c[(0, y)], c[(1, y)], c[(2, y)], c[(3, y)] = 'o', 'Ω', 'ψ', 'Φ'
            if y % 10 == 4:
                c[(2, y)] = 'φ'
    if e:
        for y in range(TILE):
            c[(TILE - 1, y)], c[(TILE - 2, y)], c[(TILE - 3, y)], c[(TILE - 4, y)] = 'o', 'χ', 'ψ', 'Ω'
            if y % 10 == 4:
                c[(TILE - 3, y)] = 'φ'
    if s:
        for x in range(TILE):
            c[(x, TILE - 1)], c[(x, TILE - 2)], c[(x, TILE - 3)] = 'o', 'Φ', 'Ω'
        for k in range(3):
            x = 4 + int(unit('clad-drip', i, k) * 56)
            for y in range(TILE - 6, TILE - 3):
                c[(x, y)] = 'a'
    if n:
        for x in range(TILE):
            c[(x, 0)], c[(x, 1)], c[(x, 2)] = 'χ', 'ψ', 'Φ'
    return to_image(c)


def basalt_end(i, side):
    """A basalt surface's end: its crust over a cliff face on `side`."""
    c = from_image(basalt(i, f'cliff_{side}'))
    top = from_image(basalt(i, 'top'))
    for (x, y), sym in top.items():
        if y < 8:
            c[(x, y)] = sym
    edge = 0 if side == 'left' else TILE - 1
    for y in range(3):
        c.pop((edge, y), None)
    return to_image(c)


def under_end(i, side):
    """A masonry underside at a face's foot."""
    return to_image(edge(from_image(under(i)), side))


def seamed(images, names):
    """Gives every tile of `names` the first one's side columns, so any two
    of them meet without a seam."""
    first = images[names[0]]
    for name in names[1:]:
        im = images[name]
        for x in (0, 1, TILE - 2, TILE - 1):
            im.paste(first.crop((x, 0, x + 1, TILE)), (x, 0))


def rock_tiles(images, family, draw, masks, variants, seed):
    """`<family>_<mask>_<n>`: `draw(i, open_sides)` for each mask and
    variant, the variants of a mask seamed to its first."""
    for mask in masks:
        names = []
        for n in range(variants):
            name = f'{family}_{mask}_{n}'
            images[name] = draw(seed + mask * variants + n, sides(mask))
            names.append(name)
        seamed(images, names)


# The cladding's masks: a face open at a side or below, never on top, and
# the plate one cell behind a face, closed all round.
IRON_MASKS = tuple(mask for mask in MASKS if not mask & 1)
PIT_WALLS = ('plain', 'ring', 'cracked', 'niche', 'mossy')
# Where two families meet, left then right, each pair's covered tile and its
# surface (`_top`): masonry into the ravine's rock, the ravine's rock into
# basalt, basalt into the mine's rock, the mine's rock into masonry.
BORDERS = {
    'rock_wet': (lambda top: moss_canvas(3) if top else rock_canvas(3), lambda top: wet_rock(2, (top, False, False, False), 'plain')),
    'rock_basalt': (lambda top: wet_rock(5, (top, False, False, False), 'plain'), lambda top: basalt(5, 'top' if top else 'body')),
    'basalt_rock': (lambda top: basalt(6, 'top' if top else 'body'), lambda top: mine_rock(5, (top, False, False, False))),
    'mine_rock': (lambda top: mine_rock(7, (top, False, False, False)), lambda top: moss_canvas(4) if top else rock_canvas(4)),
}


def as_image(art):
    return art if hasattr(art, 'load') else to_image(art)


def build_level_terrain(images):
    """M76's terrain tiles, by the names `level.py` picks them by."""
    for i in range(VARIANTS):
        images[f'under_{i}'] = under(i)
    seamed(images, [f'under_{i}' for i in range(VARIANTS)])
    images['under_left'] = under_end(1, 'left')
    images['under_right'] = under_end(2, 'right')
    for side in ('left', 'right'):
        for k, kind in enumerate(PIT_WALLS):
            images[f'pit_wall_{kind}_{side}'] = pit_wall(side, kind, k * 3 + (side == 'right'))
        images[f'pit_rim_{side}'] = pit_rim(side)
    for i in range(4):
        images[f'pit_floor_{i}'] = pit_floor((0, 1, 2, 3)[i])
    for i in range(4):
        images[f'channel_{i}'] = channel(i)
    images['channel_mossy'] = channel(4, None, 'mossy')
    images['channel_spout'] = channel(5, None, 'spout')
    images['channel_cracked'] = channel(6, None, 'cracked')
    images['channel_break_left'] = channel_break(12, 'left')
    images['channel_break_right'] = channel_break(8, 'right')
    for i in range(BASALT_VARIANTS):
        images[f'basalt_{i}'] = basalt(i)
    seamed(images, [f'basalt_{i}' for i in range(BASALT_VARIANTS)])
    for i in range(4):
        images[f'basalt_top_{i}'] = basalt(i + 1, 'top')
        images[f'basalt_under_{i}'] = basalt(i + 2, 'under')
    seamed(images, [f'basalt_top_{i}' for i in range(4)])
    seamed(images, [f'basalt_under_{i}' for i in range(4)])
    for side in ('left', 'right'):
        images[f'basalt_top_{side}'] = basalt_end(1 if side == 'left' else 2, side)
        images[f'basalt_cliff_{side}'] = basalt(3, f'cliff_{side}')
    rock_tiles(images, 'wet_rock', wet_rock, MASKS, ROCK_VARIANTS, 1)
    rock_tiles(images, 'mine_rock', mine_rock, MASKS, ROCK_VARIANTS, 1)
    rock_tiles(images, 'iron_clad', iron_clad, IRON_MASKS, ROCK_VARIANTS, 1)
    for name, (left, right) in BORDERS.items():
        for top in (False, True):
            images[f'border_{name}' + ('_top' if top else '')] = border(as_image(left(top)), as_image(right(top)), name + str(top))
    for i in range(2):
        images[f'bridge_deck_{i}'] = bridge_deck(i)
