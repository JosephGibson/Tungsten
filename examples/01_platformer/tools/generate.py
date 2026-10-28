#!/usr/bin/env python3
"""Deterministic offline authoring; never invoked by Cargo or the engine."""
import argparse
from dataclasses import dataclass
from datetime import date
import hashlib
import io
import json
from pathlib import Path
import re
import tempfile
from typing import Callable

from PIL import __version__ as pillow_version
from art import TOOLS, art_families, auxiliary, build_art, relief, validate_sets, validate_sprites
from effects import LINEAR_FILTER
from level import build_level, opaque_tiles, tiled_text
from placement import floating
from sfx import HAND_AUTHORED_SOUNDS, LOOPS, MUSIC, SOUNDS, build_sounds, seam_problem

ROOT=TOOLS.parent
# The template whose shell 01 copies: its fonts and damage-flash material
# come with it, copied byte for byte so `--check` catches drift, and its
# theme in 01's own colours.
TEMPLATE=ROOT.parent.parent/'templates'/'basic'
SHELL_ASSETS=('fonts/Inter/Inter-Regular.ttf','fonts/Inter/LICENSE.txt',
    'fonts/SourceSerif4/SourceSerif4-Regular.ttf','fonts/SourceSerif4/LICENSE.txt','shaders/damage_flash.wgsl')
# 01's theme (01 (h); its background and surface darker in 01 (i)): the
# template's text, every colour of the template's mapped to 01's, darker
# backgrounds and a vivid purple, so new template keys still arrive and
# `just repo-check` holds its structure to the template's. A template
# colour the map lacks stops the generator, named.
THEME='ui/themes/shell.json'
THEME_COLORS={
    '#0b0c10':'#020306',  # the background and accent text, a pressed button's text and toggle, the slider's border
    '#15171d':'#060812',  # the surface
    '#1e2129':'#11142a',  # a hovered or focused fill
    '#646b7a':'#5c6594',  # the border and the slider
    '#e9ebf1':'#e8eaf6',  # text
    '#a3a9b6':'#a3aac8',  # muted text
    '#f0a830':'#a46cff',  # the accent
    '#ffd36b':'#cbb0ff',  # focus
    '#6c7280':'#666d92',  # disabled text
    '#ff6b6b':'#ff6b78',  # danger
}


def themed(text):
    """The template's theme `text` in 01's colours: each `#rrggbb`, an
    `#rrggbbaa`'s alpha kept, through `THEME_COLORS`."""
    def mapped(match):
        rgb,alpha=match.group(1).lower(),match.group(2) or ''
        if rgb not in THEME_COLORS:raise ValueError(f'{THEME}: template colour {rgb} has no colour of 01\'s (THEME_COLORS)')
        return THEME_COLORS[rgb]+alpha
    return re.sub(r'(#[0-9a-fA-F]{6})([0-9a-fA-F]{2})?\b',mapped,text)
SHELL_SECTIONS=('fonts','font_families','font_fallback','ui_themes')
# 01's own face beside the template's: Source Serif 4 Bold, under the same
# licence as its Regular, for the area titles, from the repository's fonts.
SERIF_BOLD=ROOT.parent.parent/'assets'/'fonts'/'SourceSerif4'/'static'/'SourceSerif4-Bold.ttf'
# Prefab and data sources, copied verbatim under assets/ and listed by file:
# `prefabs/<id>.json`, `data/<kind>/<id>.json`.
CONTENT=TOOLS/'content'
# The accepted families' files' hashes (`D-223`): `--lock` and `--unlock` write
# it; every run and `--check` hold the locked families to it.
LOCKS=TOOLS/'locks.json'


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
        # A beetle's charge (M79): a few low brown puffs at its feet.
        'charge_dust':config('dust',6,5,.45,90,.34,direction=(0,-1),spread=150,burst=True),
        'fire_trail':config('spark',40,36,1.0,115,.23,spread=95),
        'torch_embers':config('spark',24,8,2.5,60,.12),
        'waterfall_spray':config('droplet',64,32,1.5,170,.18,spread=110),
        'wind_motes':config('mote',16,4,3,45,.1,direction=(1,-.2),spread=25),
        'ball_explosion':config('spark',24,24,.55,310,.24,spread=360,burst=True),
        'small_ball_impact':config('spark',18,18,.5,230,.17,spread=360,burst=True),
        # A third of 01 (a)'s sparks a hole, so three open holes fit the budget.
        'black_hole':config('spark',300,180,1.4,260,.18,spread=360)}
    configs['double_jump'].update(initial_velocity=dict(kind='radial',speed=dict(min=130,max=240)),gravity=[0,80],color_over_life=[[0,[1,.95,.6,1]],[1,[1,.6,.15,0]]])
    configs['black_hole'].update(initial_velocity=dict(kind='radial',speed=dict(min=100,max=260)),
        lifetime=dict(min=1.5,max=1.9),gravity=[0,0],drag_per_sec=0,
        angular_velocity=dict(min=3,max=10),start_scale=dict(min=.06,max=.22),
        scale_over_life=[[0,.45],[.15,1],[.75,.6],[1,.05]],
        alpha_over_life=[[0,0],[.08,.9],[.8,.85],[1,0]],
        color_over_life=[[0,[.55,.8,1,1]],[.25,[.62,.35,1,1]],[.5,[1,.3,.8,1]],[.72,[1,.55,.18,1]],[.9,[1,.95,.8,1]],[1,[.4,.1,.5,1]]])
    configs['charge_dust'].update(gravity=[0,40],drag_per_sec=1.6,color_over_life=[[0,[.66,.54,.4,1]],[1,[.42,.34,.26,0]]])
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
    # Dark gas shares a black hole's 400-particle budget and its spiral
    # steering: three open holes take 1,200 of the 2,048.
    configs['black_hole_dust'] = dict(
        configs['black_hole'], sprite='ex10_dust', max_alive=100,
        emission=dict(kind='continuous', rate_hz=50), lifetime=dict(min=1.6, max=2.2),
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
    # The held sprays (01 (h)): drawn as animated sprites (`effects`'
    # `SPRAY_CLIPS`, `toys::spray_look`), their particles accents along the
    # aim, each reaching 30 % further than 01 (f)'s. The cold spray: a
    # scatter of crystals along the cone and a light mist, its contact's
    # frost splashes; the fire spray: embers thrown along the cone and smoke
    # rising off its tip. A target the cone damages bursts every second
    # pulse: shards and glints, or embers, thrown off its face and falling.
    def aimed(sprite, cap, rate, life, speed, scale, spread=26, burst=False):
        return dict(sprite='ex10_' + sprite, max_alive=cap, seed=48657, blend='alpha',
                    emission=dict(kind='burst', count=rate, once=True) if burst
                    else dict(kind='continuous', rate_hz=rate),
                    lifetime=dict(min=life[0], max=life[1]),
                    initial_velocity=dict(kind='cone', direction=[1, 0], spread_deg=spread,
                                          speed=dict(min=speed[0], max=speed[1])),
                    gravity=[0, 0], drag_per_sec=.15, start_scale=dict(min=scale[0], max=scale[1]),
                    scale_over_life=[[0, .7], [.25, 1], [1, .2]],
                    alpha_over_life=[[0, 0], [.1, .8], [.65, .6], [1, 0]],
                    color_over_life=[[0, [1, 1, 1, 1]], [1, [.6, .75, .85, 1]]])
    ice_ramp = [[0, [.75, .95, 1, 1]], [.5, [.25, .65, 1, 1]], [1, [.1, .35, .8, 0]]]
    # 01 (j): the chill's flakes white and fine, fading to a pale blue.
    frost_ramp = [[0, [1, 1, 1, 1]], [.6, [.88, .96, 1, 1]], [1, [.78, .9, 1, 0]]]
    fire_ramp = [[0, [1, 1, .88, 1]], [.22, [1, .86, .38, 1]], [.5, [1, .52, .12, 1]],
                 [1, [.5, .13, .05, 0]]]
    configs['ice_beam'] = dict(
        aimed('ice_crystal', 110, 170, (.4, .65), (975, 1365), (.1, .24)),
        angular_velocity=dict(min=-8, max=8),
        scale_over_life=[[0, .5], [.18, 1], [.75, .85], [1, .15]],
        color_over_life=ice_ramp, alpha_over_life=[[0, .6], [.08, 1], [.7, .9], [1, 0]])
    configs['ice_mist'] = dict(
        aimed('flame_glow', 70, 80, (.5, .8), (780, 1100), (.45, 1.1)),
        drag_per_sec=.2, angular_velocity=dict(min=-2, max=2),
        scale_over_life=[[0, .3], [.3, 1.1], [.8, 1.8], [1, 2.2]],
        color_over_life=[[0, [.9, 1, 1, 1]], [.4, [.4, .8, 1, 1]], [1, [.2, .55, 1, 0]]],
        alpha_over_life=[[0, 0], [.12, .45], [.6, .3], [1, 0]])
    configs['frost_hit'] = dict(
        aimed('ice_crystal', 16, 12, (.22, .45), (140, 440), (.2, .4), spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=140, max=440)),
        gravity=[0, 420], drag_per_sec=1.6, angular_velocity=dict(min=-10, max=10),
        scale_over_life=[[0, 1], [1, .3]],
        color_over_life=[[0, [1, 1, 1, 1]], [.5, [.7, .92, 1, 1]], [1, [.4, .7, 1, 0]]],
        alpha_over_life=[[0, 1], [.7, .9], [1, 0]])
    configs['flame_spray'] = dict(
        aimed('spark', 120, 200, (.28, .45), (710, 1048), (.1, .22), spread=24),
        gravity=[0, -60], drag_per_sec=.3, angular_velocity=dict(min=-6, max=6),
        scale_over_life=[[0, .6], [.2, 1], [1, .2]],
        color_over_life=fire_ramp, alpha_over_life=[[0, .7], [.1, 1], [.7, .85], [1, 0]])
    configs['flame_mist'] = dict(
        aimed('dust', 70, 70, (.6, 1.0), (420, 600), (.45, 1.0), spread=28),
        gravity=[0, -260], drag_per_sec=1.1, angular_velocity=dict(min=-2, max=2),
        scale_over_life=[[0, .4], [.4, 1.1], [1, 1.9]],
        color_over_life=[[0, [1, .8, .45, 1]], [.3, [.55, .3, .18, 1]], [1, [.16, .12, .12, 0]]],
        alpha_over_life=[[0, 0], [.15, .5], [.7, .3], [1, 0]])
    configs['flame_hit'] = dict(
        aimed('spark', 18, 14, (.2, .45), (160, 520), (.14, .3), spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=160, max=520)),
        gravity=[0, 520], drag_per_sec=1.2, angular_velocity=dict(min=-8, max=8),
        scale_over_life=[[0, 1], [1, .25]],
        color_over_life=fire_ramp, alpha_over_life=[[0, 1], [.7, .9], [1, 0]])
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
    # 01 (b): the gun's flash and ricochets, the statuses' drips, flakes and
    # sparks, and the endings' bursts, all over today's sprites.
    configs['muzzle_flash'] = dict(
        config('spark', 10, 10, .12, 420, .12, direction=(1, 0), spread=50, burst=True),
        lifetime=dict(min=.06, max=.12), gravity=[0, 0], drag_per_sec=6,
        alpha_over_life=[[0, 1], [1, 0]],
        color_over_life=[[0, [1, 1, .85, 1]], [.5, [1, .7, .25, 1]], [1, [1, .35, .1, 0]]])
    configs['ricochet'] = dict(
        config('spark', 12, 12, .3, 520, .1, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=160, max=520)),
        lifetime=dict(min=.12, max=.3), gravity=[0, 600], drag_per_sec=1.2,
        alpha_over_life=[[0, 1], [.7, .8], [1, 0]],
        color_over_life=[[0, [1, 1, .8, 1]], [.5, [1, .75, .3, 1]], [1, [.6, .3, .1, 0]]])
    configs['steam_burst'] = dict(
        config('dust', 28, 28, 1.1, 420, .7, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=140, max=420)),
        lifetime=dict(min=.6, max=1.1), start_scale=dict(min=.35, max=.7),
        gravity=[0, -120], drag_per_sec=2.2, angular_velocity=dict(min=-1.5, max=1.5),
        scale_over_life=[[0, .5], [.3, 1.1], [1, 1.6]],
        alpha_over_life=[[0, 0], [.08, .8], [.6, .45], [1, 0]],
        color_over_life=[[0, [1, 1, 1, 1]], [1, [.75, .8, .88, 1]]])
    # 01 (f): bleeding drips denser, larger and a brighter maroon.
    configs['bleed_drips'] = dict(
        config('droplet', 24, 14, .8, 50, .17, direction=(0, 1), spread=50),
        gravity=[0, 480], drag_per_sec=.2, lifetime=dict(min=.45, max=.8),
        start_scale=dict(min=.1, max=.17), scale_over_life=[[0, 1], [1, .8]],
        alpha_over_life=[[0, 0], [.08, 1], [.8, .9], [1, 0]],
        color_over_life=[[0, [.8, .1, .16, 1]], [.6, [.62, .06, .12, 1]], [1, [.42, .04, .09, 0]]])
    configs['chill_flakes'] = dict(
        config('ice_crystal', 10, 6, 1.4, 30, .12, spread=180),
        gravity=[0, 40], drag_per_sec=.8, angular_velocity=dict(min=-3, max=3),
        lifetime=dict(min=.8, max=1.4), start_scale=dict(min=.06, max=.12),
        alpha_over_life=[[0, 0], [.15, .9], [.7, .7], [1, 0]], color_over_life=frost_ramp)
    # e01sfx (V30): shock's sparks over the crisp glint, white-hot at birth
    # and fading through lilac to violet, darting out and stopping short,
    # flickering on and off as they go, quicker and twice as many.
    configs['shock_sparks'] = dict(
        config('ice_glint', 24, 90, .22, 540, 1.2, spread=360),
        initial_velocity=dict(kind='radial', speed=dict(min=220, max=540)),
        lifetime=dict(min=.1, max=.22), gravity=[0, 0], drag_per_sec=7,
        angular_velocity=dict(min=-20, max=20), start_scale=dict(min=.7, max=1.2),
        scale_over_life=[[0, 1], [.5, .85], [1, .4]],
        alpha_over_life=[[0, 1], [.18, .25], [.32, 1], [.5, .2], [.64, 1], [.82, .3], [1, 0]],
        color_over_life=[[0, [1, 1, 1, 1]], [.4, [.92, .86, 1, 1]], [.7, [.7, .46, 1, 1]],
                         [1, [.46, .24, .92, 1]]])
    configs['burn_out'] = dict(
        config('dust', 16, 16, 1.4, 160, .6, spread=120, burst=True),
        lifetime=dict(min=.8, max=1.4), gravity=[0, -140], drag_per_sec=1.4,
        angular_velocity=dict(min=-1.5, max=1.5), start_scale=dict(min=.3, max=.6),
        scale_over_life=[[0, .5], [.4, 1.2], [1, 1.6]],
        alpha_over_life=[[0, 0], [.1, .75], [.6, .5], [1, 0]],
        color_over_life=[[0, [.75, .3, .1, 1]], [.25, [.2, .17, .17, 1]], [1, [.08, .07, .08, 1]]])
    configs['poof'] = dict(
        config('dust', 14, 14, .7, 220, .45, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=60, max=220)),
        lifetime=dict(min=.4, max=.7), gravity=[0, -40], drag_per_sec=3,
        scale_over_life=[[0, .5], [.3, 1], [1, 1.3]],
        alpha_over_life=[[0, 0], [.08, .85], [.6, .5], [1, 0]],
        color_over_life=[[0, [.88, .84, .76, 1]], [1, [.6, .58, .55, 1]]])
    configs['frost_blast'] = dict(
        configs['ice_shatter'], sprite='ex10_ice_crystal', max_alive=40,
        emission=dict(kind='burst', count=40, once=True),
        initial_velocity=dict(kind='radial', speed=dict(min=200, max=720)),
        gravity=[0, 220], lifetime=dict(min=.3, max=.8), start_scale=dict(min=.1, max=.28))
    # 01 (e): the staff light guttering out, a curl of smoke off its last
    # orange, and a crawler's puff of frost breath as it shrinks from the light.
    configs['staff_gutter'] = dict(
        config('dust', 10, 10, 1.1, 70, .32, spread=40, burst=True),
        lifetime=dict(min=.7, max=1.1), gravity=[0, -70], drag_per_sec=1.8,
        angular_velocity=dict(min=-1.2, max=1.2), start_scale=dict(min=.14, max=.32),
        scale_over_life=[[0, .4], [.3, 1], [1, 1.6]],
        alpha_over_life=[[0, 0], [.1, .7], [.6, .35], [1, 0]],
        color_over_life=[[0, [1, .7, .35, 1]], [.15, [.55, .52, .5, 1]], [1, [.3, .3, .32, 1]]])
    configs['frost_breath'] = dict(
        config('flame_glow', 12, 12, .7, 90, .5, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=25, max=90)),
        lifetime=dict(min=.45, max=.7), gravity=[0, -20], drag_per_sec=2.4,
        start_scale=dict(min=.25, max=.5), scale_over_life=[[0, .4], [.35, 1], [1, 1.5]],
        color_over_life=[[0, [.9, 1, 1, 1]], [.5, [.55, .85, 1, 1]], [1, [.3, .6, 1, 0]]],
        alpha_over_life=[[0, 0], [.1, .7], [.6, .4], [1, 0]])
    # 01 (c): a frozen target's glints twinkling about its cube and drifting
    # off it over the 16 px `ice_glint`, and the wisp's embers rising and
    # cooling off its trailing side over the spark.
    configs['frozen_glints'] = dict(
        config('ice_glint', 14, 9, 1.2, 30, 1.0, spread=360),
        initial_velocity=dict(kind='radial', speed=dict(min=6, max=30)),
        lifetime=dict(min=.6, max=1.2), gravity=[0, 14], drag_per_sec=1.4,
        angular_velocity=dict(min=-2, max=2), start_scale=dict(min=.4, max=1.0),
        scale_over_life=[[0, .2], [.2, 1], [.45, .45], [.7, 1], [1, 0]],
        alpha_over_life=[[0, 0], [.1, 1], [.35, .35], [.6, 1], [1, 0]],
        color_over_life=[[0, [1, 1, 1, 1]], [1, [.72, .9, 1, 1]]])
    configs['wisp_embers'] = dict(
        configs['torch_embers'], max_alive=24, emission=dict(kind='continuous', rate_hz=14),
        initial_velocity=dict(kind='cone', direction=[-.3, -1], spread_deg=50,
                              speed=dict(min=20, max=75)),
        gravity=[0, -90], lifetime=dict(min=.4, max=1.2), start_scale=dict(min=.04, max=.15))
    # The quick fixes: an orb taken, a ring of sparks in its own colour that
    # rise and fade, with a few glints among them.
    for kind, ramp in (('health', [[0, [1, .78, .74, 1]], [.3, [1, .3, .28, 1]], [1, [.55, .06, .1, 0]]]),
                       ('mana', [[0, [.85, .92, 1, 1]], [.3, [.4, .55, 1, 1]], [1, [.18, .14, .62, 0]]])):
        configs[f'orb_{kind}'] = dict(
            config('spark', 26, 26, .75, 190, .3, spread=360, burst=True),
            initial_velocity=dict(kind='radial', speed=dict(min=70, max=190)),
            lifetime=dict(min=.45, max=.75), gravity=[0, -60], drag_per_sec=2.2,
            start_scale=dict(min=.14, max=.3), scale_over_life=[[0, .6], [.2, 1], [1, .1]],
            alpha_over_life=[[0, 0], [.08, 1], [.6, .7], [1, 0]], color_over_life=ramp)
    # 01 (f): a blow's blood and a bleed's first spurt, the splat of what
    # iron kills, a burning target's embers, the push's warp in the gravity
    # purple (palette `Ж`–`Щ`), a strike's sparks, a thaw's glints, iron's
    # hard landing and a refused spell's fizzle.
    blood = [[0, [.96, .2, .22, 1]], [.3, [.78, .08, .13, 1]], [1, [.42, .03, .08, 0]]]
    configs['blood_burst'] = dict(
        config('droplet', 18, 18, .6, 420, .24, spread=150, burst=True),
        lifetime=dict(min=.32, max=.6), gravity=[0, 1100], drag_per_sec=1.1,
        start_scale=dict(min=.12, max=.24), scale_over_life=[[0, 1], [.7, .85], [1, .5]],
        alpha_over_life=[[0, 1], [.7, .95], [1, 0]], color_over_life=blood)
    configs['blood_spurt'] = dict(
        config('droplet', 12, 12, .7, 380, .2, spread=40, burst=True),
        lifetime=dict(min=.4, max=.7), gravity=[0, 1000], drag_per_sec=.8,
        start_scale=dict(min=.1, max=.2), scale_over_life=[[0, 1], [.7, .85], [1, .5]],
        alpha_over_life=[[0, 1], [.7, .95], [1, 0]], color_over_life=blood)
    configs['gore_burst'] = dict(
        config('droplet', 56, 56, .9, 760, .4, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=180, max=760)),
        lifetime=dict(min=.45, max=.9), gravity=[0, 1300], drag_per_sec=1.4,
        start_scale=dict(min=.16, max=.4), scale_over_life=[[0, .8], [.15, 1], [1, .6]],
        alpha_over_life=[[0, 1], [.75, .95], [1, 0]],
        color_over_life=[[0, [.9, .14, .18, 1]], [.25, [.64, .05, .1, 1]], [1, [.26, .02, .05, 0]]])
    configs['burn_embers'] = dict(
        configs['torch_embers'], max_alive=24, emission=dict(kind='continuous', rate_hz=24),
        initial_velocity=dict(kind='cone', direction=[0, -1], spread_deg=70,
                              speed=dict(min=30, max=110)),
        gravity=[0, -120], lifetime=dict(min=.35, max=1.0), start_scale=dict(min=.06, max=.2))
    configs['strike_sparks'] = dict(
        config('spark', 32, 32, .5, 650, .2, spread=150, burst=True),
        lifetime=dict(min=.18, max=.5), gravity=[0, 1200], drag_per_sec=1.0,
        angular_velocity=dict(min=-12, max=12), start_scale=dict(min=.08, max=.2),
        alpha_over_life=[[0, 1], [.6, .9], [1, 0]],
        color_over_life=[[0, [1, 1, 1, 1]], [.25, [.85, .75, 1, 1]], [.6, [.62, .42, 1, 1]],
                         [1, [.36, .2, .75, 0]]])
    configs['thaw_glints'] = dict(
        config('ice_glint', 16, 16, .9, 160, .9, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=40, max=160)),
        lifetime=dict(min=.45, max=.9), gravity=[0, 260], drag_per_sec=1.6,
        angular_velocity=dict(min=-3, max=3), start_scale=dict(min=.45, max=.9),
        scale_over_life=[[0, 1], [.5, .7], [1, 0]], alpha_over_life=[[0, 1], [.6, .8], [1, 0]],
        color_over_life=[[0, [1, 1, 1, 1]], [.5, [.75, .93, 1, 1]], [1, [.45, .75, 1, 1]]])
    configs['iron_dust'] = dict(
        config('dust', 22, 22, .9, 420, .62, spread=170, burst=True),
        lifetime=dict(min=.5, max=.9), gravity=[0, 60], drag_per_sec=3.0,
        angular_velocity=dict(min=-1.5, max=1.5), start_scale=dict(min=.3, max=.62),
        scale_over_life=[[0, .45], [.3, 1], [1, 1.45]],
        alpha_over_life=[[0, 0], [.06, .8], [.55, .45], [1, 0]],
        color_over_life=[[0, [.78, .7, .6, 1]], [.4, [.55, .5, .46, 1]], [1, [.32, .3, .3, 1]]])
    configs['fizzle'] = dict(
        config('spark', 16, 16, .45, 140, .16, spread=360, burst=True),
        initial_velocity=dict(kind='radial', speed=dict(min=30, max=140)),
        lifetime=dict(min=.25, max=.45), gravity=[0, 220], drag_per_sec=3.2,
        angular_velocity=dict(min=-8, max=8), start_scale=dict(min=.07, max=.16),
        alpha_over_life=[[0, .9], [.3, 1], [.5, .3], [.7, .9], [1, 0]],
        color_over_life=[[0, [.92, .86, 1, 1]], [.4, [.6, .55, .7, 1]], [1, [.3, .28, .34, 0]]])
    # 01 (g): a lit fuse's sparks and smoke, a crate catching fire and
    # falling in, a keg's staves flung out over the missile's four bursts,
    # and a rock breaking where it lands.
    def full(sprite, cap, emission, life, velocity, gravity, drag, scale, size, alpha, color, spin=(0, 0)):
        return dict(sprite='ex10_' + sprite, max_alive=cap, seed=48657, blend='alpha', emission=emission,
                    lifetime=dict(min=life[0], max=life[1]), initial_velocity=velocity, gravity=list(gravity),
                    drag_per_sec=drag, start_scale=dict(min=scale[0], max=scale[1]), scale_over_life=size,
                    alpha_over_life=alpha, color_over_life=color, angular_velocity=dict(min=spin[0], max=spin[1]))
    def burst(count):
        return dict(kind='burst', count=count, once=True)
    def cone(direction, spread, low, high):
        return dict(kind='cone', direction=list(direction), spread_deg=spread, speed=dict(min=low, max=high))
    # 01 (j): the frost's flakes by its stage, a pulse every 0.45 s of one,
    # one, two and three crystals, each emitter its target's own.
    for stage, (count, cap) in enumerate(((1, 3), (1, 4), (2, 7), (3, 10)), start=1):
        configs[f'chill_flakes_{stage}'] = full(
            'ice_crystal', cap, dict(kind='pulse', count_per_pulse=count, interval_sec=.45), (.8, 1.3),
            cone((0, -1), 180, 10, 24 + 4 * stage), (0, 36), .8, (.05, .08 + .01 * stage),
            [[0, .7], [.25, 1], [1, .2]], [[0, 0], [.15, .75], [.7, .55], [1, 0]], frost_ramp, (-3, 3))
    # 01 (j): a hole's opening, sparks set on a ring about it that rush in
    # to the core over its 0.3 s (`hole_particles` places and steers them).
    configs['hole_open'] = full(
        'spark', 72, burst(72), (.32, .4), dict(kind='radial', speed=dict(min=360, max=520)), (0, 0), 0,
        (.08, .22), [[0, .6], [.5, 1], [1, .3]], [[0, 0], [.1, 1], [.8, .9], [1, 0]],
        [[0, [.85, .75, 1, 1]], [.5, [.96, .55, .98, 1]], [1, [1, .92, .8, 1]]], (3, 10))
    # 01 (j): the force push's wave kicks grit up where it scrapes a floor,
    # accents to its dust's clip; thrown up in a cone, so it needs no turn
    # for a left aim.
    configs['push_grit'] = full(
        'dust', 16, burst(6), (.25, .5), cone((0, -1), 110, 140, 360), (0, 900), 2.0, (.05, .12),
        [[0, 1], [1, .5]], [[0, .9], [.7, .6], [1, 0]],
        [[0, [.66, .6, .52, 1]], [1, [.4, .36, .32, 0]]], (-3, 3))
    configs['fuse_sparks'] = full(
        'spark', 18, dict(kind='continuous', rate_hz=48), (.16, .4), cone((0, -1), 200, 50, 170), (0, 420), .6,
        (.05, .12), [[0, 1], [1, .3]], [[0, 1], [.7, .9], [1, 0]],
        [[0, [1, 1, .86, 1]], [.3, [1, .86, .32, 1]], [.7, [1, .42, .1, 1]], [1, [.5, .1, .05, 0]]])
    configs['fuse_smoke'] = full(
        'dust', 8, dict(kind='continuous', rate_hz=7), (.6, 1.0), cone((0, -1), 35, 15, 40), (0, -30), 1.2,
        (.08, .14), [[0, .6], [1, 1.6]], [[0, 0], [.2, .35], [1, 0]],
        [[0, [.62, .6, .58, 1]], [1, [.34, .33, .34, 1]]], (-1, 1))
    configs['crate_ignite'] = full(
        'flame_glow', 14, burst(14), (.25, .55), cone((0, -1), 140, 60, 210), (0, -220), 1.4, (.22, .48),
        [[0, .5], [.2, 1], [1, .1]], [[0, 0], [.08, 1], [.6, .7], [1, 0]], ember_ramp, (-3, 3))
    configs['crate_collapse'] = full(
        'char_splinter', 20, burst(20), (.55, 1.05), cone((0, -1), 220, 140, 420), (0, 620), .7, (.26, .42),
        [[0, 1], [1, .85]], [[0, 1], [.75, .9], [1, 0]], [[0, [1, 1, 1, 1]], [1, [.55, .5, .48, 1]]], (-12, 12))
    configs['collapse_embers'] = full(
        'spark', 26, burst(26), (.5, 1.2), cone((0, -1), 150, 60, 280), (0, -60), 1.1, (.06, .16),
        [[0, 1], [1, .3]], [[0, 1], [.7, .85], [1, 0]], ember_ramp, (-6, 6))
    configs['keg_staves'] = full(
        'keg_stave', 10, burst(10), (.5, 1.0), dict(kind='radial', speed=dict(min=260, max=720)), (0, 700), .9,
        (.2, .32), [[0, 1], [1, .9]], [[0, 1], [.8, .9], [1, 0]], [[0, [1, 1, 1, 1]], [1, [.6, .55, .52, 1]]],
        (-14, 14))
    # 01 (h): the title's stars, twinkling over its night sky, and a shooting
    # star's streak. 01 (i): three sizes, the small ones cool and the large
    # ones warm; each star alone at its point, one twinkle at a time over a
    # slow lifetime of its own, so no two keep time.
    for size, scale, life, color in (
        ('small', (.45, .75), (5.6, 10.8), [[0, [.84, .9, 1, 1]], [1, [.72, .8, 1, 1]]]),
        ('medium', (.9, 1.35), (6.4, 12.0), [[0, [1, 1, 1, 1]], [1, [.86, .82, 1, 1]]]),
        ('large', (1.7, 2.3), (8.0, 14.0), [[0, [1, .96, .88, 1]], [1, [.92, .88, 1, 1]]]),
    ):
        configs[f'title_twinkle_{size}'] = full(
            'title_star', 1, dict(kind='continuous', rate_hz=1.5), life, cone((0, -1), 360, 0, 1), (0, 0), 1.0,
            scale, [[0, .55], [.5, 1], [1, .55]], [[0, 0], [.4, 1], [.6, 1], [1, 0]], color)
    configs['title_meteor'] = full(
        'title_star', 140, dict(kind='continuous', rate_hz=220), (.25, .5), cone((0, -1), 360, 0, 18), (0, 0), 1.0,
        (.7, 1.3), [[0, 1], [1, .15]], [[0, 1], [1, 0]], [[0, [1, 1, 1, 1]], [1, [.7, .78, 1, 1]]])
    configs['rock_chips'] = full(
        'rock_chip', 8, burst(8), (.3, .6), cone((0, -1), 160, 90, 260), (0, 650), .5, (.14, .24),
        [[0, 1], [1, .8]], [[0, 1], [.7, .9], [1, 0]], [[0, [1, 1, 1, 1]], [1, [.7, .7, .7, 1]]], (-10, 10))
    # 01 (k): the lava's freeze and thaw, each burst over within a second so
    # a basin's bursts stay under the cap: frost mist off its surface as the
    # freeze's front comes down, cold glints as it starts, sparks from its
    # cracks and spatter as it melts.
    configs['lava_mist'] = full(
        'dust', 24, burst(3), (.6, 1.0), cone((0, -1), 80, 10, 35), (0, -25), 1.8, (.3, .45),
        [[0, .7], [.5, 1.3], [1, 1.9]], [[0, 0], [.25, .09], [.7, .06], [1, 0]],
        [[0, [.9, .96, 1, 1]], [1, [.75, .85, .95, 0]]])
    configs['lava_frost'] = full(
        'ice_crystal', 24, burst(4), (.5, .9), cone((0, -1), 70, 30, 90), (0, -20), 1.2, (.05, .1),
        [[0, 1], [1, .6]], [[0, 0], [.15, .9], [.7, .6], [1, 0]],
        [[0, [1, 1, 1, 1]], [1, [.8, .92, 1, 0]]], (-3, 3))
    configs['lava_sparks'] = full(
        'spark', 24, burst(7), (.25, .6), cone((0, -1), 50, 140, 330), (0, 900), .4, (.06, .13),
        [[0, 1], [1, .4]], [[0, 1], [.6, .9], [1, 0]],
        [[0, [1, 1, .86, 1]], [.25, [1, .86, .32, 1]], [.65, [1, .42, .1, 1]], [1, [.5, .1, .05, 0]]])
    configs['lava_spatter'] = full(
        'droplet', 32, burst(6), (.35, .7), cone((0, -1), 60, 160, 380), (0, 1500), .2, (.12, .22),
        [[0, 1], [1, .7]], [[0, 1], [.75, 1], [1, 0]],
        [[0, [1, .95, .6, 1]], [.3, [1, .62, .2, 1]], [.8, [.85, .22, .1, 1]], [1, [.4, .08, .06, 0]]])
    # e01tomes: each book's motes in its colour, at most 8 alive off its
    # centre: the pact's embers rising and cooling to ash, bounding's small
    # blue sparks rising, arcane's violet sparks drifting out and twinkling;
    # each take's sparks following it out; and the bound's sparks ringing
    # out from its ring and the motes it leaves as the wizard rises. Particles
    # draw under the book, so its motes leave it fast and then drift.
    twinkle = [[0, 0], [.12, 1], [.35, .45], [.55, 1], [.8, .6], [1, 0]]
    configs['tome_pact_motes'] = full(
        'mote', 8, dict(kind='continuous', rate_hz=6), (.9, 1.3), cone((0, -1), 160, 40, 75), (0, -30), 2.2,
        (.3, .45), [[0, .6], [.2, 1], [1, .5]], [[0, 0], [.1, 1], [.7, .8], [1, 0]],
        [[0, [1, .9, .55, 1]], [.35, [1, .52, .16, 1]], [.7, [.75, .3, .14, 1]], [1, [.4, .36, .36, 1]]])
    configs['tome_bounding_motes'] = full(
        'title_star', 8, dict(kind='continuous', rate_hz=6), (.8, 1.3), cone((0, -1), 130, 45, 80), (0, -40), 2.0,
        (.7, 1.1), [[0, .5], [.25, 1], [1, .3]], twinkle,
        [[0, [.88, .97, 1, 1]], [.5, [.52, .82, 1, 1]], [1, [.3, .62, 1, 0]]], (-2, 2))
    configs['tome_arcane_motes'] = full(
        'title_star', 8, dict(kind='continuous', rate_hz=5), (1.0, 1.6), dict(kind='radial', speed=dict(min=40, max=70)),
        (0, -12), 2.4, (.7, 1.1), [[0, .5], [.25, 1], [1, .3]], twinkle,
        [[0, [.97, .93, 1, 1]], [.4, [.76, .52, 1, 1]], [1, [.5, .26, .92, 0]]], (-3, 3))
    for kind, sprite, color in (
        ('pact', 'mote', [[0, [1, .96, .8, 1]], [.25, [1, .7, .28, 1]], [.6, [.95, .32, .1, 1]], [1, [.4, .16, .1, 0]]]),
        ('bounding', 'title_star', [[0, [1, 1, 1, 1]], [.3, [.68, .9, 1, 1]], [1, [.3, .62, 1, 0]]]),
        ('arcane', 'title_star', [[0, [1, 1, 1, 1]], [.3, [.86, .7, 1, 1]], [1, [.52, .26, .95, 0]]]),
    ):
        configs[f'tome_{kind}_take_sparks'] = full(
            sprite, 24, burst(24), (.45, .9), dict(kind='radial', speed=dict(min=70, max=240)), (0, -70), 2.0,
            (.9, 1.4) if sprite == 'title_star' else (.35, .55), [[0, 1], [.5, .8], [1, .2]],
            [[0, 1], [.7, .85], [1, 0]], color, (-4, 4))
    sky = [[0, [1, 1, 1, 1]], [.3, [.72, .92, 1, 1]], [1, [.32, .64, 1, 0]]]
    configs['bound_sparks'] = full(
        'title_star', 20, burst(20), (.25, .5), dict(kind='radial', speed=dict(min=160, max=340)), (0, 260), 2.6,
        (.7, 1.1), [[0, 1], [1, .3]], [[0, 1], [.6, .9], [1, 0]], sky, (-6, 6))
    configs['bound_trail'] = full(
        'mote', 12, burst(12), (.4, .75), dict(kind='radial', speed=dict(min=6, max=34)), (0, 40), 1.4,
        (.3, .45), [[0, 1], [1, .4]], [[0, 0], [.1, 1], [.6, .7], [1, 0]], sky)
    # e01foes: the beetle's paw dust, kicked up from its raking foot every
    # 0.15 s of its tell and drifting back, earth-brown, the clods its frames
    # throw; the goblin's dash, a scuff of dust off its heels skidding out
    # low along the ground; the snowball's burst, chips of packed snow flung
    # out and falling with a few flakes, white. Particles draw under the
    # sprites, so each launches clear of the body or the burst first.
    earth = [[0, [.62, .5, .38, 1]], [.6, [.5, .42, .34, 1]], [1, [.38, .33, .29, 0]]]
    configs['paw_dust'] = full(
        'dust', 12, burst(4), (.35, .6), cone((0, -1), 120, 50, 120), (0, 160), 2.2, (.18, .32),
        [[0, .6], [.3, 1], [1, 1.4]], [[0, 0], [.12, .85], [.6, .6], [1, 0]], earth, (-2, 2))
    configs['dash_dust'] = full(
        'dust', 10, burst(8), (.3, .55), cone((0, -1), 170, 60, 170), (0, 220), 3.0, (.2, .34),
        [[0, .5], [.3, 1], [1, 1.5]], [[0, 0], [.1, .8], [.55, .55], [1, 0]],
        [[0, [.7, .66, .6, 1]], [1, [.45, .43, .4, 0]]], (-2, 2))
    configs['snowball_burst'] = full(
        'ice_chip', 16, burst(16), (.35, .75), dict(kind='radial', speed=dict(min=110, max=260)), (0, 520), 1.4,
        (.12, .26), [[0, 1], [1, .6]], [[0, 1], [.7, .9], [1, 0]],
        [[0, [1, 1, 1, 1]], [.5, [.86, .94, 1, 1]], [1, [.7, .84, .96, 0]]], (-10, 10))
    # e01sfx (V27): a strike on the ground, its sparks bursting low and wide
    # along it over the white star, a few rising, white to violet; heavy
    # gravity flattens their hops and they fade as they come down.
    configs['ground_sparks'] = full(
        'title_star', 32, burst(32), (.18, .4), cone((0, -1), 176, 240, 700), (0, 2400), 1.4, (.6, 1.2),
        [[0, 1], [.6, .75], [1, .3]], [[0, 1], [.4, .95], [.75, .3], [1, 0]],
        [[0, [1, 1, 1, 1]], [.2, [.92, .86, 1, 1]], [.55, [.68, .46, 1, 1]], [1, [.42, .22, .88, 1]]], (-10, 10))
    # e01sfx (V33): a crate breaking, splinters of its fresh pale pine flung
    # up and out over the faceted chip, which turns their faces light to
    # dark as they spin, browning as they fall.
    configs['plank_splinters'] = full(
        'ice_chip', 20, burst(20), (.45, .9), cone((0, -1), 200, 160, 480), (0, 900), .8, (.09, .19),
        [[0, 1], [1, .85]], [[0, 1], [.75, .9], [1, 0]],
        [[0, [.93, .74, .47, 1]], [.6, [.86, .66, .4, 1]], [1, [.58, .42, .27, 1]]], (-14, 14))
    # e01sfx (V37): the sprays on stone, wood and whatever else is not alive,
    # each bit thrown up and out small and bright, then slowing and
    # swelling. The frost's chips white and fast over the soft glow, its
    # steam pale blue and faint, as subtle as the chill's frost; the fire's
    # sparks white-hot to orange over the puff, its soot a dark puff that
    # hangs on the stone and fades.
    configs['frost_wall_hit'] = full(
        'flame_glow', 14, burst(10), (.35, .7), cone((0, -1), 220, 90, 300), (0, -40), 5.0,
        (.12, .22), [[0, .4], [.25, 1], [1, 2.6]], [[0, 1], [.2, .75], [.6, .35], [1, 0]],
        [[0, [1, 1, 1, 1]], [.3, [.9, .96, 1, 1]], [1, [.74, .86, 1, 1]]], (-2, 2))
    configs['flame_wall_hit'] = full(
        'dust', 16, burst(11), (.55, .95), cone((0, -1), 220, 160, 520), (0, -50), 4.6,
        (.08, .15), [[0, .7], [.3, .9], [1, 3.6]], [[0, 1], [.3, .9], [.65, .42], [1, 0]],
        [[0, [1, 1, .85, 1]], [.22, [1, .72, .28, 1]], [.38, [.5, .28, .16, 1]], [.55, [.14, .12, .11, 1]],
         [1, [.09, .08, .08, 1]]], (-3, 3))
    # e01hollow's geyser: embers spat from its mouth while it sputters and
    # erupts, thrown high, arcing out and falling back as they cool; at an
    # eruption's start a burst of them flung up and out of the plume, fast
    # enough to clear it within a fifth of a second.
    geyser_ramp = [[0, [1, 1, .9, 1]], [.25, [1, .84, .36, 1]], [.6, [1, .45, .1, 1]],
                   [.85, [.6, .12, .05, 1]], [1, [.25, .05, .04, 0]]]
    configs['geyser_embers'] = dict(
        config('spark', 16, 18, 1.3, 330, .2, spread=50),
        initial_velocity=dict(kind='cone', direction=[0, -1], spread_deg=50, speed=dict(min=150, max=330)),
        lifetime=dict(min=.6, max=1.3), start_scale=dict(min=.07, max=.2), gravity=[0, 420], drag_per_sec=.5,
        angular_velocity=dict(min=-6, max=6), color_over_life=geyser_ramp,
        scale_over_life=[[0, .8], [.2, 1], [1, .3]], alpha_over_life=[[0, 0], [.05, 1], [.7, .9], [1, 0]])
    configs['geyser_burst'] = dict(
        config('spark', 24, 24, 1.1, 620, .24, spread=110, burst=True),
        initial_velocity=dict(kind='cone', direction=[0, -1], spread_deg=110, speed=dict(min=260, max=620)),
        lifetime=dict(min=.5, max=1.1), start_scale=dict(min=.08, max=.24), gravity=[0, 700], drag_per_sec=1.1,
        angular_velocity=dict(min=-9, max=9), color_over_life=geyser_ramp,
        scale_over_life=[[0, 1], [.5, .7], [1, .2]], alpha_over_life=[[0, 1], [.6, .95], [1, 0]])
    # e01flask: the flask's splash, drops flung out past the burst's ring to
    # the splash's reach and falling, bright first; poison's bubbles welling
    # off a body, launched fast enough to clear it, then drifting up as they
    # swell and pop; Corroded's sparse fizz, white glints darting off the
    # plates; its burst as it takes, drops of the molten melt spat up and
    # falling, white-hot green flashing violet as they cool.
    toxic = [[0, [.9, 1, .62, 1]], [.35, [.6, .9, .3, 1]], [1, [.3, .6, .12, 0]]]
    configs['flask_splash'] = full(
        'droplet', 28, burst(28), (.4, .75), dict(kind='radial', speed=dict(min=160, max=380)), (0, 760), 1.5,
        (.1, .2), [[0, 1], [.6, .9], [1, .6]], [[0, 1], [.7, .9], [1, 0]], toxic, (-6, 6))
    configs['poison_bubbles'] = full(
        'mote', 14, dict(kind='continuous', rate_hz=7), (.9, 1.4), cone((0, -1), 70, 70, 140), (0, -30), 2.0,
        (.14, .22), [[0, .55], [.7, 1], [.9, 1.25], [1, .4]], [[0, 0], [.1, .95], [.85, .8], [1, 0]],
        [[0, [.8, 1, .55, 1]], [.6, [.55, .88, .28, 1]], [1, [.4, .75, .2, 0]]])
    verdigris = [[0, [.78, .95, .86, 1]], [.5, [.48, .74, .62, 1]], [1, [.3, .5, .42, 0]]]
    configs['corrode_fizz'] = full(
        'ice_glint', 8, dict(kind='continuous', rate_hz=5), (.25, .5), dict(kind='radial', speed=dict(min=70, max=140)),
        (0, -40), 4.5, (.28, .45), [[0, .6], [.3, 1], [1, .3]], [[0, 0], [.1, 1], [.6, .8], [1, 0]], verdigris, (-4, 4))
    configs['corrode_burst'] = full(
        'droplet', 22, burst(22), (.5, .9), cone((0, -1), 210, 150, 360), (0, 1000), 1.1, (.1, .19),
        [[0, 1], [.5, .9], [1, .6]], [[0, 1], [.75, .95], [1, 0]],
        [[0, [.92, 1, .7, 1]], [.25, [.62, .92, .3, 1]], [.6, [.66, .5, 1, 1]], [1, [.3, .55, .15, 0]]], (-6, 6))
    return configs


SHADERS=('soft_glow',)
# Soft-glow uniforms: falloff exponent, hot-core boost, dither steps, peak alpha.
MATERIALS={'soft_halo':('soft_glow',[1.8,0.0,1.0,0.1]),'soft_flame':('soft_glow',[2.2,0.35,1.0,0.6])}


def mirror_facing(images,lit,animations):
    """Left-facing copies of the wizard's frames and clips, the fireball's, the
    fire missile's, the enemies' and the beetle's charge streaks: nothing flips
    a sprite in place, so the kit's animator, and 01's fireballs, missiles and
    enemies, show mirrored art by facing. Their maps derive from the mirrored
    silhouettes. After `spray_clips`, which makes the streaks' clip."""
    from PIL import ImageOps
    from actors import ENEMY_CLIPS
    from effects import FACING_CLIPS
    flying=('fireball','fire_missile')
    for clip in sorted(c for c in animations if c.startswith('player') or c in flying or c in ENEMY_CLIPS or c in FACING_CLIPS):
        frames=[]
        for frame in animations[clip]['frames']:
            name=frame['sprite'].removeprefix('ex10_');left=name+'_left'
            images[left]=ImageOps.mirror(images[name])
            if name in lit:lit.add(left)
            frames.append(dict(frame,sprite='ex10_'+left))
        animations[clip+'_left']=dict(animations[clip],frames=frames)


def spray_clips(animations,families=('effects','tomes')):
    """The sprays' clips (`effects.SPRAY_CLIPS`) and the force push's wave and
    dust (`effects.PUSH_CLIPS`), unmirrored: `toys` turns each to its aim and
    flips it top to bottom when the aim faces left, and mirrors the dust.
    And e01tomes' (`effects.TOME_CLIPS`): each book's idle loop and take,
    and the bound's ring, which nothing turns; e01foes' (`effects.FOE_CLIPS`):
    the snowball's, its burst's and the beetle's streaks, which
    `mirror_facing` then twins; e01flask's (`effects.FLASK_CLIPS`,
    `effects.CORRODE_CLIPS`): the flask's tumble and its burst, and
    Corrosive's reaction. The tomes' are the tomes family's, the rest the
    effects'; `families` names whose."""
    from effects import CORRODE_CLIPS, FLASK_CLIPS, FOE_CLIPS, PUSH_CLIPS, SPRAY_CLIPS, TOME_CLIPS
    tables={'effects':{**SPRAY_CLIPS,**PUSH_CLIPS,**FOE_CLIPS,**FLASK_CLIPS,**CORRODE_CLIPS},'tomes':TOME_CLIPS}
    for family in families:
        for clip,(_,frames,ms,looping) in tables[family].items():
            animations[clip]=dict(looping=looping,frames=[dict(sprite=f'ex10_{clip}_{f}',duration_ms=ms) for f in range(frames)])


def mirror_anchors(anchors,images):
    """Each left-facing wizard frame's staff light anchor: its right-facing
    twin's mirrored about the frame's middle, x 48, as its art is."""
    from actors import WIZARD
    for name,(x,y) in list(anchors.items()):
        if name+'_left' in images:anchors[name+'_left']=[round(WIZARD-x,3),y]


def check_ice_fits(images):
    """Each kind's fitted ice cube sits on the box its prefab's hurtbox makes,
    wherever that prefab exists: the dummy's, and each enemy's once its prefab
    lands, so a hurtbox the run tunes cannot leave its ice behind. Each cube's
    cracked twin, which a thaw flickers to, covers what the whole one does."""
    from art import STATUS_CLASSES
    from effects import HURTBOXES
    for kind,box in HURTBOXES.items():
        path=CONTENT/'prefabs'/f'{kind}.json'
        if not path.exists():continue
        hurtbox=json.loads(path.read_text())['components']['hurtbox']
        (ox,oy),shape=hurtbox.get('offset',[0,0]),hurtbox['shape']
        hx,hy=(shape['circle'],)*2 if 'circle' in shape else shape['aabb']
        made=(ox-hx,oy-hy,ox+hx,oy+hy)
        if made!=box:raise ValueError(f'status_ice_{kind}: its cube fits {box}; its prefab hurtbox makes {made}')
    for name in [*HURTBOXES,*STATUS_CLASSES]:
        whole,cracked=images[f'status_ice_{name}'],images[f'status_ice_{name}_cracked']
        if cracked.size!=whole.size or cracked.getbbox()!=whole.getbbox():
            raise ValueError(f'status_ice_{name}_cracked: covers {cracked.getbbox()}, not its cube\'s {whole.getbbox()}')


def check_frost_fits(images):
    """Each frost sprite keeps to its box, its kind's fitted box or its
    class's status box, grown by `effects.FROST_REACH` for its crust's
    spikes, as `check_ice_fits` holds the cubes to their hurtboxes."""
    from art import STATUS_CLASSES
    from effects import FROST_KINDS, FROST_REACH, FROST_STAGES, fitted_box, status_box
    boxes={**{kind:fitted_box(kind) for kind in FROST_KINDS},**{name:status_box(size) for name,size in STATUS_CLASSES.items()}}
    for name,(x0,y0,x1,y1) in boxes.items():
        for stage in range(1,FROST_STAGES+1):
            bbox=images[f'frost_{name}_{stage}'].getbbox()
            if bbox is None:raise ValueError(f'frost_{name}_{stage}: empty')
            left,top,right,bottom=bbox
            if left<x0-FROST_REACH or top<y0-FROST_REACH or right-1>x1+FROST_REACH or bottom-1>y1+FROST_REACH:
                raise ValueError(f'frost_{name}_{stage}: reaches {bbox}, past its box {(x0,y0,x1,y1)} by more than {FROST_REACH}')


def write_json(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_bytes(json_bytes(data))


def json_bytes(data):
    return (json.dumps(data,indent=2,sort_keys=True)+'\n').encode()


def png(im):
    out=io.BytesIO();im.save(out,format='PNG',compress_level=9,optimize=False);return out.getvalue()


class Made:
    """What one family makes: its sprites, lit names and clips, the
    wizard's anchors, the level's prepared source and what it read, the
    particle configs, then every file by path and its manifest entries by
    place (`entries_of`)."""

    def __init__(self):
        self.images,self.lit,self.animations,self.anchors={},set(),{},{}
        self.source,self.read,self.configs,self.files,self.entries=None,set(),{},{},{}


class Watched(dict):
    """Sprites by name that record the names read from and written to them."""

    def __init__(self,*args):
        super().__init__(*args);self.read,self.written=set(),set()

    def __getitem__(self,name):
        self.read.add(name);return super().__getitem__(name)

    def __setitem__(self,name,im):
        self.written.add(name);super().__setitem__(name,im)


def art(family):
    """Art family `family`'s builder: its drawing (`art.art_families`), then
    its sprays' clips (`spray_clips`), its left-facing twins and the staff
    light's anchors."""
    def build(made,sprites,lit):
        made.anchors=art_families()[family](made.images,made.lit,made.animations) or {}
        if family in ('effects','tomes'):spray_clips(made.animations,(family,))
        mirror_facing(made.images,made.lit,made.animations);mirror_anchors(made.anchors,made.images)
    return build


def level_family(made,sprites,lit):
    """The map, its tileset record, opaque tiles and prefabs, and the
    moving platforms' decks it draws from the deck tile, all from
    `sprites`, every family's."""
    watched=Watched(sprites);tiled,source,prefabs=build_level(watched,lit)
    made.images={name:dict.__getitem__(watched,name) for name in watched.written}
    made.lit={name for name in made.images if name in lit}
    made.files['assets/tilemaps/level.tmj']=tiled_text(tiled).encode()
    made.entries[('tilemaps','ex10_level')]=dict(path='tilemaps/level.tmj')
    # The map's tile IDs, read back by the next run, so `--check` fails on a
    # tileset change.
    made.files['tools/tileset.json']=json_bytes(source['tileset'])
    path='data/opaque_tiles/level.json'
    made.files['assets/'+path]=json_bytes(opaque_tiles(watched,tiled))
    made.entries[('data','opaque_tiles','level')]=dict(path=path)
    # The prefabs the map's object layers name, one per class.
    for name,prefab in sorted(prefabs.items()):
        path=f'prefabs/{name}.json';made.files['assets/'+path]=json_bytes(prefab)
        made.entries[('prefabs',name)]=dict(path=path)
    made.source=source
    made.read=watched.read|{s.removeprefix('ex10_') for s in strings(tiled,prefabs,source['props']) if s.startswith('ex10_')}


def strings(*data):
    """Every string inside `data`."""
    for item in data:
        if isinstance(item,str):yield item
        elif isinstance(item,dict):yield from strings(*item.values())
        elif isinstance(item,list):yield from strings(*item)


def content_family(made,sprites,lit):
    """The shell's fonts, theme and damage-flash material, copied from the
    template, 01's serif face, the content's prefabs and data, the glow
    shader and materials, the ambient emitter's prefab and the particle
    configs: sources copied or written from this file."""
    template=json.loads((TEMPLATE/'assets/manifest.json').read_text())
    shell={section:json.loads(json.dumps(template[section])) for section in SHELL_SECTIONS}
    shell['fonts']['serif_bold']=dict(path='fonts/SourceSerif4/SourceSerif4-Bold.ttf')
    shell['font_families']['serif']['faces'].append('serif_bold')
    shell['shaders']=dict(damage_flash=template['shaders']['damage_flash'])
    shell['materials']=dict(damage_flash=template['materials']['damage_flash'])
    made.entries.update(entries_of(shell))
    for path in SHELL_ASSETS:
        made.files['assets/'+path]=(TEMPLATE/'assets'/path).read_bytes()
    made.files['assets/'+THEME]=themed((TEMPLATE/'assets'/THEME).read_text()).encode()
    made.files['assets/fonts/SourceSerif4/SourceSerif4-Bold.ttf']=SERIF_BOLD.read_bytes()
    for item in sorted(CONTENT.rglob('*')):
        if not item.is_file():continue
        path=item.relative_to(CONTENT).as_posix()
        made.files['assets/'+path]=item.read_bytes()
        parts=path.removesuffix('.json').split('/')
        if parts[0]=='prefabs' and len(parts)==2:made.entries[('prefabs',parts[1])]=dict(path=path)
        elif parts[0]=='data' and len(parts)==3:made.entries[('data',parts[1],parts[2])]=dict(path=path)
        else:raise ValueError('content outside prefabs/ and data/<kind>/: '+path)
    # The soft-glow shader is authored in tools/shaders and owned like any output.
    for name in SHADERS:
        path=f'shaders/{name}.wgsl'
        made.files['assets/'+path]=(TOOLS/path).read_bytes()
        made.entries[('shaders','ex10_'+name)]=dict(path=path)
    for name,(shader,f32s) in MATERIALS.items():
        made.entries[('materials','ex10_'+name)]=dict(shader='ex10_'+shader,uniform_defaults=dict(
            vec4=[[0.0]*4 for _ in range(4)],f32s=f32s,i32s=[0]*4))
    # The class of the level's `emitters` objects: each object's properties
    # set its emitter's config and seed over this prefab's.
    made.files['assets/prefabs/ambient_emitter.json']=json_bytes(dict(components=dict(
        ambient_emitter=None,particle_emitter=dict(config='ex10_torch_embers'))))
    made.entries[('prefabs','ex10_ambient_emitter')]=dict(path='prefabs/ambient_emitter.json')
    made.configs=particle_configs()
    for name,data in sorted(made.configs.items()):
        path=f'particles/{name}.json';made.files['assets/'+path]=json_bytes(data)
        made.entries[('particles','ex10_'+name)]=dict(path=path)
    # The hand-authored sounds' registrations.
    made.entries.update(registered(lambda name:name not in SOUNDS))


def registered(chosen):
    """The `sounds.json` registrations of the files `chosen` picks, by
    place. Every registered sound must be synthesized (`sfx.SOUNDS`) or
    explicitly hand-authored."""
    entries={}
    for key,sound in json.loads((TOOLS/'sounds.json').read_text()).items():
        name=sound['path'].removeprefix('sounds/')
        if name not in SOUNDS and sound['path'] not in HAND_AUTHORED_SOUNDS:
            raise ValueError(f"sound {sound['path']} is neither synthesized nor hand-authored")
        if chosen(name):entries[('sounds',key)]=sound
    return entries


def wavs(music):
    """The music and the areas' ambience loops' builder when `music`
    (`sfx.MUSIC`), the sound effects' otherwise: their WAVs and
    registrations."""
    def build(made,sprites,lit):
        names={name for name in SOUNDS if (name in MUSIC)==music}
        for name,data in sorted(build_sounds(names).items()):
            if name in LOOPS and (problem:=seam_problem(name,data)):
                raise ValueError(f'loop {name}: {problem}')
            made.files['assets/sounds/'+name]=data
        made.entries.update(registered(names.__contains__))
    return build


@dataclass(frozen=True)
class Family:
    """A family's builder, `build(made, sprites, lit)`, and `uses`, the
    families whose code or art its outputs draw on: `--family` rebuilds a
    family with every family it uses. A source-driven family is never
    locked."""
    build:Callable
    uses:tuple=()
    lockable:bool=True


# Every output's family, in build order: the art's, whose sprites the level
# then reads, then the sources' and the sounds'. An output drawn from
# another family's art is that family's, as each summon's icon is its
# enemy's (`effects.ICON_FAMILIES`), and draws on the code of the families
# its family uses: the icons' tiles are the effects', and scenery and the
# backdrops draw with the terrain's masonry and rock. The level declares
# every family whose art it reads, which a full run checks. A change to
# shared code (`pixels.py`, `art.py`, `palette.json`, `actors.py`'s rig
# helpers, `terrain.RELIEF_PREFIXES`) needs a full run.
FAMILIES={
    'player':Family(art('player')),
    'projectiles':Family(art('projectiles'),uses=('effects',)),
    'balls':Family(art('balls')),
    'dummy':Family(art('dummy'),uses=('effects',)),
    **{enemy:Family(art(enemy),uses=('effects',)) for enemy in ('wisp','crawler','beetle','bat','troll','goblin')},
    'stones':Family(art('stones')),
    'scenery':Family(art('scenery'),uses=('terrain',)),
    'props':Family(art('props'),uses=('effects',)),
    'terrain':Family(art('terrain')),
    'backdrops':Family(art('backdrops'),uses=('terrain',)),
    'effects':Family(art('effects')),
    'tomes':Family(art('tomes'),uses=('effects',)),
    # The tiles and scenery, the pickups' orbs and the tomes' books.
    'level':Family(level_family,uses=('terrain','scenery','backdrops','effects','tomes'),lockable=False),
    'content':Family(content_family,lockable=False),
    'sounds':Family(wavs(False)),
    'music':Family(wavs(True)),
}


def users(names):
    """`names` and every family that uses any of them, through every use."""
    run=set(names)
    while grown:={name for name,family in FAMILIES.items() if name not in run and run.intersection(family.uses)}:
        run|=grown
    return run


def entries_of(manifest):
    """Each manifest entry by its place: `(section, key)`, `(section,
    kind, key)` under `data`, and a list, the fallback chain, whole as
    `(section,)`."""
    entries={}
    for section,value in manifest.items():
        if isinstance(value,list):entries[(section,)]=value
        elif section=='data':entries.update({(section,kind,key):entry for kind,group in value.items() for key,entry in group.items()})
        else:entries.update({(section,key):entry for key,entry in value.items()})
    return entries


def manifest_of(entries):
    manifest={section:{} for section in ['sprites','animations','tilemaps','particles','shaders','materials','prefabs','sounds']}
    for place,entry in entries.items():
        *path,key=place;group=manifest
        if not path:manifest[key]=entry;continue
        for part in path:group=group.setdefault(part,{})
        group[key]=entry
    return manifest


def kept(base,manifest):
    """The sprites, lit names, clips and particle configs `manifest` lists
    under `base`, by name, which a family run reads for the families it
    leaves out."""
    from PIL import Image
    sprites,lit,animations,configs={},set(),{},{}
    for key,entry in manifest.get('sprites',{}).items():
        name=key.removeprefix('ex10_')
        with Image.open(base/'assets'/entry['path']) as im:sprites[name]=im.copy()
        if 'normal_map' in entry:lit.add(name)
    for section,into in (('animations',animations),('particles',configs)):
        for key,entry in manifest.get(section,{}).items():
            into[key.removeprefix('ex10_')]=json.loads((base/'assets'/entry['path']).read_text())
    return sprites,lit,animations,configs


def sprite_files(made):
    """`made`'s sprites, the lit ones' normal and emissive maps, its clips
    and the wizard's anchors as files, with their manifest entries."""
    for name,im in sorted(made.images.items()):
        path=f'sprites/{name}.png'
        made.files['assets/'+path]=png(im)
        # Glow gradients are sampled smoothly; everything else stays crisp pixel art.
        entry=dict(path=path,filter='linear' if name in LINEAR_FILTER else 'nearest')
        if name in made.lit:
            normal,emissive=auxiliary(im,relief(name))
            for key,suffix,img in [('normal_map','n',normal),('emissive_mask','e',emissive)]:
                path=f'sprites/{name}_{suffix}.png';entry[key]=path
                made.files['assets/'+path]=png(img)
        made.entries[('sprites','ex10_'+name)]=entry
    for name,data in sorted(made.animations.items()):
        path=f'animations/{name}.json';made.files['assets/'+path]=json_bytes(data)
        made.entries[('animations','ex10_'+name)]=dict(path=path)
    if made.anchors:
        # Where each wizard frame's staff orb sits, which the staff light's glow follows.
        path='data/lantern_anchors/player.json'
        made.files['assets/'+path]=json_bytes({'ex10_'+name:anchor for name,anchor in sorted(made.anchors.items())})
        made.entries[('data','lantern_anchors','player')]=dict(path=path)


def check_level_uses(made):
    """The level's art comes only from the families it declares it uses."""
    owners={name:family for family,m in made.items() for records in (m.images,m.animations) for name in records}
    read={owners[name] for name in made['level'].read if name in owners}-{'level'}
    if undeclared:=sorted(read-set(FAMILIES['level'].uses)):
        raise ValueError(f"the level reads the art of {', '.join(undeclared)}: add it to FAMILIES['level'].uses")


def generate(dest,families=None,base=ROOT):
    """Builds `families`, every family when None, into `dest` and returns
    each built family's owned paths. A family run writes its families'
    files, and `assets/manifest.json` and `tools/outputs.json` with the
    entries and paths of the families it leaves out kept from `base`,
    whose sprites, clips and configs its checks and the level read. It
    removes nothing: an output a family stops making stays listed until a
    full run, and `--check` fails on it until then."""
    if pillow_version!='12.3.0':raise RuntimeError('Use tools/requirements.txt: Pillow==12.3.0')
    if families is not None and (unknown:=set(families)-set(FAMILIES)):raise ValueError(f'no family {sorted(unknown)}')
    run=[name for name in FAMILIES if families is None or name in families]
    manifest=previous={} if families is None else None
    if families is not None:
        manifest=json.loads((base/'assets/manifest.json').read_text())
        previous=json.loads((base/'tools/outputs.json').read_text())
    sprites,lit,animations,configs=kept(base,manifest)
    made={}
    for name in run:
        made[name]=m=Made();FAMILIES[name].build(m,sprites,lit)
        sprites.update(m.images);lit.difference_update(m.images);lit.update(m.lit)
        animations.update(m.animations);configs.update(m.configs)
    validate_sets(sprites);check_ice_fits(sprites);check_frost_fits(sprites)
    entries=entries_of(manifest);fresh={}
    for name,m in made.items():
        validate_sprites(m.images,m.lit);sprite_files(m)
        for path in m.files:
            if path in fresh:raise ValueError(f'{path}: made by {fresh[path]} and {name}')
            fresh[path]=name
        entries.update(m.entries)
    if families is None:check_level_uses(made)
    manifest=manifest_of(entries)
    validate_references(manifest,animations,configs,made['level'].source if 'level' in made else None)
    for m in made.values():
        for path,data in m.files.items():
            out=dest/path;out.parent.mkdir(parents=True,exist_ok=True);out.write_bytes(data)
    write_json(dest/'assets/manifest.json',manifest)
    write_json(dest/'tools/outputs.json',sorted({*previous,*fresh,'assets/manifest.json','tools/outputs.json'}))
    return {name:sorted(m.files) for name,m in made.items()}


def read_locks():
    """`tools/locks.json`: each locked family's date and its files' SHA-256s."""
    locks=json.loads(LOCKS.read_text()) if LOCKS.exists() else {}
    if unknown:=sorted(set(locks)-set(FAMILIES)):raise SystemExit(f'{LOCKS.name}: no family {", ".join(unknown)}')
    return locks


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def lock_breaks(locks,files,verb='would change'):
    """A line for each locked family whose files, `{family: {path: SHA-256}}`,
    are not its lock's: a file changed, gained or lost."""
    breaks=[]
    for name,hashes in files.items():
        if name not in locks:continue
        held=locks[name]['files']
        if changed:=sorted(p for p in hashes.keys()|held.keys() if hashes.get(p)!=held.get(p)):
            more=f' and {len(changed)-12} more' if len(changed)>12 else ''
            breaks.append(f"{name} is locked ({locks[name]['date']}) and {verb} {', '.join(changed[:12])}{more}; "
                          f"unlocking it is the owner's call (--unlock {name})")
    return breaks


def hashed(root,paths):
    """`{family: {path: SHA-256}}` of `paths`' files under `root`, a file
    that is missing left out."""
    return {name:{p:sha256((root/p).read_bytes()) for p in files if (root/p).is_file()} for name,files in paths.items()}


def lock(names,unlock):
    """`--lock` and `--unlock`: record each named family's files' hashes,
    once the tree holds what the family makes, or drop its lock."""
    locks=read_locks()
    for name in unlock:
        if locks.pop(name,None) is None:raise SystemExit(f'{name} is not locked')
    if refused:=[name for name in names if not FAMILIES[name].lockable]:
        raise SystemExit(f'{", ".join(refused)}: source-driven, never locked')
    if names:
        with tempfile.TemporaryDirectory(prefix='platformer-lock-') as temp:
            dest=Path(temp);paths=generate(dest,set(names))
            for name in names:
                if stale:=[p for p in paths[name] if not (ROOT/p).is_file() or (ROOT/p).read_bytes()!=(dest/p).read_bytes()]:
                    raise SystemExit(f'{name}: {len(stale)} files differ from what it makes ({", ".join(stale[:5])}); '
                                     f'generate it first')
                locks[name]=dict(date=date.today().isoformat(),files=hashed(dest,{name:paths[name]})[name])
    write_json(LOCKS,locks)
    print(f'Locked {", ".join(names) or "none"}; unlocked {", ".join(unlock) or "none"}')


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
    # The level's own, when it is built.
    if source is None:return
    for p in source['props']:
        if p['animation'] and p['animation'] not in manifest['animations']:raise ValueError('unknown prop animation')
    if sum(configs[p['config'].removeprefix('ex10_')]['max_alive'] for p in source['emitters'])>256:raise ValueError('ambient cap exceeds 256')
    if len({p['seed'] for p in source['emitters']})!=len(source['emitters']):raise ValueError('duplicate emitter seed')


def floating_report():
    """`--floating`: the level built in memory, every piece that floats
    (`placement.floating`) and a total; exit 1 on any."""
    images,lit,animations,anchors=build_art();spray_clips(animations);mirror_facing(images,lit,animations)
    tiled,source,_=build_level(images,lit,check=False)
    lines=floating(source,tiled,images)
    for line in lines:print(line)
    print(f'floating: {len(lines)} pieces' if lines else 'no floating scenery')
    raise SystemExit(1 if lines else 0)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true')
    parser.add_argument('--floating',action='store_true',help='list the scenery that floats and exit 1 on any')
    parser.add_argument('--family',action='append',choices=FAMILIES,metavar='NAME',
                        help='build only this family and every family that uses it (repeatable): '+', '.join(FAMILIES))
    parser.add_argument('--lock',action='append',choices=FAMILIES,metavar='NAME',
                        help="record this family's files' hashes in locks.json, once the tree holds what it makes (repeatable)")
    parser.add_argument('--unlock',action='append',choices=FAMILIES,metavar='NAME',
                        help="drop this family's lock, the owner's call (repeatable)")
    args=parser.parse_args()
    if args.floating:floating_report()
    if args.lock or args.unlock:return lock(args.lock or [],args.unlock or [])
    if args.check and args.family:parser.error('--check is whole: drop --family')
    run=None if args.family is None else users(args.family)
    locks=read_locks()
    with tempfile.TemporaryDirectory(prefix='platformer-generate-') as temp:
        dest=Path(temp);paths=generate(dest,run)
        # Before anything is written: no locked family's files may change.
        breaks=lock_breaks(locks,hashed(dest,paths))
        owned=json.loads((dest/'tools/outputs.json').read_text())
        previous=json.loads((TOOLS/'outputs.json').read_text()) if (TOOLS/'outputs.json').exists() else json.loads((TOOLS/'legacy_outputs.json').read_text())
        obsolete=sorted(set(previous)-set(owned))
        if args.check:
            changed=[p for p in owned if not (ROOT/p).exists() or (ROOT/p).read_bytes()!=(dest/p).read_bytes()]
            changed += [p for p in obsolete if (ROOT/p).exists()]
            # No uncovered stray PNGs or configs may silently evade the inventory.
            expected=set(owned)|{'assets/'+path for path in HAND_AUTHORED_SOUNDS}
            changed += [str(p.relative_to(ROOT)) for p in (ROOT/'assets').rglob('*') if p.is_file() and str(p.relative_to(ROOT)) not in expected]
            # And every locked file in the tree holds its hash.
            breaks+=lock_breaks(locks,hashed(ROOT,{name:list(held['files']) for name,held in locks.items()}),'changed in the tree')
            if changed or breaks:
                raise SystemExit('\n'.join([*(['Generated outputs differ: '+', '.join(sorted(set(changed)))] if changed else []),*breaks]))
            print(f'OK: {len(owned)} owned outputs match and {len(locks)} locked families hold; art, level, references and budgets validated')
        else:
            if breaks:raise SystemExit('\n'.join(breaks))
            for path in obsolete:
                # Only the explicit previous inventory authorizes deletion.
                (ROOT/path).unlink(missing_ok=True)
            # A file whose bytes are unchanged stays untouched, so the hot-reload
            # watcher and `git status` see only real changes. A family run
            # writes its families' files and the two shared ones.
            mine=owned if run is None else [*(p for ps in paths.values() for p in ps),'assets/manifest.json','tools/outputs.json']
            written=0
            for path in mine:
                target,data=ROOT/path,(dest/path).read_bytes()
                if target.is_file() and target.read_bytes()==data:continue
                target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data);written+=1
            built='' if run is None else ' of '+', '.join(name for name in FAMILIES if name in run)
            print(f'Wrote {written} of {len(mine)} outputs{built}, the rest unchanged; removed {len(obsolete)} obsolete owned files')


if __name__=='__main__':main()
