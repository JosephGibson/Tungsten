#!/usr/bin/env python3
"""Deterministic offline authoring; never invoked by Cargo or the engine."""
import argparse
import json
from pathlib import Path
import tempfile

from PIL import __version__ as pillow_version
from art import TOOLS, auxiliary, build_art, validate_art
from level import build_level

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
        'black_hole':config('spark',1200,720,1.4,260,.18,spread=360)}
    configs['double_jump'].update(initial_velocity=dict(kind='radial',speed=dict(min=130,max=240)),gravity=[0,80],color_over_life=[[0,[1,.95,.6,1]],[1,[1,.6,.15,0]]])
    configs['black_hole'].update(initial_velocity=dict(kind='radial',speed=dict(min=100,max=260)),gravity=[0,-16],angular_velocity=dict(min=-9.5,max=9.5),color_over_life=[[0,[.95,.72,1,1]],[.32,[.82,.24,1,1]],[1,[.08,0,.24,1]]])
    configs['ball_explosion'].update(initial_velocity=dict(kind='radial',speed=dict(min=90,max=310)),gravity=[0,180],color_over_life=[[0,[1,.9,.4,1]],[.4,[1,.42,.08,1]],[1,[.35,.1,.06,0]]])
    configs['fire_trail'].update(gravity=[0,-65],drag_per_sec=.4,color_over_life=[[0,[1,1,.5,1]],[.35,[1,.45,.08,1]],[1,[.6,.1,.02,0]]])
    configs['waterfall_spray']['gravity']=[0,260]
    return configs


def write_json(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')


def generate(dest):
    if pillow_version!='12.3.0':raise RuntimeError('Use tools/requirements.txt: Pillow==12.3.0')
    images,lit,animations=build_art();validate_art(images,lit)
    tiled,rust,source=build_level(images)
    configs=particle_configs()
    manifest={section:{} for section in ['sprites','animations','tilemaps','particles']}
    manifest['sounds']=json.loads((TOOLS/'sounds.json').read_text())
    owned=[]
    def output(path):
        owned.append(path);out=dest/path;out.parent.mkdir(parents=True,exist_ok=True);return out
    for name,im in sorted(images.items()):
        path=f'sprites/{name}.png'
        im.save(output('assets/'+path),compress_level=9,optimize=False)
        entry=dict(path=path,filter='nearest')
        if name in lit:
            normal,emissive=auxiliary(im)
            for key,suffix,img in [('normal_map','n',normal),('emissive_mask','e',emissive)]:
                path=f'sprites/{name}_{suffix}.png';entry[key]=path
                img.save(output('assets/'+path),compress_level=9,optimize=False)
        manifest['sprites']['ex10_'+name]=entry
    for section,records in [('animations',animations),('particles',configs)]:
        for name,data in sorted(records.items()):
            path=f'{section}/{name}.json';write_json(output('assets/'+path),data)
            manifest[section]['ex10_'+name]=dict(path=path)
    manifest['tilemaps']['ex10_level']=dict(path='tilemaps/level.tmj')
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
            expected=set(owned)|{'assets/sounds/black_hole.ogg'}
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
