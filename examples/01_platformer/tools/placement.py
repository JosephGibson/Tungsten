"""Resolve explicit scenery supports and pixel-tight platform collision bounds."""
import json
from PIL import Image
from art import PALETTE, TOOLS

# How far a prop hung under a platform (`anchor` `under`) tucks its top
# opaque row up into the underside's ragged lip, in pixels.
UNDER_TUCK = 6


def underside(support, images, tile):
    """Where `support`'s underside lies, in tiles: a ground platform's
    bottom, a deck's visible depth under its row."""
    from level import FAMILIES
    if support['style'] in FAMILIES:
        return support['bottom']
    return support['row'] + images[support['style']].getbbox()[3] / tile


def prepare(source, images):
    tile=source['tile'];platforms={p['name']:p for p in source['platforms']}
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
    source['lantern_anchors']=[]
    for name,im in sorted(images.items()):
        if not name.startswith('player') or name.endswith('_left'):continue
        flame=[(x+.5,y+.5) for y in range(30,60) for x in range(40,63) if im.getpixel((x,y)) in (PALETTE['y'],PALETTE['Y'])]
        if not flame:raise ValueError('player frame lacks lantern flame: '+name)
        anchor=[round(sum(p[a] for p in flame)/len(flame),3) for a in (0,1)]
        source['lantern_anchors'].append((name,anchor))
        # A left-facing frame is the right-facing one mirrored, its lantern too.
        if name+'_left' in images:source['lantern_anchors'].append((name+'_left',[64-anchor[0],anchor[1]]))
    source['lantern_anchors'].sort()
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
    for group in source['groups']:
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
        for p in source['props']:
            if p['sprite'].startswith(name+'_big_') and x<=p['x']<x+w and y<=p['y']<y+h:p['y']+=target_y-y
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
