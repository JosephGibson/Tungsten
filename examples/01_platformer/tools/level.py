"""Hand-authored platforms -> the Tiled map: tile layers, and object layers whose
classes name the prefabs generated here for the level's props, slab colliders,
iron boxes, hazards, moving platforms and areas.

M76: a ground platform's `style` names its family (`ground` and `pit` masonry,
`wet`, `basalt`, `mine`, `channel`, `bridge_deck`); each cell takes its
family's tile by its open sides, an underside under open air, a border where
two families meet, the pit's lining around `pit`; the mine's faces are iron,
static boxes in their own physics layer; caves and `backs` fill the
background layer; `lava` basins become lava and a hazard each."""
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
# A lava basin's skin, this far into its surface row; the frost child's
# and the player's touch run from `LAVA_REACH` above it to the bed, so what
# stands on the surface overlaps them and the throw, away from the box's
# centre, is up and out (M79).
LAVA_TOP, LAVA_REACH = 6, 4
# Lava's bite: typed fire that starts the burn, a throw on every touch,
# its window's too (the kit's `push_always`), touched again soon after.
LAVA_DAMAGE, LAVA_KNOCKBACK, LAVA_INTERVAL = 16, 1500, 0.1
# The surface the player stands on, never sinking in: a box this deep from
# the skin, in its own physics layer, which only the player's controller
# meets; its mask meets nothing, so balls, blocks and enemies fall in.
LAVA_SURFACE, LAVA_LAYER = 16, 4
# The physics layers of the walls a shot meets (`D-202`): stone slabs in 1,
# the mine's iron in 2, wooden decks in 3; tiles and every other body in 0.
SLAB_LAYER, IRON_LAYER, WOOD_LAYER = 1, 2, 3
# The elevator's sheave in its gantry's art, in tiles from the gantry's
# top-left: the wheel's centre over the shaft, its rim's bottom.
LIFT_SHEAVE = (0.5, 0.6)
WOOD = ('bridge_rope', 'bridge_weathered')
# A ground platform's style and the family its tiles come from.
FAMILIES = {'ground': 'masonry', 'pit': 'masonry', 'wet': 'wet', 'basalt': 'basalt', 'mine': 'mine',
            'channel': 'channel', 'bridge_deck': 'bridge_deck'}
# The pairs of families that meet through a border tile, left then right.
BORDER_PAIRS = {('masonry', 'wet'): 'rock_wet', ('wet', 'basalt'): 'rock_basalt',
                ('basalt', 'mine'): 'basalt_rock', ('mine', 'masonry'): 'mine_rock'}
# The pit's lining, by weight.
PIT_KINDS = (('plain', 10), ('mossy', 3), ('cracked', 2), ('ring', 1), ('niche', 1))
# A view at the 0.35 zoom on a 16:9 window, in tiles, and the map's lights
# any such window may hold, so the player's lantern, a strike and three fire
# missiles stay under the renderer's 16.
LIGHT_WINDOW, LIGHT_BUDGET = (92, 52), 10
# A bat roosts on terrain within this many pixels above its spawn.
ROOST_REACH = 480
# The tests' sky: columns and rows of the map that hold nothing.
TEST_SKY = (38, 20)
# Draw order among the level's sprites, every tile layer under them: props
# behind the world, then the world's props, hazards and decks.
Z_BACK, Z_WORLD, Z_DECK, Z_SPIKES, Z_FIRE = -20, -10, -4, -5, 6
# The soft glow and point light a prop or a hazard carries: its centre from the
# sprite's top-left, its radius and its colour, the light's strength (0.8 when
# left out) and a fireball's hot core.
GLOWS = {'ex10_lantern': dict(offset=[32, 36], radius=150, color=[255, 181, 98, 200]),
         'ex10_crystal': dict(offset=[32, 36], radius=150, color=[94, 210, 255, 180]),
         'ex10_hanging_lantern': dict(offset=[32, 24], radius=130, color=[255, 181, 98, 200]),
         'ex10_campfire_0': dict(offset=[33, 42], radius=175, color=[255, 125, 45, 230], light=1.2)}
LAVA_GLOW = dict(radius=220, color=[255, 120, 40, 230], light=1.3)
FIRE_GLOW = dict(offset=FIRE_CENTRE, radius=175, color=[255, 125, 45, 255], light=1.6, core=62)
# Molten drips leak from each fireball's underside, a child emitter.
FIRE_DRIPS = 'ex10_fire_drips'


def slab_runs(slabs):
    """`slabs` (`[x, y, width, height, wood]`) with every run that shares a
    row, a height and a material and touches end to end merged into one box,
    by row, then x: one collider per platform, so a body sunk into a deck
    meets no join."""
    runs = []
    for x, y, w, h, wood in sorted(slabs, key=lambda s: (s[1], s[3], s[4], s[0])):
        if runs and runs[-1][1] == y and runs[-1][3] == h and runs[-1][4] == wood and runs[-1][0] + runs[-1][2] == x:
            runs[-1][2] += w
        else:
            runs.append([x, y, w, h, wood])
    return runs


def boxes(cells):
    """`cells` covered by the fewest boxes a greedy pass finds: each from its
    top-left free cell, as wide as its row runs, then as deep as every row
    under it runs as wide. `[col, row, width, height]` in tiles."""
    free = set(cells)
    out = []
    for x, y in sorted(cells, key=lambda c: (c[1], c[0])):
        if (x, y) not in free:
            continue
        w = 1
        while (x + w, y) in free:
            w += 1
        h = 1
        while all((x + i, y + h) in free for i in range(w)):
            h += 1
        for j in range(h):
            for i in range(w):
                free.discard((x + i, y + j))
        out.append([x, y, w, h])
    return out


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

    # The slabs sit in physics layer 1 (stone) or 3 (wood), which an enemy's
    # sight casts against beside the tiles and a shot meets by; every other
    # body stays in layer 0, and the slabs' mask keeps every layer, so nothing
    # collides differently.
    slabs = []
    for x, y, w, h, wood in slab_runs(source['slab_colliders']):
        cls = f"ex10_{'wood' if wood else 'slab'}_{int(w)}x{int(h)}"
        prefabs[cls] = dict(components=dict(rigid_body=dict(kind='static',
            shape=dict(kind='aabb', half_extents=[w / 2, h / 2]), offset=[w / 2, h / 2],
            layers=[WOOD_LAYER if wood else SLAB_LAYER])))
        slabs.append(dict(obj(cls, x, y), width=w, height=h, point=False))
    layer('slabs', slabs)
    # The mine's iron faces, merged boxes in physics layer 2.
    irons = []
    for x, y, w, h in source['iron_boxes']:
        cls = f'ex10_iron_{int(w)}x{int(h)}'
        prefabs[cls] = dict(components=dict(rigid_body=dict(kind='static',
            shape=dict(kind='aabb', half_extents=[w / 2, h / 2]), offset=[w / 2, h / 2], layers=[IRON_LAYER])))
        irons.append(dict(obj(cls, x, y), width=w, height=h, point=False))
    layer('iron', irons)

    # The drips the fireballs leak, which the fire missile borrows too.
    prefabs[FIRE_DRIPS] = dict(components=dict(particle_emitter=dict(config='ex10_fireball_drips')))
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
    # The lava basins after the fires: a surface the player stands on, a
    # touch from just over it to the bed that burns and throws, a sizzle at
    # each touch, and the lake's light. A child of each touches what is
    # frozen with fire, which blows it apart whether it takes fire or not (a
    # frozen wisp).
    for i, basin in enumerate(source.get('lava', [])):
        width = (basin['right'] - basin['left']) * tile
        depth = (lava_bed(source, basin) - basin['row']) * tile - LAVA_TOP + LAVA_REACH
        box = dict(shape=dict(aabb=[width / 2, depth / 2]),
                   offset=[width / 2, LAVA_TOP - LAVA_REACH + depth / 2])
        frost = f'ex10_lava_frost_{i}'
        prefabs[frost] = dict(components=dict(contact_damage=dict(
            box, damage={'damage.fire': HAZARD_DAMAGE}, filter=dict(all=['status.frozen']))))
        bite = dict(box, damage={'damage.fire': LAVA_DAMAGE}, filter=dict(all=['team.player']),
                    knockback=LAVA_KNOCKBACK, interval=LAVA_INTERVAL, push_always=True)
        surface = dict(kind='static', shape=dict(kind='aabb', half_extents=[width / 2, LAVA_SURFACE / 2]),
                       offset=[width / 2, LAVA_TOP + LAVA_SURFACE / 2], layers=[LAVA_LAYER], mask=[])
        components = dict(hazard=dict(fire=False, lava=True), contact_damage=bite, rigid_body=surface,
                          cues=dict(hit=[dict(sound=dict(id='ex10_lava_sizzle_sfx', min_interval=0.3))]),
                          spawn_children=[dict(prefab=frost, offset=[0, 0])])
        if basin.get('light'):
            components['glow'] = dict(LAVA_GLOW, offset=[width / 2, LAVA_TOP])
        cls = f'ex10_lava_{i}'
        prefabs[cls] = dict(components=components)
        hazards.append(obj(cls, basin['left'] * tile, basin['row'] * tile))
    layer('hazards', hazards)

    platforms = []
    deck = images['lift_deck']
    for i, m in enumerate(source['moving_platforms']):
        if 'elevator' in m:
            # The mine's elevator (M79): a cage whose floor's top rests at
            # `position`'s row in the shaft's collar, which 01 sends up `rise`
            # rows when stood on; its sprite stands over its floor's body.
            lift = m['elevator']
            cage = images['lift_deck_elevator']
            half = [m['half_width'] * tile, source['deck_depth'] / 2]
            floor = cage.height - source['deck_depth']
            cls = f'ex10_platform_{i}'
            prefabs[cls] = dict(sprite=dict(asset_id='ex10_lift_deck_elevator', z_order=Z_DECK), components=dict(
                rigid_body=dict(kind='kinematic', shape=dict(kind='aabb', half_extents=half),
                                offset=[half[0], floor + half[1]]),
                elevator=dict(rise=lift['rise'] * tile, speed=lift['speed'])))
            platforms.append(obj(cls, m['position'][0] * tile - half[0], m['position'][1] * tile - floor))
            continue
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
        fields = dict(name=area['name'], indoor=area.get('indoor', False), ambience=area.get('ambience'),
                      backdrop=area.get('backdrop'), night=area.get('night', False),
                      horizon=area.get('horizon', 0), ridges=area.get('ridges', True),
                      woodland=area.get('woodland', True))
        prefabs[cls] = dict(components=dict(area=fields, zone=dict(filter=dict(all=['team.player'])),
            rigid_body=dict(kind='sensor', shape=dict(kind='aabb', half_extents=half), offset=half)))
        areas.append(obj(cls, first * tile, 0, name=area['name']))
    layer('areas', areas)

    # The ambient emitters as point objects of the prefab class
    # `ex10_ambient_emitter`; their properties set the emitter's config and seed.
    layer('emitters', [obj('ex10_ambient_emitter', p['x'] * tile, p['y'] * tile, properties=[
        dict(name='particle_emitter.config', type='string', value=p['config']),
        dict(name='particle_emitter.seed', type='int', value=p['seed'])]) for p in source['emitters']])
    # Markers the game reads by name: the player's feet at a fresh run's start,
    # and where a fresh run sets each loose iron block, crate and keg down,
    # its box's centre.
    spawn = source['spawn']
    # The elevator's sheave, where its cable hangs from: the gantry's first
    # piece, set down on its support, plus the wheel's place in its art.
    sheaves = [obj('', (p['x'] + LIFT_SHEAVE[0]) * tile, (p['y'] + LIFT_SHEAVE[1]) * tile, name='lift_sheave')
               for p in source['props'] if p['sprite'] == 'ex10_lift_gantry_big_0_0']
    layer('markers', [obj('', spawn['col'] * tile, spawn['surface_row'] * tile, name='spawn')]
          + [obj('', block['x'] * tile, block['y'] * tile, name='iron') for block in source.get('iron', [])]
          + [obj('', box['x'] * tile, box['y'] * tile, name=kind) for kind in ('crate', 'keg')
             for box in source.get(kind + 's', [])]
          + sheaves)
    # The training dummies; each placement's tags set the `dummy` prefab's, one
    # by one.
    layer('dummies', [obj('dummy', d['x'] * tile, d['y'] * tile, name=d['name'], properties=[
        dict(name=f'tags.{i}', type='string', value=tag) for i, tag in enumerate(d['tags'])])
        for d in source['dummies']])
    # The enemies, after every other layer but the pickups, so every earlier
    # object keeps its save ID; each object's class names its kind's prefab,
    # its point the sprite's top-left, and a leash in tiles bounds a patrol.
    layer('enemies', [obj(e['kind'], e['x'] * tile, e['y'] * tile, name=e['name'], properties=[
        dict(name='enemy.leash', type='float', value=e['leash'] * tile)] if 'leash' in e else [])
        for e in source['enemies']])
    # The placed health orbs, the map's last layer, after the enemies so the
    # map's earlier objects keep their save IDs; the run takes their fade off.
    layer('pickups', [obj('health_orb', p['left'] * tile, p['top'] * tile, name=p['name'])
        for p in source.get('pickups', [])])
    return layers, prefabs


def weighted(rng, choices):
    """One of `choices` (`(value, weight)` pairs) drawn by weight."""
    roll = rng.randrange(sum(weight for _, weight in choices))
    for value, weight in choices:
        roll -= weight
        if roll < 0:
            return value
    raise AssertionError(choices)


def terrain_cells(source):
    """Every occupied cell and the platform it takes its tile from: a ground
    family's platform before a deck, then file order."""
    cells = {}
    for p in source['platforms']:
        for y in range(p['row'], p['bottom']):
            for x in range(p['left'], p['right']):
                held = cells.get((x, y))
                if held is None or (held['style'] not in FAMILIES and p['style'] in FAMILIES):
                    cells[(x, y)] = p
    return cells


def terrain_tiles(source, cells, rng):
    """Each occupied cell's sprite, and the cells drawn as iron cladding."""
    cols, rows = source['cols'], source['rows']
    pit = source.get('pit')
    inside = set()
    if pit:
        inside = {(x, y) for x in range(pit['left'], pit['right']) for y in range(pit['rim'], pit['floor'])}

    def family(c):
        p = cells.get(c)
        return FAMILIES.get(p['style']) if p else None

    def closed(c):
        x, y = c
        if not 0 <= x < cols or y >= rows:
            return True
        return y >= 0 and family(c) is not None

    def face(c):
        x, y = c
        return family(c) == 'mine' and (not closed((x - 1, y)) or not closed((x + 1, y)))

    tiles, iron = {}, set()
    for x, y in sorted(cells, key=lambda c: (c[1], c[0])):
        p = cells[(x, y)]
        fam = FAMILIES.get(p['style'])
        if fam is None:
            tiles[(x, y)] = p['style']
            continue
        up, down = closed((x, y - 1)), closed((x, y + 1))
        left, right = closed((x - 1, y)), closed((x + 1, y))
        mask = (not up) * 1 + (not right) * 2 + (not down) * 4 + (not left) * 8
        if fam == 'masonry' and ((x + 1, y) in inside or (x - 1, y) in inside):
            side = 'left' if (x + 1, y) in inside else 'right'
            sprite = f'pit_rim_{side}' if not up else f'pit_wall_{weighted(rng, PIT_KINDS)}_{side}'
        elif fam == 'masonry' and pit and y == pit['floor'] and pit['left'] <= x < pit['right']:
            sprite = f'pit_floor_{rng.randrange(4)}'
        elif fam == 'masonry':
            if up:
                variant = rng.randrange(8)
                if not down:
                    sprite = 'under_left' if not left else 'under_right' if not right else f'under_{variant}'
                else:
                    sprite = 'cliff_left' if not left else 'cliff_right' if not right else f'rock_{variant}'
            else:
                sprite = 'ground_left' if not left else 'ground_right' if not right else f'moss_rock_{rng.randrange(8)}'
        elif fam == 'wet':
            sprite = f'wet_rock_{mask}_{rng.randrange(4)}'
        elif fam == 'basalt':
            if not up:
                sprite = 'basalt_top_left' if not left else 'basalt_top_right' if not right else f'basalt_top_{rng.randrange(4)}'
            elif not down:
                sprite = f'basalt_under_{rng.randrange(4)}'
            else:
                sprite = 'basalt_cliff_left' if not left else 'basalt_cliff_right' if not right else f'basalt_{rng.randrange(16)}'
        elif fam == 'mine':
            # Iron hangs on a face and one cell behind it, never under open air.
            if up and (mask & 14 or face((x - 1, y)) or face((x + 1, y))):
                sprite = f'iron_clad_{mask}_{rng.randrange(4)}'
                iron.add((x, y))
            else:
                sprite = f'mine_rock_{mask}_{rng.randrange(4)}'
        elif fam == 'channel':
            sprite = ('channel_break_left' if not left else 'channel_break_right' if not right
                      else weighted(rng, (('channel_0', 3), ('channel_1', 3), ('channel_2', 3), ('channel_3', 3),
                                          ('channel_mossy', 2), ('channel_cracked', 1))))
        else:
            sprite = weighted(rng, (('bridge_deck_0', 3), ('bridge_deck_1', 1)))
        pair = BORDER_PAIRS.get((fam, family((x + 1, y))))
        if pair and (x, y) not in iron and fam != 'channel':
            sprite = f'border_{pair}' + ('_top' if not up else '')
        tiles[(x, y)] = sprite
    return tiles, iron


def background_tiles(source, cells, props, rng):
    """The background layer: the caves' backs under an indoor area's roof,
    the miners' shoring in its `shoring` columns, the tunnel mouths, the
    outdoor `backs` spans and the river's depths."""
    cols, rows = source['cols'], source['rows']
    ground = {c for c, p in cells.items() if p['style'] in FAMILIES}
    out = {}
    for area in source['landmarks']:
        if not area.get('indoor'):
            continue
        first, last = area['columns']
        shoring = area.get('shoring', [cols, cols])
        for x in range(first, last + 1):
            roofed = False
            for y in range(rows):
                if (x, y) in ground:
                    roofed = True
                    continue
                if not roofed or (x, y) in cells:
                    continue
                if (x, y - 1) in ground:
                    # Behind the roof's lowest cell, whose underside the art
                    # cuts away, the back runs on up, so the cut shows the
                    # cave's wall and never the sky; a plain tile, outside
                    # the random stream.
                    out[(x, y - 1)] = f'cave_back_rock_{x % 4}'
                if shoring[0] <= x <= shoring[1]:
                    kind = weighted(rng, (('timber_0', 3), ('timber_1', 3), ('xbrace_0', 2), ('ore_0', 1), ('ore_1', 1), ('rock_0', 2)))
                    out[(x, y)] = 'cave_back_ladder' if x == shoring[0] + 1 and rng.randrange(3) == 0 else f'cave_back_{kind}'
                elif (x, y - 1) in ground and rng.randrange(5) < 2:
                    out[(x, y)] = f'cave_back_stalactites_{rng.randrange(2)}'
                elif x % 11 == 5:
                    out[(x, y)] = f'cave_back_pillar_{rng.randrange(2)}'
                else:
                    out[(x, y)] = weighted(rng, ((f'cave_back_rock_{rng.randrange(4)}', 12), (f'cave_back_ember_{rng.randrange(4)}', 2)))
    for tunnel in source.get('tunnels', []):
        for r in range(2):
            for c in range(2):
                out[(tunnel['col'] + c, tunnel['row'] + r)] = f"cave_tunnel_{tunnel['kind']}_big_{r}_{c}"
    falls = {int(p['x']) for p in props if p['sprite'].startswith('ex10_falls_')}
    # Every span's cells first, so a wall built of many spans takes its edges
    # from its outline, not from each span's.
    backs = {}
    for span in source.get('backs', []):
        for x in range(span['left'], span['right']):
            for y in range(span['top'], rows):
                # A wall runs on behind rocks jutting in front of it.
                if (x, y) not in ground:
                    backs[(x, y)] = span['family']

    def open_(c):
        return c not in backs and c not in ground and 0 <= c[0] < cols and c[1] < rows

    def face(c):
        return c in backs and (open_((c[0] - 1, c[1])) or open_((c[0] + 1, c[1])))

    for (x, y), family in sorted(backs.items(), key=lambda item: (item[0][1], item[0][0])):
        mask = open_((x, y - 1)) * 1 + open_((x + 1, y)) * 2 + open_((x, y + 1)) * 4 + open_((x - 1, y)) * 8
        if family == 'wet_back':
            variant = 2 if x in falls else rng.randrange(2)
            out[(x, y)] = f'wet_back_{mask}_{variant}'
        elif not mask & 1 and (mask & 14 or face((x - 1, y)) or face((x + 1, y))):
            out[(x, y)] = f'cut_back_iron_{mask}_{rng.randrange(2)}'
        else:
            out[(x, y)] = f'cut_back_{mask}_{rng.randrange(2)}'
    # The same behind each ceiling's or ledge's lowest cell over a wall:
    # plain inner tiles, after the walls' own, outside the random stream.
    for (x, y), family in sorted(backs.items(), key=lambda item: (item[0][1], item[0][0])):
        if (x, y - 1) in ground and (x, y - 1) not in out:
            out[(x, y - 1)] = f'{family}_0_{x % 2}'
    river = source.get('river')
    if river:
        for x in range(river['left'], river['right']):
            for y in range(river['row'] + 1, rows):
                depth = y - river['row']
                out[(x, y)] = 'river_deep_0' if depth <= 3 else 'river_dark_1' if depth <= 7 else 'river_dark_2'
    return out


def lava_bed(source, basin):
    """The row of the bed under `basin`: the highest platform row below its
    surface across its first column."""
    x = basin['left']
    return min(p['row'] for p in source['platforms']
               if p['left'] <= x < p['right'] and p['row'] > basin['row'])


def water_props(source, cells):
    """The lava in each basin and the river's surface, as props with their
    clips: lava from a basin's `row` down to the terrain."""
    ground = {c for c, p in cells.items() if p['style'] in FAMILIES}
    props = []
    for basin in source.get('lava', []):
        for x in range(basin['left'], basin['right']):
            y = basin['row']
            while (x, y) not in ground and y < source['rows']:
                part = 'surface' if y == basin['row'] else 'deep'
                props.append(dict(sprite=f'ex10_lava_{part}_0', animation=f'ex10_lava_{part}', depth='world', x=x, y=y))
                y += 1
    river = source.get('river')
    if river:
        for x in range(river['left'], river['right']):
            kind = 'river_rock' if x in river.get('rocks', []) else 'river_rapids' if x in river.get('rapids', []) else 'river'
            props.append(dict(sprite=f'ex10_{kind}_0', animation=f'ex10_{kind}', depth='world', x=x, y=river['row']))
    return props


def build_level(images, lit):
    source=json.loads((TOOLS/'level.json').read_text())
    from placement import prepare
    source=prepare(source,images)
    cols,rows=source['cols'],source['rows']
    from backdrops import BACKDROPS
    catalog=sorted('ex10_'+s for s in images if s not in BACKDROPS)
    gids={s:i+1 for i,s in enumerate(catalog)}
    layers={name:[0]*(cols*rows) for name in ['background','decorations','terrain','foreground','collision']}
    rng=rng_for('terrain-variation')
    def put(layer,x,y,sprite):
        if not 0<=x<cols or not 0<=y<rows:raise ValueError('placement outside map')
        layers[layer][y*cols+x]=gids[sprite]
    cells=terrain_cells(source)
    tiles,iron=terrain_tiles(source,cells,rng)
    for (x,y),sprite in sorted(tiles.items(),key=lambda c:(c[0][1],c[0][0])):
        put('terrain',x,y,'ex10_'+sprite)
        if cells[(x,y)]['style'] in FAMILIES and (x,y) not in iron:put('collision',x,y,'ex10_ground')
    tile=source['tile']
    source['iron_cells']=sorted(iron)
    source['iron_boxes']=[[x*tile,y*tile,w*tile,h*tile] for x,y,w,h in boxes(iron)]
    source['props']+=water_props(source,cells)
    for (x,y),sprite in sorted(background_tiles(source,cells,source['props'],rng).items(),key=lambda c:(c[0][1],c[0][0])):
        put('background',x,y,'ex10_'+sprite)
    for p in source['decorations']:put(p['layer'],p['col'],p['row'],p['sprite'])
    # The far mist: each band's tiles across its columns in the decorations
    # layer, over the far walls and under the terrain, where no terrain is or
    # the terrain's art leaves room for it to show (a ceiling's cut-away
    # underside, a deck), so a band never starts on a hard edge.
    from backdrops import FAR_MISTS, TILE as BAND_TILE
    def opaque(cell):
        return cell in tiles and images[tiles[cell]].getchannel('A').getextrema()[0] == 255
    for mist in source.get('mists', []):
        band = FAR_MISTS[mist['band']]
        for r in range(-(-band[0] // BAND_TILE)):
            for x in range(mist['left'], mist['right']):
                if not opaque((x, mist['row'] + r)):
                    put('decorations', x, mist['row'] + r, f"ex10_far_mist_{mist['band']}_big_{r}_{x % 16}")
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
    shallow={(int(x/source['tile']),int(y/source['tile'])) for x,y,w,h,_ in source.get('slab_colliders',[])}
    # Every terrain cell is a collision tile, a slab or an iron box's.
    iron={tuple(c) for c in source.get('iron_cells',[])}
    if any(bool(terrain[i])!=(bool(collision[i]) or (i%cols,i//cols) in shallow or (i%cols,i//cols) in iron) for i in range(cols*rows)):raise ValueError('route/collider disagreement')
    validate_layout(source,tiled,iron,shallow)
    def safe(x,y):
        return terrain[y*cols+x] and all(not terrain[(y-dy)*cols+x] for dy in (1,))
    if not safe(int(source['spawn']['col']),source['spawn']['surface_row']):raise ValueError('unsafe spawn')
    platforms={p['name']:p for p in source['platforms']}
    for route in source['routes'].values():
        for name in route:
            p=platforms[name]
            if not any(safe(x,p['row']) for x in range(p['left'],p['right'])):raise ValueError('route lacks headroom')
    for p in source.get('moving_platforms',[]):
        if 'elevator' in p:
            # The elevator's shaft: inside the map, and no tile, slab or iron
            # across the cage's width from its rest to its top and the head
            # room over it there (M79).
            lift=p['elevator'];x,row=p['position'];half=p['half_width']
            if row-lift['rise']-2<0 or row>=rows or x-half<0 or x+half>cols:raise ValueError('elevator outside map')
            columns=range(math.floor(x-half),math.ceil(x+half))
            for y in range(row-lift['rise']-1,row):
                for c in columns:
                    if terrain[y*cols+c] or (c,y) in iron:raise ValueError(f'elevator shaft blocked at {c},{y}')
            for slab in source['slab_colliders']:
                sx,sy=slab[0]/source['tile'],slab[1]/source['tile']
                if x-half<sx+1 and sx<x+half and row-lift['rise']-1<=sy<row:raise ValueError(f'elevator shaft blocked by a slab at {sx},{sy}')
    for p in source.get('hazards',[])+[m for m in source.get('moving_platforms',[]) if 'elevator' not in m]:
        values=p['position']+p['travel']+[p['period'],p['phase']]
        if not all(math.isfinite(v) for v in values) or p['period']<=0:raise ValueError('invalid motion')
        for center,travel,limit in zip(p['position'],p['travel'],(cols,rows)):
            if center-abs(travel)<0 or center+abs(travel)>=limit:raise ValueError('motion outside map')
    spawn=(source['spawn']['col'],source['spawn']['surface_row']-28/source['tile'])
    for p in source.get('hazards',[]):
        if all(abs(center-safe)<abs(travel)+1 for center,safe,travel in zip(p['position'],spawn,p['travel'])):raise ValueError('hazard at spawn')
    for p in source['props']+source['decorations']:
        if p['sprite'] not in sprite_ids:raise ValueError('unknown prop reference')



def validate_layout(source, tiled, iron, shallow):
    """M76's checks: the areas tile the map's columns, the tests' sky stays
    empty, no view holds past the light budget, bats find a roof, fires
    sweep clear, the pit holds the balls inside its shell, no iron lies under
    open air, lava stays in its basin and `backs` stay outdoors."""
    cols, rows, tile = source['cols'], source['rows'], source['tile']
    columns = [area['columns'] for area in source['landmarks']]
    if columns[0][0] != 0 or columns[-1][1] != cols - 1 or any(b[0] != a[1] + 1 for a, b in zip(columns, columns[1:])):
        raise ValueError('landmarks must tile the columns 0 to cols - 1')
    solid = set()
    for layer in tiled['layers']:
        if layer['type'] == 'tilelayer':
            for i, gid in enumerate(layer['data']):
                if gid:
                    solid.add((i % cols, i // cols))
                    if i % cols < TEST_SKY[0] and i // cols < TEST_SKY[1]:
                        raise ValueError(f"tests' sky: a {layer['name']} tile at {i % cols},{i // cols}")
        elif layer['name'] != 'areas':
            # An area's zone spans every row by design; nothing else may.
            for o in layer['objects']:
                if o['x'] < TEST_SKY[0] * tile and o['y'] < TEST_SKY[1] * tile:
                    raise ValueError(f"tests' sky: {o['type'] or o['name']} at {o['x'] / tile},{o['y'] / tile}")
    collision = next(l['data'] for l in tiled['layers'] if l['name'] == 'collision')
    walls = {(i % cols, i // cols) for i, gid in enumerate(collision) if gid} | shallow | iron
    # The map's lights: glowing props, fires, the lava lake, wisps and the
    # campfire, each at its point in tiles.
    lights = [(p['x'], p['y']) for p in source['props'] if p['sprite'] in GLOWS]
    lights += [tuple(h['position']) for h in source['hazards'] if h['fire']]
    lights += [((b['left'] + b['right']) / 2, b['row']) for b in source.get('lava', []) if b.get('light')]
    lights += [(e['x'], e['y']) for e in source['enemies'] if e['kind'] == 'wisp']
    w, h = LIGHT_WINDOW
    for x0 in range(0, max(1, cols - w + 1)):
        for y0 in range(0, max(1, rows - h + 1), 4):
            inside = [l for l in lights if x0 <= l[0] < x0 + w and y0 <= l[1] < y0 + h]
            if len(inside) > LIGHT_BUDGET:
                raise ValueError(f'{len(inside)} lights in the view from {x0},{y0}: {sorted(inside)}')
    for e in source['enemies']:
        if e['kind'] != 'bat':
            continue
        col, top = int(e['x'] + 0.5), e['y'] + 0.5
        if not any((col, r) in walls for r in range(int(top - ROOST_REACH / tile), int(top) + 1)):
            raise ValueError(f"bat {e['name']}: no roof within {ROOST_REACH} px")
    for i, h in enumerate(source['hazards']):
        if not h['fire']:
            continue
        hx, hy = (FIRE_HALF[0] / tile, FIRE_HALF[1] / tile)
        for k in range(41):
            t = math.sin(math.tau * k / 40)
            cx, cy = (c + travel * t for c, travel in zip(h['position'], h['travel']))
            for x in range(int(cx - hx), int(cx + hx) + 1):
                for y in range(int(cy - hy), int(cy + hy) + 1):
                    if (x, y) in walls:
                        raise ValueError(f'fire {i} sweeps into terrain at {x},{y}')
    pit = source.get('pit')
    if pit:
        left, right, rim, floor = pit['left'], pit['right'], pit['rim'], pit['floor']
        if any((x, y) in solid and (x, y) not in shallow for x in range(left, right) for y in range(rim, floor)):
            raise ValueError('the pit interior holds a tile')
        if (right - left) * (floor - rim) * tile * tile < 3.1e6:
            raise ValueError('the pit interior is under 3.1 Mpx2')
        shell = [(x, y) for y in range(rim, floor + 4) for x in list(range(left - 4, left)) + list(range(right, right + 4))]
        shell += [(x, y) for y in range(floor, floor + 4) for x in range(left, right)]
        if any(c not in walls for c in shell):
            raise ValueError('the pit shell must be four tiles thick')
    for x, y in iron:
        if (x, y - 1) not in solid:
            raise ValueError(f'iron under open air at {x},{y}')
    terrain = {(i % cols, i // cols) for i, gid in enumerate(next(l['data'] for l in tiled['layers'] if l['name'] == 'terrain')) if gid}
    for b in source.get('lava', []):
        y = b['row']
        while not (b['left'], y) in terrain and y < rows:
            if (b['left'] - 1, y) not in terrain or (b['right'], y) not in terrain:
                raise ValueError(f"lava basin at {b['left']}: open at its side, row {y}")
            y += 1
        if any((x, y) not in terrain for x in range(b['left'], b['right'])):
            raise ValueError(f"lava basin at {b['left']}: open floor")
    outdoor = [a['columns'] for a in source['landmarks'] if not a.get('indoor')]
    for span in source.get('backs', []):
        if not any(a[0] <= span['left'] and span['right'] - 1 <= a[1] for a in outdoor):
            raise ValueError(f"backs at {span['left']}: outside an outdoor area")
