"""Hand-authored platforms -> the Tiled map: tile layers, and object layers whose
classes name the prefabs generated here for the level's props, slab colliders,
hazards, moving platforms and areas."""
import json
import math
from PIL import Image
from art import TOOLS, rng_for

# What the player's touch on a hazard costs, and the push away from it.
HAZARD_DAMAGE = 34
HAZARD_KNOCKBACK = 560
# Each hazard's touch box, its half extents and centre from the sprite's
# top-left: the spikes' teeth sit low in their tile, the fireball fills it.
SPIKES_HALF, SPIKES_CENTRE = [27, 14], [32, 48]
FIRE_HALF, FIRE_CENTRE = [18, 24], [32, 32]
# Draw order among the level's sprites, every tile layer under them: props
# behind the world, then the world's props, hazards and decks.
Z_BACK, Z_WORLD, Z_DECK, Z_SPIKES, Z_FIRE = -20, -10, -4, -5, 6
# The soft glow and point light a prop or a hazard carries: its centre from the
# sprite's top-left, its radius and its colour, the light's strength (0.8 when
# left out) and a fireball's hot core.
GLOWS = {'ex10_lantern': dict(offset=[32, 36], radius=150, color=[255, 181, 98, 200]),
         'ex10_crystal': dict(offset=[32, 36], radius=150, color=[94, 210, 255, 180])}
FIRE_GLOW = dict(offset=FIRE_CENTRE, radius=175, color=[255, 125, 45, 255], light=1.6, core=62)
# Molten drips leak from each fireball's underside, a child emitter.
FIRE_DRIPS = 'ex10_fire_drips'


def slab_runs(slabs):
    """`slabs` (`[x, y, width, height]`) with every run that shares a row and a
    height and touches end to end merged into one box, by row, then x: one
    collider per platform, so a body sunk into a deck meets no join."""
    runs = []
    for x, y, w, h in sorted(slabs, key=lambda s: (s[1], s[3], s[0])):
        if runs and runs[-1][1] == y and runs[-1][3] == h and runs[-1][0] + runs[-1][2] == x:
            runs[-1][2] += w
        else:
            runs.append([x, y, w, h])
    return runs


def build_objects(source, images, lit):
    """The map's object layers, in spawn order, and the prefabs their classes
    name: `{id: prefab}`. A parameter held in an array (a box's half extents,
    an oscillator's amplitude, a path's points) takes a prefab of its own."""
    tile = source['tile']
    prefabs, layers = {}, []

    def layer(name, objects):
        layers.append(dict(id=0, type='objectgroup', name=name, draworder='index', visible=True,
            opacity=1, x=0, y=0, objects=objects))

    def obj(cls, x, y, name='', properties=()):
        return dict(id=0, name=name, type=cls, x=x, y=y, width=0, height=0, rotation=0,
            visible=True, point=True, properties=list(properties))

    props = []
    for p in source['props']:
        cls = f"ex10_prop_{p['sprite'].removeprefix('ex10_')}_{p['depth']}"
        prefab = dict(sprite=dict(asset_id=p['sprite'], z_order=Z_BACK if p['depth'] == 'back' else Z_WORLD),
            components={})
        if p['animation']:
            prefab['components']['animation'] = dict(id=p['animation'])
        if p['sprite'] in GLOWS:
            prefab['components']['glow'] = GLOWS[p['sprite']]
        if prefabs.setdefault(cls, prefab) != prefab:
            raise ValueError('prop class with two shapes: ' + cls)
        props.append(obj(cls, p['x'] * tile, p['y'] * tile))
    layer('props', props)

    slabs = []
    for x, y, w, h in slab_runs(source['slab_colliders']):
        cls = f'ex10_slab_{int(w)}x{int(h)}'
        prefabs[cls] = dict(components=dict(rigid_body=dict(kind='static',
            shape=dict(kind='aabb', half_extents=[w / 2, h / 2]), offset=[w / 2, h / 2])))
        slabs.append(dict(obj(cls, x, y), width=w, height=h, point=False))
    layer('slabs', slabs)

    hazards = []
    touch = dict(damage=HAZARD_DAMAGE, filter=dict(all=['team.player']), knockback=HAZARD_KNOCKBACK)
    for i, h in enumerate(source['hazards']):
        centre = [v * tile for v in h['position']]
        if not h['fire']:
            prefabs['ex10_spikes'] = dict(sprite=dict(asset_id='ex10_spikes', z_order=Z_SPIKES), components=dict(
                hazard=dict(fire=False), contact_damage=dict(touch, shape=dict(aabb=SPIKES_HALF), offset=SPIKES_CENTRE)))
            hazards.append(obj('ex10_spikes', centre[0] - SPIKES_CENTRE[0], centre[1] - SPIKES_CENTRE[1]))
            continue
        # Today's sine motion, centre + travel * sin(2 pi t / period + phase), on the
        # kit's oscillator: it starts where the sine stands at t 0.
        phase = (h['phase'] / math.tau) % 1.0
        start = [c + t * tile * math.sin(h['phase']) for c, t in zip(centre, h['travel'])]
        cls = f'ex10_fire_{i}'
        prefabs[FIRE_DRIPS] = dict(components=dict(particle_emitter=dict(config='ex10_fireball_drips')))
        # A fireball faces its horizontal travel; one that only rises and falls, or
        # stands, watches the player. The animator shows the mirrored clip by facing.
        # Its trail and drips are children at its centre and underside, since an
        # emitter emits from its entity's point, the sprite's top-left.
        trail = f'ex10_fire_trail_{i}'
        prefabs[trail] = dict(components=dict(particle_emitter=dict(config='ex10_fire_trail', seed=700 + i)))
        components = dict(hazard=dict(fire=True), glow=FIRE_GLOW,
            character_animator=dict(idle=dict(left='ex10_fireball_left', right='ex10_fireball')),
            contact_damage=dict(touch, shape=dict(aabb=FIRE_HALF), offset=FIRE_CENTRE),
            spawn_children=[dict(prefab=trail, offset=FIRE_CENTRE),
                            dict(prefab=FIRE_DRIPS, offset=[FIRE_CENTRE[0], FIRE_CENTRE[1] + 12])])
        if h['travel'][0]:
            components['facing'] = {'from': 'motion', 'snap': 'horizontal'}
        else:
            components['aim'] = dict({'from': 'target'}, target=dict(nearest=dict(all=['team.player'])),
                offset=FIRE_CENTRE)
            components['facing'] = {'from': 'aim', 'snap': 'horizontal'}
        if any(h['travel']):
            components['oscillator'] = dict(position=dict(amplitude=[t * tile for t in h['travel']],
                period=h['period']), phase=phase)
        prefabs[cls] = dict(sprite=dict(asset_id='ex10_fireball_0', z_order=Z_FIRE), components=components)
        hazards.append(obj(cls, start[0] - FIRE_CENTRE[0], start[1] - FIRE_CENTRE[1]))
    layer('hazards', hazards)

    platforms = []
    deck = images['lift_deck']
    for i, m in enumerate(source['moving_platforms']):
        half = [m['half_width'] * tile, source['deck_depth'] / 2]
        # Three deck tiles across its width, as today's deck draws.
        width = round(2 * half[0])
        art = Image.new('RGBA', (width, tile))
        for col in range(3):
            left, right = round(col * width / 3), round((col + 1) * width / 3)
            art.paste(deck.resize((right - left, tile), Image.NEAREST), (left, 0))
        name = f'lift_deck_{i}'
        images[name] = art
        lit.add(name)
        # A ping-pong between the sine's two ends, eased by a sine, is today's motion:
        # each leg takes half a period.
        travel = [t * tile for t in m['travel']]
        centre = [v * tile for v in m['position']]
        cls = f'ex10_platform_{i}'
        prefabs[cls] = dict(sprite=dict(asset_id='ex10_' + name, z_order=Z_DECK), components=dict(
            rigid_body=dict(kind='kinematic', shape=dict(kind='aabb', half_extents=half), offset=half),
            path_follower=dict(points=[[0, 0], [2 * travel[0], 2 * travel[1]]],
                speed=4 * math.hypot(*travel) / m['period'], mode='ping_pong', easing='sine_in_out')))
        platforms.append(obj(cls, centre[0] - travel[0] - half[0], centre[1] - travel[1] - half[1]))
    layer('platforms', platforms)

    areas = []
    for i, area in enumerate(source['landmarks']):
        first, last = area['columns']
        half = [(last + 1 - first) * tile / 2, source['rows'] * tile / 2]
        cls = f'ex10_area_{i}'
        prefabs[cls] = dict(components=dict(area=dict(name=area['name']), zone=dict(filter=dict(all=['team.player'])),
            rigid_body=dict(kind='sensor', shape=dict(kind='aabb', half_extents=half), offset=half)))
        areas.append(obj(cls, first * tile, 0, name=area['name']))
    layer('areas', areas)

    # The ambient emitters as point objects of the prefab class
    # `ex10_ambient_emitter`; their properties set the emitter's config and seed.
    layer('emitters', [obj('ex10_ambient_emitter', p['x'] * tile, p['y'] * tile, properties=[
        dict(name='particle_emitter.config', type='string', value=p['config']),
        dict(name='particle_emitter.seed', type='int', value=p['seed'])]) for p in source['emitters']])
    # Markers the game reads by name: the player's feet at a fresh run's start.
    spawn = source['spawn']
    layer('markers', [obj('', spawn['col'] * tile, spawn['surface_row'] * tile, name='spawn')])
    # The training dummies, the map's last layer, so every earlier object keeps
    # its save ID; each placement's tags set the `dummy` prefab's, one by one.
    layer('dummies', [obj('dummy', d['x'] * tile, d['y'] * tile, name=d['name'], properties=[
        dict(name=f'tags.{i}', type='string', value=tag) for i, tag in enumerate(d['tags'])])
        for d in source['dummies']])
    return layers, prefabs


def build_level(images, lit):
    source=json.loads((TOOLS/'level.json').read_text())
    from placement import prepare
    source=prepare(source,images)
    cols,rows=source['cols'],source['rows']
    catalog=sorted('ex10_'+s for s in images if s not in ('sky','distant_ridges','near_woodland','clouds_far','clouds_near'))
    gids={s:i+1 for i,s in enumerate(catalog)}
    layers={name:[0]*(cols*rows) for name in ['background','decorations','terrain','foreground','collision']}
    rng=rng_for('terrain-variation')
    occupied=set()
    def put(layer,x,y,sprite):
        if not 0<=x<cols or not 0<=y<rows:raise ValueError('placement outside map')
        layers[layer][y*cols+x]=gids[sprite]
    for p in source['platforms']:
        for y in range(p['row'],p['bottom']):
            for x in range(p['left'],p['right']):
                occupied.add((x,y))
    for x,y in sorted(occupied,key=lambda xy:(xy[1],xy[0])):
        p=min((p for p in source['platforms'] if p['left']<=x<p['right'] and p['row']<=y<p['bottom']),key=lambda p:p['style']!='ground')
        above=(x,y-1) in occupied; left=(x-1,y) in occupied; right=(x+1,y) in occupied
        if above:
            # Exposed wall faces get finished edges; the variant is drawn regardless
            # so the random stream, and every other tile's variant, stays the same.
            variant=rng.randrange(8)
            sprite='cliff_left' if not left else 'cliff_right' if not right else f'rock_{variant}'
        elif p['style']!='ground':
            sprite=p['style']
        elif not left:sprite='ground_left'
        elif not right:sprite='ground_right'
        else:sprite='moss_rock_'+str(rng.randrange(8))
        put('terrain',x,y,'ex10_'+sprite)
        if p['style']=='ground':put('collision',x,y,'ex10_ground')
    for p in source['decorations']:put(p['layer'],p['col'],p['row'],p['sprite'])
    tile=source['tile']
    tile_layers=[dict(id=i+1,type='tilelayer',name=name,width=cols,height=rows,visible=True,opacity=1,x=0,y=0,data=data,properties=[dict(name='kind',type='string',value='collision') ] if name=='collision' else []) for i,(name,data) in enumerate(layers.items())]
    # The object layers sit before `collision`, so the first and last layers stay
    # tile layers; IDs number layers, then objects, through the map.
    objects,prefabs=build_objects(source,images,lit)
    next_id=1
    for group in objects:
        for o in group['objects']:
            o['id']=next_id;next_id+=1
    tiled=dict(type='map',version='1.10',orientation='orthogonal',renderorder='right-down',infinite=False,width=cols,height=rows,tilewidth=tile,tileheight=tile,
        nextobjectid=next_id,layers=tile_layers[:-1]+objects+tile_layers[-1:],
        tilesets=[dict(firstgid=1,name='twilight_ruin',tilewidth=tile,tileheight=tile,tilecount=len(catalog),columns=0,tiles=[dict(id=i,image='../sprites/'+s.removeprefix('ex10_')+'.png',imagewidth=tile,imageheight=tile,properties=[dict(name='sprite_id',type='string',value=s)]) for i,s in enumerate(catalog)])])
    for i,group in enumerate(tiled['layers']):group['id']=i+1
    validate_level(source,tiled,set(gids))
    return tiled,source,prefabs


def validate_level(source,tiled,sprite_ids):
    cols,rows=source['cols'],source['rows']; ts=tiled['tilesets'][0]
    if ts['firstgid']!=1 or [t['id'] for t in ts['tiles']]!=list(range(ts['tilecount'])):raise ValueError('non-contiguous tileset')
    for t in ts['tiles']:
        if t['properties'][0]['value'] not in sprite_ids:raise ValueError('unknown sprite reference')
    for layer in tiled['layers']:
        if layer['type']!='tilelayer':continue
        if len(layer['data'])!=cols*rows:raise ValueError('wrong layer length')
        if any(not isinstance(g,int) or g<0 or g>ts['tilecount'] for g in layer['data']):raise ValueError('invalid GID')
    collision=next(l['data'] for l in tiled['layers'] if l['name']=='collision')
    terrain=next(l['data'] for l in tiled['layers'] if l['name']=='terrain')
    shallow={(int(x/source['tile']),int(y/source['tile'])) for x,y,w,h in source.get('slab_colliders',[])}
    if any(bool(terrain[i])!=(bool(collision[i]) or (i%cols,i//cols) in shallow) for i in range(cols*rows)):raise ValueError('route/collider disagreement')
    def safe(x,y):
        return terrain[y*cols+x] and all(not terrain[(y-dy)*cols+x] for dy in (1,))
    if not safe(int(source['spawn']['col']),source['spawn']['surface_row']):raise ValueError('unsafe spawn')
    platforms={p['name']:p for p in source['platforms']}
    for route in source['routes'].values():
        for name in route:
            p=platforms[name]
            if not any(safe(x,p['row']) for x in range(p['left'],p['right'])):raise ValueError('route lacks headroom')
    for p in source.get('hazards',[])+source.get('moving_platforms',[]):
        values=p['position']+p['travel']+[p['period'],p['phase']]
        if not all(math.isfinite(v) for v in values) or p['period']<=0:raise ValueError('invalid motion')
        for center,travel,limit in zip(p['position'],p['travel'],(cols,rows)):
            if center-abs(travel)<0 or center+abs(travel)>=limit:raise ValueError('motion outside map')
    spawn=(source['spawn']['col'],source['spawn']['surface_row']-28/source['tile'])
    for p in source.get('hazards',[]):
        if all(abs(center-safe)<abs(travel)+1 for center,safe,travel in zip(p['position'],spawn,p['travel'])):raise ValueError('hazard at spawn')
    for p in source['props']+source['decorations']:
        if p['sprite'] not in sprite_ids:raise ValueError('unknown prop reference')
