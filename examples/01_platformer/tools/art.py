"""Palette-only pixel art. Grids are source; derived poses keep hard edges."""
import hashlib
import json
import math
import random
from pathlib import Path

from PIL import Image, ImageDraw

TOOLS = Path(__file__).resolve().parent
# Black-hole layers are drawn larger so their soft edges hold up when scaled.
EFFECT_SIZES = {name: (96, 96) for name in (
    'vortex', 'vortex_core', 'accretion_disk', 'photon_ring', 'infall_streak')}
# The spell, gun and summon icons, 36 px tiles (01 (h)'s icon family), which
# the HUD shows at whole-number scale.
ICONS = ('fire_missile', 'flamethrower', 'ice_spray', 'force_push', 'black_hole',
         'iron_block', 'bowling_ball', 'small_balls', 'lightning', 'gun', 'dummy',
         'wisp', 'crawler', 'beetle', 'bat', 'troll', 'goblin', 'crate', 'keg')
ICON_SIZES = {'icon_' + name: (36, 36) for name in ICONS}
# The elements' and weapons' art off the 64 px grid: the bolt's segments, the
# status overlays' small and large classes (the medium class is 64), the
# muzzle flash's frames and the tracer.
STATUS_CLASSES = {'small': 32, 'medium': 64, 'large': 128}
ELEMENT_SIZES = {
    **{f'bolt_{i}': (24, 64) for i in range(8)},
    **{f'status_{look}_{name}{frame}': (size, size)
       for name, size in STATUS_CLASSES.items() if size != 64
       for look, frames in (('ice', ('', '_cracked')), ('halo', ('',)), ('crackle', ('_0', '_1')))
       for frame in frames},
    **{f'muzzle_{i}': (16, 16) for i in range(3)},
    'tracer': (32, 3),
    # 01 (c): the troll's fitted ice cube on its 128 frame, the frozen
    # look's glint and the drops' orbs; 01 (f): the cube's cracked twin.
    'status_ice_troll': (128, 128),
    'status_ice_troll_cracked': (128, 128),
    'ice_glint': (16, 16),
    'health_orb': (24, 24),
    'mana_orb': (24, 24),
    # 01 (g): the goblin's rock and bomb, and the marks over an enemy.
    **{f'goblin_{thing}_{i}': (16, 16) for thing in ('rock', 'bomb') for i in range(2)},
    'mark_alert': (16, 16),
    'mark_search': (16, 16),
    # 01 (h): the sprays' clips' frames, the bars' liquids, the gun's
    # cartridges and the title's star.
    **{f'{clip}_{i}': size for clip, (size, frames) in (
        ('flame_muzzle', ((40, 32), 6)), ('flame_puff', ((48, 48), 8)), ('flame_splash', ((48, 48), 6)),
        ('frost_muzzle', ((32, 32), 6)), ('frost_puff', ((48, 48), 8)), ('frost_core', ((32, 14), 8)),
        ('frost_splash', ((48, 48), 6))) for i in range(frames)},
    'hud_liquid_health': (128, 10),
    'hud_liquid_mana': (128, 10),
    'hud_bullet': (5, 11),
    'hud_bullet_spent': (5, 11),
    # The title's twinkle.
    'title_star': (15, 15),
}
# Art sized by its name's prefix: the troll's frames are 128 px, the one
# enemy drawn larger than a tile, and the wizard's 96 px, room for his hat
# and his staff about the courier's body.
SIZE_PREFIXES = {'troll_': (128, 128), 'player': (96, 96)}
# The wizard's frames keep their lowest pixel on his feet's row, 93, the
# outline's row under his soles (`actors.WIZARD_FEET`), the lying ones too.
PLAYER_FEET = 93
# A walker's frames keep their lowest pixel on its feet's row: the outline's
# row under its soles (`actors.SOLE` and `BIG_SOLE`); the goblin's by its
# clips, apart from its rock and bomb.
WALKER_FEET = {'crawler_': 61, 'beetle_': 61, 'troll_': 125,
               **{f'goblin_{clip}_': 61 for clip in ('idle', 'walk', 'windup', 'throw')}}


def expected_size(name):
    for prefix, size in SIZE_PREFIXES.items():
        if name.startswith(prefix):
            return size
    return {**EFFECT_SIZES, **ICON_SIZES, **ELEMENT_SIZES}.get(name, (64, 64))
def validate_palette(palette):
    if palette.get('.') != [0, 0, 0, 0]:
        raise ValueError('palette must define transparent dot')
    for symbol, rgba in palette.items():
        if len(symbol) != 1 or len(rgba) != 4 or any(type(c) is not int or not 0 <= c <= 255 for c in rgba):
            raise ValueError('palette requires one-character symbols and four RGBA bytes')
    return {k: tuple(v) for k, v in palette.items()}


PALETTE = validate_palette(json.loads((TOOLS / 'palette.json').read_text()))


def rng_for(name):
    return random.Random(int.from_bytes(hashlib.sha256(name.encode()).digest()[:8], 'big'))


def read_grid(text, width=64, height=64):
    rows = text.splitlines()
    if len(rows) != height or any(len(row) != width for row in rows):
        raise ValueError(f'grid must be exactly {width}x{height}')
    if any(c not in PALETTE for row in rows for c in row):
        raise ValueError('unknown palette symbol')
    im = Image.new('RGBA', (width, height))
    im.putdata([PALETTE[c] for row in rows for c in row])
    return im


def canvas(size=(64, 64), color='.'):
    im = Image.new('RGBA', size, PALETTE[color])
    return im, ImageDraw.Draw(im)


def relief(name):
    """Tiling terrain and multi-tile scenery take normals from their shading."""
    from terrain import RELIEF_PREFIXES
    return name.startswith(RELIEF_PREFIXES) or '_big_' in name


def relief_heights(im):
    """Luminance as height, normalised over opaque pixels; transparency is zero."""
    heights = {}
    for y in range(im.height):
        for x in range(im.width):
            r, g, b, a = im.getpixel((x, y))
            if a:
                heights[(x, y)] = 0.3 * r + 0.59 * g + 0.11 * b
    if not heights:
        return heights
    low, high = min(heights.values()), max(heights.values())
    span = max(high - low, 1.0)
    return {p: (h - low) / span for p, h in heights.items()}


def auxiliary(im, relief=False):
    normal = Image.new('RGBA', im.size, (128, 128, 255, 0))
    emissive = Image.new('RGBA', im.size, (0, 0, 0, 255))
    glow = {PALETTE[c] for c in 'yYCc'}
    alpha = im.getchannel('A')
    heights = relief_heights(im) if relief else None
    # Fully opaque tiles repeat, so their slopes wrap across the tile edge.
    wraps = heights is not None and len(heights) == im.width * im.height
    def height(x, y):
        if wraps:
            return heights[(x % im.width, y % im.height)]
        return heights.get((x, y), 0.0)
    for y in range(im.height):
        for x in range(im.width):
            rgba = im.getpixel((x, y))
            if not rgba[3]:
                continue
            if heights is not None:
                # Shading detail becomes relief; no whole-tile tilt, so seams stay flat.
                nx = (height(x-1, y) - height(x+1, y)) * 1.1
                ny = (height(x, y-1) - height(x, y+1)) * 1.1
                length = math.sqrt(nx*nx+ny*ny+1)
                normal.putpixel((x,y), (round(128+127*nx/length), round(128+127*ny/length), round(128+127/length), rgba[3]))
                if rgba in glow:
                    emissive.putpixel((x,y), (*rgba[:3],255))
                continue
            # Same final geometry drives bevel normals and detail emission.
            left = alpha.getpixel((max(0, x-1), y)) / 255
            right = alpha.getpixel((min(im.width-1, x+1), y)) / 255
            up = alpha.getpixel((x, max(0, y-1))) / 255
            down = alpha.getpixel((x, min(im.height-1, y+1))) / 255
            nx = (left-right)*0.6 + (x/im.width-0.5)*0.25
            ny = (up-down)*0.6 + (y/im.height-0.5)*0.25
            length = math.sqrt(nx*nx+ny*ny+1)
            normal.putpixel((x,y), (round(128+127*nx/length), round(128+127*ny/length), round(128+127/length), rgba[3]))
            if rgba in glow:
                emissive.putpixel((x,y), (*rgba[:3],255))
    return normal, emissive


def build_art():
    """Every sprite by name, the lit ones, the clips, and each wizard
    frame's staff light anchor (`actors.build_player`)."""
    from actors import (
        build_balls, build_dummy, build_enemies, build_fire_missile, build_fireball, build_player,
    )
    from backdrops import build_backdrops
    from effects import build_effects
    from props import build_props
    from scenery import build_scenery
    from terrain import build_terrain
    images={}; lit=set(); animations={}
    anchors=build_player(images, lit, animations)
    build_fireball(images, animations)
    build_fire_missile(images, animations)
    build_balls(images, lit, animations)
    build_dummy(images)
    build_enemies(images, animations)
    build_scenery(images, lit, animations)
    build_props(images, lit)
    build_terrain(images, lit)
    build_backdrops(images, lit)
    build_effects(images)
    from polish_art import enrich_art
    enrich_art(images,lit,animations)
    return images,lit,animations,anchors


def validate_art(images,lit):
    from backdrops import BACKDROPS
    allowed=set(PALETTE.values())
    for name,im in images.items():
        # A moving platform's deck is its width wide.
        wide=name.startswith('lift_deck_') and im.height==64
        if name not in BACKDROPS and not wide and im.size!=expected_size(name):raise ValueError(f'{name}: wrong dimensions')
        if not set(im.get_flattened_data())<=allowed:raise ValueError(f'{name}: outside palette')
        if name.startswith(('player', 'ball', 'lantern', 'vines')) or name == 'marker':
            box=im.getbbox()
            if not box or box[0]<1 or box[1]<1 or box[2]>im.width-1 or box[3]>im.height-1:raise ValueError(f'{name}: clipped frame')
            if name.startswith('player') and box[3]!=PLAYER_FEET:raise ValueError(f'{name}: unstable foot anchor')
        for prefix,feet in WALKER_FEET.items():
            if name.startswith(prefix) and (im.getbbox() or (0,0,0,0))[3]!=feet:raise ValueError(f'{name}: feet off row {feet - 1}')
    for name in ('ground_1','ground_2','ground_3'):
        if images[name].crop((0,0,64,6)).tobytes()!=images['ground'].crop((0,0,64,6)).tobytes():raise ValueError('terrain seam')
    from backdrops import validate_backdrops
    from terrain import validate_terrain
    validate_terrain(images)
    validate_backdrops(images)
    for name in lit:
        n,e=auxiliary(images[name],relief(name))
        if n.size!=images[name].size or e.size!=images[name].size:raise ValueError('auxiliary size')
        if n.getchannel('A').tobytes()!=images[name].getchannel('A').tobytes():raise ValueError('normal mask mismatch')
