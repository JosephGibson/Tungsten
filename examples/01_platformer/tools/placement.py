"""Resolve explicit scenery supports and pixel-tight platform collision bounds,
and check that nothing in the built level floats (01 (k))."""
import json
import re
from PIL import Image
from art import TOOLS

# How far a prop hung under a platform (`anchor` `under`) tucks its top
# opaque row up into the underside's ragged lip, in pixels.
UNDER_TUCK = 6
# The floating check's numbers (01 (k)): a foot meets the drawn surface
# within `GAP` px under it and sinks at most `SINK` px into it, over `MOST`
# of its columns; a hung top meets it from `GAP` px above to `UNDER_TUCK`
# px into it.
GAP, SINK, MOST = 2, 6, 0.6


def underside(support, images, tile):
    """Where `support`'s underside lies, in tiles: a ground platform's
    bottom, a deck's visible depth under its row."""
    from level import FAMILIES
    if support['style'] in FAMILIES:
        return support['bottom']
    return support['row'] + images[support['style']].getbbox()[3] / tile


def settle(solid, pts, anchor):
    """The smallest move, 0 to 12 px (down for `ground`, up for `under`), at
    which opaque pixels `pts` pass the floating check's rule for `anchor`;
    0 when none does, which the check then lists."""
    sign = -1 if anchor == 'under' else 1
    for d in range(13):
        moved = [(x, y + sign * d) for x, y in pts]
        if anchor == 'ground':
            share, sink = stands(solid, moved)
            if share >= MOST and sink <= SINK:
                return sign * d
        elif hangs(solid, moved) >= MOST:
            return sign * d
    return 0


def prepare(source, images, tiles):
    """Places the level's supported props, groups, pickups, crates, kegs and
    supports and derives the slab colliders. `tiles` maps each terrain cell
    to its sprite: a supported prop or group stands or hangs on what they
    draw, by the floating check's own test (01 (k))."""
    tile=source['tile'];platforms={p['name']:p for p in source['platforms']}
    drawn=Placed(images,tile)
    for (x,y),name in tiles.items():drawn.add(name,x*tile,y*tile)
    # Every thin platform owns shallow rectangles instead of full tile collision.
    source['slab_colliders']=[]
    from level import FAMILIES
    for p in source['platforms']:
        if p['style'] in FAMILIES:continue
        box=images[p['style']].getbbox()
        if box[:2]!=(0,0) or box[2]!=tile:raise ValueError('platform silhouette needs explicit horizontal bounds')
        for x in range(p['left'],p['right']):
            # Portions buried in solid terrain need no second, overlapping body.
            if any(q['style'] in FAMILIES and q['left']<=x<q['right'] and q['row']<=p['row']<q['bottom'] for q in source['platforms']):continue
            source['slab_colliders'].append([x*tile,p['row']*tile,tile,box[3],p['style'] in ('bridge_rope','bridge_weathered')])
    source['deck_depth']=images['lift_deck'].getbbox()[3]
    for p in source['props']:
        if 'support' not in p:continue
        support=platforms[p['support']];im=images[p['sprite'].removeprefix('ex10_')];box=im.getbbox()
        center=p['x']+(box[0]+box[2])/2/tile
        if not support['left']<=center<support['right']:raise ValueError(f"prop outside support: {p['sprite']} at {p['x']} on {support['name']}")
        if p['anchor']=='ground':
            # A new solid stair supersedes its buried parent surface.
            covers=[q for q in source['platforms'] if q['style'] in FAMILIES and q['left']<=center<q['right'] and q['row']<support['row']<=q['bottom']]
            if covers:support=min(covers,key=lambda q:q['row']);p['support']=support['name']
            p['y']=support['row']-box[3]/tile
        elif p['anchor']=='under':
            p['y']=underside(support,images,tile)-(UNDER_TUCK+box[1])/tile
        if p['anchor'] in ('ground','under'):
            # From the support's edge onto what the tiles draw: the rock
            # families' ragged tops and undersides, a weathered deck's planks.
            p['y']+=settle(drawn.hit,drawn.points(p['sprite'].removeprefix('ex10_'),round(p['x']*tile),round(p['y']*tile)),p['anchor'])/tile
        else:
            depth=images[support['style']].getbbox()[3]
            p['y']=support['row']+(depth-box[1])/tile
    # The placed health orbs: each over its support's surface, its centre
    # `float` tiles up, refused off its support; a solid stair over the
    # support takes its place, as for a grounded prop.
    for p in source.get('pickups', []):
        support = platforms[p['support']]
        if not support['left'] <= p['x'] < support['right']:
            raise ValueError(f"pickup outside support: {p['name']} at {p['x']} on {support['name']}")
        covers = [q for q in source['platforms'] if q['style'] in FAMILIES and q['left'] <= p['x'] < q['right']
                  and q['row'] < support['row'] <= q['bottom']]
        if covers:
            support = min(covers, key=lambda q: q['row'])
            p['support'] = support['name']
        half = images['health_orb'].width / 2 / tile
        p['left'], p['top'] = p['x'] - half, support['row'] - p['float'] - half
    # The placed crates and kegs: each box's centre `y`, its bottom half a
    # pixel over its support's surface, refused with any of its box off its
    # support; a solid stair over the support takes its place. A `tier`
    # stacks a box that many boxes up, each bottom corner over a box of its
    # kind on the tier under it.
    for kind in ('crate', 'keg'):
        prefab = json.loads((TOOLS / 'content' / 'prefabs' / f'{kind}.json').read_text())
        hx, hy = (v / tile for v in prefab['components']['rigid_body']['shape']['half_extents'])
        lift = 0.5 / tile
        placed = source.get(kind + 's', [])
        for p in placed:
            support = platforms[p['support']]
            covers = [q for q in source['platforms'] if q['style'] in FAMILIES and q['left'] <= p['x'] < q['right']
                      and q['row'] < support['row'] <= q['bottom']]
            if covers:
                support = min(covers, key=lambda q: q['row'])
                p['support'] = support['name']
            if not support['left'] <= p['x'] - hx <= p['x'] + hx <= support['right']:
                raise ValueError(f"{kind} outside support: {p['x']} on {support['name']}")
            tier = p.get('tier', 0)
            p['y'] = support['row'] - (2 * tier + 1) * hy - (tier + 1) * lift
        for p in placed:
            under = [q for q in placed if q['support'] == p['support'] and q.get('tier', 0) == p.get('tier', 0) - 1]
            for corner in (p['x'] - hx, p['x'] + hx):
                if p.get('tier', 0) and not any(abs(corner - q['x']) <= hx for q in under):
                    raise ValueError(f"{kind} stacked over nothing: {p['x']} on {p['support']}")
    # Each group's own pieces: those inside its box where they are written,
    # none inside two groups' boxes.
    owners={}
    for k,group in enumerate(source['groups']):
        for i,p in enumerate(source['props']):
            if p['sprite'].startswith(group['sprite']+'_big_') and group['x']<=p['x']<group['x']+group['width'] and group['y']<=p['y']<group['y']+group['height']:
                owners.setdefault(i,[]).append(k)
    for i,ks in owners.items():
        if len(ks)>1:raise ValueError(f"a piece inside two groups' boxes: {source['props'][i]['sprite']} at {source['props'][i]['x']}, {source['props'][i]['y']}")
    for k,group in enumerate(source['groups']):
        name=group['sprite'];x=group['x'];y=group['y'];w=group['width'];h=group['height'];support=platforms[group['support']]
        composite=Image.new('RGBA',(w*tile,h*tile))
        for r in range(h):
            for c in range(w):composite.paste(images[f'{name.removeprefix("ex10_")}_big_{r}_{c}'],(c*tile,r*tile))
        box=composite.getbbox();edge=box[1] if group['anchor'] in ('wall','under') else box[3]
        target_y=support['row']-edge/tile
        if group['anchor']=='under':target_y=underside(support,images,tile)-(UNDER_TUCK+box[1])/tile
        # Ground both feet/trunk, not merely the center of a wide composition.
        if group['anchor']=='ground':
            foot=composite.getchannel('A').crop((0,box[3]-1,w*tile,box[3])).getbbox()
            if x+foot[0]/tile<support['left'] or x+foot[2]/tile>support['right']:raise ValueError('composite feet outside support: '+name)
        if group['anchor'] in ('ground','under'):
            a=composite.getchannel('A').tobytes()
            pts=[(round(x*tile)+i%(w*tile),round(target_y*tile)+i//(w*tile)) for i,v in enumerate(a) if v>=128]
            target_y+=settle(drawn.hit,pts,group['anchor'])/tile
        # Its pieces and where its box's top lands, which the floating
        # check reads.
        group['pieces'],group['top']=sorted(i for i,ks in owners.items() if ks==[k]),target_y
        for i in group['pieces']:source['props'][i]['y']+=target_y-y
    # Attached supports: start exactly at the visible deck underside, extend
    # into solid ground at the base, and render behind the collision surfaces.
    for support in source['supports']:
        top=platforms[support['platform']];ground=platforms[support['ground']]
        y=top['row']+images[top['style']].getbbox()[3]/tile
        while y<ground['row']:
            source['props'].append(dict(sprite='ex10_timber_support',x=support['x'],y=y,depth='back',animation=None))
            y+=1
    # Reposition flame emitters to the grounded lantern's actual flame.
    for emitter in source['emitters']:
        if emitter['config']=='ex10_torch_embers':
            lamp=min((p for p in source['props'] if p['sprite']=='ex10_lantern'),key=lambda p:abs(p['x']+.5-emitter['x']))
            emitter['x']=lamp['x']+.5;emitter['y']=lamp['y']+.5
    return source


# The floating check (01 (k)). What a prop is when its `anchor` doesn't say:
# the scenery lists' standing and hung sprites; the streams, judged as
# stacks; the sprites that sit in or on what holds them; and the pieces a
# stream may leave from beside the drawn terrain.
STREAMS = {'pour': 'pour', 'pour_wide': 'pour', 'pour_foot': 'pour', 'lavafall': 'lavafall'}
EXEMPT = ('water', 'river', 'pool', 'lava_', 'spill_')
LIQUIDS = ('water', 'river', 'pool', 'lava_surface', 'lava_deep')
SOURCES = ('gargoyle', 'fissure', 'spill_')
# A gap strip's pieces (01 (k)) as the arcade pieces they replace, by their
# row and column: the broken end's pier, the bay, the next pier and the
# springing of the bay beyond it.
GAP_STRIP = (('pier_top', 'arch_left', 'arch_key', 'arch_right', 'pier_top', 'arch_left'),
             ('capital', 'spring_left', 'opening', 'spring_right', 'capital', 'spring_left'))
# The background structures drawn whole and cut into tiles, which terrain
# draws over: the cave mouth.
STRUCTURES = ('cave_mouth',)


def stream_kind(name):
    """The stream a sprite or clip `name` belongs to, or None."""
    return STREAMS.get(name) or ('falls' if name.startswith('falls_') else None)


class Placed:
    """Sprites at whole-pixel points, looked up by pixel: opaque where one's
    alpha is 128 or more."""

    def __init__(self, images, tile):
        self.images, self.tile, self.cells, self.masks, self.opaque = images, tile, {}, {}, {}

    def mask(self, name):
        if name not in self.masks:
            im = self.images[name]
            self.masks[name] = (im.width, im.height, bytes(a >= 128 for a in im.getchannel('A').tobytes()))
        return self.masks[name]

    def points(self, name, ox=0, oy=0):
        """`name`'s opaque pixels with its top-left at (`ox`, `oy`)."""
        if name not in self.opaque:
            w, _, m = self.mask(name)
            self.opaque[name] = [(i % w, i // w) for i, a in enumerate(m) if a]
        return [(ox + x, oy + y) for x, y in self.opaque[name]]

    def add(self, name, ox, oy, tag=None):
        w, h, _ = self.mask(name)
        for cy in range(oy // self.tile, (oy + h - 1) // self.tile + 1):
            for cx in range(ox // self.tile, (ox + w - 1) // self.tile + 1):
                self.cells.setdefault((cx, cy), []).append((name, ox, oy, tag))

    def hit(self, x, y, skip=()):
        """Whether a sprite not tagged `skip` is opaque at (`x`, `y`)."""
        for name, ox, oy, tag in self.cells.get((x // self.tile, y // self.tile), ()):
            w, h, m = self.mask(name)
            if (tag is None or tag != skip) and 0 <= x - ox < w and 0 <= y - oy < h and m[(y - oy) * w + x - ox]:
                return True
        return False


def stands(solid, pts):
    """The share of a foot's columns (its bottom two opaque rows') with solid
    within `GAP` px under its bottom row, and the deepest any sinks."""
    bottom = max(y for _, y in pts)
    xs = sorted({x for x, y in pts if y >= bottom - 1})
    meet = sink = 0
    for x in xs:
        meet += any(solid(x, bottom + d) for d in range(1, GAP + 1))
        depth = 0
        while depth < 32 and solid(x, bottom - depth):
            depth += 1
        sink = max(sink, depth)
    return meet / len(xs), sink


def hangs(solid, pts):
    """The share of a top's columns (its top two opaque rows') with solid
    from `GAP` px over its top row to `UNDER_TUCK` px into it."""
    top = min(y for _, y in pts)
    xs = sorted({x for x, y in pts if y <= top + 1})
    return sum(any(solid(x, top + d) for d in range(-GAP, UNDER_TUCK + 1)) for x in xs) / len(xs)


def judge(anchor, solid, back, pts):
    """Why opaque pixels `pts` fail `anchor`'s rule, as (kind, detail), or
    None: `ground` stands, `under` hangs, `wall` touches a wall within 3 px
    to its side and `back` lies wholly over the background layer (`back`
    tells a pixel's cell has a background tile)."""
    if anchor == 'ground':
        share, sink = stands(solid, pts)
        if share < MOST:
            return 'stand', f'{share:.0%} of its foot on the drawn surface'
        if sink > SINK:
            return 'sunk', f'its foot {sink} px into the drawn surface'
    elif anchor == 'under':
        share = hangs(solid, pts)
        if share < MOST:
            return 'hang', f'{share:.0%} of its top under the drawn surface'
    elif anchor == 'wall':
        if not any(solid(x + d, y) for x, y in pts for d in (-3, -2, -1, 1, 2, 3)):
            return 'wall', 'touches no wall within 3 px'
    elif anchor == 'back':
        if not all(back(x, y) for x, y in pts):
            return 'back', 'not wholly over the background layer'
    else:
        return 'unjudged', f'anchor {anchor}'
    return None


def floating(source, tiled, images):
    """Every piece of the built level that floats, sinks or states no way
    it stands, as lines `kind name x y detail` sorted by x (tiles): props by
    their `anchor`, else their sprite's list in scenery.py; each group's
    composite; streams as stacks; supports; decorations by structure. Reads
    `tiled`'s terrain and background layers and `prepare`'s placements; the
    rules are in tools/README.md (Final placement)."""
    import scenery
    tile, cols = source['tile'], tiled['width']
    catalog = {t['id'] + 1: t['properties'][0]['value'].removeprefix('ex10_') for t in tiled['tilesets'][0]['tiles']}
    data = {l['name']: l['data'] for l in tiled['layers'] if l['type'] == 'tilelayer'}
    terrain = {(i % cols, i // cols): catalog[g] for i, g in enumerate(data['terrain']) if g}
    background = {(i % cols, i // cols) for i, g in enumerate(data['background']) if g}
    drawn = Placed(images, tile)
    for (x, y), name in terrain.items():
        drawn.add(name, x * tile, y * tile)
    solid = drawn.hit

    def back(x, y):
        return (x // tile, y // tile) in background

    standing = {name for name, _ in scenery.LEVEL_PROPS}
    hung = {name for name, _ in scenery.HUNG_PROPS}
    exempt = {name for name, _ in scenery.MISTS + scenery.RISING + scenery.FAR_PROPS}
    props, groups = source['props'], source['groups']

    def clip(p):
        """A prop's drawing: its clip's name, else its sprite's."""
        return (p['animation'] or p['sprite']).removeprefix('ex10_')

    def kind(name):
        return re.sub(r'_big_\d+_\d+$', '', name)

    def at(p):
        return round(p['x'] * tile), round(p['y'] * tile)

    def pixels(p):
        return drawn.points(p['sprite'].removeprefix('ex10_'), *at(p))

    lines = []

    def fail(what, name, x, y, detail):
        lines.append((x, y, f"{what} {name} {('%.4f' % x).rstrip('0').rstrip('.')} {('%.4f' % y).rstrip('0').rstrip('.')} {detail}"))

    # What a stream may end on, and what its top may leave from beside the
    # drawn terrain: spills, the gargoyle, the fissure, another column of it.
    liquid, sources = Placed(images, tile), Placed(images, tile)
    for p in props:
        name = kind(clip(p))
        if name.startswith(LIQUIDS) and name not in standing:
            liquid.add(p['sprite'].removeprefix('ex10_'), *at(p))
        if name.startswith(SOURCES) or (stream_kind(name) and '_big_' not in p['sprite']):
            sources.add(p['sprite'].removeprefix('ex10_'), *at(p), tag=(stream_kind(name), p['x']))

    # Groups: each one's own pieces, where they landed, as its anchor says;
    # a piece inside two groups' boxes where it was written, or in none.
    owner = {i: k for k, g in enumerate(groups) for i in g.get('pieces', [])}
    for i, p in enumerate(props):
        if '_big_' not in p['sprite']:
            continue
        k = owner.get(i)
        y = round(p['y'] - (groups[k]['top'] - groups[k]['y']), 6) if k is not None else p['y']
        boxes = [g for g in groups if p['sprite'].startswith(g['sprite'] + '_big_')
                 and g['x'] <= p['x'] < g['x'] + g['width'] and g['y'] <= y < g['y'] + g['height']]
        name = p['sprite'].removeprefix('ex10_')
        if len(boxes) > 1:
            fail('group', name, p['x'], p['y'], f"inside {len(boxes)} groups' boxes")
        elif k is None and not kind(clip(p)).startswith(EXEMPT) and kind(clip(p)) not in exempt:
            fail('unjudged', name, p['x'], p['y'], "in no group's box")
    for g in groups:
        name = g['sprite'].removeprefix('ex10_')
        if not g.get('pieces'):
            fail('group', name, g['x'], g['y'], 'moves no piece')
            continue
        why = judge(g['anchor'], solid, back, [pt for i in g['pieces'] for pt in pixels(props[i])])
        if why:
            fail('group-' + why[0], name, g['x'], g['top'], why[1])

    # Streams: each column of a stream's segments as a stack.
    columns = {}
    for p in props:
        if stream_kind(clip(p)) and '_big_' not in p['sprite']:
            columns.setdefault((stream_kind(clip(p)), p['x']), []).append(p)
    for (stream, x), parts in sorted(columns.items()):
        parts = [(p, pixels(p)) for p in sorted(parts, key=lambda p: p['y'])]
        parts = [(p, pts) for p, pts in parts if pts]
        if not parts:
            continue
        first, pts = parts[0]
        top = min(b for _, b in pts)
        if not any(solid(a + dx, b + dy) or sources.hit(a + dx, b + dy, skip=(stream, x))
                   for a, b in pts if b <= top + 1 for dx in range(-GAP, GAP + 1) for dy in range(-GAP, 1)):
            fail('stream-top', stream, x, first['y'], f'its top meets no source within {GAP} px')
        for (_, above), (p, pts) in zip(parts, parts[1:]):
            gap = min(b for _, b in pts) - max(b for _, b in above) - 1
            if gap >= GAP:
                fail('stream-gap', stream, x, p['y'], f'{gap} px under the segment above')
        through = [p['y'] for p, pts in parts if sum(solid(a, b) for a, b in pts) > len(pts) / 4]
        if through:
            fail('stream-through', stream, x, through[0], f"segments at y {', '.join(f'{y:g}' for y in through)} run through the drawn terrain")
        last, pts = parts[-1]
        bottom = max(b for _, b in pts)
        xs = sorted({a for a, b in pts if b >= bottom - 1})
        if sum(any(solid(a, bottom + d) or liquid.hit(a, bottom + d) for d in range(1, GAP + 1)) for a in xs) < MOST * len(xs):
            a, d = xs[len(xs) // 2], 1
            while d < 16 * tile and not (solid(a, bottom + d) or liquid.hit(a, bottom + d)):
                d += 1
            fail('stream-short', stream, x, last['y'], f'ends {d - 1} px over the surface under it')

    # Supports: each column of timber's last piece meets the ground (it may
    # run on into it, behind the terrain).
    timber = {}
    for p in props:
        if clip(p) == 'timber_support':
            timber.setdefault(p['x'], []).append(p)
    for x, parts in sorted(timber.items()):
        last = max(parts, key=lambda p: p['y'])
        share, _ = stands(solid, pixels(last))
        if share < MOST:
            fail('support', 'timber_support', x, last['y'], f'{share:.0%} of its foot on the drawn surface')

    # Single props, by anchor, else by their sprite's list.
    for p in props:
        name, sprite = kind(clip(p)), p['sprite'].removeprefix('ex10_')
        if '_big_' in sprite or stream_kind(name) or name == 'timber_support':
            continue
        anchor = p.get('anchor') or ('ground' if {name, sprite} & standing else 'under' if {name, sprite} & hung else None)
        if anchor is None:
            if not (name in exempt or name.startswith(EXEMPT)):
                fail('unjudged', sprite, p['x'], p['y'], 'no anchor and in no list of scenery.py')
            continue
        why = judge(anchor, solid, back, pixels(p))
        if why:
            fail(why[0], sprite, p['x'], p['y'], why[1])

    # Decorations, by their part in a structure: pier columns end on a base
    # over terrain or a cutwater over the river; springings meet a capital
    # or pier top beside them and arch tops their springings; a parapet
    # stands; a background structure fills its bottom row over terrain.
    parts, decorations = {}, {}
    for d in source['decorations']:
        name = d['sprite'].removeprefix('ex10_')
        decorations[(d['col'], d['row'])] = name
        strip = re.fullmatch(r'arcade_gap_\d+_(\d)_(\d)', name)
        part = name.removeprefix('arcade_front_').removeprefix('arcade_')
        if strip and int(strip[1]) < len(GAP_STRIP) and int(strip[2]) < len(GAP_STRIP[0]):
            part = GAP_STRIP[int(strip[1])][int(strip[2])]
        elif part in ('pier_base', 'pier_cutwater'):
            part = part.removeprefix('pier_')
        elif re.fullmatch(r'pier_(front_|wet_)?\d+', part):
            part = 'shaft'
        elif part.startswith('parapet_'):
            part = 'parapet'
        elif kind(name) in STRUCTURES:
            part = 'structure'
        elif part not in GAP_STRIP[0] + GAP_STRIP[1]:
            part = None
        parts[(d['col'], d['row'])] = part
    river = source.get('river')
    piers = {}
    for (c, r), part in sorted(parts.items()):
        name = decorations[(c, r)]
        if part is None:
            fail('unjudged', name, c, r, 'a decoration in no structure')
        elif part in ('pier_top', 'capital', 'shaft', 'base', 'cutwater'):
            piers.setdefault(c, []).append(r)
        elif part in ('spring_left', 'spring_right'):
            if parts.get((c - 1 if part == 'spring_left' else c + 1, r)) not in ('capital', 'pier_top'):
                fail('arch', name, c, r, 'its springing meets no capital or pier top')
        elif part in ('arch_left', 'arch_right'):
            if parts.get((c, r + 1)) != part.replace('arch', 'spring'):
                fail('arch', name, c, r, 'stands on no springing')
        elif part == 'arch_key':
            if parts.get((c - 1, r)) != 'arch_left' or parts.get((c + 1, r)) != 'arch_right':
                fail('arch', name, c, r, 'its arch is broken')
        elif part == 'parapet':
            share, _ = stands(solid, drawn.points(name, c * tile, r * tile))
            if share < MOST:
                fail('parapet', name, c, r, f'{share:.0%} of its foot on the drawn surface')
        elif part == 'structure':
            below = (c, r + 1)
            if below in terrain and parts.get(below) != 'structure':
                w, h, m = drawn.mask(name)
                filled = [x for x in range(w) if any(m[y * w + x] for y in range(h))]
                if sum(m[(h - 1) * w + x] for x in filled) < MOST * len(filled):
                    fail('structure', name, c, r, "its bottom row stops short of its tile's foot over the terrain")
    for c, rows in sorted(piers.items()):
        low = max(rows)
        if rows != list(range(min(rows), low + 1)):
            fail('pier', decorations[(c, low)], c, low, 'its column has a hole')
        elif not ((parts[(c, low)] == 'base' and (c, low + 1) in terrain) or (parts[(c, low)] == 'cutwater' and river
                  and river['left'] <= c < river['right'] and low + 1 == river['row'])):
            fail('pier', decorations[(c, low)], c, low, 'its column ends on no base over terrain or cutwater over the river')
    return [line for _, _, line in sorted(lines)]
