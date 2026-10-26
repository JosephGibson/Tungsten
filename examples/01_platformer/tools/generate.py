#!/usr/bin/env python3
"""Deterministic offline authoring; never invoked by Cargo or the engine."""
import argparse
import json
from pathlib import Path
import re
import tempfile

from PIL import __version__ as pillow_version
from art import TOOLS, auxiliary, build_art, relief, validate_art
from effects import LINEAR_FILTER
from level import build_level
from placement import floating
from sfx import HAND_AUTHORED_SOUNDS, LOOPS, build_sounds, seam_problem

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
    configs['shock_sparks'] = dict(
        config('spark', 12, 14, .25, 260, .1, spread=360),
        initial_velocity=dict(kind='radial', speed=dict(min=80, max=260)),
        lifetime=dict(min=.1, max=.25), gravity=[0, 0], drag_per_sec=3,
        angular_velocity=dict(min=-12, max=12), alpha_over_life=[[0, 1], [.6, .9], [1, 0]],
        color_over_life=[[0, [1, .95, 1, 1]], [.3, [.75, .55, 1, 1]], [1, [.45, .25, .85, 0]]])
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
    return configs


SHADERS=('soft_glow',)
# Soft-glow uniforms: falloff exponent, hot-core boost, dither steps, peak alpha.
MATERIALS={'soft_halo':('soft_glow',[1.8,0.0,1.0,0.1]),'soft_flame':('soft_glow',[2.2,0.35,1.0,0.6])}


def mirror_facing(images,lit,animations):
    """Left-facing copies of the wizard's frames and clips, the fireball's, the
    fire missile's and the enemies': nothing flips a sprite in place, so the kit's
    animator, and 01's fireballs, missiles and enemies, show mirrored art by
    facing. Their maps derive from the mirrored silhouettes."""
    from PIL import ImageOps
    from actors import ENEMY_CLIPS
    flying=('fireball','fire_missile')
    for clip in sorted(c for c in animations if c.startswith('player') or c in flying or c in ENEMY_CLIPS):
        frames=[]
        for frame in animations[clip]['frames']:
            name=frame['sprite'].removeprefix('ex10_');left=name+'_left'
            images[left]=ImageOps.mirror(images[name])
            if name in lit:lit.add(left)
            frames.append(dict(frame,sprite='ex10_'+left))
        animations[clip+'_left']=dict(animations[clip],frames=frames)


def spray_clips(animations):
    """The sprays' clips (`effects.SPRAY_CLIPS`) and the force push's wave and
    dust (`effects.PUSH_CLIPS`), unmirrored: `toys` turns each to its aim and
    flips it top to bottom when the aim faces left, and mirrors the dust."""
    from effects import PUSH_CLIPS, SPRAY_CLIPS
    for clip,(_,frames,ms,looping) in {**SPRAY_CLIPS,**PUSH_CLIPS}.items():
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
    path.write_text(json.dumps(data,indent=2,sort_keys=True)+'\n')


def generate(dest):
    if pillow_version!='12.3.0':raise RuntimeError('Use tools/requirements.txt: Pillow==12.3.0')
    images,lit,animations,anchors=build_art();mirror_facing(images,lit,animations);mirror_anchors(anchors,images)
    spray_clips(animations)
    tiled,source,level_prefabs=build_level(images,lit);validate_art(images,lit);check_ice_fits(images);check_frost_fits(images)
    configs=particle_configs()
    manifest={section:{} for section in ['sprites','animations','tilemaps','particles','shaders','materials','prefabs']}
    manifest['sounds']=json.loads((TOOLS/'sounds.json').read_text())
    template=json.loads((TEMPLATE/'assets/manifest.json').read_text())
    for section in SHELL_SECTIONS:manifest[section]=json.loads(json.dumps(template[section]))
    manifest['fonts']['serif_bold']=dict(path='fonts/SourceSerif4/SourceSerif4-Bold.ttf')
    manifest['font_families']['serif']['faces'].append('serif_bold')
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
    output('assets/'+THEME).write_text(themed((TEMPLATE/'assets'/THEME).read_text()))
    output('assets/fonts/SourceSerif4/SourceSerif4-Bold.ttf').write_bytes(SERIF_BOLD.read_bytes())
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
    # The sprites every pixel of which is opaque: a backdrop quad the map's
    # tiles of them cover is never seen, so the level skips it (level/draw.rs).
    path='data/opaque_tiles/level.json'
    opaque=sorted('ex10_'+name for name,image in images.items()
        if image.mode!='RGBA' or image.getchannel('A').getextrema()==(255,255))
    write_json(output('assets/'+path),{'sprites':opaque})
    manifest.setdefault('data',{}).setdefault('opaque_tiles',{})['level']=dict(path=path)
    # Where each wizard frame's staff orb sits, which the staff light's glow follows.
    path='data/lantern_anchors/player.json'
    write_json(output('assets/'+path),{'ex10_'+name:anchor for name,anchor in sorted(anchors.items())})
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


def floating_report():
    """`--floating`: the level built in memory, every piece that floats
    (`placement.floating`) and a total; exit 1 on any."""
    images,lit,animations,anchors=build_art();mirror_facing(images,lit,animations);spray_clips(animations)
    tiled,source,_=build_level(images,lit,check=False)
    lines=floating(source,tiled,images)
    for line in lines:print(line)
    print(f'floating: {len(lines)} pieces' if lines else 'no floating scenery')
    raise SystemExit(1 if lines else 0)


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true')
    parser.add_argument('--floating',action='store_true',help='list the scenery that floats and exit 1 on any');args=parser.parse_args()
    if args.floating:floating_report()
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
