"""Hand-authored platforms -> Tiled and typed Rust placement tables."""
import json
import math
import re
import subprocess
from art import TOOLS, rng_for


def build_level(images):
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
    tiled=dict(type='map',version='1.10',orientation='orthogonal',renderorder='right-down',infinite=False,width=cols,height=rows,tilewidth=tile,tileheight=tile,
        layers=[dict(id=i+1,type='tilelayer',name=name,width=cols,height=rows,visible=True,opacity=1,x=0,y=0,data=data,properties=[dict(name='kind',type='string',value='collision') ] if name=='collision' else []) for i,(name,data) in enumerate(layers.items())],
        tilesets=[dict(firstgid=1,name='twilight_ruin',tilewidth=tile,tileheight=tile,tilecount=len(catalog),columns=0,tiles=[dict(id=i,image='../sprites/'+s.removeprefix('ex10_')+'.png',imagewidth=tile,imageheight=tile,properties=[dict(name='sprite_id',type='string',value=s)]) for i,s in enumerate(catalog)])])
    validate_level(source,tiled,set(gids))
    return tiled,rust_layout(source),source


def validate_level(source,tiled,sprite_ids):
    cols,rows=source['cols'],source['rows']; ts=tiled['tilesets'][0]
    if ts['firstgid']!=1 or [t['id'] for t in ts['tiles']]!=list(range(ts['tilecount'])):raise ValueError('non-contiguous tileset')
    for t in ts['tiles']:
        if t['properties'][0]['value'] not in sprite_ids:raise ValueError('unknown sprite reference')
    for layer in tiled['layers']:
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


def rust_layout(s):
    lines=['//! Generated by tools/generate.py from tools/level.json; edit the source.',
        'pub(crate) const TILE: f32 = '+str(float(s['tile']))+';',
        f'pub(crate) const MAP_COLS: u32 = {s["cols"]};',f'pub(crate) const MAP_ROWS: u32 = {s["rows"]};',
        f'pub(crate) const SPAWN_COL: f32 = {float(s["spawn"]["col"])};',f'pub(crate) const SPAWN_ROW: f32 = {float(s["spawn"]["surface_row"])};',f'pub(crate) const KILL_ROW: f32 = {float(s["kill_row"])};',
        '#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub(crate) enum PropDepth { Back, World }',
        'pub(crate) struct PropPlacement { pub(crate) sprite: &\'static str, pub(crate) animation: Option<&\'static str>, pub(crate) tile_position: [f32; 2], pub(crate) depth: PropDepth }',
        'pub(crate) const PROPS: &[PropPlacement] = &[']
    for p in s['props']:
        anim='Some("'+p['animation']+'")' if p['animation'] else 'None'
        lines.append(f'PropPlacement {{ sprite: "{p["sprite"]}", animation: {anim}, tile_position: [{float(p["x"])}, {float(p["y"])}], depth: PropDepth::{p["depth"].title()} }},')
    lines += ['];', 'pub(crate) const LANTERN_ANCHORS: &[(&str, [f32;2])] = &[']
    for name,anchor in s['lantern_anchors']:
        lines.append(f'("ex10_{name}", [{anchor[0]}, {anchor[1]}]),')
    lines += ['];', f"pub(crate) const DECK_DEPTH: f32 = {float(s['deck_depth'])};", 'pub(crate) const SLAB_COLLIDERS: &[[f32; 4]] = &[']
    for rectangle in s['slab_colliders']:lines.append('['+', '.join(str(float(v)) for v in rectangle)+'],')
    lines+= ['];','pub(crate) struct EmitterPlacement { pub(crate) config: &\'static str, pub(crate) tile_position: [f32; 2], pub(crate) seed: u64 }','pub(crate) const EMITTERS: &[EmitterPlacement] = &[']
    for p in s['emitters']:lines.append(f'EmitterPlacement {{ config: "{p["config"]}", tile_position: [{float(p["x"])}, {float(p["y"])}], seed: {p["seed"]} }},')
    lines += ['];', '#[derive(Clone, Copy)] pub(crate) struct MotionPlacement { pub(crate) position: [f32;2], pub(crate) travel: [f32;2], pub(crate) period: f32, pub(crate) phase: f32 }',
        'pub(crate) struct HazardPlacement { pub(crate) fire: bool, pub(crate) motion: MotionPlacement }',
        'pub(crate) const HAZARDS: &[HazardPlacement] = &[']
    def motion(p):
        return 'MotionPlacement { position: ['+', '.join(str(float(v)) for v in p['position'])+'], travel: ['+', '.join(str(float(v)) for v in p['travel'])+f"], period: {float(p['period'])}, phase: {float(p['phase'])} }}"
    for p in s['hazards']:lines.append('HazardPlacement { fire: '+str(p['fire']).lower()+', motion: '+motion(p)+' },')
    lines += ['];', 'pub(crate) struct PlatformPlacement { pub(crate) motion: MotionPlacement, pub(crate) half_width: f32 }', 'pub(crate) const MOVING_PLATFORMS: &[PlatformPlacement] = &[']
    for p in s['moving_platforms']:lines.append('PlatformPlacement { motion: '+motion(p)+f", half_width: {float(p['half_width'])} }},")
    lines+= ['];','#[cfg(test)] pub(crate) struct RoutePlatform { pub(crate) name: &\'static str, pub(crate) left: f32, pub(crate) right: f32, pub(crate) row: f32 }','#[cfg(test)] pub(crate) const PLATFORMS: &[RoutePlatform] = &[']
    for p in s['platforms']:lines.append(f'RoutePlatform {{ name: "{p["name"]}", left: {float(p["left"])}, right: {float(p["right"])}, row: {float(p["row"])} }},')
    lines+= ['];','#[cfg(test)] pub(crate) const ROUTES: &[(&str, &[&str])] = &[']
    for name,route in sorted(s['routes'].items()):lines.append(f'("{name}", &['+', '.join('"'+n+'"' for n in route)+']),')
    lines+= ['];']
    # Emit the same formatting used by cargo fmt; no post-generation drift.
    def readable_float(match):
        whole,fraction=match.groups()
        whole=f'{int(whole):_}'
        fraction='_'.join(fraction[i:i+3] for i in range(0,len(fraction),3))
        return whole+'.'+fraction
    text=re.sub(r'\b(\d+)\.(\d+)\b',readable_float,'\n'.join(lines)+'\n')
    return subprocess.run(['rustfmt' ,'--edition','2024','--emit','stdout'],input=text,text=True,capture_output=True,check=True).stdout
