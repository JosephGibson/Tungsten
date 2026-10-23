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


# The spell icons: 32 px palette art in the courier's style, outlined once,
# shown by the HUD at whole-number scale with their element's border.
ICON = 32


def icon_image(pixels):
    size = (ICON, ICON)
    return to_image(outline(pixels, 'o', size), size)


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
    tail = Part((ICON, ICON))
    tail.polygon([(18, 9), (23, 14), (6, 28), (4, 26)])
    pixels = shade(tail.pixels(), 'L', light='K', shadow='M')
    for x, y in list(pixels):  # streaks along the trail
        if (x + 2 * y) % 5 == 0:
            pixels[(x, y)] = 'K'
    pixels.update(icon_disc((21, 11), 7, 'LKJY'))
    pixels.update({(20, 9): 'Z', (19, 9): 'Z', (20, 8): 'Z'})
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
    """A short pistol side on: an iron slide and barrel, a wooden grip."""
    slide = Part((ICON, ICON))
    slide.polygon([(5, 10), (27, 10), (27, 16), (5, 16)])
    slide.rectangle((27, 11, 29, 14))
    grip = Part((ICON, ICON))
    grip.polygon([(7, 16), (14, 16), (12, 27), (5, 27)])
    guard = Part((ICON, ICON))
    guard.limb([(14, 16), (15, 20), (19, 20), (19, 16)], 1)
    pixels = compose([
        (shade(grip.pixels(), "'", light='a', shadow='('), 'o'),
        (shade(guard.pixels(), 'Φ'), None),
        (shade(slide.pixels(), 'Ω', light='χ', shadow='Φ'), 'o'),
    ])
    for x in range(8, 25, 3):  # the slide's grip ridges
        pixels[(x, 13)] = 'Φ'
    pixels[(26, 10)] = 'φ'  # the front sight catching the moon
    return pixels


def icon_dummy():
    """The training dummy, the summon slot's first type, at half size on an
    indigo summoning ring."""
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
    `lift` pixels, standing on an indigo summoning ring."""
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
}


def build_icons(images):
    if set(ICON_ART) != set(ICON_SIZES):
        raise ValueError('every spell icon needs its art')
    for name, draw in ICON_ART.items():
        images[name] = icon_image(draw())


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


def build_element_art(images):
    """The elements' and the gun's art: the bolt's segments, the status
    overlays in their three size classes, the fitted ice cubes and their
    cracked twins, which a thaw flickers to, the ball's cube, the glint,
    the muzzle flash, the tracer and the drops' orbs. All unlit: overlays
    and flashes draw over the lit world."""
    for i in range(len(BOLT_EXITS)):
        images[f'bolt_{i}'] = bolt_segment(i)
    for name, size in STATUS_CLASSES.items():
        images[f'status_ice_{name}'] = status_ice(size)
        images[f'status_ice_{name}_cracked'] = status_ice(size, cracked=True)
        images[f'status_halo_{name}'] = status_halo(size)
        for frame in range(2):
            images[f'status_crackle_{name}_{frame}'] = status_crackle(size, frame)
    for kind in HURTBOXES:
        for suffix, cracked in (('', False), ('_cracked', True)):
            images[f'status_ice_{kind}{suffix}'] = ice_cube(
                ice_frame(kind), fitted_box(kind), (kind,), cracked)
    images['ice_shell'] = ice_shell()
    images['ice_glint'] = ice_glint()
    images['health_orb'] = orb('z', 'F', 'M', 'Ψ', 'X')
    images['mana_orb'] = orb('Π', 'Ξ', 'Γ', 'ϕ', 'Δ')
    for frame in range(3):
        images[f'muzzle_{frame}'] = muzzle(frame)
    images['tracer'] = tracer()


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
