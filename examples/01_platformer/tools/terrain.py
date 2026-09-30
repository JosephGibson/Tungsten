"""Ruin masonry, turf, platforms, trestles and the waterfall's wet cliff.

Every opaque terrain tile is cut from one masonry texture that repeats exactly
every 64 pixels. Variants only change stones that lie wholly inside a tile, so
any arrangement of variants meets without a seam; the turf lip, cliff edges and
platform decks are likewise identical wherever tiles touch.
"""
import math
from dataclasses import dataclass

from art import rng_for
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
                   'timber_support', 'corner_', 'fill_', 'stone_', 'platform', 'ledge')


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
