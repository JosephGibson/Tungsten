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
from sfx import HAND_AUTHORED_SOUNDS, build_sounds

ROOT=TOOLS.parent


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
        config('flame_glow', 22, 22, .65, 320, .8, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=80, max=320)),
        lifetime=dict(min=.3, max=.65), start_scale=dict(min=.35, max=.8),
        gravity=[0, -90], drag_per_sec=3.2, color_over_life=ember_ramp,
        scale_over_life=[[0, .6], [.2, 1], [1, .15]],
        alpha_over_life=[[0, 0], [.05, 1], [.5, .8], [1, 0]])
    configs['extinguish'] = dict(
        config('dust', 14, 14, 1.2, 110, .34, spread=140, burst=True),
        lifetime=dict(min=.7, max=1.2), start_scale=dict(min=.18, max=.34),
        gravity=[0, -90], drag_per_sec=1.6, angular_velocity=dict(min=-1.2, max=1.2),
        scale_over_life=[[0, .5], [.3, 1], [1, 1.35]],
        alpha_over_life=[[0, 0], [.08, .85], [.6, .45], [1, 0]],
        color_over_life=[[0, [1, .55, .2, 1]], [.18, [.85, .85, .9, 1]], [1, [.55, .58, .66, 1]]])
    # The fireball's push: a dust ring thrown out to about its 160-pixel reach.
    configs['blast_dust'] = dict(
        config('dust', 20, 20, .8, 560, .55, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=300, max=560)),
        lifetime=dict(min=.45, max=.8), start_scale=dict(min=.35, max=.6),
        gravity=[0, 40], drag_per_sec=3.6, angular_velocity=dict(min=-1.5, max=1.5),
        scale_over_life=[[0, .5], [.35, 1], [1, 1.5]],
        alpha_over_life=[[0, 0], [.06, .7], [.5, .4], [1, 0]],
        color_over_life=[[0, [1, .78, .5, 1]], [.25, [.62, .55, .5, 1]], [1, [.32, .3, .3, 1]]])
    # A marble the iron brick smashes: glass chips that fall away.
    configs['ball_smash'] = dict(
        config('spark', 12, 12, .55, 260, .12, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=90, max=260)),
        lifetime=dict(min=.3, max=.6), start_scale=dict(min=.06, max=.13),
        gravity=[0, 720], drag_per_sec=.8, angular_velocity=dict(min=-9, max=9),
        alpha_over_life=[[0, 0], [.05, 1], [.6, .85], [1, 0]],
        color_over_life=[[0, [1, 1, 1, 1]], [.5, [.8, .86, .92, 1]], [1, [.5, .55, .62, 0]]])
    return configs


SHADERS=('soft_glow',)
# Soft-glow uniforms: falloff exponent, hot-core boost, dither steps, peak alpha.
MATERIALS={'soft_halo':('soft_glow',[1.8,0.0,1.0,0.1]),'soft_flame':('soft_glow',[2.2,0.35,1.0,0.6])}


def write_json(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')


def generate(dest):
    if pillow_version!='12.3.0':raise RuntimeError('Use tools/requirements.txt: Pillow==12.3.0')
    images,lit,animations=build_art();validate_art(images,lit)
    tiled,rust,source=build_level(images)
    configs=particle_configs()
    manifest={section:{} for section in ['sprites','animations','tilemaps','particles','shaders','materials']}
    manifest['sounds']=json.loads((TOOLS/'sounds.json').read_text())
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
        output('assets/sounds/' + name).write_bytes(data)
    authored = {'sounds/' + name for name in sounds} | set(HAND_AUTHORED_SOUNDS)
    for sound in manifest['sounds'].values():
        if sound['path'] not in authored:
            raise ValueError(f"sound {sound['path']} is neither synthesized nor hand-authored")
    write_json(output('assets/tilemaps/level.tmj'),tiled)
    output('src/level_layout.rs').write_text(rust)
    validate_references(manifest,animations,configs,source)
    write_json(output('assets/manifest.json'),manifest)
    owned.append('tools/outputs.json')
    write_json(dest/'tools/outputs.json',sorted(owned))
    return sorted(owned)


def validate_references(manifest,animations,configs,source):
    ids=[s for entries in manifest.values() for s in entries]
    if len(ids)!=len(set(ids)) or any(not s.startswith('ex10_') for s in ids):raise ValueError('duplicate or unprefixed ID')
    shared=json.loads((ROOT.parent.parent/'assets/manifest.json').read_text())
    if set(ids)&{s for entries in shared.values() if isinstance(entries,dict) for s in entries}:raise ValueError('shared manifest ID collision')
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
