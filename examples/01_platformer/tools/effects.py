"""Particle, glow and black-hole sprites.

Particle sprites are neutral greys or whites so each config's colour curve tints
them. The glow gradients are dithered between the palette's alpha steps and are
sampled with linear filtering, so they stay smooth when particles or halos
scale them up. The black-hole layers are white alpha fields on 96 px canvases,
tinted at extraction; each fades to transparent before its canvas edge.
"""
import math

from art import EFFECT_SIZES, PALETTE, canvas
from pixels import dither, smoothstep, to_image

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


def build_effects(images):
    images['halo'] = radial_glow(HALO_STEPS, 1.8)
    images['flame_glow'] = radial_glow(FLAME_STEPS, 2.2)
    images['dust'] = puff()
    images['droplet'] = droplet()
    images['mote'] = firefly()
    spark_and_cursor(images)
    build_black_hole(images)
