"""Particle, glow and black-hole sprites.

Particle sprites are neutral greys or whites so each config's colour curve tints
them. The glow gradients are dithered between the palette's alpha steps and are
sampled with linear filtering, so they stay smooth when particles or halos
scale them up. The black-hole layers are white alpha fields on 96 px canvases,
tinted at extraction; each fades to transparent before its canvas edge.
"""
import math

from art import EFFECT_SIZES, ELEMENT_SIZES, ICON_SIZES, PALETTE, STATUS_CLASSES, canvas
from pixels import BAYER, NEIGHBOURS, Part, compose, dither, outline, shade, smoothstep, to_image, unit

HALO_STEPS = '!@#$%^&*'  # white at alpha 1, 2, 3, 5, 8, 12, 17, 24
FLAME_STEPS = 'ejklpqux'  # white at alpha 4, 8, 16, 28, 44, 68, 104, 160
LINEAR_FILTER = {'halo', 'flame_glow'} | set(EFFECT_SIZES)
# Every white alpha step in the palette plus opaque white, one symbol per level.
WHITE_LEVELS = sorted({PALETTE[s][3]: s for s in HALO_STEPS + FLAME_STEPS + 'Ψ'}.items())


def radial_glow(steps, falloff):
    """Alpha falls off as (1 - r) ** falloff; neighbouring steps are dithered."""
    levels = [0] + [PALETTE[s][3] for s in steps]
    peak = levels[-1]
    colors = {}
    for y in range(64):
        for x in range(64):
            r = math.hypot(x - 31.5, y - 31.5) / 31.5
            target = peak * max(0.0, 1 - r) ** falloff
            for i in range(len(levels) - 1):
                low, high = levels[i], levels[i + 1]
                if low <= target <= high:
                    pick = i + 1 if dither(x, y, (target - low) / max(high - low, 1)) else i
                    if pick:
                        colors[(x, y)] = steps[pick - 1]
                    break
    return to_image(colors)


def puff():
    """Soft dust puff: lit crown, shaded underside, dithered edge."""
    colors = {}
    lobes = ((25, 36, 12), (36, 30, 13), (44, 38, 10), (32, 40, 11))
    for y in range(64):
        for x in range(64):
            inside = max(1 - math.hypot(x - cx, y - cy) / r for cx, cy, r in lobes)
            if inside <= 0 or not dither(x, y, min(1.0, inside * 3.5)):
                continue
            height = (y - 18) / 32
            colors[(x, y)] = 'φ' if height < 0.35 else 'χ' if height < 0.7 else 'ψ'
    return to_image(colors)


def droplet():
    """Neutral teardrop with a glint; spray configs tint it."""
    colors = {}
    for y in range(10, 50):
        for x in range(20, 45):
            # A round belly below a tapering point.
            belly = math.hypot(x - 32, y - 38) <= 9.5
            point = y < 38 and abs(x - 32) <= (y - 10) * 9.5 / 28
            if belly or point:
                colors[(x, y)] = 'ψ' if x > 34 or y > 44 else 'χ'
    for x, y in ((28, 34), (28, 35), (29, 33), (28, 36), (29, 34)):
        colors[(x, y)] = 'Ψ'
    return to_image(colors)


def firefly():
    """A soft dot with a white-hot core; the wind-mote config makes it blink."""
    colors = {}
    for y in range(64):
        for x in range(64):
            r = math.hypot(x - 31.5, y - 31.5)
            if r < 2.5:
                colors[(x, y)] = 'Ψ'
            elif r < 5:
                colors[(x, y)] = 'φ'
            elif r < 9 and dither(x, y, (9 - r) / 5):
                colors[(x, y)] = 'χ'
    return to_image(colors)


def spark_and_cursor(images):
    for name in ('spark', 'cursor'):
        im, d = canvas()
        if name == 'cursor':
            for pts in [[(14, 24), (14, 14), (24, 14)], [(40, 14), (50, 14), (50, 24)],
                        [(14, 40), (14, 50), (24, 50)], [(40, 50), (50, 50), (50, 40)]]:
                d.line(pts, fill=PALETTE['Y'], width=3)
            d.rectangle((30, 30, 33, 33), fill=PALETTE['Y'])
        else:
            d.polygon([(32, 12), (36, 27), (50, 32), (36, 36), (32, 51), (28, 36), (14, 32), (28, 28)],
                      fill=PALETTE['Y'])
        images[name] = im


def white_alpha(x, y, alpha):
    """Dithered white palette symbol for `alpha` (0..255), or None when clear."""
    low_alpha, low = 0, None
    for high_alpha, high in WHITE_LEVELS:
        if alpha <= high_alpha:
            return high if dither(x, y, (alpha - low_alpha) / (high_alpha - low_alpha)) else low
        low_alpha, low = high_alpha, high
    return low


def polar_field(name, field):
    """Render `field(r, angle, x, y)` over the canvas; r is 1 at the canvas edge."""
    size = EFFECT_SIZES[name]
    center, radius = (size[0] - 1) / 2, size[0] / 2
    colors = {}
    for y in range(size[1]):
        for x in range(size[0]):
            r = math.hypot(x - center, y - center) / radius
            symbol = field(r, math.atan2(y - center, x - center), x, y)
            if symbol:
                colors[(x, y)] = symbol
    return to_image(colors, size)


def ring(r, center, width):
    return math.exp(-((r - center) / width) ** 2)


def vortex(r, angle, x, y):
    """Four trailing log-spiral arms, narrow near the core, bright on their inner edge."""
    if r < 0.1:
        return None
    radial = smoothstep((r - 0.14) / 0.22) * (1 - smoothstep((r - 0.5) / 0.46))
    u = 4 * angle - 5.5 * math.log(r)
    arm = max(0.0, math.cos(u)) ** (8 - 5 * r) * (0.75 + 0.25 * math.sin(u))
    return white_alpha(x, y, 255 * radial * min(1.0, arm + 0.12))


def accretion_disk(r, angle, x, y):
    """A hot inner rim falling off outward, broken into turbulent spiral bands."""
    if r < 0.18:
        return None
    swirl = math.log(r)
    bands = (0.55 + 0.3 * math.cos(7 * angle - 9 * swirl + 1.6 * math.sin(3 * angle))
             + 0.15 * math.cos(15 * angle - 20 * swirl))
    body = smoothstep((r - 0.2) / 0.1) * (1 - smoothstep((r - 0.3) / 0.66)) ** 1.4
    return white_alpha(x, y, 255 * min(1.0, body * bands + 0.55 * ring(r, 0.3, 0.035)))


def vortex_core(r, angle, x, y):
    """Opaque horizon with a dithered soft edge and a faint lensing arc beyond it."""
    if dither(x, y, 1 - smoothstep((r - 0.75) / 0.09)):
        return 'θ'
    window = smoothstep((math.sin(angle + 0.6) - 0.1) / 0.6)
    return white_alpha(x, y, 110 * ring(r, 0.9, 0.035) * window)


def photon_ring(r, angle, x, y):
    """A thin bright ring, beamed brighter on one side, with a faint outer echo."""
    beam = 0.45 + 0.55 * (0.5 + 0.5 * math.cos(angle - 0.8)) ** 1.5
    return white_alpha(x, y, 255 * beam * (ring(r, 0.78, 0.045) + 0.3 * ring(r, 0.88, 0.03)))


def infall_streak():
    """A tapered streak pointing along +x: bright round head, fading thin tail."""
    size = EFFECT_SIZES['infall_streak']
    center = (size[1] - 1) / 2
    colors = {}
    for y in range(size[1]):
        for x in range(size[0]):
            u = (x - 8) / 76
            if not 0 <= u <= 1.08:
                continue
            width = 0.6 + 3.4 * min(u, 1.0) ** 1.5
            across = max(0.0, 1 - (abs(y - center) / width) ** 2)
            head = max(0.0, 1 - math.hypot(x - 84, y - center) / 4.5)
            symbol = white_alpha(x, y, 255 * min(1.0, min(u, 1.0) ** 1.8 * across + head))
            if symbol:
                colors[(x, y)] = symbol
    return to_image(colors, size)


def build_black_hole(images):
    for name, field in (('vortex', vortex), ('accretion_disk', accretion_disk),
                        ('vortex_core', vortex_core), ('photon_ring', photon_ring)):
        images[name] = polar_field(name, field)
    images['infall_streak'] = infall_streak()


# The spell, gun and summon icons, one family (01 (h)): each 32 px subject
# in the house style, lit from the top left with a rim of its element's
# light on its far edges, over a dark backing tinted by the element with a
# glow behind the subject, in a two-pixel bevelled frame in the element's
# colour, on a 36 px tile the HUD shows at whole-number scale.
ICON = 32
ICON_TILE = ICON_SIZES['icon_gun'][0]
ICON_INSET = (ICON_TILE - ICON) // 2
# Each element's frame (light, mid, dark), its backing's ramp from the
# corners in, and the rim light on its subjects' far edges: gravity's
# purple apart from shock's violet, and mana's blue for the summons.
ICON_ELEMENTS = {
    'fire': dict(frame=('J', 'K', 'L'), back=('θ', 'Θ', 'M', 'ϑ', 'L', 'K'), rim='J'),
    'cold': dict(frame=('`', ',', ')'), back=('θ', 'х', '|', '{', ')', '}'), rim='`'),
    'gravity': dict(frame=('Щ', 'Ш', 'Ц'), back=('θ', 'Ж', 'Ф', 'Ц', 'Ш', 'Щ'), rim='Щ'),
    'mana': dict(frame=('Ю', 'З', 'Й'), back=('θ', 'г', 'И', 'Й', 'З', 'Ю'), rim='Ю'),
    'physical': dict(frame=('I', 'σ', 'τ'), back=('θ', 'ν', 'α', 'δ', 'ъ', 'ψ'), rim='φ'),
    'shock': dict(frame=('ϕ', 'Σ', 'Λ'), back=('θ', 'Δ', 'Δ', 'Λ', 'Σ', 'ϕ'), rim='ϕ'),
    # e01tomes: a book's tile in its own metal, square at its corners with a
    # bracket on each, as its book's corners are, apart from the spells'
    # bevels; its backing in its glow's colour, and the bracket's light and
    # rivet.
    'pact': dict(frame=('ψ', 'Ω', 'Φ'), back=('θ', 'д', 'ь', 'Θ', 'M', 'L'), rim='Ъ', corners=('χ', 'B')),
    'bounding': dict(frame=('φ', 'χ', 'ψ'), back=('θ', 'х', 'И', 'Й', 'З', 'Ч'), rim='Я', corners=('Ψ', 'Ω')),
    'arcane': dict(frame=('J', 'A', 'У'), back=('θ', 'Ж', 'Δ', 'Ф', 'Λ', 'Э'), rim='Ы', corners=('Z', 'a')),
}
ICON_ELEMENT = {
    'icon_fire_missile': 'fire', 'icon_flamethrower': 'fire', 'icon_ice_spray': 'cold',
    'icon_force_push': 'gravity', 'icon_black_hole': 'gravity', 'icon_iron_block': 'physical',
    'icon_bowling_ball': 'physical', 'icon_small_balls': 'physical', 'icon_lightning': 'shock',
    'icon_gun': 'physical', 'icon_dummy': 'mana', 'icon_wisp': 'mana', 'icon_crawler': 'mana',
    'icon_beetle': 'mana', 'icon_bat': 'mana', 'icon_troll': 'mana', 'icon_goblin': 'mana',
    'icon_crate': 'mana', 'icon_keg': 'mana',
    'icon_tome_pact': 'pact', 'icon_tome_bounding': 'bounding', 'icon_tome_arcane': 'arcane',
}
# The summons' ring, drawn in indigo, recoloured to mana's blue.
SUMMON_RING = {'Ξ': 'З', 'Π': 'Й'}


def icon_tile(name, subject):
    """`name`'s 36 px tile from its 32 px `subject` pixels."""
    element = ICON_ELEMENTS[ICON_ELEMENT[name]]
    light, mid, dark = element['frame']
    back = element['back']
    summon = ICON_ELEMENT[name] == 'mana'
    sub = {}
    for (x, y), s in subject.items():
        if summon and s in SUMMON_RING and y >= 24:
            s = SUMMON_RING[s]
        sub[(x + ICON_INSET, y + ICON_INSET)] = s
    inside = set(sub)
    # The subject lit from the top left: its far edges take the element's
    # rim light, its near edges stay as drawn.
    lit = dict(sub)
    for (x, y), s in sub.items():
        far = (x + 1, y) not in inside or (x, y + 1) not in inside
        near = (x - 1, y) not in inside or (x, y - 1) not in inside
        if far and not near and s not in ('o', 'θ', 'ε'):
            lit[(x, y)] = element['rim']
    canvas = {}
    c = (ICON_TILE - 1) / 2
    # The backing: a dark tile, a soft glow of the element round the subject.
    near_subject = {}
    for x, y in inside:
        for dx in range(-4, 5):
            for dy in range(-4, 5):
                d = math.hypot(dx, dy)
                if d <= 4:
                    q = (x + dx, y + dy)
                    near_subject[q] = max(near_subject.get(q, 0.0), 1 - d / 4.5)
    for y in range(2, ICON_TILE - 2):
        for x in range(2, ICON_TILE - 2):
            r = math.hypot(x - c, y - c) / (ICON_TILE / 2)
            v = 0.1 + 0.3 * max(0.0, 1 - r) + 0.58 * near_subject.get((x, y), 0.0) ** 1.5
            v += (BAYER[y % 4][x % 4] / 16 - 0.5) * 0.28
            canvas[(x, y)] = back[min(len(back) - 1, max(0, int(v * len(back))))]
    # The subject, outlined where it meets the glow.
    for p, s in outline(dict(lit), 'o', (ICON_TILE, ICON_TILE)).items():
        canvas[p] = s
    # The frame: an outer dark line, a bevel lit along the top and left;
    # a tome's square, a bracket over each corner.
    end = ICON_TILE - 1
    corners = element.get('corners')
    for y in range(ICON_TILE):
        for x in range(ICON_TILE):
            edge = min(x, y, end - x, end - y)
            corner = min(x, end - x) + min(y, end - y)
            if corner < 2 and not corners:
                canvas.pop((x, y), None)
                continue
            if edge == 0:
                canvas[(x, y)] = 'o'
            elif edge == 1:
                canvas[(x, y)] = light if (x <= y and x + y < ICON_TILE) or y == 1 and x < end - 1 else dark
                if x == end - 1 or y == end - 1:
                    canvas[(x, y)] = dark
                if (x == 1 or y == 1) and not (x == end - 1 or y == end - 1):
                    canvas[(x, y)] = light if x + y < ICON_TILE else mid
    if corners:
        bright, rivet = corners
        for cx, cy, sx, sy in ((0, 0, 1, 1), (end, 0, -1, 1), (0, end, 1, -1), (end, end, -1, -1)):
            for i in range(1, 7):
                for j in (1, 2):
                    canvas[(cx + sx * i, cy + sy * j)] = bright if j == 1 or i < 3 else mid
                    canvas[(cx + sx * j, cy + sy * i)] = bright if j == 1 or i < 3 else mid
                canvas[(cx + sx * i, cy + sy * 3)] = 'o' if i < 4 else canvas.get((cx + sx * i, cy + sy * 3), 'o')
                canvas[(cx + sx * 3, cy + sy * i)] = 'o' if i < 4 else canvas.get((cx + sx * 3, cy + sy * i), 'o')
            canvas[(cx + sx * 2, cy + sy * 2)] = rivet
    return to_image(canvas, (ICON_TILE, ICON_TILE))


def icon_disc(center, radius, symbols, light=(-0.55, -0.65)):
    """A lambert-shaded ball from its darkest symbol to its lightest."""
    cx, cy = center
    out = {}
    for y in range(ICON):
        for x in range(ICON):
            dx, dy = (x + 0.5 - cx) / radius, (y + 0.5 - cy) / radius
            d2 = dx * dx + dy * dy
            if d2 > 1:
                continue
            lit = max(0.0, -(dx * light[0] + dy * light[1]) * 0.8 + math.sqrt(1 - d2) * 0.6)
            out[(x, y)] = symbols[min(len(symbols) - 1, int(lit * len(symbols)))]
    return out


def icon_cone(x0, x1, half0, half1, seed, layers):
    """A cone of flame or frost along +x with wavy edges, hottest on its axis."""
    out = {}
    for x in range(x0, x1):
        u = (x - x0) / max(1, x1 - x0 - 1)
        half = half0 + (half1 - half0) * u
        wobble = 1.6 * math.sin(x * 0.9 + seed) + 1.0 * math.sin(x * 2.3 + seed * 2)
        for y in range(ICON):
            off = abs(y + 0.5 - 16 - wobble * 0.4)
            edge = half + wobble * 0.5 * u
            if off > edge:
                continue
            heat = 1 - off / max(edge, 1e-3) * 0.85 - u * 0.35
            out[(x, y)] = layers[min(len(layers) - 1, max(0, int(heat * len(layers))))]
    return out


def icon_nozzle():
    nozzle = Part((ICON, ICON))
    nozzle.rectangle((2, 13, 9, 19))
    return shade(nozzle.pixels(), 'Ω', light='ψ', shadow='Φ')


def icon_fire_missile():
    """The fire missile's rocket (`actors.missile_shape`) turned 40 degrees
    up and to the right, its nose at the top right, its plume trailing to
    the bottom left."""
    from actors import missile_shape, missile_symbol
    angle = math.radians(40)
    ax, ay = math.cos(angle), -math.sin(angle)
    tip = (27.0, 6.5)
    pixels = {}
    for y in range(ICON):
        for x in range(ICON):
            dx, dy = x + 0.5 - tip[0], y + 0.5 - tip[1]
            u = -(dx * ax + dy * ay)
            v = -dx * ay + dy * ax
            s = missile_symbol(missile_shape(u, v, 0.3, k=0.78), x, y)
            if s:
                pixels[(x, y)] = s
    return pixels


def icon_flamethrower():
    pixels = icon_nozzle()
    pixels.update(icon_cone(10, 31, 2.0, 11.0, 0.7, 'MLKJY'))
    return pixels


def icon_ice_spray():
    pixels = icon_nozzle()
    mist = icon_cone(10, 31, 2.0, 11.0, 2.1, '|{)}_')
    pixels.update({p: s for p, s in mist.items() if (p[0] + p[1]) % 3 or s in '_`'})
    for cx, cy in [(15, 15), (21, 11), (22, 20), (27, 15), (27, 8), (26, 24)]:
        for dx, dy, s in [(0, 0, '`'), (1, 0, '_'), (-1, 0, '_'), (0, 1, '_'), (0, -1, '_')]:
            pixels[(cx + dx, cy + dy)] = s
    return pixels


def icon_force_push():
    pixels = {}
    for i, radius in enumerate((6, 11, 16)):
        band = Part((ICON, ICON))
        arc = [(4 + radius * math.cos(a), 16 + radius * math.sin(a))
               for a in (math.radians(t) for t in range(-60, 61, 6))]
        band.limb(arc, 3 if i < 2 else 2)
        pixels.update(shade(band.pixels(), 'Π' if i else 'Ξ', light='Ξ', shadow='Γ'))
    palm = Part((ICON, ICON))
    palm.ellipse((5, 16), 3, 4)
    pixels.update(shade(palm.pixels(), '(', light='a', shadow='['))
    return pixels


def icon_black_hole():
    pixels = {}
    ring = Part((ICON, ICON))
    ring.ellipse((16, 16), 14, 6)
    inner = Part((ICON, ICON))
    inner.ellipse((16, 16), 9, 3)
    for x, y in ring.pixels() - inner.pixels():
        swirl = math.sin(math.atan2((y - 16) * 2.2, x - 16) * 3 + x * 0.3)
        pixels[(x, y)] = 'Σ' if swirl > 0.55 else ('Λ' if swirl > -0.2 else 'Δ')
    pixels.update(icon_disc((16, 16), 6.5, 'θθε'))
    for x in range(10, 23):  # the photon ring over the core's top
        pixels[(x, round(16 - math.sqrt(max(0.0, 42 - (x - 16) ** 2))))] = 'π'
    return pixels


def icon_iron_block():
    body = Part((ICON, ICON))
    body.rectangle((5, 8, 26, 24))
    pixels = shade(body.pixels(), 'Ω', light='χ', shadow='Φ', rim='Φ')
    for x in range(6, 26):
        pixels[(x, 9)] = 'ψ'
    for x, y in [(8, 12), (23, 12), (8, 21), (23, 21)]:  # rivets
        pixels[(x, y)] = 'φ'
        pixels[(x + 1, y + 1)] = 'Φ'
    for y in range(12, 22):
        pixels[(16, y)] = 'Φ'
    return pixels


def icon_bowling_ball():
    """The bowling ball's resin, finger holes and iron sheen."""
    cx = cy = ICON / 2
    radius = 12.5
    pixels = {}
    for y in range(ICON):
        for x in range(ICON):
            dx, dy = (x + 0.5 - cx) / radius, (y + 0.5 - cy) / radius
            d2 = dx * dx + dy * dy
            if d2 > 1:
                continue
            lit = max(0.0, (-dx * 0.5 - dy * 0.7) * 0.7 + math.sqrt(1 - d2) * 0.5)
            pixels[(x, y)] = 'γβαδ'[min(3, int(lit * 4))]
            if lit > 0.82:
                pixels[(x, y)] = 'ψ' if lit < 0.9 else 'χ'
    for hx, hy, hr in [(-0.25, -0.3, 0.13), (0.08, -0.38, 0.13), (-0.05, 0.02, 0.15)]:
        for y in range(ICON):
            for x in range(ICON):
                if math.hypot((x + 0.5 - cx) / radius - hx, (y + 0.5 - cy) / radius - hy) < hr:
                    pixels[(x, y)] = 'θ'
    return pixels


def icon_small_balls():
    pixels = {}
    for cx, cy, radius, ramp in [(10, 20, 5.5, 'Φ:;δ'), (21, 21, 5.5, 'Φ:;δ'),
                                 (15.5, 11, 5.5, 'ΦΩψχ'), (25, 11, 4.5, 'Φ:;δ')]:
        pixels.update(icon_disc((cx, cy), radius, ramp))
        pixels[(round(cx - radius * 0.4), round(cy - radius * 0.45))] = 'φ'
    return pixels


def zigzag(start, end, kinks, seed, jitter):
    """A jagged line from `start` to `end` through `kinks` hashed offsets
    across it: the shape of every bolt, arc and crack."""
    (x0, y0), (x1, y1) = start, end
    nx, ny = -(y1 - y0), x1 - x0
    n = math.hypot(nx, ny) or 1.0
    points = [start]
    for i in range(1, kinks):
        u = i / kinks
        side = (unit('zig', seed, i) - 0.5) * 2 * jitter
        points.append((x0 + (x1 - x0) * u + nx / n * side, y0 + (y1 - y0) * u + ny / n * side))
    points.append(end)
    return points


def icon_lightning():
    """A storm cloud over a bolt: a violet glow, a light core, a white spine."""
    cloud = Part((ICON, ICON))
    for cx, cy, r in ((10, 9, 6), (17, 7, 7), (24, 9, 6), (16, 11, 6)):
        cloud.ellipse((cx, cy), r, r * 0.8)
    pixels = shade(cloud.pixels(), 'α', light='δ', shadow='β')
    path = [(18, 12), (13, 19), (18, 19), (11, 29)]
    for width, symbol in ((5, 'Λ'), (3, 'Σ'), (1, 'Ψ')):
        bolt = Part((ICON, ICON))
        bolt.limb(path, width)
        pixels.update({p: symbol for p in bolt.pixels()})
    return pixels


def icon_gun():
    """A semi-automatic pistol, three-quarter lit, tilted 14 degrees up at
    the muzzle: a steel slide with its serrations, ejection port, sights
    and a muzzle bore, a frame with its trigger guard and trigger, a
    checkered walnut grip with a brass screw, the magazine's base plate."""
    from actors import turned
    pivot, angle = (16.0, 17.0), math.radians(-14)
    def t(points):
        return turned(points, pivot, angle)
    slide = Part((ICON, ICON))
    slide.polygon(t([(5, 9), (28, 9), (29, 10), (29, 14), (5, 14)]))
    frame = Part((ICON, ICON))
    frame.polygon(t([(9, 14), (26, 14), (26, 16), (15, 16), (14, 17), (10, 17)]))
    grip = Part((ICON, ICON))
    grip.polygon(t([(6, 14), (13, 14), (12.5, 17), (11, 27), (4, 27), (5, 20)]))
    guard = Part((ICON, ICON))
    guard.limb(t([(14, 16), (14.5, 20), (19, 20.5), (20, 16)]), 1)
    trigger = Part((ICON, ICON))
    trigger.limb(t([(16.5, 16), (16, 18.5)]), 1)
    plate = Part((ICON, ICON))
    plate.polygon(t([(3.5, 26.5), (11.5, 26.5), (11.5, 28.5), (3.5, 28.5)]))
    pixels = {}
    pixels.update(shade(grip.pixels(), '"', light="'", shadow='ω', back='ω', rim="'"))
    # The grip's checkering: a diamond lattice of shadow.
    for x, y in grip.pixels():
        if (x + y) % 3 == 0 and (x - y) % 3 == 0:
            pixels[(x, y)] = 'ω'
    sx, sy = map(round, t([(8.5, 21.5)])[0])
    pixels[(sx, sy)] = 'A'
    pixels.update(shade(plate.pixels(), 'Φ', light='Ω', shadow='θ'))
    pixels.update(shade(frame.pixels(), 'Φ', light='Ω', shadow='ν'))
    pixels.update({p: 'Φ' for p in guard.pixels()})
    pixels.update({p: 'Ω' for p in trigger.pixels()})
    pixels.update(shade(slide.pixels(), 'Ω', light='χ', shadow='Φ', back='Φ', rim='ψ'))
    # A highlight raking along the slide's top, the serrations at its rear,
    # the ejection port, the sights and the bore.
    for x in range(8, 28):
        p = tuple(map(round, t([(x, 10)])[0]))
        if p in slide.pixels():
            pixels[p] = 'φ' if 11 <= x <= 22 else 'χ'
    for x in (7, 9, 11):
        for y in (11, 12, 13):
            p = tuple(map(round, t([(x, y)])[0]))
            if p in slide.pixels():
                pixels[p] = 'Φ'
    for x in range(16, 21):
        p = tuple(map(round, t([(x, 11.5)])[0]))
        pixels[p] = 'ν'
    for x, y in ((27.5, 8.4), (6.5, 8.4)):
        pixels[tuple(map(round, t([(x, y)])[0]))] = 'χ'
    pixels[tuple(map(round, t([(29.2, 12)])[0]))] = 'θ'
    return pixels


def icon_dummy():
    """The training dummy, the summon slot's first type, at half size on a
    summoning ring (recoloured to mana's blue on its tile)."""
    from actors import dummy_canvas
    ring, inner = Part((ICON, ICON)), Part((ICON, ICON))
    ring.ellipse((16, 28), 13, 3)
    inner.ellipse((16, 28), 10, 2)
    pixels = {p: 'Ξ' if p[1] <= 27 else 'Π' for p in ring.pixels() - inner.pixels()}
    dummy = dummy_canvas()
    for y in range(ICON):
        for x in range(ICON):
            symbol = dummy.get((2 * x + 1, 2 * y + 1))
            if symbol and y >= 2:
                pixels[(x, y - 2)] = symbol
    return pixels


def icon_of(canvas, step, lift):
    """A summon type's icon: its frame's canvas at 1/`step` size, raised
    `lift` pixels, standing on a summoning ring (mana's blue on its tile)."""
    ring, inner = Part((ICON, ICON)), Part((ICON, ICON))
    ring.ellipse((16, 28), 13, 3)
    inner.ellipse((16, 28), 10, 2)
    pixels = {p: 'Ξ' if p[1] <= 27 else 'Π' for p in ring.pixels() - inner.pixels()}
    half = step // 2
    for y in range(ICON + lift):
        for x in range(ICON):
            symbol = canvas.get((step * x + half, step * y + half))
            if symbol and 0 <= y - lift < ICON:
                pixels[(x, y - lift)] = symbol
    return pixels


def icon_wisp():
    from actors import wisp_canvas
    return icon_of(wisp_canvas(0.25), 2, 4)


def icon_crawler():
    from actors import crawler_canvas
    return icon_of(crawler_canvas(), 2, 2)


def icon_beetle():
    from actors import beetle_canvas
    return icon_of(beetle_canvas(), 2, 2)


def icon_bat():
    from actors import bat_canvas
    return icon_of(bat_canvas('fly', 0.3), 2, 0)


def icon_troll():
    """The troll at a quarter of its 128 frame."""
    from actors import TrollPose, troll_canvas
    return icon_of(troll_canvas(TrollPose()), 4, 2)


def icon_goblin():
    from actors import goblin_canvas
    return icon_of(goblin_canvas(), 2, 2)


def icon_prop(canvas, scale, lift=3):
    """A prop's summon icon: its frame's canvas at 1/`scale`, its foot
    `lift` pixels over the summoning ring's centre."""
    from actors import SOLE
    ring, inner = Part((ICON, ICON)), Part((ICON, ICON))
    ring.ellipse((16, 28), 13, 3)
    inner.ellipse((16, 28), 10, 2)
    pixels = {p: 'Ξ' if p[1] <= 27 else 'Π' for p in ring.pixels() - inner.pixels()}
    for y in range(ICON):
        for x in range(ICON):
            symbol = canvas.get((round(32 + (x - 15.5) * scale), round(SOLE - (28 - lift - y) * scale)))
            if symbol and symbol != 'o':
                pixels[(x, y)] = symbol
    return pixels


def icon_crate():
    from props import crate_slatted
    return icon_prop(crate_slatted(), 2.6)


def icon_keg():
    from props import keg_canvas
    return icon_prop(keg_canvas(), 2.4)


ICON_ART = {
    'icon_fire_missile': icon_fire_missile,
    'icon_flamethrower': icon_flamethrower,
    'icon_ice_spray': icon_ice_spray,
    'icon_force_push': icon_force_push,
    'icon_black_hole': icon_black_hole,
    'icon_iron_block': icon_iron_block,
    'icon_bowling_ball': icon_bowling_ball,
    'icon_small_balls': icon_small_balls,
    'icon_lightning': icon_lightning,
    'icon_gun': icon_gun,
    'icon_dummy': icon_dummy,
    'icon_wisp': icon_wisp,
    'icon_crawler': icon_crawler,
    'icon_beetle': icon_beetle,
    'icon_bat': icon_bat,
    'icon_troll': icon_troll,
    'icon_goblin': icon_goblin,
    'icon_crate': icon_crate,
    'icon_keg': icon_keg,
    # e01tomes: each book's first idle frame, unoutlined, as its subject.
    'icon_tome_pact': lambda: tome_pact(0, outlined=False),
    'icon_tome_bounding': lambda: tome_bounding(0, outlined=False),
    'icon_tome_arcane': lambda: tome_arcane(0, outlined=False),
}


def build_icons(images):
    if set(ICON_ART) != set(ICON_SIZES) or set(ICON_ELEMENT) != set(ICON_SIZES):
        raise ValueError('every spell icon needs its art and its element')
    for name, draw in ICON_ART.items():
        images[name] = icon_tile(name, draw())


# Lightning: eight segments, each entering at its top centre and leaving
# `BOLT_EXITS` pixels aside at its bottom, which a strike stacks from the sky
# down, each piece starting where the last left.
BOLT_SIZE = ELEMENT_SIZES['bolt_0']
BOLT_EXITS = (-8, -4, 0, 4, 8, -6, 6, 2)


def bolt_segment(index):
    """A violet haze, a glow, a light core and a white spine along one jagged
    path; every third piece forks a short twig."""
    width, height = BOLT_SIZE
    start, end = (width / 2, 0), (width / 2 + BOLT_EXITS[index], height - 1)
    path = zigzag(start, end, 5, ('bolt', index), 4.5)
    pixels = {}
    for stroke, symbol in ((9, '*'), (5, 'Λ'), (3, 'Σ'), (1, 'Ψ')):
        line = Part(BOLT_SIZE)
        line.limb(path, stroke)
        pixels.update({p: symbol for p in line.pixels()})
    if index % 3 == 1:
        twig_end = (path[2][0] + (7 if BOLT_EXITS[index] <= 0 else -7), path[2][1] + 14)
        twig = Part(BOLT_SIZE)
        twig.limb(zigzag(path[2], twig_end, 3, ('twig', index), 2), 1)
        pixels.update({p: 'Λ' for p in twig.pixels() if pixels.get(p, '*') == '*'})
    return to_image(pixels, BOLT_SIZE)


def status_box(size):
    """The box a status overlay covers in a sprite of `size`: inset at the
    sides and the top, down to where the frame's feet stand."""
    side, top = round(size * 0.094), round(size * 0.05)
    return side, top, size - 1 - side, round(size * 0.97) - 1


# The frozen look (01 (c), after the owner's reviews): a frosty cube of ice
# in three-quarter view fitted to the frozen body's hurtbox, in place of (b)'s
# flat block. Each kind with a hurtbox has its own cube on a full frame, so
# what lies outside the hurtbox (the bat's wings, the beetle's horn, the
# dummy's arms) shows outside the ice; the size classes keep a cube for a
# frozen body with no fitted one.
#
# Each kind's hurtbox in its frame, (left, top, right, bottom): the box its
# prefab's hurtbox makes, which `generate.check_ice_fits` holds it to.
HURTBOXES = {
    'wisp': (19, 21, 45, 47),  # a circle of 13 about (32, 34), as its box
    'crawler': (8, 41, 56, 59),  # aabb 24 x 9 about (32, 50)
    'beetle': (8, 35, 56, 59),  # aabb 24 x 12 about (32, 47)
    'bat': (22, 19, 42, 37),  # aabb 10 x 9 about (32, 28)
    'troll': (44, 12, 104, 123),  # aabb 30 x 55.5 about (74, 67.5), on its 128 frame
    'dummy': (18, 5, 46, 61),  # aabb 14 x 28 about (32, 33)
    'goblin': (25, 21, 43, 59),  # aabb 9 x 19 about (34, 40)
}
# 01 (g)'s props, which have no hurtbox: the box each one's cube covers,
# its art out to its outline (the crate's body a box of half 28, the keg's
# of half (19, 25) inside its bulging staves).
PROP_BOXES = {
    'crate': (3, 3, 60, 60),
    'keg': (12, 10, 52, 60),
}
# The pixels a cube reaches past its hurtbox on each side.
ICE_MARGIN = 2


def ice_frame(kind):
    """The side of `kind`'s frame: the troll's 128, the others' 64."""
    return 128 if kind == 'troll' else 64


def fitted_box(kind):
    """`kind`'s hurtbox grown by the margin, inside its frame."""
    x0, y0, x1, y1 = HURTBOXES[kind]
    side = ice_frame(kind)
    return (max(0, x0 - ICE_MARGIN), max(0, y0 - ICE_MARGIN),
            min(side - 1, x1 + ICE_MARGIN), min(side - 1, y1 + ICE_MARGIN))


def ice_cube(size, box, keys, cracked=False):
    """A frosty cube of ice in three-quarter view over `box` of a frame of
    `size`, which the body shows through, a clearer window in its middle: a
    snow cap on the top, a side in shade, white frost thick at the edges and
    corners, a drift along the foot, frost ferns from the corners, bubbles,
    cracks rising from the foot, a chipped corner and sparkles. `cracked` is
    the same cube about to thaw: split from a blow across its front, its cap
    melting and running (`crack_ice`), on the same box."""
    x0, y0, x1, y1 = box
    w, h = x1 - x0, y1 - y0
    d = max(2, min(6, round(min(w, h) * 0.14)))
    xf, yt = x1 - d, y0 + d
    fw, fh = xf - x0, y1 - yt
    px = {}
    for y in range(yt, y1 + 1):  # the front: clear in the middle, frosting outward
        for x in range(x0, xf + 1):
            u = (x - x0) / max(1, fw) - 0.5
            v = (y - yt) / max(1, fh) - 0.5
            edge = max(abs(u) * 2, abs(v) * 2)  # 0 at the centre, 1 at the edges
            corner = (abs(u) * 2) * (abs(v) * 2)
            frost = max(0.0, edge - 0.5) / 0.5 + corner * 0.3
            b = BAYER[y % 4][x % 4] / 16
            if frost > 0.85 and b < frost - 0.3:
                px[(x, y)] = 'φ'
            elif frost > b + 0.15:
                px[(x, y)] = 'x' if frost > 0.6 else 'u'
            elif edge < 0.6:
                px[(x, y)] = 'ϰ' if b > 0.12 else 'ϱ'
            else:
                px[(x, y)] = 'ϱ'
    drift = max(2, fh // 8)  # a drift of frost along the foot
    for x in range(x0, xf + 1):
        top = y1 - drift - round(1.2 * math.sin(x * 0.7) + unit(keys, 'drift', x) * 1.5)
        for y in range(max(yt, top), y1 + 1):
            px[(x, y)] = 'φ' if y > top + 1 or (x + y) % 3 else 'Ψ'
    for k in range(1, d + 1):  # the top: a snow cap
        y = yt - k
        for x in range(x0 + k, xf + k + 1):
            px[(x, y)] = 'Ψ' if BAYER[y % 4][x % 4] < 7 else 'φ'
    for k in range(1, d + 1):  # the side: in shade, frosted toward the top
        x = xf + k
        for y in range(yt - k, y1 - k + 1):
            v = (y - (yt - k)) / max(1, fh)
            b = BAYER[y % 4][x % 4]
            px[(x, y)] = 'x' if v < 0.25 and b < 8 else ('ς' if v > 0.55 and b > 5 else 'ϱ')
    for x in range(x0, xf + 1):  # edges: white along the top, frost at the sides
        px[(x, yt)] = 'Ψ'
    for y in range(yt, y1 + 1):
        px[(x0, y)] = 'φ'
        px[(xf, y)] = 'Ψ' if y < yt + fh // 3 else 'φ'
    for k in range(1, d + 1):
        px[(x0 + k, yt - k)] = 'Ψ'
        px[(xf + k, yt - k)] = 'Ψ'
        px[(xf + k, y1 - k)] = 'φ'
    for x in range(x0 + d, x1 + 1):
        px[(x, y0)] = 'Ψ'
    for y in range(y0, y1 - d + 1):
        px[(x1, y)] = 'φ'
    for cx, cy, sx, sy in ((x0, y1, 1, -1), (xf, y1, -1, -1), (x0, yt, 1, 1)):  # frost ferns
        length = max(3, min(fw, fh) // 3)
        for branch in range(2):
            angle = (0.35 + 0.5 * branch) * math.pi / 2
            for t in range(length):
                x = round(cx + sx * t * math.cos(angle))
                y = round(cy + sy * t * math.sin(angle))
                if (x, y) in px:
                    px[(x, y)] = 'Ψ'
                if t % 3 == 1:
                    for side in (-1, 1):
                        fx = round(x + sx * math.cos(angle + side * 0.8))
                        fy = round(y + sy * math.sin(angle + side * 0.8))
                        if (fx, fy) in px:
                            px[(fx, fy)] = 'x'
    r = 1 if min(w, h) < 60 else 2
    for i in range(2 + min(w, h) // 20):  # bubbles in the clear window
        bx = x0 + fw // 4 + int(unit(keys, 'bubble', i) * fw / 2)
        by = yt + fh // 4 + int(unit(keys, 'bubble-y', i) * fh / 2)
        for dx in range(-r, r + 1):
            for dy in range(-r, r + 1):
                if max(abs(dx), abs(dy)) == r and (bx + dx, by + dy) in px:
                    px[(bx + dx, by + dy)] = 'ϱ'
        px[(bx - r + 1, by - r + 1)] = 'Ψ'
    for i in range(2):  # cracks rising from the foot
        x, y = x0 + 2 + int(unit(keys, 'crack', i) * (fw - 4)), y1 - drift - 1
        for t in range(fh // 3):
            y -= 1
            if t % 3 == 0:
                x += 1 if unit(keys, 'crack-turn', i, t) < 0.5 else -1
            if (x, y) in px and px[(x, y)] in ('ϰ', 'ϱ', 'u'):
                px[(x, y)] = 'ς' if t % 4 else 'Ψ'
    for corner in ((x0, yt), (x0 + 1, yt), (x0, yt + 1)):  # a chipped corner
        px.pop(corner, None)
    arm = 1 if min(w, h) < 40 else 2 if min(w, h) < 90 else 3
    for gx, gy in ((x0 + fw // 4, yt + 2), (x0 + fw // 2, yt - d // 2), (xf - 2, yt + fh // 3)):
        if (gx, gy) in px:  # sparkles
            px[(gx, gy)] = 'Ψ'
            for t in range(1, arm + 1):
                for ddx, ddy in NEIGHBOURS:
                    if (gx + ddx * t, gy + ddy * t) in px:
                        px[(gx + ddx * t, gy + ddy * t)] = 'Ψ' if t < arm else 'x'
    if cracked:
        crack_ice(px, box, d, keys)
    return to_image(px, (size, size))


def crack_ice(px, box, d, keys):
    """A cube about to thaw, drawn over its whole self in `px`: fractures
    from a blow on its front, each a dark line with a lit lip and twigs off
    it, crushed white ice at the blow, the cap's snow gone glassy and
    running down the front in streaks, and chips out of the cap's lip. It
    keeps every outer row and column, so the cracked cube fits as the whole
    one does."""
    x0, y0, x1, y1 = box
    xf, yt = x1 - d, y0 + d
    fw, fh = xf - x0, y1 - yt
    inner = lambda x, y: x0 < x < xf and yt < y < y1  # noqa: E731
    cx = x0 + fw * (0.36 + 0.28 * unit(keys, 'blow'))
    cy = yt + fh * (0.3 + 0.25 * unit(keys, 'blow-y'))
    arms = 5 + min(fw, fh) // 28
    step = 1.0
    for arm in range(arms):  # the fractures, out from the blow
        angle = (arm + 0.35 * unit(keys, 'arm', arm)) * math.tau / arms
        x, y = cx, cy
        reach = max(fw, fh) * (0.45 + 0.35 * unit(keys, 'reach', arm))
        for t in range(int(reach)):
            angle += (unit(keys, 'bend', arm, t) - 0.5) * 0.5
            x += step * math.cos(angle)
            y += step * math.sin(angle)
            p = (round(x), round(y))
            if not inner(*p):
                break
            px[p] = 'ς'
            lip = (p[0] + 1, p[1]) if abs(math.sin(angle)) > 0.7 else (p[0], p[1] + 1)
            if inner(*lip) and px.get(lip) != 'ς':
                px[lip] = 'Ψ' if t % 3 else 'x'
            if t > 3 and t % 5 == 2 and unit(keys, 'twig', arm, t) < 0.55:
                side = 1 if unit(keys, 'twig-side', arm, t) < 0.5 else -1
                ta, tx, ty = angle + side * 0.9, x, y
                for _ in range(2 + min(fw, fh) // 24):
                    tx += math.cos(ta)
                    ty += math.sin(ta)
                    q = (round(tx), round(ty))
                    if not inner(*q):
                        break
                    px[q] = 'ς'
    r = 1 if min(fw, fh) < 40 else 2
    for dx in range(-r, r + 1):  # crushed ice where the blow landed
        for dy in range(-r, r + 1):
            q = (round(cx) + dx, round(cy) + dy)
            if inner(*q) and abs(dx) + abs(dy) <= r:
                px[q] = 'Ψ' if (dx + dy) % 2 == 0 else 'φ'
    for x in range(x0 + 1, x1):  # the cap's snow melting to clear ice
        for y in range(y0 + 1, yt + 1):
            if px.get((x, y)) == 'Ψ' and BAYER[y % 4][x % 4] % 3 == 0:
                px[(x, y)] = 'ϱ'
    for i in range(2 + fw // 14):  # meltwater running down the front
        x = x0 + 2 + int(unit(keys, 'melt', i) * max(1, fw - 4))
        length = 2 + int(unit(keys, 'melt-len', i) * fh * 0.3)
        for t in range(length):
            q = (x, yt + 1 + t)
            if inner(*q):
                px[q] = 'ϱ' if t < length - 1 else 'Ψ'
    for i in range(1 + fw // 24):  # chips out of the cap's lip
        x = x0 + 2 + int(unit(keys, 'chip', i) * max(1, fw - 6))
        for q in ((x, yt), (x + 1, yt), (x, yt - 1)):
            if x0 < q[0] < x1 and y0 < q[1] < y1:
                px.pop(q, None)


def status_ice(size, cracked=False):
    """Frozen, for a body with no fitted cube: the cube over the frame's
    status box."""
    return ice_cube(size, status_box(size), ('class', size), cracked)


def ice_glint():
    """A four-point sparkle, which a frozen target's `frozen_glints` twinkle
    over its cube."""
    size = ELEMENT_SIZES['ice_glint'][0]
    px, c = {}, size // 2
    px[(c, c)] = 'Ψ'
    for t in range(1, 6):
        for dx, dy in NEIGHBOURS:
            px[(c + dx * t, c + dy * t)] = 'Ψ' if t < 3 else 'x' if t < 5 else 'q'
    for t in (1, 2):
        for dx, dy in ((1, 1), (1, -1), (-1, 1), (-1, -1)):
            px[(c + dx * t, c + dy * t)] = 'u' if t == 1 else 'p'
    return to_image(px, (size, size))


def status_outline(size):
    """The outline a generic body leaves in a sprite of `size`: an upright
    ellipse over most of the frame, which the crackle hugs."""
    cx, cy, rx, ry = size / 2, size * 0.55, size * 0.34, size * 0.42
    inside = {(x, y) for y in range(size) for x in range(size)
              if ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2 <= 1}
    return sorted(p for p in inside
                  if any((p[0] + dx, p[1] + dy) not in inside for dx, dy in NEIGHBOURS))


def status_crackle(size, frame):
    """Shocked: violet arcs along the body's outline with a near-white spine,
    in two frames the game flickers between."""
    edge = status_outline(size)
    pixels = {}
    for i in range(3 + size // 32):
        a = edge[int(unit('arc', size, frame, i) * len(edge))]
        b = edge[int(unit('arc-b', size, frame, i) * len(edge))]
        if math.dist(a, b) > size * 0.45:
            b = (a[0] + (b[0] - a[0]) * 0.4, a[1] + (b[1] - a[1]) * 0.4)
        path = zigzag(a, b, 4, ('arc', size, frame, i), max(2, size // 16))
        glow, spine = Part((size, size)), Part((size, size))
        glow.limb(path, 3 if size >= 64 else 2)
        spine.limb(path, 1)
        pixels.update({p: 'Λ' for p in glow.pixels()})
        pixels.update({p: 'Σ' if (p[0] + p[1] + frame) % 3 else 'ϕ' for p in spine.pixels()})
    return to_image(pixels, (size, size))


def status_halo(size):
    """Stunned: small stars in a ring above the head."""
    pixels = {}
    cx, cy, rx, ry = size / 2, max(3, round(size * 0.07)), size * 0.22, size * 0.06
    stars = 4 if size < 64 else 5
    arm = 1 if size < 64 else 2
    for i in range(stars):
        a = i * math.tau / stars
        x, y = round(cx + rx * math.cos(a)), round(cy + ry * math.sin(a))
        pixels[(x, y)] = 'Z'
        for d in range(1, arm + 1):
            for dx, dy in NEIGHBOURS:
                pixels[(x + dx * d, y + dy * d)] = 'Y' if d < arm else 'J'
    return to_image(pixels, (size, size))


def ice_shell():
    """A frozen ball's ice, drawn over its sprite under a cold tint: a small
    frosty cube, matching the bodies'."""
    return ice_cube(64, (3, 3, 60, 60), ('ball',))


def orb(base, light, shadow, glint, halo):
    """A drop's orb: a sphere of `base` with a moonlit crown and a glint, in
    a dithered halo of `halo`, palette pixels only."""
    size = ELEMENT_SIZES['health_orb'][0]
    part = Part((size, size))
    c = size / 2 - 0.5
    part.ellipse((c, c), size / 2 - 5, size / 2 - 5)
    canvas = outline(shade(part.pixels(), base, light=light, shadow=shadow, rim=light), 'o', (size, size))
    for dx, dy in ((-3, -3), (-2, -3), (-3, -2)):
        canvas[(size // 2 + dx, size // 2 + dy)] = glint
    for y in range(size):
        for x in range(size):
            r = math.hypot(x - c, y - c)
            if (x, y) not in canvas and r <= size / 2 - 0.5 and dither(x, y, (size / 2 - r) / 4 * 0.6):
                canvas[(x, y)] = halo
    return to_image(canvas, (size, size))


def muzzle(frame):
    """The gun's flash, three frames: a hot star with diagonal sparks."""
    pixels = {}
    c = 8
    arm, diagonal = ((4, 2), (6, 3), (3, 1))[frame]
    pixels[(c, c)] = 'Ψ'
    for d in range(1, arm + 1):
        for dx, dy in NEIGHBOURS:
            pixels[(c + dx * d, c + dy * d)] = 'ZZYJK'[min(d, 4)]
    for d in range(1, diagonal + 1):
        for dx, dy in ((1, 1), (1, -1), (-1, 1), (-1, -1)):
            pixels[(c + dx * d, c + dy * d)] = 'Y' if d < 2 else 'J'
    return to_image(pixels, ELEMENT_SIZES['muzzle_0'])


def tracer():
    """A bullet's streak along +x: a dim tail to a white-hot head."""
    pixels = {}
    for x in range(32):
        pixels[(x, 1)] = 'Ψ' if x > 26 else 'Z' if x > 18 else 'Y' if x > 10 else 'x' if x > 4 else 'q'
        if x > 20:
            pixels[(x, 0)] = pixels[(x, 2)] = 'u'
    return to_image(pixels, ELEMENT_SIZES['tracer'])


def mark(kind):
    """An enemy's mark (01 (g)): '!' in the fire yellows while it is alert,
    '?' in bone while it searches; 16 px, a dark outline."""
    size = ELEMENT_SIZES['mark_alert']
    c = {}
    if kind == 'alert':
        for y in range(1, 10):
            w = 2 if y < 7 else 1
            for x in range(8 - w, 8 + w):
                c[(x, y)] = 'Y' if x == 8 - w else ('J' if x < 7 + w else 'K')
        for x, y in ((6, 12), (7, 12), (8, 12), (6, 13), (7, 13), (8, 13), (7, 14)):
            c[(x, y)] = 'Y' if (x, y) == (6, 12) else ('K' if y == 14 or x == 8 else 'J')
    else:
        glyph = ['.####.', '##..##', '##..##', '....##', '...##.', '..##..', '..##..', '......', '..##..',
                 '..##..']
        for j, row in enumerate(glyph):
            for i, ch in enumerate(row):
                if ch == '#':
                    c[(5 + i, 2 + j)] = 'w' if (i < 3 or j < 1) else 'σ'
    return to_image(outline(c, 'o', size), size)


def build_element_art(images):
    """The elements' and the gun's art: the bolt's segments, the status
    overlays in their three size classes, the fitted ice cubes and their
    cracked twins, which a thaw flickers to, the props' cubes, the ball's
    cube, the glint, the muzzle flash, the tracer, the drops' orbs and the
    enemies' marks. All unlit: overlays and flashes draw over the lit
    world."""
    for i in range(len(BOLT_EXITS)):
        images[f'bolt_{i}'] = bolt_segment(i)
    for name, size in STATUS_CLASSES.items():
        images[f'status_ice_{name}'] = status_ice(size)
        images[f'status_ice_{name}_cracked'] = status_ice(size, cracked=True)
        images[f'status_halo_{name}'] = status_halo(size)
        # Four frames; until e01sfx's asset session draws them, 2 and 3 are
        # copies of 0 and 1.
        for frame in range(4):
            images[f'status_crackle_{name}_{frame}'] = status_crackle(size, frame % 2)
    for kind in HURTBOXES:
        for suffix, cracked in (('', False), ('_cracked', True)):
            images[f'status_ice_{kind}{suffix}'] = ice_cube(
                ice_frame(kind), fitted_box(kind), (kind,), cracked)
    for kind, box in PROP_BOXES.items():
        images[f'status_ice_{kind}'] = ice_cube(64, box, (kind,))
    images['ice_shell'] = ice_shell()
    images['ice_glint'] = ice_glint()
    images['health_orb'] = orb('z', 'F', 'M', 'Ψ', 'X')
    images['mana_orb'] = orb('Й', 'З', 'И', 'Ю', 'И')
    for frame in range(3):
        images[f'muzzle_{frame}'] = muzzle(frame)
    images['tracer'] = tracer()
    for kind in ('alert', 'search'):
        images[f'mark_{kind}'] = mark(kind)


# The sprays' sprites (01 (h)): each spray is a muzzle flare at the staff,
# a stream of animated puffs that grow as they fly, a splash where the cone
# meets something and, for the ice, a bright core beam tiled down its axis;
# its particles stay as accents. Each clip is drawn along +x for `toys` to
# turn to the aim, in palette pixels: the fire ramp `M L K J Y Z`, smoke
# greys, the cold symbols and the white alpha steps. A clip: its frame
# size, its frames, the milliseconds a frame and whether it loops.
SPRAY_CLIPS = {
    'flame_muzzle': ((40, 32), 6, 50, True),
    'flame_puff': ((48, 48), 8, 40, False),
    'flame_splash': ((48, 48), 6, 60, True),
    'frost_muzzle': ((32, 32), 6, 50, True),
    'frost_puff': ((48, 48), 8, 45, False),
    'frost_core': ((32, 14), 8, 40, True),
    'frost_splash': ((48, 48), 6, 60, True),
}
SMOKE = ('/', 'b', 'Φ', 'B')


def ordered(x, y):
    """The ordered dither's offset at a pixel, in [-0.5, 0.5)."""
    return BAYER[y % 4][x % 4] / 16 - 0.5


def fire_symbol(heat, x, y, spread=0.08):
    h = heat + ordered(x, y) * spread
    if h < 0.1:
        return None
    for limit, s in ((0.24, 'M'), (0.40, 'L'), (0.56, 'K'), (0.72, 'J'), (0.86, 'Y')):
        if h < limit:
            return s
    return 'Z'


def wobble(seed, theta, f, terms=((3, 0.20, 1.7), (5, 0.13, -2.3), (8, 0.07, 3.1))):
    """A ragged edge: a sum of lobes round `theta`, turning with frame `f`."""
    return sum(a * math.sin(k * theta + unit(seed, k) * math.tau + w * f) for k, a, w in terms)


def flame_puff(f, n):
    """One puff of the flamethrower's stream, moving along +x: a white-hot
    tongue billowing into a rounded front with a licking tail, cooling
    through yellow and orange to red, trailing smoke as it dies."""
    w, h = SPRAY_CLIPS['flame_puff'][0]
    t = f / (n - 1)
    cx, cy = 28.0 - 2 * t, 25.0
    r = 4.5 + 11.0 * t ** 0.7
    tail = r * (1.45 + 0.1 * t)
    squash = 0.72 + 0.22 * t
    peak = 1.32 - 0.82 * t
    px = {}
    for y in range(h):
        for x in range(w):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            theta = math.atan2(dy, dx)
            c = math.cos(theta)
            base = r if c >= 0 else r + (tail - r) * (-c) ** 1.5
            edge = base * (1 + wobble('puff', theta, f, ((5, 0.09, 1.9), (9, 0.06, -2.6), (13, 0.045, 3.3))))
            d = math.hypot(dx, dy / squash)
            q = d / max(edge, 0.5)
            s = None
            if q <= 1.0:
                heat = max(0.0, 1 - q) ** 0.7 * peak + max(0.0, 0.4 - q) * (1 - t) * 0.8
                s = fire_symbol(heat, x, y)
            # A billow of smoke rising behind the cooling flame, growing with age.
            if t > 0.4 and s in (None, 'M'):
                sr = r * 0.62 * (t - 0.4) / 0.6 + 2.0
                sx, sy = cx - r * 0.55, cy - r * 0.5
                sa = math.atan2(y + 0.5 - sy, x + 0.5 - sx)
                sd = math.hypot(x + 0.5 - sx, (y + 0.5 - sy) * 1.15) / (
                    sr * (1 + wobble('smoke', sa, f, ((5, 0.12, 1.3), (9, 0.07, -2.1)))))
                if sd < 1 and unit('smoke-px', x, y, f) > 0.12 + max(0.0, sd - 0.7):
                    s = SMOKE[min(3, int(sd * 2.4) + (t > 0.8))]
            if s:
                px[(x, y)] = s
    # A few embers flung off the rim.
    if 1 <= f <= 5:
        for k in range(3):
            a = unit('puff-ember', f, k) * math.tau
            rr = r * (1.2 + 0.4 * unit('puff-ember-r', f, k))
            ex, ey = round(cx + math.cos(a) * rr), round(cy + math.sin(a) * rr * squash)
            if 0 < ex < w - 1 and 0 < ey < h - 1 and (ex, ey) not in px:
                px[(ex, ey)] = 'J' if k % 2 else 'Y'
    return to_image(px, (w, h))


def flame_muzzle(f, n):
    """The flame bursting from the staff's orb: a white-hot root, at (3, 16),
    opening into a cone of licking tongues, flickering in a loop."""
    w, h = SPRAY_CLIPS['flame_muzzle'][0]
    ox, oy = 3.0, 16.0
    phase = f / n * math.tau
    length = 32 + 3 * math.sin(phase)
    px = {}
    for y in range(h):
        for x in range(w):
            u = x + 0.5 - ox
            if u < 0 or u > length + 4:
                continue
            v = y + 0.5 - oy
            half = 1.8 + u * 0.42 + 1.4 * math.sin(u * 0.55 - phase * 2 + 0.6) * min(1.0, u / 10)
            half += 0.8 * math.sin(u * 1.3 + phase * 3) * min(1.0, u / 14)
            if abs(v) > half:
                continue
            across = abs(v) / max(half, 0.5)
            along = u / length
            heat = max(0.0, 1 - along) ** 0.9 * max(0.0, 1 - across ** 1.6) * 1.25 + max(0.0, 0.3 - along) * 0.9
            s = fire_symbol(heat, x, y)
            if s:
                px[(x, y)] = s
    return to_image(px, (w, h))


def flame_splash(f, n):
    """Where the flames meet something, at (36, 25): tongues splashing back
    toward the caster and climbing its face, rounded where they wrap the
    point, embers thrown off."""
    w, h = SPRAY_CLIPS['flame_splash'][0]
    hx, hy = 36.0, 25.0
    phase = f / n * math.tau
    px = {}
    for y in range(h):
        for x in range(w):
            dx, dy = x + 0.5 - hx, y + 0.5 - hy
            theta = math.atan2(dy, dx)
            back = max(0.0, -math.cos(theta))
            up = max(0.0, -math.sin(theta))
            reach = 6 + 11 * back ** 0.6 + 8 * up ** 2 * (1 - back)
            reach *= 1 + wobble('splash', theta, phase, ((4, 0.18, 2.0), (7, 0.1, -3.0), (11, 0.05, 4.0)))
            d = math.hypot(dx, dy * 0.9)
            if d > reach:
                continue
            q = d / reach
            heat = max(0.0, 1 - q) ** 0.75 * 1.25 + max(0.0, 0.25 - q) * 0.8
            s = fire_symbol(heat, x, y)
            if s:
                px[(x, y)] = s
    for k in range(5):
        a = math.pi * (0.55 + 0.9 * unit('splash-ember', f, k))
        rr = 14 + 9 * unit('splash-ember-r', f, k)
        ex, ey = round(hx + math.cos(a) * rr), round(hy + math.sin(a) * rr)
        if 0 < ex < w - 1 and 0 < ey < h - 1 and (ex, ey) not in px:
            px[(ex, ey)] = 'Y' if k % 2 else 'J'
    return to_image(px, (w, h))


def flake(px, x, y, big, bright):
    """A tiny snowflake: a plus, or a plus with diagonal tips."""
    c = 'Ψ' if bright else '`'
    px[(x, y)] = c
    for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        px.setdefault((x + dx, y + dy), 'φ' if bright else ',')
    if big:
        for dx, dy in ((2, 0), (-2, 0), (0, 2), (0, -2)):
            px.setdefault((x + dx, y + dy), '`')
        for dx, dy in ((1, 1), (-1, -1), (1, -1), (-1, 1)):
            px.setdefault((x + dx, y + dy), 'ϱ')


def frost_puff(f, n):
    """One puff of the ice beam's stream: a hard bright knot of crystals
    blooming into a cold swirl of frost, flakes riding out on it, then
    thinning away; translucent at its edge."""
    w, h = SPRAY_CLIPS['frost_puff'][0]
    t = f / (n - 1)
    cx, cy = 25.0 - 3 * t, 24.0
    r0 = 4.0 + 14.0 * t ** 0.7
    px = {}
    for y in range(h):
        for x in range(w):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            theta = math.atan2(dy, dx)
            edge = r0 * (1 + wobble('frost', theta, f, ((4, 0.1, 1.1), (7, 0.07, -1.7), (11, 0.04, 2.3))))
            d = math.hypot(dx / (1.25 - 0.15 * t), dy / (0.82 + 0.14 * t))
            q = d / max(edge, 0.5)
            if q > 1:
                continue
            v = (1 - q) * (1.2 - 0.72 * t) + ordered(x, y) * 0.1
            if math.sin(theta * 2 + q * 5 - f * 0.9) > 0.6 and q > 0.25:
                v += 0.12
            # It thins away from its edge inward as it dies.
            if t > 0.5 and unit('frost-thin', x, y, f) < (t - 0.5) * 1.9 - (1 - q) * 0.35:
                continue
            for limit, s in ((0.95, 'Ψ'), (0.8, 'φ'), (0.64, '`'), (0.5, 'ϱ'), (0.36, ','), (0.22, 'ϰ'), (0.08, 'ς')):
                if v > limit:
                    px[(x, y)] = s
                    break
    for k in range(6):
        a = unit('frost-flake', k) * math.tau + t * 0.8
        rr = r0 * (0.35 + 0.75 * unit('frost-flake-r', k)) * (0.6 + 0.6 * t)
        fx, fy = round(cx + math.cos(a) * rr), round(cy + math.sin(a) * rr * 0.9)
        if 2 < fx < w - 3 and 2 < fy < h - 3 and t < 0.95:
            flake(px, fx, fy, big=k % 3 == 0 and t < 0.8, bright=t < 0.6 or k % 2 == 0)
    return to_image(px, (w, h))


def frost_core(f, n):
    """A tile of the ice beam's core, seamless across its width: a white line
    in a cyan sheath, a frost helix twisting along it and faceted crystals
    flowing outward, four pixels a frame."""
    w, h = SPRAY_CLIPS['frost_core'][0]
    mid = 6.5
    px = {}
    shift = f * (w / n)
    for x in range(w):
        phase = (x - shift) / w * math.tau
        helix = mid + 3.6 * math.sin(phase * 2)
        helix2 = mid - 3.6 * math.sin(phase * 2)
        for y in range(h):
            v = abs(y + 0.5 - (mid + 0.5))
            if v < 1.0:
                s = 'Ψ'
            elif v < 2.0:
                s = 'φ' if (x + f) % 5 else 'Ψ'
            elif v < 3.0:
                s = '`'
            elif v < 4.0:
                s = 'ϱ' if ordered(x, y) < 0.25 else ','
            elif v < 5.2:
                s = 'ϰ' if ordered(x, y) < 0.1 else 'q'
            elif v < 6.4:
                s = 'p' if ordered(x, y) < -0.1 else None
            else:
                s = None
            for hy in (helix, helix2):
                if abs(y + 0.5 - (hy + 0.5)) < 0.6 and v >= 2.0:
                    s = '`' if v < 4.5 else 'ϱ'
            if s:
                px[(x, y)] = s
    # Crystal facets flowing along it.
    for k in range(4):
        fx = round((k * 8 + 3 + shift) % w)
        side = -1 if k % 2 else 1
        for dy, half in ((3, 1), (4, 2), (5, 1)):
            y = round(mid + side * dy)
            for dx in range(-half, half + 1):
                if 0 <= y < h:
                    px[((fx + dx) % w, y)] = 'φ' if dx == 0 else '`'
    return to_image(px, (w, h))


def frost_muzzle(f, n):
    """The ice beam leaving the orb: a bright knot, at (8, 16), in a cold
    halo, ringed by diamond-section ice shards that flare forward and
    flicker."""
    w, h = SPRAY_CLIPS['frost_muzzle'][0]
    ox, oy = 8.0, 16.0
    px = {}
    for y in range(h):
        for x in range(w):
            d = math.hypot(x + 0.5 - ox, y + 0.5 - oy)
            if d < 2.6:
                px[(x, y)] = 'Ψ'
            elif d < 4.0:
                px[(x, y)] = 'φ'
            elif d < 6.0:
                px[(x, y)] = '`' if ordered(x, y) < -0.2 else 'ϱ'
            elif d < 8.0 and ordered(x, y) < 0.1:
                px[(x, y)] = 'ϰ'
    for k in range(5):
        a = (k - 2) * 0.55 + 0.07 * math.sin(f + k)
        length = 10 + 5 * unit('frost-spike', k) + 2.5 * math.sin(f / n * math.tau + k * 1.7)
        for i in range(int(length)):
            frac = i / length
            sx, sy = ox + math.cos(a) * (3 + i), oy + math.sin(a) * (3 + i)
            width = 2.2 * (frac / 0.3 if frac < 0.3 else (1 - frac) / 0.7)
            for o in (-2, -1, 0, 1, 2):
                if abs(o) <= width:
                    q = (round(sx - math.sin(a) * o), round(sy + math.cos(a) * o))
                    if 0 <= q[0] < w and 0 <= q[1] < h:
                        px[q] = 'Ψ' if o == 0 and frac < 0.6 else ('`' if o <= 0 else ',')
    return to_image(px, (w, h))


def frost_splash(f, n):
    """Where the ice beam meets something, at (36, 24): diamond ice shards
    growing back toward the caster from a frost burst at the point, rounded
    where it wraps the point, sparkles round it."""
    w, h = SPRAY_CLIPS['frost_splash'][0]
    hx, hy = 36.0, 24.0
    px = {}
    ring = 7 + 2 * math.sin(f / n * math.tau)
    for y in range(h):
        for x in range(w):
            dx, dy = x + 0.5 - hx, (y + 0.5 - hy) * 0.85
            d = math.hypot(dx * (1.6 if dx > 0 else 1.0), dy)
            if d < 3:
                px[(x, y)] = 'Ψ'
            elif d < 5:
                px[(x, y)] = 'φ'
            elif d < ring:
                px[(x, y)] = '`' if ordered(x, y) < -0.25 else 'ϱ'
            elif d < ring + 3 and ordered(x, y) < 0.0:
                px[(x, y)] = 'ϰ'
    for k in range(7):
        a = math.pi + (k - 3) * 0.4 + 0.08 * math.sin(f * 1.3 + k)
        grow = 0.75 + 0.25 * math.sin(f / n * math.tau + k * 2.1)
        length = (10 + 8 * unit('frost-splash', k)) * grow
        for i in range(int(length)):
            frac = i / length
            sx, sy = hx + math.cos(a) * (3 + i), hy + math.sin(a) * (3 + i)
            width = 2.4 * (frac / 0.3 if frac < 0.3 else (1 - frac) / 0.7)
            for o in (-2, -1, 0, 1, 2):
                if abs(o) <= width:
                    q = (round(sx - math.sin(a) * o), round(sy + math.cos(a) * o))
                    if 0 <= q[0] < w and 0 <= q[1] < h:
                        px[q] = 'Ψ' if o == 0 and frac < 0.5 else ('`' if o <= 0 else ',')
    for k in range(4):
        a = math.pi * (0.5 + unit('frost-spark', f, k))
        rr = 13 + 9 * unit('frost-spark-r', f, k)
        sx, sy = round(hx + math.cos(a) * rr), round(hy + math.sin(a) * rr)
        if 1 < sx < w - 2 and 1 < sy < h - 2:
            flake(px, sx, sy, big=False, bright=True)
    return to_image(px, (w, h))


SPRAY_DRAW = {
    'flame_muzzle': flame_muzzle,
    'flame_puff': flame_puff,
    'flame_splash': flame_splash,
    'frost_muzzle': frost_muzzle,
    'frost_puff': frost_puff,
    'frost_core': frost_core,
    'frost_splash': frost_splash,
}


# The frost (01 (j)): a chilled creature's frost in four stages by the cold
# it has taken, from the first point of it: rime at the feet, frost up the
# legs, over the body, then a crust over all of it, until the freeze's cube
# takes over. Fitted to each kind's hurtbox on its frame as the cube is, a
# rounded box the body shows through, dense at the foot and along the
# edges, glazed thinly in the middle, with feathers climbing from the foot
# and crystals on its ragged top; the size classes keep a set over the
# status box for a body with no fitted one. After the owner's review of the
# first draw, white and subtle: rime in the palette's white alpha steps
# (`Ψ φ x ϱ`, a sparse `u`) with no dark rim, its sides fading toward its
# top so no frame shows round the body.
FROST_STAGES = 4
FROST_TOPS = (0.2, 0.45, 0.72, 1.0)  # how far up the box each stage reaches
FROST_KINDS = ('wisp', 'beetle', 'bat', 'troll', 'goblin', 'dummy')
# How far past its box a frost reaches, room for its crust's spikes, which
# `generate.check_frost_fits` holds it to.
FROST_REACH = 6


def frost(size, box, keys, stage):
    """The frost of `stage` on a `size` frame over `box`, white rime denser at
    the foot and the edges, a faint glaze the body shows through, white
    feathers and crystals."""
    x0, y0, x1, y1 = box
    w, h = x1 - x0 + 1, y1 - y0 + 1
    reach = FROST_TOPS[stage - 1]
    radius = max(2.0, min(w, h) * 0.3)
    small = min(w, h) < 30
    px = {}

    def wobble_at(x, y, scale=1.0):
        return scale * (1.4 * math.sin(x * 0.9 + unit(keys, 'wx') * 6) * math.cos(y * 0.7 + unit(keys, 'wy') * 6)
                        + 0.8 * (unit(keys, 'edge', x // 2, y // 2) - 0.5))

    def edge_distance(x, y):
        cx = min(max(x, x0 + radius), x1 - radius)
        cy = min(max(y, y0 + radius), y1 - radius)
        return radius - math.hypot(x - cx, y - cy)

    def top_at(x):
        u = (x - x0) / max(1, w - 1)
        climb = 0.08 * (1 - math.sin(u * math.pi)) if stage < FROST_STAGES else 0.0
        lump = (0.06 * math.sin(u * 7.1 + unit(keys, 'lump') * 6)
                + 0.04 * math.sin(u * 17 + 1.3 + stage)) * (stage < FROST_STAGES)
        spike = 0.05 * max(0.0, math.sin(u * 29 + unit(keys, 'spike-run') * 6)) ** 6
        return y1 + 1 - h * min(1.0, reach + climb + lump + spike)

    patches = []
    for i in range(2 + stage + w // 16 + 3 * (stage == FROST_STAGES)):
        px_ = x0 + unit(keys, 'patch-x', i) * w
        py_ = y1 + 1 - unit(keys, 'patch-y', i) * h * reach * 0.95
        r = (1.6 + 2.2 * unit(keys, 'patch-r', i)) * (0.8 + 0.2 * stage) * (0.6 if small else 1.0)
        patches.append((px_, py_, r))

    for y in range(max(0, y0 - 2), min(size, y1 + 1)):
        for x in range(max(0, x0 - 3), min(size, x1 + 4)):
            inside = edge_distance(x, y) + wobble_at(x, y)
            if inside < 0:
                continue
            top = top_at(x)
            if y < top + wobble_at(x, y, 0.8):
                continue
            depth = (y - top) / max(1.0, y1 + 1 - top)
            near_edge = max(0.0, 1.0 - inside / (2.5 if small else 3.5))
            foot = max(0.0, 1.0 - (y1 - y) / max(2.0, h * 0.08 + stage))
            # The sides frost up from the foot, fading toward the frost's
            # top, so no frame shows round the body.
            rime = max(foot, near_edge * (0.5 + 0.08 * stage) * smoothstep(depth / 0.6))
            for cx, cy, r in patches:
                if cy >= top:
                    rime = max(rime, 0.85 * (1.0 - math.hypot(x - cx, (y - cy) * 1.2) / r))
            if stage == FROST_STAGES:
                rime = max(rime, 0.22)
            b = BAYER[y % 4][x % 4] / 16
            if rime > 0.75:
                px[(x, y)] = 'Ψ' if b < 0.45 else 'φ'
            elif rime > 0.55:
                px[(x, y)] = 'x' if b < 0.5 else 'ϱ'
            elif rime > 0.38 and b < 0.35:
                px[(x, y)] = 'x'
            elif depth > 0.3 and b < 0.06:
                px[(x, y)] = 'u'
    # White feathers climbing from the foot, thin, longer each stage.
    for i in range(1 + w // 18 + stage):
        fx = round(x0 + 1 + unit(keys, 'feather-side', i) * (w - 3))
        length = int(h * reach * (0.3 + 0.35 * unit(keys, 'feather-len', i)))
        lean = (unit(keys, 'feather-lean', i) - 0.5) * 0.5
        bend = (unit(keys, 'feather-bend', i) - 0.5) * 0.04
        for k in range(length):
            x = round(fx + lean * k + bend * k * k)
            y = y1 - k
            if (not (0 <= x < size and 0 <= y < size) or y < top_at(x) - 1
                    or edge_distance(x, y) < 0.5):
                break
            px[(x, y)] = 'Ψ' if k < length - 2 else 'x'
            if k % 3 == 1 and not small:
                for d in (-1, 1):
                    q = (x + d, y - 1)
                    if q in px or edge_distance(*q) > 0:
                        px[q] = 'u'
    # Crystals on the ragged top edge, sparse.
    for x in range(x0, x1 + 1):
        if unit(keys, 'crystal', x, stage) > (0.16 if not small else 0.26):
            continue
        top = math.floor(top_at(x))
        if edge_distance(x, top) < -1:
            continue
        length = 1 + int(unit(keys, 'crystal-len', x) * (2 if small else 3))
        for k in range(length):
            y = top - k
            if 0 <= y < size:
                px[(x, y)] = 'Ψ' if k < length - 1 else 'x'
    arm = 1 if small else 2
    for i in range(stage):
        sx = round(x0 + 2 + unit(keys, 'spark', i) * (w - 4))
        sy = round(max(top_at(sx) + 1, y1 - unit(keys, 'spark-y', i) * h * reach * 0.8))
        if (sx, sy) in px:
            px[(sx, sy)] = 'Ψ'
            for k in range(1, arm + 1):
                for dx, dy in NEIGHBOURS:
                    q = (sx + dx * k, sy + dy * k)
                    if q in px:
                        px[q] = 'Ψ' if k < arm else 'x'
    if stage == FROST_STAGES:
        for i in range(5 + w // 8):
            a = unit(keys, 'crust', i) * math.tau
            cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
            ex = min(max(cx + math.cos(a) * (w / 2 + 0.5), x0), x1)
            ey = min(max(cy + math.sin(a) * (h / 2 + 0.5), y0), y1)
            for k in range(1 + int(unit(keys, 'crust-len', i) * (2 if small else 3))):
                q = (round(ex + math.cos(a) * k), round(ey + math.sin(a) * k))
                if 0 <= q[0] < size and 0 <= q[1] < size:
                    px.setdefault(q, 'Ψ' if k == 0 else 'x')
    return to_image(px, (size, size))


def build_frost_art(images):
    """Every frost sprite, unlit: `frost_<kind>_<stage>` on the kind's frame
    at its fitted box, `frost_<class>_<stage>` over the class's status box."""
    for kind in FROST_KINDS:
        for stage in range(1, FROST_STAGES + 1):
            images[f'frost_{kind}_{stage}'] = frost(ice_frame(kind), fitted_box(kind), ('frost', kind), stage)
    for name, size in STATUS_CLASSES.items():
        for stage in range(1, FROST_STAGES + 1):
            images[f'frost_{name}_{stage}'] = frost(size, status_box(size), ('frost', name), stage)


def build_spray_art(images):
    """Every frame of the sprays' clips, unlit: `<clip>_<n>`."""
    for clip, (_, frames, _, _) in SPRAY_CLIPS.items():
        for f in range(frames):
            images[f'{clip}_{f}'] = SPRAY_DRAW[clip](f, frames)


# The force push's wave (01 (j)): a front of bent air flying from the wizard
# toward the cursor, a crescent bowed forward, its leading edge a bright
# seam with a gravity-purple fringe (`Щ Ш`), two fainter ripples of warped
# air behind it and speed streaks trailing back, tapering and fading to its
# tips. Drawn along +x for `toys::wave` to turn to the aim, its across axis
# scaled to the front's width, so its frames are sampled linearly; it
# shimmers over a looping clip. The dust it raises where it scrapes a
# floor: a low cloud kicked along +x from its foot at the art's
# bottom-left, billowing and thinning. A clip: its frame size, its frames,
# the milliseconds a frame and whether it loops.
PUSH_CLIPS = {
    'push_wave': ((72, 128), 6, 40, True),
    'push_dust': ((48, 32), 7, 55, False),
}
LINEAR_FILTER |= {f'push_wave_{f}' for f in range(PUSH_CLIPS['push_wave'][1])}
PUSH_DUST = ('h', 'H', 'f', 'κ', 'λ', 'ι', 'μ')


def push_wave(f, n):
    (w, h), _, _, _ = PUSH_CLIPS['push_wave']
    cy = (h - 1) / 2
    px = {}
    lead = w - 6

    def front(y):
        # The leading edge's x at row y: bowed forward at the middle, the
        # tips swept back.
        v = (y - cy) / (h / 2)
        return lead - 22.0 * v * v - 3.0 * v ** 4

    def taper(y):
        v = abs(y - cy) / (h / 2)
        return max(0.0, 1.0 - v ** 2.4)

    phase = f / n * math.tau
    for y in range(h):
        t = taper(y)
        if t <= 0.02:
            continue
        x0 = front(y) + 1.2 * math.sin(phase + y * 0.21)
        thick = 2.0 + 7.5 * t
        for x in range(w):
            d = x0 - x  # how far behind the edge
            if d < -1.0:
                continue
            a = 0.0
            # The seam: bright on the edge, a band of warp behind it.
            if d <= 1.0:
                a = 255 * t
            elif d <= thick:
                a = (210 - 150 * (d / thick)) * t
            # Two ripples of warped air behind, fainter, drifting back.
            for k, (gap, width, peak) in enumerate(((thick + 7.0, 2.2, 120), (thick + 17.0, 2.6, 70))):
                ripple = gap + 2.5 * math.sin(phase * (1 + k) + y * (0.13 + 0.04 * k))
                a = max(a, peak * t ** (1.3 + k) * math.exp(-((d - ripple) / width) ** 2))
            if a < 3:
                continue
            if d <= 1.0 and t > 0.25:
                s = 'Ψ' if (y + f) % 7 else 'Щ'
            elif d <= 3.0 and t > 0.45 and (x + y + f) % 4 == 0:
                s = 'Щ'
            elif thick - 2 < d <= thick and t > 0.5 and dither(x, y, 0.35):
                s = 'Ш'
            else:
                s = white_alpha(x, y, min(255, a))
            if s:
                px[(x, y)] = s
    # Speed streaks trailing back from the seam, each a bright head and a
    # thinning tail, jittered from frame to frame.
    for k in range(13):
        y = round(cy + (unit('streak-y', k) - 0.5) * h * 0.82)
        t = taper(y)
        if t < 0.15:
            continue
        head = front(y) - 3 - 2 * unit('streak-head', k, f)
        length = (14 + 34 * unit('streak-len', k)) * t * (0.75 + 0.5 * unit('streak-f', k, f))
        for i in range(int(length)):
            x = round(head - i)
            if x < 0:
                break
            fade = 1 - i / length
            s = white_alpha(x, y, 200 * fade * t) if i > 1 else 'Ψ'
            if s and (x, y) not in px:
                px[(x, y)] = s
    return to_image(px, (w, h))


def push_dust(f, n):
    (w, h), _, _, _ = PUSH_CLIPS['push_dust']
    t = f / (n - 1)
    px = {}
    # A few lobes rolling forward and up from the foot, growing and thinning.
    lobes = [(4 + 30 * t + 6 * unit('lobe-x', k), h - 3 - (4 + 12 * t) * unit('lobe-y', k),
              (3 + 9 * t) * (0.6 + 0.6 * unit('lobe-r', k))) for k in range(6)]
    for y in range(h):
        for x in range(w):
            v = 0.0
            for cx, cy, r in lobes:
                d = math.hypot((x + 0.5 - cx) / 1.25, y + 0.5 - cy) / max(r, 0.5)
                v = max(v, 1 - d)
            if v <= 0:
                continue
            v = v * (1.15 - 0.95 * t) + (BAYER[y % 4][x % 4] / 16 - 0.5) * 0.25
            # It thins from its edge inward as it settles.
            if t > 0.45 and unit('dust-thin', x, y, f) < (t - 0.45) * 1.6:
                continue
            if v > 0.08:
                px[(x, y)] = PUSH_DUST[min(len(PUSH_DUST) - 1, int(v * len(PUSH_DUST)))]
    # Grit flung ahead of the cloud.
    for k in range(5):
        if f >= 5:
            break
        gx = round(10 + (12 + 22 * unit('grit-x', k)) * (t + 0.15))
        gy = round(h - 4 - (2 + 10 * unit('grit-y', k)) * math.sin(min(1.0, t * 2.2) * math.pi * 0.8))
        if 0 <= gx < w and 0 <= gy < h:
            px[(gx, gy)] = 'Φ' if k % 2 else 'b'
    return to_image(px, (w, h))


PUSH_DRAW = {'push_wave': push_wave, 'push_dust': push_dust}


def build_push_art(images):
    """Every frame of the force push's clips, unlit: `<clip>_<n>`."""
    for clip, (_, frames, _, _) in PUSH_CLIPS.items():
        for f in range(frames):
            images[f'{clip}_{f}'] = PUSH_DRAW[clip](f, frames)


# The HUD's art (01 (h)): the bars' liquids, palette strips tileable
# across their width, which the HUD scrolls inside each fill, deep to lit
# with a glint; and the gun's rounds as cartridges, live and spent.
LIQUID_RAMPS = {
    'health': ('Θ', 'M', 'ϑ', 'z', 'F', '+'),
    'mana': ('в', 'И', 'Й', 'З', 'Ю', 'Ю'),
    # e01tomes: the pact's shield over the health, a light, cool blue.
    'shield': (',', 'Ч', 'Ч', 'Ю', 'Я', 'Я'),
}
LIQUID_GLINT = 'φ'


def hud_liquid(kind):
    """A strip of liquid, tileable across: lit at its surface, deep at its
    bottom, crossed by three slow currents, with bubbles wrapping across
    the seam."""
    w, h = ELEMENT_SIZES[f'hud_liquid_{kind}']
    ramp = LIQUID_RAMPS[kind]
    px = {}
    for y in range(h):
        for x in range(w):
            v = y / (h - 1)
            depth = 0.86 - 0.62 * v
            flow = (0.10 * math.sin(math.tau * (x / w * 2 + 0.25 * v))
                    + 0.07 * math.sin(math.tau * (x / w * 5 - 0.6 * v) + 1.7)
                    + 0.05 * math.sin(math.tau * (x / w * 9 + 0.9 * v) + 0.4))
            value = depth + flow
            if y == 1:
                value += 0.12  # the surface's sheen
            value += (BAYER[y % 4][x % 4] / 16 - 0.5) * 0.16
            px[(x, y)] = ramp[min(len(ramp) - 1, max(0, int(value * len(ramp))))]
    for i in range(7):
        bx = int(unit('bubble', kind, i) * w)
        by = 2 + int(unit('bubble-y', kind, i) * (h - 5))
        r = 1 if i % 3 else 2
        for dx in range(-r, r + 1):
            for dy in range(-r, r + 1):
                d = math.hypot(dx, dy)
                if r - 0.6 <= d <= r + 0.4:
                    px[((bx + dx) % w, by + dy)] = ramp[-2]
        px[((bx - (r > 1)) % w, by - r + 1)] = LIQUID_GLINT
    return to_image(px, (w, h))


def hud_bullet(spent=False):
    """A cartridge, upright: a copper round over a brass case with its rim;
    a spent one an empty case's dark outline."""
    w, h = ELEMENT_SIZES['hud_bullet']
    px = {}
    for y in range(h):
        for x in range(w):
            cx = x - (w - 1) / 2
            if y < 4:  # the round, an ogive
                half = 0.6 + y * 0.55
                if abs(cx) <= half:
                    px[(x, y)] = ('a' if cx < -0.5 else '"' if cx > 0.5 else "'") if y > 0 else "'"
            elif y < h - 1:  # the case
                if abs(cx) <= 2:
                    px[(x, y)] = 'J' if cx < -0.5 else 'A' if cx < 1 else '('
                    if y == 4:
                        px[(x, y)] = '('  # the crimp
            else:  # the rim
                px[(x, y)] = '('
    if spent:
        px = {p: ('Φ' if s in "a\"'" else 'ν') for p, s in px.items() if p[1] >= 4}
        for p in [p for p in px if 0 < p[0] < w - 1 and 4 < p[1] < h - 1]:
            del px[p]
    return to_image(px, (w, h))


def title_star():
    """The title's twinkle (01 (h)): a four-point star in the palette's white
    steps, its core `Ψ` and its arms stepping down `x u q p l k`, unlit."""
    w, h = ELEMENT_SIZES['title_star']
    c = w // 2
    px = {}
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            px[(c + dx, c + dy)] = 'Ψ' if dx == 0 or dy == 0 else 'x'
    for d, s in zip(range(2, 8), 'xuqplk'):
        for sx, sy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            px[(c + sx * d, c + sy * d)] = s
    for d, s in zip(range(2, 4), 'pk'):
        for sx, sy in ((1, 1), (-1, 1), (1, -1), (-1, -1)):
            px[(c + sx * d, c + sy * d)] = s
    return to_image(px, (w, h))


def build_hud_art(images):
    for kind in LIQUID_RAMPS:
        images[f'hud_liquid_{kind}'] = hud_liquid(kind)
    images['hud_bullet'] = hud_bullet()
    images['hud_bullet_spent'] = hud_bullet(spent=True)
    images['title_star'] = title_star()


# The tomes (e01tomes): three books hidden off the routes, each its own
# cover, colour and glow. A book is drawn closed on its 32 px frame, its
# front cover facing, its head and fore-edge receding up and to the right,
# its centre the frame's (16, 16). It is lit, so its glowing parts come from
# the glow set alone (`art.GLOW`): the pact's ember from `Ъ y Y`, bounding's
# pale cyan from `Ч Я`, arcane's violet over white from `Э Ы`, the last four
# used by no other art. Its idle loop is a slow shimmer of those parts, a
# bright band moving round them, never a flicker.
TOME_KINDS = ('pact', 'bounding', 'arcane')
TOME_FRAMES = 4
TOME_SIZE = ELEMENT_SIZES['tome_pact_idle_0']
# Each book's front cover (an inclusive box), how far its sheets recede
# right (`depth`, its back board's edge the last pixel) and up (`rise`),
# its cover's shading and its sheets' tones: the seams between them, the
# sheets' edges and, for a glowing book, the glow's three steps.
TOME_LOOKS = {
    'pact': dict(front=(6, 7, 21, 28), depth=5, rise=2,
                 cover=dict(base='а', light='ω', shadow='б', back='б', rim='п'),
                 sheet=('M', 'L'), glow=('Ъ', 'y', 'Y')),
    'bounding': dict(front=(7, 7, 21, 28), depth=4, rise=2,
                     cover=dict(base='З', light='Ю', shadow='Й', back='Й', rim='Ю'),
                     sheet=(',', '`'), glow=('Ч', 'Ч', 'Я')),
    'arcane': dict(front=(7, 7, 21, 28), depth=4, rise=2,
                   cover=dict(base='Δ', light='Λ', shadow='Ж', back='Ж', rim='Λ'),
                   sheet=('σ', 'w'), glow=None),
}


def tome_parts(kind):
    """A closed book's parts: its front cover; its sheets, from a pixel inside
    the cover's edges back to the back board, each pixel by the sheet it
    shows and whether it lies on the head; the spine's cap at the head's
    left; and the back board's edge about the sheets."""
    look = TOME_LOOKS[kind]
    x0, y0, x1, y1 = look['front']
    depth, rise = look['depth'], look['rise']
    cover = {(x, y) for x in range(x0, x1 + 1) for y in range(y0, y1 + 1)}
    sheets = {}
    for k in range(depth, 0, -1):
        up = -(-k * rise // depth)
        for x in range(x0 + 2, x1):
            for y in range(y0 + 1, y1):
                sheets[(x + k, y - up)] = (k, y == y0 + 1)
    spine = {(x, y0 - 1) for x in range(x0 + 1, x0 + 5)} | {(x, y0 - 2) for x in range(x0 + 2, x0 + 5)}
    spine -= cover
    sheets = {p: v for p, v in sheets.items() if p not in cover and p not in spine}
    back = {(x + depth, y - rise) for x, y in cover} - cover - set(sheets) - spine
    return cover, sheets, spine, back


def tome_shimmer(p, f, width=0.3):
    """How bright a glowing pixel `p` along its path (0 to 1) stands in frame
    `f`: two soft bands moving along it, a quarter of a band a frame."""
    u = (p * 2 - f / TOME_FRAMES) % 1.0
    return max(0.0, 1 - min(u, 1 - u) / width)


def tome_pages(c, kind, f, sheets):
    """The sheets' edges on the head and the fore-edge: each sheet's edge,
    and the seams between them, where a glowing book's light seeps out,
    brightest where the shimmer runs from the spine over the head and down."""
    look = TOME_LOOKS[kind]
    x0, y0, x1, y1 = look['front']
    seam, edge = look['sheet']
    for (x, y), (k, head) in sheets.items():
        along = ((x - x0) / (x1 - x0 + look['depth']) * 0.3 if head
                 else 0.3 + (y - y0 + look['rise']) / (y1 - y0) * 0.7)
        # Every other sheet a seam, and some sheets proud of their
        # neighbours by a tone.
        gap = (k + (x if head else 0)) % 2 == 0
        if look['glow'] is None:
            c[(x, y)] = seam if gap and unit('tome-seam', kind, k, y // 2) < 0.7 else edge
            continue
        low, mid, hot = look['glow']
        if gap:
            shine = tome_shimmer(along, f)
            c[(x, y)] = hot if shine > 0.5 else (mid if shine > 0.05 or unit('tome-ember', kind, x, y) < 0.45 else low)
        else:
            c[(x, y)] = seam if unit('tome-sheet', kind, k, y // 3, x // 4) < 0.4 else edge


def tome_cracks(c, inside, seed, crack, lip, plates=14):
    """Cracked hide: plates round hashed seeds, split by `crack` lines with a
    raised `lip` under each, as dried leather splits."""
    xs = [x for x, _ in inside]
    ys = [y for _, y in inside]
    seeds = [(min(xs) + unit(seed, 'x', i) * (max(xs) - min(xs)),
              min(ys) + unit(seed, 'y', i) * (max(ys) - min(ys))) for i in range(plates)]
    cracks = set()
    for (x, y) in sorted(inside):
        d = sorted(math.hypot(x - sx, (y - sy) * 0.75) for sx, sy in seeds)
        if d[1] - d[0] < 0.8:
            cracks.add((x, y))
    for p in sorted(cracks):
        c[p] = crack
        below = (p[0], p[1] + 1)
        if below in inside and below not in cracks:
            c[below] = lip


def tome_corner(c, cx, cy, sx, sy, legs=5):
    """An iron corner cap with its legs `legs` px along the cover's edges
    from the corner (`sx`, `sy` the way in), a rivet in it and a dark line
    where it meets the hide."""
    for i in range(legs + 1):
        for j in range(legs + 1 - i):
            p = (cx + sx * i, cy + sy * j)
            if i + j == legs:
                c[p] = 'θ'
            elif (i == 0 and sx > 0) or (j == 0 and sy > 0):
                c[p] = 'ψ'
            elif i == 0 or j == 0:
                c[p] = 'Ω'
            else:
                c[p] = 'Φ' if i + j < legs - 1 else 'B'
    c[(cx + sx * 1, cy + sy * 1)] = 'χ'
    c[(cx + sx * 2, cy + sy * 2)] = 'θ'


HIDE = ('а', 'ω', 'б', 'п', 'Θ', 'д')


def tome_sigil(c, cx, cy, f):
    """The pact's sigil, burnt into the hide: a ring of ember with two horns
    sweeping up out of it about a slit-pupilled eye, the hide scorched round
    it and cracked with embers, the shimmer running round the ring."""
    for y in range(cy - 7, cy + 8):
        for x in range(cx - 7, cx + 8):
            r = math.hypot(x - cx, y - cy)
            if 4.6 <= r < 6.2 and c.get((x, y)) in HIDE and unit('scorch', x, y) < 0.8:
                c[(x, y)] = 'д' if r < 5.4 else 'б'
    for y in range(cy - 5, cy + 6):
        for x in range(cx - 5, cx + 6):
            r = math.hypot(x - cx, y - cy)
            if 3.5 <= r < 4.6:
                a = (math.atan2(y - cy, x - cx) / math.tau + 0.25) % 1.0
                shine = tome_shimmer(a, f, 0.28)
                c[(x, y)] = 'Y' if shine > 0.55 else ('y' if shine > 0.1 else 'Ъ')
            elif r < 3.5:
                c[(x, y)] = 'д'
    for side in (-1, 1):  # the horns, out of the ring's shoulders
        for i, (dx, dy) in enumerate(((3, -3), (4, -4), (4, -5), (5, -6))):
            c[(cx + side * dx, cy + dy)] = 'y' if i < 2 else 'Ъ'
    # The eye: an almond of ember, its slit pupil hottest.
    for dx, dy in ((-2, 0), (-1, -1), (0, -1), (1, -1), (-1, 1), (0, 1), (1, 1), (2, 0), (-1, 0), (1, 0)):
        c[(cx + dx, cy + dy)] = 'Ъ'
    for dy in (-1, 0, 1):
        c[(cx, cy + dy)] = 'Y' if dy == 0 or f % 2 == 0 else 'y'
    # Embers cracking out of the ring into the hide below the horns.
    for k in range(5):
        a = math.tau * (0.1 + 0.16 * k + 0.05 * unit('sigil-crack', k))
        for t in range(1 + round(unit('sigil-crack-l', k) * 2)):
            p = (round(cx + math.cos(a) * (4.8 + t)), round(cy + math.sin(a) * (4.8 + t)))
            if c.get(p) in HIDE:
                c[p] = 'K' if t == 0 else 'L'


def tome_pact(f, outlined=True):
    """The demonic pact: a heavy grimoire bound in dark, cracked hide with
    iron corners and an ember sigil, its page edges glowing ember."""
    look = TOME_LOOKS['pact']
    x0, y0, x1, y1 = look['front']
    cover, sheets, spine, back = tome_parts('pact')
    c = compose([
        (shade(back, 'б', light='а', shadow='д'), None),
        (shade(spine, 'а', light='ω', shadow='б'), None),
        ({p: 'M' for p in sheets}, None),
        (shade(cover, **look['cover']), 'д'),
    ])
    tome_pages(c, 'pact', f, sheets)
    hide = {p for p in cover if c[p] == 'а'}
    for p in sorted(hide):  # oxblood soaked into the hide
        if unit('pact-stain', p[0] // 3, p[1] // 4) < 0.3 and unit('pact-stain-t', *p) < 0.7:
            c[p] = 'Θ'
    tome_cracks(c, hide, 'pact-hide', 'д', 'п', plates=18)
    # The joint: a groove down the cover beside the spine, the spine's
    # raised bands at its cap.
    for y in range(y0 + 1, y1):
        c[(x0 + 2, y)] = 'д'
        c[(x0 + 1, y)] = 'ω' if y % 5 else 'п'
    for p in ((x0 + 2, y0 - 1), (x0 + 3, y0 - 2)):
        c[p] = 'ω'
    for cx, cy, sx, sy in ((x0, y0, 1, 1), (x1, y0, -1, 1), (x0, y1, 1, -1), (x1, y1, -1, -1)):
        tome_corner(c, cx, cy, sx, sy)
    tome_sigil(c, (x0 + x1) // 2 + 1, (y0 + y1) // 2 + 2, f)
    return outline(c, 'o', TOME_SIZE) if outlined else c


# Bounding's tooled gusts over the clasp: a line of wind ending in a curl,
# from its left end.
GUST = ((0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, -1), (7, -2), (7, -3), (6, -4), (5, -4),
        (4, -3))
# The clasp's wing, pointing to the spine: its leading edge along the top,
# three feathers stepping down to their tips.
WING_CLASP = (
    '....φφχ',
    '..φφχψψ',
    'φφχχψψΩ',
    '.χ.ψ.Ω.',
)


def tome_bounding(f, outlined=True):
    """The tome of bounding: sky-blue tooled leather with a silver wing clasp,
    its top sheets lifting as if in a draught, its pages glowing pale cyan."""
    look = TOME_LOOKS['bounding']
    x0, y0, x1, y1 = look['front']
    depth, rise = look['depth'], look['rise']
    cover, sheets, spine, back = tome_parts('bounding')
    c = compose([
        (shade(back, 'Й', light='З', shadow='И'), None),
        (shade(spine, 'Й', light='З', shadow='И'), None),
        ({p: '`' for p in sheets}, None),
        (shade(cover, **look['cover']), 'И'),
    ])
    tome_pages(c, 'bounding', f, sheets)
    # The tooling: a border pressed in two pixels from the edges and three
    # chevrons rising up the cover; each groove dark with a lit lip under it.
    groove = set()
    for x in range(x0 + 2, x1 - 1):
        groove |= {(x, y0 + 2), (x, y1 - 2)}
    for y in range(y0 + 2, y1 - 1):
        groove |= {(x0 + 3, y), (x1 - 2, y)}
    for p in sorted(groove):
        if p in cover:
            c[p] = 'И'
            below = (p[0], p[1] + 1)
            if below in cover and below not in groove:
                c[below] = 'Ю'
    for gx, gy in ((x0 + 3, y0 + 7), (x0 + 4, y0 + 12)):
        for dx, dy in GUST:
            c[(gx + dx, gy + dy)] = 'Й'
    # The clasp: a strap from the back board over the fore-edge onto the
    # cover, fastened by a silver wing.
    sy = y1 - 5
    for x in range(x1 - 4, x1 + depth + 1):
        up = 0 if x <= x1 else -(-(x - x1) * rise // depth)
        for dy, s in ((-1, 'Й'), (0, 'И'), (1, 'И')):
            c[(x, sy - up + dy)] = s
    wing = {(x1 - 10 + i, sy - 2 + j): s for j, row in enumerate(WING_CLASP)
            for i, s in enumerate(row) if s != '.'}
    for x, y in wing:
        for dx, dy in NEIGHBOURS:
            q = (x + dx, y + dy)
            if q not in wing and q in cover:
                c[q] = 'И'
    c.update(wing)
    # The top sheets lifting off the head by the fore-edge, by frame.
    lift = (0, 1, 2, 1)[f]
    for i, length in enumerate((5, 3)):
        for t in range(length):
            x = x1 + depth - 1 - t - i
            y = y0 - rise - 1 - max(0, lift - i) + (t * (1 + i)) // 3
            if (x, y) not in c or c[(x, y)] in ('Й', 'З', 'И'):
                c[(x, y)] = 'Я' if t < 2 else '`'
    return outline(c, 'o', TOME_SIZE) if outlined else c


def tome_arcane(f, outlined=True):
    """The tome of arcane power: a violet cover with gold-leaf fleurons, a ring
    of gold leaf about a cut gem, its runes glowing violet over white, the
    shimmer passing round them; a ribbon marking its place."""
    look = TOME_LOOKS['arcane']
    x0, y0, x1, y1 = look['front']
    cover, sheets, spine, back = tome_parts('arcane')
    c = compose([
        (shade(back, 'Δ', light='Λ', shadow='Ж'), None),
        (shade(spine, 'Δ', light='Λ', shadow='Ж'), None),
        ({p: 'w' for p in sheets}, None),
        (shade(cover, **look['cover']), 'Ж'),
    ])
    tome_pages(c, 'arcane', f, sheets)
    for p in ((x0 + 2, y0 - 1), (x0 + 4, y0 - 2)):  # gilt bands on the spine's cap
        c[p] = 'A'
    for cx, cy, sx, sy in ((x0 + 1, y0 + 1, 1, 1), (x1 - 1, y0 + 1, -1, 1),
                           (x0 + 1, y1 - 1, 1, -1), (x1 - 1, y1 - 1, -1, -1)):
        for dx, dy, s in ((0, 0, 'J'), (1, 0, 'A'), (2, 0, 'У'), (0, 1, 'A'), (0, 2, 'У'), (1, 1, 'У')):
            c[(cx + sx * dx, cy + sy * dy)] = s
    cx, cy = (x0 + x1) / 2, (y0 + y1) / 2 + 0.5
    # The ring of gold leaf, its runes cut through it glowing, the shimmer
    # moving round them.
    runes = 8
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            r = math.hypot(x - cx, (y - cy) * 0.9)
            if 5.2 <= r < 6.3:
                a = (math.atan2(y - cy, x - cx) / math.tau + 0.25) % 1.0
                slot = a * runes
                if abs(slot - round(slot)) < 0.22:
                    lit = (round(slot) - f * 2) % runes in (0, 1)
                    c[(x, y)] = 'Ы' if lit else 'Э'
                else:
                    c[(x, y)] = 'J' if y < cy - 2 and x < cx else ('A' if y < cy + 2 else 'У')
    # The gem: a cut violet stone, its table bright, its lower facets dark,
    # in a bezel of gold.
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            u, v = (x - cx) / 2.6, (y - cy) / 3.4
            d = abs(u) + abs(v)
            if d <= 1.0:
                if u + v < -0.35:
                    c[(x, y)] = 'Ы'
                elif u + v > 0.55:
                    c[(x, y)] = 'Λ'
                else:
                    c[(x, y)] = 'Ы' if (x + y + f) % 4 == 0 and v < 0.2 else 'Э'
            elif d <= 1.45 and c.get((x, y)) in ('Δ', 'Λ', 'Ж'):
                c[(x, y)] = 'У' if u + v > 0 else 'A'
    # A ribbon hanging from the sheets below the fore-edge, swaying.
    sway = (0, 1, 1, 0)[f]
    rx = x1 + 2
    for t in range(6):
        p = (rx + (t * sway) // 4, y1 - 3 + t)
        if p not in cover:
            c[p] = 'Ш' if t < 2 else 'Ц'
    end = (rx + sway, y1 + 3)
    c[(end[0] - 1, end[1])] = 'Ц'
    c[(end[0] + 1, end[1])] = 'Ц'
    return outline(c, 'o', TOME_SIZE) if outlined else c


TOME_DRAW = {'pact': tome_pact, 'bounding': tome_bounding, 'arcane': tome_arcane}

# The take (unlit, 64 px, its centre on the book's): the book flares in its
# colour, breaks into shards of light that fly out and dissolve, and a ring
# of light opens behind them; its sparks follow it out (`take_sparks`).
# Each kind's light, dark to white, and the haze it flares in, faint to
# strong (`None`: the palette's white steps).
TAKE_RAMPS = {
    'pact': ('M', 'L', 'K', 'Ъ', 'y', 'Y', 'Z', 'Ψ'),
    'bounding': ('И', 'Й', 'З', 'Ч', 'Ю', '`', 'Я', 'Ψ'),
    'arcane': ('Ж', 'Δ', 'Λ', 'Э', 'Σ', 'ϕ', 'Ы', 'Ψ'),
}
TAKE_HAZE = {'pact': 'эюяБГД', 'bounding': None, 'arcane': None}
TAKE_SIZE = ELEMENT_SIZES['tome_pact_take_0']
TAKE_FRAMES = 6


def haze(kind, x, y, alpha):
    """A translucent pixel of `kind`'s flare at `alpha` (0..255), dithered
    between its steps, or None."""
    steps = TAKE_HAZE[kind]
    if steps is None:
        return white_alpha(x, y, alpha)
    levels = [(PALETTE[s][3], s) for s in steps]
    low_alpha, low = 0, None
    for high_alpha, high in levels:
        if alpha <= high_alpha:
            return high if dither(x, y, (alpha - low_alpha) / (high_alpha - low_alpha)) else low
        low_alpha, low = high_alpha, high
    return levels[-1][1]


def lum(symbol):
    r, g, b, _ = PALETTE[symbol]
    return (0.3 * r + 0.59 * g + 0.11 * b) / 255


def tome_take(kind, f):
    w, h = TAKE_SIZE
    ramp = TAKE_RAMPS[kind]
    cx, cy = w / 2, h / 2
    px = {}
    # The flare's haze, swelling then fading.
    radius, peak = (12, 18, 22, 22, 20, 16)[f], (60, 110, 150, 70, 30, 10)[f]
    for y in range(h):
        for x in range(w):
            r = math.hypot(x + 0.5 - cx, y + 0.5 - cy) / radius
            if r < 1:
                s = haze(kind, x, y, peak * (1 - r) ** 1.6)
                if s:
                    px[(x, y)] = s
    # Rays out of the book while it flares, longest as it breaks.
    length = (7, 14, 22, 0, 0, 0)[f]
    for i in range(8):
        a = (i + 0.35 * unit('take-ray', kind, i)) * math.tau / 8
        reach = length * (0.6 + 0.4 * unit('take-ray-l', kind, i))
        for t in range(int(reach)):
            r = 6 + t
            p = (round(cx - 0.5 + math.cos(a) * r), round(cy - 0.5 + math.sin(a) * r))
            fade = 1 - t / max(reach, 1)
            if fade > 0.55:
                px[p] = ramp[4 + round(fade * 2)]
            elif (s := haze(kind, *p, 200 * fade)):
                px[p] = s
    # The book: washed toward its light, then solid light breaking into
    # shards that fly out and dissolve.
    book = TOME_DRAW[kind](0)
    levels = sorted({lum(s) for s in book.values()})
    low, high = levels[0], levels[-1]
    seeds = [(8 + unit('shard-x', kind, i) * 16, 6 + unit('shard-y', kind, i) * 22) for i in range(9)]
    travel = (0, 0, 2, 6, 11, 16)[f]
    erode = (0, 0, 0, 1.0, 2.0, 2.8)[f]
    for (bx, by), s in sorted(book.items()):
        p = (bx + 16, by + 16)
        v = (lum(s) - low) / max(high - low, 1e-6)
        if f < 2:
            # Washed toward its light, its detail kept: the flare.
            px[p] = ramp[min(7, 1 + 2 * f + round(v * (6 - 2 * f)))]
            continue
        d = sorted((math.hypot(bx - sx, by - sy), i) for i, (sx, sy) in enumerate(seeds))
        if d[1][0] - d[0][0] < erode:
            continue
        sx, sy = seeds[d[0][1]]
        n = math.hypot(sx - 16, sy - 16) or 1
        q = (round(p[0] + (sx - 16) / n * travel), round(p[1] + (sy - 16) / n * travel))
        edge = d[1][0] - d[0][0] - erode
        px[q] = ramp[max(3, min(7, 8 - f + (edge > 1.5) - (edge < 0.7)))]
    # A ring of light opening behind the shards.
    if f >= 2:
        ring_r = (0, 0, 9, 15, 21, 26)[f]
        for y in range(h):
            for x in range(w):
                d = abs(math.hypot(x + 0.5 - cx, y + 0.5 - cy) - ring_r)
                if d < 1.1 and (f < 5 or dither(x, y, 0.5)) and \
                        ((x, y) not in px or PALETTE[px[(x, y)]][3] < 255):
                    px[(x, y)] = ramp[(7, 6, 5, 4)[f - 2]]
    # Twinkles left where the shards went out.
    if f >= 3:
        for k in range(5):
            a = unit('twinkle', kind, k) * math.tau
            r = 12 + 14 * unit('twinkle-r', kind, k) + 3 * (f - 3)
            x, y = round(cx + math.cos(a) * r), round(cy + math.sin(a) * r)
            if (k + f) % 2:
                px[(x, y)] = ramp[7]
                for dx, dy in NEIGHBOURS:
                    px[(x + dx, y + dy)] = ramp[5]
    return to_image(px, (w, h))


# The bound (unlit, 64 x 32, its centre on the feet): a ring of sky-blue
# light flattened under the wizard's feet, flashing and opening outward,
# a gust of light rising from it; apart from the double jump's warm sparks.
BOUND_SIZE = ELEMENT_SIZES['bound_ring_0']
BOUND_FRAMES = 6


def bound_ring(f):
    w, h = BOUND_SIZE
    cx, cy = (w - 1) / 2, h / 2
    rx = (5, 10, 15, 20, 24, 27)[f]
    ry = rx * 0.28 + 0.6
    band = (2.2, 1.9, 1.6, 1.3, 1.1, 0.9)[f]
    px = {}
    for y in range(h):
        for x in range(w):
            dx, dy = (x - cx) / rx, (y + 0.5 - cy) / ry
            e = math.hypot(dx, dy)
            off = abs(e - 1) * rx
            front = y + 0.5 >= cy
            if off < band:
                core = off < band * 0.45
                if f >= 4 and not dither(x, y, (0.75, 0.45)[f - 4]):
                    continue
                if front:
                    px[(x, y)] = ('Ψ' if f < 2 else 'Я') if core else 'Ч'
                else:
                    px[(x, y)] = 'Я' if core and f < 3 else ('Ч' if core else ',')
            elif e < 1 and f < 4:
                # The light inside the ring, strongest by its rim.
                s = white_alpha(x, y, (110, 80, 50, 24)[f] * max(0.0, e - 0.25) / 0.75)
                if s:
                    px[(x, y)] = s
    if f == 0:  # the flash under the feet
        for x in range(round(cx) - 3, round(cx) + 4):
            for y in (round(cy) - 1, round(cy)):
                px[(x, y)] = 'Ψ' if abs(x - cx) < 2 else 'Я'
    if f < 4:  # the gust rising off it: three streaks, heads climbing
        for k, (gx, reach) in enumerate(((-9, 1.0), (-2, 0.8), (6, 0.9))):
            head = round(cy - 3 - (5, 10, 14, 17)[f] * reach)
            length = round((4, 7, 8, 6)[f] * reach)
            x = round(cx + gx + (f > 1) * (1 if gx > 0 else -1))
            for i in range(1, length + 1):
                s = white_alpha(x, head + i, 220 * (1 - i / (length + 1)) * (1 - 0.2 * f))
                if s:
                    px[(x, head + i)] = s
            px[(x, head)] = 'Ψ' if f < 2 else 'Я'
    for k in range(6):  # sparks set along the ring
        if not 1 <= f <= 4:
            break
        a = (k + unit('bound-spark', k)) * math.tau / 6
        x, y = round(cx + math.cos(a) * rx), round(cy + math.sin(a) * ry)
        if 0 <= x < w and 0 <= y < h:
            px[(x, y)] = 'Ψ'
    return to_image(px, (w, h))


# The clips: each book's idle loop, its take, and the bound's ring; a clip's
# frame size, its frames, the milliseconds a frame and whether it loops.
TOME_CLIPS = {
    **{f'tome_{kind}_idle': (TOME_SIZE, TOME_FRAMES, 180, True) for kind in TOME_KINDS},
    **{f'tome_{kind}_take': (TAKE_SIZE, TAKE_FRAMES, 80, False) for kind in TOME_KINDS},
    'bound_ring': (BOUND_SIZE, BOUND_FRAMES, 50, False),
}


def build_tome_art(images, lit):
    """Every book's idle frames, lit, its take's frames and the bound's
    ring's, unlit: `tome_<kind>_idle_<f>`, `tome_<kind>_take_<f>` and
    `bound_ring_<f>`."""
    for kind, draw in TOME_DRAW.items():
        for f in range(TOME_FRAMES):
            name = f'tome_{kind}_idle_{f}'
            images[name] = to_image(draw(f), TOME_SIZE)
            lit.add(name)
        for f in range(TAKE_FRAMES):
            images[f'tome_{kind}_take_{f}'] = tome_take(kind, f)
    for f in range(BOUND_FRAMES):
        images[f'bound_ring_{f}'] = bound_ring(f)


# e01foes' effects: the crawler's snowball turning and its burst, and the
# beetle's charge streaks drawn behind its rear, which `generate.mirror_facing`
# twins for a beetle facing left (`FACING_CLIPS`). A clip: its frame size,
# its frames, the milliseconds a frame and whether it loops.
FOE_CLIPS = {
    'snowball': ((16, 16), 4, 60, True),
    'snowball_burst': ((48, 48), 6, 50, False),
    'charge_streak': ((64, 24), 4, 50, True),
}
FACING_CLIPS = ('charge_streak',)
# The snow's white-blue ramp, shadow to light (the crawler's clump's too).
SNOW_RAMP = ('Ѣ', 'ѣ', 'Ѳ', 'Ѵ', 'Ψ')
# The powder's whites by alpha, faint to opaque.
POWDER = ('#', '%', '&', 'l', 'p', 'q', 'u', 'x', 'Ѵ', 'Ψ')


def snowball(f):
    """The snowball, a quarter turn of its spin a frame: a lumpy sphere of
    packed snow, white-blue, lit from the top-left, ice grains glinting and
    pits of shadow turning with it, a pale blue rim; its centre (8, 8),
    within 6 pixels of it."""
    size = FOE_CLIPS['snowball'][0]
    spin = f * math.pi / 4  # its lumps and grains repeat each half turn
    px = {}
    for y in range(size[1]):
        for x in range(size[0]):
            dx, dy = x + 0.5 - 8, y + 0.5 - 8
            r, theta = math.hypot(dx, dy), math.atan2(dy, dx)
            edge = 4.3 + 0.45 * math.sin(2 * (theta - spin) + 0.7) + 0.25 * math.sin(4 * (theta - spin) + 1.9)
            if r > edge:
                continue
            nz = math.sqrt(max(0.0, 1 - (r / edge) ** 2))
            lit = (-0.55 * dx / edge - 0.65 * dy / edge + 0.55 * nz) * 0.75 + 0.3
            px[(x, y)] = SNOW_RAMP[min(len(SNOW_RAMP) - 1, max(0, int(lit * len(SNOW_RAMP) + ordered(x, y) * 0.5)))]
    for k in range(6):  # grains and pits on its surface, turning with it
        a = unit('snowball-grain', k) * math.pi + spin + (math.pi if k % 2 else 0)
        rr = 1.2 + 2.6 * unit('snowball-grain-r', k)
        q = (round(8 + math.cos(a) * rr - 0.5), round(8 + math.sin(a) * rr - 0.5))
        if q in px:
            px[q] = 'Ψ' if k % 3 == 0 else ('ѣ' if k % 3 == 1 else 'Ѳ')
    for q in [(x + dx, y + dy) for x, y in px for dx, dy in NEIGHBOURS]:
        if q not in px and 0 <= q[0] < size[0] and 0 <= q[1] < size[1]:
            px[q] = 'ϰ' if q[1] < 8 else 'Ѣ'
    px[(6, 5)] = 'Ψ'
    return to_image(px, size)


def snowball_burst(f):
    """The snowball bursting where it met something: a bright puff of
    powder blooming and settling, chips of packed snow flying out and
    falling, a few flakes riding the puff; white and frosty, translucent at
    its edge, with no dark rim. Its centre (24, 24) on the contact."""
    size = FOE_CLIPS['snowball_burst'][0]
    n = FOE_CLIPS['snowball_burst'][1]
    t = f / (n - 1)
    cx, cy = 24.0, 24.0 + 5.0 * t * t  # the powder sinking as it settles
    r0 = 4.5 + 13.0 * t ** 0.55
    px = {}
    for y in range(size[1]):
        for x in range(size[0]):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            theta = math.atan2(dy, dx)
            edge = r0 * (1 + wobble('snowburst', theta, f, ((5, 0.12, 1.3), (8, 0.08, -2.1), (13, 0.05, 2.7))))
            # Squashed as it settles: flatter and wider at its foot.
            d = math.hypot(dx / (1 + 0.25 * t), dy / (1 - 0.3 * t * (dy > 0) - 0.1 * t))
            q = d / max(edge, 0.5)
            if q > 1:
                continue
            v = (1 - q) * (1.25 - 0.9 * t) + ordered(x, y) * 0.12
            if t > 0.35 and unit('snowburst-thin', x, y, f) < (t - 0.35) * 1.3 - (1 - q) * 0.4:
                continue
            level = min(len(POWDER) - 1, int(v * len(POWDER) * 1.05))
            if level >= 1:
                px[(x, y)] = POWDER[level]
    for k in range(9):  # chips of packed snow flung out, falling as they fly
        a = (k / 9) * math.tau + unit('snowchip', k) * 0.6
        speed = 14 + 12 * unit('snowchip-v', k)
        tt = t + 0.12
        x = cx + math.cos(a) * speed * tt * 1.6
        y = 24 + math.sin(a) * speed * tt * 1.3 + 30 * tt * tt
        q = (round(x), round(y))
        if 1 <= q[0] < size[0] - 2 and 1 <= q[1] < size[1] - 2 and t < 0.92:
            px[q] = 'Ψ' if t < 0.5 else 'Ѵ'
            if k % 3 == 0 and t < 0.7:
                px[(q[0] + 1, q[1])] = 'Ѳ'
                px[(q[0], q[1] + 1)] = 'ѣ'
    for k in range(5):  # flakes riding the puff out
        a = unit('snowflake', k) * math.tau + t * 0.9
        rr = r0 * (0.5 + 0.6 * unit('snowflake-r', k))
        q = (round(cx + math.cos(a) * rr), round(cy + math.sin(a) * rr * 0.85 - 2 * t))
        if 3 <= q[0] < size[0] - 3 and 3 <= q[1] < size[1] - 3 and 0.15 < t < 0.95:
            flake(px, q[0], q[1], big=k % 2 == 0 and t < 0.7, bright=t < 0.55 or k % 2 == 0)
    return to_image(px, size)


def charge_streak(f):
    """A charge's streaks, flickering a frame at a time: bold speed lines
    tapering away behind the beetle, white-grey cores over darker iron
    lines, broken into dashes toward their tails; its right edge's middle
    (64, 12) on the beetle's rear."""
    w, h = FOE_CLIPS['charge_streak'][0]
    px = {}
    # Each line: its row from the middle, its length, its weight and its tone.
    lines = ((-8, 34, 1, 'dark'), (-5, 52, 2, 'light'), (-1, 60, 2, 'dark'), (2, 46, 2, 'light'),
             (6, 56, 1, 'dark'), (9, 30, 1, 'light'))
    for k, (row, length, weight, tone) in enumerate(lines):
        length *= 0.75 + 0.35 * unit('streak', k, f)
        jitter = round(2 * unit('streak-j', k, f)) - 1 if k % 2 else 0
        y0 = h // 2 + row + jitter
        for x in range(w - 1, max(-1, round(w - length)), -1):
            u = (w - 1 - x) / length  # 0 at the beetle, 1 at the tail
            if u > 0.45 and unit('streak-dash', k, f, x // 3) < (u - 0.45) * 1.6:
                continue  # broken into dashes toward its tail
            thick = weight if u < 0.55 else 1
            for dy in range(thick):
                if tone == 'light':
                    s = 'Ψ' if u < 0.12 else 'φ' if u < 0.3 else 'x' if u < 0.55 else 'u' if u < 0.8 else 'p'
                else:
                    s = 'ψ' if u < 0.18 else 'Ω' if u < 0.4 else 'ѐ' if u < 0.7 else 'ё'
                if 0 <= y0 + dy < h:
                    px[(x, y0 + dy)] = s
    return to_image(px, (w, h))


def build_foe_art(images):
    """The foes' effects' frames, unlit: `<clip>_<f>`."""
    for name, draw in (('snowball', snowball), ('snowball_burst', snowball_burst), ('charge_streak', charge_streak)):
        for f in range(FOE_CLIPS[name][1]):
            images[f'{name}_{f}'] = draw(f)


def build_sfx_art(images, lit):
    """e01sfx's effects: the ground strike's scorch, lit as the crates are,
    and the hurt edge over the view, unlit. Today's art stands in at each
    one's size until its asset session draws them: a burning crate's last
    char stage and the shock ring, resized. After `polish_art.enrich_art`,
    which draws the ring."""
    from PIL import Image
    for name, source in (('scorch', 'pine_crate_char_2'), ('hurt_edge', 'shock_ring')):
        images[name] = images[source].resize(ELEMENT_SIZES[name], Image.Resampling.NEAREST)
    lit.add('scorch')


def build_effects(images):
    images['halo'] = radial_glow(HALO_STEPS, 1.8)
    images['flame_glow'] = radial_glow(FLAME_STEPS, 2.2)
    images['dust'] = puff()
    images['droplet'] = droplet()
    images['mote'] = firefly()
    spark_and_cursor(images)
    build_black_hole(images)
    build_icons(images)
    build_element_art(images)
    build_hud_art(images)
    build_frost_art(images)
    build_spray_art(images)
    build_push_art(images)
    build_foe_art(images)
