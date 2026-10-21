"""Particle, glow and black-hole sprites.

Particle sprites are neutral greys or whites so each config's colour curve tints
them. The glow gradients are dithered between the palette's alpha steps and are
sampled with linear filtering, so they stay smooth when particles or halos
scale them up. The black-hole layers are white alpha fields on 96 px canvases,
tinted at extraction; each fades to transparent before its canvas edge.
"""
import math

from art import EFFECT_SIZES, ELEMENT_SIZES, ICON_SIZES, PALETTE, STATUS_CLASSES, canvas
from pixels import NEIGHBOURS, Part, compose, dither, outline, shade, smoothstep, to_image, unit

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


def status_ice(size):
    """Frozen: a translucent block of ice over the frame, which shows through
    it under the game's cold tint: bright edges, glints and two cracks."""
    x0, y0, x1, y1 = status_box(size)
    pixels = {(x, y): 'ϱ' if x in (x0, x1) or y in (y0, y1) else 'ϰ'
              for y in range(y0, y1 + 1) for x in range(x0, x1 + 1)}
    span = max(4, (x1 - x0) // 3)
    for i in range(3):  # diagonal glints near the top
        gx, gy = x0 + 2 + i * span // 2, y0 + 2
        for t in range(max(2, span // 2)):
            if (gx + t, gy + t) in pixels and gx + t < x1 - 1:
                pixels[(gx + t, gy + t)] = 'Ψ' if t < 2 else 'ϱ'
    for i in range(2):  # cracks rising from the foot
        cx = x0 + int(unit('crack', size, i) * (x1 - x0))
        end = (cx + (6 if i else -5) * size // 64, y1 - (y1 - y0) // 2)
        crack = Part((size, size))
        crack.limb(zigzag((cx, y1 - 1), end, 4, ('crack', size, i), 2), 1)
        for p in crack.pixels():
            if p in pixels and pixels[p] == 'ϰ':
                pixels[p] = 'ς'
    return to_image(pixels, (size, size))


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
    """A frozen ball's shell, drawn over its sprite under a cold tint: a
    translucent disc with a bright rim and a glint."""
    pixels = {}
    c, r = 31.5, 30.0
    for y in range(64):
        for x in range(64):
            d = math.hypot(x - c, y - c)
            if d <= r:
                pixels[(x, y)] = 'ϱ' if d > r - 1.5 else 'ϰ'
    for t in range(10):
        pixels[(round(c - r * 0.45) + t, round(c - r * 0.55) + t // 2)] = 'Ψ'
    return to_image(pixels)


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
    overlays in their three size classes, the ice shell, the muzzle flash and
    the tracer. All unlit: overlays and flashes draw over the lit world."""
    for i in range(len(BOLT_EXITS)):
        images[f'bolt_{i}'] = bolt_segment(i)
    for name, size in STATUS_CLASSES.items():
        images[f'status_ice_{name}'] = status_ice(size)
        images[f'status_halo_{name}'] = status_halo(size)
        for frame in range(2):
            images[f'status_crackle_{name}_{frame}'] = status_crackle(size, frame)
    images['ice_shell'] = ice_shell()
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
