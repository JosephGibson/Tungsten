"""Resolve explicit scenery supports and pixel-tight platform collision bounds."""
from PIL import Image
from art import PALETTE


def prepare(source, images):
    tile=source['tile'];platforms={p['name']:p for p in source['platforms']}
    # Every thin platform owns shallow rectangles instead of full tile collision.
    source['slab_colliders']=[]
    for p in source['platforms']:
        if p['style']=='ground':continue
        box=images[p['style']].getbbox()
        if box[:2]!=(0,0) or box[2]!=tile:raise ValueError('platform silhouette needs explicit horizontal bounds')
        for x in range(p['left'],p['right']):
            # Portions buried in solid terrain need no second, overlapping body.
            if any(q['style']=='ground' and q['left']<=x<q['right'] and q['row']<=p['row']<q['bottom'] for q in source['platforms']):continue
            source['slab_colliders'].append([x*tile,p['row']*tile,tile,box[3]])
    source['lantern_anchors']=[]
    for name,im in sorted(images.items()):
        if not name.startswith('player'):continue
        flame=[(x+.5,y+.5) for y in range(30,60) for x in range(40,63) if im.getpixel((x,y)) in (PALETTE['y'],PALETTE['Y'])]
        if not flame:raise ValueError('player frame lacks lantern flame: '+name)
        source['lantern_anchors'].append((name,[round(sum(p[a] for p in flame)/len(flame),3) for a in (0,1)]))
    source['deck_depth']=images['lift_deck'].getbbox()[3]
    for p in source['props']:
        if 'support' not in p:continue
        support=platforms[p['support']];im=images[p['sprite'].removeprefix('ex10_')];box=im.getbbox()
        center=p['x']+(box[0]+box[2])/2/tile
        if not support['left']<=center<support['right']:raise ValueError('prop outside support')
        if p['anchor']=='ground':
            # A new solid stair supersedes its buried parent surface.
            covers=[q for q in source['platforms'] if q['style']=='ground' and q['left']<=center<q['right'] and q['row']<support['row']<=q['bottom']]
            if covers:support=min(covers,key=lambda q:q['row']);p['support']=support['name']
            p['y']=support['row']-box[3]/tile
        else:
            depth=images[support['style']].getbbox()[3]
            p['y']=support['row']+(depth-box[1])/tile
    for group in source['groups']:
        name=group['sprite'];x=group['x'];y=group['y'];w=group['width'];h=group['height'];support=platforms[group['support']]
        composite=Image.new('RGBA',(w*tile,h*tile))
        for r in range(h):
            for c in range(w):composite.paste(images[f'{name.removeprefix("ex10_")}_big_{r}_{c}'],(c*tile,r*tile))
        box=composite.getbbox();edge=box[1] if group['anchor']=='wall' else box[3]
        target_y=support['row']-edge/tile
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
    # The cliff face anchors the waterfall's lip and the full falling column.
    waterfall=source['waterfall_source']
    for x in range(waterfall['left'],waterfall['right']):
        for row in range(waterfall['row'],source['rows']):
            # The column's top tile is a mossy spring where the water breaks out.
            sprite='ex10_cliff_back_top' if row==waterfall['row'] else 'ex10_cliff_back'
            source['decorations'].append(dict(sprite=sprite,col=x,row=row,layer='background'))
    # Reposition flame emitters to the grounded lantern's actual flame.
    for emitter in source['emitters']:
        if emitter['config']=='ex10_torch_embers':
            lamp=min((p for p in source['props'] if p['sprite']=='ex10_lantern'),key=lambda p:abs(p['x']+.5-emitter['x']))
            emitter['x']=lamp['x']+.5;emitter['y']=lamp['y']+.5
    return source
