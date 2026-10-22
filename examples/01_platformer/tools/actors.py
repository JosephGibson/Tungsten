"""Player and enemy art rendered from posed skeletons into palette pixels.

The courier is rebuilt for every frame from one small rig: hips, neck, feet,
hands, lantern swing, cloak hem, scarf chain and hood tip. Each part is filled
flat, shaded from its own silhouette, rimmed where it overlaps the parts behind
it and outlined once, so every clip shares one model and no raster is rotated.
The fireball is an analytic ball of flame whose tongues stream up and behind.
"""
import math
from dataclasses import dataclass

from art import PALETTE
from pixels import (
    BAYER, NEIGHBOURS, Part, add, compose, despeckle, outline, point, shade, smoothstep,
    to_image, unit,
)

SIZE = 64
SOLE = 59  # lowest drawn row; the outline adds row 60, so boxes end at 61
ANKLE = SOLE - 2  # a flat boot's ankle when planted
LANTERN_REGION = (40, 30, 63, 60)  # placement.py searches this box for the flame
THIGH = SHIN = 8.5
UPPER_ARM = FOREARM = 6.5
REST_HIP = (30.0, 42.0)
FIREBALL_FRAMES = 10


@dataclass
class Pose:
    """One frame of the courier, facing right. Positions are sprite pixels."""
    hip: tuple = REST_HIP
    lean: float = 1.0  # neck x offset from the hip
    head: tuple = (0.0, 0.0)  # extra head offset, e.g. for a tuck
    near_foot: tuple = (34.0, ANKLE)
    far_foot: tuple = (26.0, ANKLE)
    near_toe: float = 0.0  # radians; positive points the toe down
    far_toe: float = 0.0
    near_planted: bool = True  # a planted boot keeps its lowest point on the sole row
    far_planted: bool = True
    near_hand: tuple = (42.0, 37.0)
    far_hand: tuple = (24.0, 40.0)
    lantern: float = 0.0  # radians; positive swings the lantern forward
    hem_flare: float = 0.0  # back hem trails this far behind the hip
    hem_lift: float = 0.0  # back hem rises; negative drops it
    hem_front: float = 0.0  # front hem pushed forward
    hem_wave: float = 0.0  # ripple phase
    hem_amp: float = 0.6
    hem_rise: float = 0.0  # whole hem rises, showing more leg
    scarf_angle: float = 110.0  # degrees in y-down space: 90 hangs, 180 trails back
    scarf_amp: float = 8.0
    scarf_curl: float = 0.0  # root angle minus tip angle; bends the tail
    scarf_phase: float = 0.0
    hood_tip: tuple = (0.0, 0.0)
    satchel: tuple = (0.0, 0.0)
    eyes: str = 'open'  # open, closed or squint
    flame: float = 1.0  # the lantern's flame: 1 whole, 0 an ember


def two_bone(root, target, upper, lower, bend):
    """Joint and reachable end for a two-bone limb; bend picks the joint side."""
    dx, dy = target[0] - root[0], target[1] - root[1]
    distance = max(math.hypot(dx, dy), 1e-6)
    ux, uy = dx / distance, dy / distance
    reach = min(distance, upper + lower - 1e-3)
    end = (root[0] + ux * reach, root[1] + uy * reach)
    along = (upper * upper - lower * lower + reach * reach) / (2 * reach)
    side = math.sqrt(max(upper * upper - along * along, 0.0))
    # (uy, -ux) points forward (+x) for a limb hanging straight down.
    joint = (root[0] + ux * along + uy * side * bend, root[1] + uy * along - ux * side * bend)
    return joint, end


def boot(ankle, toe_angle, planted):
    """Boot around the ankle; planted boots sit on the sole row, others stay above it."""
    local = [(-2, -1), (-2, 2), (5, 2), (5, 0), (2, -1)]
    cos, sin = math.cos(toe_angle), math.sin(toe_angle)
    points = [(ankle[0] + x * cos - y * sin, ankle[1] + x * sin + y * cos) for x, y in local]
    lowest = max(p[1] for p in points)
    if planted or lowest > SOLE:
        points = [(x, y + SOLE - lowest) for x, y in points]
    return points


def leg(hip, foot, toe, planted, trousers, boots):
    # Out of reach, the ankle stops short; a planted boot then rises onto its toe.
    knee, ankle = two_bone(hip, foot, THIGH, SHIN, 1)
    if planted and ankle[1] < foot[1] - 0.5:
        toe = max(toe, 0.5 + (foot[1] - ankle[1]) * 0.25)
    shape = Part()
    shape.limb([hip, knee, ankle], 4)
    shoe = Part()
    shoe.polygon(boot(ankle, toe, planted))
    return shade(shape.pixels(), **trousers), shade(shoe.pixels(), **boots)


def arm(shoulder, hand, sleeve, glove, cuff=None):
    # Elbows bend down and back.
    elbow, wrist = two_bone(shoulder, hand, UPPER_ARM, FOREARM, -1)
    shape = Part()
    shape.limb([shoulder, elbow], 5)
    shape.limb([elbow, wrist], 4)
    inside = shape.pixels()
    colors = shade(inside, **sleeve)
    if cuff:
        for x, y in inside:
            if math.hypot(x - wrist[0], y - wrist[1]) < 2.2:
                colors[(x, y)] = cuff
    fist = Part()
    fist.ellipse(wrist, 1.5, 1.5)
    return colors, shade(fist.pixels(), **glove), wrist


NEAR_SLEEVE = dict(base='Q', light='U', shadow='P')
FAR_SLEEVE = dict(base='P', light='Q', shadow='O')
NEAR_GLOVE = dict(base='(', light='a', shadow='[')
FAR_GLOVE = dict(base='(', shadow='[')
NEAR_TROUSERS = dict(base=']', shadow='[')
FAR_TROUSERS = dict(base='[', shadow='o')
NEAR_BOOT = dict(base='(', light='a', shadow='[')
FAR_BOOT = dict(base='[', light='(', shadow='o')


def cloak(neck, hip, pose):
    """Belted cloak from shoulders to a rippling hem that flares behind."""
    hem_y = hip[1] + 8.5 - pose.hem_rise
    back_shoulder = (neck[0] - 5, neck[1] + 1)
    front_shoulder = (neck[0] + 4, neck[1] + 1)
    front_waist = (hip[0] + 5 + pose.hem_front * 0.3, hip[1] - 1)
    front_hem = (hip[0] + 7 + pose.hem_front, min(hem_y - pose.hem_lift * 0.3, SOLE))
    back_waist = (hip[0] - 7 - pose.hem_flare * 0.35, hip[1] - 2 - pose.hem_lift * 0.3)
    back_hem = (hip[0] - 8 - pose.hem_flare, min(hem_y - pose.hem_lift, SOLE))
    hem = []
    steps = 7
    for k in range(steps + 1):
        f = k / steps  # 0 at the front, 1 at the back
        x = front_hem[0] + (back_hem[0] - front_hem[0]) * f
        y = front_hem[1] + (back_hem[1] - front_hem[1]) * f
        y += pose.hem_amp * (0.4 + f) * math.sin(pose.hem_wave + k * 1.25)
        hem.append((x, min(y, SOLE)))
    shape = Part()
    shape.polygon([back_shoulder, front_shoulder, front_waist] + hem + [back_waist])
    inside = shape.pixels()
    colors = shade(inside, 'Q', light='U', shadow='P', back='P', trim='W', rim='U',
                   trim_from=round(hip[1] + 3))
    # Folds rise from the hem and follow its ripple.
    for k, offset in enumerate((-5.0, -1.0, 3.0)):
        x = round(hip[0] + offset - pose.hem_flare * 0.25 * (k == 0))
        bottom = max((y for (px, y) in inside if px == x), default=None)
        if bottom is None:
            continue
        length = 4 + (k + round(pose.hem_wave)) % 3
        for y in range(bottom - length, bottom):
            if (x, y) in inside:
                colors[(x, y)] = 'P'
    # Leather belt and brass buckle.
    for y in (round(hip[1]) - 3, round(hip[1]) - 2):
        for x in range(round(hip[0]) - 7, round(hip[0]) + 8):
            if (x, y) in inside:
                colors[(x, y)] = 'A' if round(hip[0]) + 2 <= x <= round(hip[0]) + 3 else '('
    return colors


def hood(neck, pose):
    center = (neck[0] + 2.5 + pose.head[0], neck[1] - 9 + pose.head[1])
    shape = Part()
    shape.ellipse(center, 8, 8.5)
    # A soft liripipe tail droops from the back of the crown.
    tip = (center[0] - 11 + pose.hood_tip[0], center[1] + 1 + pose.hood_tip[1])
    shape.polygon([(center[0] - 4, center[1] - 7.5), (center[0] - 7.5, center[1] - 2), tip])
    shape.limb([(center[0] - 6, center[1] - 4), tip], 2)
    # The mantle drapes onto both shoulders.
    shape.polygon([(center[0] - 7.5, center[1] + 1), (neck[0] - 6, neck[1] + 3),
                   (neck[0] + 5, neck[1] + 3), (center[0] + 7, center[1] + 3)])
    inside = shape.pixels()
    colors = shade(inside, 'Q', light='W', shadow='P', back='O')
    # A second, softer highlight row across the front of the crown.
    for x, y in inside:
        if (x, y + 1) in inside and (x, y - 1) not in inside and x > center[0] - 4:
            colors[(x, y + 1)] = 'U'
    face = Part()
    face_center = (center[0] + 3.5, center[1] + 1.5)
    face.ellipse(face_center, 4, 4.5)
    void = face.pixels() & inside
    for x, y in inside:
        near_void = any((x + dx, y + dy) in void for dx, dy in NEIGHBOURS)
        if near_void and (x, y) not in void and x >= face_center[0] - 1:
            colors[(x, y)] = 'U'  # the hood lining catches the lantern
    for p in void:
        colors[p] = 'o'
    ex, ey = point((face_center[0] + 1, face_center[1] - 0.5))
    if pose.eyes == 'closed':
        eyes = {(x, ey + 1): 'a' for x in (ex - 3, ex - 2, ex, ex + 1)}
    elif pose.eyes == 'squint':
        eyes = {(x, ey + 1): 'y' for x in (ex - 3, ex - 2, ex, ex + 1)}
    else:
        eyes = {(ex - 3, ey): 'y', (ex - 3, ey + 1): 'y', (ex, ey): 'Y', (ex + 1, ey): 'y',
                (ex, ey + 1): 'y', (ex + 1, ey + 1): 'y'}
    colors.update({p: s for p, s in eyes.items() if p in void})
    return colors


def scarf(neck, pose):
    style = dict(base='z', light='+', shadow='X')
    wrap = Part()
    wrap.polygon([(neck[0] - 5, neck[1] - 2), (neck[0] + 5, neck[1] - 1),
                  (neck[0] + 5, neck[1] + 2), (neck[0] - 5, neck[1] + 2)])
    wrap.ellipse((neck[0] + 5, neck[1] + 1), 1.5, 1.5)  # the knot
    tail = Part()
    points = [(neck[0] - 4, neck[1])]
    for i in range(6):
        bend = pose.scarf_curl * (1 - i / 5)
        wave = pose.scarf_amp * math.sin(pose.scarf_phase - i * 1.05)
        angle = math.radians(pose.scarf_angle + bend + wave)
        points.append(add(points[-1], (math.cos(angle), math.sin(angle)), 3.3))
    tail.limb(points[:3], 4)
    tail.limb(points[2:5], 3)
    tail.limb(points[4:], 2)
    return shade(wrap.pixels(), **style), shade(tail.pixels(), **style)


def satchel(hip, pose):
    corner = add((hip[0] - 11, hip[1] - 8), pose.satchel)
    bag = Part()
    bag.polygon([corner, (corner[0] + 5, corner[1]), (corner[0] + 5, corner[1] + 7),
                 (corner[0], corner[1] + 7)])
    colors = shade(bag.pixels(), 'a', light='A', shadow='(', back='(')
    flap_y = round(corner[1]) + 2
    for x in range(round(corner[0]), round(corner[0]) + 6):
        if (x, flap_y) in colors:
            colors[(x, flap_y)] = '('
    return colors, (corner[0] + 5, corner[1] + 1)


def strap(neck, anchor):
    band = Part()
    band.limb([(neck[0] + 4, neck[1] + 2), anchor], 1)
    return {p: '(' for p in band.pixels()}


def lantern(hand, angle, flame=1.0):
    """Upright lantern hanging from the hand by its bail; only the bail swings.
    A flame below 1 dies down the glass from the top, to an ember at 0."""
    pivot = (hand[0] + 2, hand[1] + 1)
    top = add(pivot, (math.sin(angle), math.cos(angle)), 4.5)
    cx, ty = point(top)
    colors = {}
    bail = Part()
    bail.limb([pivot, (cx, ty)], 1)
    for p in bail.pixels():
        colors[p] = 's'
    for x in range(cx - 2, cx + 3):
        colors[(x, ty)] = 'a'
        colors[(x, ty + 8)] = 'a'
    for x in range(cx - 3, cx + 4):
        colors[(x, ty + 1)] = 'A'
        colors[(x, ty + 7)] = '('
    for y in range(ty + 2, ty + 7):
        colors[(cx - 3, y)] = 'o'
        colors[(cx + 3, y)] = 'o'
        for x in range(cx - 2, cx + 3):
            colors[(x, y)] = 'Y' if abs(x - cx) <= 1 and ty + 3 <= y <= ty + 5 else 'y'
    if flame < 1.0:
        # The glass darkens from the top down; below half, the hot core goes
        # and the ember keeps the bottom row's middle.
        lit = ty + 6 - round(4 * flame)
        for y in range(ty + 2, ty + 7):
            for x in range(cx - 2, cx + 3):
                if y < lit or (y == ty + 6 and abs(x - cx) > 1 and flame < 0.5):
                    colors[(x, y)] = 'o'
                elif flame < 0.5:
                    colors[(x, y)] = 'y'
    return colors


def render(pose):
    hip = pose.hip
    neck = (hip[0] + pose.lean, hip[1] - 17)
    near_shoulder = (neck[0] + 2, neck[1] + 3)
    far_shoulder = (neck[0] - 3, neck[1] + 3)
    wrap, tail = scarf(neck, pose)
    far_sleeve, far_glove, _ = arm(far_shoulder, pose.far_hand, FAR_SLEEVE, FAR_GLOVE)
    far_leg, far_boot = leg((hip[0] - 2, hip[1]), pose.far_foot, pose.far_toe,
                            pose.far_planted, FAR_TROUSERS, FAR_BOOT)
    near_leg, near_boot = leg((hip[0] + 1, hip[1]), pose.near_foot, pose.near_toe,
                              pose.near_planted, NEAR_TROUSERS, NEAR_BOOT)
    bag, bag_anchor = satchel(hip, pose)
    near_sleeve, near_glove, wrist = arm(near_shoulder, pose.near_hand, NEAR_SLEEVE,
                                         NEAR_GLOVE, cuff='W')
    lamp = lantern(wrist, pose.lantern, pose.flame)
    canvas = compose([
        (tail, None),
        (far_sleeve, 'o'), (far_glove, None),
        (far_leg, 'o'), (far_boot, 'o'),
        (near_leg, 'o'), (near_boot, 'o'),
        (bag, 'o'),
        (cloak(neck, hip, pose), 'o'),
        (strap(neck, bag_anchor), None),
        (hood(neck, pose), 'o'),
        (wrap, 'o'),
        (lamp, 'o'),
        (near_sleeve, 'o'), (near_glove, 'o'),
    ])
    # Drop isolated specks so the outline stays one clean line.
    for p in [p for p in canvas if not any((p[0] + dx, p[1] + dy) in canvas for dx, dy in NEIGHBOURS)]:
        del canvas[p]
    glass = {p for p, s in lamp.items() if s in 'yY' and canvas.get(p) == s}
    return to_image(outline(canvas)), glass


def finish(canvas, lamp):
    """The frame's image and its lantern glass, the specks dropped first."""
    for p in [p for p in canvas if not any((p[0] + dx, p[1] + dy) in canvas for dx, dy in NEIGHBOURS)]:
        del canvas[p]
    glass = {p for p, s in lamp.items() if s in 'yY' and canvas.get(p) == s}
    return to_image(outline(canvas)), glass


def render_toss(pose, front, ball):
    """A toss frame: the free (far) hand swings a bowling ball from behind
    the cloak on the wind-up and over it after, the lantern hand braced."""
    hip = pose.hip
    neck = (hip[0] + pose.lean, hip[1] - 17)
    near_shoulder = (neck[0] + 2, neck[1] + 3)
    far_shoulder = (neck[0] - 3, neck[1] + 3)
    wrap, tail = scarf(neck, pose)
    far_leg, far_boot = leg((hip[0] - 2, hip[1]), pose.far_foot, pose.far_toe,
                            pose.far_planted, FAR_TROUSERS, FAR_BOOT)
    near_leg, near_boot = leg((hip[0] + 1, hip[1]), pose.near_foot, pose.near_toe,
                              pose.near_planted, NEAR_TROUSERS, NEAR_BOOT)
    bag, bag_anchor = satchel(hip, pose)
    near_sleeve, near_glove, wrist = arm(near_shoulder, pose.near_hand, NEAR_SLEEVE,
                                         NEAR_GLOVE, cuff='W')
    lamp = lantern(wrist, pose.lantern, pose.flame)
    sleeve, glove = (NEAR_SLEEVE, NEAR_GLOVE) if front else (FAR_SLEEVE, FAR_GLOVE)
    throw_sleeve, throw_glove, throw_wrist = arm(far_shoulder, pose.far_hand, sleeve, glove)
    held = {}
    if ball:
        orb = Part()
        orb.ellipse((throw_wrist[0] + 1, throw_wrist[1] - 1), 3, 3)
        held = shade(orb.pixels(), 'β', light='δ', shadow='γ')
    throw = [(throw_sleeve, 'o'), (throw_glove, 'o'), (held, 'o')]
    body = [(far_leg, 'o'), (far_boot, 'o'), (near_leg, 'o'), (near_boot, 'o'), (bag, 'o'),
            (cloak(neck, hip, pose), 'o'), (strap(neck, bag_anchor), None),
            (hood(neck, pose), 'o'), (wrap, 'o')]
    behind, over = ([], throw) if front else (throw, [])
    canvas = compose([(tail, None), *behind, *body, *over,
                      (lamp, 'o'), (near_sleeve, 'o'), (near_glove, 'o')])
    return finish(canvas, lamp)


def gun(wrist, aim):
    """A short pistol at the wrist along `aim` (radians, up positive), drawn
    as a polygon at that angle: no raster is rotated."""
    ax, ay = math.cos(aim), -math.sin(aim)
    px, py = -ay, ax
    def at(u, v):
        return (wrist[0] + ax * u + px * v, wrist[1] + ay * u + py * v)
    barrel = Part()
    barrel.polygon([at(-1, -1.6), at(8, -1.6), at(8, 0.6), at(-1, 0.6)])
    grip = Part()
    grip.polygon([at(-1, 0.0), at(1.5, 0.0), at(0.5, 3.5), at(-2, 3.5)])
    colors = shade(grip.pixels(), '[', shadow='o')
    colors.update(shade(barrel.pixels(), 'Ω', light='χ', shadow='Φ'))
    return colors


# The aim frames' directions, degrees up from forward: the five the courier
# draws facing the aim; the three behind it are these mirrored.
AIMS = (90, 45, 0, -45, -90)


def render_aim(degrees):
    """Standing, the free arm and its gun along the aim, drawn over the
    cloak so a downward aim stays visible."""
    aim = math.radians(degrees)
    pose = Pose(lantern=0.1, scarf_angle=132, scarf_curl=-22, scarf_amp=8)
    hip = pose.hip
    neck = (hip[0] + pose.lean, hip[1] - 17)
    near_shoulder = (neck[0] + 2, neck[1] + 3)
    far_shoulder = (neck[0] - 1, neck[1] + 4)
    hand = (far_shoulder[0] + math.cos(aim) * 12.0, far_shoulder[1] - math.sin(aim) * 12.0)
    wrap, tail = scarf(neck, pose)
    far_leg, far_boot = leg((hip[0] - 2, hip[1]), pose.far_foot, pose.far_toe, True,
                            FAR_TROUSERS, FAR_BOOT)
    near_leg, near_boot = leg((hip[0] + 1, hip[1]), pose.near_foot, pose.near_toe, True,
                              NEAR_TROUSERS, NEAR_BOOT)
    bag, bag_anchor = satchel(hip, pose)
    near_sleeve, near_glove, wrist = arm(near_shoulder, pose.near_hand, NEAR_SLEEVE,
                                         NEAR_GLOVE, cuff='W')
    lamp = lantern(wrist, pose.lantern, pose.flame)
    gun_sleeve, gun_glove, gun_wrist = arm(far_shoulder, hand, FAR_SLEEVE, FAR_GLOVE)
    canvas = compose([
        (tail, None),
        (far_leg, 'o'), (far_boot, 'o'), (near_leg, 'o'), (near_boot, 'o'),
        (bag, 'o'), (cloak(neck, hip, pose), 'o'), (strap(neck, bag_anchor), None),
        (hood(neck, pose), 'o'), (wrap, 'o'),
        (gun_sleeve, 'o'), (gun_glove, 'o'), (gun(gun_wrist, aim), 'o'),
        (lamp, 'o'), (near_sleeve, 'o'), (near_glove, 'o'),
    ])
    return finish(canvas, lamp)


def cast_frame(im):
    """A frame's cast twin: its lantern's glass all at the hot core and the
    frame round it lit, so its emissive mask widens."""
    out = im.copy()
    px = out.load()
    x0, y0, x1, y1 = LANTERN_REGION
    hot = (PALETTE['y'], PALETTE['Y'])
    glass = {(x, y) for y in range(y0, y1) for x in range(x0, x1) if px[x, y] in hot}
    for p in glass:
        px[p] = PALETTE['Y']
    ring = {(x + dx, y + dy) for x, y in glass for dx in (-1, 0, 1) for dy in (-1, 0, 1)} - glass
    for p in ring:
        if px[p][3]:
            px[p] = PALETTE['y']
    return out


def run_foot(hip_x, phase):
    """Foot target, planted flag and toe angle for a 50% stance run cycle."""
    if phase < 0.5:
        s = phase / 0.5
        return (hip_x + 6 - 14 * s, ANKLE), True, 0.0
    u = (phase - 0.5) / 0.5
    lift = 8.5 * math.sin(math.pi * u) ** 0.9 + 2.5 * math.sin(math.pi * min(1.0, u * 2.2))
    x = hip_x - 8 + 14 * smoothstep(u)
    return (x, ANKLE - lift), False, 0.7 * (1 - u) - 0.3 * u


def idle_poses():
    poses = []
    for i in range(8):
        p = i / 8
        breath = -round(0.5 - 0.5 * math.cos(math.tau * p) + 0.01)
        poses.append(Pose(
            hip=(REST_HIP[0], REST_HIP[1] + breath),
            near_hand=(42, 37 + breath), far_hand=(24, 40 + breath),
            lantern=0.2 * math.sin(math.tau * p - 0.9),
            hem_wave=math.tau * p, hem_amp=0.7,
            scarf_angle=132, scarf_curl=-22, scarf_amp=8, scarf_phase=math.tau * p,
            hood_tip=(0.6 * math.sin(math.tau * p - 0.6), 0.8 * math.sin(math.tau * p)),
            eyes='closed' if i == 6 else 'open'))
    return poses, [300, 240, 240, 300, 240, 240, 100, 240]


def run_poses():
    poses = []
    for i in range(12):
        p = i / 12
        step = (2 * p) % 1
        bob = round(2 * math.cos(math.tau * (step - 0.15)))
        hip = (REST_HIP[0] - 1, REST_HIP[1] + bob)
        near, near_planted, near_toe = run_foot(hip[0] + 1, p)
        far, far_planted, far_toe = run_foot(hip[0] - 2, (p + 0.5) % 1)
        neck = (hip[0] + 3.5, hip[1] - 17)
        swing = math.cos(math.tau * p)
        # The free arm pumps against the near leg; the lantern arm stays braced.
        far_hand = add((neck[0] - 3, neck[1] + 3), (1 + 7 * swing, 8 - 2 * abs(swing)))
        near_hand = (43 + 2.0 * math.sin(math.tau * p + 0.5), neck[1] + 12 - 1.5 * swing)
        poses.append(Pose(
            hip=hip, lean=3.5, near_foot=near, far_foot=far, near_toe=near_toe,
            far_toe=far_toe, near_planted=near_planted, far_planted=far_planted,
            near_hand=near_hand, far_hand=far_hand,
            lantern=0.1 - 0.35 * math.cos(math.tau * p - 1.3),
            hem_flare=5 + 1.5 * math.sin(math.tau * 2 * p), hem_lift=3, hem_front=-1,
            hem_wave=math.tau * 2 * p, hem_amp=1.4, hem_rise=2,
            scarf_angle=192, scarf_curl=-42, scarf_amp=20, scarf_phase=math.tau * 2 * p,
            hood_tip=(-2, -3 + 0.8 * math.sin(math.tau * 2 * p - 1)),
            satchel=(0, -0.8 * math.sin(math.tau * 2 * p - 1))))
    return poses, [45] * 12


def jump_poses():
    hip = (REST_HIP[0], REST_HIP[1] - 2)
    takeoff = Pose(
        hip=hip, lean=1.5, near_foot=(hip[0] + 1.5, ANKLE), near_toe=0.9,
        far_foot=(hip[0] - 6, 50), far_toe=0.6, far_planted=False,
        near_hand=(43, 29), far_hand=(21, 14), lantern=-0.3,
        hem_flare=2, hem_lift=-2, hem_amp=0.5, scarf_angle=112, scarf_amp=6, hood_tip=(1, 4))
    rise = Pose(
        hip=(hip[0], hip[1] + 1), lean=1.5, near_foot=(hip[0] + 5, 51), near_toe=0.2,
        near_planted=False, far_foot=(hip[0] - 3.5, ANKLE), far_toe=1.0,
        near_hand=(43, 30), far_hand=(23, 15), lantern=-0.25,
        hem_flare=3, hem_lift=-3, hem_wave=1.2, hem_amp=1.0, scarf_angle=140, scarf_curl=-24,
        scarf_amp=10,
        scarf_phase=1.0, hood_tip=(0, 3))
    apex = Pose(
        hip=(hip[0], hip[1] + 1), lean=1.0, near_foot=(hip[0] + 4, 53), near_toe=0.3,
        near_planted=False, far_foot=(hip[0] - 2.5, ANKLE), far_toe=0.9,
        near_hand=(44, 33), far_hand=(19, 29), lantern=0.2,
        hem_flare=4, hem_lift=3, hem_wave=2.4, hem_amp=1.2, scarf_angle=178, scarf_curl=-40,
        scarf_amp=14,
        scarf_phase=2.0, hood_tip=(-1, -2))
    # The rise clip must finish inside ~13 frames; the apex pose then holds.
    return [takeoff, rise, apex], [60, 70, 70]


def fall_poses():
    poses = []
    for i in range(4):
        p = i / 4
        flutter = math.sin(math.tau * p)
        hip = (REST_HIP[0], REST_HIP[1] - 1)
        poses.append(Pose(
            hip=hip, lean=0.5,
            near_foot=(hip[0] + 3, 55 + flutter), near_toe=0.5, near_planted=False,
            far_foot=(hip[0] - 3, ANKLE), far_toe=0.8,
            near_hand=(44, 31 + flutter), far_hand=(20, 21 - flutter),
            lantern=0.25 * math.sin(math.tau * p + 1.0),
            hem_flare=3, hem_lift=6 + flutter, hem_wave=math.tau * p, hem_amp=2.0,
            scarf_angle=238, scarf_curl=-62, scarf_amp=22, scarf_phase=math.tau * p,
            hood_tip=(2, -7 + flutter)))
    return poses, [80] * 4


def land_poses():
    impact = Pose(
        hip=(REST_HIP[0], REST_HIP[1] + 2), near_foot=(37, ANKLE), far_foot=(24, ANKLE),
        near_hand=(43, 39), far_hand=(20, 42), lantern=0.45,
        hem_flare=3, hem_front=2, hem_amp=0.4, scarf_angle=112, scarf_curl=-10, scarf_amp=4, hood_tip=(0, 3),
        satchel=(0, 1), eyes='squint')
    recover = Pose(
        hip=(REST_HIP[0], REST_HIP[1] + 1), near_foot=(36, ANKLE), far_foot=(25, ANKLE),
        near_hand=(42, 38), far_hand=(22, 41), lantern=-0.2,
        hem_flare=1.5, hem_front=1, hem_wave=1.5, hem_amp=0.6, scarf_angle=124, scarf_curl=-18,
        scarf_amp=6,
        scarf_phase=1.5, hood_tip=(0, 1))
    settle = Pose(lantern=0.1, hem_flare=0.5, hem_wave=3.0, scarf_angle=132, scarf_curl=-22,
                  scarf_phase=3.0)
    return [impact, recover, settle], [60, 60, 60]


def double_jump_poses():
    """Tuck, then the cloak bursts open like a bell and the courier springs up."""
    hip = (REST_HIP[0], REST_HIP[1] - 1)
    # Knees to chest: the whole ball drops to the collider's feet before springing.
    tuck = Pose(
        hip=(hip[0], hip[1] + 5), lean=3.0, head=(1, 1), near_foot=(hip[0] + 6, ANKLE),
        far_foot=(hip[0] + 1, ANKLE), near_hand=(43, 40), far_hand=(37, 39), lantern=0.4,
        hem_flare=2, hem_lift=-1, hem_front=2, hem_amp=0.8, scarf_angle=110,
        scarf_curl=-30, scarf_amp=10, hood_tip=(3, 4))
    burst = Pose(
        hip=(hip[0], hip[1] - 1), lean=1.0, near_foot=(hip[0] + 3, ANKLE), near_toe=1.0,
        far_foot=(hip[0] - 6, 53), far_toe=0.6, far_planted=False,
        near_hand=(44, 30), far_hand=(19, 15), lantern=-0.35,
        hem_flare=9, hem_front=5, hem_lift=5, hem_wave=1.0, hem_amp=2.0,
        scarf_angle=222, scarf_curl=-50, scarf_amp=22, scarf_phase=1.0, hood_tip=(-2, -6))
    spread = Pose(
        hip=(hip[0], hip[1] - 1), lean=1.0, near_foot=(hip[0] + 3, 54), near_toe=0.5,
        near_planted=False, far_foot=(hip[0] - 3, ANKLE), far_toe=1.0,
        near_hand=(44, 31), far_hand=(20, 19), lantern=-0.1,
        hem_flare=6, hem_front=3, hem_lift=4, hem_wave=2.2, hem_amp=1.8,
        scarf_angle=204, scarf_curl=-46, scarf_amp=18, scarf_phase=2.2, hood_tip=(-2, -4))
    rise = Pose(
        hip=(hip[0], hip[1] - 1), lean=1.5, near_foot=(hip[0] + 4, 52), near_toe=0.3,
        near_planted=False, far_foot=(hip[0] - 3, ANKLE), far_toe=1.0,
        near_hand=(43, 31), far_hand=(23, 18), lantern=0.15,
        hem_flare=4, hem_front=1, hem_lift=1, hem_wave=3.4, hem_amp=1.2,
        scarf_angle=180, scarf_curl=-40, scarf_amp=14, scarf_phase=3.4, hood_tip=(-1, -2))
    return [tuck, burst, spread, rise], [50, 60, 70, 120]


def death_poses():
    """The courier's death: it staggers back from the blow, its knees
    buckle, it kneels and slumps forward over the lantern it sets down, and
    the flame dies to an ember; the last frame holds."""
    h = REST_HIP
    stagger = Pose(hip=(h[0] - 2, h[1] + 1), lean=-2.0, head=(-1.0, 0.5),
                   near_foot=(34, ANKLE), far_foot=(23, ANKLE), far_toe=0.15,
                   near_hand=(46, 35), far_hand=(16, 37), lantern=-0.15,
                   hem_flare=3, hem_front=-1, hem_wave=1.0, hem_amp=0.8,
                   scarf_angle=165, scarf_curl=-14, scarf_amp=6, scarf_phase=1.0,
                   hood_tip=(-1, -2), eyes='squint')
    buckle = Pose(hip=(h[0] - 1, h[1] + 4), lean=2.0, head=(0.5, 2.0),
                  near_foot=(36, ANKLE), far_foot=(24, ANKLE),
                  near_hand=(43, 41), far_hand=(22, 46), lantern=0.35,
                  hem_flare=1.5, hem_wave=2.0, hem_amp=0.6, hem_rise=-1,
                  scarf_angle=140, scarf_curl=-10, scarf_amp=4, scarf_phase=2.0,
                  hood_tip=(0, 1), eyes='closed')
    # The far knee meets the ground, its boot behind it; the near foot stays
    # planted ahead, the lantern resting on the ground in the near hand.
    kneel = Pose(hip=(h[0], h[1] + 7), lean=4.5, head=(1.5, 4.0),
                 near_foot=(37, ANKLE), far_foot=(21, ANKLE + 1), far_toe=1.2, far_planted=False,
                 near_hand=(42, 45), far_hand=(31, 53), lantern=0.1,
                 hem_flare=0.5, hem_wave=3.0, hem_amp=0.4, hem_rise=-2,
                 scarf_angle=110, scarf_curl=-4, scarf_amp=3, scarf_phase=3.0,
                 hood_tip=(1, 2), eyes='closed', flame=0.6)
    slump = Pose(hip=(h[0], h[1] + 8), lean=6.0, head=(3.0, 6.0),
                 near_foot=(37, ANKLE), far_foot=(21, ANKLE + 1), far_toe=1.2, far_planted=False,
                 near_hand=(42, 45), far_hand=(36, 55), lantern=0.0,
                 hem_wave=3.6, hem_amp=0.2, hem_rise=-2.5,
                 scarf_angle=96, scarf_curl=0, scarf_amp=2, scarf_phase=3.6,
                 hood_tip=(2, 3), eyes='closed', flame=0.3)
    rest = Pose(hip=(h[0], h[1] + 8), lean=6.5, head=(3.5, 6.5),
                near_foot=(37, ANKLE), far_foot=(21, ANKLE + 1), far_toe=1.2, far_planted=False,
                near_hand=(42, 45), far_hand=(36, 55), lantern=0.0,
                hem_wave=3.6, hem_amp=0.1, hem_rise=-2.5,
                scarf_angle=92, scarf_curl=0, scarf_amp=1, scarf_phase=3.6,
                hood_tip=(2.5, 3.5), eyes='closed', flame=0.0)
    return [stagger, buckle, kneel, slump, rest], [90, 110, 140, 160, 260]


PLAYER_CLIPS = {
    # clip: (pose and duration source, looping)
    'player_idle': (idle_poses, True),
    'player_walk': (run_poses, True),
    'player_jump': (jump_poses, False),
    'player_fall': (fall_poses, True),
    'player_land': (land_poses, False),
    'player_double_jump': (double_jump_poses, False),
    'player_death': (death_poses, False),
}


def toss_frames():
    """The toss: wind-up, swing, release and follow-through; the ball
    leaves the hand at the release."""
    hip = REST_HIP
    base = dict(lantern=0.15, scarf_angle=132, scarf_curl=-22, scarf_amp=8, hem_amp=0.7)
    wind = Pose(hip=(hip[0] - 1, hip[1] + 1), lean=-0.5, near_hand=(41, 38), far_hand=(15, 44),
                far_foot=(24, ANKLE), near_foot=(36, ANKLE), hem_flare=1.5, **base)
    swing = Pose(hip=hip, lean=1.5, near_hand=(42, 38), far_hand=(31, 45), hem_front=1,
                 far_foot=(25, ANKLE), near_foot=(37, ANKLE), **base)
    release = Pose(hip=(hip[0] + 1, hip[1]), lean=3.0, near_hand=(42, 37), far_hand=(45, 31),
                   hem_front=2, far_foot=(25, ANKLE), near_foot=(38, ANKLE), near_toe=0.2, **base)
    follow = Pose(hip=(hip[0] + 1, hip[1]), lean=2.5, near_hand=(42, 37), far_hand=(47, 25),
                  hem_front=1.5, far_foot=(25, ANKLE), near_foot=(37, ANKLE), near_toe=0.1, **base)
    poses = [wind, swing, release, follow]
    frames = [render_toss(pose, front=i > 0, ball=i < 2) for i, pose in enumerate(poses)]
    return frames, [70, 50, 60, 120]


def aim_frames():
    """The arm and gun at each of [`AIMS`], from straight up to straight
    down; 01 shows the frame itself, so the clip only holds them."""
    return [render_aim(degrees) for degrees in AIMS], [100] * len(AIMS)


# Clips drawn frame by frame, not posed through `render`.
DRAWN_CLIPS = {
    'player_toss': (toss_frames, False),
    'player_aim': (aim_frames, False),
}


def frame_name(clip, index):
    # The idle clip starts on the registry's start sprite, `player`.
    return 'player' if clip == 'player_idle' and index == 0 else f'{clip}_{index}'


def posed(poses_for):
    """A posed clip's frames rendered, with their durations."""
    poses, durations = poses_for()
    return [render(pose) for pose in poses], durations


def check_frame(name, im, glass):
    """Feet on the baseline; the flame window sees all of the glass and nothing else."""
    if im.getbbox()[3] != SOLE + 2:
        raise ValueError(f'{name}: lowest pixel must sit on row {SOLE + 1}')
    x0, y0, x1, y1 = LANTERN_REGION
    glow = (PALETTE['y'], PALETTE['Y'])
    window = {(x, y) for y in range(y0, y1) for x in range(x0, x1) if im.getpixel((x, y)) in glow}
    if not glass or window != glass:
        raise ValueError(f'{name}: lantern glass must sit wholly inside the flame window')


def build_player(images, lit, animations):
    clips = [(clip, posed(poses_for), looping) for clip, (poses_for, looping) in PLAYER_CLIPS.items()]
    clips += [(clip, frames_for(), looping) for clip, (frames_for, looping) in DRAWN_CLIPS.items()]
    for clip, (rendered, durations), looping in clips:
        frames = []
        for i, ((im, glass), duration) in enumerate(zip(rendered, durations, strict=True)):
            name = frame_name(clip, i)
            check_frame(name, im, glass)
            images[name] = im
            lit.add(name)
            frames.append({'sprite': 'ex10_' + name, 'duration_ms': duration})
        animations[clip] = {'looping': looping, 'frames': frames}


def fireball_frame(index):
    """Ball of flame for a right-moving hazard; tongues stream up and behind."""
    t = index / FIREBALL_FRAMES
    cx, cy, radius = 32.0, 35.0, 14.0
    trail = math.atan2(-0.86, -0.5)
    canvas = {}
    for y in range(SIZE):
        for x in range(SIZE):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            r = math.hypot(dx, dy)
            theta = math.atan2(dy, dx)
            behind = max(0.0, math.cos(theta - trail))
            # Travelling ripples; high powers sharpen them into separate tongues.
            wave_a = 0.5 + 0.5 * math.sin(5 * theta - math.tau * 2 * t)
            wave_b = 0.5 + 0.5 * math.sin(8 * theta + math.tau * 3 * t + 1.3)
            tongues = 0.75 * wave_a ** 5 + 0.25 * wave_b ** 6
            envelope = (radius + 1.0 + behind ** 1.3 * (2 + 17 * tongues)
                        + (1 - behind) * 0.9 * wave_b)
            if r > envelope:
                continue
            heat = 1 - r / envelope
            if r < radius:
                # Plasma bands swirl around the core.
                heat += 0.07 * math.sin(3 * theta + 0.4 * r - math.tau * t)
                heat += 0.04 * math.sin(5 * theta - 0.3 * r + math.tau * 2 * t)
            heat += (BAYER[y % 4][x % 4] / 16 - 0.5) * 0.05
            for limit, symbol in ((0.08, 'M'), (0.2, 'L'), (0.37, 'K'), (0.54, 'J'), (0.7, 'Y')):
                if heat < limit:
                    canvas[(x, y)] = symbol
                    break
            else:
                canvas[(x, y)] = 'Z'
    # Loose embers peel off the trailing side and cool as they climb.
    for k in range(5):
        age = (t + k * 0.21) % 1
        angle = trail + (k - 2) * 0.32
        distance = radius + 14 + age * 11
        ex, ey = point((cx + math.cos(angle) * distance, cy + math.sin(angle) * distance - age * 3))
        if 1 <= ex < SIZE - 1 and 1 <= ey < SIZE - 1 and (ex, ey) not in canvas:
            canvas[(ex, ey)] = 'J' if age < 0.45 else 'L' if age < 0.8 else 'M'
    return to_image(canvas)


def build_fireball(images, animations):
    # Self-lit: drawn unlit at its authored colours, so no normal/emissive maps.
    frames = []
    for i in range(FIREBALL_FRAMES):
        name = f'fireball_{i}'
        images[name] = fireball_frame(i)
        frames.append({'sprite': 'ex10_' + name, 'duration_ms': 60})
    animations['fireball'] = {'looping': True, 'frames': frames}


BALL_FRAMES = 6
BRONZE = '/(aAyY'  # deep shadow to specular
MARBLE = 'ΦΩψχφ'  # neutral greys: the runtime hue tint colours the whole marble


def sphere(size, radius):
    """(x, y, nx, ny, nz) for each cell of a `size` grid inside a centred sphere."""
    centre = (size - 1) / 2
    for y in range(size):
        for x in range(size):
            nx, ny = (x - centre) / radius, (y - centre) / radius
            if nx * nx + ny * ny <= 1:
                yield x, y, nx, ny, math.sqrt(1 - nx * nx - ny * ny)


def upscale(canvas, scale):
    """Author at render size, then store each cell as a block so nearest sampling stays exact."""
    return {(x * scale + i, y * scale + j): symbol for (x, y), symbol in canvas.items()
            for i in range(scale) for j in range(scale)}


def lambert(nx, ny, nz, light):
    lx, ly, lz = light
    length = math.sqrt(lx * lx + ly * ly + lz * lz)
    return max(0.0, (nx * lx + ny * ly + nz * lz) / length)


def rune_orb(frame):
    """Heavy bronze orb, drawn at its 32-pixel render size. Three glowing meridians
    and three rune rings roll around a tilted axis; three-fold symmetry loops in six frames."""
    spin = frame * (math.tau / 3) / BALL_FRAMES
    tilt = math.radians(22)
    canvas = {}
    for x, y, nx, ny, nz in sphere(32, 13.8):
        # Fill light keeps the shadowed side legible against dark stone.
        light = 0.2 + 0.8 * lambert(nx, ny, nz, (0.45, -0.65, 0.62))
        symbol = BRONZE[min(4, int(light * 4.6))]
        if light > 0.98:
            symbol = 'Y'
        # Surface coordinates on the tilted, spinning ball.
        py = ny * math.cos(tilt) - nz * math.sin(tilt)
        pz = ny * math.sin(tilt) + nz * math.cos(tilt)
        latitude = math.asin(max(-1.0, min(1.0, -py)))
        longitude = math.atan2(nx, pz) + spin
        inlay = None
        for k in range(3):
            meridian = (longitude - k * math.tau / 3 + math.pi) % math.tau - math.pi
            if abs(meridian) * math.cos(latitude) < 0.06 and abs(latitude) < 1.2:
                inlay = 'c'
            ring = math.acos(max(-1.0, min(1.0, math.cos(latitude) * math.cos(meridian - math.pi / 3))))
            if 0.2 < ring < 0.3:
                inlay = 'c'
            elif ring < 0.08:
                inlay = 'C'
        if inlay:
            symbol = inlay if nz > 0.28 else 'D'  # inlays sink into shadow at the limb
        canvas[(x, y)] = symbol
    return to_image(upscale(outline(canvas, size=(32, 32)), 2))


def glass_marble(frame):
    """Glass marble, drawn at its 16-pixel render size in neutral greys. A cat's-eye
    ribbon turns inside it; the ribbon is two-fold, so half a turn loops in six frames."""
    spin = frame * math.pi / BALL_FRAMES
    canvas = {}
    for x, y, nx, ny, nz in sphere(16, 6.4):
        light = 0.35 + 0.65 * lambert(nx, ny, nz, (-0.5, -0.6, 0.62))
        tone = min(4, int(light * 4.4))
        if nx * nx + ny * ny > 0.78:
            tone = 1  # a thin darker rim keeps the round silhouette
        u = nx * math.cos(spin) + ny * math.sin(spin)
        v = -nx * math.sin(spin) + ny * math.cos(spin)
        if abs(u - 0.34 * math.sin(2.4 * v)) < 0.15 and nx * nx + ny * ny <= 0.78:
            tone = min(4, tone + 1)
        canvas[(x, y)] = MARBLE[tone]
    canvas[(5, 5)] = canvas[(6, 5)] = 'Ψ'  # a fixed glint: the light does not spin
    return to_image(upscale(outline(canvas, 'Φ', size=(16, 16)), 4))


BOWLING = 'θνγβαδ'  # resin from its darkest hole to its lit crown


def bowling_ball(frame):
    """Heavy resin bowling ball, drawn at its 32-pixel render size: a dark slate
    shell with a paler swirl and three finger holes that roll over its face; the
    holes are three-fold, so a third of a turn loops in six frames."""
    spin = frame * (math.tau / 3) / BALL_FRAMES
    canvas = {}
    for x, y, nx, ny, nz in sphere(32, 13.8):
        light = 0.18 + 0.82 * lambert(nx, ny, nz, (0.4, -0.7, 0.6))
        tone = min(5, 1 + int(light * 4.6))
        # The swirl: a soft band of paler resin across the shell.
        if abs(math.sin(3.1 * nx + 2.2 * ny + spin * 2)) < 0.16 and tone < 5:
            tone += 1
        canvas[(x, y)] = BOWLING[tone]
        # Three holes on a small triangle that turns with the roll.
        for k in range(3):
            angle = spin + k * math.tau / 3
            hx, hy = 0.34 * math.cos(angle), -0.12 + 0.3 * math.sin(angle)
            if (nx - hx) ** 2 + (ny - hy) ** 2 < 0.012 and nz > 0.35:
                canvas[(x, y)] = 'θ'
    canvas[(11, 9)] = canvas[(12, 9)] = canvas[(11, 10)] = 'π'  # the shell's gloss
    return to_image(upscale(outline(canvas, size=(32, 32)), 2))


def build_balls(images, lit, animations):
    for clip, first, draw, duration in (('ball_spin', 'ball', rune_orb, 90),
                                        ('ball_small_spin', 'ball_small', glass_marble, 65),
                                        ('bowling_roll', 'bowling_ball', bowling_ball, 80)):
        names = [first if i == 0 else f'{first}_{i}' for i in range(BALL_FRAMES)]
        for i, name in enumerate(names):
            images[name] = draw(i)
            lit.add(name)
        animations[clip] = {'looping': True, 'frames': [
            {'sprite': 'ex10_' + name, 'duration_ms': duration} for name in names]}


# The training dummy stands on the courier's baseline: its base's lowest
# drawn row is the courier's soles', so the outline ends both boxes at 61.
DUMMY_FEET = SOLE + 2


def dummy_canvas():
    """A burlap sack stuffed with straw on a wooden post and crossbar: a red
    target on the chest, rope at the neck and the waist, straw at the seams,
    stitched eyes and mouth. Drawn unlit, so the kit's hit flash shows on it."""
    feet = SOLE
    base = Part()
    base.polygon([(18, feet), (46, feet), (42, feet - 4), (22, feet - 4)])
    post = Part()
    post.rectangle((30, 24, 33, feet - 4))
    bar = Part()
    bar.rectangle((12, 27, 51, 30))
    body = Part()
    body.ellipse((32, 36), 11, 13)
    head = Part()
    head.ellipse((32, 15), 8, 8)
    canvas = compose([
        (shade(base.pixels(), '(', light='a', shadow='ω'), 'o'),
        (shade(post.pixels(), "'", light='a', shadow='('), 'o'),
        (shade(bar.pixels(), "'", light='a', shadow='('), 'o'),
        (shade(body.pixels(), 'τ', light='σ', shadow='s', rim='s'), 'o'),
        (shade(head.pixels(), 'τ', light='σ', shadow='s'), 'o'),
    ])
    # Straw at the bar's ends and the neck, hashed per tuft.
    for x0, y0, side in ((11, 28, -1), (52, 28, 1)):
        for i in range(5):
            dx = side * (1 + int(unit('straw', x0, i) * 3))
            dy = int(unit('straw-y', x0, i) * 5) - 2
            for t in range(abs(dx) + 1):
                canvas[(x0 + side * t, y0 + dy)] = 'y' if (i + t) % 2 else 'A'
    for i, x in enumerate(range(27, 38, 2)):
        canvas[(x, 23 - (i % 2))] = 'y'
    # Rope at the neck and the waist.
    for y, xs in ((23, range(25, 40)), (44, range(21, 44))):
        for x in xs:
            if (x, y) in canvas:
                canvas[(x, y)] = 'a' if x % 2 else "'"
    # The target: bone and red rings.
    for y in range(28, 43):
        for x in range(25, 40):
            r = math.hypot(x - 32, y - 35)
            if r <= 6.2 and (x, y) in canvas:
                canvas[(x, y)] = 'E' if r > 4.4 or r <= 1.6 else 'I'
    # Stitched eyes, crosses, and a stitched mouth.
    for x, y in ((29, 13), (30, 14), (29, 15), (31, 13), (31, 15),
                 (34, 13), (35, 14), (34, 15), (36, 13), (36, 15)):
        canvas[(x, y)] = 'o'
    for x in range(29, 36):
        canvas[(x, 19)] = 's' if x % 2 else 'o'
    return outline(despeckle(canvas))


def build_dummy(images):
    """The training dummy's one frame, unlit: no normal or emissive map."""
    im = to_image(dummy_canvas())
    if im.getbbox()[3] != DUMMY_FEET:
        raise ValueError(f'dummy: lowest pixel must sit on row {SOLE + 1}')
    images['dummy'] = im


# ------------------------------------------------------------ the bestiary
# 01 (c)'s five enemies, from the art-review sheet the owner reviewed on
# 2026-10-21 (M75's plan). Each is posed per frame from one rig and drawn
# unlit, with no normal or emissive map, so the kit's hit flash shows
# through `damage_flash`; the moon catching their top and right edges and
# their glowing eyes are painted into their colours. The walkers keep their
# feet on a fixed row: the crawler's and the beetle's lowest pixel on row 60,
# as the courier's is, the troll's on row 124 of its 128 frame.

BIG = 128
BIG_SOLE = 123  # the troll's lowest drawn row; the outline adds row 124


def strand(points, widths):
    """A polygon along `points`, each point its own width wide: a limb with
    its muscles' swell, or a horn's taper."""
    left, right = [], []
    n = len(points)
    for i, (x, y) in enumerate(points):
        a, b = points[max(0, i - 1)], points[min(n - 1, i + 1)]
        dx, dy = b[0] - a[0], b[1] - a[1]
        length = math.hypot(dx, dy) or 1.0
        nx, ny = -dy / length, dx / length
        w = widths[i] / 2
        left.append((x + nx * w, y + ny * w))
        right.append((x - nx * w, y - ny * w))
    return left + right[::-1]


def speckle(canvas, region, keys, symbols, chance, only=None):
    """Scatters `symbols` over `region`'s pixels already in `canvas`."""
    for (x, y) in sorted(region):
        here = canvas.get((x, y))
        if here and here != 'o' and (only is None or here in only) and unit(keys, x, y) < chance:
            canvas[(x, y)] = symbols[int(unit(keys, 'pick', x, y) * len(symbols))]


def turned(points, pivot, angle):
    """`points` turned by `angle` radians about `pivot`: a rig's limb, not a
    raster, so the pixels stay crisp."""
    cos, sin = math.cos(angle), math.sin(angle)
    px, py = pivot
    return [(px + (x - px) * cos - (y - py) * sin, py + (x - px) * sin + (y - py) * cos)
            for x, y in points]


# The wisp: a round flame with a face, self-lit like the fireball hazards.

WISP_FRAMES = 8


def wisp_canvas(t=0.0, flare=False, bob=0.0):
    """A round flame with a face; its tongues stream up and behind, shorter
    than the fireball's, longer and wider in a flare (its alert)."""
    cx, cy = 32.0, 34.0 + bob
    radius = 15.0 if flare else 12.0
    reach = 15.0 if flare else 8.0
    trail = math.atan2(-0.95, -0.3)
    canvas = {}
    for y in range(SIZE):
        for x in range(SIZE):
            dx, dy = x + 0.5 - cx, y + 0.5 - cy
            r = math.hypot(dx, dy)
            theta = math.atan2(dy, dx)
            behind = max(0.0, math.cos(theta - trail))
            wave_a = 0.5 + 0.5 * math.sin(5 * theta - math.tau * 2 * t)
            wave_b = 0.5 + 0.5 * math.sin(8 * theta + math.tau * 3 * t + 1.3)
            tongues = 0.75 * wave_a ** 5 + 0.25 * wave_b ** 6
            envelope = radius + 1.0 + behind ** 1.2 * (2 + reach * tongues) + (1 - behind) * 0.6 * wave_b
            if r > envelope:
                continue
            heat = 1 - r / envelope + (BAYER[y % 4][x % 4] / 16 - 0.5) * 0.05
            if r < radius:  # plasma swirling about the core
                heat += 0.05 * math.sin(3 * theta + 0.4 * r - math.tau * t)
            for limit, symbol in ((0.08, 'M'), (0.2, 'L'), (0.37, 'K'), (0.54, 'J'), (0.7, 'Y')):
                if heat < limit:
                    canvas[(x, y)] = symbol
                    break
            else:
                canvas[(x, y)] = 'Z'
    top = round(cy) - 3 - (1 if flare else 0)
    for ex in (37, 44):  # dark eyes with a hot glint; wider in a flare
        for y in range(top, round(cy) + 2):
            for x in (ex, ex + 1) + ((ex + 2,) if flare else ()):
                canvas[(x, y)] = 'M'
        canvas[(ex + 1, top + 1)] = 'Z'
    if flare:  # an open mouth
        for x in range(39, 44):
            for y in (round(cy) + 5, round(cy) + 6):
                canvas[(x, y)] = 'M'
    return canvas


def wisp_float_frames():
    """Its flame flickering, bobbing a pixel."""
    frames = [to_image(wisp_canvas(i / WISP_FRAMES, bob=round(math.sin(i * math.tau / WISP_FRAMES))))
              for i in range(WISP_FRAMES)]
    return frames, [80] * WISP_FRAMES


def wisp_flare_frames():
    """Its alert: the flame flaring wide, eyes and mouth open."""
    return [to_image(wisp_canvas(i / 6, flare=True)) for i in range(6)], [60] * 6


# The crawler: alien and low, its plates and crystals frost-blue.

CRAWLER_FRAMES = 6


def crawler_canvas(phase=0.0, scared=0.0, shiver=0, coil=0.0, stretch=0.0, squash=0.0):
    """Five carapace plates, eight high-kneed jointed legs, a wedge head with
    a cluster of four glowing eyes, mandibles and swept feelers, faceted ice
    crystals growing from its back and a crystal sting; a faint cold glow
    between its plates. `phase` steps its legs in two alternating sets of
    four; `scared` (0 to 1) recoils it from the light: its body drawn back
    over its planted feet and low at the front, its plates raised, its
    feelers swept back flat, its jaws shut and its eyes squinting; `shiver`
    draws its body a pixel further back, a tremble. The lunge in three
    parts: `coil` (0 to 1) winds it up, drawn back and pressed low at the
    front, knees high, sting raised, jaws parting, every eye lit; `stretch`
    (0 to 1) is the leap, the body long and raised, the forelegs reaching
    and the hind legs trailing, jaws wide, feelers swept back, its feet on
    the anchor row like every walker frame's;
    `squash` (0 to 1) is the landing, low and splayed, jaws shut."""
    far_legs, near_legs, plates, head, crystals, feelers = (Part() for _ in range(6))
    posed = scared or coil or stretch or squash
    bob = round(0.6 * abs(math.sin(math.tau * phase))) if not posed else 0
    back = round(3 * scared) + shiver + round(4 * coil) - round(2 * stretch)
    # The body raised in the leap and pressed in a coil and a landing, its
    # plates raised in a scare besides.
    raised = round(3 * stretch) - round(coil) - round(3 * squash)
    lifted = round(2 * scared) + raised
    spread = 2.0 * stretch  # each plate further from the middle: the leap's length

    def tilt(x):  # the front drops in a scare and a coil, rises in the leap
        return (x - 30) * (0.05 * stretch - 0.07 * scared - 0.09 * coil)

    def stride(k):  # (forward offset, lift) of leg set k's foot
        angle = math.tau * (phase + 0.5 * k)
        return 3.0 * math.sin(angle), 2.5 * max(0.0, math.cos(angle))

    still = posed
    centres = [(11 - back - 2 * spread, 51 - bob - lifted), (19 - back - spread, 49 - bob - lifted),
               (28 - back, 48 - bob - lifted), (37 - back + spread, 48 - bob - lifted),
               (45 - back + 2 * spread, 49 - bob - lifted)]
    for i, x in enumerate((10, 20, 30, 40)):
        ahead, lift = stride((i + 1) % 2) if not still else (0.0, 0.0)
        y0 = 50 - bob - tilt(x) - raised
        hip = x - back + spread * (i - 1.5)
        reach = (6 * stretch + 5 * squash) * (1 if i >= 2 else -1)  # forelegs ahead, hind legs back
        knee = ((hip - 4 if i < 2 else hip + 4) + ahead * 0.5 + reach * 0.5,
                39 - bob - tilt(x) - lift * 0.6 - 5 * coil - raised)
        foot = ((x - 8 if i < 2 else x + 7) + ahead + reach, SOLE - 1 - lift)
        far_legs.limb([(hip + 2, y0), (knee[0] + 2, knee[1]), (foot[0] + 2, foot[1])], 2)
    for i, x in enumerate((13, 23, 33, 43)):
        ahead, lift = stride(i % 2) if not still else (0.0, 0.0)
        y0 = 52 - bob - tilt(x) - raised
        hip = x - back + spread * (i - 1.5)
        reach = (7 * stretch + 5 * squash) * (1 if i >= 2 else -1)
        knee = ((hip - 5 if i < 2 else hip + 5) + ahead * 0.5 + reach * 0.5,
                40 - bob - tilt(x) - lift * 0.6 - 5 * coil - raised)
        foot = ((x - 9 if i < 2 else x + 9) + ahead + reach, SOLE - lift)
        near_legs.limb([(hip, y0), knee, foot], 2)
    for cx, cy in centres:
        plates.ellipse((cx, cy - tilt(cx)), 7 + round(squash), 6 + round(scared) - round(squash))
    hx = 53 - round(5 * scared) - shiver - round(2 * coil) + round(4 * stretch)
    hy = round(49 - bob - tilt(53) + scared - raised - 2 * stretch + squash)
    head.polygon([(hx - 6, hy - 5), (hx + 4, hy - 4), (hx + 9, hy), (hx + 4, hy + 4), (hx - 6, hy + 5)])
    gape = 1 - (1 if scared >= 0.5 else 0) + round(coil) + round(3 * stretch) - round(squash)
    for side in (-1, 1):  # mandibles
        head.limb([(hx + 7, hy + side * 2), (hx + 10, hy + side * (2 + gape)), (hx + 9, hy + side * (4 + gape))], 2)
    sweep = round(2 * coil) - round(5 * scared) - round(6 * stretch)
    flat = round(3 * scared) + round(4 * stretch)  # the feelers laid back, or swept by the leap
    feelers.limb([(hx - 1, hy - 4), (hx + 2 + sweep, hy - 12 + flat), (hx - 4 + 2 * sweep, hy - 19 + 2 * flat)], 1)
    feelers.limb([(hx + 2, hy - 4), (hx + 7 + sweep, hy - 11 + flat), (hx + 6 + 2 * sweep, hy - 18 + 2 * flat)], 1)
    for i, (cx, cy) in enumerate(centres[1:4]):
        top = cy - tilt(cx) - 6
        h = 9 if i == 1 else 7
        crystals.polygon([(cx - 3, top + 2), (cx + 2, top + 2), (cx + 1, top - h)])
        crystals.polygon([(cx + 2, top + 2), (cx + 5, top + 1), (cx + 4, top - h + 4)])
    sting = round(4 * scared) + round(7 * coil) - round(2 * stretch)  # raised in a scare and a coil
    crystals.polygon([(6 - back, 50 - bob - tilt(6)), (2 - back + sting // 2, 46 - bob - tilt(2) - sting),
                      (max(0, sting - back), 41 - bob - sting), (4 - back + sting // 2, 47 - bob - tilt(4) - sting // 2)])
    canvas = compose([
        (shade(far_legs.pixels(), '{', shadow='|'), None),
        (shade(feelers.pixels(), '}', light='_'), None),
        (shade(plates.pixels(), '}', light='_', shadow='{', back='|', rim='`'), None),
        (shade(head.pixels(), ')', light='_', shadow='{', back='|', rim='`'), 'o'),
        (shade(near_legs.pixels(), ')', light='}', shadow='{'), 'o'),
    ])
    for (ax, ay), (bx, _) in zip(centres, centres[1:]):  # the cold glow between plates
        sx = (ax + bx) // 2 + 3
        base = round(ay - tilt(ax))
        for y in range(base - 5, base + 5):
            if (sx, y) in canvas and canvas[(sx, y)] != 'o':
                canvas[(sx, y)] = '|' if y % 3 else ','
    for (x, y) in sorted(crystals.pixels()):
        lit = (x + y) % 2 == 0 and unit('facet', x // 2, y // 3) < 0.6
        canvas[(x, y)] = 'Ψ' if lit and unit('glint', x, y) < 0.25 else ('ϱ' if lit else '`')
    if scared >= 0.5:  # the four eyes squinting to a dim slit
        for dx in (1, 2, 3, 4):
            canvas[(hx + dx, hy - 1)] = '`'
    else:
        fierce = coil >= 0.5 or stretch > 0  # every eye lit for the lunge
        for dx, dy in ((2, -2), (4, -1), (1, 0), (3, 1)):  # four glowing eyes
            canvas[(hx + dx, hy + dy)] = 'Ψ' if fierce or (dx, dy) == (4, -1) else '`'
    speckle(canvas, plates.pixels(), 'pore', ['{'], 0.05)
    return outline(canvas)


def crawler_walk_frames():
    """Its legs stepping in two sets of four, its body rising between."""
    frames = [to_image(crawler_canvas(i / CRAWLER_FRAMES)) for i in range(CRAWLER_FRAMES)]
    return frames, [90] * CRAWLER_FRAMES


def crawler_windup_frames():
    """The lunge's tell: it coils over its 0.4 s, then trembles, set."""
    frames = [crawler_canvas(coil=t, shiver=s) for t, s in ((0.35, 0), (0.7, 0), (1.0, 0), (1.0, 1))]
    return [to_image(c) for c in frames], [100, 100, 100, 100]


def crawler_lunge_frames():
    """The leap: it stretches out long and flies, the last frame held."""
    return [to_image(crawler_canvas(stretch=t)) for t in (0.6, 1.0, 1.0)], [60, 80, 400]


def crawler_recover_frames():
    """The landing: low and splayed, then gathering itself."""
    return [to_image(crawler_canvas(squash=t)) for t in (1.0, 0.55, 0.2)], [100, 120, 140]


def crawler_scared_frames():
    """Its scare at the light's edge: recoiled, plates raised, eyes
    squinting, trembling a pixel."""
    return [to_image(crawler_canvas(scared=1.0, shiver=s)) for s in (0, 1, 0, 1)], [90, 70, 90, 70]


# The beetle: a rhinoceros beetle of riveted iron, rusting.

BEETLE_FRAMES = 6
BEETLE_IRON = dict(base='Ω', light='χ', shadow='Φ', rim='ψ', back='B')
IRON_TONES = ('Ω', 'χ', 'Φ', 'ψ', 'B', 'b', 'φ')


def beetle_canvas(pose='walk', phase=0.0):
    """A dome of banded wing cases with rows of punctures, a riveted shield
    with a short horn, the long ridged horn of its head, red eyes, a fanned
    antenna and mouthparts, legs with spines and hooked claws, belly plates,
    scratches, and rust flaking along its seams, rim and rivets. `pose` is
    `walk`, `paw` (a front foot scraping the ground before a charge) or
    `charge` (head down, running); `phase` steps its legs."""
    dip = 3 if pose == 'charge' else 0
    amp = 4.0 if pose == 'charge' else 2.5
    far, near, dome, plate, head, horn, horn2, belly = (Part() for _ in range(8))

    def stride(k):  # (forward offset, lift) of leg set k's foot
        angle = math.tau * (phase + 0.5 * k)
        return amp * math.sin(angle), 2.0 * max(0.0, math.cos(angle))

    bob = round(0.6 * abs(math.sin(math.tau * phase))) if pose != 'paw' else 0
    for i, x in enumerate((17, 31, 43)):  # the far legs: sets B, A, B
        ahead, lift = stride((i + 1) % 2) if pose != 'paw' else (0.0, 0.0)
        far.limb([(x + 2, 52 - bob), (x - 2 + ahead * 0.5, 55 - bob - lift * 0.5),
                  (x - 1 + ahead, SOLE - 2 - lift)], 3)
    legs = []
    # A width-3 limb reaches a row past its points, so its feet sit a row up.
    for i, (hip, knee, foot) in enumerate((((21, 52), (15, 55), (16, SOLE - 1)),
                                          ((33, 53), (31, 56), (34, SOLE - 1)),
                                          ((45, 52), (50, 55), (51, SOLE - 1)))):
        if pose == 'paw' and i == 2:  # the scraping foot: raised, forward, down, back
            reach, lift = ((4, 7), (6, 4), (2, 0), (-3, 0))[int(phase * 4) % 4]
            legs.append([(45, 51), (50 + reach // 2, 50 - lift // 2), (51 + reach, SOLE - 1 - lift)])
            continue
        ahead, lift = stride(i % 2) if pose != 'paw' else (0.0, 0.0)
        legs.append([(hip[0], hip[1] - bob), (knee[0] + ahead * 0.5, knee[1] - bob - lift * 0.5),
                     (foot[0] + ahead, foot[1] - lift)])
    for leg in legs:
        near.limb(leg, 3)
    dome.ellipse((25, 47 - bob), 20, 17)
    dome.draw.rectangle((0, 53 - bob, 63, 63), fill=0)
    dome.rectangle((7, 47 - bob, 43, 53 - bob))
    belly.polygon([(9, 52 - bob), (44, 52 - bob), (42, 55 - bob), (11, 55 - bob)])
    plate.polygon([(39, 35 + dip - bob), (48, 33 + dip - bob), (53, 38 + dip - bob), (53, 49 + dip - bob),
                   (41, 53 - bob)])
    horn2.polygon(strand([(47, 35 + dip - bob), (50, 30 + dip - bob), (52, 27 + dip - bob)], [4, 2.5, 1]))
    hx, hy = 56, 47 + dip - bob
    head.ellipse((hx, hy), 5, 5)
    horn_path = [(hx + 1, hy - 3), (hx + 4, hy - 9), (hx + 6, hy - 16), (hx + 5, hy - 21), (hx + 3, hy - 23)]
    if pose == 'charge':  # the horn levelled at what it charges
        horn_path = [(hx + 2, hy - 2), (hx + 6, hy - 6), (hx + 7, hy - 12), (hx + 7, hy - 17), (hx + 5, hy - 20)]
    horn.polygon(strand(horn_path, [5, 4, 3, 2, 1]))
    canvas = compose([
        (shade(far.pixels(), 'b', shadow='o'), None),
        (shade(belly.pixels(), 'b', light='B', shadow='o'), None),
        (shade(dome.pixels(), **BEETLE_IRON), None),
        (shade(plate.pixels(), 'Φ', light='φ', shadow='b', rim='χ'), 'o'),
        (shade(horn2.pixels(), 'B', light='ψ', shadow='b'), None),
        (shade(head.pixels(), 'B', light='ψ', shadow='b'), 'o'),
        (shade(horn.pixels(), 'Ω', light='χ', shadow='Φ', rim='φ'), 'o'),
        (shade(near.pixels(), 'B', light='Ω', shadow='b'), 'o'),
    ])
    top = 31 - bob
    rivets = []
    for bx in (14, 25, 35):  # bands across the wing cases, rivets along them
        for y in range(top, 53 - bob):
            x = bx + round((y - top) * 0.18)
            if canvas.get((x, y)) in ('Ω', 'χ', 'Φ', 'ψ'):
                canvas[(x, y)] = 'Φ'
                if (y - top) % 6 == 3:
                    canvas[(x + 1, y)] = 'χ'
                    rivets.append((x + 1, y))
    for bx in (19, 30):  # rows of punctures between the bands
        for y in range(top + 3, 50 - bob, 3):
            x = bx + round((y - top) * 0.18)
            if canvas.get((x, y)) == 'Ω':
                canvas[(x, y)] = 'Φ'
    for x in range(8, 44):  # the cases' lower rim
        if canvas.get((x, 50 - bob), 'o') != 'o':
            canvas[(x, 50 - bob)] = 'ψ' if x % 2 else 'Φ'
    for x in range(12, 43, 5):  # belly plates
        for y in (53 - bob, 54 - bob):
            if canvas.get((x, y)) in ('b', 'B'):
                canvas[(x, y)] = 'o'
    for x, y in ((44, 39 + dip - bob), (48, 37 + dip - bob), (50, 43 + dip - bob)):  # the shield's rivets
        if (x, y) in canvas:
            canvas[(x, y)] = 'χ'
    for x, y in horn_path[1:4]:  # ridges along the horn
        if (x, y) in canvas:
            canvas[(x, y)] = 'Φ'
    for x0, y0 in ((12, 38 - bob), (22, 35 - bob), (33, 41 - bob)):  # scratches
        for t in range(3):
            if canvas.get((x0 + t, y0 + t)) in IRON_TONES:
                canvas[(x0 + t, y0 + t)] = 'ψ'
    seams = {(bx + round((y - top) * 0.18), y) for bx in (14, 25, 35) for y in range(top, 53 - bob)}
    for (x, y), symbol in sorted(canvas.items()):  # rust along the seams and the rim
        if symbol not in IRON_TONES:
            continue
        noise = (unit('rust-a', x // 3, (y + bob) // 3) + unit('rust-b', x // 2, (y + bob) // 2)) / 2
        near_seam = any((x + dx, y) in seams for dx in (-1, 0, 1)) or 47 - bob <= y <= 51 - bob
        if noise + (0.22 if near_seam else 0.0) > 0.8:
            canvas[(x, y)] = 'a' if noise > 0.86 else ('(' if (x + y + bob) % 2 else '"')
    for x, y in rivets[::2]:  # streaks running down from rivets
        for t in range(1, 4):
            if canvas.get((x, y + t), 'o') != 'o':
                canvas[(x, y + t)] = '(' if t < 3 else '"'
    for leg in legs:  # spines along the shins, hooked claws
        (kx, ky), (fx, fy) = leg[1], leg[2]
        for t in (0.35, 0.65):
            canvas[(round(kx + (fx - kx) * t) + 2, round(ky + (fy - ky) * t))] = 'ψ'
        canvas[(round(fx) + 2, round(fy))] = 'B'
    canvas[(hx + 6, hy + 1)] = 'b'  # mouthparts
    canvas[(hx + 6, hy + 2)] = 'B'
    for x, y in ((hx + 4, hy - 1), (hx + 6, hy - 2), (hx + 7, hy - 3), (hx + 7, hy - 1), (hx + 8, hy - 2)):
        canvas[(x, y)] = 'Φ' if x < hx + 7 else 'ψ'  # a fanned antenna
    for (ex, ey), tone in (((hx + 1, hy - 2), 'z'), ((hx + 2, hy - 2), 'F'), ((hx + 1, hy - 1), 'F'),
                           ((hx + 2, hy - 1), 'F'), ((hx + 1, hy), 'E'), ((hx + 2, hy), 'z')):
        canvas[(ex, ey)] = tone  # red eyes, glowing
    canvas[(hx + 2, hy - 2)] = 'Ψ'
    canvas[(hx + 3, hy - 1)] = 'E'
    if pose == 'charge':  # dust kicked up behind
        for k, (dx, dy) in enumerate(((-2, 0), (-5, -2), (-8, 1), (-4, -3), (-11, -1))):
            if (k + int(phase * 4)) % 2 == 0 or k < 2:
                canvas[(6 + dx, SOLE - 1 + dy)] = 'σ'
    elif pose == 'paw' and int(phase * 4) % 4 >= 2:  # dust from the scrape
        for dx, dy in ((-1, -1), (-4, -2), (-6, 0)):
            canvas[(50 + dx, SOLE - 1 + dy)] = 'σ'
    return outline(canvas)


def beetle_walk_frames():
    """Its legs stepping in two alternating sets of three."""
    frames = [to_image(beetle_canvas('walk', i / BEETLE_FRAMES)) for i in range(BEETLE_FRAMES)]
    return frames, [110] * BEETLE_FRAMES


def beetle_paw_frames():
    """The charge's tell: a front foot pawing the ground, raising dust."""
    return [to_image(beetle_canvas('paw', i / 4)) for i in range(4)], [100] * 4


def beetle_charge_frames():
    """The charge: head down, horn levelled, legs churning, dust behind."""
    return [to_image(beetle_canvas('charge', i / 4)) for i in range(4)], [60] * 4


# The bat: near-black fur, dark purple wings with their bones showing.

BAT_FUR = dict(base='[', light=']', shadow='θ', rim='v')
BAT_WING = dict(base='Δ', light='Π', shadow='Γ', rim='Λ')
BAT_FLAPS = 6


def scallop(left, right, low, high, points=5):
    """A wing's trailing edge from `left` to `right`, dipping to `low` and
    rising to `high` between its finger tips."""
    edge = []
    for i in range(points * 2 + 1):
        u = i / (points * 2)
        x = left[0] + (right[0] - left[0]) * u
        y = (low if i % 2 == 0 else high) + (left[1] - right[1]) * (1 - u) * 0.2
        edge.append((x, y))
    return edge


def bat_canvas(pose='fly', flap=0.0, flash=None, sway=0.0):
    """A near-black furred body and dark purple wings with their finger
    bones showing, red eyes and small fangs. `pose` is `fly` (its wings at
    `flap` radians above level), `roost` (hanging by its feet, swaying by
    `sway` radians) or `dive` (wings swept back); `flash` outlines it in
    that symbol, the tell before its dive."""
    wing, fur, head = Part(), Part(), Part()
    bones = []
    if pose == 'roost':
        pivot = (32, 1)
        fur.limb(turned([(30, 1), (31, 6)], pivot, sway), 2)
        fur.limb(turned([(34, 1), (33, 6)], pivot, sway), 2)
        (cx, cy), = turned([(32, 17)], pivot, sway)
        wing.ellipse((cx, cy), 9, 12)
        wing.polygon(turned([(23, 10), (41, 10), (37, 30), (27, 30)], pivot, sway))
        (hx, hy), = turned([(32, 31)], pivot, sway)
        head.ellipse((hx, hy), 5, 4)
        head.polygon(turned([(28, 33), (30, 38), (31, 33)], pivot, sway))
        head.polygon(turned([(33, 33), (34, 38), (36, 33)], pivot, sway))
        eyes = [point(p) for p in turned([(30, 30), (34, 30)], pivot, sway)]
        fangs = [point(p) for p in turned([(31, 28), (33, 28)], pivot, sway)]
        bones = [turned([(26, 9), (27, 29)], pivot, sway), turned([(38, 9), (37, 29)], pivot, sway)]
    elif pose == 'dive':
        fur.ellipse((38, 38), 7, 5)
        head.ellipse((47, 44), 5, 4)
        head.polygon([(44, 40), (45, 34), (48, 40)])
        upper = [(36, 33), (8, 12), (14, 20), (10, 23), (18, 26), (14, 30), (24, 31), (34, 37)]
        lower = [(34, 41), (6, 34), (13, 38), (9, 42), (18, 43), (15, 47), (26, 45), (36, 43)]
        wing.polygon(turned(upper, (36, 35), flap))
        wing.polygon(turned(lower, (35, 42), -flap))
        eyes, fangs = [(49, 44)], [(50, 47)]
        bones = [turned([(34, 34), (9, 13)], (36, 35), flap), turned([(33, 36), (15, 29)], (36, 35), flap),
                 turned([(34, 42), (7, 35)], (35, 42), -flap), turned([(34, 43), (16, 46)], (35, 42), -flap)]
    else:
        lift = round(-1.5 * math.sin(flap))  # the body rises as the wings beat down
        fur.ellipse((32, 30 + lift), 5, 8)
        fur.limb([(30, 37 + lift), (29, 42 + lift)], 2)
        fur.limb([(34, 37 + lift), (35, 42 + lift)], 2)
        head.ellipse((32, 21 + lift), 5, 4)
        head.polygon([(28, 19 + lift), (29, 12 + lift), (31, 18 + lift)])
        head.polygon([(33, 18 + lift), (35, 12 + lift), (36, 19 + lift)])
        for side in (-1, 1):
            root, tip = (32 + side * 3, 25 + lift), (32 + side * 29, 15 + lift)
            edge = scallop(tip, (32 + side * 4, 33 + lift), 33 + lift, 27 + lift)
            elbow = (32 + side * 18, 14 + lift)
            fold = 1 - 0.15 * max(0.0, -flap)  # the wing folds a little as it beats down
            tip, elbow = ((root[0] + (x - root[0]) * fold, y) for x, y in (tip, elbow))
            edge = [(root[0] + (x - root[0]) * fold, y) for x, y in edge]
            outline_points = turned([root, (root[0] + (side * 15) * fold, 13 + lift), tip] + edge, root,
                                    -side * flap)
            wing.polygon(outline_points)
            elbow, tip = turned([elbow, tip], root, -side * flap)
            edge = turned(edge, root, -side * flap)
            bones.append([root, elbow, tip])
            for k in (2, 4, 6):  # fingers fanning to the trailing spikes
                bones.append([elbow, edge[k]])
        eyes, fangs = [(30, 21 + lift), (34, 21 + lift)], [(31, 24 + lift), (33, 24 + lift)]
    canvas = compose([(shade(wing.pixels(), **BAT_WING), None),
                      (shade(fur.pixels(), **BAT_FUR), 'o'),
                      (shade(head.pixels(), **BAT_FUR), 'o')])
    wing_px = wing.pixels()
    for path in bones:  # finger bones across the membrane
        line = Part()
        line.limb(path, 1)
        for p in sorted(line.pixels()):
            if p in wing_px and canvas.get(p) in ('Δ', 'Γ', 'Π'):
                canvas[p] = 'Λ' if (p[0] + p[1]) % 3 else 'Π'
    speckle(canvas, wing_px, 'membrane', ['Γ'], 0.12, only=('Δ',))
    for eye in eyes:
        canvas[eye] = 'F'
    for fang in fangs:
        canvas[fang] = 'w'
    return outline(canvas, flash or 'o')


def bat_roost_frames():
    """Hanging from a ceiling by its feet, swaying, wings wrapped."""
    return [to_image(bat_canvas('roost', sway=a)) for a in (0.0, 0.05, 0.0, -0.05)], [220] * 4


def bat_fly_frames():
    """Its wings beating."""
    flaps = [0.55 * math.sin(i * math.tau / BAT_FLAPS) for i in range(BAT_FLAPS)]
    return [to_image(bat_canvas('fly', flap)) for flap in flaps], [70] * BAT_FLAPS


def bat_windup_frames():
    """The dive's tell: wings raised and its outline flashing white."""
    poses = ((0.2, 'Ψ'), (0.4, 'x'), (0.55, 'Ψ'), (0.55, 'x'))
    return [to_image(bat_canvas('fly', flap, flash)) for flap, flash in poses], [60] * 4


def bat_dive_frames():
    """The dive: wings swept back, fluttering."""
    return [to_image(bat_canvas('dive', flap)) for flap in (0.0, 0.07)], [60] * 2


# The troll: lanky on a 128 frame, no hump, built on a skeleton.

TROLL_MOSS = dict(base='g', light='W', shadow='T', rim='m', back='N')
TROLL_MOSS_FAR = dict(base='T', light='g', shadow='N', rim='G')
TROLL_HIDE = dict(base='(', light='a', shadow='[', rim="'")
TROLL_THIGH, TROLL_SHIN = 23.0, 19.0
TROLL_WALK = 8
# Each hand in its forearm's frame, wrist at the origin and fingers along +y:
# its palm, its fingers (each a bone chain) and, on the near hand, a thumb.
NEAR_PALM = [(-6, -1), (5, -1), (7, 9), (-7, 10)]
NEAR_FINGERS = [[(-6, 8), (-7, 15), (-6, 19)], [(-2, 9), (-2, 17), (-1, 21)],
                [(2, 9), (3, 17), (3, 21)], [(6, 8), (7, 15), (6, 18)], [(5, 1), (9, 5), (9, 9)]]
FAR_PALM = [(-5, 0), (5, 0), (6, 9), (-6, 10)]
FAR_FINGERS = [[(-4, 9), (-5, 16)], [(0, 10), (0, 18)], [(4, 9), (5, 15)]]
# A fist, for a hand raised overhead or swung: the fingers curled under.
FIST = [(-6, -1), (6, -1), (7, 6), (4, 9), (-4, 9), (-7, 6)]
FIST_KNUCKLES = [(-4, 8), (-1, 9), (2, 9), (5, 7)]
NEAR_FOOT = [(-6, -1), (5, -1), (13, 2), (15, 6), (-7, 6)]
FAR_FOOT = [(-6, -1), (3, -1), (10, 2), (11, 6), (-7, 6)]


@dataclass
class TrollPose:
    """One frame of the troll, facing right. Positions are frame pixels."""
    crouch: float = 0.0  # the hips drop this far: a slam's or a pickup's bend
    lean: float = 0.0  # the chest ahead of the hips; negative leans back
    breath: float = 0.0  # the chest's rise
    lift: float = 0.0  # the head's rise
    near_ankle: tuple = (72.0, 117.0)
    far_ankle: tuple = (52.0, 117.0)
    near_toe: float = 0.0  # radians a raised foot's toes point down
    far_toe: float = 0.0
    near_elbow: tuple = (91.0, 68.0)
    near_wrist: tuple = (94.0, 91.0)
    far_elbow: tuple = (52.0, 67.0)
    far_wrist: tuple = (52.0, 89.0)
    hands: str = 'open'  # open, fists, or clasped overhead (both hands as one)
    sway: float = 0.0  # the hide's front flap swings this far
    dust: bool = False  # a slam's impact kicks up dust at the hands


def in_hand(wrist, elbow, points):
    """`points` from a hand's own frame to the frame: the wrist at the
    origin, the fingers along the forearm's direction."""
    angle = math.atan2(wrist[1] - elbow[1], wrist[0] - elbow[0]) - math.pi / 2
    return turned([(wrist[0] + x, wrist[1] + y) for x, y in points], wrist, angle)


def troll_canvas(pose):
    """Lanky, no hump, on a skeleton: a thick neck that flows from the upper
    back into the skull, rounded shoulders, arms with a swell above a bony
    elbow and forearms thickening to big long-fingered hands, a lean chest
    over a narrow waist, long legs with bony knees and calves, broad clawed
    feet. A long head thrust forward: a jutting brow, a deep-set ember eye,
    a hooked nose, an underbite with tusks, a pointed ear with a ring, mossy
    hair down the neck. A hide loincloth on a strap that follows the waist,
    its front flap folded with a ragged hem, its back flap behind the far
    thigh, a bone at the knot; a scar, ribs, warts, nails."""
    size = (BIG, BIG)
    names = ('far_leg', 'far_foot', 'far_arm', 'far_hand', 'back_flap', 'glute', 'torso',
             'far_shoulder', 'neck', 'near_leg', 'near_foot', 'strap', 'front_flap', 'head', 'brow',
             'nose', 'jaw', 'tusks', 'ear', 'near_shoulder', 'near_arm', 'near_hand')
    P = {name: Part(size) for name in names}
    c, lean, up = pose.crouch, pose.lean, pose.breath

    def chest(x, y):  # a point on the trunk: it leans and breathes above the waist
        share = max(0.0, min(1.0, (76 - y) / 40))
        return (x + lean * share, y + c - up * share)

    # Legs: thigh, a bony knee, a swelling calf, broad feet with toes.
    ankles = {}
    for side, hip, ankle, toe, foot, widths in (
            ('far', (59, 76 + c), pose.far_ankle, pose.far_toe, FAR_FOOT, ([13, 11, 8], [8, 9, 6], 4)),
            ('near', (69, 77 + c), pose.near_ankle, pose.near_toe, NEAR_FOOT, ([14, 12, 9], [9, 11, 7], 5))):
        knee, end = two_bone(hip, ankle, TROLL_THIGH, TROLL_SHIN, 1)
        thigh, shin, knob = widths
        P[side + '_leg'].polygon(strand([hip, ((hip[0] + knee[0]) / 2, (hip[1] + knee[1]) / 2), knee], thigh))
        P[side + '_leg'].ellipse(knee, knob, knob)
        P[side + '_leg'].polygon(strand([knee, ((knee[0] + end[0]) / 2, (knee[1] + end[1]) / 2), end], shin))
        P[side + '_foot'].polygon(turned([(end[0] + x, end[1] + y) for x, y in foot], end, toe))
        ankles[side] = end
    # The trunk: a lean chest over a narrow waist, the glutes behind the hide.
    P['glute'].ellipse((63, 75 + c), 11, 6)
    P['torso'].polygon([chest(66, 36), chest(85, 43), chest(87, 52), chest(84, 60), chest(80, 68),
                        chest(77, 76), chest(60, 76), chest(58, 62), chest(60, 48)])
    far_shoulder, near_shoulder = chest(66, 42), chest(82, 45)
    P['far_shoulder'].ellipse(far_shoulder, 6, 7)
    P['near_shoulder'].ellipse(near_shoulder, 7, 8)
    # The neck flows from the upper back into the skull's base, thick; the
    # head drops as the chest leans past the hips.
    hx = 90 + lean
    hy = 24 + c - up - pose.lift + max(0.0, lean) * 0.8
    back, front = chest(66, 36), chest(85, 43)
    P['neck'].polygon([back, (hx - 8, hy - 6), (hx, hy + 2), (hx + 3, hy + 10), front, chest(72, 42)])
    P['head'].ellipse((hx, hy), 10, 11)
    P['brow'].polygon([(hx - 3, hy - 9), (hx + 9, hy - 8), (hx + 10, hy - 4), (hx + 1, hy - 5)])
    P['nose'].polygon([(hx + 6, hy - 6), (hx + 15, hy), (hx + 14, hy + 5), (hx + 9, hy + 6), (hx + 7, hy + 1)])
    P['jaw'].polygon([(hx - 6, hy + 4), (hx + 10, hy + 6), (hx + 9, hy + 11), (hx - 3, hy + 13)])
    P['tusks'].polygon([(hx + 3, hy + 6), (hx + 4, hy + 1), (hx + 6, hy + 6)])
    P['tusks'].polygon([(hx + 7, hy + 6), (hx + 8, hy + 2), (hx + 10, hy + 6)])
    P['ear'].polygon([(hx - 6, hy - 6), (hx - 18, hy - 14), (hx - 8, hy + 3)])
    # The hide: a strap along the waist, the front flap, the back flap.
    w = c
    P['strap'].polygon(strand([(53, 73 + w), (66, 75 + w), (81, 74 + w)], [3, 3, 3]))
    s = pose.sway
    P['front_flap'].polygon([(58, 75 + w), (81, 74 + w), (82 + s, 83 + w), (80 + s, 92 + w), (77 + s, 88 + w),
                             (74 + s, 95 + w), (71 + s, 89 + w), (67 + s, 94 + w), (64 + s, 89 + w),
                             (60 + s, 93 + w), (57, 84 + w)])
    P['back_flap'].polygon([(51, 73 + w), (59, 75 + w), (58 - s, 87 + w), (55 - s, 91 + w), (51 - s, 86 + w)])
    # Arms: a shoulder, a swelling upper arm, a bony elbow, a forearm that
    # thickens to the hand; each hand turned with its forearm. The elbows
    # and wrists are the pose's own joints, so a hanging arm keeps the
    # sheet's bend and the far one hangs behind the far leg.
    hands = {}
    for side, root, elbow, end, upper, fore, palm, fingers in (
            ('far', chest(64, 43), pose.far_elbow, pose.far_wrist, [10, 9, 7], [7, 10, 8], FAR_PALM,
             FAR_FINGERS),
            ('near', chest(83, 46), pose.near_elbow, pose.near_wrist, [12, 10, 8], [8, 11, 9], NEAR_PALM,
             NEAR_FINGERS)):
        arm = P[side + '_arm']
        arm.polygon(strand([root, ((root[0] + elbow[0]) / 2, (root[1] + elbow[1]) / 2), elbow], upper))
        arm.ellipse(elbow, 4, 4)
        arm.polygon(strand([elbow, ((elbow[0] + end[0]) / 2, (elbow[1] + end[1]) / 2), end], fore))
        hand = P[side + '_hand']
        if pose.hands == 'fists':
            hand.polygon(in_hand(end, elbow, FIST))
        elif pose.hands == 'open':
            hand.polygon(in_hand(end, elbow, palm))
            for bones in fingers:
                hand.limb(in_hand(end, elbow, bones), 3)
        hands[side] = (end, elbow, pose.hands != 'open')
    if pose.hands == 'clasped':  # both hands locked together over the head
        (nx, ny), (fx, fy) = pose.near_wrist, pose.far_wrist
        P['near_hand'].ellipse(((nx + fx) / 2, min(ny, fy) - 1), 10, 4)
    canvas = compose([
        (shade(P['far_arm'].pixels(), **TROLL_MOSS_FAR), None),
        (shade(P['far_hand'].pixels(), **TROLL_MOSS_FAR), None),
        (shade(P['far_leg'].pixels(), **TROLL_MOSS_FAR), 'o'),
        (shade(P['far_foot'].pixels(), **TROLL_MOSS_FAR), None),
        (shade(P['back_flap'].pixels(), '[', light='(', shadow='o'), None),
        (shade(P['glute'].pixels(), **TROLL_MOSS), None),
        (shade(P['far_shoulder'].pixels(), **TROLL_MOSS_FAR), None),
        (shade(P['torso'].pixels(), **TROLL_MOSS), None),
        (shade(P['neck'].pixels(), **TROLL_MOSS), None),
        (shade(P['near_leg'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['near_foot'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['front_flap'].pixels(), **TROLL_HIDE), 'o'),
        (shade(P['strap'].pixels(), '"', light="'", shadow='('), None),
        (shade(P['ear'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['head'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['jaw'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['brow'].pixels(), 'T', light='g', shadow='N', rim='G'), None),
        (shade(P['nose'].pixels(), 'G', light='W', shadow='T', rim='m'), 'o'),
        (shade(P['tusks'].pixels(), 'w', light='Z', shadow='σ'), None),
        (shade(P['near_shoulder'].pixels(), **TROLL_MOSS), None),
        (shade(P['near_arm'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['near_hand'].pixels(), **TROLL_MOSS), 'o'),
    ])
    skin = set().union(*(P[n].pixels() for n in ('torso', 'near_leg', 'near_arm', 'head', 'neck', 'near_shoulder')))
    speckle(canvas, skin, 'wart', ['T'], 0.025, only=('g',))
    hx, hy = round(hx), round(hy)
    for x, y in ((hx + 3, hy - 3), (hx + 4, hy - 4), (hx + 7, hy - 3), (hx + 4, hy - 2)):
        canvas[(x, y)] = 'N'  # the eye's socket under the brow
    canvas[(hx + 5, hy - 3)] = 'J'  # a deep-set ember eye
    canvas[(hx + 6, hy - 3)] = 'K'
    canvas[(hx + 12, hy + 4)] = 'N'  # a nostril
    canvas[(hx + 13, hy + 4)] = 'T'
    for t in range(4):  # the ear's inside
        canvas[(hx - 9 - t * 2, hy - 7 - t * 2)] = 'T'
        canvas[(hx - 8 - t * 2, hy - 6 - t * 2)] = 'T'
    for x in range(hx - 2, hx + 9):  # the mouth's line under the tusks
        if canvas.get((x, hy + 6)) in ('g', 'W', 'm', 'T'):
            canvas[(x, hy + 6)] = 'N'
    for dx, dy in ((0, 0), (1, 1), (0, 2), (-1, 1)):  # a ring in the ear
        canvas[(hx - 10 + dx, hy - 6 + dy)] = 'A'
    for i in range(8):  # mossy hair from the scalp down the back of the neck
        x, y = hx - 2 - i * 3, hy - 9 + i * 3 + (1 if i % 2 else 0)
        for t in range(4):
            canvas[(x - t // 2, y - t)] = 'D' if t < 2 else ('Q' if t < 3 else 'U')
            canvas[(x - t // 2 + 1, y - t)] = 'D' if t < 3 else 'Q'
    trunk = P['torso'].pixels()
    for t in range(8):  # a scar across the chest, stitched
        x, y = point(chest(75 + t, 49 + t))
        if (x, y) in trunk and canvas.get((x, y)) in ('g', 'W', 'T', 'm'):
            canvas[(x, y)] = 'm'
            if t % 3 == 1 and canvas.get((x - 1, y + 1)) in ('g', 'T'):
                canvas[(x - 1, y + 1)] = 'W'
    for i in range(3):  # ribs along the flank
        for t in range(6):
            x, y = point(chest(64 + t, 54 + i * 5 + round(math.sin(t / 2) * 1.0)))
            if (x, y) in trunk and canvas.get((x, y)) in ('g', 'W'):
                canvas[(x, y)] = 'T'
    navel = point(chest(78, 67))
    if canvas.get(navel) in ('g', 'W'):
        canvas[navel] = 'T'
    near_hand, near_elbow, near_fist = hands['near']
    knuckles = FIST_KNUCKLES if near_fist else [(-6, 9), (-2, 9), (2, 9), (6, 9)]
    for x, y in in_hand(near_hand, near_elbow, knuckles):
        if canvas.get(point((x, y))) in ('g', 'T', 'm'):
            canvas[point((x, y))] = 'W'  # knuckles
    if not near_fist:
        for bones in NEAR_FINGERS:  # nails at the finger tips
            tip = point(in_hand(near_hand, near_elbow, [bones[-1]])[0])
            if tip in canvas:
                canvas[tip] = 'w'
    folds = ((63, 79), (69, 78), (75, 78))
    for x, y in folds:  # folds in the front flap
        for t in range(8):
            if canvas.get((x, round(y + c + t))) in ('(', 'a', "'"):
                canvas[(x, round(y + c + t))] = '['
    for x in range(59, 81, 3):  # stitches along its top
        if canvas.get((x, round(76 + c))) in ('(', 'a', "'"):
            canvas[(x, round(76 + c))] = 'a'
    for x in range(69, 75):  # a bone at the knot
        canvas[(x, round(74 + c))] = 'w'
    for x, y in ((68, 73), (68, 75), (75, 73), (75, 75)):
        canvas[(x, round(y + c))] = 'w'
    for side, claws, toes in (('near', ((12, 4), (14, 5), (9, 5)), (7, 10)), ('far', ((8, 4), (10, 5)), ())):
        ankle = ankles[side]
        toe = pose.near_toe if side == 'near' else pose.far_toe
        for x, y in turned([(ankle[0] + dx, ankle[1] + dy) for dx, dy in claws], ankle, toe):
            if point((x, y)) in canvas:
                canvas[point((x, y))] = 'w'  # toe claws
        for x, y in turned([(ankle[0] + dx, ankle[1] + 3) for dx in toes], ankle, toe):
            if canvas.get(point((x, y))) in ('g', 'W', 'm'):
                canvas[point((x, y))] = 'T'  # toes
    if pose.dust:  # the slam's dust at the hands
        for hand, _, _ in hands.values():
            for k in range(6):
                x = round(hand[0] + (k - 2.5) * 4 + unit('slam', k) * 3)
                y = BIG_SOLE - round(unit('slam-y', k) * 5)
                if (x, y) not in canvas and 0 < x < BIG - 1:
                    canvas[(x, y)] = 'σ' if k % 2 else 'τ'
    return outline(canvas, 'o', size)


def troll_idle_frames():
    """Breathing: the chest rising, its hands swaying with it."""
    poses = [TrollPose(breath=b, near_elbow=(91, 68 - b), near_wrist=(94 - b * 0.5, 91 - b),
                       far_elbow=(52, 67 - b), far_wrist=(52, 89 - b)) for b in (0, 1, 1, 0)]
    return [to_image(troll_canvas(p), (BIG, BIG)) for p in poses], [260, 220, 260, 220]


def troll_walk_frames():
    """A heavy stride: each foot lifting and reaching while the other plants,
    the arms swinging against the legs."""
    poses = []
    for i in range(TROLL_WALK):
        a = i * math.tau / TROLL_WALK
        near_x, near_lift = 72 + 9 * math.sin(a), 4 * max(0.0, math.cos(a))
        far_x, far_lift = 54 - 9 * math.sin(a), 4 * max(0.0, -math.cos(a))
        swing = 6 * math.sin(a)  # the near arm back as the near leg goes ahead
        poses.append(TrollPose(
            crouch=1 - round(1.2 * abs(math.cos(a))), lean=2,
            near_ankle=(near_x, 117 - near_lift), near_toe=0.06 * near_lift,
            far_ankle=(far_x, 117 - far_lift), far_toe=0.06 * far_lift,
            near_elbow=(92 - swing * 0.4, 68), near_wrist=(95 - swing, 91 - abs(swing) * 0.25),
            far_elbow=(53 + swing * 0.4, 67), far_wrist=(53 + swing, 89 - abs(swing) * 0.25),
            sway=round(-1.5 * math.sin(a))))
    return [to_image(troll_canvas(p), (BIG, BIG)) for p in poses], [120] * TROLL_WALK


# The arms locked overhead, as the sheet's wind-up draws them: what the slam
# swings down from and the throw flings forward.
OVERHEAD = dict(lean=-3, lift=4, near_elbow=(92, 20), near_wrist=(81, 7), far_elbow=(53, 20),
                far_wrist=(68, 7), hands='clasped')


def troll_windup_frames():
    """The slam's wind-up: its arms rising before it to lock overhead,
    leaning back."""
    poses = [TrollPose(lean=-1, lift=1, near_elbow=(96, 62), near_wrist=(110, 50), far_elbow=(80, 62),
                       far_wrist=(98, 52), hands='fists'),
             TrollPose(lean=-2, lift=3, near_elbow=(100, 34), near_wrist=(98, 14), far_elbow=(86, 32),
                       far_wrist=(88, 12), hands='fists'),
             TrollPose(**OVERHEAD)]
    return [to_image(troll_canvas(p), (BIG, BIG)) for p in poses], [200, 300, 700]


def troll_slam_frames():
    """The slam: its fists swinging down to the ground before it, a deep
    crouch, dust."""
    poses = [TrollPose(lean=3, lift=2, near_elbow=(98, 30), near_wrist=(110, 22), far_elbow=(90, 30),
                       far_wrist=(102, 20), hands='fists'),
             TrollPose(lean=8, crouch=3, near_elbow=(102, 58), near_wrist=(114, 76), far_elbow=(96, 60),
                       far_wrist=(108, 78), hands='fists'),
             TrollPose(lean=12, crouch=7, near_elbow=(104, 86), near_wrist=(110, 106), far_elbow=(98, 88),
                       far_wrist=(104, 106), hands='fists', dust=True)]
    return [to_image(troll_canvas(p), (BIG, BIG)) for p in poses], [60, 60, 260]


def troll_pickup_frames():
    """Lifting something: bending to the ground, then raising it overhead."""
    poses = [TrollPose(lean=7, crouch=4, near_elbow=(98, 78), near_wrist=(104, 96), far_elbow=(90, 78),
                       far_wrist=(98, 96)),
             TrollPose(lean=12, crouch=8, near_elbow=(102, 86), near_wrist=(108, 100), far_elbow=(96, 86),
                       far_wrist=(102, 100)),
             TrollPose(**dict(OVERHEAD, lean=-2, lift=3))]
    return [to_image(troll_canvas(p), (BIG, BIG)) for p in poses], [140, 200, 200]


def troll_throw_frames():
    """A throw: locked overhead, flung forward, the arms following through."""
    poses = [TrollPose(**OVERHEAD),
             TrollPose(lean=5, lift=2, near_elbow=(96, 28), near_wrist=(110, 30), far_elbow=(88, 26),
                       far_wrist=(102, 26), hands='fists'),
             TrollPose(lean=9, crouch=2, near_elbow=(98, 62), near_wrist=(108, 80), far_elbow=(92, 62),
                       far_wrist=(102, 78))]
    return [to_image(troll_canvas(p), (BIG, BIG)) for p in poses], [140, 80, 260]


# The enemies' clips: (frames and their durations, looping). Each clip's
# frames are `<clip>_<i>`, and `generate.mirror_facing` gives every clip a
# left-facing twin.
ENEMY_CLIPS = {
    'wisp_float': (wisp_float_frames, True),
    'wisp_flare': (wisp_flare_frames, True),
    'crawler_walk': (crawler_walk_frames, True),
    'crawler_windup': (crawler_windup_frames, False),
    'crawler_lunge': (crawler_lunge_frames, False),
    'crawler_recover': (crawler_recover_frames, False),
    'crawler_scared': (crawler_scared_frames, True),
    'beetle_walk': (beetle_walk_frames, True),
    'beetle_paw': (beetle_paw_frames, True),
    'beetle_charge': (beetle_charge_frames, True),
    'bat_roost': (bat_roost_frames, True),
    'bat_fly': (bat_fly_frames, True),
    'bat_windup': (bat_windup_frames, True),
    'bat_dive': (bat_dive_frames, True),
    'troll_idle': (troll_idle_frames, True),
    'troll_walk': (troll_walk_frames, True),
    'troll_windup': (troll_windup_frames, False),
    'troll_slam': (troll_slam_frames, False),
    'troll_pickup': (troll_pickup_frames, False),
    'troll_throw': (troll_throw_frames, False),
}
def build_enemies(images, animations):
    """The five enemies' clips, unlit: no normal or emissive map."""
    for clip, (frames_for, looping) in ENEMY_CLIPS.items():
        rendered, durations = frames_for()
        frames = []
        for i, (im, duration) in enumerate(zip(rendered, durations, strict=True)):
            name = f'{clip}_{i}'
            images[name] = im
            frames.append({'sprite': 'ex10_' + name, 'duration_ms': duration})
        animations[clip] = {'looping': looping, 'frames': frames}
