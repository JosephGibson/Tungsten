"""The boot splash's logo and bar, drawn from shapes and glows and written as RGBA PNG.

`boot_logo.png` is 768 × 256 px, its art inside a 24 px margin and clear
around it: Tungsten's mark, a filament coiled into a W, the element's
symbol, glowing in the shell theme's accent with its hottest wire in the
theme's focus colour, beside "TUNGSTEN" in geometric capitals in its text
colour, under "MADE WITH" in its muted one. The boot view draws the logo at
half its size × the boot's scale and centres it, so it has no anchor and
no code depends on the drawing. `boot_bar.png` is 16 × 16 px and opaque,
the accent with a faint glow along its middle; the view stretches it to
the load's progress.

Each layer is a shape or a glow, painted in order over clear pixels. A
shape is a union of parts, each a distance from its edge that changes no
faster than the point moves: a part whose edge lies further than `REACH`
from a pixel's middle crosses none of the pixel's samples, so only a pixel
an edge passes near is sampled 4 × 4 times, as `sprites.py` samples its
shapes, and the rest are wholly in or out. A glow is read at the pixel's
middle. The coil's points come from `math.sin` and `math.cos`, as the
enemy's spikes in `sprites.py` do; arithmetic, comparisons and square roots
decide each pixel, so a render repeats pixel for pixel, and `--check`
compares decoded pixels, as `sprites.py`'s does. A clear pixel takes its
painted neighbours' colour, so the linear filter blends no black into the
art's edges.

    python3 -B tools/logo.py            # write assets/sprites/boot_logo.png and boot_bar.png
    python3 -B tools/logo.py --out DIR  # write DIR instead
    python3 -B tools/logo.py --check    # compare them pixel for pixel
"""
import argparse
import math
import pathlib
import struct
import sys
import zlib

sys.dont_write_bytecode = True
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import sprites  # noqa: E402

# The logo's size and the margin its art keeps clear, and the bar's size,
# in pixels: what the boot view's code depends on.
LOGO = (768, 256)
MARGIN = 24
BAR = (16, 16)
# The shell theme's colours (`assets/ui/themes/shell.json`).
ACCENT = (0xF0, 0xA8, 0x30)
FOCUS = (0xFF, 0xD3, 0x6B)
TEXT = (0xE9, 0xEB, 0xF1)
MUTED = (0xA3, 0xA9, 0xB6)

# The coil, in pixels: a turn each `PITCH` along the W, `RADIUS` about it,
# the wire `WIRE` either side of its line; the near half of each turn
# drawn `LEAN` × the radius lower, as seen from above; the wire straight
# for `BARE` at each corner and end, its turns opening over the next
# `TAPER`. The far halves are `FAR` darker and `BACK` as thick; the near
# halves `HOT` of the way to the focus colour, their cores `CORE` × the
# wire's width in it.
PITCH, RADIUS, WIRE, LEAN, BARE, TAPER = 13.5, 8.8, 2.65, 0.6, 6.0, 30.0
FAR, BACK, HOT, CORE = 0.06, 0.85, 0.3, 0.55
# The accent's glows about the W: (reach in px, peak alpha, power), a wide
# haze under the coil and a tighter glow over its far halves.
HAZE = (38.0, 0.18, 4)
GLOW = (15.0, 0.36, 2)

# Each pixel's 16 sample points, from its middle; the farthest lies
# 0.53 px away, so a part's edge further than `REACH` crosses none.
SAMPLES = [((sx + 0.5) / sprites.GRID - 0.5, (sy + 0.5) / sprites.GRID - 0.5)
           for sy in range(sprites.GRID) for sx in range(sprites.GRID)]
REACH = 0.54
# The side of the square cells that list the parts near them.
CELL = 8


def mix(a, b, t):
    """The colour `t` of the way from `a` to `b`."""
    return tuple(x + (y - x) * t for x, y in zip(a, b))


# Parts: each a (distance, bounds) pair, the distance from its edge,
# negative inside.

def capsule(ax, ay, bx, by, r):
    """The points within `r` of the segment from (ax, ay) to (bx, by)."""
    dx, dy = bx - ax, by - ay
    length = dx * dx + dy * dy
    inverse = 1.0 / length if length else 0.0

    def distance(x, y):
        px, py = x - ax, y - ay
        t = (px * dx + py * dy) * inverse
        t = 0.0 if t < 0.0 else 1.0 if t > 1.0 else t
        ex, ey = px - t * dx, py - t * dy
        return math.sqrt(ex * ex + ey * ey) - r
    return distance, (min(ax, bx) - r, min(ay, by) - r, max(ax, bx) + r, max(ay, by) + r)


def disc(cx, cy, r):
    """A disc about (cx, cy)."""
    return capsule(cx, cy, cx, cy, r)


def box(x0, y0, x1, y1):
    """An axis-aligned rectangle."""
    return (lambda x, y: max(x0 - x, x - x1, y0 - y, y - y1)), (x0, y0, x1, y1)


def convex(*corners):
    """A convex polygon by its corners, in either order."""
    mx = sum(x for x, _ in corners) / len(corners)
    my = sum(y for _, y in corners) / len(corners)
    planes = []
    for (ax, ay), (bx, by) in zip(corners, corners[1:] + corners[:1]):
        nx, ny = by - ay, ax - bx
        norm = math.sqrt(nx * nx + ny * ny)
        nx, ny = nx / norm, ny / norm
        c = nx * ax + ny * ay
        if nx * mx + ny * my > c:
            nx, ny, c = -nx, -ny, -c
        planes.append((nx, ny, c))

    def distance(x, y):
        return max(nx * x + ny * y - c for nx, ny, c in planes)
    xs, ys = [x for x, _ in corners], [y for _, y in corners]
    return distance, (min(xs), min(ys), max(xs), max(ys))


def arc(cx, cy, radius, half, start, end):
    """The stroke `half` px either side of the circle of `radius` about
    (cx, cy), from `start` to `end` degrees clockwise on screen from +x,
    its ends cut square along the radii."""
    span = (end - start) % 360 or 360
    a, b = math.radians(start), math.radians(start + span)
    ux, uy, wx, wy = math.cos(a), math.sin(a), math.cos(b), math.sin(b)
    join = max if span <= 180 else min

    def distance(x, y):
        px, py = x - cx, y - cy
        band = abs(math.sqrt(px * px + py * py) - radius) - half
        if span == 360:
            return band
        return max(band, join(uy * px - ux * py, wx * py - wy * px))
    reach = radius + half
    return distance, (cx - reach, cy - reach, cx + reach, cy + reach)


def clip(part, limit):
    """`part` where it lies inside `limit`, another part."""
    (f, bounds), (g, _) = part, limit
    return (lambda x, y: max(f(x, y), g(x, y))), bounds


# Layers: ('shape', colour, alpha, parts), or ('glow', colour, alpha,
# (field, bounds)), the field 0 to 1 at a point.

def shape(colour, parts, alpha=1.0):
    return 'shape', colour, alpha, parts


def glow(colour, alpha, field, bounds):
    return 'glow', colour, alpha, (field, bounds)


def paint(width, height, layers, margin=0):
    """`layers` painted in order over clear pixels inside `margin`; returns
    the RGBA rows."""
    columns = (width + CELL - 1) // CELL
    cells = [[] for _ in range(columns * ((height + CELL - 1) // CELL))]
    for index, (kind, colour, alpha, parts) in enumerate(layers):
        for item in [parts] if kind == 'glow' else parts:
            x0, y0, x1, y1 = item[1]
            for cy in range(max(0, int(y0 - 1) // CELL), min(height - 1, int(y1 + 1)) // CELL + 1):
                for cx in range(max(0, int(x0 - 1) // CELL), min(width - 1, int(x1 + 1)) // CELL + 1):
                    cell = cells[cy * columns + cx]
                    if cell and cell[-1][0] == index:
                        cell[-1][6].append(item[0])
                    else:
                        cell.append((index, kind == 'glow', *colour, alpha, [item[0]]))
    rows = []
    for py in range(height):
        row = bytearray(width * 4)
        rows.append(row)
        if not margin <= py < height - margin:
            continue
        y = py + 0.5
        base = (py // CELL) * columns
        for px in range(margin, width - margin):
            cell = cells[base + px // CELL]
            if not cell:
                continue
            x = px + 0.5
            # Each layer over this pixel: (colour, alpha, the parts its
            # samples must test, or None when it covers them all).
            plan, partial = [], False
            for _, is_glow, r, g, b, alpha, items in cell:
                if is_glow:
                    a = alpha * items[0](x, y)
                    if a > 0.0:
                        plan.append((r, g, b, a, None))
                    continue
                near = None
                for f in items:
                    d = f(x, y)
                    if d <= REACH:
                        if d < -REACH:
                            near = ()
                            break
                        if near is None:
                            near = [f]
                        else:
                            near.append(f)
                if near is not None:
                    plan.append((r, g, b, alpha, near or None))
                    partial = partial or bool(near)
            if not plan:
                continue
            if partial:
                sr = sg = sb = sa = 0.0
                for ox, oy in SAMPLES:
                    sx, sy = x + ox, y + oy
                    cr = cg = cb = ca = 0.0
                    for r, g, b, a, near in plan:
                        if near is not None:
                            for f in near:
                                if f(sx, sy) <= 0.0:
                                    break
                            else:
                                continue
                        keep = 1.0 - a
                        cr, cg, cb, ca = r * a + cr * keep, g * a + cg * keep, b * a + cb * keep, a + ca * keep
                    sr, sg, sb, sa = sr + cr, sg + cg, sb + cb, sa + ca
                count = len(SAMPLES)
                cr, cg, cb, ca = sr / count, sg / count, sb / count, sa / count
            else:
                cr = cg = cb = ca = 0.0
                for r, g, b, a, _ in plan:
                    keep = 1.0 - a
                    cr, cg, cb, ca = r * a + cr * keep, g * a + cg * keep, b * a + cb * keep, a + ca * keep
            if ca <= 0.0:
                continue
            at = px * 4
            row[at:at + 4] = bytes((min(255, round(cr / ca)), min(255, round(cg / ca)),
                                    min(255, round(cb / ca)), round(ca * 255)))
    return [bytes(row) for row in bleed(width, rows)]


def bleed(width, rows):
    """Each clear pixel beside painted ones given their alpha-weighted
    colour, its alpha kept 0."""
    height = len(rows)
    sums = {}
    for y, row in enumerate(rows):
        for x in range(width):
            a = row[x * 4 + 3]
            if not a:
                continue
            r, g, b = row[x * 4] * a, row[x * 4 + 1] * a, row[x * 4 + 2] * a
            for ny in range(max(0, y - 1), min(height, y + 2)):
                near = rows[ny]
                for nx in range(max(0, x - 1), min(width, x + 2)):
                    if not near[nx * 4 + 3]:
                        total = sums.setdefault((ny, nx), [0, 0, 0, 0])
                        total[0] += r
                        total[1] += g
                        total[2] += b
                        total[3] += a
    for (y, x), (r, g, b, total) in sums.items():
        rows[y][x * 4:x * 4 + 3] = bytes((round(r / total), round(g / total), round(b / total)))
    return rows


# The letters: each takes its left edge, its top, its height and its
# stroke's width, and returns its parts and its width. Round letters reach
# a little past the cap height and the baseline, as they must to look as
# tall as the flat ones.

def diagonal(x0, y0, x1, y1, weight):
    """A stroke from (x0, y0) to (x1, y1), `weight` px across, cut level
    at both ends."""
    half = weight * math.hypot(x1 - x0, y1 - y0) / abs(y1 - y0) / 2
    return convex((x0 - half, y0), (x0 + half, y0), (x1 + half, y1), (x1 - half, y1))


def letter_a(x, top, h, s):
    w, d = 0.84 * h, 1.04 * s
    hull = convex((x + w / 2, top - 0.02 * h), (x + w, top + h), (x, top + h))
    inside = box(x, top, x + w, top + h)
    return [clip(diagonal(x + w / 2, top, x + d / 2, top + h, d), inside),
            clip(diagonal(x + w / 2, top, x + w - d / 2, top + h, d), inside),
            clip(box(x, top + 0.62 * h, x + w, top + 0.62 * h + s), hull)], w


def letter_d(x, top, h, s):
    w, r = 0.78 * h, h / 2
    cx = x + w - r
    return [box(x, top, x + s, top + h), box(x, top, cx, top + s), box(x, top + h - s, cx, top + h),
            arc(cx, top + r, r - s / 2, s / 2, 270, 90)], w


def letter_e(x, top, h, s):
    w = 0.6 * h
    return [box(x, top, x + s, top + h), box(x, top, x + w, top + s),
            box(x, top + (h - s) / 2, x + 0.9 * w, top + (h + s) / 2),
            box(x, top + h - s, x + w, top + h)], w


def letter_g(x, top, h, s):
    w = 1.03 * h
    r = w / 2
    cx, cy = x + r, top + h / 2
    bar = clip(box(cx + 0.04 * h, cy, cx + r, cy + s), disc(cx, cy, r))
    return [arc(cx, cy, r - s / 2, s / 2, 0, 312), bar], w


def letter_h(x, top, h, s):
    w = 0.74 * h
    return [box(x, top, x + s, top + h), box(x + w - s, top, x + w, top + h),
            box(x, top + (h - s) / 2, x + w, top + (h + s) / 2)], w


def letter_i(x, top, h, s):
    return [box(x, top, x + s, top + h)], s


def letter_m(x, top, h, s):
    w, d = 0.92 * h, 1.04 * s
    valley = top + 0.74 * h
    inside = box(x, top, x + w, top + h)
    return [box(x, top, x + s, top + h), box(x + w - s, top, x + w, top + h),
            clip(diagonal(x + d / 2, top, x + w / 2, valley, d), inside),
            clip(diagonal(x + w - d / 2, top, x + w / 2, valley, d), inside)], w


def letter_n(x, top, h, s):
    w, d = 0.8 * h, 1.06 * s
    return [box(x, top, x + s, top + h), box(x + w - s, top, x + w, top + h),
            clip(diagonal(x + d / 2, top, x + w - d / 2, top + h, d), box(x, top, x + w, top + h))], w


def letter_s(x, top, h, s):
    radii = 1.03 * h / 2 - s / 2
    upper, lower = 0.47 * radii, 0.53 * radii
    w = 2 * lower + s
    cx, cy = x + w / 2, top - 0.015 * h + s / 2 + upper
    return [arc(cx, cy, upper, s / 2, 90, 335),
            arc(cx, cy + upper + lower, lower, s / 2, 270, 155)], w


def letter_t(x, top, h, s):
    w = 0.78 * h
    return [box(x, top, x + w, top + s), box(x + (w - s) / 2, top, x + (w + s) / 2, top + h)], w


def letter_u(x, top, h, s):
    w = 0.74 * h
    r = w / 2
    cy = top + h - r + 0.015 * h
    return [box(x, top, x + s, cy), box(x + w - s, top, x + w, cy),
            arc(x + r, cy, r - s / 2, s / 2, 0, 180)], w


def letter_w(x, top, h, s):
    w, d = 1.18 * h, 1.04 * s
    inside = box(x, top, x + w, top + h)
    left, right = x + 0.27 * w, x + 0.73 * w
    return [clip(diagonal(x + d / 2, top, left, top + h, d), inside),
            clip(diagonal(x + w / 2, top, left, top + h, d), inside),
            clip(diagonal(x + w / 2, top, right, top + h, d), inside),
            clip(diagonal(x + w - d / 2, top, right, top + h, d), inside)], w


LETTERS = {'A': letter_a, 'D': letter_d, 'E': letter_e, 'G': letter_g, 'H': letter_h,
           'I': letter_i, 'M': letter_m, 'N': letter_n, 'S': letter_s, 'T': letter_t,
           'U': letter_u, 'W': letter_w}
# Pairs set closer than the tracking, by a share of the height: a T's arm
# or an E's open side leaves more room than a stem does.
KERNING = {'TU': 0.05, 'TE': 0.05, 'ST': 0.03, 'EN': 0.02, 'NG': 0.01, 'IT': 0.02, 'TH': 0.02}


def word(text, left, top, height, weight, tracking):
    """`text`'s capitals as parts from `left`, their tops at `top`, each
    `height` px tall in strokes `weight` px wide; returns the parts and the
    right edge."""
    parts, x = [], left
    for at, letter in enumerate(text):
        if letter == ' ':
            x += 0.42 * height
            continue
        drawn, width = LETTERS[letter](x, top, height, weight)
        parts += drawn
        x += width + tracking - KERNING.get(text[at:at + 2], 0.0) * height
    return parts, x - tracking


# The mark: a wire coiled about a W.

def polyline(points):
    """The polyline through `points`: its length, the distances along it
    of its corners and ends, and a function giving the point and unit
    tangent at a distance along it."""
    pieces, total = [], 0.0
    for (ax, ay), (bx, by) in zip(points, points[1:]):
        length = math.hypot(bx - ax, by - ay)
        pieces.append((total, length, ax, ay, (bx - ax) / length, (by - ay) / length))
        total += length

    def at(s):
        for start, length, ax, ay, tx, ty in pieces:
            if s <= start + length:
                break
        s = min(max(s - start, 0.0), length)
        return ax + tx * s, ay + ty * s, tx, ty
    return total, [piece[0] for piece in pieces] + [total], at


def coil(points):
    """The wire coiled about the polyline through `points` (`PITCH` and
    the rest): its half turns in order, each (points, depths, openness),
    the even ones nearer the eye. A point's depth is 1 nearest the eye, a
    half turn's openness its mean radius over `RADIUS`."""
    total, corners, at = polyline(points)
    halves, steps = [], 16
    for half in range(int(total / (PITCH / 2)) + 1):
        line, depths, openness = [], [], 0.0
        for step in range(steps + 1):
            s = min(total, (half + step / steps) * PITCH / 2)
            t = min(max((min(abs(s - corner) for corner in corners) - BARE) / TAPER, 0.0), 1.0)
            r = RADIUS * t * t * (3 - 2 * t)
            theta = math.tau * s / PITCH
            x, y, tx, ty = at(s)
            across, depth = r * math.cos(theta), math.sin(theta)
            line.append((x - ty * across, y + tx * across + LEAN * r * depth))
            depths.append(depth)
            openness += r / RADIUS / (steps + 1)
        halves.append((line, depths, openness))
        if s >= total:
            break
    return halves


def strokes(line, radii):
    """Capsules along `line`, each segment its two ends' mean radius."""
    return [capsule(*a, *b, (ra + rb) / 2)
            for a, b, ra, rb in zip(line, line[1:], radii, radii[1:]) if ra + rb > 0.0]


def falloff(points, reach, power):
    """A glow about the polyline through `points`: 1 on it, fading to 0
    `reach` px away."""
    lines = [capsule(*a, *b, 0.0)[0] for a, b in zip(points, points[1:])]

    def field(x, y):
        q = min(f(x, y) for f in lines) / reach
        t = 1.0 - q * q
        if t <= 0.0:
            return 0.0
        value = 1.0
        for _ in range(power):
            value *= t
        return value
    xs, ys = [x for x, _ in points], [y for _, y in points]
    return field, (min(xs) - reach, min(ys) - reach, max(xs) + reach, max(ys) + reach)


def mark(cx, cy):
    """The filament about (cx, cy): the haze, the far halves of its turns,
    the glow over them, then the near halves with their hot cores."""
    w, h = 140.0, 112.0
    left, top = cx - w / 2, cy - h / 2
    spine = [(left, top), (left + w / 4, top + h), (cx, top + 0.26 * h),
             (left + 3 * w / 4, top + h), (left + w, top)]
    halves = coil(spine)
    hot, cool = mix(ACCENT, FOCUS, HOT), mix(ACCENT, (0, 0, 0), FAR)
    reach, alpha, power = HAZE
    layers = [glow(ACCENT, alpha, *falloff(spine, reach, power))]
    for line, _, openness in halves[1::2]:
        layers.append(shape(mix(hot, cool, openness), strokes(line, [WIRE * BACK] * len(line))))
    reach, alpha, power = GLOW
    layers.append(glow(ACCENT, alpha, *falloff(spine, reach, power)))
    for line, depths, openness in halves[::2]:
        layers.append(shape(hot, strokes(line, [WIRE] * len(line))))
        core = [CORE * WIRE * (math.sqrt(max(0.0, d)) * openness + 1 - openness) for d in depths]
        layers.append(shape(FOCUS, strokes(line, core)))
    return layers


def logo():
    """The mark, and the two lines of words centred on it."""
    width, height = LOGO
    middle = height / 2
    layers = mark(140.0, middle)
    big, weight, tracking = 62.0, 10.0, 0.24 * 62.0
    small, thin, gap = 20.0, 4.4, 16.0
    top = middle - (small + gap + big) / 2
    right = width - MARGIN - 2
    left = right - word('TUNGSTEN', 0.0, 0.0, big, weight, tracking)[1]
    over, _ = word('MADE WITH', left, top, small, thin, 0.42 * small)
    wordmark, _ = word('TUNGSTEN', left, top + small + gap, big, weight, tracking)
    layers.append(shape(MUTED, over))
    layers.append(shape(TEXT, wordmark))
    return width, height, layers


def bar():
    """The bar: the accent, its middle rows warmed toward the focus
    colour."""
    width, height = BAR
    layers = []
    for y in range(height):
        q = (y + 0.5 - height / 2) / (height / 2)
        t = (1.0 - q * q) * (1.0 - q * q)
        layers.append(shape(mix(ACCENT, FOCUS, 0.4 * t), [box(0, y, width, y + 1)]))
    return width, height, layers


# Each file's drawing and the margin its art keeps clear.
DRAWN = {'boot_logo.png': (logo, MARGIN), 'boot_bar.png': (bar, 0)}


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--check', action='store_true',
                        help='compare the files in assets/sprites/ with a fresh render; write nothing')
    parser.add_argument('--out', type=pathlib.Path, default=sprites.SPRITES,
                        help='the folder to write, assets/sprites/ by default')
    args = parser.parse_args()
    drawn = {}
    for name, (draw, margin) in DRAWN.items():
        width, height, layers = draw()
        drawn[name] = (width, height, paint(width, height, layers, margin))
    if args.check:
        failed = False
        for name, image in drawn.items():
            path = sprites.SPRITES / name
            try:
                matches = path.is_file() and sprites.decode(path.read_bytes()) == image
            except (ValueError, zlib.error, struct.error) as error:
                print(f'{name}: unreadable: {error}', file=sys.stderr)
                matches = None
            if matches:
                print(f'{name}: matches')
            else:
                if matches is not None:
                    print(f'{name}: differs from a fresh render', file=sys.stderr)
                failed = True
        if failed:
            return 1
        print(f'{len(drawn)} sprites match')
        return 0
    args.out.mkdir(parents=True, exist_ok=True)
    for name, (width, height, rows) in drawn.items():
        (args.out / name).write_bytes(sprites.encode(width, height, rows))
    print(f'wrote {len(drawn)} sprites to {args.out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
