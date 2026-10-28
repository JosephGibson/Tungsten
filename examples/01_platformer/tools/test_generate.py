"""Focused source/format failures and byte determinism; run with unittest."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
import wave

from art import STATUS_CLASSES, TOOLS, build_art, read_grid, validate_art, validate_palette
from generate import generate
from level import FAMILIES, build_level, validate_level
from placement import MOST, SINK, Placed, floating, hangs, stands, stream_kind


def tiles(v):
    """`v` as the floating check writes a place."""
    return ('%.4f' % v).rstrip('0').rstrip('.')


class AuthoringTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.images,cls.lit,cls.animations,_=build_art()
        cls.tiled,cls.source,_=build_level(cls.images,cls.lit)
        cls.ids={'ex10_'+n for n in cls.images}

    def test_palette_rejects_malformed_symbols_rgba_and_transparency(self):
        for palette in [{}, {'.': [0, 0, 0, 255]}, {'.': [0, 0, 0, 0], 'xx': [1, 2, 3, 255]}, {'.': [0, 0, 0, 0], 'x': [1, 2, 999, 255]}]:
            with self.assertRaises(ValueError):validate_palette(palette)

    def test_grid_dimensions_and_symbols(self):
        for bad in ['.\n'*64,('.'*64+'\n')*63,('?'*64+'\n')*64]:
            with self.assertRaises(ValueError):read_grid(bad)
        self.assertEqual(read_grid(('.'*64+'\n')*64).size,(64,64))

    def test_frame_palette_anchor_and_seams(self):
        validate_art(self.images,self.lit)
        bad=dict(self.images);bad['player']=bad['player'].copy();bad['player'].putpixel((0,0),bad['player'].getpixel((48,65)))
        with self.assertRaisesRegex(ValueError,'clipped'):validate_art(bad,self.lit)

    def test_waterfall_segments_tile_and_animation_wraps(self):
        from PIL import ImageChops
        for i in range(4):
            frame = self.images[f'waterfall_flow_{i}']
            self.assertEqual(frame.crop((0, 0, 64, 32)).tobytes(), frame.crop((0, 32, 64, 64)).tobytes())
            following = self.images[f'waterfall_flow_{(i + 1) % 4}']
            self.assertEqual(ImageChops.offset(frame, 0, 8).tobytes(), following.tobytes())

    def test_reject_invalid_gid_and_unknown_sprite(self):
        for gid in [-1,2**31,len(self.tiled['tilesets'][0]['tiles'])+1]:
            tiled=copy.deepcopy(self.tiled);tiled['layers'][0]['data'][0]=gid
            with self.assertRaisesRegex(ValueError,'GID'):validate_level(self.source,tiled,self.ids)
        tiled=copy.deepcopy(self.tiled);tiled['tilesets'][0]['tiles'][0]['properties'][0]['value']='missing'
        with self.assertRaisesRegex(ValueError,'reference'):validate_level(self.source,tiled,self.ids)

    def test_reject_layer_length_and_collision_disagreement(self):
        tiled=copy.deepcopy(self.tiled);tiled['layers'][0]['data'].pop()
        with self.assertRaisesRegex(ValueError,'length'):validate_level(self.source,tiled,self.ids)
        tiled=copy.deepcopy(self.tiled);tiled['layers'][-1]['data'][0]=1
        with self.assertRaisesRegex(ValueError,'disagreement'):validate_level(self.source,tiled,self.ids)

    def test_motion_rejects_invalid_period_bounds_and_unsafe_spawn(self):
        # The map places no hazard today; the data may, so the test brings one.
        fire=dict(fire=True,position=[30,25],travel=[1,0],period=5,phase=0)
        for change,error in [({'period':0},'motion'),({'position':[-2,10]},'outside'),({'position':[6.5,35.5]},'spawn')]:
            source=copy.deepcopy(self.source);source['hazards']=[dict(fire,**change)]
            with self.assertRaisesRegex(ValueError,error):validate_level(source,self.tiled,self.ids)

    def test_walk_has_distinct_intermediates_and_fixed_foot_anchors(self):
        frames=self.animations['player_walk']['frames']
        self.assertEqual(len(frames),12)
        pixels=[self.images[f['sprite'].removeprefix('ex10_')].tobytes() for f in frames]
        self.assertEqual(len(set(pixels)),12)
        for frame in frames:
            self.assertEqual(self.images[frame['sprite'].removeprefix('ex10_')].getbbox()[3],93)

    def test_grounded_props_and_hanging_vegetation_meet_their_support(self):
        # Each supported prop on its support's drawn edge, what the terrain
        # tiles draw, not the cell's edge (01 (k)).
        catalog={t['id']+1:t['properties'][0]['value'].removeprefix('ex10_') for t in self.tiled['tilesets'][0]['tiles']}
        terrain=next(l['data'] for l in self.tiled['layers'] if l['name']=='terrain')
        drawn=Placed(self.images,64);cols=self.source['cols']
        for i,g in enumerate(terrain):
            if g:drawn.add(catalog[g],i%cols*64,i//cols*64)
        for p in self.source['props']:
            if 'support' not in p:continue
            pts=drawn.points(p['sprite'].removeprefix('ex10_'),round(p['x']*64),round(p['y']*64))
            if p['anchor']=='ground':
                share,sink=stands(drawn.hit,pts);self.assertGreaterEqual(share,MOST,msg=str(p));self.assertLessEqual(sink,SINK,msg=str(p))
            else:
                self.assertEqual(p['anchor'],'under',msg=str(p));self.assertGreaterEqual(hangs(drawn.hit,pts),MOST,msg=str(p))
        self.assertEqual(sum(g['sprite']=='ex10_arch' for g in self.source['groups']),1)

    def test_floating_scenery_fails_the_build(self):
        source=json.loads((TOOLS/'level.json').read_text())
        gargoyle=next(p for p in source['props'] if p.get('anchor')=='wall')
        gargoyle['y']-=2
        with self.assertRaisesRegex(ValueError,'floating'):build_level(self.images,self.lit,source)

    def test_shallow_collision_matches_alpha_and_never_uses_full_tile(self):
        layer=next(l['data'] for l in self.tiled['layers'] if l['name']=='collision')
        for x,y,width,height,wood in self.source['slab_colliders']:
            self.assertEqual(height,23);self.assertEqual(width,64)
            self.assertEqual(layer[int(y/64)*self.source['cols']+int(x/64)],0)
        for p in self.source['platforms']:
            if p['style'] not in FAMILIES:self.assertEqual(self.images[p['style']].getbbox(),(0,0,64,23))
        self.assertEqual(self.source['deck_depth'],23)

    def test_open_starting_courtyard_has_unobstructed_launch_space(self):
        clearing=next(p for p in self.source['platforms'] if p['name']=='clearing')
        self.assertEqual(clearing['right']-clearing['left'],16)
        terrain=next(l['data'] for l in self.tiled['layers'] if l['name']=='terrain')
        for y in range(31,36):
            for x in range(4,9):self.assertEqual(terrain[y*self.source['cols']+x],0)
        # The start's wall (01 (k)): columns 0-1 rock from row 21, open air
        # over it.
        for x in range(2):
            for y in range(21):self.assertEqual(terrain[y*self.source['cols']+x],0)
            for y in range(21,36):self.assertNotEqual(terrain[y*self.source['cols']+x],0)

    def test_midnight_sky_and_moon_keep_clear_brightness_separation(self):
        sky=self.images['sky'];moon=self.images['moon']
        mean=sum(sum(px[:3])/3 for px in sky.get_flattened_data())/(sky.width*sky.height)
        self.assertLess(mean,25)
        self.assertGreater(moon.getpixel((32,32))[0],230)
        self.assertEqual(moon.getbbox(),(7,7,57,57))
        self.assertNotEqual(self.images['heart_full'].tobytes(),self.images['heart_empty'].tobytes())

    def test_all_owned_outputs_are_deterministic(self):
        with tempfile.TemporaryDirectory() as a,tempfile.TemporaryDirectory() as b:
            first=generate(Path(a));second=generate(Path(b));self.assertEqual(first,second)
            for path in first:self.assertEqual((Path(a)/path).read_bytes(),(Path(b)/path).read_bytes(),path)

    def test_small_ball_has_six_distinct_lit_frames(self):
        frames=self.animations['ball_small_spin']['frames']
        names=[f['sprite'].removeprefix('ex10_') for f in frames]
        self.assertEqual(len(set(self.images[n].tobytes() for n in names)),6)
        for name in names:
            self.assertIn(name,self.lit)
            self.assertNotEqual(self.images[name].tobytes(),self.images['ball'].tobytes())

    def lines_after(self, change):
        """The floating check's lines for a copy of the level that `change`
        edits in place."""
        source = copy.deepcopy(self.source)
        change(source)
        return floating(source, self.tiled, self.images)

    def judged_clean(self, anchor):
        """The first supported prop with `anchor` that the check passes."""
        found = {(line.split()[1], float(line.split()[2])) for line in floating(self.source, self.tiled, self.images)}
        for i, p in enumerate(self.source['props']):
            name = p['sprite'].removeprefix('ex10_')
            if p.get('anchor') == anchor and 'support' in p and (name, round(p['x'], 4)) not in found:
                return i, name
        self.fail('no clean ' + anchor + ' prop')

    def test_floating_lists_a_lifted_grounded_prop_as_stand(self):
        i, name = self.judged_clean('ground')
        def lift(source):
            source['props'][i]['y'] -= 8 / 64
        self.assertTrue(any(line.startswith(f'stand {name} ') for line in self.lines_after(lift)))

    def test_floating_lists_a_dropped_hung_prop_as_hang(self):
        i, name = self.judged_clean('under')
        def drop(source):
            source['props'][i]['y'] += 8 / 64
        self.assertTrue(any(line.startswith(f'hang {name} ') for line in self.lines_after(drop)))

    def pour(self, source, x=None):
        """The props of the pour stack at `x`, else of the first one of six
        segments or more that the check passes, top first."""
        stacks = {}
        for p in source['props']:
            if stream_kind((p['animation'] or p['sprite']).removeprefix('ex10_')) == 'pour':
                stacks.setdefault(p['x'], []).append(p)
        found = ' '.join(floating(self.source, self.tiled, self.images))
        for at, parts in sorted(stacks.items()):
            if at == x or (x is None and len(parts) >= 6 and f' {tiles(at)} ' not in found):
                return sorted(parts, key=lambda p: p['y'])
        self.fail('no such pour')

    def test_floating_lists_a_pour_short_of_its_surface(self):
        x = self.pour(self.source)[0]['x']
        def cut(source):
            source['props'].remove(self.pour(source, x)[-1])
        self.assertTrue(any(line.startswith(f'stream-short pour {tiles(x)} ') for line in self.lines_after(cut)))

    def test_floating_lists_a_gap_in_a_pour(self):
        x = self.pour(self.source)[0]['x']
        def cut(source):
            source['props'].remove(self.pour(source, x)[4])
        self.assertTrue(any(line.startswith(f'stream-gap pour {tiles(x)} ') for line in self.lines_after(cut)))

    def test_floating_lists_a_prop_with_no_anchor_and_no_class_as_unjudged(self):
        def add(source):
            source['props'].append(dict(sprite='ex10_lantern', x=60, y=34, depth='world', animation=None))
        self.assertIn('unjudged lantern 60 34 no anchor and in no list of scenery.py', self.lines_after(add))

    def test_floating_lists_a_vine_piece_inside_two_groups_boxes(self):
        k, group = next((k, g) for k, g in enumerate(self.source['groups']) if g['sprite'] == 'ex10_vine_0')
        def overlap(source):
            # The group's top piece half a tile right, into a second vine
            # group's box beside its own.
            piece = source['props'][group['pieces'][0]]
            piece['x'] += 0.5
            source['groups'].append(dict(group, x=group['x'] + 0.5, pieces=[]))
        lines = self.lines_after(overlap)
        self.assertIn(f"group vine_0_big_0_0 {group['x'] + 0.5:g}", ' '.join(lines))
        self.assertIn("inside 2 groups' boxes", ' '.join(lines))

    def test_floating_lists_a_parapet_moved_up_a_row(self):
        i, d = next((i, d) for i, d in enumerate(self.source['decorations']) if d['sprite'] == 'ex10_parapet_0')
        self.assertFalse(any(line.startswith('parapet ') for line in floating(self.source, self.tiled, self.images)))
        def lift(source):
            source['decorations'][i]['row'] -= 1
        self.assertTrue(any(line.startswith(f"parapet parapet_0 {d['col']} {d['row'] - 1} ") for line in self.lines_after(lift)))

    def test_floating_lists_an_unknown_decoration_sprite_as_unjudged(self):
        def add(source):
            source['decorations'].append(dict(col=60, row=30, layer='background', sprite='ex10_lantern'))
        self.assertIn('unjudged lantern 60 30 a decoration in no structure', self.lines_after(add))

    def test_pit_has_clear_interior_and_four_tile_shell(self):
        cols=self.source['cols'];pit=self.source['pit']
        collision=next(l['data'] for l in self.tiled['layers'] if l['name']=='collision')
        left,right,rim,floor=pit['left'],pit['right'],pit['rim'],pit['floor']
        for y in range(rim,floor):
            for x in range(left,right):self.assertEqual(collision[y*cols+x],0)
            for x in list(range(left-4,left))+list(range(right,right+4)):
                self.assertNotEqual(collision[y*cols+x],0)
        for y in range(floor,floor+4):
            for x in range(left-4,right+4):self.assertNotEqual(collision[y*cols+x],0)
        self.assertGreater(self.source['kill_row'],floor)


# e01foes' contract, the files its families own as the game reads them: each
# clip's frame size, durations and looping, then whether it has a left-facing
# twin and whether its frames stand on row 60; each particle config's cap;
# each sound's seconds. Its placeholders pass it, and so must its art.
ASSETS = TOOLS.parent / 'assets'
FOE_CLIPS = {
    'crawler_walk': ((64, 64), [90] * 6, True, True, True),
    'crawler_windup': ((64, 64), [100] * 4, False, True, True),
    'crawler_lunge': ((64, 64), [60, 80, 400], False, True, True),
    'crawler_recover': ((64, 64), [100, 120, 140], False, True, True),
    'crawler_scared': ((64, 64), [90, 70, 90, 70], True, True, True),
    'crawler_retreat': ((64, 64), [80] * 6, True, True, True),
    'crawler_hop': ((64, 64), [70, 90, 240], False, True, False),
    'crawler_snowball_windup': ((64, 64), [90] * 4, False, True, True),
    'crawler_snowball_throw': ((64, 64), [50, 80, 120], False, True, True),
    'snowball': ((16, 16), [60] * 4, True, False, False),
    'snowball_burst': ((48, 48), [50] * 6, False, False, False),
    'beetle_paw': ((64, 64), [100] * 8, False, True, True),
    'beetle_charge': ((64, 64), [60] * 4, True, True, True),
    'charge_streak': ((64, 24), [50] * 4, True, True, False),
    'goblin_dash_windup': ((64, 64), [120, 180], False, True, True),
    'goblin_dash': ((64, 64), [70] * 4, True, True, True),
    'goblin_panic': ((64, 64), [70] * 6, True, True, True),
}
FOE_PARTICLES = {'snowball_burst': 24, 'paw_dust': 12, 'dash_dust': 10}
FOE_SOUNDS = {
    'beetle_paw': (.75, .85), 'beetle_plough': (0, .35), 'crawler_retreat': (.4, .7),
    'crawler_hop': (0, .35), 'snowball_throw': (0, .35), 'snowball_burst': (0, .45),
    'goblin_crouch': (0, .35), 'goblin_dash': (0, .35), 'goblin_yelp': (.4, .8),
}


class FoeContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((ASSETS / 'manifest.json').read_text())

    def sprite(self, name):
        from PIL import Image
        with Image.open(ASSETS / self.manifest['sprites'][name]['path']) as im:
            return im.convert('RGBA')

    def clip(self, name):
        return json.loads((ASSETS / self.manifest['animations'][name]['path']).read_text())

    def test_every_clip_keeps_its_size_timing_twin_and_feet(self):
        from PIL import ImageOps
        for clip, (size, durations, looping, twinned, grounded) in FOE_CLIPS.items():
            data = self.clip('ex10_' + clip)
            self.assertEqual(data['looping'], looping, clip)
            self.assertEqual([f['duration_ms'] for f in data['frames']], durations, clip)
            sprites = [f['sprite'] for f in data['frames']]
            self.assertEqual(sprites, [f'ex10_{clip}_{i}' for i in range(len(durations))], clip)
            self.assertEqual('ex10_' + clip + '_left' in self.manifest['animations'], twinned, clip)
            if twinned:
                left = self.clip('ex10_' + clip + '_left')
                self.assertEqual([f['duration_ms'] for f in left['frames']], durations, clip)
                self.assertEqual([f['sprite'] for f in left['frames']], [s + '_left' for s in sprites], clip)
            for name in sprites:
                im = self.sprite(name)
                self.assertEqual(im.size, size, name)
                if twinned:
                    self.assertEqual(self.sprite(name + '_left').tobytes(), ImageOps.mirror(im).tobytes(), name)
                if grounded:
                    self.assertEqual(im.getbbox()[3] - 1, 60, f'{name}: its lowest opaque row')

    def test_the_crawler_walk_reaches_thirty_to_thirty_two_pixels_each_side(self):
        # Lightning meets a crawler within 32 px of its centre, x 32 on its
        # 64 px frame, which bounds the reach (`KindInfo::strike`).
        boxes = [self.sprite(f'ex10_crawler_walk_{i}').getbbox() for i in range(6)]
        self.assertGreaterEqual(32 - min(b[0] for b in boxes), 30)
        self.assertGreaterEqual(max(b[2] for b in boxes) - 32, 30)
        self.assertEqual(self.sprite('ex10_icon_crawler').size, (36, 36))

    def test_particles_keep_their_caps(self):
        for name, cap in FOE_PARTICLES.items():
            config = json.loads((ASSETS / self.manifest['particles']['ex10_' + name]['path']).read_text())
            self.assertLessEqual(config['max_alive'], cap, name)
            self.assertIn(config['sprite'], self.manifest['sprites'], name)

    def test_sounds_keep_their_lengths_and_volumes(self):
        for name, (shortest, longest) in FOE_SOUNDS.items():
            sound = self.manifest['sounds'][f'ex10_{name}_sfx']
            self.assertEqual(sound['path'], f'sounds/{name}.wav')
            self.assertFalse(sound['looping'], name)
            self.assertTrue(.4 <= sound['volume'] <= .75, name)
            with wave.open(str(ASSETS / sound['path']), 'rb') as wav:
                seconds = wav.getnframes() / wav.getframerate()
            self.assertTrue(shortest <= seconds <= longest, f'{name}: {seconds} s')


# e01sfx's contract, the files its families own as the
# game reads them: each sprite's size, the shock overlay's four frames a
# class, each particle config's cap, and each sound's shortest and longest
# seconds, its quietest and loudest volume and whether it loops. Its
# placeholders pass it, and so must its art.
SFX_SPRITES = {
    **{f'status_crackle_{name}_{f}': (size, size) for name, size in STATUS_CLASSES.items() for f in range(4)},
    'scorch': (48, 16),
    **{f'plank_{i}': (32, 16) for i in range(5)},
    'hurt_edge': (320, 180),
}
SFX_PARTICLES = {'shock_sparks': 24, 'ground_sparks': 32, 'plank_splinters': 20,
                 'frost_wall_hit': 18, 'flame_wall_hit': 18}
SFX_SOUNDS = {
    'ground_strike': (.3, 1.0, .35, .8, False),
    'player_tick': (0, .2, .2, .4, False),
    'crate_break': (.25, .9, .35, .8, False),
    'thunder': (1.6, 3.6, .35, .8, False),
    'fire_crackle': (2.0, 2.0, .35, .8, True),
}


class EffectsContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((ASSETS / 'manifest.json').read_text())

    def test_sprites_keep_their_sizes_and_the_overlay_four_frames(self):
        from PIL import Image
        for name, size in SFX_SPRITES.items():
            with Image.open(ASSETS / self.manifest['sprites']['ex10_' + name]['path']) as im:
                self.assertEqual(im.size, size, name)
        for name in STATUS_CLASSES:
            self.assertNotIn(f'ex10_status_crackle_{name}_4', self.manifest['sprites'], name)

    def test_particles_keep_their_caps(self):
        for name, cap in SFX_PARTICLES.items():
            config = json.loads((ASSETS / self.manifest['particles']['ex10_' + name]['path']).read_text())
            self.assertLessEqual(config['max_alive'], cap, name)
            self.assertIn(config['sprite'], self.manifest['sprites'], name)

    def test_sounds_keep_their_lengths_volumes_and_loops(self):
        for name, (shortest, longest, quietest, loudest, looping) in SFX_SOUNDS.items():
            sound = self.manifest['sounds'][f'ex10_{name}_sfx']
            self.assertEqual(sound['path'], f'sounds/{name}.wav')
            self.assertEqual(sound['looping'], looping, name)
            self.assertTrue(quietest <= sound['volume'] <= loudest, name)
            with wave.open(str(ASSETS / sound['path']), 'rb') as wav:
                seconds = wav.getnframes() / wav.getframerate()
            self.assertTrue(shortest <= seconds <= longest, f'{name}: {seconds} s')


# e01hollow's contract, the files its families own as the game reads them:
# each sprite's size and how many a family has, each stone's lowest opaque
# row on its body box's bottom, each particle config's cap, and each sound's
# shortest and longest seconds and its quietest and loudest volume. Its
# placeholders pass it, and so must its art.
HOLLOW_FAMILIES = {
    'geyser_vent': ((64, 32), 1),
    'geyser_vent_choked': ((64, 32), 1),
    'geyser_sputter_': ((64, 64), 4),
    'geyser_plume_': ((64, 176), 6),
    'stone_small_': ((32, 32), 2),
    'stone_medium_': ((32, 32), 2),
    'stone_large_': ((64, 64), 2),
}
HOLLOW_STONE_FEET = {'small': 31, 'medium': 31, 'large': 63}
HOLLOW_PARTICLES = {'geyser_embers': 16, 'geyser_burst': 24}
HOLLOW_SOUNDS = {
    'geyser_sputter': (.4, .7, .3, .7),
    'geyser_erupt': (.6, 1.0, .3, .7),
    'stone_knock': (.08, .4, .3, .7),
}


class HollowContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((ASSETS / 'manifest.json').read_text())

    def sprite(self, name):
        from PIL import Image
        with Image.open(ASSETS / self.manifest['sprites']['ex10_' + name]['path']) as im:
            return im.convert('RGBA')

    def test_sprites_keep_their_sizes_counts_and_the_stones_feet(self):
        for family, (size, count) in HOLLOW_FAMILIES.items():
            names = [family] if count == 1 else [f'{family}{i}' for i in range(count)]
            for name in names:
                self.assertEqual(self.sprite(name).size, size, name)
            if count > 1:
                self.assertNotIn(f'ex10_{family}{count}', self.manifest['sprites'], family)
        for size, feet in HOLLOW_STONE_FEET.items():
            for i in range(2):
                self.assertEqual(self.sprite(f'stone_{size}_{i}').getbbox()[3] - 1, feet, f'stone_{size}_{i}')

    def test_particles_keep_their_caps(self):
        for name, cap in HOLLOW_PARTICLES.items():
            config = json.loads((ASSETS / self.manifest['particles']['ex10_' + name]['path']).read_text())
            self.assertLessEqual(config['max_alive'], cap, name)
            self.assertIn(config['sprite'], self.manifest['sprites'], name)

    def test_sounds_keep_their_lengths_and_volumes(self):
        for name, (shortest, longest, quietest, loudest) in HOLLOW_SOUNDS.items():
            sound = self.manifest['sounds'][f'ex10_{name}_sfx']
            self.assertEqual(sound['path'], f'sounds/{name}.wav')
            self.assertFalse(sound['looping'], name)
            self.assertTrue(quietest <= sound['volume'] <= loudest, name)
            with wave.open(str(ASSETS / sound['path']), 'rb') as wav:
                seconds = wav.getnframes() / wav.getframerate()
            self.assertTrue(shortest <= seconds <= longest, f'{name}: {seconds} s')


# e01flask's contract, the files its families own as the game reads them:
# each sprite family's size and count, each clip's durations and looping,
# the armoured troll's clips frame for frame the troll's, 128 px with their
# feet on row 124 and twinned, each particle config's cap, and each sound's
# longest seconds and its quietest and loudest volume. Its placeholders pass
# it, and so must its art.
FLASK_FAMILIES = {
    'toxic_flask_': ((16, 16), 4),
    'toxic_flask_burst_': ((96, 96), 6),
    **{f'status_poison_{name}_': ((size, size), 4) for name, size in STATUS_CLASSES.items()},
    'icon_toxic_flask': ((36, 36), 1),
    'icon_troll_armoured': ((36, 36), 1),
}
FLASK_CLIPS = {'toxic_flask': ([60] * 4, True), 'toxic_flask_burst': ([50] * 6, False)}
TROLL_CLIPS = ('idle', 'walk', 'windup', 'slam', 'pickup', 'throw', 'swipe_windup', 'swipe')
FLASK_PARTICLES = {'flask_splash': 32, 'poison_bubbles': 16, 'corrode_fizz': 10, 'corrode_burst': 24}
FLASK_SOUNDS = {'flask_throw': .4, 'flask_burst': .8, 'poisoned': .6, 'corroded': 1.0}


class FlaskContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads((ASSETS / 'manifest.json').read_text())

    def sprite(self, name):
        from PIL import Image
        with Image.open(ASSETS / self.manifest['sprites']['ex10_' + name]['path']) as im:
            return im.convert('RGBA')

    def clip(self, name):
        return json.loads((ASSETS / self.manifest['animations']['ex10_' + name]['path']).read_text())

    def test_sprites_keep_their_sizes_and_counts(self):
        for family, (size, count) in FLASK_FAMILIES.items():
            names = [family] if count == 1 else [f'{family}{i}' for i in range(count)]
            for name in names:
                self.assertEqual(self.sprite(name).size, size, name)
            if count > 1:
                self.assertNotIn(f'ex10_{family}{count}', self.manifest['sprites'], family)

    def test_the_flask_clips_keep_their_timing(self):
        for clip, (durations, looping) in FLASK_CLIPS.items():
            data = self.clip(clip)
            self.assertEqual(data['looping'], looping, clip)
            self.assertEqual([f['duration_ms'] for f in data['frames']], durations, clip)
            self.assertEqual([f['sprite'] for f in data['frames']],
                             [f'ex10_{clip}_{i}' for i in range(len(durations))], clip)
            self.assertNotIn(f'ex10_{clip}_left', self.manifest['animations'], clip)

    def test_the_armoured_troll_clips_match_the_trolls(self):
        from PIL import ImageOps
        for clip in TROLL_CLIPS:
            plain, armoured = self.clip(f'troll_{clip}'), self.clip(f'troll_armoured_{clip}')
            self.assertEqual(armoured['looping'], plain['looping'], clip)
            self.assertEqual([f['duration_ms'] for f in armoured['frames']],
                             [f['duration_ms'] for f in plain['frames']], clip)
            left = self.clip(f'troll_armoured_{clip}_left')
            for i, frame in enumerate(armoured['frames']):
                name = f'troll_armoured_{clip}_{i}'
                self.assertEqual(frame['sprite'], 'ex10_' + name)
                self.assertEqual(left['frames'][i]['sprite'], f'ex10_{name}_left')
                im = self.sprite(name)
                self.assertEqual(im.size, (128, 128), name)
                self.assertEqual(im.getbbox()[3] - 1, 124, f'{name}: its lowest opaque row')
                self.assertEqual(self.sprite(name + '_left').tobytes(), ImageOps.mirror(im).tobytes(), name)

    def test_particles_keep_their_caps(self):
        for name, cap in FLASK_PARTICLES.items():
            config = json.loads((ASSETS / self.manifest['particles']['ex10_' + name]['path']).read_text())
            self.assertLessEqual(config['max_alive'], cap, name)
            self.assertIn(config['sprite'], self.manifest['sprites'], name)

    def test_sounds_keep_their_lengths_and_volumes(self):
        for name, longest in FLASK_SOUNDS.items():
            sound = self.manifest['sounds'][f'ex10_{name}_sfx']
            self.assertEqual(sound['path'], f'sounds/{name}.wav')
            self.assertFalse(sound['looping'], name)
            self.assertTrue(.3 <= sound['volume'] <= .8, name)
            with wave.open(str(ASSETS / sound['path']), 'rb') as wav:
                seconds = wav.getnframes() / wav.getframerate()
            self.assertTrue(0 < seconds <= longest, f'{name}: {seconds} s')


if __name__=='__main__':unittest.main()
