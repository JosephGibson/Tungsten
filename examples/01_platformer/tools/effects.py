"""Particle, glow and black-hole sprites.

Particle sprites are neutral greys or whites so each config's colour curve tints
them. The glow gradients are dithered between the palette's alpha steps and are
sampled with linear filtering, so they stay smooth when particles or halos
scale them up. The black-hole layers are white alpha fields on 96 px canvases,
tinted at extraction; each fades to transparent before its canvas edge.
"""
import math

from art import EFFECT_SIZES, ICON_SIZES, PALETTE, canvas
from pixels import Part, dither, outline, shade, smoothstep, to_image

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


ICON_ART = {
    'icon_fire_missile': icon_fire_missile,
    'icon_flamethrower': icon_flamethrower,
    'icon_ice_spray': icon_ice_spray,
    'icon_force_push': icon_force_push,
    'icon_black_hole': icon_black_hole,
    'icon_iron_block': icon_iron_block,
    'icon_bowling_ball': icon_bowling_ball,
    'icon_small_balls': icon_small_balls,
}


def build_icons(images):
    if set(ICON_ART) != set(ICON_SIZES):
        raise ValueError('every spell icon needs its art')
    for name, draw in ICON_ART.items():
        images[name] = icon_image(draw())


def build_effects(images):
    images['halo'] = radial_glow(HALO_STEPS, 1.8)
    images['flame_glow'] = radial_glow(FLAME_STEPS, 2.2)
    images['dust'] = puff()
    images['droplet'] = droplet()
    images['mote'] = firefly()
    spark_and_cursor(images)
    build_black_hole(images)
    build_icons(images)
