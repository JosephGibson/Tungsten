"""Focused source/format failures and byte determinism; run with unittest."""
import copy
from pathlib import Path
import tempfile
import unittest

from art import build_art, read_grid, validate_art, validate_palette
from generate import generate
from level import build_level, validate_level


class AuthoringTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.images,cls.lit,cls.animations=build_art()
        cls.tiled,_,cls.source=build_level(cls.images)
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
        bad=dict(self.images);bad['player']=bad['player'].copy();bad['player'].putpixel((0,0),bad['player'].getpixel((32,32)))
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
        for change,error in [({'period':0},'motion'),({'position':[-2,10]},'outside'),({'position':[6.5,35.5]},'spawn')]:
            source=copy.deepcopy(self.source);source['hazards'][0].update(change)
            with self.assertRaisesRegex(ValueError,error):validate_level(source,self.tiled,self.ids)

    def test_walk_has_distinct_intermediates_and_fixed_foot_anchors(self):
        frames=self.animations['player_walk']['frames']
        self.assertEqual(len(frames),12)
        pixels=[self.images[f['sprite'].removeprefix('ex10_')].tobytes() for f in frames]
        self.assertEqual(len(set(pixels)),12)
        for frame in frames:
            self.assertEqual(self.images[frame['sprite'].removeprefix('ex10_')].getbbox()[3],61)

    def test_grounded_props_and_hanging_vegetation_meet_their_support(self):
        platforms={p['name']:p for p in self.source['platforms']}
        for p in self.source['props']:
            if 'support' not in p:continue
            support=platforms[p['support']];box=self.images[p['sprite'].removeprefix('ex10_')].getbbox()
            if p['anchor']=='ground':self.assertAlmostEqual(p['y']*64+box[3],support['row']*64,msg=str(p))
            else:self.assertAlmostEqual(p['y']*64+box[1],support['row']*64+self.images[support['style']].getbbox()[3],msg=str(p))
        self.assertEqual(sum(g['sprite']=='ex10_arch' for g in self.source['groups']),1)

    def test_shallow_collision_matches_alpha_and_never_uses_full_tile(self):
        layer=next(l['data'] for l in self.tiled['layers'] if l['name']=='collision')
        for x,y,width,height in self.source['slab_colliders']:
            self.assertEqual(height,23);self.assertEqual(width,64)
            self.assertEqual(layer[int(y/64)*self.source['cols']+int(x/64)],0)
        for p in self.source['platforms']:
            if p['style']!='ground':self.assertEqual(self.images[p['style']].getbbox(),(0,0,64,23))
        self.assertEqual(self.source['deck_depth'],23)

    def test_open_starting_courtyard_has_unobstructed_launch_space(self):
        clearing=next(p for p in self.source['platforms'] if p['name']=='clearing')
        self.assertEqual(clearing['right']-clearing['left'],16)
        terrain=next(l['data'] for l in self.tiled['layers'] if l['name']=='terrain')
        for y in range(31,36):
            for x in range(4,9):self.assertEqual(terrain[y*self.source['cols']+x],0)

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

    def test_pit_has_clear_interior_and_four_tile_shell(self):
        cols=self.source['cols']
        collision=next(l['data'] for l in self.tiled['layers'] if l['name']=='collision')
        for y in range(18,46):
            for x in range(132,180):self.assertEqual(collision[y*cols+x],0)
            for x in list(range(128,132))+list(range(180,184)):
                self.assertNotEqual(collision[y*cols+x],0)
        for y in range(46,50):
            for x in range(128,184):self.assertNotEqual(collision[y*cols+x],0)
        self.assertGreater(self.source['kill_row'],46)


if __name__=='__main__':unittest.main()
