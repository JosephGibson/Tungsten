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
    from actors import build_balls, build_fireball, build_player
    from backdrops import build_backdrops
    from effects import build_effects
    from scenery import build_scenery
    from terrain import build_terrain
    images={}; lit=set(); animations={}
    build_player(images, lit, animations)
    build_fireball(images, animations)
    build_balls(images, lit, animations)
    build_scenery(images, lit, animations)
    build_terrain(images, lit)
    build_backdrops(images)
    build_effects(images)
    from polish_art import enrich_art
    enrich_art(images,lit,animations)
    return images,lit,animations


def validate_art(images,lit):
    allowed=set(PALETTE.values())
    for name,im in images.items():
        if name not in ('sky','distant_ridges','near_woodland','clouds_far','clouds_near') and im.size!=EFFECT_SIZES.get(name,(64,64)):raise ValueError(f'{name}: wrong dimensions')
        if not set(im.get_flattened_data())<=allowed:raise ValueError(f'{name}: outside palette')
        if name.startswith(('player', 'ball', 'lantern', 'vines')) or name == 'marker':
            box=im.getbbox()
            if not box or box[0]<1 or box[1]<1 or box[2]>63 or box[3]>63:raise ValueError(f'{name}: clipped frame')
            if name.startswith('player') and box[3]!=61:raise ValueError(f'{name}: unstable foot anchor')
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
