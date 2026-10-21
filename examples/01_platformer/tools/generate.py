#!/usr/bin/env python3
"""Deterministic offline authoring; never invoked by Cargo or the engine."""
import argparse
import json
from pathlib import Path
import tempfile

from PIL import __version__ as pillow_version
from art import TOOLS, auxiliary, build_art, relief, validate_art
from effects import LINEAR_FILTER
from level import build_level
from sfx import HAND_AUTHORED_SOUNDS, LOOPS, build_sounds, seam_problem

ROOT=TOOLS.parent
# The template whose shell 01 copies: its theme, fonts and damage-flash
# material come with it, copied byte for byte so `--check` catches drift.
TEMPLATE=ROOT.parent.parent/'templates'/'basic'
SHELL_ASSETS=('ui/themes/shell.json','fonts/Inter/Inter-Regular.ttf','fonts/Inter/LICENSE.txt',
    'fonts/SourceSerif4/SourceSerif4-Regular.ttf','fonts/SourceSerif4/LICENSE.txt','shaders/damage_flash.wgsl')
SHELL_SECTIONS=('fonts','font_families','font_fallback','ui_themes')
# Prefab and data sources, copied verbatim under assets/ and listed by file:
# `prefabs/<id>.json`, `data/<kind>/<id>.json`.
CONTENT=TOOLS/'content'


def particle_configs():
    def config(sprite,cap,rate,lifetime,speed,scale,direction=(0,-1),spread=65,burst=False):
        return dict(sprite='ex10_'+sprite,max_alive=cap,seed=48657,blend='alpha',
            emission=dict(kind='burst',count=rate,once=True) if burst else dict(kind='continuous',rate_hz=rate),
            lifetime=dict(min=lifetime*.65,max=lifetime),initial_velocity=dict(kind='cone',direction=list(direction),spread_deg=spread,speed=dict(min=speed*.4,max=speed)),
            gravity=[0,10],drag_per_sec=.8,start_scale=dict(min=scale*.5,max=scale),
            scale_over_life=[[0,.7],[.25,1],[1,.2]],alpha_over_life=[[0,0],[.1,.8],[.65,.6],[1,0]],
            color_over_life=[[0,[1,1,1,1]],[1,[.6,.75,.85,1]]])
    configs={
        'landing_dust':config('dust',12,12,.5,180,.4,spread=155,burst=True),
        'double_jump':config('spark',18,18,.45,240,.24,spread=360,burst=True),
        'jump_puff':config('dust',8,8,.35,120,.3,spread=155,burst=True),
        'fire_trail':config('spark',40,36,1.0,115,.23,spread=95),
        'torch_embers':config('spark',24,8,2.5,60,.12),
        'waterfall_spray':config('droplet',64,32,1.5,170,.18,spread=110),
        'wind_motes':config('mote',16,4,3,45,.1,direction=(1,-.2),spread=25),
        'ball_explosion':config('spark',24,24,.55,310,.24,spread=360,burst=True),
        'small_ball_impact':config('spark',18,18,.5,230,.17,spread=360,burst=True),
        'black_hole':config('spark',900,540,1.4,260,.18,spread=360)}
    configs['double_jump'].update(initial_velocity=dict(kind='radial',speed=dict(min=130,max=240)),gravity=[0,80],color_over_life=[[0,[1,.95,.6,1]],[1,[1,.6,.15,0]]])
    configs['black_hole'].update(initial_velocity=dict(kind='radial',speed=dict(min=100,max=260)),
        lifetime=dict(min=1.5,max=1.9),gravity=[0,0],drag_per_sec=0,
        angular_velocity=dict(min=3,max=10),start_scale=dict(min=.06,max=.22),
        scale_over_life=[[0,.45],[.15,1],[.75,.6],[1,.05]],
        alpha_over_life=[[0,0],[.08,.9],[.8,.85],[1,0]],
        color_over_life=[[0,[.55,.8,1,1]],[.25,[.62,.35,1,1]],[.5,[1,.3,.8,1]],[.72,[1,.55,.18,1]],[.9,[1,.95,.8,1]],[1,[.4,.1,.5,1]]])
    configs['ball_explosion'].update(initial_velocity=dict(kind='radial',speed=dict(min=90,max=310)),gravity=[0,180],color_over_life=[[0,[1,.9,.4,1]],[.4,[1,.42,.08,1]],[1,[.35,.1,.06,0]]])
    # Warm flame bodies and finer, longer-lived torch embers; both rise.
    fire_ramp=[[0,[1,1,.8,1]],[.18,[1,.85,.25,1]],[.45,[1,.38,.035,1]],
        [.75,[.7,.09,.015,1]],[1,[.18,.025,.04,0]]]
    configs['fire_trail'].update(sprite='ex10_flame_glow',gravity=[0,-130],drag_per_sec=.3,
        lifetime=dict(min=.35,max=1.15),start_scale=dict(min=.16,max=.5),
        initial_velocity=dict(kind='cone',direction=[0,-1],spread_deg=55,speed=dict(min=45,max=145)),
        angular_velocity=dict(min=-1.8,max=1.8),color_over_life=fire_ramp,
        scale_over_life=[[0,.55],[.16,1],[.6,.65],[1,.04]])
    configs['torch_embers'].update(gravity=[0,-75],drag_per_sec=.25,
        lifetime=dict(min=.5,max=2.2),start_scale=dict(min=.045,max=.2),
        initial_velocity=dict(kind='cone',direction=[0,-1],spread_deg=40,speed=dict(min=24,max=100)),
        angular_velocity=dict(min=-4,max=4),color_over_life=fire_ramp,
        scale_over_life=[[0,.6],[.12,1],[.65,.45],[1,.04]])
    configs['small_ball_impact'].update(initial_velocity=dict(kind='radial',speed=dict(min=80,max=230)),
        gravity=[0,180],drag_per_sec=1.5,lifetime=dict(min=.22,max=.5),
        alpha_over_life=[[0,0],[.06,1],[.5,.85],[1,0]],
        color_over_life=[[0,[1,1,1,1]],[.6,[1,1,1,1]],[1,[.45,.45,.45,0]]])
    configs['waterfall_spray']['gravity']=[0,260]
    # Burning-ball wisps and sparks: 64 pool pairs stay near half the global
    # 2,048 budget, leaving room for blasts and impacts over a burning pit.
    configs['ball_burn']=dict(configs['fire_trail'],max_alive=12,
        emission=dict(kind='continuous',rate_hz=20),gravity=[0,-180],
        lifetime=dict(min=.22,max=.5),start_scale=dict(min=.08,max=.2),
        alpha_over_life=[[0,0],[.06,.75],[.45,.5],[1,0]],
        initial_velocity=dict(kind='cone',direction=[0,-1],spread_deg=50,speed=dict(min=40,max=110)))
    configs['ball_burn_embers']=dict(configs['torch_embers'],max_alive=6,
        emission=dict(kind='continuous',rate_hz=8),gravity=[0,-100],drag_per_sec=.6,
        lifetime=dict(min=.5,max=1.2),start_scale=dict(min=.045,max=.11),
        alpha_over_life=[[0,0],[.04,1],[.7,.8],[1,0]],
        initial_velocity=dict(kind='cone',direction=[0,-1],spread_deg=90,speed=dict(min=90,max=230)))
    # The fireball hazard leaks licking flames upward and molten drips downward.
    # `ball_burn` above was copied from the earlier trail and keeps those values.
    ember_ramp = [[0, [1, .98, .86, 1]], [.2, [1, .8, .32, 1]], [.5, [.98, .42, .1, 1]],
                  [.8, [.62, .11, .06, 1]], [1, [.2, .04, .05, 0]]]
    configs['fire_trail'].update(
        max_alive=32, emission=dict(kind='continuous', rate_hz=56),
        lifetime=dict(min=.3, max=.8), start_scale=dict(min=.16, max=.42),
        initial_velocity=dict(kind='cone', direction=[0, -1], spread_deg=100,
                              speed=dict(min=25, max=110)),
        gravity=[0, -170], drag_per_sec=.9, angular_velocity=dict(min=-2.5, max=2.5),
        scale_over_life=[[0, .45], [.18, 1], [.65, .6], [1, .05]],
        alpha_over_life=[[0, 0], [.07, .95], [.55, .75], [1, 0]], color_over_life=ember_ramp)
    configs['fireball_drips'] = dict(
        configs['fire_trail'], max_alive=16, emission=dict(kind='continuous', rate_hz=18),
        lifetime=dict(min=.5, max=1.0), start_scale=dict(min=.07, max=.15),
        initial_velocity=dict(kind='cone', direction=[0, 1], spread_deg=150,
                              speed=dict(min=30, max=140)),
        gravity=[0, 480], drag_per_sec=.4, angular_velocity=dict(min=0, max=0),
        scale_over_life=[[0, 1], [.7, .75], [1, .2]],
        alpha_over_life=[[0, 0], [.05, 1], [.7, .9], [1, 0]])
    # Wind motes are fireflies: they drift, wander and blink yellow-green.
    configs['wind_motes'].update(
        initial_velocity=dict(kind='cone', direction=[1, -0.2], spread_deg=120, speed=dict(min=10, max=34)),
        gravity=[0, -4], drag_per_sec=0.4, start_scale=dict(min=.06, max=.11),
        alpha_over_life=[[0, 0], [.12, 1], [.3, .2], [.48, 1], [.66, .15], [.84, .9], [1, 0]],
        color_over_life=[[0, [.82, 1, .46, 1]], [1, [.66, .9, .36, 1]]])
    # Dark gas shares the black hole's 1,200-particle budget and its spiral steering.
    configs['black_hole_dust'] = dict(
        configs['black_hole'], sprite='ex10_dust', max_alive=300,
        emission=dict(kind='continuous', rate_hz=150), lifetime=dict(min=1.6, max=2.2),
        initial_velocity=dict(kind='radial', speed=dict(min=60, max=200)),
        start_scale=dict(min=.16, max=.36), angular_velocity=dict(min=-1.5, max=1.5),
        scale_over_life=[[0, .5], [.3, 1], [1, .3]],
        alpha_over_life=[[0, 0], [.15, .5], [.7, .4], [1, 0]],
        color_over_life=[[0, [.1, .07, .24, 1]], [.6, [.16, .08, .3, 1]], [1, [.05, .02, .1, 1]]])
    # The fireball spell: a dense comet tail, a flame bloom on impact and the
    # ember-to-steam puff of a ball the black hole puts out.
    configs['spell_trail'] = dict(
        config('flame_glow', 56, 140, .42, 50, .5, spread=360),
        lifetime=dict(min=.18, max=.42), gravity=[0, -120], drag_per_sec=2,
        angular_velocity=dict(min=-3, max=3), color_over_life=ember_ramp,
        scale_over_life=[[0, 1], [.5, .6], [1, .1]],
        alpha_over_life=[[0, 0], [.06, 1], [.5, .7], [1, 0]])
    configs['fireball_blast'] = dict(
        config('flame_glow', 48, 48, .8, 760, 1.25, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=220, max=760)),
        lifetime=dict(min=.25, max=.8), start_scale=dict(min=.45, max=1.25),
        gravity=[0, -240], drag_per_sec=2.6,
        angular_velocity=dict(min=-4, max=4), color_over_life=ember_ramp,
        scale_over_life=[[0, .4], [.14, 1], [.5, .8], [1, .05]],
        alpha_over_life=[[0, 1], [.15, 1], [.55, .8], [1, 0]])
    # Fast hot fragments shoot beyond the flame bloom, arc and cool to red.
    configs['blast_embers'] = dict(
        config('spark', 64, 64, 1.1, 1250, .22, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=420, max=1250)),
        lifetime=dict(min=.35, max=1.1), start_scale=dict(min=.07, max=.22),
        gravity=[0, 650], drag_per_sec=1.2, angular_velocity=dict(min=-10, max=10),
        color_over_life=ember_ramp, alpha_over_life=[[0, 1], [.55, 1], [1, 0]])
    # Smoke grows behind the hot fragments and lingers after the flash.
    configs['blast_smoke'] = dict(
        config('dust', 20, 20, 1.5, 400, .85, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=100, max=400)),
        lifetime=dict(min=.8, max=1.5), start_scale=dict(min=.4, max=.85),
        gravity=[0, -180], drag_per_sec=2.0, angular_velocity=dict(min=-2, max=2),
        scale_over_life=[[0, .4], [.3, 1.3], [1, 2]],
        alpha_over_life=[[0, 0], [.15, .55], [.6, .4], [1, 0]],
        color_over_life=[[0, [.55, .22, .08, 1]], [.25, [.16, .12, .13, 1]],
                         [1, [.06, .055, .065, 1]]])
    configs['extinguish'] = dict(
        config('dust', 14, 14, 1.2, 110, .34, spread=140, burst=True),
        lifetime=dict(min=.7, max=1.2), start_scale=dict(min=.18, max=.34),
        gravity=[0, -90], drag_per_sec=1.6, angular_velocity=dict(min=-1.2, max=1.2),
        scale_over_life=[[0, .5], [.3, 1], [1, 1.35]],
        alpha_over_life=[[0, 0], [.08, .85], [.6, .45], [1, 0]],
        color_over_life=[[0, [1, .55, .2, 1]], [.18, [.85, .85, .9, 1]], [1, [.55, .58, .66, 1]]])
    # A hard outward dust front underneath the flames and flying embers.
    configs['blast_dust'] = dict(
        config('dust', 24, 24, .85, 1000, .7, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=650, max=1000)),
        lifetime=dict(min=.35, max=.85), start_scale=dict(min=.3, max=.7),
        gravity=[0, 40], drag_per_sec=2.4, angular_velocity=dict(min=-1.5, max=1.5),
        scale_over_life=[[0, .5], [.35, 1], [1, 1.5]],
        alpha_over_life=[[0, 0], [.06, .7], [.5, .4], [1, 0]],
        color_over_life=[[0, [1, .78, .5, 1]], [.25, [.62, .55, .5, 1]], [1, [.32, .3, .3, 1]]])
    # A marble the iron brick smashes: glass chips that fall away.
    configs['ball_smash'] = dict(
        config('spark', 24, 24, .65, 520, .16, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=150, max=520)),
        lifetime=dict(min=.25, max=.65), start_scale=dict(min=.06, max=.16),
        gravity=[0, 720], drag_per_sec=.8, angular_velocity=dict(min=-9, max=9),
        alpha_over_life=[[0, 0], [.05, 1], [.6, .85], [1, 0]],
        color_over_life=[[0, [1, 1, 1, 1]], [.5, [.8, .86, .92, 1]], [1, [.5, .55, .62, 0]]])
    # A held cold jet: snow crystals, expanding mist and surface frost splashes.
    ice_ramp = [[0, [.75, .95, 1, 1]], [.5, [.25, .65, 1, 1]], [1, [.1, .35, .8, 0]]]
    configs['ice_beam'] = dict(
        config('ice_crystal', 144, 260, .65, 1050, .2, direction=(1, 0), spread=28),
        initial_velocity=dict(kind='cone', direction=[1, 0], spread_deg=28,
                              speed=dict(min=750, max=1050)),
        lifetime=dict(min=.4, max=.65), gravity=[0, 0], drag_per_sec=.15,
        angular_velocity=dict(min=-8, max=8), start_scale=dict(min=.08, max=.2),
        scale_over_life=[[0, .5], [.18, 1], [.75, .85], [1, .15]],
        color_over_life=ice_ramp, alpha_over_life=[[0, .6], [.08, 1], [.7, .9], [1, 0]])
    configs['ice_mist'] = dict(
        config('flame_glow', 100, 120, .8, 850, 1.2, direction=(1, 0), spread=28),
        initial_velocity=dict(kind='cone', direction=[1, 0], spread_deg=28,
                              speed=dict(min=600, max=850)),
        lifetime=dict(min=.5, max=.8), gravity=[0, 0], drag_per_sec=.2,
        start_scale=dict(min=.4, max=1.2), angular_velocity=dict(min=-2, max=2),
        scale_over_life=[[0, .3], [.3, 1.2], [.8, 2], [1, 2.5]],
        color_over_life=[[0, [.9, 1, 1, 1]], [.4, [.4, .8, 1, 1]], [1, [.2, .55, 1, 0]]],
        alpha_over_life=[[0, 0], [.12, .65], [.6, .4], [1, 0]])
    # The fire spray: sparks streaming along the aim through a hot haze, a
    # shorter reach than the cold spray's.
    fire_ramp = [[0, [1, .95, .6, 1]], [.45, [1, .5, .12, 1]], [1, [.45, .12, .05, 0]]]
    configs['flame_spray'] = dict(
        config('spark', 120, 220, .4, 620, .22, direction=(1, 0), spread=28),
        initial_velocity=dict(kind='cone', direction=[1, 0], spread_deg=28,
                              speed=dict(min=420, max=620)),
        lifetime=dict(min=.25, max=.4), gravity=[0, -60], drag_per_sec=.3,
        angular_velocity=dict(min=-6, max=6), start_scale=dict(min=.1, max=.24),
        scale_over_life=[[0, .6], [.2, 1], [1, .2]],
        color_over_life=fire_ramp, alpha_over_life=[[0, .6], [.1, 1], [.7, .8], [1, 0]])
    configs['flame_mist'] = dict(
        configs['ice_mist'],
        color_over_life=[[0, [1, .9, .5, 1]], [.4, [1, .45, .1, 1]], [1, [.3, .1, .05, 0]]],
        initial_velocity=dict(kind='cone', direction=[1, 0], spread_deg=28,
                              speed=dict(min=380, max=520)))
    configs['ice_contact'] = dict(
        config('ice_crystal', 48, 100, .55, 190, .2, spread=360),
        initial_velocity=dict(kind='radial', speed=dict(min=55, max=190)),
        lifetime=dict(min=.25, max=.55), gravity=[0, 80], drag_per_sec=1.2,
        angular_velocity=dict(min=-6, max=6), color_over_life=ice_ramp,
        alpha_over_life=[[0, .8], [.15, 1], [.7, .8], [1, 0]])
    configs['ice_frost'] = dict(
        config('ice_crystal', 24, 24, .7, 240, .24, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=60, max=240)),
        lifetime=dict(min=.3, max=.7), angular_velocity=dict(min=-5, max=5),
        gravity=[0, 55], drag_per_sec=1.8, color_over_life=ice_ramp,
        alpha_over_life=[[0, 1], [.6, .8], [1, 0]])
    configs['ice_shatter'] = dict(
        configs['ice_frost'], sprite='ex10_ice_chip', max_alive=32,
        emission=dict(kind='burst', count=32, once=True),
        initial_velocity=dict(kind='radial', speed=dict(min=150, max=600)),
        gravity=[0, 600], lifetime=dict(min=.35, max=.9), start_scale=dict(min=.08, max=.25),
        angular_velocity=dict(min=-12, max=12))
    return configs


SHADERS=('soft_glow',)
# Soft-glow uniforms: falloff exponent, hot-core boost, dither steps, peak alpha.
MATERIALS={'soft_halo':('soft_glow',[1.8,0.0,1.0,0.1]),'soft_flame':('soft_glow',[2.2,0.35,1.0,0.6])}


def mirror_facing(images,lit,animations):
    """Left-facing copies of the courier's frames and clips, and of the fireball's:
    nothing flips a sprite in place, so the kit's animator, and 01's fireballs, show
    mirrored art by facing. Their maps derive from the mirrored silhouettes."""
    from PIL import ImageOps
    for clip in sorted(c for c in animations if c.startswith('player') or c=='fireball'):
        frames=[]
        for frame in animations[clip]['frames']:
            name=frame['sprite'].removeprefix('ex10_');left=name+'_left'
            images[left]=ImageOps.mirror(images[name])
            if name in lit:lit.add(left)
            frames.append(dict(frame,sprite='ex10_'+left))
        animations[clip+'_left']=dict(animations[clip],frames=frames)


def cast_twins(images,lit):
    """Each courier frame's twin on a cast, its lantern hot (`actors.cast_frame`):
    01 shows it for a moment after each cast. A left-facing twin is the
    right-facing one mirrored, as its frame is."""
    from PIL import ImageOps
    from actors import cast_frame
    for name in sorted(n for n in images if n.startswith('player') and not n.endswith('_left')):
        twin=cast_frame(images[name]);images[name+'_cast']=twin;lit.add(name+'_cast')
        if name+'_left' in images:images[name+'_cast_left']=ImageOps.mirror(twin);lit.add(name+'_cast_left')


def write_json(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')


def generate(dest):
    if pillow_version!='12.3.0':raise RuntimeError('Use tools/requirements.txt: Pillow==12.3.0')
    images,lit,animations=build_art();mirror_facing(images,lit,animations);cast_twins(images,lit)
    tiled,source,level_prefabs=build_level(images,lit);validate_art(images,lit)
    configs=particle_configs()
    manifest={section:{} for section in ['sprites','animations','tilemaps','particles','shaders','materials','prefabs']}
    manifest['sounds']=json.loads((TOOLS/'sounds.json').read_text())
    template=json.loads((TEMPLATE/'assets/manifest.json').read_text())
    for section in SHELL_SECTIONS:manifest[section]=template[section]
    manifest['shaders']['damage_flash']=template['shaders']['damage_flash']
    manifest['materials']['damage_flash']=template['materials']['damage_flash']
    owned=[]
    def output(path):
        owned.append(path);out=dest/path;out.parent.mkdir(parents=True,exist_ok=True);return out
    for name,im in sorted(images.items()):
        path=f'sprites/{name}.png'
        im.save(output('assets/'+path),compress_level=9,optimize=False)
        # Glow gradients are sampled smoothly; everything else stays crisp pixel art.
        entry=dict(path=path,filter='linear' if name in LINEAR_FILTER else 'nearest')
        if name in lit:
            normal,emissive=auxiliary(im,relief(name))
            for key,suffix,img in [('normal_map','n',normal),('emissive_mask','e',emissive)]:
                path=f'sprites/{name}_{suffix}.png';entry[key]=path
                img.save(output('assets/'+path),compress_level=9,optimize=False)
        manifest['sprites']['ex10_'+name]=entry
    for section,records in [('animations',animations),('particles',configs)]:
        for name,data in sorted(records.items()):
            path=f'{section}/{name}.json';write_json(output('assets/'+path),data)
            manifest[section]['ex10_'+name]=dict(path=path)
    manifest['tilemaps']['ex10_level']=dict(path='tilemaps/level.tmj')
    # The class of the level's `emitters` objects: each object's properties
    # set its emitter's config and seed over this prefab's.
    write_json(output('assets/prefabs/ambient_emitter.json'),dict(components=dict(
        ambient_emitter=None,particle_emitter=dict(config='ex10_torch_embers'))))
    manifest['prefabs']['ex10_ambient_emitter']=dict(path='prefabs/ambient_emitter.json')
    for path in SHELL_ASSETS:
        output('assets/'+path).write_bytes((TEMPLATE/'assets'/path).read_bytes())
    for item in sorted(CONTENT.rglob('*')):
        if not item.is_file():continue
        path=item.relative_to(CONTENT).as_posix()
        output('assets/'+path).write_bytes(item.read_bytes())
        parts=path.removesuffix('.json').split('/')
        if parts[0]=='prefabs' and len(parts)==2:manifest['prefabs'][parts[1]]=dict(path=path)
        elif parts[0]=='data' and len(parts)==3:manifest.setdefault('data',{}).setdefault(parts[1],{})[parts[2]]=dict(path=path)
        else:raise ValueError('content outside prefabs/ and data/<kind>/: '+path)
    # The soft-glow shader is authored in tools/shaders and owned like any output.
    for name in SHADERS:
        path=f'shaders/{name}.wgsl'
        output('assets/'+path).write_bytes((TOOLS/path).read_bytes())
        manifest['shaders']['ex10_'+name]=dict(path=path)
    for name,(shader,f32s) in MATERIALS.items():
        manifest['materials']['ex10_'+name]=dict(shader='ex10_'+shader,uniform_defaults=dict(
            vec4=[[0.0]*4 for _ in range(4)],f32s=f32s,i32s=[0]*4))
    # Synthesized WAVs are owned outputs; every registered sound must be one of
    # them or an explicitly hand-authored file.
    sounds = build_sounds()
    for name, data in sorted(sounds.items()):
        if name in LOOPS and (problem := seam_problem(name, data)):
            raise ValueError(f'loop {name}: {problem}')
        output('assets/sounds/' + name).write_bytes(data)
    authored = {'sounds/' + name for name in sounds} | set(HAND_AUTHORED_SOUNDS)
    for sound in manifest['sounds'].values():
        if sound['path'] not in authored:
            raise ValueError(f"sound {sound['path']} is neither synthesized nor hand-authored")
    write_json(output('assets/tilemaps/level.tmj'),tiled)
    # Where each courier frame's lantern flame sits, which the lantern's glow follows.
    path='data/lantern_anchors/player.json'
    write_json(output('assets/'+path),{'ex10_'+name:anchor for name,anchor in source['lantern_anchors']})
    manifest.setdefault('data',{}).setdefault('lantern_anchors',{})['player']=dict(path=path)
    # The prefabs the map's object layers name, one per class.
    for name,prefab in sorted(level_prefabs.items()):
        path=f'prefabs/{name}.json';write_json(output('assets/'+path),prefab)
        manifest['prefabs'][name]=dict(path=path)
    validate_references(manifest,animations,configs,source)
    write_json(output('assets/manifest.json'),manifest)
    owned.append('tools/outputs.json')
    write_json(dest/'tools/outputs.json',sorted(owned))
    return sorted(owned)


def validate_references(manifest,animations,configs,source):
    # 01 loads its manifest as its only root, so IDs need only be unique in
    # it; the art's keep their `ex10_` prefix, the shell's take theirs.
    for section in ('sprites','animations','particles'):
        if any(not s.startswith('ex10_') for s in manifest[section]):raise ValueError('unprefixed '+section+' ID')
    sprites=set(manifest['sprites'])
    for clip in animations.values():
        for frame in clip['frames']:
            if frame['sprite'] not in sprites:raise ValueError('unknown animation sprite')
    for cfg in configs.values():
        if cfg['sprite'] not in sprites:raise ValueError('unknown particle sprite')
    for material in manifest['materials'].values():
        if material['shader'] not in manifest['shaders']:raise ValueError('unknown material shader')
    for p in source['props']:
        if p['animation'] and p['animation'] not in manifest['animations']:raise ValueError('unknown prop animation')
    if sum(configs[p['config'].removeprefix('ex10_')]['max_alive'] for p in source['emitters'])>256:raise ValueError('ambient cap exceeds 256')
    if len({p['seed'] for p in source['emitters']})!=len(source['emitters']):raise ValueError('duplicate emitter seed')


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');args=parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='platformer-generate-') as temp:
        dest=Path(temp);owned=generate(dest)
        previous=json.loads((TOOLS/'outputs.json').read_text()) if (TOOLS/'outputs.json').exists() else json.loads((TOOLS/'legacy_outputs.json').read_text())
        obsolete=sorted(set(previous)-set(owned))
        if args.check:
            changed=[p for p in owned if not (ROOT/p).exists() or (ROOT/p).read_bytes()!=(dest/p).read_bytes()]
            changed += [p for p in obsolete if (ROOT/p).exists()]
            # No uncovered stray PNGs or configs may silently evade the inventory.
            expected=set(owned)|{'assets/'+path for path in HAND_AUTHORED_SOUNDS}
            changed += [str(p.relative_to(ROOT)) for p in (ROOT/'assets').rglob('*') if p.is_file() and str(p.relative_to(ROOT)) not in expected]
            if changed:raise SystemExit('Generated outputs differ: '+', '.join(sorted(set(changed))))
            print(f'OK: {len(owned)} owned outputs match; art, level, references and budgets validated')
        else:
            for path in obsolete:
                # Only the explicit previous inventory authorizes deletion.
                (ROOT/path).unlink(missing_ok=True)
            for path in owned:
                target=ROOT/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((dest/path).read_bytes())
            print(f'Wrote {len(owned)} outputs; removed {len(obsolete)} obsolete owned files')


if __name__=='__main__':main()
