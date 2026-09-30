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
    BAYER, NEIGHBOURS, Part, add, compose, outline, point, shade, smoothstep, to_image,
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


def lantern(hand, angle):
    """Upright lantern hanging from the hand by its bail; only the bail swings."""
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
    lamp = lantern(wrist, pose.lantern)
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


PLAYER_CLIPS = {
    # clip: (pose and duration source, looping)
    'player_idle': (idle_poses, True),
    'player_walk': (run_poses, True),
    'player_jump': (jump_poses, False),
    'player_fall': (fall_poses, True),
    'player_land': (land_poses, False),
    'player_double_jump': (double_jump_poses, False),
}


def frame_name(clip, index):
    # The idle clip starts on the registry's start sprite, `player`.
    return 'player' if clip == 'player_idle' and index == 0 else f'{clip}_{index}'


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
    for clip, (poses_for, looping) in PLAYER_CLIPS.items():
        poses, durations = poses_for()
        frames = []
        for i, (pose, duration) in enumerate(zip(poses, durations, strict=True)):
            name = frame_name(clip, i)
            im, glass = render(pose)
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


def build_balls(images, lit, animations):
    for clip, first, draw, duration in (('ball_spin', 'ball', rune_orb, 90),
                                        ('ball_small_spin', 'ball_small', glass_marble, 65)):
        names = [first if i == 0 else f'{first}_{i}' for i in range(BALL_FRAMES)]
        for i, name in enumerate(names):
            images[name] = draw(i)
            lit.add(name)
        animations[clip] = {'looping': True, 'frames': [
            {'sprite': 'ex10_' + name, 'duration_ms': duration} for name in names]}
