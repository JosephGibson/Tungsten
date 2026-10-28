"""Player and enemy art rendered from posed skeletons into palette pixels.

The wizard is rebuilt for every frame from one rig on a 96 px frame: hips,
neck, feet, hands, the staff's lean and grip, the robe's hem, the hat's tip
and the beard's swing. Each part (legs and soft boots, bell sleeves, the
robe with its mantle, rope belt, pouch, trim and sewn stars, the head with
its nose, brow and beard, the felt hat, the gnarled staff and its amber
orb) is filled flat, shaded from its own silhouette, rimmed where it
overlaps the parts behind it and outlined once, so every clip shares one
model; the death's lying frames turn the upright rig a quarter in whole
pixels, and no other raster is rotated. The fireball is an analytic ball of
flame whose tongues stream up and behind; the fire missile a rocket of fire.
"""
import math
from dataclasses import dataclass, replace
from functools import partial

from PIL import Image

from art import PALETTE
from pixels import (
    BAYER, NEIGHBOURS, Part, add, compose, despeckle, fbm, grade, outline, point, ramp, shade, smoothstep,
    to_image, unit,
)

SIZE = 64
SOLE = 59  # a 64 px frame's lowest drawn row; the outline adds row 60, so boxes end at 61
# The wizard's frame: his body as the courier's was (half 20 x 28), its
# centre at (48, 65), so his feet stand on row 93 (the lowest drawn row 91,
# the outline's 92), with room above for the hat and about for the staff.
WIZARD = 96
WIZARD_SOLE = 91
WIZARD_FEET = WIZARD_SOLE + 2  # the bounding box's bottom edge
FRAME = (WIZARD, WIZARD)
ANKLE = WIZARD_SOLE - 2  # a flat boot's ankle when planted
THIGH = SHIN = 8.5
UPPER_ARM = FOREARM = 6.5
REST_HIP = (46.0, 74.0)
STAFF = 54.0  # the staff's length, foot to top
STAND_HAND = (61.0, 66.0)
STAND_TILT = 0.13
FIREBALL_FRAMES = 10


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


def stand_grip(hand_y=STAND_HAND[1], tilt=STAND_TILT):
    """The grip that stands the staff's foot on the ground."""
    return (WIZARD_SOLE - 1 - hand_y) / (math.cos(tilt) * STAFF)


# The wizard's colours, indigo with brass trim (the plan's Q1): the robe and
# hat's ramp, the far sleeve's, the deep folds, the trim, the sewn stars,
# the hat's patch and the lining a flared hem shows.
WAY = dict(robe=dict(base='Π', light='Ξ', shadow='Γ', back='Γ', rim='Ξ'),
           far=dict(base='Γ', light='Π', shadow='ν', back='ν'),
           fold='Γ', deep='ν', trim='A', trim_dark='a', star='ϕ', patch='Δ', lining='Ф')


SKIN = dict(base='R', light='w', shadow='r')
BEARD = dict(base='σ', light='w', shadow='τ', back='τ', rim='w')
WOOD = dict(base='"', light="'", shadow='ω', back='ω', rim="'")
BOOT = dict(base='ω', light='"', shadow='/')
BOOT_FAR = dict(base='/', light='ω', shadow='>')
LEG = dict(base='[', shadow='o')


@dataclass
class WizardPose:
    """One frame of the wizard, facing right; sprite pixels."""
    hip: tuple = REST_HIP
    lean: float = 1.0
    head: tuple = (0.0, 0.0)
    near_foot: tuple = (50.0, ANKLE)
    far_foot: tuple = (42.0, ANKLE)
    near_toe: float = 0.0
    far_toe: float = 0.0
    near_planted: bool = True
    far_planted: bool = True
    near_hand: tuple = STAND_HAND
    far_hand: tuple = (40.0, 72.0)
    # The staff: its angle from upright (radians, + leans its top forward)
    # and how far up it the near hand holds it (0 its foot, 1 its top);
    # `staff_at` places it outright (foot, top) when not in the hand.
    staff: float = STAND_TILT
    grip: float = 0.47
    staff_at: tuple = None
    both_hands: bool = False
    hem_flare: float = 0.0
    hem_lift: float = 0.0
    hem_front: float = 0.0
    hem_wave: float = 0.0
    hem_amp: float = 0.6
    hem_rise: float = 0.0
    hat_tip: tuple = (0.0, 0.0)
    hat_tilt: float = 0.0
    beard: float = 0.0  # radians the beard's tail swings back
    beard_wave: float = 0.0
    eyes: str = 'open'
    flare: bool = False
    far_item: str = None  # 'pistol', 'ball' or None
    staff_behind: bool = False  # in the far hand, behind the body
    near_palm: bool = False  # the near hand open, pushing
    aim: float = 0.0  # the pistol's aim, radians up from forward
    palm: bool = False  # the far hand open, pushing
    bare: bool = False  # the hat knocked off: no brim shadow, a fringe round the crown
    parts: tuple = ('body', 'hat', 'staff')  # what the frame draws


def boot(ankle, toe_angle, planted):
    """A wizard's soft boot with a curled toe; planted ones sit on the sole row."""
    local = [(-2.5, -1.5), (-2.5, 2), (4, 2), (6, 1), (7, -0.5), (5.5, 0), (2, -1.5)]
    cos, sin = math.cos(toe_angle), math.sin(toe_angle)
    points = [(ankle[0] + x * cos - y * sin, ankle[1] + x * sin + y * cos) for x, y in local]
    lowest = max(p[1] for p in points)
    if planted or lowest > WIZARD_SOLE:
        points = [(x, y + WIZARD_SOLE - lowest) for x, y in points]
    return points


def leg(hip, foot, toe, planted, boots):
    knee, ankle = two_bone(hip, foot, THIGH, SHIN, 1)
    if planted and ankle[1] < foot[1] - 0.5:
        toe = max(toe, 0.5 + (foot[1] - ankle[1]) * 0.25)
    shape = Part(FRAME)
    shape.limb([hip, knee, ankle], 4)
    shoe = Part(FRAME)
    shoe.polygon(boot(ankle, toe, planted))
    return shade(shape.pixels(), **LEG), shade(shoe.pixels(), **boots)


def sleeve(shoulder, hand, ramp, cuff=True):
    """A bell sleeve from the shoulder, widening to its cuff, with the hand
    out of it; returns the sleeve, the hand and the wrist."""
    elbow, wrist = two_bone(shoulder, hand, UPPER_ARM, FOREARM, -1)
    shape = Part(FRAME)
    shape.polygon(strand([shoulder, elbow, wrist], [5.0, 5.5, 7.5]))
    inside = shape.pixels()
    colors = shade(inside, **ramp)
    if cuff:
        for x, y in inside:
            if math.hypot(x + 0.5 - wrist[0], y + 0.5 - wrist[1]) < 2.4:
                colors[(x, y)] = WAY['trim']
    fist = Part(FRAME)
    out = (wrist[0] + (wrist[0] - elbow[0]) * 0.28, wrist[1] + (wrist[1] - elbow[1]) * 0.28)
    fist.ellipse(out, 1.7, 1.7)
    return colors, shade(fist.pixels(), **SKIN), out, elbow


def robe(neck, hip, pose):
    """The robe from the shoulders to the ankles: a mantle, a belted waist,
    a hem that ripples and flares behind, folds that follow the hem, a
    brass trim down the front and along the hem, and a few sewn stars."""
    hem_y = ANKLE - 1.5 - pose.hem_rise
    back_shoulder = (neck[0] - 6, neck[1] + 1.5)
    front_shoulder = (neck[0] + 5, neck[1] + 1.5)
    chest = (neck[0] + 6.5, neck[1] + 7)
    front_waist = (hip[0] + 5 + pose.hem_front * 0.3, hip[1] - 1)
    front_hem = (hip[0] + 9 + pose.hem_front, min(hem_y - pose.hem_lift * 0.25, WIZARD_SOLE - 2))
    back_hem = (hip[0] - 10 - pose.hem_flare, min(hem_y - pose.hem_lift, WIZARD_SOLE - 2))
    back_waist = (hip[0] - 7 - pose.hem_flare * 0.3, hip[1] - 2 - pose.hem_lift * 0.3)
    back = (neck[0] - 7, neck[1] + 7)
    hem = []
    steps = 9
    for k in range(steps + 1):
        f = k / steps
        x = front_hem[0] + (back_hem[0] - front_hem[0]) * f
        y = front_hem[1] + (back_hem[1] - front_hem[1]) * f
        y += pose.hem_amp * (0.5 + f) * math.sin(pose.hem_wave + k * 1.1)
        hem.append((x, min(y, WIZARD_SOLE - 1)))
    shape = Part(FRAME)
    shape.polygon([back_shoulder, front_shoulder, chest, front_waist] + hem + [back_waist, back])
    inside = shape.pixels()
    colors = shade(inside, **WAY['robe'])
    # Folds rise from the hem along the ripple.
    for k, offset in enumerate((-7.0, -3.0, 1.0, 5.0)):
        x = round(hip[0] + offset - pose.hem_flare * 0.3 * (k == 0))
        bottom = max((y for (px, y) in inside if px == x), default=None)
        if bottom is None:
            continue
        length = 6 + (k * 3 + round(pose.hem_wave * 2)) % 5
        for y in range(bottom - length, bottom):
            if (x, y) in inside:
                colors[(x, y)] = WAY['fold']
            if (x + 1, y) in inside and y > bottom - length // 2:
                colors[(x + 1, y)] = WAY['deep'] if k % 2 else WAY['fold']
    # Each fold's lit edge beside it.
    for (x, y), c in list(colors.items()):
        if c == WAY['fold'] and colors.get((x - 1, y)) == WAY['robe']['base'] and (x - 1, y) in inside:
            colors[(x - 1, y)] = WAY['robe']['light'] if (x + y) % 3 == 0 else colors[(x - 1, y)]
    # The lining shows where the hem flares out behind.
    if pose.hem_flare > 2.5:
        for x, y in inside:
            if x < back_hem[0] + 3 + pose.hem_flare * 0.3 and y > hip[1] + 6:
                colors[(x, y)] = WAY['lining']
    # The trim: along the hem and down the front opening.
    for x, y in inside:
        if (x, y + 1) not in inside or (x, y + 2) not in inside and (x, y + 1) in inside:
            colors[(x, y)] = WAY['trim'] if (x, y + 1) not in inside else WAY['trim_dark']
    front_x = lambda y: front_waist[0] + (front_hem[0] - front_waist[0]) * max(0.0, (y - front_waist[1])) / max(1.0, front_hem[1] - front_waist[1]) - 1.5
    for y in range(round(front_waist[1]) + 1, round(front_hem[1])):
        x = round(front_x(y))
        if (x, y) in inside:
            colors[(x, y)] = WAY['trim']
    # A rope belt with a knot and a hanging tassel, and a pouch.
    for y in (round(hip[1]) - 3, round(hip[1]) - 2):
        for x in range(round(hip[0]) - 8, round(hip[0]) + 8):
            if (x, y) in inside:
                colors[(x, y)] = 'a' if (x + y) % 3 else '('
    knot = (round(hip[0]) + 3, round(hip[1]) - 2)
    for dx, dy in ((0, 0), (1, 0), (0, 1), (1, 1)):
        colors[(knot[0] + dx, knot[1] + dy)] = 'A'
    for dy in range(2, 7):
        colors[(knot[0] + (dy > 4), knot[1] + dy)] = 'a' if dy % 2 else 'A'
    # Stars sewn above the hem.
    for f in (0.2, 0.5, 0.8):
        x = front_hem[0] + (back_hem[0] - front_hem[0]) * f
        y = front_hem[1] + (back_hem[1] - front_hem[1]) * f - 4
        if point((x, y)) in inside:
            plus(colors, (x, y), WAY['star'], inside)
    # The mantle: a short cape over the shoulders, its edge trimmed.
    mantle = Part(FRAME)
    mantle.polygon([(neck[0] - 7, neck[1] + 1), (neck[0] + 6, neck[1] + 1), (neck[0] + 7.5, neck[1] + 6),
                    (neck[0] + 2, neck[1] + 8.5), (neck[0] - 4, neck[1] + 8), (neck[0] - 8.5, neck[1] + 5)])
    cape = mantle.pixels()
    for x, y in cape:
        colors[(x, y)] = WAY['fold'] if (x, y + 1) in cape else WAY['trim']
        if (x, y - 1) not in cape:
            colors[(x, y)] = WAY['robe']['base']
    return colors


def pouch(hip, pose):
    corner = (hip[0] - 9 - pose.hem_flare * 0.2, hip[1] - 1)
    bag = Part(FRAME)
    bag.polygon([corner, (corner[0] + 5, corner[1]), (corner[0] + 5, corner[1] + 6),
                 (corner[0] + 1, corner[1] + 7)])
    colors = shade(bag.pixels(), '(', light='a', shadow='/', back='/')
    colors[point((corner[0] + 2.5, corner[1] + 2))] = 'A'
    return colors


def plus(colors, at, symbol, inside=None):
    """A sewn four-point star, three pixels across."""
    x, y = point(at)
    for p in ((x, y), (x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)):
        if inside is None or p in inside:
            colors[p] = symbol


def head_and_hat(neck, pose):
    """The head in profile under the brim: a big bulbous nose, a bushy brow
    jutting over a bright eye in the brim's shadow, long grey hair behind;
    the beard from cheek and chin to the belt with a drooping moustache; the
    hat's wide tilted brim, its tall cone of worn felt, dithered on its
    shaded side, with a patch, a brass band and buckle, a crescent and
    stars, and its top bent back and flopping down."""
    cx, cy = neck[0] + 2.5 + pose.head[0], neck[1] - 8 + pose.head[1]
    skull = Part(FRAME)
    skull.ellipse((cx, cy), 5.5, 6)
    nose = Part(FRAME)
    nose.polygon([(cx + 3.5, cy - 1.5), (cx + 7.0, cy + 0.5), (cx + 8.6, cy + 2.4),
                  (cx + 7.6, cy + 3.9), (cx + 5.5, cy + 3.6), (cx + 4, cy + 2.6)])
    face = skull.pixels() | nose.pixels()
    colors = shade(face, **SKIN)
    if pose.bare:
        # Hatless: a bald crown, lit, ringed behind by a fringe of grey hair
        # with a few stray wisps standing up.
        for x, y in skull.pixels():
            if x + 0.5 < cx - 1.5 and y + 0.5 < cy + 1 and (x + 0.5 - cx) * 0.8 + (y + 0.5 - cy) < -2.5:
                colors[(x, y)] = 'τ' if (x + y) % 3 else 'S'
                if (x, y - 1) not in skull.pixels() or (x - 1, y) not in skull.pixels():
                    colors[(x, y)] = 'σ'
        for dx, dy in ((-4.5, -6.6), (-2.5, -7.4), (-6.2, -5.0), (-1.0, -7.0)):
            colors[point((cx + dx, cy + dy))] = 'σ'
    else:
        # The brim's shadow over the brow.
        for x, y in face:
            if y <= cy - 2:
                colors[(x, y)] = 'r'
    for x, y in nose.pixels():
        if (x, y - 1) not in face or y <= cy + 0.5:
            colors[(x, y)] = 'w' if y > cy - 1 else 'R'
        elif (x, y + 1) not in face:
            colors[(x, y)] = 'r'
    # Long grey hair at the back of the head, below the brim, to the nape.
    hair = Part(FRAME)
    hair.polygon([(cx - 5.8, cy - 3), (cx - 2.2, cy - 3.2), (cx - 2.6, cy + 2), (cx - 3.6, cy + 6.5),
                  (cx - 6.6, cy + 7.5), (cx - 7, cy + 2)])
    for x, y in hair.pixels():
        colors[(x, y)] = 'τ' if (x * 2 + y) % 4 else 'S'
        if (x + 1, y) not in hair.pixels() or (x, y - 1) not in hair.pixels():
            colors[(x, y)] = 'σ'
    # A bushy white brow jutting over the eye, the eye bright in shadow.
    for i, dy in enumerate((1, 0, 0, -1, 0, 1)):
        colors[(round(cx + 1 + i), round(cy - 1.6) + dy)] = 'w' if i % 2 else 'σ'
    eye = (round(cx + 3), round(cy - 0.0))
    colors[eye] = 'w'
    colors[(eye[0] + 1, eye[1])] = 'o'
    colors[(eye[0], eye[1] + 1)] = 'r'
    colors[(eye[0] + 1, eye[1] + 1)] = 'r'
    if pose.eyes == 'closed':
        colors[eye] = 'r'
        colors[(eye[0] + 1, eye[1])] = 'X'
    elif pose.eyes == 'squint':
        colors[eye] = 'X'
        colors[(eye[0] + 1, eye[1])] = 'X'
    # The beard: from cheek and chin down the chest to the belt, its tail
    # curling back with the move; wavy darker strands; a drooping moustache.
    chin = (cx + 3.2, cy + 4.0)
    swing = pose.beard
    pts = [chin]
    for i in range(1, 8):
        a = math.pi / 2 + 0.08 + swing * (i / 7) ** 1.4 + 0.07 * math.sin(pose.beard_wave + i * 1.3)
        pts.append(add(pts[-1], (math.cos(a), math.sin(a)), 2.5))
    beard = Part(FRAME)
    beard.polygon(strand(pts, [6.8, 7.0, 6.4, 5.6, 4.6, 3.4, 2.2, 1.0]))
    beard.polygon([(cx - 0.5, cy + 2.5), (cx + 1.5, cy + 2.2), (cx + 3.5, cy + 3.8), (cx + 0.5, cy + 6)])
    stache = Part(FRAME)
    stache.polygon([(cx + 3.6, cy + 3.2), (cx + 7.6, cy + 3.6), (cx + 8.2, cy + 6.2), (cx + 6.6, cy + 5.2),
                    (cx + 4.2, cy + 5.4)])
    hair_px = beard.pixels()
    bcolors = shade(hair_px, **BEARD)
    for x, y in hair_px:
        wave = math.sin(y * 0.9 + x * 0.35 + pose.beard_wave)
        if wave > 0.72 and (x, y - 1) in hair_px:
            bcolors[(x, y)] = 'τ'
        elif wave < -0.85 and (x - 1, y) in hair_px:
            bcolors[(x, y)] = 'w'
    tip = point(pts[-1])
    bcolors[tip] = 'S'
    bcolors.update(shade(stache.pixels(), 'w', shadow='σ'))
    # The hat: a wide tilted brim, a tall felt cone bending back near its
    # top, the top flopping down behind.
    brim_c = (cx + 0.5 + pose.hat_tilt, cy - 4.6)
    base_y = brim_c[1] - 1
    t = pose.hat_tip
    spine = [(brim_c[0], base_y + 1), (brim_c[0] - 0.5, base_y - 8),
             (brim_c[0] - 2 + t[0] * 0.25, base_y - 15 + t[1] * 0.25),
             (brim_c[0] - 5 + t[0] * 0.5, base_y - 21 + t[1] * 0.5),
             (brim_c[0] - 9.5 + t[0] * 0.8, base_y - 24.5 + t[1] * 0.8),
             (brim_c[0] - 13 + t[0], base_y - 22 + t[1])]
    cone = Part(FRAME)
    cone.polygon(strand(spine, [14.0, 11.5, 8.5, 5.6, 3.2, 1.2]))
    cone_px = cone.pixels()
    hat = shade(cone_px, **WAY['robe'])
    centre_x = {}
    for x, y in cone_px:
        centre_x.setdefault(y, []).append(x)
    for x, y in cone_px:
        xs = centre_x[y]
        mid = (min(xs) + max(xs)) / 2
        if hat[(x, y)] == WAY['robe']['base'] and x < mid - 0.5 and (x + y) % 2 == 0:
            hat[(x, y)] = WAY['fold']  # the felt's shaded side, dithered
        if x == max(xs) - 1 and hat[(x, y)] == WAY['robe']['base']:
            hat[(x, y)] = WAY['robe']['light']
    # The crumple where it bends, and a patch stitched on the cone.
    bend = spine[2]
    for x, y in cone_px:
        if abs(y - bend[1]) < 0.8 and x < bend[0] + 2:
            hat[(x, y)] = WAY['deep']
    patch = (round(brim_c[0] - 4), round(base_y - 11))
    for dx in range(3):
        for dy in range(3):
            q = (patch[0] + dx, patch[1] + dy)
            if q in cone_px:
                hat[q] = WAY['patch'] if (dx, dy) != (1, 1) else WAY['fold']
    for dx, dy in ((-1, 0), (3, 2), (1, -1)):
        q = (patch[0] + dx, patch[1] + dy)
        if q in cone_px:
            hat[q] = WAY['trim_dark']
    # Band and buckle.
    for x, y in cone_px:
        if base_y - 3 <= y <= base_y - 1:
            hat[(x, y)] = WAY['trim'] if y == base_y - 3 else WAY['trim_dark']
    for dx, dy in ((2, -3), (3, -3), (4, -3), (2, -2), (4, -2), (2, -1), (3, -1), (4, -1)):
        hat[(round(brim_c[0]) + dx, round(base_y) + dy)] = 'σ' if (dx, dy) == (2, -3) else 'Φ'
    # A crescent and a star sewn on the cone.
    moon = Part(FRAME)
    mc = (brim_c[0] + 0.5, base_y - 7.5)
    moon.ellipse(mc, 2.4, 2.6)
    bite = Part(FRAME)
    bite.ellipse((mc[0] + 1.4, mc[1] - 0.7), 2.0, 2.1)
    for p in moon.pixels() - bite.pixels():
        if p in cone_px:
            hat[p] = WAY['star']
    plus(hat, (brim_c[0] - 3.5, base_y - 17), WAY['star'], cone_px)
    brim = Part(FRAME)
    brim.polygon([(brim_c[0] - 13, brim_c[1] - 0.8), (brim_c[0] - 4, brim_c[1] - 2.4),
                  (brim_c[0] + 6, brim_c[1] - 2.0), (brim_c[0] + 13.5, brim_c[1] + 0.6),
                  (brim_c[0] + 11, brim_c[1] + 2.2), (brim_c[0], brim_c[1] + 1.8),
                  (brim_c[0] - 11.5, brim_c[1] + 1.0)])
    brim_px = brim.pixels()
    bri = shade(brim_px, WAY['robe']['base'], light=WAY['robe']['light'], shadow=WAY['deep'])
    return colors, bcolors, hat, bri, (cx, cy)


def staff_points(pose):
    """The staff's foot and top."""
    if pose.staff_at:
        foot, top = pose.staff_at
    else:
        d = (math.sin(pose.staff), -math.cos(pose.staff))
        hand = pose.near_hand
        foot = add(hand, d, -STAFF * pose.grip)
        top = add(hand, d, STAFF * (1 - pose.grip))
    for a, b in ((foot, top), (top, foot)):
        if a[1] > WIZARD_SOLE - 1 and b[1] < a[1]:
            dx, dy = b[0] - a[0], b[1] - a[1]
            t = (a[1] - (WIZARD_SOLE - 1)) / -dy
            foot, top = add(foot, (dx, dy), t), add(top, (dx, dy), t)
            break
    return foot, top


def staff(pose):
    """The gnarled staff, knotted, and its crook round the orb; the wood,
    the orb's glass and the glass's centre, the staff light's anchor."""
    foot, top = staff_points(pose)
    dx, dy = top[0] - foot[0], top[1] - foot[1]
    length = math.hypot(dx, dy) or 1.0
    ux, uy = dx / length, dy / length
    nx, ny = -uy, ux
    pts, widths = [], []
    for i in range(9):
        f = i / 8
        wob = 0.6 * math.sin(f * 11.0 + 0.7)
        pts.append((foot[0] + dx * f * 0.93 + nx * wob, foot[1] + dy * f * 0.93 + ny * wob))
        widths.append(2.6 + 0.9 * (unit('knot', i) > 0.62))
    shaft = Part(FRAME)
    shaft.polygon(strand(pts, widths))
    wood = shade(shaft.pixels(), **WOOD)
    # The crook: two prongs curling round the orb.
    orb_c = (top[0] - nx * 0.5, top[1] - ny * 0.5)
    crook = Part(FRAME)
    for side in (1, -1):
        arc = []
        for k in range(6):
            a = math.atan2(-uy, -ux) + side * (0.55 + k * 0.42)
            arc.append((orb_c[0] + math.cos(a) * 4.4, orb_c[1] + math.sin(a) * 4.4))
        crook.limb([pts[-1]] + arc, 2)
    wood.update(shade(crook.pixels(), 'ω', light="'", shadow='/'))
    # A leather grip wound where the hand holds it, and a sprig of leaves.
    if not pose.staff_at:
        for k in range(-3, 4):
            q = point((pose.near_hand[0] + ux * k * 0.9, pose.near_hand[1] + uy * k * 0.9))
            for e in (q, (q[0] + 1, q[1])):
                if e in wood:
                    wood[e] = '(' if k % 2 else 'a'
    stem = add(top, (ux, uy), -6.5)
    for side, length in ((1, 3.2), (-1, 2.6)):
        leaf = Part(FRAME)
        tip = (stem[0] + nx * side * length - ux * 1.5, stem[1] + ny * side * length - uy * 1.5)
        leaf.limb([stem, tip], 2)
        wood.update(shade(leaf.pixels(), 'G', light='m', shadow='g'))
    radius = 3.6 if pose.flare else 2.6
    glass = Part(FRAME)
    glass.ellipse(orb_c, radius, radius)
    # Plain amber glass, not emissive: dull while the light is off, and
    # shining under the glow's hot core while it burns (`player_lantern`).
    orb = {}
    inside = glass.pixels()
    for x, y in inside:
        dx, dy = x + 0.5 - orb_c[0], y + 0.5 - orb_c[1]
        r = math.hypot(dx, dy)
        orb[(x, y)] = 'J' if r < radius * 0.55 else ('K' if dx + dy < radius * 1.05 else 'L')
    glint = point((orb_c[0] - radius * 0.35, orb_c[1] - radius * 0.4))
    orb[glint] = 'Z'
    # The anchor: the drawn glass's centre, whatever covers it.
    anchor = tuple(round(sum(p[a] + 0.5 for p in inside) / len(inside), 3) for a in (0, 1))
    return wood, orb, anchor


def pistol(wrist, aim):
    ax, ay = math.cos(aim), -math.sin(aim)
    px, py = -ay, ax

    def at(u, v):
        return (wrist[0] + ax * u + px * v, wrist[1] + ay * u + py * v)
    barrel = Part(FRAME)
    barrel.polygon([at(-1, -1.6), at(8, -1.6), at(8, 0.6), at(-1, 0.6)])
    grip = Part(FRAME)
    grip.polygon([at(-1, 0.0), at(1.5, 0.0), at(0.5, 3.5), at(-2, 3.5)])
    colors = shade(grip.pixels(), '"', shadow='ω')
    colors.update(shade(barrel.pixels(), 'Ω', light='χ', shadow='Φ'))
    return colors


def render(pose):
    """The frame and its staff light's anchor, the orb's centre."""
    hip = pose.hip
    neck = (hip[0] + pose.lean, hip[1] - 17)
    near_shoulder = (neck[0] + 2, neck[1] + 3)
    far_shoulder = (neck[0] - 3, neck[1] + 3)
    face, beard, hat, brim, _ = head_and_hat(neck, pose)
    far_hand = pose.far_hand
    far_sleeve, far_fist, far_wrist, _ = sleeve(far_shoulder, far_hand, WAY['far'])
    if pose.both_hands:
        far_sleeve, far_fist, far_wrist, _ = sleeve(far_shoulder, pose.far_hand, WAY['robe'])
    far_leg, far_boot = leg((hip[0] - 2, hip[1]), pose.far_foot, pose.far_toe, pose.far_planted, BOOT_FAR)
    near_leg, near_boot = leg((hip[0] + 1, hip[1]), pose.near_foot, pose.near_toe, pose.near_planted, BOOT)
    near_sleeve, near_fist, near_wrist, _ = sleeve(near_shoulder, pose.near_hand, WAY['robe'])
    if pose.near_palm:
        palm = Part(FRAME)
        palm.ellipse((near_wrist[0] + 1.0, near_wrist[1]), 1.7, 2.6)
        near_fist = shade(palm.pixels(), **SKIN)
        for dy in (-2, 0, 2):  # spread fingers
            near_fist[point((near_wrist[0] + 3.0, near_wrist[1] + dy))] = 'R'
    wood, orb, anchor = staff(pose)
    held = {}
    if pose.far_item == 'pistol':
        held = pistol(far_wrist, pose.aim)
    elif pose.far_item == 'ball':
        ball = Part(FRAME)
        ball.ellipse((far_wrist[0] + 1, far_wrist[1] - 1), 3, 3)
        held = shade(ball.pixels(), 'β', light='δ', shadow='γ')
    if pose.palm:
        palm = Part(FRAME)
        palm.ellipse((far_wrist[0] + 0.8, far_wrist[1]), 1.6, 2.4)
        far_fist = shade(palm.pixels(), **SKIN)
    front_far = pose.far_item in ('pistol',) or pose.palm or pose.both_hands
    layers = [(far_leg, 'o'), (far_boot, 'o'), (near_leg, 'o'), (near_boot, 'o')]
    if pose.staff_behind:
        layers = [(wood, 'o'), (orb, 'o')] + layers
    if not front_far:
        layers = [(far_sleeve, 'o'), (far_fist, 'o'), (held, 'o')] + layers
    layers += [(robe(neck, hip, pose), 'o'), (pouch(hip, pose), 'o'), (face, 'o'), (beard, 'o'),
               (hat, 'o'), (brim, 'o')]
    if pose.staff_behind:
        layers += [(near_sleeve, 'o'), (near_fist, 'o')]
    elif pose.staff_at:
        # On the ground, drawn after the body as it lies in front.
        layers += [(wood, 'o'), (orb, 'o')]
        layers += [(near_sleeve, 'o'), (near_fist, 'o')]
    else:
        layers += [(wood, 'o'), (orb, 'o'), (near_sleeve, 'o'), (near_fist, 'o')]
    if front_far:
        layers += [(far_sleeve, 'o'), (far_fist, 'o'), (held, 'o')]
    # The parts the frame draws: the body (all but the hat and the staff),
    # the hat, the staff.
    groups = {'hat': (hat, brim), 'staff': (wood, orb)}

    def wanted(part):
        for name, members in groups.items():
            if any(part is m for m in members):
                return name in pose.parts
        return 'body' in pose.parts
    layers = [(part, sep) for part, sep in layers if wanted(part)]
    canvas = compose(layers)
    # The orb stays whole over what crosses it, unless the staff is behind.
    if not pose.staff_behind and 'staff' in pose.parts:
        canvas.update(orb)
    for p in [p for p in canvas if not any((p[0] + dx, p[1] + dy) in canvas for dx, dy in NEIGHBOURS)]:
        del canvas[p]
    return to_image(outline(canvas, 'o', FRAME), FRAME), anchor


# --- poses: the courier's, moved into the bigger frame, with the staff ----


def run_foot(hip_x, phase):
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
        poses.append(WizardPose(
                 hip=(REST_HIP[0], REST_HIP[1] + breath), near_hand=(61, 66 + breath),
                 far_hand=(40, 72 + breath), grip=stand_grip(66 + breath),
                 hem_wave=math.tau * p, hem_amp=0.7, hat_tip=(0.6 * math.sin(math.tau * p - 0.6), 0.5 * math.sin(math.tau * p)),
                 beard=0.05 * math.sin(math.tau * p), beard_wave=math.tau * p,
                 eyes='closed' if i == 6 else 'open'))
    return poses, [300, 240, 240, 300, 240, 240, 100, 240]


def walk_poses():
    poses = []
    for i in range(12):
        p = i / 12
        step = (2 * p) % 1
        bob = round(2 * math.cos(math.tau * (step - 0.15)))
        hip = (REST_HIP[0] - 1, REST_HIP[1] + bob)
        near, near_planted, near_toe = run_foot(hip[0] + 1, p)
        far, far_planted, far_toe = run_foot(hip[0] - 2, (p + 0.5) % 1)
        swing = math.cos(math.tau * p)
        neck = (hip[0] + 3, hip[1] - 17)
        far_hand = add((neck[0] - 3, neck[1] + 3), (1 + 6 * swing, 8 - 2 * abs(swing)))
        poses.append(WizardPose(
                 hip=hip, lean=3.0, near_foot=near, far_foot=far, near_toe=near_toe, far_toe=far_toe,
                 near_planted=near_planted, far_planted=far_planted,
                 near_hand=(60 + 1.5 * math.sin(math.tau * p + 0.5), neck[1] + 9 - 1.2 * swing), far_hand=far_hand,
                 staff=0.22 + 0.12 * math.sin(math.tau * p + 0.4), grip=0.42,
                 hem_flare=4 + 1.5 * math.sin(math.tau * 2 * p), hem_lift=2, hem_front=-1,
                 hem_wave=math.tau * 2 * p, hem_amp=1.3, hem_rise=1,
                 hat_tip=(-3, 2 + 0.8 * math.sin(math.tau * 2 * p - 1)), beard=0.45, beard_wave=math.tau * 2 * p))
    return poses, [45] * 12


def jump_poses():
    hip = (REST_HIP[0], REST_HIP[1] - 2)
    takeoff = WizardPose(hip=hip, lean=1.5, near_foot=(hip[0] + 1.5, ANKLE), near_toe=0.9,
                         far_foot=(hip[0] - 6, 82), far_toe=0.6, far_planted=False,
                         near_hand=(60, 60), far_hand=(37, 50), staff=0.35, grip=0.45,
                         hem_flare=2, hem_lift=-2, hem_amp=0.5, hat_tip=(1, 3), beard=-0.2)
    rise = WizardPose(hip=(hip[0], hip[1] + 1), lean=1.5, near_foot=(hip[0] + 5, 83), near_toe=0.2,
                      near_planted=False, far_foot=(hip[0] - 3.5, ANKLE), far_toe=1.0,
                      near_hand=(60, 61), far_hand=(38, 50), staff=0.45, grip=0.45,
                      hem_flare=3, hem_lift=-3, hem_wave=1.2, hem_amp=1.2, hat_tip=(0, 4), beard=-0.35)
    apex = WizardPose(hip=(hip[0], hip[1] + 1), lean=1.0, near_foot=(hip[0] + 4, 85), near_toe=0.3,
                      near_planted=False, far_foot=(hip[0] - 2.5, ANKLE), far_toe=0.9,
                      near_hand=(60, 64), far_hand=(35, 62), staff=0.25, grip=0.45,
                      hem_flare=4, hem_lift=3, hem_wave=2.4, hem_amp=1.6, hat_tip=(-1, -1), beard=0.2)
    return [takeoff, rise, apex], [60, 70, 70]


def fall_poses():
    poses = []
    for i in range(4):
        p = i / 4
        fl = math.sin(math.tau * p)
        hip = (REST_HIP[0], REST_HIP[1] - 1)
        poses.append(WizardPose(
                 hip=hip, lean=0.5, near_foot=(hip[0] + 3, 87 + fl), near_toe=0.5, near_planted=False,
                 far_foot=(hip[0] - 3, ANKLE), far_toe=0.8, near_hand=(61, 60 + fl), far_hand=(35, 54 - fl),
                 staff=0.55, grip=0.45, hem_flare=3, hem_lift=6 + fl, hem_wave=math.tau * p, hem_amp=2.2,
                 hat_tip=(2, -5 + fl), beard=-0.6, beard_wave=math.tau * p))
    return poses, [80] * 4


def land_poses():
    impact = WizardPose(hip=(REST_HIP[0], REST_HIP[1] + 2), near_foot=(53, ANKLE), far_foot=(40, ANKLE),
                        near_hand=(60, 69), far_hand=(36, 74), staff=0.15, grip=0.4, hem_flare=3, hem_front=2,
                        hem_amp=0.4, hat_tip=(0, 4), beard=0.15, eyes='squint')
    recover = WizardPose(hip=(REST_HIP[0], REST_HIP[1] + 1), near_foot=(52, ANKLE), far_foot=(41, ANKLE),
                         near_hand=(59, 67), far_hand=(38, 73), staff=0.06, grip=0.45, hem_flare=1.5, hem_front=1,
                         hem_wave=1.5, hem_amp=0.6, hat_tip=(0, 2))
    settle = WizardPose(grip=stand_grip(), hem_flare=0.5, hem_wave=3.0)
    return [impact, recover, settle], [60, 60, 60]


def double_jump_poses():
    hip = (REST_HIP[0], REST_HIP[1] - 1)
    tuck = WizardPose(hip=(hip[0], hip[1] + 5), lean=3.0, head=(1, 1), near_foot=(hip[0] + 6, ANKLE),
                      far_foot=(hip[0] + 1, ANKLE), near_hand=(59, 70), far_hand=(53, 71), staff=0.5, grip=0.5,
                      hem_flare=2, hem_lift=-1, hem_front=2, hem_amp=0.8, hat_tip=(3, 4), beard=0.1)
    burst = WizardPose(hip=(hip[0], hip[1] - 1), lean=1.0, near_foot=(hip[0] + 3, ANKLE), near_toe=1.0,
                       far_foot=(hip[0] - 6, 85), far_toe=0.6, far_planted=False, near_hand=(61, 58),
                       far_hand=(35, 48), staff=-0.15, grip=0.4, hem_flare=9, hem_front=5, hem_lift=5,
                       hem_wave=1.0, hem_amp=2.2, hat_tip=(-2, -4), beard=-0.7, flare=True)
    spread = WizardPose(hip=(hip[0], hip[1] - 1), lean=1.0, near_foot=(hip[0] + 3, 86), near_toe=0.5,
                        near_planted=False, far_foot=(hip[0] - 3, ANKLE), far_toe=1.0, near_hand=(61, 60),
                        far_hand=(36, 54), staff=0.1, grip=0.42, hem_flare=6, hem_front=3, hem_lift=4,
                        hem_wave=2.2, hem_amp=2.0, hat_tip=(-2, -3), beard=-0.5)
    rise = WizardPose(hip=(hip[0], hip[1] - 1), lean=1.5, near_foot=(hip[0] + 4, 84), near_toe=0.3,
                      near_planted=False, far_foot=(hip[0] - 3, ANKLE), far_toe=1.0, near_hand=(60, 62),
                      far_hand=(39, 56), staff=0.3, grip=0.45, hem_flare=4, hem_front=1, hem_lift=1,
                      hem_wave=3.4, hem_amp=1.2, hat_tip=(-1, -1), beard=-0.3)
    return [tuck, burst, spread, rise], [50, 60, 70, 120]


def hurt_poses():
    """F1's hurt clip: knocked back, the head snapping back, the staff
    jolted; then half-way back."""
    h = REST_HIP
    hit = WizardPose(hip=(h[0] - 2, h[1] + 1), lean=-3.0, head=(-1.5, 0.5), near_foot=(51, ANKLE),
                     far_foot=(39, ANKLE), far_toe=0.2, near_hand=(60, 63), far_hand=(33, 66),
                     staff=0.42, grip=0.45, hem_flare=1, hem_front=3, hem_wave=0.5, hem_amp=1.0,
                     hat_tip=(4, 3), hat_tilt=-1.0, beard=0.5, eyes='squint')
    back = WizardPose(hip=(h[0] - 1, h[1] + 1), lean=-1.0, head=(-0.5, 0.5), near_foot=(51, ANKLE),
                      far_foot=(40, ANKLE), near_hand=(60, 65), far_hand=(36, 70), staff=0.28, grip=0.45,
                      hem_flare=1.5, hem_front=1.5, hem_wave=1.3, hem_amp=0.8, hat_tip=(2, 2), beard=0.25,
                      eyes='squint')
    return [hit, back], [90, 110]


# The death: struck, he reels back, sits down hard and falls flat on his back;
# the hat flies off behind him and lands upright, the staff is flung ahead.
# Frames 1-4 draw the upright rig; 5-7 turn it a quarter anticlockwise (whole
# pixels, so nothing jags): the head to the left, the face up, the toes up.
DEATH_GROUND = WIZARD_FEET - 1  # the outline's row under the soles


def drawn(pose, *parts):
    """The pose's chosen parts, cropped; where the crop sits; the staff
    light's anchor, in the frame."""
    im, orb = render(replace(pose, parts=parts))
    box = im.getbbox()
    return im.crop(box), (box[0], box[1]), orb


def grounded(crop, left=None, right=None, lift=0):
    """`crop` placed with its lowest row on the ground, `lift` above it."""
    x = left if left is not None else right - crop.width + 1
    return crop, (round(x), DEATH_GROUND - (crop.height - 1) - lift)


def moved(anchor, drawn_at, placed_at):
    """`anchor`, drawn with its crop at `drawn_at`, as the crop is placed."""
    return (anchor[0] + placed_at[0] - drawn_at[0], anchor[1] + placed_at[1] - drawn_at[1])


def stacked(layers):
    out = Image.new('RGBA', FRAME, (0, 0, 0, 0))
    for im, at in layers:
        out.alpha_composite(im, at)
    return out


def hat_off(pose, dx, dy, tip):
    """The hat as the pose wears it, lifted `dx`, `dy` off his head."""
    crop, at, _ = drawn(replace(pose, hat_tip=tip), 'hat')
    return crop, (at[0] + dx, at[1] + dy)


def hat_landed(left, tip):
    crop, _, _ = drawn(WizardPose(hat_tip=tip), 'hat')
    return grounded(crop, left=left)


def staff_down(foot_x, top_x, rise=0.0):
    """The staff lying on the ground, its top `rise` up off it, placed, and
    its anchor."""
    crop, at, orb = drawn(WizardPose(staff_at=((foot_x, 88.0), (top_x, 88.0 - rise))), 'staff')
    placed = grounded(crop, right=top_x + 6)
    return placed, moved(orb, at, placed[1])


def dust(canvas, xs, seed, spread=2.2):
    """Puffs of grey dust along the ground at `xs`."""
    px = canvas.load()
    for i, x0 in enumerate(xs):
        for k in range(6):
            x = round(x0 + (k - 2.5) * spread + unit(seed, i, k) * 2)
            y = DEATH_GROUND - 1 - round(unit(seed + '-y', i, k) * 4)
            if 0 < x < WIZARD - 1 and px[x, y][3] == 0:
                px[x, y] = PALETTE['σ' if k % 2 else 'τ']
                if k % 3 == 0 and px[x, y - 1][3] == 0:
                    px[x, y - 1] = PALETTE['S']


def supine(near_hand, far_hand, beard=0.0):
    """Lying on his back: the upright rig, the robe straight over his legs,
    turned a quarter anticlockwise."""
    pose = WizardPose(lean=0.0, near_hand=near_hand, far_hand=far_hand, near_foot=(48, ANKLE), far_foot=(45, ANKLE),
                      hem_front=-3, hem_flare=-2, hem_amp=0.2, hem_wave=0.0, beard=beard, eyes='closed', bare=True)
    body, _, _ = drawn(pose, 'body')
    return body.transpose(Image.Transpose.ROTATE_90)


def death_frames():
    """The death's seven frames, each with its anchor, and their
    durations: the blow, the stagger, the sit, the topple, the impact on his
    back with dust, the rebound and the rest."""
    h = REST_HIP
    frames = []
    # The blow: the head snaps back, the arms fly forward, the hat lifts.
    pose = WizardPose(hip=(h[0] - 2, h[1] + 1), lean=-4.0, head=(-2.0, 0.5), near_foot=(51, ANKLE - 2),
                      near_toe=-0.3, near_planted=False, far_foot=(38, ANKLE), far_toe=0.2, near_hand=(63, 60),
                      far_hand=(55, 64), palm=True, staff=0.55, grip=0.45, hem_flare=2.5, hem_front=-2,
                      hem_wave=1.0, hem_amp=1.0, beard=0.7, eyes='squint', bare=True)
    body, at, orb = drawn(pose, 'body', 'staff')
    frames.append((stacked([(body, at), hat_off(pose, -1, -5, (6.0, 1.0))]), orb))
    # Reeling back on the far foot, the near foot kicking up; the staff flung
    # from his hand; the hat sailing off behind.
    pose = WizardPose(hip=(h[0] - 5, h[1] + 3), lean=-7.0, head=(-1.5, 1.0), near_foot=(53, ANKLE - 4),
                      near_toe=-0.6, near_planted=False, far_foot=(35, ANKLE), far_toe=0.3, near_hand=(60, 63),
                      near_palm=True, far_hand=(30, 68), hem_flare=3.5, hem_front=-1, hem_wave=2.0, hem_amp=1.2,
                      beard=0.9, eyes='squint', bare=True, staff_at=((60.0, 74.0), (84.0, 50.0)))
    body, at, _ = drawn(pose, 'body')
    stick, sat, orb = drawn(pose, 'staff')
    frames.append((stacked([(stick, sat), (body, at), hat_off(pose, -13, -13, (8.0, 3.0))]), orb))
    # He sits down hard, legs shooting forward, a hand behind him; the staff
    # strikes the ground ahead, the hat falls behind.
    pose = WizardPose(hip=(h[0] - 4, 86.0), lean=-6.0, head=(1.0, 2.0), near_foot=(58, ANKLE), near_toe=-0.9,
                      far_foot=(55, ANKLE), far_toe=-0.8, near_hand=(54, 74), far_hand=(29, 87), hem_front=9,
                      hem_wave=1.5, hem_amp=0.5, hem_rise=-1, beard=0.2, eyes='closed', bare=True)
    body, at, _ = drawn(pose, 'body')
    stick, sat, orb = drawn(WizardPose(staff_at=((62.0, 88.0), (90.0, 70.0))), 'staff')
    struck = grounded(stick, right=93)
    frames.append((stacked([struck, grounded(body, left=at[0]), hat_off(pose, -24, -8, (6.0, 6.0))]),
                   moved(orb, sat, struck[1])))
    # Rocking back: the torso falls, the legs come up; the hat lands behind.
    pose = WizardPose(hip=(h[0] - 2, 87.0), lean=-13.0, head=(-1.0, 3.0), near_foot=(61, ANKLE - 5), near_toe=-0.5,
                      near_planted=False, far_foot=(58, ANKLE - 3), far_toe=-0.4, far_planted=False,
                      near_hand=(56, 62), far_hand=(26, 86), hem_front=10, hem_wave=2.5, hem_amp=0.6, hem_rise=-1,
                      beard=-0.2, eyes='closed', bare=True)
    body, at, _ = drawn(pose, 'body')
    stick, orb = staff_down(46.0, 84.0, rise=2.0)
    frames.append((stacked([stick, grounded(body, left=at[0]), hat_landed(2, (5.0, 7.0))]), orb))
    # Flat on his back: the arms thrown up, the beard flipped, dust.
    stick, orb = staff_down(40.0, 86.0)
    out = stacked([stick, grounded(supine((63, 58), (59, 63), beard=-0.8), left=26),
                   hat_landed(2, (5.0, 6.0))])
    dust(out, (30, 50, 70), 'death-impact')
    frames.append((out, orb))
    # The rebound: the body lifts a pixel, the arms fall.
    out = stacked([stick, grounded(supine((59, 66), (44, 71), beard=-0.3), left=26, lift=1),
                   hat_landed(2, (4.0, 5.0))])
    dust(out, (26, 76), 'death-bounce', spread=1.6)
    frames.append((out, orb))
    # At rest: a hand on his belly, the beard over his chest, the toes up;
    # the hat upright behind his head, the staff along the ground.
    frames.append((stacked([stick, grounded(supine((54, 75), (40, 74), beard=0.1), left=26),
                            hat_landed(2, (4.0, 4.0))]), orb))
    frames = [(im, (round(o[0], 3), round(o[1], 3))) for im, o in frames]
    return frames, [100, 110, 110, 90, 80, 120, 300]


def toss_poses():
    hip = REST_HIP
    base = dict(staff=STAND_TILT, hem_amp=0.7)
    wind = WizardPose(hip=(hip[0] - 1, hip[1] + 1), lean=-0.5, near_hand=(60, 66), far_hand=(31, 76),
                      far_foot=(40, ANKLE), near_foot=(52, ANKLE), hem_flare=1.5, far_item='ball',
                      grip=stand_grip(66), **base)
    swing = WizardPose(hip=hip, lean=1.5, near_hand=(61, 66), far_hand=(47, 77), hem_front=1,
                       far_foot=(41, ANKLE), near_foot=(53, ANKLE), far_item='ball', grip=stand_grip(66), **base)
    release = WizardPose(hip=(hip[0] + 1, hip[1]), lean=3.0, near_hand=(61, 65), far_hand=(61, 63),
                         hem_front=2, far_foot=(41, ANKLE), near_foot=(54, ANKLE), near_toe=0.2, palm=True,
                         grip=stand_grip(65), **base)
    follow = WizardPose(hip=(hip[0] + 1, hip[1]), lean=2.5, near_hand=(61, 65), far_hand=(63, 57),
                        hem_front=1.5, far_foot=(41, ANKLE), near_foot=(53, ANKLE), near_toe=0.1,
                        grip=stand_grip(65), **base)
    return [wind, swing, release, follow], [70, 50, 60, 120]


AIMS = (90, 45, 0, -45, -90)


def staff_aim(degrees):
    """Standing, the staff levelled along the aim in both hands, the orb
    leading: the sprays' pose, held while they pour."""
    a = math.radians(degrees)
    neck = (REST_HIP[0] + 1.0, REST_HIP[1] - 17)
    shoulder = (neck[0] + 2, neck[1] + 3)
    ahead = 5.0 * max(0.0, math.sin(a))  # held clear of the face when raised
    hand = (shoulder[0] + ahead + math.cos(a) * 11.5, shoulder[1] - math.sin(a) * 11.5)
    back_hand = (shoulder[0] + ahead + math.cos(a) * 3.0 - 2, shoulder[1] - math.sin(a) * 3.0 + 3)
    d = (math.cos(a), -math.sin(a))
    top = add(hand, d, 21 if degrees > -60 else 14)
    foot = add(hand, d, -30 if degrees > -60 else -37)
    return WizardPose(near_hand=hand, far_hand=back_hand, staff_at=(foot, top), both_hands=True,
                      hem_amp=0.7, beard=0.1, hat_tip=(-1, 1), flare=True)


def gun_aim(degrees):
    """Standing, the staff upright in the near hand and the pistol in the far
    hand along the aim: the gun's pose."""
    a = math.radians(degrees)
    neck = (REST_HIP[0] + 1.0, REST_HIP[1] - 17)
    far_shoulder = (neck[0] - 1, neck[1] + 4)
    hand = (far_shoulder[0] + math.cos(a) * 12.0, far_shoulder[1] - math.sin(a) * 12.0)
    return WizardPose(far_hand=hand, far_item='pistol', aim=a, grip=stand_grip(),
                      hem_amp=0.7)


def cast_bolt():
    """A missile or a bolt: the staff drawn back over the shoulder, thrust
    at the aim with the orb flaring, then recovered."""
    hip = REST_HIP
    draw = WizardPose(hip=(hip[0] - 1, hip[1]), lean=-1.0, near_hand=(52, 60), far_hand=(40, 66),
                      staff_at=((60.0, 74.0), (33.0, 44.0)), near_foot=(52, ANKLE), far_foot=(39, ANKLE),
                      hem_flare=1.5, beard=0.2, hat_tip=(2, 1))
    thrust = WizardPose(hip=(hip[0] + 2, hip[1] + 1), lean=4.0, near_hand=(65, 62), far_hand=(55, 64),
                        staff_at=((44.0, 66.0), (88.0, 56.0)), near_foot=(56, ANKLE), far_foot=(38, ANKLE),
                        far_toe=0.3, hem_flare=4, hem_front=2, hem_wave=1.0, hem_amp=1.2, beard=-0.3,
                        hat_tip=(-3, 2), flare=True, both_hands=True)
    recover = WizardPose(hip=(hip[0] + 1, hip[1]), lean=2.0, near_hand=(62, 64), far_hand=(45, 70),
                         staff=0.55, grip=0.45, near_foot=(54, ANKLE), far_foot=(40, ANKLE),
                         hem_flare=2, hem_front=1, hem_wave=2.0, beard=-0.1, hat_tip=(-1, 1))
    return [draw, thrust, recover], [80, 120, 100]


def cast_push():
    """The force push or the black hole: the free hand gathers at the chest,
    then shoves out palm first, the staff raised behind in the other."""
    hip = REST_HIP
    gather = WizardPose(hip=(hip[0] - 1, hip[1]), lean=-0.5, near_hand=(54, 64), far_hand=(42, 62),
                        near_palm=True, staff_behind=True, staff_at=((36.0, 88.0), (31.0, 36.0)),
                        near_foot=(52, ANKLE), far_foot=(39, ANKLE), hem_flare=1.0, beard=0.15)
    shove = WizardPose(hip=(hip[0] + 2, hip[1]), lean=3.5, near_hand=(64, 61), far_hand=(42, 60),
                       near_palm=True, staff_behind=True, staff_at=((40.0, 86.0), (30.0, 33.0)),
                       near_foot=(55, ANKLE), far_foot=(38, ANKLE), far_toe=0.3, hem_flare=3.5,
                       hem_front=1.5, hem_wave=1.2, hem_amp=1.2, beard=-0.35, hat_tip=(-3, 2), flare=True)
    hold = replace(shove, near_hand=(63, 62), lean=3.0, hem_wave=2.0, beard=-0.2, flare=False)
    return [gather, shove, hold], [80, 120, 100]


def cast_conjure():
    """Iron, the balls or a summon: the staff raised high in both hands, the
    orb flaring, then struck down on the ground."""
    hip = REST_HIP
    raise_ = WizardPose(hip=(hip[0], hip[1] - 1), lean=0.5, near_hand=(54, 51), far_hand=(48, 54),
                        staff=0.05, grip=0.35, both_hands=True, near_foot=(51, ANKLE), far_foot=(41, ANKLE),
                        hem_amp=0.8, beard=0.1, hat_tip=(-1, 1), flare=True)
    strike = WizardPose(hip=(hip[0] + 1, hip[1] + 2), lean=3.0, near_hand=(60, 67), far_hand=(55, 69),
                        staff=0.12, grip=(WIZARD_SOLE - 67) / STAFF, both_hands=True, near_foot=(53, ANKLE),
                        far_foot=(40, ANKLE), hem_flare=1.5, hem_front=2, hem_wave=1.0, hem_amp=1.4,
                        beard=0.35, hat_tip=(1, 3), flare=True)
    settle = WizardPose(grip=stand_grip(), hem_wave=2.0, beard=0.1)
    return [raise_, strike, settle], [100, 90, 110]


def staff_poses():
    """The sprays' pose at each of [`AIMS`], from straight up to straight
    down; 01 shows the one nearest the aim, so the clip only holds them."""
    return [staff_aim(degrees) for degrees in AIMS], [100] * len(AIMS)


def aim_poses():
    """The gun's pose at each of [`AIMS`]; 01 shows the one nearest the aim."""
    return [gun_aim(degrees) for degrees in AIMS], [100] * len(AIMS)


POSED_CLIPS = {
    # clip: (pose and duration source, looping)
    'player_idle': (idle_poses, True),
    'player_walk': (walk_poses, True),
    'player_jump': (jump_poses, False),
    'player_fall': (fall_poses, True),
    'player_land': (land_poses, False),
    'player_double_jump': (double_jump_poses, False),
    'player_hurt': (hurt_poses, False),
    'player_toss': (toss_poses, False),
    'player_bolt': (cast_bolt, False),
    'player_push': (cast_push, False),
    'player_conjure': (cast_conjure, False),
    'player_staff': (staff_poses, False),
    'player_aim': (aim_poses, False),
}
# Clips drawn frame by frame, not posed through `render` alone.
DRAWN_CLIPS = {
    'player_death': (death_frames, False),
}
ORB = {PALETTE[c] for c in 'JKLZ'}  # the orb's glass and its glint


def check_frame(name, im, anchor):
    """Feet on the baseline, the lying frames' too; the staff light's anchor
    inside the frame, on the orb's glass."""
    if im.getbbox()[3] != WIZARD_FEET:
        raise ValueError(f'{name}: lowest pixel must sit on row {WIZARD_FEET - 1}')
    x, y = anchor
    if not (0 <= x < WIZARD and 0 <= y < WIZARD):
        raise ValueError(f'{name}: the staff light\'s anchor {anchor} lies outside the frame')
    if im.getpixel((int(x), int(y))) not in ORB:
        raise ValueError(f'{name}: the staff light\'s anchor {anchor} is off the orb')


def frame_name(clip, index):
    # The idle clip starts on the registry's start sprite, `player`.
    return 'player' if clip == 'player_idle' and index == 0 else f'{clip}_{index}'


def posed(poses_for):
    """A posed clip's frames rendered, each with its anchor, and their durations."""
    poses, durations = poses_for()
    return [render(pose) for pose in poses], durations


def build_player(images, lit, animations):
    """The wizard's clips, every frame lit; returns each right-facing
    frame's staff light anchor, the orb's centre, by sprite name."""
    clips = [(clip, posed(poses_for), looping) for clip, (poses_for, looping) in POSED_CLIPS.items()]
    clips += [(clip, frames_for(), looping) for clip, (frames_for, looping) in DRAWN_CLIPS.items()]
    anchors = {}
    for clip, (rendered, durations), looping in clips:
        frames = []
        for i, ((im, anchor), duration) in enumerate(zip(rendered, durations, strict=True)):
            name = frame_name(clip, i)
            check_frame(name, im, anchor)
            images[name] = im
            lit.add(name)
            anchors[name] = list(anchor)
            frames.append({'sprite': 'ex10_' + name, 'duration_ms': duration})
        animations[clip] = {'looping': looping, 'frames': frames}
    return anchors


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


# The fire missile (01 (h)): a rocket of fire facing +x, its own clip apart
# from the fire hazards' fireball. Its nose sits on the fireball's centre row
# and 25 px ahead of it, so the prefab's offsets and glow stay and the hit
# point, `MISSILE_RADIUS` ahead of the quad's centre, lies inside the body;
# scaled to fill the 40 px quad `toys::MISSILE_SIZE` draws it at.
MISSILE_FRAMES = 10
MISSILE_TIP = (57.0, 35.0)
MISSILE_SCALE = 1.45


def missile_shape(u, v, t, k=1.0):
    """The palette symbol at `u` pixels behind the tip and `v` across the
    axis (down positive), at phase `t` of the flicker, or the plume's heat
    as `('heat', h)`; None outside. `k` scales the whole missile: a white-hot
    nose cone, a body lit along its top and shaded beneath with a glowing
    band, two swept fins, a dark nozzle, and behind it a flame plume that
    licks and cools."""
    u, v = u / k, v / k
    if u < -0.4:
        return None
    a = abs(v)
    body = 3.4
    if u < 6:
        half = body * (max(u, 0.0) / 6) ** 0.7 + 0.3
        if a <= half:
            return 'Z' if a < half * 0.5 or u < 1.5 else 'Y'
    elif u < 21:
        if a <= body:
            if 10.5 <= u <= 12:
                return 'Y'  # the glowing band
            if v < -2.2:
                return 'J'
            if v < -0.8:
                return 'K'
            if v < 1.6:
                return 'L'
            return 'M'
        # Swept fins: the leading edge from the body at u 14 out to 4 px
        # more at u 20.5, the trailing edge square.
        if 14 <= u <= 20.5 and a <= body + (u - 14) * 0.62:
            return 'L' if v < 0 else 'M'
    elif u < 22.5:
        if a <= body - 0.6:
            return 'M'  # the nozzle
    # The plume: hot at the nozzle, widening, cooling and licking behind.
    if 21 <= u <= 46:
        s = (u - 21) / 25
        lick = 1.4 * math.sin(u * 0.55 - t * math.tau * 3) + 0.8 * math.sin(u * 1.3 + t * math.tau * 5 + 1.1)
        half = 2.6 + 3.2 * s + lick * (0.25 + s * 0.8)
        if a <= half:
            heat = 1.0 - 0.95 * s + 0.25 * (1 - a / max(half, 0.6)) - 0.1
            return ('heat', heat)
    return None


def missile_symbol(heat, x, y):
    """A shape's symbol, the plume's heat dithered onto the fire ramp."""
    if heat is None:
        return None
    if isinstance(heat, str):
        return heat
    heat = heat[1] + (BAYER[y % 4][x % 4] / 16 - 0.5) * 0.12
    if heat < 0.06:
        return None
    for limit, s in ((0.18, 'M'), (0.34, 'L'), (0.52, 'K'), (0.7, 'J'), (0.86, 'Y')):
        if heat < limit:
            return s
    return 'Z'


def missile_frame(index):
    """One 64 px frame of the missile in flight, facing +x, with embers shed
    from its plume cooling as they fall back."""
    t = index / MISSILE_FRAMES
    canvas = {}
    for y in range(SIZE):
        for x in range(SIZE):
            s = missile_symbol(missile_shape(MISSILE_TIP[0] - (x + 0.5), y + 0.5 - MISSILE_TIP[1], t,
                                             k=MISSILE_SCALE), x, y)
            if s:
                canvas[(x, y)] = s
    for k in range(6):
        age = (t + k * 0.17) % 1
        ex = round(MISSILE_TIP[0] - 34 - age * 18 - unit('ember', k) * 4)
        ey = round(MISSILE_TIP[1] + (unit('ember-y', k) - 0.5) * 10 + age * 3)
        if 0 <= ex < SIZE and 0 <= ey < SIZE and (ex, ey) not in canvas:
            canvas[(ex, ey)] = 'J' if age < 0.4 else 'K' if age < 0.75 else 'L'
    return to_image(canvas)


def build_fire_missile(images, animations):
    # Self-lit, as the fireball is: no normal or emissive maps.
    frames = []
    for i in range(MISSILE_FRAMES):
        name = f'fire_missile_{i}'
        images[name] = missile_frame(i)
        frames.append({'sprite': 'ex10_' + name, 'duration_ms': 60})
    animations['fire_missile'] = {'looping': True, 'frames': frames}


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


# The crawler (e01foes' redraw, V19): alien, low and many-legged, its five
# frost-blue plates graded from a moonlit top-left to a dark underside.

CRAWLER_FRAMES = 6
CRAWLER_LIGHT = (-0.6, -0.7, 0.5)
# Its ramps, dark to light: the carapace and head; a leg's upper segment,
# lit along its top, and its shin; the far side's legs; the crystals' lit
# and shadowed facets; the snow it packs (`Ѣ ѣ Ѳ Ѵ`, white-blue).
FROST_PLATE = ('|', '{', ')', '}', 'ѳ', 'ѵ', '_')
FROST_FEMUR = ('|', '{', ')', '}', 'ѳ')
FROST_SHIN = ('х', '|', '{')
FROST_FAR = ('х', 'х', '|', '{')
ICE_LIT = ('ѵ', '_', '`', 'Ψ')
ICE_DARK = ('{', ')', '}', 'ѳ')
SNOW = ('Ѣ', 'ѣ', 'Ѳ', 'Ѵ', 'Ψ')
# Its plates, tail to head: centre and radii.
CRAWLER_PLATES = ((12.0, 50.5, 6.0, 5.0), (19.5, 49.0, 6.5, 6.0), (28.0, 48.0, 7.0, 6.5),
                  (36.5, 48.0, 7.0, 6.0), (44.5, 49.0, 6.5, 5.5))
# Its crystals, on the plates they grow from: the plate, its offset along
# it, its height, its width and its lean.
CRAWLER_CRYSTALS = ((1, 0.0, 7, 4, -1.5), (2, -1.0, 11, 5, -0.5), (2, 3.5, 5, 3, 2.0), (3, 0.5, 8, 4, 1.0),
                    (4, -1.0, 4, 3, 1.5))
# Each leg under its body, tail to head: its hip, how far out its knee and
# foot reach and how high its knee rises over the hip; the end pairs arch
# higher and wider, so the legs fan out below the plates.
CRAWLER_LEGS = ((13, 5, 8, 5.5), (22, 4, 7, 2.5), (31, 4, 7, 2.5), (40, 6, 10, 5.5))
CRAWLER_HIP = 54.5
CRAWLER_FEMUR = (2.3, 2.0, 1.7)
CRAWLER_SHIN = (1.6, 1.2, 1.0)


@dataclass
class CrawlerPose:
    """One frame of the crawler, facing right; pixels unless named."""
    phase: float = 0.0  # its legs' stride through a cycle
    gait: float = 1.0  # its feet swing forward (1), back (-1) or stay planted (0)
    back: float = 0.0  # its body drawn back over its feet
    rise: float = 0.0  # its body raised, pressed low when negative
    tilt: float = 0.0  # its front dropped, raised when negative
    flare: float = 0.0  # its plates raised off each other
    spread: float = 0.0  # its plates drawn apart, the leap's length
    legs: str = 'stand'  # stand, coil, reach, splay, spring, tuck or land
    knees: float = 0.0  # its knees raised
    head: tuple = (0.0, 0.0)  # its head moved
    pitch: float = 0.0  # its head turned, up when negative (radians)
    gape: float = 0.0  # its jaws parted
    eyes: str = 'calm'  # calm, lit or slit
    feelers: float = 0.0  # swept back flat at 1, forward when negative
    sting: float = 0.0  # its sting raised
    shiver: int = 0  # a pixel's tremble, back
    glint: int = 0  # the walk's step, which moves its crystals' glint
    clump: float = 0.0  # a clump of snow in its jaws, its radius
    flecks: int = 0  # snow flung from its jaws, how far it has flown


def crawler_leg(pose, i, near, at):
    """Leg `i` (tail to head) on the near or far side: its hip, knee and
    foot. `at(x)` is the body's rise at column x."""
    hip_x, reach, stride, high = CRAWLER_LEGS[i]
    hip_x -= 0 if near else 2
    out = -1 if i < 2 else 1  # the hind pair's knees and feet point back
    hip = (hip_x - pose.back - pose.shiver + pose.spread * (i - 1.5), CRAWLER_HIP - at(hip_x) - (0 if near else 1.5))
    ground = SOLE if near else SOLE - 1
    k = (i + (0 if near else 1)) % 2  # two alternating sets of four
    angle = math.tau * (pose.phase + 0.5 * k)
    swing = 3.0 * pose.gait * math.sin(angle)
    lift = 2.5 * max(0.0, math.cos(angle)) if pose.gait else 0.0
    x, y = hip
    if pose.legs == 'stand':
        knee = (x + out * reach + swing * 0.5, y - high - lift * 0.6 - pose.knees)
        foot = (x + out * stride + swing, ground - lift)
    elif pose.legs == 'coil':  # bunched under it, knees high
        knee = (x + out * 3.5, y - high - 2 - pose.knees)
        foot = (x + out * 5, ground)
    elif pose.legs == 'reach':  # forelegs flung ahead, hind legs trailing on the ground
        if out > 0:
            knee = (x + 6, y - high)
            foot = (x + 12 + 2 * (i - 2), ground - 5 - 2 * (i - 2))
        else:
            knee = (x - 5, y - high + 1)
            foot = (x - 12, ground)
    elif pose.legs == 'splay':  # low and spread, landing
        knee = (x + out * 7, y - high * 0.5)
        foot = (x + out * 12, ground)
    elif pose.legs == 'spring':  # pushed off: legs straight down and back
        knee = (x + out * 2 - 2, y + 1)
        foot = (x + out * 2 - 5, ground)
    elif pose.legs == 'tuck':  # folded up under it, aloft
        knee = (x + out * 5, y - 2)
        foot = (x + out * 2, y + 3)
    else:  # land: unfolding for the landing, forelegs ahead
        knee = (x + out * 6, y - high * 0.5)
        foot = (x + out * 8 + (1 if out > 0 else -1), y + 5)
    # Every knee and foot inside the frame, its outline too.
    knee, foot = ((min(60.5, max(2.5, q[0])), q[1]) for q in (knee, foot))
    return hip, knee, foot


def crawler_head(pose, at):
    """Its head's points, turned by its pitch about its neck: the wedge, its
    two mandibles, its eyes, the jaws' point and its crest; and its centre."""
    hx = 52 - pose.back - pose.shiver + pose.head[0] + 0.5 * pose.spread
    hy = 49 - at(52) + pose.head[1]
    neck = (hx - 5, hy)

    def place(points):
        return turned([(hx + x, hy + y) for x, y in points], neck, pose.pitch)

    wedge = place([(-6, -5), (-4, -6), (-2, -8.5), (0, -6), (2, -7), (3.5, -4.5), (8.5, -0.5), (8, 1), (4, 4),
                   (-6, 5)])
    g = pose.gape
    # Pincers: each mandible reaching ahead and hooking in at its tip.
    jaws = [place([(6, side * 1.5), (8.2, side * (2 + g)), (9.3, side * (1.2 + g)), (9.6, side * (0.4 + g * 0.7))])
            for side in (-1, 1)]
    eyes = {(dx, dy): point(place([(dx, dy)])[0]) for dx, dy in ((2, -2), (4, -1), (1, 0), (3, 1))}
    mouth = place([(10, 0)])[0]
    crest = place([(-2, -8), (2, -6.5), (-4, -5.5)])
    return wedge, jaws, eyes, mouth, crest, (hx, hy)


def crawler_canvas(pose=None):
    """Five carapace plates, each graded from its moonlit top-left to its
    shadowed underside in a few dithered steps, a pale ridge along its top
    and a dark seam behind it with the cold glow showing through; faceted
    ice crystals on its back, a lit and a shadowed facet each, one catching
    a glint; eight jointed legs stepping in two sets of four, the end pairs
    high-kneed, their upper segments lit along the top and their shins dark;
    a wedge head with a crest, four glowing eyes and ice pincers; long
    jointed feelers; a crystal sting. `CrawlerPose` poses it."""
    p = pose or CrawlerPose()
    bob = round(0.6 * abs(math.sin(math.tau * p.phase))) if p.gait and p.legs == 'stand' else 0

    def at(x):  # the body's rise at column x: raised, its front dropped by the tilt
        return bob + p.rise - p.tilt * (x - 30) / 24

    shift = p.back + p.shiver
    far = [crawler_leg(p, i, False, at) for i in range(4)]
    near = [crawler_leg(p, i, True, at) for i in range(4)]
    far_part = Part()
    for hip, knee, foot in far:
        far_part.polygon(strand([hip, lerp(hip, knee, 0.5), knee], [2.0, 1.8, 1.6]))
        far_part.polygon(strand([knee, lerp(knee, foot, 0.5), foot], [1.4, 1.2, 1.0]))
    layers = [(grade(far_part.pixels(), FROST_FAR, CRAWLER_LIGHT, depth=0.8), None)]
    # The sting: an ice tail curling up over its rear to a barb that points
    # ahead, curled higher in a scare and a coil.
    tx, ty = 9 - shift - 2 * p.spread, 51.5 - at(8)
    r = p.sting
    back = min(tx - 3.6, 6.0)  # held inside the frame when the body draws back
    curl = [(tx, ty), (tx - 0.7 * back, ty - 2), (tx - back, ty - 6.5 - 0.4 * r), (tx - 0.75 * back, ty - 10.5 - r),
            (tx - 0.2 * back, ty - 12.5 - r), (tx + 1.5, ty - 12 - r)]
    sting = Part()
    sting.polygon(strand(curl, [4.6, 3.8, 3.0, 2.4, 1.6, 0.6]))
    layers.append((grade(sting.pixels(), ICE_DARK + ICE_LIT[1:3], CRAWLER_LIGHT, bias=0.15), 'х'))
    canvas = compose(layers)
    plates = []
    for i, (cx, cy, rx, ry) in enumerate(CRAWLER_PLATES):
        x = cx - shift + p.spread * (i - 2)
        low = 1.0 if p.legs == 'splay' else 0.0
        plates.append((x, cy - at(cx) - 0.8 * p.flare + low, rx + low * 0.5, ry + 0.6 * p.flare - low))
    for i, (x, y, rx, ry) in enumerate(plates):
        part = Part()
        part.ellipse((x, y), rx, ry)
        inside = part.pixels()
        colors = grade(inside, FROST_PLATE, CRAWLER_LIGHT, fall=0.3, bias=0.08)
        for (px, py) in inside:  # a pale ridge along its top, brightest toward the back
            if (px, py - 1) not in inside:
                colors[(px, py)] = '_' if px < x - rx * 0.2 else 'ѵ' if px < x + rx * 0.5 else colors[(px, py)]
        for (px, py), symbol in colors.items():  # rime catching here and there on its back
            if py < y - 2 and symbol in ('}', 'ѳ') and unit('crawler-rime', i, round(px - x), round(py - y)) < 0.05:
                colors[(px, py)] = 'ѵ'
        if i:  # its seam: two dark pixels over the plate behind, the cold glow showing through
            edge = [(px, py) for px, py in inside if (px - 1, py) not in inside]
            for px, py in edge:
                for k in (1, 2):
                    if (px - k, py) in canvas and canvas[(px - k, py)] != 'o':
                        glow = k == 2 and y - 2 <= py <= y + 3
                        canvas[(px - k, py)] = ',' if glow else 'х'
        canvas = compose([(canvas, None), (colors, None)])
    # Its crystals: each a pointed prism, a lit facet and a shadowed one.
    shards = []
    for k, (plate, along, h, w, lean) in enumerate(CRAWLER_CRYSTALS):
        x, y, rx, ry = plates[plate]
        bx = x + along
        top = y - ry * math.sqrt(max(0.0, 1 - (along / rx) ** 2)) + 2
        h += 0.6 * p.flare
        tip = (bx + lean, top - h)
        mid = (bx + lean * 0.35, top + 1)
        lit, dark = Part(), Part()
        lit.polygon([(bx - w / 2, top + 1), (bx - w / 2 + 0.3, top - h * 0.5), tip, mid])
        dark.polygon([tip, (bx + w / 2 - 0.2, top - h * 0.45), (bx + w / 2, top + 1), mid])
        lit_px = lit.pixels() - dark.pixels()
        colors = grade(dark.pixels(), ICE_DARK, CRAWLER_LIGHT, depth=1.0, bias=0.05)
        colors.update(grade(lit_px, ICE_LIT, CRAWLER_LIGHT, depth=1.0, bias=0.1))
        for (cx, cy) in colors:  # a dark edge along its sides, none under its foot
            for dx, dy in NEIGHBOURS:
                q = (cx + dx, cy + dy)
                if q in canvas and q not in colors and q[1] < top:
                    canvas[q] = 'х'
        canvas.update(colors)
        shards.append((point(tip), lit_px))
    wedge, jaws, eyes, mouth, crest, (hx, hy) = crawler_head(p, at)
    # The feelers: jointed, from the crown, swept up and back, or laid flat.
    feelers = []
    for base, reach in (((-1, -5), (1, -13, -5, -20)), ((2, -5), (6, -12, 4, -19))):
        b = (hx + base[0], hy + base[1])
        up = ((hx + reach[0], hy + reach[1]), (hx + reach[2], hy + reach[3]))
        flat = ((hx - 6 + base[0], hy - 9), (hx - 17 + base[0], hy - 8.5 - 0.5 * base[0]))
        u = max(0.0, p.feelers)
        mid, end = lerp(up[0], flat[0], u), lerp(up[1], flat[1], u)
        if p.feelers < 0:  # quivering forward, alert
            mid, end = (mid[0] - 2 * p.feelers, mid[1]), (end[0] - 5 * p.feelers, end[1] - p.feelers)
        feelers.append(turned([b, mid, end], (hx - 5, hy), p.pitch * 0.5))
    feeler_part = Part()
    for points in feelers:
        feeler_part.limb(points, 1)
    head, jaw = Part(), Part()
    head.polygon(wedge)
    for points in jaws:
        jaw.polygon(strand(points, [1.8, 1.6, 1.2, 0.8]))
    layers = [(canvas, None), ({q: '{' for q in feeler_part.pixels()}, None),
              (grade(head.pixels(), FROST_PLATE, CRAWLER_LIGHT, fall=0.2, bias=0.05), 'х')]
    if p.clump:  # packed snow held between its jaws
        ball = Part()
        ball.ellipse(mouth, p.clump, p.clump * 0.9)
        layers.append((grade(ball.pixels(), SNOW, CRAWLER_LIGHT, bias=0.15), 'х'))
    layers.append((grade(jaw.pixels(), ('{', ')', 'ѳ', '_', '`'), CRAWLER_LIGHT, depth=0.8, bias=0.1), 'х'))
    femurs, shins, knees = Part(), Part(), Part()
    for hip, knee, foot in near:
        femurs.polygon(strand([hip, lerp(hip, knee, 0.5), knee], CRAWLER_FEMUR))
        shins.polygon(strand([knee, lerp(knee, foot, 0.55), foot], CRAWLER_SHIN))
        knees.ellipse(knee, 1.0, 1.0)
    layers += [(grade(shins.pixels(), FROST_SHIN, CRAWLER_LIGHT, depth=0.8), 'х'),
               (grade(femurs.pixels(), FROST_FEMUR, CRAWLER_LIGHT, depth=0.8, bias=-0.05), 'х'),
               (grade(knees.pixels(), FROST_FEMUR, CRAWLER_LIGHT, depth=0.8, bias=0.15), None)]
    canvas = compose(layers)
    for hip, knee, foot in near + far:  # a foot on the ground stands on the sole
        fx, fy = point(foot)
        if fy >= SOLE - 1:
            canvas.setdefault((fx, fy), 'х')
    for points in feelers:  # a joint every few pixels, a pale tip
        canvas[point(points[-1])] = '`'
        for t in (0.45, 0.75):
            q = point(lerp(points[1], points[2], t))
            if q in canvas:
                canvas[q] = '}'
    for q in (point(c) for c in crest):  # the crest's pale ridge
        if q in canvas:
            canvas[q] = '_'
    if p.eyes == 'slit':  # squinting to a dim slit
        for dx in (1, 2, 3, 4):
            canvas[point(turned([(hx + dx, hy - 1)], (hx - 5, hy), p.pitch)[0])] = '`'
    else:
        for q in eyes.values():  # dark sockets, then the eyes glowing in them
            for nx, ny in NEIGHBOURS:
                n = (q[0] + nx, q[1] + ny)
                if canvas.get(n) in FROST_PLATE[1:]:
                    canvas[n] = '|'
        for (dx, dy), q in eyes.items():
            canvas[q] = 'Ψ' if p.eyes == 'lit' or (dx, dy) == (4, -1) else '`'
    # The glint: a crystal catches the moon, a step along the crystals each
    # pair of frames, flickering between two of its pixels.
    tip, lit = shards[(1, 3, 0)[(p.glint // 2) % 3]]
    edge = sorted((q for q in lit if (q[0] + 1, q[1]) not in lit), key=lambda q: (q[1], q[0]))
    if edge:
        canvas[edge[min(len(edge) - 1, 1 + 2 * (p.glint % 2))]] = 'Ψ'
    canvas[tip] = 'Ψ'
    if p.flecks:  # loose snow shaken from its jaws as the head jerks
        mx, my = mouth
        for j, (dx, dy) in enumerate(((0.5, -2), (-1.5, -3), (1, 2), (-2.5, -1.5))):
            q = point((min(62, mx + dx * p.flecks), my + dy * p.flecks))
            if j < 2 + p.flecks:
                canvas[q] = 'Ψ' if j % 2 == 0 else 'Ѳ'
    return outline({q: s for q, s in canvas.items() if 0 <= q[0] < SIZE and 0 <= q[1] <= SOLE})


def crawler_frames(poses, durations):
    return [to_image(crawler_canvas(p)) for p in poses], durations


def crawler_walk_frames():
    """Its legs stepping in two sets of four, its body rising between, a
    crystal's glint stepping along its back."""
    return crawler_frames([CrawlerPose(phase=i / CRAWLER_FRAMES, glint=i) for i in range(CRAWLER_FRAMES)],
                          [90] * CRAWLER_FRAMES)


SCARED = dict(gait=0.0, back=3, tilt=2.5, flare=1.0, legs='coil', knees=1, head=(-2, 1), feelers=1.0,
              eyes='slit', sting=4)


def crawler_windup_frames():
    """The lunge's tell: it coils over its 0.4 s, drawn back and pressed low
    at the front, knees high, sting raised, jaws parting, every eye lit; then
    trembles, set."""
    poses = [CrawlerPose(gait=0.0, back=round(4 * t), tilt=3 * t, rise=-t, legs='coil', knees=4 * t,
                         head=(-2 * t, 0), gape=t, eyes='lit' if t >= 0.5 else 'calm', feelers=-0.5 * t,
                         sting=7 * t, shiver=s, glint=2)
             for t, s in ((0.35, 0), (0.7, 0), (1.0, 0), (1.0, 1))]
    return crawler_frames(poses, [100, 100, 100, 100])


def crawler_lunge_frames():
    """The leap: it stretches out long and raised, forelegs flung ahead,
    hind legs trailing, jaws wide, feelers swept back; the last held."""
    poses = [CrawlerPose(gait=0.0, spread=2 * t, rise=3 * t, tilt=-2 * t, legs='reach', head=(-1.5 * t, -t),
                         pitch=-0.12 * t, gape=3 * t, eyes='lit', feelers=1.0, sting=-1, glint=4)
             for t in (0.6, 1.0, 1.0)]
    return crawler_frames(poses, [60, 80, 400])


def crawler_recover_frames():
    """The landing: low and splayed, then gathering itself."""
    poses = [CrawlerPose(gait=0.0, rise=-3 * t, legs='splay' if t > 0.5 else 'stand', head=(0, 1 * t),
                         feelers=0.3 * t, glint=1)
             for t in (1.0, 0.55, 0.2)]
    return crawler_frames(poses, [100, 120, 140])


def crawler_scared_frames():
    """Its scare at the light's edge: recoiled over its planted feet, low at
    the front, plates raised, feelers laid flat, jaws shut, its eyes a slit,
    its sting raised, trembling a pixel."""
    return crawler_frames([CrawlerPose(shiver=s, glint=g, **SCARED) for s, g in ((0, 0), (1, 1), (0, 2), (1, 3))],
                          [90, 70, 90, 70])


def crawler_retreat_frames():
    """Backing out of the light, facing what it fears: its feet stepping
    back in two sets of four, the body leaning back over its hind legs, low
    at the front, plates raised, feelers laid flat, its eyes a slit and its
    sting raised, trembling as it goes."""
    poses = [CrawlerPose(phase=i / CRAWLER_FRAMES, gait=-1.0, back=2, tilt=2.0, flare=1.0, head=(-2, 1),
                         feelers=1.0, eyes='slit', sting=5, shiver=i % 2, glint=i)
             for i in range(CRAWLER_FRAMES)]
    return crawler_frames(poses, [80] * CRAWLER_FRAMES)


def crawler_hop_frames():
    """A hop or a leap: a spring off its bunched legs; aloft, legs tucked
    under it and feelers swept back; then held tucked, its legs unfolding
    for the landing."""
    poses = [CrawlerPose(gait=0.0, rise=3, tilt=-1.5, legs='spring', feelers=0.6, eyes='lit', sting=1, glint=1),
             CrawlerPose(gait=0.0, rise=1, tilt=-1, spread=-0.5, legs='tuck', feelers=1.0, eyes='lit', sting=-1,
                         glint=3),
             CrawlerPose(gait=0.0, rise=0, tilt=0.5, legs='land', feelers=0.8, eyes='lit', glint=3)]
    return crawler_frames(poses, [70, 90, 240])


def crawler_snowball_windup_frames():
    """The snowball's tell, 0.36 s: its head dips and its jaws gather frost
    into a clump, packed tighter each frame, then the head cocks back with
    the clump clamped, set to flick."""
    poses = [CrawlerPose(gait=0.0, tilt=1.0, legs='coil', head=(-1, 2), pitch=0.25, gape=2, clump=1.0, glint=0),
             CrawlerPose(gait=0.0, tilt=1.2, legs='coil', head=(-1.5, 2), pitch=0.2, gape=2, clump=2.0, glint=1),
             CrawlerPose(gait=0.0, tilt=0.6, legs='coil', knees=1, head=(-2.5, 0), pitch=0.0, gape=1.5, clump=2.8,
                         eyes='lit', glint=2),
             CrawlerPose(gait=0.0, back=1, tilt=0.0, legs='coil', knees=2, head=(-3, -1), pitch=-0.25, gape=1.5,
                         clump=3.2, eyes='lit', sting=2, glint=3)]
    return crawler_frames(poses, [90] * 4)


def crawler_snowball_throw_frames():
    """The snowball's flick, 0.25 s: the head jerked up and ahead, jaws
    open, the snow leaving them at (66, 40); the follow-through; settling."""
    poses = [CrawlerPose(gait=0.0, tilt=-1.0, legs='stand', head=(0, -3.3), pitch=-0.35, gape=2.5, eyes='lit',
                         flecks=1, glint=4),
             CrawlerPose(gait=0.0, tilt=-0.6, legs='stand', head=(-1, -4), pitch=-0.5, gape=3, eyes='lit',
                         flecks=2, glint=5),
             CrawlerPose(gait=0.0, tilt=0.0, legs='stand', head=(-1, -1), pitch=-0.15, gape=1, glint=0)]
    return crawler_frames(poses, [50, 80, 120])


# The beetle: a rhinoceros beetle of riveted iron, rusting.

BEETLE_FRAMES = 6
BEETLE_IRON = dict(base='Ω', light='χ', shadow='Φ', rim='ψ', back='B')
IRON_TONES = ('Ω', 'χ', 'Φ', 'ψ', 'B', 'b', 'φ')
# e01foes: how far its front rises at the height of its rear (pixels at its
# head), from column `BEETLE_PIVOT` over its planted hind feet, and how far a
# charge drops it; its raking foot's reach and lift through the tell's eight
# frames, a scrape twice; the earth its scrapes throw; its legs' blur.
BEETLE_REAR = 10.0
BEETLE_LEAN = 2.0
BEETLE_PIVOT = 14
RAKE = ((5, 6), (7, 1), (3, 0), (-3, 0), (5, 7), (7, 1), (3, 0), (-4, 0))
CLODS = ('ω', '"', "'")
SMEAR = ('ѐ', 'ё', 'ι')


def sheared(part, lift):
    """`part` with each column raised `lift(x)` whole pixels: a tilt that
    keeps its pixels crisp."""
    out = Part(part.size)
    for x in range(part.size[0]):
        rise = lift(x)
        column = part.mask.crop((x, 0, x + 1, part.size[1]))
        out.mask.paste(column, (x, -rise))
    return out


def horn_glint(canvas, tip, size):
    """A star of moonlight on the horn's tip, `size` 0 to 1: a white core,
    its arms reaching further and fading, a sparkle across them at its
    fullest; drawn over the outline, where nothing rims it."""
    reach = round(1 + 4 * size)
    x, y = tip
    canvas[(x, y)] = 'Ψ'
    fade = ('Ψ', 'φ', 'x', 'u', 'q')
    over = {'x': 'φ', 'u': 'χ', 'q': 'ψ'}  # over the horn itself, opaque

    def put(q, tone):
        if 0 <= q[0] < SIZE and 0 <= q[1] < SIZE:
            canvas[q] = over.get(tone, tone) if q in canvas and canvas[q] != 'o' else tone

    for k in range(1, reach + 1):
        tone = fade[min(len(fade) - 1, k - 1 + (0 if size > 0.5 else 1))]
        for dx, dy in NEIGHBOURS:
            put((x + dx * k, y + dy * k), tone)
    if size > 0.45:
        for dx, dy in ((1, 1), (-1, -1), (1, -1), (-1, 1)):
            put((x + dx, y + dy), 'x' if size > 0.75 else 'u')
    if size > 0.8:
        for dx, dy in ((2, 2), (-2, -2), (2, -2), (-2, 2)):
            if (x + dx, y + dy) not in canvas:
                put((x + dx, y + dy), 'q')


def beetle_canvas(pose='walk', phase=0.0, rear=0.0, glint=0.0, stripped=False):
    """A dome of banded wing cases with rows of punctures, a riveted shield
    with a short horn, the long ridged horn of its head, red eyes, a fanned
    antenna and mouthparts, legs with spines and hooked claws, belly plates,
    scratches, and rust flaking along its seams, rim and rivets. `pose` is
    `walk`, `paw` (the charge's tell: rearing by `rear`, 0 to 1, its front
    and horn rising over its planted hind feet as its shell tilts, a foreleg
    raking the ground through `phase` and throwing clods back, a glint
    swelling at its horn's tip by `glint`) or `charge` (head down and
    dropped, the horn levelled, its legs a blur, dust behind); `phase` steps
    its legs. `stripped` (e01flask): Corrosive has eaten the wing cases
    through to the soft body under them (`strip_shell`)."""
    dip = 4 if pose == 'charge' else 0
    amp = 4.5 if pose == 'charge' else 2.5
    far, near, dome, plate, head, horn, horn2, belly = (Part() for _ in range(8))
    span = SIZE - 6 - BEETLE_PIVOT

    def lift(x):  # its rise at column x: rearing, or dropped at the front in a charge
        u = min(1.0, max(0.0, (x - BEETLE_PIVOT) / span))
        if pose == 'charge':
            return -round(BEETLE_LEAN * u)
        return round(rear * BEETLE_REAR * u)

    def at(x, y):
        return (x, y - lift(x))

    def stride(k):  # (forward offset, lift) of leg set k's foot
        angle = math.tau * (phase + 0.5 * k)
        return amp * math.sin(angle), 2.0 * max(0.0, math.cos(angle))

    bob = round(0.6 * abs(math.sin(math.tau * phase))) if pose != 'paw' else 0
    for i, x in enumerate((17, 31, 43)):  # the far legs: sets B, A, B
        ahead, up = stride((i + 1) % 2) if pose != 'paw' else (0.0, 0.0)
        far.limb([at(x + 2, 52 - bob), (x - 2 + ahead * 0.5, 55 - bob - up * 0.5 - lift(x) * 0.5),
                  (x - 1 + ahead, SOLE - 2 - up)], 3)
    legs = []
    # A width-3 limb reaches a row past its points, so its feet sit a row up.
    for i, (hip, knee, foot) in enumerate((((21, 52), (15, 55), (16, SOLE - 1)),
                                          ((33, 53), (31, 56), (34, SOLE - 1)),
                                          ((45, 52), (50, 55), (51, SOLE - 1)))):
        if pose == 'paw' and i == 2:  # the raking foot: raised ahead, set down, dragged back
            reach, up = RAKE[int(phase * len(RAKE)) % len(RAKE)]
            legs.append([at(45, 51), (50 + reach // 2, 50 - up // 2 - lift(48) * 0.6), (51 + reach, SOLE - 1 - up)])
            continue
        ahead, up = stride(i % 2) if pose != 'paw' else (0.0, 0.0)
        legs.append([at(hip[0], hip[1] - bob), (knee[0] + ahead * 0.5, knee[1] - bob - up * 0.5 - lift(knee[0]) * 0.5),
                     (foot[0] + ahead, foot[1] - up)])
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
    if pose == 'charge':  # the horn levelled at what it charges, leaning ahead
        horn_path = [(hx + 2, hy - 2), (hx + 5, hy - 6), (hx + 6.5, hy - 11), (hx + 6.5, hy - 16), (hx + 5, hy - 19)]
    elif pose == 'paw':  # the horn swept back as the front rises
        horn_path = turned(horn_path, horn_path[0], -0.18 * rear)
    horn.polygon(strand(horn_path, [5, 4, 3, 2, 1]))
    if pose != 'walk':
        dome, belly, plate, horn2, head, horn = (sheared(p, lift) for p in (dome, belly, plate, horn2, head, horn))
    shell = dome.pixels()
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
            if canvas.get(at(x, y)) in ('Ω', 'χ', 'Φ', 'ψ'):
                canvas[at(x, y)] = 'Φ'
                if (y - top) % 6 == 3:
                    canvas[at(x + 1, y)] = 'χ'
                    rivets.append(at(x + 1, y))
    for bx in (19, 30):  # rows of punctures between the bands
        for y in range(top + 3, 50 - bob, 3):
            x = bx + round((y - top) * 0.18)
            if canvas.get(at(x, y)) == 'Ω':
                canvas[at(x, y)] = 'Φ'
    for x in range(8, 44):  # the cases' lower rim
        if canvas.get(at(x, 50 - bob), 'o') != 'o':
            canvas[at(x, 50 - bob)] = 'ψ' if x % 2 else 'Φ'
    for x in range(12, 43, 5):  # belly plates
        for y in (53 - bob, 54 - bob):
            if canvas.get(at(x, y)) in ('b', 'B'):
                canvas[at(x, y)] = 'o'
    for x, y in ((44, 39 + dip - bob), (48, 37 + dip - bob), (50, 43 + dip - bob)):  # the shield's rivets
        if at(x, y) in canvas:
            canvas[at(x, y)] = 'χ'
    for x, y in horn_path[1:4]:  # ridges along the horn
        if at(*point((x, y))) in canvas:
            canvas[at(*point((x, y)))] = 'Φ'
    for x0, y0 in ((12, 38 - bob), (22, 35 - bob), (33, 41 - bob)):  # scratches
        for t in range(3):
            if canvas.get(at(x0 + t, y0 + t)) in IRON_TONES:
                canvas[at(x0 + t, y0 + t)] = 'ψ'
    seams = {(bx + round((y - top) * 0.18), y) for bx in (14, 25, 35) for y in range(top, 53 - bob)}
    for (x, y), symbol in sorted(canvas.items()):  # rust along the seams and the rim
        if symbol not in IRON_TONES:
            continue
        y0 = y + lift(x)
        noise = (unit('rust-a', x // 3, (y0 + bob) // 3) + unit('rust-b', x // 2, (y0 + bob) // 2)) / 2
        near_seam = any((x + dx, y0) in seams for dx in (-1, 0, 1)) or 47 - bob <= y0 <= 51 - bob
        if noise + (0.22 if near_seam else 0.0) > 0.8:
            canvas[(x, y)] = 'a' if noise > 0.86 else ('(' if (x + y0 + bob) % 2 else '"')
    for x, y in rivets[::2]:  # streaks running down from rivets
        for t in range(1, 4):
            if canvas.get((x, y + t), 'o') != 'o':
                canvas[(x, y + t)] = '(' if t < 3 else '"'
    if stripped:
        strip_shell(canvas, shell, lambda x, y: (x, y + lift(x) + bob))
    for leg in legs:  # spines along the shins, hooked claws
        (kx, ky), (fx, fy) = leg[1], leg[2]
        for t in (0.35, 0.65):
            canvas[(round(kx + (fx - kx) * t) + 2, round(ky + (fy - ky) * t))] = 'ψ'
        canvas[(round(fx) + 2, round(fy))] = 'B'
    canvas[at(hx + 6, hy + 1)] = 'b'  # mouthparts
    canvas[at(hx + 6, hy + 2)] = 'B'
    for x, y in ((hx + 4, hy - 1), (hx + 6, hy - 2), (hx + 7, hy - 3), (hx + 7, hy - 1), (hx + 8, hy - 2)):
        canvas[at(x, y)] = 'Φ' if x < hx + 7 else 'ψ'  # a fanned antenna
    for (ex, ey), tone in (((hx + 1, hy - 2), 'z'), ((hx + 2, hy - 2), 'F'), ((hx + 1, hy - 1), 'F'),
                           ((hx + 2, hy - 1), 'F'), ((hx + 1, hy), 'E'), ((hx + 2, hy), 'z')):
        canvas[at(ex, ey)] = tone  # red eyes, glowing
    canvas[at(hx + 2, hy - 2)] = 'Ψ'
    canvas[at(hx + 3, hy - 1)] = 'E'
    if pose == 'charge':  # dust kicked up behind
        for k, (dx, dy) in enumerate(((-2, 0), (-5, -2), (-8, 1), (-4, -3), (-11, -1))):
            if (k + int(phase * 4)) % 2 == 0 or k < 2:
                canvas[(6 + dx, SOLE - 1 + dy)] = 'σ'
    elif pose == 'paw':  # clods the scrape throws back, flying and falling
        f = int(phase * len(RAKE)) % len(RAKE)
        for start, speed, rise, size in ((2, 5.0, 5.0, 3), (3, 6.5, 3.5, 2), (3, 3.5, 6.0, 2), (6, 5.5, 5.5, 3),
                                         (7, 7.0, 3.0, 2), (7, 4.0, 6.5, 2)):
            age = f - start + 0.7
            if not 0 < age <= 2.7:
                continue
            cx = 50 - speed * age
            cy = SOLE - 2 - rise * age + 2.4 * age ** 2
            if cy > SOLE - 1:
                continue
            for dx in range(size):
                for dy in range(min(size, 2)):
                    if size == 3 and (dx, dy) in ((0, 1), (2, 1)):
                        continue  # a lumpy clod, not a block
                    canvas[point((cx + dx, cy + dy))] = CLODS[2] if dy == 0 and dx < 2 else CLODS[(dx + start) % 2]
    if pose == 'walk':
        return outline(canvas)
    canvas = outline({q: s for q, s in canvas.items() if 0 <= q[1] <= SOLE})
    if pose == 'paw' and glint > 0:
        horn_glint(canvas, at(*point(horn_path[-1])), glint)
    if pose == 'charge':  # its near legs a blur: their last two places smeared behind them
        for back, tone in ((0.08, SMEAR[0]), (0.16, SMEAR[1]), (0.24, SMEAR[2])):
            ghosts = Part()
            for i, (hip, knee, foot) in enumerate((((21, 52), (15, 55), (16, SOLE - 1)),
                                                  ((33, 53), (31, 56), (34, SOLE - 1)),
                                                  ((45, 52), (50, 55), (51, SOLE - 1)))):
                angle = math.tau * (phase - back + 0.5 * (i % 2))
                ahead, up = amp * math.sin(angle), 2.0 * max(0.0, math.cos(angle))
                ghosts.limb([at(hip[0], hip[1] - bob),
                             (knee[0] + ahead * 0.5, knee[1] - bob - up * 0.5 - lift(knee[0]) * 0.5),
                             (foot[0] + ahead, foot[1] - up)], 2)
            for i, x in enumerate((17, 31, 43)):
                angle = math.tau * (phase - back + 0.5 * ((i + 1) % 2))
                ahead, up = amp * math.sin(angle), 2.0 * max(0.0, math.cos(angle))
                ghosts.limb([at(x + 2, 52 - bob), (x - 2 + ahead * 0.5, 55 - bob - up * 0.5 - lift(x) * 0.5),
                             (x - 1 + ahead, SOLE - 2 - up)], 2)
            for q in ghosts.pixels():
                if q not in canvas and q[1] <= SOLE:
                    canvas[q] = tone
    return canvas


# e01flask: what Corrosive leaves of a shell: the soft body under the
# wing cases, pale as a grub's, banded where its segments meet; verdigris
# crusting the torn edges and run down from them, dark to light.
SOFT_BODY = ('τ', 'σ', 'w')
VERDIGRIS = ('կ', 'հ', 'ձ', 'ղ', 'ճ')
ACID_CRUST = ('հ', 'ձ')


def strip_shell(canvas, shell, body):
    """Eats the wing cases through: where a slow noise in the body's own
    frame (`body` maps a pixel to it, so the holes ride every pose) runs
    high, the soft body shows, lit from above and banded by its segments;
    the shell round each hole is crusted green, the acid runs down from the
    holes' lower edges, and what is left is pitted and dulled."""
    inside = {q for q in shell if canvas.get(q, 'o') != 'o'}
    top = min((y for _, y in inside), default=0)
    tall = max(1, max((y for _, y in inside), default=1) - top)
    rim = {(x, y) for x, y in inside
           if any((x + dx, y + dy) not in inside for dx in (-2, 0, 2) for dy in (-2, 0, 2))}
    holes = set()
    for x, y in inside - rim:
        bx, by = body(x, y)
        if fbm(bx / 6.5, by / 5.0, 'beetle-eaten') > 0.58 and by < SOLE - 8:
            holes.add((x, y))
    for x, y in holes:
        bx, by = body(x, y)
        lit = 1.0 - (y - top) / tall + (0.3 if (x, y - 1) not in holes else 0.0)
        canvas[(x, y)] = 'S' if by % 4 == 0 else ramp(0.2 + 0.6 * lit, SOFT_BODY)
    for x, y in sorted(inside - holes):
        if any((x + dx, y + dy) in holes for dx, dy in NEIGHBOURS):
            canvas[(x, y)] = ACID_CRUST[(x + y) % 2]
        elif unit('beetle-pit', x, y) < 0.06:
            canvas[(x, y)] = 'Φ'
        elif canvas[(x, y)] in ('χ', 'ψ') and fbm(x / 4, y / 4, 'beetle-dull') > 0.5:
            canvas[(x, y)] = 'Ω'
    for x, y in sorted(holes):  # the acid run down from each hole's lower edge
        if (x, y + 1) in inside and (x, y + 1) not in holes and unit('beetle-drip', x, y) < 0.3:
            for k in range(1, 2 + int(unit('beetle-drip-len', x, y) * 3)):
                if (x, y + k) in inside and (x, y + k) not in holes:
                    canvas[(x, y + k)] = ACID_CRUST[1] if k < 2 else ACID_CRUST[0]


def beetle_walk_frames(stripped=False):
    """Its legs stepping in two alternating sets of three."""
    frames = [to_image(beetle_canvas('walk', i / BEETLE_FRAMES, stripped=stripped)) for i in range(BEETLE_FRAMES)]
    return frames, [110] * BEETLE_FRAMES


def beetle_paw_frames(stripped=False):
    """The charge's tell, 0.8 s: rearing, its front and horn rising over its
    planted hind feet and its shell tilting, a foreleg raking the ground
    twice and throwing clods back, a glint swelling at its horn's tip."""
    n = len(RAKE)
    frames = [to_image(beetle_canvas('paw', f / n, rear=min(1.0, 0.2 + 0.8 * f / (n - 2)), glint=(f + 1) / n,
                                     stripped=stripped))
              for f in range(n)]
    return frames, [100] * n


def beetle_charge_frames(stripped=False):
    """The charge: head down and dropped, horn levelled, legs a blur, dust
    behind."""
    return [to_image(beetle_canvas('charge', i / 4, stripped=stripped)) for i in range(4)], [60] * 4


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
    armoured: bool = False  # e01flask: the armoured troll's iron over the same body
    stripped: bool = False  # e01flask: an armoured troll's iron after Corrosive, eaten to remnants


# e01flask: the armoured troll's iron, dark to light, a step darker on the
# far side; rust along its seams and under its rivets.
IRON = ('-', '~', ':', ';', 'Ω', 'ψ', 'χ')
IRON_FAR = ('-', '-', '~', ':', ';', 'Ω')
RUST = ('п', 'к', '"', 'У')
MOON = (0.5, -0.62, 0.6)  # toward the moon, which lights the troll from its front and above


def moonlit(nx, ny, nz):
    """How brightly the moon lights a surface facing (`nx`, `ny`, `nz`): a
    soft diffuse light and a hard sheen, as polished iron takes it."""
    length = math.sqrt(nx * nx + ny * ny + nz * nz) or 1.0
    lx, ly, lz = MOON
    norm = math.sqrt(lx * lx + ly * ly + lz * lz)
    d = max(0.0, (nx * lx + ny * ly + nz * lz) / (length * norm))
    return 0.3 + 0.5 * d + 0.35 * d ** 6


def iron_tube(inside, a, b, symbols, spread=0.05):
    """A plate wrapped round a limb from `a` to `b`, lit as a cylinder about
    that axis."""
    dx, dy = b[0] - a[0], b[1] - a[1]
    length = math.hypot(dx, dy) or 1.0
    nx, ny = -dy / length, dx / length
    offsets = {(x, y): (x + 0.5 - a[0]) * nx + (y + 0.5 - a[1]) * ny for x, y in inside}
    half = max([abs(o) for o in offsets.values()] + [1.0]) + 0.3
    colors = {}
    for (x, y), o in offsets.items():
        s = max(-1.0, min(1.0, o / half))
        colors[(x, y)] = ramp(moonlit(nx * s, ny * s, math.sqrt(1 - s * s)), symbols, x, y, spread)
    return colors


def iron_dome(inside, centre, rx, ry, symbols, spread=0.05):
    """A domed plate (a helm's crown, a pauldron, a knee cop) about
    `centre`, lit as a ball."""
    colors = {}
    for x, y in inside:
        dx, dy = (x + 0.5 - centre[0]) / rx, (y + 0.5 - centre[1]) / ry
        nz = math.sqrt(max(0.05, 1 - dx * dx - dy * dy))
        colors[(x, y)] = ramp(moonlit(dx, dy, nz), symbols, x, y, spread)
    return colors


def rust_seam(colors, seam, keys, chance=0.4):
    """Rust along a seam: the plate's pixels on `seam`'s near side, in
    patches where a slow noise runs high, flecked in the rust ramp's dark
    steps, the odd one bright."""
    for x, y in sorted(seam):
        patch = fbm(x / 5, y / 5, repr(keys))
        if (x, y) in colors and unit(keys, x, y) < chance * 2.6 * max(0.0, patch - 0.42):
            colors[(x, y)] = RUST[0] if unit(keys, 'tone', x, y) < 0.45 else (
                RUST[1] if unit(keys, 'tone', x, y) < 0.85 else RUST[3])


def rivet(colors, x, y, keys, drip=True):
    """A rivet's head, its shadow under it and, now and then, a rust streak
    run down from it."""
    x, y = round(x), round(y)
    if (x, y) not in colors:
        return
    colors[(x, y)] = 'ψ'
    if (x - 1, y + 1) in colors:
        colors[(x - 1, y + 1)] = '~'
    if drip and unit(keys, x, y) < 0.5:
        for k in range(1, 2 + int(unit(keys, 'len', x, y) * 3)):
            if (x, y + 1 + k) in colors:
                colors[(x, y + 1 + k)] = RUST[1] if k < 2 else RUST[0]


def regions(pixels):
    """`pixels` split into their 4-connected pieces."""
    left, pieces = set(pixels), []
    while left:
        stack = [left.pop()]
        piece = set(stack)
        while stack:
            x, y = stack.pop()
            for dx, dy in NEIGHBOURS:
                q = (x + dx, y + dy)
                if q in left:
                    left.discard(q)
                    piece.add(q)
                    stack.append(q)
        pieces.append(piece)
    return pieces


def eaten(colors, local, seed, keep):
    """What Corrosive leaves of a plate (e01flask): its pixels where a slow
    noise in the plate's own frame (`local` maps a pixel to it, so the
    remnant rides the joint it is strapped to) stays under `keep` (a
    number, or a function of the frame's point); the torn edges crusted in
    the acid's green, runs of it dried down from the lower ones over
    whatever lies under, the rest pitted and dulled."""
    def keeps(q):
        u, v = local(q)
        return fbm(u, v, seed) < (keep(u, v) if callable(keep) else keep)
    kept = {q: s for q, s in colors.items() if keeps(q)}
    for blob in regions(set(kept)):  # no specks of iron left floating
        if len(blob) < 8:
            for q in blob:
                del kept[q]
    for blob in regions(set(colors) - set(kept)):  # no pinholes: the acid eats in bites
        if len(blob) < 6:
            kept.update({q: colors[q] for q in blob})
    gone = set(colors) - set(kept)
    out = dict(kept)
    for (x, y), s in kept.items():
        if any((x + dx, y + dy) in gone for dx, dy in NEIGHBOURS):
            out[(x, y)] = VERDIGRIS[3] if (x, y - 1) in gone else ACID_CRUST[(x + y) % 2]
            if (x, y + 1) in gone and unit(seed, 'run', x, y) < 0.45:
                for k in range(1, 3 + int(unit(seed, 'run-len', x, y) * 4)):
                    out[(x, y + k)] = VERDIGRIS[2] if k < 3 else VERDIGRIS[1]
        elif unit(seed, 'pit', x, y) < 0.07:
            out[(x, y)] = '-'
        elif s in ('χ', 'ψ') and fbm(x / 3, y / 3, (seed, 'dull')) > 0.45:
            out[(x, y)] = 'Ω'
    return out


def troll_armour(pose, P, chest, hx, hy, joints):
    """The armoured troll's iron over the troll's own skeleton, so every
    plate rides the joint it is strapped to: a dented helm with a nasal and
    cheek guards, its rim riveted; riveted pauldrons, a domed cap over two
    lames that follow the upper arm; a breastplate of overlapping bands,
    its fauld over the loincloth's top; bracers and greaves with knee cops;
    rust on the seams and run from the rivets. Each piece's colours by its
    layer's name, and the helm's pixels."""
    size = (BIG, BIG)
    c, up = pose.crouch, pose.breath
    out = {}

    def chest_frame(q):  # a screen pixel back in the trunk's own frame, before it leans and breathes
        y = q[1] - c
        for _ in range(2):
            share = max(0.0, min(1.0, (76 - y) / 40))
            y = q[1] - c + up * share
        return (q[0] - pose.lean * share, y)
    # Greaves and knee cops, from under the knee to the ankle.
    for side, symbols, widths, cop in (('far', IRON_FAR, [9, 10, 8], (3.5, 3)),
                                       ('near', IRON, [11, 12, 9], (4.5, 3.5))):
        knee, ankle = joints[side + '_knee'], joints[side + '_ankle']
        along = [add(knee, (ankle[0] - knee[0], ankle[1] - knee[1]), t) for t in (0.2, 0.55, 0.93)]
        part = Part(size)
        part.polygon(strand(along, widths))
        colors = iron_tube(part.pixels(), along[0], along[-1], symbols)
        seam = {(x, y) for x, y in colors if (x, y - 1) not in colors or (x, y + 1) not in colors}
        rust_seam(colors, seam, ('greave', side), 0.3)
        mid = along[1]
        rivet(colors, mid[0] + 2, mid[1] - 4, ('greave-rivet', side))
        rivet(colors, along[-1][0] + 2, along[-1][1] - 3, ('greave-rivet', side, 1), drip=False)
        out[side + '_greave'] = colors
        knob = Part(size)
        knob.ellipse((knee[0] + 1, knee[1]), cop[0], cop[1])
        colors = iron_dome(knob.pixels(), (knee[0] + 1, knee[1] - 1), cop[0] + 1, cop[1] + 1, symbols)
        rivet(colors, knee[0] + 1, knee[1], ('cop', side), drip=False)
        out[side + '_knee'] = colors
    # Bracers over the forearms, from below the elbow to the wrist.
    for side, symbols, widths in (('far', IRON_FAR, [8, 9, 8]), ('near', IRON, [10, 11, 10])):
        elbow, wrist = joints[side + '_elbow'], joints[side + '_wrist']
        along = [add(elbow, (wrist[0] - elbow[0], wrist[1] - elbow[1]), t) for t in (0.35, 0.65, 0.95)]
        part = Part(size)
        part.polygon(strand(along, widths))
        colors = iron_tube(part.pixels(), along[0], along[-1], symbols)
        band = Part(size)
        band.limb([along[1], along[1]], 1)
        seam = Part(size)
        seam.polygon(strand([add(along[1], (wrist[0] - elbow[0], wrist[1] - elbow[1]), -0.03),
                             add(along[1], (wrist[0] - elbow[0], wrist[1] - elbow[1]), 0.03)], [14, 14]))
        lines = {q for q in seam.pixels() if q in colors}
        for q in lines:
            colors[q] = symbols[0]
        rust_seam(colors, {(x, y + 1) for x, y in lines}, ('bracer', side), 0.45)
        rivet(colors, *along[0], ('bracer-rivet', side), drip=False)
        rivet(colors, *along[2], ('bracer-rivet', side, 1), drip=False)
        out[side + '_bracer'] = colors
    # Pauldrons: a domed cap on the shoulder over two lames that follow the
    # upper arm, the lower lame first so each plate overlaps the next.
    for side, symbols, cap, root in (('far', IRON_FAR, (6, 5), chest(65, 42)), ('near', IRON, (8, 6), chest(83, 45))):
        elbow = joints[side + '_elbow']
        arm = (elbow[0] - root[0], elbow[1] - root[1])
        layers = []
        for k, (t0, t1, w0, w1) in enumerate(((0.3, 0.48, cap[0] * 2 - 2, cap[0] * 2 - 4),
                                              (0.12, 0.32, cap[0] * 2, cap[0] * 2 - 2))):
            lame = Part(size)
            lame.polygon(strand([add(root, arm, t0), add(root, arm, t1)], [w0, w1]))
            colors = iron_tube(lame.pixels(), add(root, arm, t0), add(root, arm, t1), symbols)
            lower = {(x, y) for x, y in colors if (x, y + 1) not in colors}
            rust_seam(colors, lower, ('lame', side, k), 0.35)
            mid = add(root, arm, (t0 + t1) / 2)
            rivet(colors, mid[0] + arm[1] / math.hypot(*arm) * (w0 / 2 - 2),
                  mid[1] - arm[0] / math.hypot(*arm) * (w0 / 2 - 2), ('lame-rivet', side, k))
            layers.append(colors)
        dome = Part(size)
        top = (root[0], root[1] - 2)
        dome.ellipse(top, cap[0], cap[1])
        colors = iron_dome(dome.pixels(), (top[0] + 1, top[1] - 1), cap[0] + 1, cap[1] + 1, symbols)
        rim = {(x, y) for x, y in colors if (x, y + 1) not in colors or (x + 1, y) not in colors}
        rust_seam(colors, rim, ('pauldron', side), 0.3)
        for i in range(3):
            a = math.radians(200 + i * 55)
            rivet(colors, top[0] + math.cos(a) * (cap[0] - 2), top[1] + math.sin(a) * (cap[1] - 2),
                  ('pauldron-rivet', side, i), drip=i == 1)
        out[side + '_pauldron'] = layers + [colors]
    # The breastplate: a collar plate and four bands down the trunk, leaning
    # and breathing with it, and the fauld, two more bands over the hips.
    bounds = [39, 48, 56, 64, 72, 78, 84]
    plate = Part(size)
    plate.polygon([chest(61, 41), chest(68, 38), chest(85, 42), chest(88, 50), chest(87, 58), chest(83, 66),
                   chest(80, 73), (81, 76 + c), (83, 84 + c), (58, 84 + c), (59, 76 + c), chest(58, 68),
                   chest(57, 57), chest(59, 47)])
    inside = plate.pixels()
    rows = {}
    for x, y in inside:
        lo, hi = rows.get(y, (x, x))
        rows[y] = (min(lo, x), max(hi, x))

    def screen_y(yc):  # a chest row's screen row: the trunk breathes above the waist
        return yc + c - up * max(0.0, min(1.0, (76 - yc) / 40))
    edges = [screen_y(b) for b in bounds]
    colors, seams = {}, set()
    for x, y in inside:
        k = max(0, min(len(edges) - 2, sum(1 for e in edges[1:-1] if y + 0.5 >= e)))
        y0, y1 = edges[k], edges[k + 1]
        lo, hi = rows[y]
        s = max(-1.0, min(1.0, 2 * (x + 0.5 - lo) / max(1, hi - lo + 1) - 1))
        v = (y + 0.5 - y0) / max(1.0, y1 - y0)
        # Each lame lies under the one above: shadowed at its top, flat
        # across its middle, its rolled lower edge catching the moon.
        tilt = -0.25 if v > 0.82 else (0.45 if v < 0.22 and k > 0 else 0.05)
        colors[(x, y)] = ramp(moonlit(s * 0.8, tilt, math.sqrt(max(0.05, 1 - s * s))), IRON, x, y, 0.05)
        if k > 0 and y == math.floor(y0):
            seams.add((x, y))
    for q in seams:
        colors[q] = '-'
    rust_seam(colors, {(x, y + 1) for x, y in seams} | {(x, y + 2) for x, y in seams}, 'band-rust', 0.5)
    for k in range(len(bounds) - 1):  # a rivet at each band's front and back
        y = round((edges[k] + edges[k + 1]) / 2)
        if y in rows:
            lo, hi = rows[y]
            rivet(colors, hi - 3, y, ('band-rivet', k))
            rivet(colors, lo + 3, y, ('band-rivet', k, 'back'), drip=False)
    for i in range(4):  # a dent in the second band: a dark crease, its lip lit below
        x, y = point(chest(72 + i, 49 + i // 2))
        if (x, y) in colors:
            colors[(x, y)] = '~'
            if (x, y + 1) in colors:
                colors[(x, y + 1)] = 'ψ'
    out['breastplate'] = colors
    # The helm: a crown over the skull to a rim that falls from the brow to
    # the nape, the rim's band riveted; a nasal down the nose's bridge and a
    # cheek guard over the jaw, leaving the ember eye in the rim's shadow.
    def rim_y(x):
        return hy - 5 + (hx + 9 - x) * (6.0 / 21)
    crown = Part(size)
    crown.ellipse((hx - 1, hy - 2), 12, 11)
    crown_px = {(x, y) for x, y in crown.pixels() if y < rim_y(x) and x <= hx + 10}
    colors = iron_dome(crown_px, (hx + 1, hy - 5), 13, 12, IRON)
    band = {(x, y) for x, y in crown_px if y >= rim_y(x) - 2.2}
    for x, y in band:
        colors[(x, y)] = ramp(0.35 + 0.35 * ((x, y - 1) not in band), IRON, x, y, 0.2)
    rust_seam(colors, {(x, y) for x, y in band if (x, y + 1) not in crown_px}, 'helm-rim', 0.45)
    for i in range(6):
        rivet(colors, hx - 9 + i * 3.6, rim_y(hx - 9 + i * 3.6) - 1.4, ('helm-rivet', i), drip=i in (1, 4))
    for i in range(4):  # a dent in the crown
        x, y = round(hx - 4 + i), round(hy - 10 + (i % 2))
        if (x, y) in colors and (x, y + 1) in colors:
            colors[(x, y)] = '~'
            colors[(x, y + 1)] = 'χ' if i == 1 else 'ψ'
    out['helm'] = colors
    nasal = Part(size)
    nasal.polygon([(hx + 7, hy - 6), (hx + 10, hy - 6), (hx + 13, hy + 1), (hx + 11, hy + 2)])
    out['nasal'] = iron_tube(nasal.pixels(), (hx + 8.5, hy - 6), (hx + 12, hy + 1.5), IRON)
    cheek = Part(size)
    cheek.polygon([(hx - 5, hy - 1), (hx + 2, hy - 3), (hx + 2, hy + 5), (hx, hy + 9), (hx - 4, hy + 7)])
    colors = iron_tube(cheek.pixels(), (hx - 1.5, hy - 2), (hx - 1.5, hy + 9), IRON)
    rust_seam(colors, {(x, y) for x, y in colors if (x, y + 1) not in colors}, 'cheek', 0.5)
    rivet(colors, hx, hy + 1, 'cheek-rivet')
    out['cheek'] = colors
    if pose.stripped:
        strip_armour(out, pose, joints, hx, hy, chest_frame)
    return out


def strip_armour(out, pose, joints, hx, hy, chest_frame):
    """Corrosive's leavings of the armoured troll's iron (e01flask): each
    piece eaten in its own frame (`eaten`), the breastplate to its collar
    and a few torn bands, thinning down the trunk; the helm to a holed cap
    and a stub of its nasal, the cheek guard to a scrap; the pauldrons to a
    holed cap each, their lames nearly gone; ragged rings of the bracers
    and the greaves, the knee cops gone."""
    def along(a, b):  # a limb plate's frame: along its axis, across it
        dx, dy = b[0] - a[0], b[1] - a[1]
        length = math.hypot(dx, dy) or 1.0
        return lambda q: (((q[0] - a[0]) * dx + (q[1] - a[1]) * dy) / length / 4,
                          ((q[0] - a[0]) * -dy + (q[1] - a[1]) * dx) / length / 3)
    out['breastplate'] = eaten(out['breastplate'], lambda q: tuple(v / 10 for v in chest_frame(q)), 'strip-breast',
                               lambda u, v: 0.58 - 0.22 * min(1.0, max(0.0, (v * 10 - 39) / 45)))
    head = lambda q: ((q[0] - hx) / 5, (q[1] - hy) / 5)
    out['helm'] = eaten(out['helm'], head, 'strip-helm', 0.55)
    out['nasal'] = {q: s for q, s in out['nasal'].items() if q[1] < hy - 3}
    out['cheek'] = eaten(out['cheek'], head, 'strip-cheek', 0.42)
    for side in ('far', 'near'):
        knee, ankle = joints[side + '_knee'], joints[side + '_ankle']
        elbow, wrist = joints[side + '_elbow'], joints[side + '_wrist']
        out[side + '_greave'] = eaten(out[side + '_greave'], along(knee, ankle), ('strip-greave', side), 0.48)
        out[side + '_bracer'] = eaten(out[side + '_bracer'], along(elbow, wrist), ('strip-bracer', side), 0.48)
        out[side + '_knee'] = {}
        lames = out[side + '_pauldron']
        cap = lames[-1]
        centre = (sum(x for x, _ in cap) / max(1, len(cap)), sum(y for _, y in cap) / max(1, len(cap)))
        shoulder = lambda q, c=centre: ((q[0] - c[0]) / 3, (q[1] - c[1]) / 3)
        out[side + '_pauldron'] = [eaten(lame, shoulder, ('strip-lame', side, k), 0.4) for k, lame in
                                   enumerate(lames[:-1])] + [eaten(cap, shoulder, ('strip-cap', side), 0.54)]


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
    joints = {'near_elbow': pose.near_elbow, 'near_wrist': pose.near_wrist,
              'far_elbow': pose.far_elbow, 'far_wrist': pose.far_wrist}
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
        joints[side + '_knee'], joints[side + '_ankle'] = knee, end
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
    if pose.armoured:  # the helm presses the ear down and back under its rim
        P['ear'].polygon([(hx - 6, hy + 1), (hx - 20, hy - 1), (hx - 8, hy + 7)])
    else:
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
    armour = troll_armour(pose, P, chest, hx, hy, joints) if pose.armoured else {}

    def iron(*names):  # the armour's layers, each rimmed dark where it overlaps
        layers = []
        for name in names:
            piece = armour.get(name, [])
            layers += [(colors, 'o') for colors in (piece if isinstance(piece, list) else [piece])]
        return layers
    canvas = compose([
        (shade(P['far_arm'].pixels(), **TROLL_MOSS_FAR), None),
        *iron('far_bracer'),
        (shade(P['far_hand'].pixels(), **TROLL_MOSS_FAR), None),
        (shade(P['far_leg'].pixels(), **TROLL_MOSS_FAR), 'o'),
        *iron('far_greave', 'far_knee'),
        (shade(P['far_foot'].pixels(), **TROLL_MOSS_FAR), None),
        (shade(P['back_flap'].pixels(), '[', light='(', shadow='o'), None),
        (shade(P['glute'].pixels(), **TROLL_MOSS), None),
        (shade(P['far_shoulder'].pixels(), **TROLL_MOSS_FAR), None),
        *iron('far_pauldron'),
        (shade(P['torso'].pixels(), **TROLL_MOSS), None),
        (shade(P['neck'].pixels(), **TROLL_MOSS), None),
        (shade(P['near_leg'].pixels(), **TROLL_MOSS), 'o'),
        *iron('near_greave', 'near_knee'),
        (shade(P['near_foot'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['front_flap'].pixels(), **TROLL_HIDE), 'o'),
        (shade(P['strap'].pixels(), '"', light="'", shadow='('), None),
        *iron('breastplate'),
        (shade(P['ear'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['head'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['jaw'].pixels(), **TROLL_MOSS), 'o'),
        (shade(P['brow'].pixels(), 'T', light='g', shadow='N', rim='G'), None),
        (shade(P['nose'].pixels(), 'G', light='W', shadow='T', rim='m'), 'o'),
        (shade(P['tusks'].pixels(), 'w', light='Z', shadow='σ'), None),
        *iron('cheek', 'helm', 'nasal'),
        (shade(P['near_shoulder'].pixels(), **TROLL_MOSS), None),
        (shade(P['near_arm'].pixels(), **TROLL_MOSS), 'o'),
        *iron('near_bracer'),
        (shade(P['near_hand'].pixels(), **TROLL_MOSS), 'o'),
        *iron('near_pauldron'),
    ])
    helm = set(armour.get('helm', {})) | set(armour.get('cheek', {})) | set(armour.get('breastplate', {}))
    skin = set().union(*(P[n].pixels() for n in ('torso', 'near_leg', 'near_arm', 'head', 'neck', 'near_shoulder')))
    speckle(canvas, skin, 'wart', ['T'], 0.025, only=('g',))
    hx, hy = round(hx), round(hy)
    for x, y in ((hx + 3, hy - 3), (hx + 4, hy - 4), (hx + 7, hy - 3), (hx + 4, hy - 2)):
        canvas[(x, y)] = 'N'  # the eye's socket under the brow
    canvas[(hx + 5, hy - 3)] = 'J'  # a deep-set ember eye
    canvas[(hx + 6, hy - 3)] = 'K'
    canvas[(hx + 12, hy + 4)] = 'N'  # a nostril
    canvas[(hx + 13, hy + 4)] = 'T'
    if pose.armoured:  # the ear's inside, pressed back under the helm, and its ring
        for t in range(5):
            canvas[(hx - 9 - t * 2, hy + 2)] = 'T'
            canvas[(hx - 10 - t * 2, hy + 3)] = 'T'
        ring_at = (hx - 12, hy + 4)
    else:
        for t in range(4):  # the ear's inside
            canvas[(hx - 9 - t * 2, hy - 7 - t * 2)] = 'T'
            canvas[(hx - 8 - t * 2, hy - 6 - t * 2)] = 'T'
        ring_at = (hx - 10, hy - 6)
    for x in range(hx - 2, hx + 9):  # the mouth's line under the tusks
        if canvas.get((x, hy + 6)) in ('g', 'W', 'm', 'T'):
            canvas[(x, hy + 6)] = 'N'
    for dx, dy in ((0, 0), (1, 1), (0, 2), (-1, 1)):  # a ring in the ear
        canvas[(ring_at[0] + dx, ring_at[1] + dy)] = 'A'
    for i in range(8):  # mossy hair from the scalp down the back of the neck
        x, y = hx - 2 - i * 3, hy - 9 + i * 3 + (1 if i % 2 else 0)
        for t in range(4):
            for q, s in (((x - t // 2, y - t), 'D' if t < 2 else ('Q' if t < 3 else 'U')),
                         ((x - t // 2 + 1, y - t), 'D' if t < 3 else 'Q')):
                if q not in helm:
                    canvas[q] = s
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
    if not pose.armoured:  # a bone at the knot, under the fauld on the armoured troll
        for x in range(69, 75):
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


def troll_idle_frames(armoured=False, stripped=False):
    """Breathing: the chest rising, its hands swaying with it."""
    poses = [TrollPose(breath=b, near_elbow=(91, 68 - b), near_wrist=(94 - b * 0.5, 91 - b),
                       far_elbow=(52, 67 - b), far_wrist=(52, 89 - b)) for b in (0, 1, 1, 0)]
    return [to_image(troll_canvas(replace(p, armoured=armoured or stripped, stripped=stripped)), (BIG, BIG)) for p in poses], [260, 220, 260, 220]


def troll_walk_frames(armoured=False, stripped=False):
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
    return [to_image(troll_canvas(replace(p, armoured=armoured or stripped, stripped=stripped)), (BIG, BIG)) for p in poses], [120] * TROLL_WALK


# The arms locked overhead, as the sheet's wind-up draws them: what the slam
# swings down from and the throw flings forward.
OVERHEAD = dict(lean=-3, lift=4, near_elbow=(92, 20), near_wrist=(81, 7), far_elbow=(53, 20),
                far_wrist=(68, 7), hands='clasped')


def troll_windup_frames(armoured=False, stripped=False):
    """The slam's wind-up, 0.8 s since 01 (g): its arms rising before it to
    lock overhead, leaning back."""
    poses = [TrollPose(lean=-1, lift=1, near_elbow=(96, 62), near_wrist=(110, 50), far_elbow=(80, 62),
                       far_wrist=(98, 52), hands='fists'),
             TrollPose(lean=-2, lift=3, near_elbow=(100, 34), near_wrist=(98, 14), far_elbow=(86, 32),
                       far_wrist=(88, 12), hands='fists'),
             TrollPose(**OVERHEAD)]
    return [to_image(troll_canvas(replace(p, armoured=armoured or stripped, stripped=stripped)), (BIG, BIG)) for p in poses], [133, 200, 467]


def troll_slam_frames(armoured=False, stripped=False):
    """The slam: its fists swinging down to the ground before it, a deep
    crouch, dust."""
    poses = [TrollPose(lean=3, lift=2, near_elbow=(98, 30), near_wrist=(110, 22), far_elbow=(90, 30),
                       far_wrist=(102, 20), hands='fists'),
             TrollPose(lean=8, crouch=3, near_elbow=(102, 58), near_wrist=(114, 76), far_elbow=(96, 60),
                       far_wrist=(108, 78), hands='fists'),
             TrollPose(lean=12, crouch=7, near_elbow=(104, 86), near_wrist=(110, 106), far_elbow=(98, 88),
                       far_wrist=(104, 106), hands='fists', dust=True)]
    return [to_image(troll_canvas(replace(p, armoured=armoured or stripped, stripped=stripped)), (BIG, BIG)) for p in poses], [60, 60, 260]


def troll_pickup_frames(armoured=False, stripped=False):
    """Lifting something: bending to the ground, then raising it overhead."""
    poses = [TrollPose(lean=7, crouch=4, near_elbow=(98, 78), near_wrist=(104, 96), far_elbow=(90, 78),
                       far_wrist=(98, 96)),
             TrollPose(lean=12, crouch=8, near_elbow=(102, 86), near_wrist=(108, 100), far_elbow=(96, 86),
                       far_wrist=(102, 100)),
             TrollPose(**dict(OVERHEAD, lean=-2, lift=3))]
    return [to_image(troll_canvas(replace(p, armoured=armoured or stripped, stripped=stripped)), (BIG, BIG)) for p in poses], [140, 200, 200]


def troll_throw_frames(armoured=False, stripped=False):
    """A throw: locked overhead, flung forward, the arms following through."""
    poses = [TrollPose(**OVERHEAD),
             TrollPose(lean=5, lift=2, near_elbow=(96, 28), near_wrist=(110, 30), far_elbow=(88, 26),
                       far_wrist=(102, 26), hands='fists'),
             TrollPose(lean=9, crouch=2, near_elbow=(98, 62), near_wrist=(108, 80), far_elbow=(92, 62),
                       far_wrist=(102, 78))]
    return [to_image(troll_canvas(replace(p, armoured=armoured or stripped, stripped=stripped)), (BIG, BIG)) for p in poses], [140, 80, 260]


def troll_swipe_windup_frames(armoured=False, stripped=False):
    """The swipe's tell: the near arm drawn back low behind the hip, coiled."""
    poses = [TrollPose(lean=-1, crouch=2, near_elbow=(80, 62), near_wrist=(68, 78), far_elbow=(60, 64),
                       far_wrist=(70, 86)),
             TrollPose(lean=-3, crouch=4, near_elbow=(72, 58), near_wrist=(54, 70), far_elbow=(62, 62),
                       far_wrist=(76, 80), sway=1)]
    return [to_image(troll_canvas(replace(p, armoured=armoured or stripped, stripped=stripped)), (BIG, BIG)) for p in poses], [120, 180]


def troll_swipe_frames(armoured=False, stripped=False):
    """The swipe: the near arm raked under and up in front of it, claws
    open, pale streaks along the hand's arc, then following through high."""
    poses = [TrollPose(lean=4, crouch=5, near_elbow=(94, 70), near_wrist=(102, 92), far_elbow=(54, 66),
                       far_wrist=(48, 84), sway=-2),
             TrollPose(lean=7, crouch=3, near_elbow=(101, 78), near_wrist=(112, 64), far_elbow=(52, 64),
                       far_wrist=(44, 80), sway=-2),
             TrollPose(lean=4, crouch=1, lift=1, near_elbow=(98, 42), near_wrist=(104, 26), far_elbow=(54, 66),
                       far_wrist=(50, 86), sway=-1)]
    frames = [troll_canvas(replace(p, armoured=armoured or stripped, stripped=stripped)) for p in poses]
    centre = (88, 60)
    for f, (start_deg, end_deg) in ((frames[0], (150, 78)), (frames[1], (128, -8))):
        for radius, symbol in ((38, 'τ'), (43, 'σ'), (48, 'w'), (53, 'σ')):
            steps = 70
            for s in range(steps):
                a = math.radians(start_deg + (end_deg - start_deg) * s / steps)
                x = round(centre[0] + math.cos(a) * radius)
                y = round(centre[1] + math.sin(a) * radius)
                if 0 <= x < BIG and 0 <= y < BIG - 4 and (x, y) not in f and (s + radius) % 5 != 0:
                    f[(x, y)] = symbol
    return [to_image(f, (BIG, BIG)) for f in frames], [60, 70, 200]


# The goblin (01 (g)): a sapper on a skeleton like the troll's, small and
# quick, on a 64 frame with its feet on row 60, about 41 px from its cap's
# crown to its soles.

GOB_SKIN = dict(base='G', light='m', shadow='g', rim='W', back='T')
GOB_SKIN_FAR = dict(base='g', light='G', shadow='T', rim='G', back='N')
GOB_LEATHER = dict(base='(', light='a', shadow='[', rim="'")
GOB_CAP = dict(base='"', light="'", shadow='ω', rim="'")
GOB_CLOTH = dict(base='~', light=':', shadow='=', rim=':')
GOB_CLOTH_FAR = dict(base='=', light='~', shadow='-')
GOB_ROCK = dict(light='ψ', shadow='Φ', back='B', rim='χ')
GOB_BOMB = dict(light='Ω', shadow='θ', rim='Φ')
G_THIGH, G_SHIN = 6.0, 6.0
G_FOOT_NEAR = [(-2, -1), (2, -1), (6, 0.5), (8, 2.2), (8, 3), (-2.5, 3)]
G_FOOT_FAR = [(-2, -1), (1.5, -1), (5, 0.5), (7, 2.2), (7, 3), (-2.5, 3)]
G_PALM = [(-1.5, 0), (1.5, 0), (2, 2.5), (-2, 2.5)]
G_FINGERS = [[(-1.5, 2), (-2, 4.5)], [(0, 2.5), (0, 5)], [(1.6, 2), (2.2, 4.4)]]
G_FIST = [(-2, -0.5), (2, -0.5), (2.5, 2), (0, 3.2), (-2.5, 2)]
G_THINGS = 16  # the rock's and the bomb's frame


def lerp(a, b, u):
    return tuple(x + (y - x) * u for x, y in zip(a, b))


@dataclass
class GoblinPose:
    """One frame of the goblin sapper, facing right. Positions are frame
    pixels."""
    crouch: float = 0.0  # hips and all above them drop this far
    lean: float = 0.0  # chest and head ahead of the hips
    breath: float = 0.0
    near_ankle: tuple = (35.0, 57.0)
    far_ankle: tuple = (28.0, 57.0)
    near_toe: float = 0.0
    far_toe: float = 0.0
    near_elbow: tuple = (38.5, 42.0)
    near_wrist: tuple = (39.5, 48.0)
    far_elbow: tuple = (28.0, 41.5)
    far_wrist: tuple = (27.5, 47.5)
    near_hand: str = 'open'  # open, fist, rock, bomb or knife
    far_hand: str = 'open'
    ears: float = 0.0  # the tips rise (alert) or droop
    mouth: str = 'grin'  # grin, open, smirk (sly) or scream
    blink: bool = False
    eye: str = 'mean'  # mean, sly (half-lidded) or wild (wide, a pinprick pupil)
    spark: int = 0  # a held bomb's fuse frame


def goblin_hand(part, wrist, elbow, kind):
    if kind in ('fist', 'rock', 'bomb', 'knife'):
        part.polygon(in_hand(wrist, elbow, G_FIST))
    else:
        part.polygon(in_hand(wrist, elbow, G_PALM))
        for bones in G_FINGERS:
            part.limb(in_hand(wrist, elbow, bones), 2)


def goblin_canvas(pose=None):
    """A goblin sapper on a skeleton: a big round skull thrust forward under
    a leather cap with brass goggles pushed up; a long hooked nose with a
    wart; a round yellow eye with a slit pupil under a heavy brow; a grin of
    fangs over an underbite; long leaf-shaped ears swept back, their insides
    dark, a notch bitten from one and a brass ring under its lobe; skinny
    arms with knobby elbows and long clawed fingers; an open leather jerkin
    over a pot belly, laced and stitched; a rope belt with a pouch of rocks
    at the front and a bomb hanging at the back; ragged shorts, bowed thin
    shins and long clawed feet; warts."""
    pose = pose or GoblinPose()
    c, lean, e = pose.crouch, pose.lean, pose.ears
    names = ('far_leg', 'far_shin', 'far_foot', 'far_arm', 'far_hand', 'far_ear', 'bomb', 'torso', 'belly',
             'neck', 'near_leg', 'near_shin', 'near_foot', 'belt', 'pouch', 'near_ear', 'head', 'jaw', 'cap',
             'nose', 'near_arm', 'near_hand', 'held')
    P = {n: Part() for n in names}

    def chest(x, y):
        share = max(0.0, min(1.0, (46 - y) / 12))
        return (x + lean * share, y + c - pose.breath * share)

    ankles = {}
    for side, hip, ankle, toe, foot, thigh, shin in (
            ('far', (29.5, 45 + c), pose.far_ankle, pose.far_toe, G_FOOT_FAR, [5, 4.5, 3.6], [2.2, 2.6, 2]),
            ('near', (32.5, 46 + c), pose.near_ankle, pose.near_toe, G_FOOT_NEAR, [5.5, 5, 4], [2.6, 3, 2.2])):
        knee, end = two_bone(hip, ankle, G_THIGH, G_SHIN, 1)
        P[side + '_leg'].polygon(strand([hip, lerp(hip, knee, 0.55), lerp(hip, knee, 0.9)], thigh))
        P[side + '_shin'].ellipse(knee, 1.5, 1.5)
        P[side + '_shin'].polygon(strand([knee, lerp(knee, end, 0.5), end], shin))
        P[side + '_foot'].polygon(turned([(end[0] + x, end[1] + y) for x, y in foot], end, toe))
        ankles[side] = (end, knee)
    P['torso'].polygon([chest(27, 46), chest(25.5, 41), chest(26, 37), chest(29, 34), chest(34.5, 33),
                        chest(38, 35), chest(40, 39.5), chest(39.5, 44.5), chest(35, 47)])
    P['belly'].polygon([chest(35.5, 37), chest(38, 36), chest(40.5, 40), chest(40, 44.5), chest(36, 46.5),
                        chest(34.5, 41.5)])
    hx, hy = 37.5 + lean, 25.5 + c - pose.breath * 0.6
    P['neck'].polygon([chest(31, 34.5), (hx - 4, hy + 3), (hx + 1, hy + 5.5), chest(36, 34.5)])
    P['head'].ellipse((hx, hy), 7.5, 7)
    if pose.mouth == 'scream':  # the jaw dropped wide
        P['jaw'].polygon([(hx - 1, hy + 3), (hx + 7.5, hy + 3), (hx + 8.5, hy + 8), (hx + 4.5, hy + 10.5),
                          (hx - 2, hy + 8.5)])
    else:
        P['jaw'].polygon([(hx - 1, hy + 3), (hx + 7.5, hy + 2.5), (hx + 8.5, hy + 5.5), (hx + 5, hy + 8),
                          (hx - 2, hy + 7)])
    P['nose'].polygon([(hx + 6, hy - 2), (hx + 10.5, hy + 0.5), (hx + 14, hy + 3.8), (hx + 13.5, hy + 5.4),
                       (hx + 11, hy + 4.4), (hx + 7, hy + 2.8)])
    P['cap'].polygon([(hx - 8, hy - 1), (hx - 7.5, hy - 5), (hx - 4.5, hy - 8.6), (hx + 0.5, hy - 9.8),
                      (hx + 5, hy - 8.4), (hx + 7.8, hy - 5.4), (hx + 10.5, hy - 4.6), (hx + 7, hy - 3.8),
                      (hx - 6.5, hy - 2.4)])
    P['near_ear'].polygon([(hx - 4, hy - 3), (hx - 10, hy - 5.5 - e * 0.4), (hx - 16, hy - 8 - e * 0.8),
                           (hx - 22, hy - 10.5 - e * 1.3), (hx - 18, hy - 5.5 - e * 0.7),
                           (hx - 11, hy - 0.5), (hx - 5, hy + 3)])
    P['far_ear'].polygon([(hx - 3, hy - 6.5), (hx - 8, hy - 11 - e * 0.5), (hx - 13.5, hy - 15.5 - e * 1.2),
                          (hx - 10.5, hy - 9.5 - e * 0.6), (hx - 5, hy - 3.5)])
    P['belt'].polygon(strand([chest(25.5, 43.5), chest(33, 45), chest(40.5, 43.5)], [2, 2.5, 2]))
    P['pouch'].ellipse(chest(37.5, 47.2), 3.2, 2.8)
    P['bomb'].ellipse(chest(25, 47.5), 2.8, 2.8)
    hands = {}
    for side, shoulder, elbow, wrist, kind in (
            ('far', chest(28.5, 36), pose.far_elbow, pose.far_wrist, pose.far_hand),
            ('near', chest(35.5, 36.5), pose.near_elbow, pose.near_wrist, pose.near_hand)):
        arm = P[side + '_arm']
        arm.polygon(strand([shoulder, lerp(shoulder, elbow, 0.5), elbow], [3.4, 2.8, 2.4]))
        arm.ellipse(elbow, 1.4, 1.4)
        arm.polygon(strand([elbow, lerp(elbow, wrist, 0.5), wrist], [2.2, 2.6, 2.4]))
        goblin_hand(P[side + '_hand'], wrist, elbow, kind)
        hands[side] = (wrist, elbow, kind)
    wrist, elbow, kind = hands['near']
    if kind in ('rock', 'bomb'):  # held in the fingers' curl, ahead of the palm
        hold = in_hand(wrist, elbow, [(0, 3.6)])[0]
        if kind == 'rock':
            P['held'].polygon([(hold[0] + x, hold[1] + y) for x, y in
                               ((-3, -1), (-1, -3.5), (2.5, -3), (3.5, 0), (1.5, 3), (-2.5, 2.5))])
        else:
            P['held'].ellipse(hold, 3.4, 3.4)
    elif kind == 'knife':  # a short blade out of the fist, along the forearm
        P['held'].polygon(in_hand(wrist, elbow, [(-1.1, 3), (1.1, 3), (0.8, 8.5), (0, 10.5), (-0.8, 8.5)]))
    canvas = compose([
        (shade(P['far_arm'].pixels(), **GOB_SKIN_FAR), None),
        (shade(P['far_hand'].pixels(), **GOB_SKIN_FAR), None),
        (shade(P['far_ear'].pixels(), **GOB_SKIN_FAR), None),
        (shade(P['far_leg'].pixels(), **GOB_CLOTH_FAR), None),
        (shade(P['far_shin'].pixels(), **GOB_SKIN_FAR), None),
        (shade(P['far_foot'].pixels(), **GOB_SKIN_FAR), None),
        (shade(P['bomb'].pixels(), 'b', **GOB_BOMB), 'o'),
        (shade(P['torso'].pixels(), **GOB_LEATHER), 'o'),
        (shade(P['belly'].pixels(), **GOB_SKIN), None),
        (shade(P['neck'].pixels(), **GOB_SKIN), None),
        (shade(P['near_leg'].pixels(), **GOB_CLOTH), 'o'),
        (shade(P['near_shin'].pixels(), **GOB_SKIN), None),
        (shade(P['near_foot'].pixels(), **GOB_SKIN), 'o'),
        (shade(P['belt'].pixels(), 'τ', light='σ', shadow='s'), None),
        (shade(P['pouch'].pixels(), '"', light="'", shadow='ω', rim="'"), 'o'),
        (shade(P['near_ear'].pixels(), **GOB_SKIN), 'o'),
        (shade(P['head'].pixels(), **GOB_SKIN), 'o'),
        (shade(P['jaw'].pixels(), **GOB_SKIN), None),
        (shade(P['cap'].pixels(), **GOB_CAP), 'o'),
        (shade(P['nose'].pixels(), 'G', light='m', shadow='g', rim='W'), 'o'),
        (shade(P['near_arm'].pixels(), **GOB_SKIN), 'o'),
        (shade(P['near_hand'].pixels(), **GOB_SKIN), 'o'),
        (shade(P['held'].pixels(), 'Ω', **GOB_ROCK) if kind == 'rock'
         else shade(P['held'].pixels(), 'S', light='σ', shadow=';', rim='σ') if kind == 'knife'
         else shade(P['held'].pixels(), 'b', **GOB_BOMB), 'o'),
    ])
    skin = set().union(*(P[n].pixels() for n in ('belly', 'head', 'near_shin', 'near_arm', 'neck')))
    speckle(canvas, skin, 'gob-wart', ['g'], 0.035, only=('G',))
    hx, hy = round(hx), round(hy)
    # The brow: a heavy ridge slanting down to the nose, a mean look; a
    # round yellow eye under it, its pupil a slit, a glint; shut in a blink.
    for x, y in ((hx, hy - 3), (hx + 1, hy - 3), (hx + 2, hy - 3), (hx + 3, hy - 3), (hx + 4, hy - 3),
                 (hx + 5, hy - 2)):
        canvas[(x, y)] = 'T'
    eye = {(hx + 2, hy - 2): 'y', (hx + 3, hy - 2): 'Ψ', (hx + 4, hy - 2): 'y',
           (hx + 2, hy - 1): 'Y', (hx + 3, hy - 1): 'o', (hx + 4, hy - 1): 'y',
           (hx + 2, hy): 'J', (hx + 3, hy): 'o', (hx + 4, hy): 'J'}
    if pose.blink:
        for x in (hx + 1, hx + 2, hx + 3, hx + 4, hx + 5):
            canvas[(x, hy - 1)] = 'T'
    elif pose.eye == 'sly':  # the lid drawn down, the slit pupil slid ahead to its mark
        for x in range(hx, hx + 6):
            canvas[(x, hy - 2)] = 'T'
        canvas.update({(hx + 2, hy - 1): 'y', (hx + 3, hy - 1): 'Y', (hx + 4, hy - 1): 'o',
                       (hx + 2, hy): 'J', (hx + 3, hy): 'y', (hx + 4, hy): 'o'})
        for q in ((hx + 1, hy - 1), (hx + 1, hy), (hx + 5, hy - 1), (hx + 5, hy), (hx + 2, hy + 1),
                  (hx + 3, hy + 1), (hx + 4, hy + 1)):
            canvas[q] = 'N' if q[1] <= hy else 'g'
    elif pose.eye == 'wild':  # the brow flung up, the eye wide, its pupil a pinprick
        for x, y in ((hx, hy - 3), (hx + 1, hy - 4), (hx + 2, hy - 5), (hx + 3, hy - 5), (hx + 4, hy - 4),
                     (hx + 5, hy - 3)):
            canvas[(x, y)] = 'T'
        for x in range(hx + 1, hx + 6):
            for y in range(hy - 3, hy + 2):
                edge = x in (hx + 1, hx + 5) or y in (hy - 3, hy + 1)
                canvas[(x, y)] = 'N' if edge and (x, y) not in ((hx + 1, hy - 3), (hx + 5, hy - 3)) else 'Y'
        canvas[(hx + 3, hy - 1)] = 'o'
        canvas[(hx + 2, hy - 2)] = 'Ψ'
    else:
        canvas.update(eye)
        for q in ((hx + 1, hy - 2), (hx + 1, hy - 1), (hx + 1, hy), (hx + 5, hy - 1), (hx + 5, hy),
                  (hx + 2, hy + 1), (hx + 3, hy + 1), (hx + 4, hy + 1)):
            canvas[q] = 'N' if q[1] <= hy else 'g'
    # The nose's wart and nostril.
    canvas[(hx + 11, hy + 2)] = 'g'
    canvas[(hx + 9, hy + 1)] = 'T'
    canvas[(hx + 10, hy + 4)] = 'N'
    # The grin: a dark line turned up at its back corner, fangs, an underbite.
    for p in ((hx - 1, hy + 3), (hx, hy + 4), (hx + 1, hy + 4), (hx + 2, hy + 4), (hx + 3, hy + 4),
              (hx + 4, hy + 4), (hx + 5, hy + 4), (hx + 6, hy + 4), (hx + 7, hy + 3)):
        canvas[p] = 'N'
    if pose.mouth == 'scream':  # wide open: a dark maw, the tongue, fangs above and below
        for x in range(hx, hx + 8):
            for y in range(hy + 4, hy + 9):
                if (x, y) in canvas and not (y == hy + 8 and x in (hx, hx + 7)):
                    canvas[(x, y)] = 'N' if y in (hy + 4, hy + 8) or x in (hx, hx + 7) else 'M'
        for x in (hx + 3, hx + 4, hx + 5):
            canvas[(x, hy + 7)] = 'E'
        for x in (hx + 2, hx + 6):
            canvas[(x, hy + 8)] = 'w'
    elif pose.mouth == 'smirk':  # one corner pulled up, a single fang
        canvas[(hx - 1, hy + 2)] = 'N'
        canvas[(hx - 2, hy + 1)] = 'N'
    if pose.mouth == 'open':
        for x in range(hx + 1, hx + 7):
            canvas[(x, hy + 5)] = 'M'
        for x in range(hx + 2, hx + 6):
            canvas[(x, hy + 6)] = 'N'
        canvas[(hx + 3, hy + 6)] = 'E'
        canvas[(hx + 4, hy + 6)] = 'E'
        for x in (hx + 1, hx + 6):
            canvas[(x, hy + 6)] = 'w'
    for x in ((hx + 5,) if pose.mouth == 'smirk' else (hx + 1, hx + 3, hx + 5)):
        canvas[(x, hy + 5)] = 'w'  # fangs from the upper jaw
    canvas[(hx + 7, hy + 2)] = 'w'  # a tusk of the underbite
    # The ears: dark insides along the middle, a notch, a brass ring.
    for ear, key in (('near_ear', 'G'), ('far_ear', 'g')):
        inside = P[ear].pixels()
        for (x, y) in sorted(inside):
            if canvas.get((x, y)) == key and all((x + dx, y + dy) in inside for dx, dy in ((0, -1), (0, 1))) \
                    and (x, y - 2) in inside and (x, y + 2) in inside:
                canvas[(x, y)] = 'g' if ear == 'near_ear' else 'T'
    notch = (hx - 13, round(hy - 7 - e * 0.7))
    for dx, dy in ((0, 0), (1, 0), (0, -1)):
        canvas.pop((notch[0] + dx, notch[1] + dy), None)
    for dx, dy in ((0, 0), (1, 1), (0, 2), (-1, 1)):  # the ring under the lobe
        canvas[(hx - 9 + dx, hy + 2 + dy)] = 'A'
    # The goggles pushed up on the cap: a brass rim, a pale lens, a glint;
    # the strap round the cap; a stitched seam.
    gx, gy = hx + 2, hy - 7
    for dx in range(-2, 3):
        for dy in range(-2, 3):
            if dx * dx + dy * dy <= 5:
                canvas[(gx + dx, gy + dy)] = 'A' if dx * dx + dy * dy >= 3 else ('`' if dx + dy < 0 else '_')
    canvas[(gx - 1, gy - 1)] = 'Ψ'
    canvas[(gx + 2, gy + 2)] = 'a'
    for x in range(hx - 7, gx - 2):
        if canvas.get((x, gy + 1)) in ('"', "'", 'ω'):
            canvas[(x, gy + 1)] = 'ω'
    for x in range(hx - 5, hx + 1, 2):
        if canvas.get((x, hy - 9)) in ('"', "'"):
            canvas[(x, hy - 9)] = "'"
    # The jerkin: its front edge laced, a shoulder seam stitched.
    for t in range(5):
        p = point(chest(35 + t * 0.25, 37 + t * 2))
        if canvas.get(p) in ('(', 'a', '['):
            canvas[p] = 'σ' if t % 2 == 0 else '['
    for t in range(4):
        p = point(chest(28 + t * 1.6, 34.5 + t * 0.3))
        if canvas.get(p) in ('(', 'a', '['):
            canvas[p] = 'a'
    # The pouch's flap and its rocks; the hanging bomb's glint and fuse.
    px_, py_ = point(chest(37.5, 47.2))
    for dx in (-2, -1, 0, 1, 2):
        if canvas.get((px_ + dx, py_ - 1)) not in (None, 'o'):
            canvas[(px_ + dx, py_ - 1)] = 'ω'
    for dx, s in ((-1, 'ψ'), (0, 'Ω'), (1, 'ψ')):
        canvas[(px_ + dx, py_ - 3)] = s
    bx, by = point(chest(25, 47.5))
    canvas[(bx - 1, by - 1)] = 'χ'
    canvas[(bx, by - 3)] = 'a'
    canvas[(bx - 1, by - 4)] = 'σ'
    for side in ('near', 'far'):
        w, el, kind_ = hands[side]
        if kind_ == 'open':
            for bones in G_FINGERS:
                tip = point(in_hand(w, el, [(bones[-1][0], bones[-1][1] + 0.8)])[0])
                if side == 'near' or tip not in canvas:
                    canvas[tip] = 'w'
        end, knee = ankles[side]
        toe = pose.near_toe if side == 'near' else pose.far_toe
        for dx, dy in ((8.5, 2), (7, 3)):
            q = point(turned([(end[0] + dx, end[1] + dy)], end, toe)[0])
            canvas[q] = 'w'
        kp = point(knee)
        if side == 'near' and canvas.get(kp) in ('G', 'g', 'W'):
            canvas[kp] = 'm'
    for side in ('near', 'far'):  # the shorts' ragged hem
        end, knee = ankles[side]
        hip = (32.5, 46 + c) if side == 'near' else (29.5, 45 + c)
        for dx, dy in ((-1, 1), (1, 2), (2, 1)):
            q = point(lerp(hip, knee, 0.9))
            q = (q[0] + dx, q[1] + dy)
            if canvas.get(q) in ('G', 'g', 'W', 'm', 'T'):
                canvas[q] = '=' if side == 'near' else '-'
    if kind == 'bomb':  # the held bomb's glint and fuse, lit
        hold = point(in_hand(wrist, elbow, [(0, 3.6)])[0])
        tip = (hold[0] + 1, hold[1] - 5)
        canvas[(hold[0], hold[1] - 3)] = 'a'
        canvas[(hold[0] + 1, hold[1] - 4)] = 'σ'
        canvas[(hold[0] - 1, hold[1] - 1)] = 'χ'
        canvas[tip] = 'Ψ'
        for dx, dy in (((1, 0), (-1, 0), (0, -1)) if pose.spark % 2 == 0 else ((1, -1), (-1, -1))):
            canvas[(tip[0] + dx, tip[1] + dy)] = 'Y'
    elif kind == 'rock':
        hold = point(in_hand(wrist, elbow, [(0, 3.6)])[0])
        canvas[(hold[0] - 1, hold[1])] = 'Φ'
        canvas[(hold[0] + 1, hold[1] + 1)] = 'Φ'
    elif kind == 'knife':  # the brass guard at the fist and the point's glint
        for guard in in_hand(wrist, elbow, [(-1.8, 3), (1.8, 3)]):
            canvas[point(guard)] = 'A'
        canvas[point(in_hand(wrist, elbow, [(0, 9.8)])[0])] = 'w'
    # Its soles are drawn on row 60; raised a row, its outline's lowest row
    # is 60, the walkers' feet row, where its collider ends.
    return outline({(x, y - 1): symbol for (x, y), symbol in canvas.items()})


def goblin_idle_frames():
    """Breathing, its ears twitching up, a blink."""
    poses = [GoblinPose(breath=b, ears=ears, blink=blink,
                        near_elbow=(38.5, 42 - b), near_wrist=(39.5, 48 - b),
                        far_elbow=(28, 41.5 - b), far_wrist=(27.5, 47.5 - b))
             for b, ears, blink in ((0, 0, False), (1, 0, False), (1, 1.2, False), (0, 0, True))]
    return [to_image(goblin_canvas(p)) for p in poses], [240, 200, 160, 120]


def goblin_walk_frames():
    """A quick scuttle, leaning in, the arms swinging against the legs."""
    poses = []
    for i in range(6):
        a = i * math.tau / 6
        near_x, near_lift = 34 + 5 * math.sin(a), 3 * max(0.0, math.cos(a))
        far_x, far_lift = 29 - 5 * math.sin(a), 3 * max(0.0, -math.cos(a))
        swing = 3.5 * math.sin(a)
        bob = round(abs(math.cos(a)))
        poses.append(GoblinPose(
            crouch=1 - bob, lean=2.5, ears=-0.5 * bob,
            near_ankle=(near_x, 57 - near_lift), near_toe=0.12 * near_lift,
            far_ankle=(far_x, 57 - far_lift), far_toe=0.12 * far_lift,
            near_elbow=(39 - swing * 0.4, 42.5), near_wrist=(41 - swing, 48 - abs(swing) * 0.3),
            far_elbow=(29 + swing * 0.4, 42), far_wrist=(29 + swing, 47.5 - abs(swing) * 0.3)))
    return [to_image(goblin_canvas(p)) for p in poses], [85] * 6


def goblin_windup_frames(held='rock'):
    """The throw's tell: crouched, leaning back, the throwing arm drawn back
    and up behind its head with a rock or a lit bomb, the other pointing at
    the target."""
    base = dict(near_hand=held, ears=1.5, mouth='open')
    poses = [GoblinPose(crouch=1, lean=-1, near_elbow=(33, 39), near_wrist=(28, 36), far_elbow=(34, 40),
                        far_wrist=(40, 41), spark=0, **base),
             GoblinPose(crouch=2, lean=-2.5, near_elbow=(30, 35), near_wrist=(24, 31), far_elbow=(35, 38),
                        far_wrist=(42, 38), far_ankle=(26, 57), spark=1, **base),
             GoblinPose(crouch=2, lean=-3, near_elbow=(29, 33), near_wrist=(22, 28), far_elbow=(36, 37),
                        far_wrist=(43, 36), far_ankle=(25, 57), spark=2, **base)]
    return [to_image(goblin_canvas(p)) for p in poses], [110, 140, 200]


def goblin_throw_frames():
    """The release: the arm whipping over and forward, the hand opening,
    the follow-through with a foot lifting."""
    poses = [GoblinPose(crouch=1, lean=1, near_elbow=(34, 30), near_wrist=(39, 25), near_hand='fist',
                        far_elbow=(31, 41), far_wrist=(30, 46), ears=1, mouth='open'),
             GoblinPose(crouch=0, lean=3.5, near_elbow=(41, 34), near_wrist=(47, 32), far_elbow=(30, 41),
                        far_wrist=(27, 45), ears=0.5, mouth='open'),
             GoblinPose(crouch=2, lean=4.5, near_elbow=(42, 41), near_wrist=(46, 47), far_elbow=(29, 40),
                        far_wrist=(25, 44), far_ankle=(26, 55), far_toe=0.4, ears=0)]
    return [to_image(goblin_canvas(p)) for p in poses], [60, 70, 170]


def goblin_stab_windup_frames():
    """The stab's tell: crouched and drawn back, the knife low at its hip
    pointing behind, the free hand out ahead, ears up, snarling."""
    poses = [GoblinPose(crouch=2, lean=-1.5, near_elbow=(33, 42), near_wrist=(29.5, 46.5), near_hand='knife',
                        far_elbow=(36, 39.5), far_wrist=(41.5, 41), far_ankle=(26, 57), ears=1.5, mouth='open')]
    return [to_image(goblin_canvas(p)) for p in poses], [200]


def goblin_stab_frames():
    """The stab: the knife thrust out ahead at chest height off a lunging
    front foot, then drawn halfway back."""
    poses = [GoblinPose(crouch=1, lean=4, near_elbow=(43, 39), near_wrist=(49.5, 39), near_hand='knife',
                        far_elbow=(30.5, 41), far_wrist=(27.5, 45.5), near_ankle=(39, 57), far_ankle=(27, 57),
                        far_toe=0.3, ears=0.5, mouth='open'),
             GoblinPose(crouch=1, lean=2, near_elbow=(41, 40.5), near_wrist=(45.5, 43.5), near_hand='knife',
                        far_elbow=(29.5, 41), far_wrist=(27.5, 46), near_ankle=(37, 57), ears=0.5)]
    return [to_image(goblin_canvas(p)) for p in poses], [80, 140]


def goblin_hop_frames():
    """A hop: its knees tucked under it and its arms flung up, an ear
    flapping, then the arms coming down for the landing."""
    poses = [GoblinPose(crouch=-1, lean=1, near_ankle=(36, 53), far_ankle=(29.5, 53.5), near_toe=0.35,
                        far_toe=0.35, near_elbow=(40.5, 37), near_wrist=(44.5, 32.5), far_elbow=(27, 37),
                        far_wrist=(23.5, 32.5), ears=1.5, mouth='open'),
             GoblinPose(crouch=0, lean=1.5, near_ankle=(36, 55), far_ankle=(29.5, 55.5), near_toe=0.2,
                        far_toe=0.2, near_elbow=(40.5, 39.5), near_wrist=(45, 37.5), far_elbow=(27, 39),
                        far_wrist=(23, 37), ears=0.5, mouth='open')]
    return [to_image(goblin_canvas(p)) for p in poses], [110, 110]


# e01foes: the dash-in and the panic.
def goblin_dash_windup_frames():
    """The dash-in's crouch, 0.3 s: sinking low and coiled on spread feet,
    the knife drawn back past its hip, the free hand out ahead, ears laid
    back, a sly half-lidded look and a smirk."""
    poses = [GoblinPose(crouch=3, lean=1, near_ankle=(38.5, 57), far_ankle=(25.5, 57),
                        near_elbow=(31.5, 43.5), near_wrist=(27, 47.5), near_hand='knife', far_elbow=(36.5, 41),
                        far_wrist=(41.5, 44.5), ears=-1, eye='sly', mouth='smirk'),
             GoblinPose(crouch=4.5, lean=2.5, near_ankle=(39, 57), far_ankle=(25, 57),
                        near_elbow=(30.5, 44.5), near_wrist=(25.5, 47), near_hand='knife', far_elbow=(37.5, 42),
                        far_wrist=(43, 45.5), ears=-1.6, eye='sly', mouth='smirk')]
    return [to_image(goblin_canvas(p)) for p in poses], [120, 180]


def goblin_dash_frames():
    """The dash-in: a low sprint, leaning hard into it, long strides with a
    foot always down, the knife thrust out ahead, the free arm flung back,
    ears streaming."""
    # Each frame's near and far ankle: a foot down, the other swinging through.
    strides = (((40.5, 57), (27, 54)), ((37, 54.5), (33, 57)), ((29, 57), (42, 55)), ((33, 54), (38, 57)))
    poses = [GoblinPose(crouch=2.5 - (k % 2), lean=5, near_ankle=near, far_ankle=far,
                        near_toe=0.12 * (57 - near[1]), far_toe=0.12 * (57 - far[1]),
                        near_elbow=(43, 39.5 + (k % 2) * 0.5), near_wrist=(49.5, 39.5 + (k % 2)),
                        near_hand='knife', far_elbow=(27, 40), far_wrist=(21.5, 42.5 - (k % 2)),
                        ears=-1.8, mouth='open')
             for k, (near, far) in enumerate(strides)]
    return [to_image(goblin_canvas(p)) for p in poses], [70] * 4


def goblin_panic_frames():
    """Its panic while it burns: running flat out, arms flailing over its
    head, mouth wide in a scream, eyes wild, ears flapping."""
    poses = []
    for i in range(6):
        a = i * math.tau / 6
        near_x, near_lift = 35 + 6 * math.sin(a), 4 * max(0.0, math.cos(a))
        far_x, far_lift = 29 - 6 * math.sin(a), 4 * max(0.0, -math.cos(a))
        wave = 3.0 * math.sin(2 * a)
        poses.append(GoblinPose(
            crouch=1 - round(abs(math.cos(a))), lean=1.5,
            near_ankle=(near_x, 57 - near_lift), near_toe=0.12 * near_lift,
            far_ankle=(far_x, 57 - far_lift), far_toe=0.12 * far_lift,
            near_elbow=(41.5 + wave * 0.3, 29), near_wrist=(46.5 + wave, 21 - abs(wave) * 0.5),
            far_elbow=(28 - wave * 0.3, 29), far_wrist=(22.5 - wave, 21 - abs(wave) * 0.5),
            ears=1.5 if i % 2 else -0.5, eye='wild', mouth='scream'))
    return [to_image(goblin_canvas(p)) for p in poses], [70] * 6


def goblin_rock(i):
    """The thrown rock, two turns of it: a faceted stone, pale where the
    moon catches it."""
    shapes = [[(4, 7), (7, 3.5), (11.5, 4.5), (13, 8.5), (10.5, 12.5), (5.5, 12), (3, 9.5)],
              [(3.5, 8), (6, 4), (10, 3.5), (12.5, 6), (12.5, 10.5), (8.5, 13), (4.5, 11.5)]]
    size = (G_THINGS, G_THINGS)
    part = Part(size)
    part.polygon(shapes[i])
    c = shade(part.pixels(), 'Ω', **GOB_ROCK)
    for (x, y) in sorted(c):
        if c[(x, y)] == 'Ω' and unit('rock', i, x, y) < 0.2:
            c[(x, y)] = 'Φ' if unit('rock-t', i, x, y) < 0.5 else 'ψ'
    c[(7 + i, 7)] = 'B'
    c[(8 + i, 8)] = 'B'
    return to_image(outline(c, 'o', size), size)


def goblin_bomb(frame):
    """The thrown bomb, its fuse's two sparks: a black iron ball with a brass
    collar, centred on its frame, which it rolls about."""
    size = (G_THINGS, G_THINGS)
    part = Part(size)
    part.ellipse((8, 8), 4.6, 4.6)
    c = {}
    for (x, y) in part.pixels():
        lit = max(0.0, -((x + 0.5 - 8) * -0.55 + (y + 0.5 - 8) * -0.65) / 4.6 * 0.8 + 0.35)
        c[(x, y)] = 'θbBΩχ'[min(4, int(lit * 5))]
    c[(6, 6)] = 'φ'
    for x in (7, 8, 9):
        c[(x, 3)] = 'a'
    c[(8, 2)] = 'A'
    c[(9, 1)] = 'σ'
    c = outline(c, 'o', size)
    c[(10, 0)] = 'Ψ'
    if frame == 0:
        c[(11, 0)] = 'Y'
        c[(11, 1)] = 'J'
    else:
        c[(9, 0)] = 'J'
        c[(11, 1)] = 'Y'
    return to_image(c, size)


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
    'crawler_retreat': (crawler_retreat_frames, True),
    'crawler_hop': (crawler_hop_frames, False),
    'crawler_snowball_windup': (crawler_snowball_windup_frames, False),
    'crawler_snowball_throw': (crawler_snowball_throw_frames, False),
    'beetle_walk': (beetle_walk_frames, True),
    'beetle_paw': (beetle_paw_frames, False),
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
    'troll_swipe_windup': (troll_swipe_windup_frames, False),
    'troll_swipe': (troll_swipe_frames, False),
    'goblin_idle': (goblin_idle_frames, True),
    'goblin_walk': (goblin_walk_frames, True),
    'goblin_windup': (lambda: goblin_windup_frames('rock'), False),
    'goblin_windup_bomb': (lambda: goblin_windup_frames('bomb'), False),
    'goblin_throw': (goblin_throw_frames, False),
    'goblin_stab_windup': (goblin_stab_windup_frames, False),
    'goblin_stab': (goblin_stab_frames, False),
    'goblin_hop': (goblin_hop_frames, True),
    'goblin_dash_windup': (goblin_dash_windup_frames, False),
    'goblin_dash': (goblin_dash_frames, True),
    'goblin_panic': (goblin_panic_frames, True),
    # e01flask: the stripped beetle's, its shell eaten through, which
    # Corrosive leaves it with.
    'beetle_stripped_walk': (partial(beetle_walk_frames, stripped=True), True),
    'beetle_stripped_paw': (partial(beetle_paw_frames, stripped=True), False),
    'beetle_stripped_charge': (partial(beetle_charge_frames, stripped=True), True),
    # e01flask: the armoured troll's clips, the troll's poses in its iron,
    # and the stripped troll's, that iron eaten to remnants by Corrosive.
    **{f'troll_stripped_{clip}': (partial(frames_for, stripped=True), looping) for clip, frames_for, looping in (
        ('idle', troll_idle_frames, True), ('walk', troll_walk_frames, True),
        ('windup', troll_windup_frames, False), ('slam', troll_slam_frames, False),
        ('pickup', troll_pickup_frames, False), ('throw', troll_throw_frames, False),
        ('swipe_windup', troll_swipe_windup_frames, False), ('swipe', troll_swipe_frames, False))},
    **{f'troll_armoured_{clip}': (partial(frames_for, armoured=True), looping) for clip, frames_for, looping in (
        ('idle', troll_idle_frames, True), ('walk', troll_walk_frames, True),
        ('windup', troll_windup_frames, False), ('slam', troll_slam_frames, False),
        ('pickup', troll_pickup_frames, False), ('throw', troll_throw_frames, False),
        ('swipe_windup', troll_swipe_windup_frames, False), ('swipe', troll_swipe_frames, False))},
}
def build_enemies(images, animations, enemy=None):
    """The six enemies' clips, and the goblin's rock and bomb, unlit: no
    normal or emissive map; or only `enemy`'s, its clips those its name
    prefixes."""
    for clip, (frames_for, looping) in ENEMY_CLIPS.items():
        if enemy and clip.split('_')[0] != enemy:
            continue
        rendered, durations = frames_for()
        frames = []
        for i, (im, duration) in enumerate(zip(rendered, durations, strict=True)):
            name = f'{clip}_{i}'
            images[name] = im
            frames.append({'sprite': 'ex10_' + name, 'duration_ms': duration})
        animations[clip] = {'looping': looping, 'frames': frames}
    for i in range(2 if enemy in (None, 'goblin') else 0):
        images[f'goblin_rock_{i}'] = goblin_rock(i)
        images[f'goblin_bomb_{i}'] = goblin_bomb(i)


# e01hollow's stones by size: the frame's side, then the body box's centre
# and half extents in the frame (the prefabs' `offset` and `half_extents`),
# so each stone's lowest opaque row is its box's bottom.
STONE_BOXES = {'small': (32, (16, 25), (9, 7)), 'medium': (32, (16, 22), (13, 10)),
               'large': (64, (32, 49), (20, 15))}


# The stones' basalt, dark to light, and each look's planes in its body
# box's unit square: points, a name and how lit each is (0 dark to 1 lit
# from the upper left). A piece of a basalt column lying on its side, its
# hexagonal end to the west; a slab of a flow's crust, ropy on top.
STONE_RAMP = 'ενγβαξδ'
STONE_LOOKS = (
    (((0.2, 0.04), (0.88, 0.0), (1.0, 0.2), (0.4, 0.22), 'top', 0.92),
     ((0.4, 0.22), (1.0, 0.2), (0.96, 0.76), (0.4, 0.78), 'side', 0.53),
     ((0.4, 0.78), (0.96, 0.76), (0.84, 1.0), (0.2, 1.0), 'under', 0.2),
     ((0.2, 0.04), (0.4, 0.22), (0.4, 0.78), (0.2, 1.0), (0.0, 0.76), (0.0, 0.26), 'end', 0.79)),
    (((0.04, 0.62), (0.24, 0.2), (0.46, 0.0), (0.58, 0.34), (0.32, 0.62), 'top', 0.9),
     ((0.46, 0.0), (0.74, 0.1), (0.96, 0.46), (0.58, 0.34), 'right', 0.52),
     ((0.0, 1.0), (0.04, 0.62), (0.32, 0.62), (0.42, 1.0), 'left', 0.62),
     ((0.32, 0.62), (0.58, 0.34), (0.96, 0.46), (1.0, 1.0), (0.42, 1.0), 'front', 0.32)),
)


def stone(size, look):
    """Stone `look` (0 or 1) of `size` (e01hollow): rubble of the hollow's
    basalt, angular, its planes lit by how they face the upper left and
    crisp at their ridges, filling its body box (`STONE_BOXES`) with its
    lowest opaque row the box's bottom. Look 0 is a piece of a column, its
    hexagon's rim lit and cooling cracks across its end; look 1 a slab of a
    flow's crust, ropy on its top, black glass along its crest."""
    side, (cx, cy), (hx, hy) = STONE_BOXES[size]
    left, top = cx - hx, cy - hy
    w, h = 2 * hx - 1, 2 * hy - 1
    seed = (size, look)

    def at(u, v):
        ju = (unit('ju', seed, u, v) - 0.5) * 0.06 if 0 < u < 1 else 0
        jv = (unit('jv', seed, u, v) - 0.5) * 0.06 if 0 < v < 1 else 0
        return left + (u + ju) * w, top + (v + jv) * h

    c, owner = {}, {}
    for *points, name, light in STONE_LOOKS[look]:
        plane = Part((side, side))
        plane.polygon([at(u, v) for u, v in points])
        for (x, y) in plane.pixels():
            owner[(x, y)] = name
            grain = (unit('grain', seed, x // 2, y // 2) - 0.5) * 0.1
            c[(x, y)] = ramp(light + grain - 0.06 * (y - top) / max(1, h), STONE_RAMP, x, y, 0.08)
    # Ridges between the planes, lit along the upper plane's edge.
    for (x, y), name in owner.items():
        below, right = owner.get((x, y + 1)), owner.get((x + 1, y))
        if below and below != name:
            if below in ('under', 'front', 'left'):
                c[(x, y + 1)] = 'ν'
            if name in ('top', 'end'):
                c[(x, y)] = 'ξ'
        if right and right != name and name in ('end', 'left', 'top'):
            c[(x, y)] = 'α' if name != 'left' else 'γ'
    if look == 0:
        # The column's end: its hexagon's upper rim lit, its lower dark,
        # cooling cracks from its middle to three corners; a cross-joint.
        end = [p for p, n in owner.items() if n == 'end']
        for (x, y) in end:
            if owner.get((x, y - 1)) != 'end' or owner.get((x - 1, y)) != 'end':
                c[(x, y)] = 'δ' if owner.get((x, y - 1)) != 'end' else 'ξ'
            elif owner.get((x, y + 1)) != 'end' or owner.get((x + 1, y)) != 'end':
                c[(x, y)] = 'ν'
        ex = sum(x for x, _ in end) / len(end)
        ey = sum(y for _, y in end) / len(end)
        for u, v in ((0.2, 0.04), (0.0, 0.76), (0.4, 0.78)):
            tx, ty = at(u, v)
            steps = max(2, round(math.hypot(tx - ex, ty - ey) * 0.75))
            for d in range(steps):
                p = (round(ex + (tx - ex) * d / steps), round(ey + (ty - ey) * d / steps))
                if owner.get(p) == 'end':
                    c[p] = 'γ'
        jx = round(at(0.72, 0.5)[0])
        for (x, y), name in owner.items():
            if x == jx and name in ('side', 'top', 'under'):
                c[(x, y)] = 'ν' if name != 'top' else 'β'
    else:
        # A slab of a flow's crust: ropy wrinkles across its top plane, a
        # rind of black glass along its crest.
        for (x, y), name in owner.items():
            if name != 'top':
                continue
            u, v = (x - left) / max(1, w), (y - top) / max(1, h)
            rope = math.sin((u * 2.2 + v * 3.4) * math.pi * (2.2 if size != 'small' else 1.4)
                            + 0.6 * math.sin(v * 9 + look))
            if rope > 0.72:
                c[(x, y)] = 'β'
            elif rope < -0.85:
                c[(x, y)] = 'δ'
        for (x, y), name in owner.items():
            if name in ('top', 'right') and owner.get((x, y - 1)) is None:
                c[(x, y)] = '2'
                if owner.get((x, y + 1)) == name:
                    c[(x, y + 1)] = 'ξ' if name == 'top' else 'α'
        c[min((p for p, n in owner.items() if n == 'top'), key=lambda p: (p[1], p[0]))] = 'π'
    # Gas holes, dark with a lit lower lip.
    for k in range(2 + (size == 'large') * 4):
        x = round(left + 2 + unit('ves', seed, k) * (w - 4))
        y = round(top + 2 + unit('vesy', seed, k) * (h - 4))
        if owner.get((x, y)) not in (None, 'end') and (x, y + 1) in owner and c[(x, y)] != '2':
            c[(x, y)] = 'ε'
            if c[(x, y + 1)] in 'βγ':
                c[(x, y + 1)] = 'α'
    canvas = outline(c, 'o', (side, side))
    # Its own bottom row its lowest: no outline under its foot.
    bottom = top + 2 * hy - 1
    for p in [p for p in canvas if p[1] > bottom]:
        del canvas[p]
    for x in range(side):
        if canvas.get((x, bottom)) not in (None, 'o') and (x, bottom - 1) in canvas:
            canvas[(x, bottom)] = 'θ'
    return to_image(canvas, (side, side))


def build_stones(images, lit):
    """e01hollow's rubble stones, two looks a size (`stone`), lit as the
    crates are."""
    for size in STONE_BOXES:
        for i in range(2):
            images[f'stone_{size}_{i}'] = stone(size, i)
            lit.add(f'stone_{size}_{i}')
