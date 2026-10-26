# Twilight ruin authoring

The example runs from checked-in PNGs, JSON and WAVs. Nothing here runs from
Cargo, engine startup or asset loading. Python 3 and `Pillow==12.3.0` are needed
only to edit or regenerate the artwork, the level and the sounds.

Paths below are relative to this `tools/` directory unless a command starts at the repository root. For art, read [Editing art](#editing-art); for geometry, [Editing the level](#editing-the-level); for presentation, [Runtime and visual checks](#runtime-and-visual-checks); for the spells and the sounds, [Spells and the HUD](#spells-and-the-hud), [Fire missile](#fire-missile-extinguishing-and-sound-effects) and [Ice spray](#ice-spray-and-thermal-shattering); for the elements, the gun and the dummy, [Elements, weapons and the dummy](#elements-weapons-and-the-dummy). The game's Rust is in `src/features/`, relative to the example root, and [its README](../README.md) maps each mechanic to the kit item and the data file behind it.

From the repository root:

```sh
python3 -m pip install -r examples/01_platformer/tools/requirements.txt
python3 examples/01_platformer/tools/generate.py
just level-check
cargo test -p example-01-platformer
```

This is the authoring edit loop. At handoff, use the canonical
[validation matrix](../../../docs/validation.md#change-to-check-matrix), running
each applicable check once for that stage. The generator and Rust tests cover
different guarantees; keep both. For visible or audible changes, apply the
content-specific acceptance below by running:

```sh
cd examples/01_platformer && cargo run
```

The local Python tests are separate from `just script-test`. `just level-check` runs
`--check` and them together; it needs Pillow, so CI and the standard-library-only
recipes leave it out. `--check` regenerates
into a temporary directory, compares every owned byte, and fails on missing,
stale or uncovered output. It does not rewrite the working tree. PNG compression
is fixed with no timestamps, and JSON is sorted.
All generated files must accompany their sources in a change. `outputs.json` is
the exact ownership inventory. Normal generation removes only files explicitly
listed by the previous inventory (or `legacy_outputs.json` on the first migration).
It never sweeps unrelated files from the assets directory. `sounds.json` is the
sound registration; synthesized WAVs are owned outputs, every registered sound
among them since 01 (j) made the black hole's opening (see "Fireball spell,
extinguishing and sound effects").

The shell's assets come with its code: Inter and Source Serif 4 with their
licence files, and the `damage_flash` shader and material are copied byte for
byte from `templates/basic/assets/`, with their manifest entries, so `--check`
also catches drift from the template. The `shell` theme is the template's text in
01's colours (01 (h), `generate.THEME_COLORS`): each of the template's colours
maps to 01's, darker backgrounds and a vivid purple, so a key the template adds
still arrives, and a template colour the map lacks stops the generator, named;
`just repo-check` holds the theme to the template's structure, its colours
masked, as it holds `src/shell/` byte for byte. 01 adds one face of its own:
Source Serif 4 Bold, copied from the repository's `assets/fonts/` beside the
Regular and under its licence, as `serif_bold` in the `serif` family, which
the area titles draw at weight 700. `content/` holds the prefab
and data sources, copied verbatim: `content/prefabs/<id>.json` becomes the prefab
`<id>`, and `content/data/<kind>/<id>.json` the `<kind>` entry `<id>`.

## Editing art

`palette.json` maps one-character symbols to RGBA bytes. `.` is transparent.
Every pixel must use the palette. All art is drawn by code; `art.read_grid`
still validates the text-grid format (one line per pixel row, one character per
pixel) for any hand-authored sprite. The modules:

| Module | Draws |
| --- | --- |
| `pixels.py` | Shared toolkit: masks, silhouette shading, overlap rims, outlines, dithering, whole-period waves |
| `actors.py` | The wizard (his gestures, the sprays' staff pose and the gun's pistol pose), fireball, the fire missile's rocket, the small ball, the bowling ball, the training dummy, the six enemies (wisp, crawler, beetle, bat, troll, goblin, with the goblin's knife) and the goblin's rock and bomb |
| `props.py` | 01 (g)'s props: the pine crates `pine_crate_0` and `pine_crate_1` in the palette's pale pine ramp (`Л П У`), apart from the mine's dark crates, the powder keg and its four lit frames (`powder_keg_lit_<n>`), a burning crate's char overlay (`pine_crate_char_<stage>`), and the particle sprites of a crate falling in, a keg blowing and a rock breaking (`char_splinter`, `keg_stave`, `rock_chip`); `art.build_art` calls its `build_props` |
| `terrain.py` | Masonry, turf, cliff edges, decks, trestle, waterfall backing; the level's families (the pit's lining, undersides, wet rock, basalt, mine rock and its iron cladding, the aqueduct's channel, the bridge's deck) and the borders between them |
| `scenery.py` | Lamp posts, ivy, waystone, summit gate, oak, arch, roots, undergrowth, crystals, waterfall, spikes; the level's props (`LEVEL_PROPS`, `HUNG_PROPS`), tiled or cut `_big_`, its water, falls and lava clips, the pool's shore (`pool_shore`), the lava's crust frames (`lava_crust_<part>_<n>_<v>`, nine: the set, its dulling, the hold, two a crack stage) and each row's partials, 8 px apart, as the freeze's front and the melt's come down through a basin (`lava_freeze_*`, `lava_melt_*`, 01 (k)), the elevator's gantry, cage and cable, and the falls' plume; the aqueduct's water (01 (k)): a spill over each broken end's lip (`spill_<side>_<k>`), the pours (`pour_wide`) and their splash feet (`pour_foot`), the drips under the coping (`channel_drips`), the fallen voussoirs and the gargoyle's clip in pale limestone, the water over its tongue |
| `backdrops.py` | Sky, moon, clouds, ridges, woodland; the outdoor areas' five far strips, the true-black night sky, the ravine's mist bands (`BACKDROPS`), and the background tiles: cave backs and shoring, tunnel mouths, the hollow's cave mouth (`cave_mouth`), the aqueduct's arcade and the bridge's piers, each gap's arcade tops cut along one broken profile (`arcade_gap_<g>_<row>_<col>`, 01 (k)) and the wet piers beside them (`pier_wet_<k>`), the ravine's wet backs and the mine's cut |
| `effects.py` | Particle sprites (dust, spark, droplet, firefly mote, cursor), glow gradients, black-hole layers, the sprays' seven clips (`SPRAY_CLIPS`), the force push's wave and dust (`PUSH_CLIPS`), the icon family's 36-pixel tiles of the nine spells, the gun and the nine summon types, the HUD's liquids and cartridges (`hud_liquid_health`, `hud_liquid_mana`, `hud_bullet`, `hud_bullet_spent`), the lightning bolt's segments, the status overlays, the fitted ice cubes and their cracked twins (the crate's and the keg's on `PROP_BOXES`), the ball's cube, the ice glint, the muzzle flash, the tracer, the drops' orbs and the 16-pixel alert and search marks (`mark_alert`, `mark_search`); e01tomes' books, their takes and the bound's ring (`TOME_CLIPS`), the three tome tiles and the shield's liquid (`hud_liquid_shield`) |
| `polish_art.py` | Hearts (which the HUD's bars replaced), burning-ball flames, explosion shock ring, iron block and scraps with frost, snowflakes, ice shards and cold rings |
| `sfx.py` | Synthesized sound effects, the shell's menu sounds and game over's sting, the music loops and the areas' six ambience loops (WAV); e01tomes' three pickups and the bound |
| `shaders/soft_glow.wgsl` | Example-local glow material, copied to `assets/shaders/` |

`actors.py` draws the wizard, the fireball and the fire missile. The wizard is one rig posed per
frame on a 96 px frame: hips, neck, feet, hands, the staff's lean and grip, the
robe's hem, the hat's tip and the beard's swing. Each part (legs and soft boots,
bell sleeves, the robe with its mantle, rope belt, pouch, brass trim and sewn
stars, the head with its nose, brow and beard, the felt hat, the gnarled staff
and its orb) is filled from palette symbols, shaded from its own silhouette,
rimmed where it overlaps the parts behind it and outlined once. The death's
three lying frames turn the upright rig a quarter anticlockwise in whole pixels;
no other raster is rotated. Edit a clip by changing its `WizardPose` list and
durations. Generation rejects a frame unless its lowest pixel sits on row 92
(bounding-box edge 93, which the extractor anchors to the collider's feet, the
body's centre at (48, 65) of the frame), the lying frames' too, or unless its
staff light's anchor lies on the orb. The orb is plain amber glass (`J K L` and
a `Z` glint), never emissive: the staff light's glow, which follows the anchor,
lights it. `build_player` returns each frame's anchor, the drawn glass's centre,
which `generate.py` writes as the `lantern_anchors` entry `player`. Clips:

| Clip | Frames | Motion |
| --- | --- | --- |
| idle | 8, 100–300 ms | breathing, the staff planted, the hat's tip and the beard drifting, one blink |
| walk | 12, 45 ms | leaning stride: 2 px bob, toe-off heel lift, the staff swinging, the hem flaring and the beard swept back |
| jump | 3, 200 ms total | takeoff stretch, rise, apex (held); must finish within 13 frames |
| fall | 4, 80 ms | loop: the robe, the hat's tip and the beard streaming upward |
| land | 3, 60 ms | impact squint and bent knees, recover, settle; runtime squash owns scale |
| double jump | 4, 300 ms total | crouched tuck, then the robe bursts open, the orb flares and the wizard springs up |
| hurt | 2, 200 ms total | knocked back, the head snapping back and the staff jolted, then half-way back |
| death | 7, 910 ms total | struck, he reels back on one foot as the hat lifts off and the staff is flung ahead; sits down hard; topples back; lands flat on his back with a puff of dust; rebounds a pixel; and rests face up, the hat upright behind his head and the staff along the ground |
| toss | 4, 300 ms total | wind-up, swing, release, follow-through: the free hand throws a bowling ball toward the aim, the staff planted |
| bolt | 3, 300 ms total | the staff drawn back over the shoulder, then thrust at the aim, the orb flaring (the fire missile, lightning) |
| push | 3, 300 ms total | the free hand gathers, then shoves out palm first, the staff raised behind (the force push, the black hole) |
| conjure | 3, 300 ms total | the staff raised high in both hands, the orb flaring, then struck down (iron, the summons) |
| staff | 5 | the staff levelled along the aim in both hands, from straight up to straight down by eighths of a turn: the sprays' and the small balls' pose; 01 shows the one nearest the aim, standing |
| aim | 5 | the pistol in the far hand along the aim, the staff upright: the gun's pose, shown the same way |

`generate.py` mirrors every wizard, fireball and fire missile frame into a `_left`
twin, since nothing flips a sprite in place, and each anchor about the frame's
middle, x 48.

Palette symbols `Π Ξ Γ ν` (robe and hat), `Ф` (its lining), `Δ` (the hat's
patch), `ϕ` (sewn stars), `σ τ S w` (beard and hair), `R r X` (skin) and
`" ' ω /` (the staff's wood and the boots) dress the wizard. `M L K J Z` are the
fireball's ember-to-white-hot ramp, whose `J K L Z` also draw the orb.

The fireball is an analytic ball of flame. Tongues stream up and behind a
right-moving hazard, plasma bands swirl through the core, and embers peel off.
Its 10 frames loop every 600 ms. It is self-lit: it is drawn unlit at its
authored colors and has no normal or emissive map. The existing `fire_*` frames
remain the burning-ball flames.

Normal and emissive images derive from each final frame's silhouette and glowing
detail colors, including every player clip and all rune rotations. Only the glow
set's symbols glow, matched by colour value (`art.GLOW`: `y Y C c`, and e01tomes'
`Ъ Ч Я Э Ы`, which no other art uses). Terrain
(names in `terrain.RELIEF_PREFIXES`) and multi-tile scenery (`*_big_*`) use
relief normals instead. Relief treats luminance as height and has no whole-tile
tilt, so light pools cross tile seams smoothly. Fully opaque tiles wrap their
slopes across the tile edge.

Terrain is one masonry texture that repeats exactly every 64 pixels:

- **Variants.** The eight `rock_*` and eight `moss_rock_*` variants may only
  rework stones that lie wholly inside the tile (splits, cracks, stains, rare
  missing blocks, moss in the joints). Any arrangement therefore meets without a
  seam. `validate_terrain` enforces shared edge columns and a shared six-row turf lip.
- **Depth.** Fill below the surface is one tone darker; surface tiles fade into
  it by dither from row 18.
- **Edges.** `level.py` gives exposed wall faces `cliff_left`/`cliff_right`: the
  left face is in shadow, the right is moonlit. It still draws the variant for
  those tiles, so every other tile keeps its variant.
- **Decks.** Slab, bridge and lift decks span the full tile at exactly 23 pixels
  deep, their collision depth.
- **Legacy names.** The older unplaced terrain names are copies of the new
  family, so no sprite ID changed.

The level (01 (d)) adds a family per ground style (`level.FAMILIES`): `ground`
and `pit` are masonry, `wet` the ravine's rock, `basalt` the hollow's, `mine`
the cut's, `channel` the aqueduct's and `bridge_deck` the troll bridge's.
A channel run's ends are broken (01 (k)): each takes `channel_break_<side>_<k>`,
`k` the run's place along the aqueduct modulo 3, so a gap pairs right `k` with
left `k + 1`; each variant's face is ragged under its lip
(`terrain.BREAK_LIPS`), a right lip over its pier's shaft and a left one over
the haunch beside its pier, and choosing it draws nothing from the random
stream. A cell in `spouts` takes `channel_spout` after its weighted draw, so
no other tile's variant moves.
Masonry keeps the rules above and adds `under_*` for a face open below and the
pit's lining (`pit_rim_*`, `pit_wall_<kind>_*` drawn by `PIT_KINDS`' weights,
`pit_floor_*`). Wet and mine rock take one of 16 masks by their open sides (up
1, right 2, down 4, left 8) and four variants each, `wet_rock_<mask>_<v>` and
`mine_rock_<mask>_<v>`; basalt takes tops, undersides, cliffs and sixteen fill
variants. In the mine, a cell under cover on an open face, or the cell behind
one, is iron cladding, `iron_clad_<mask>_<v>`, never under open air; those
cells become the `iron` layer's boxes. Where two families meet left to right,
`level.BORDER_PAIRS` draws the left cell as `border_<pair>`, or
`border_<pair>_top` on the surface: masonry into wet rock, wet rock into
basalt, basalt into mine rock and mine rock into masonry. `validate_terrain`
holds every variant to its family's shared edges on both sides.

Art is 64 × 64 unless `art.py`'s size tables say otherwise: the backdrops, the
black hole's 96-pixel layers, the 36-pixel icon tiles, the elements' sizes (the
bolt's segments, the status overlays' small and large classes, the muzzle flash,
the tracer, the glint, the orbs, the bars' liquids and the cartridges), the
wizard, whose frames are 96 pixels, and the troll, whose frames and fitted ice
cube are 128 pixels, the one enemy larger than a tile (`SIZE_PREFIXES`). Backdrop
strips are 1024 pixels wide and
wrap seamlessly: profiles are whole-period sine sums, and shapes are drawn at x
and x ± 1024. The ridges' and woodland's last row stays opaque, because the
runtime stretches it downward. The sky must keep a mean luminance below 25; the
moon keeps bounds `(7,7,57,57)` and a bright centre. The outdoor areas' far strips
(`far_tower`, `far_arcade`, `far_cliffs`, `far_mine`, `far_valley`) wrap the same
way; `far_arcade` wraps in the middle of an opening and `far_mine`'s heaps are
periodic. `sky_black` is true black, the palette's `■` (0, 0, 0, 255), and
`mist_0` is the ravine's translucent band over its far strip. `backdrops.BACKDROPS`
keeps them all out of the tileset and out of `art.py`'s size check. Particle
sprites stay 64×64 and render small via config scale.

The far walls draw as the review drew them: the gorge's `wet_back` and the
cut's `cut_back` and `cut_back_iron` under the far tint (`terrain.FAR_TINT`,
104, 112, 136 out of 255), unlit, so they stand dark and cool behind the lit
terrain. `terrain.tinted` maps each symbol to the palette colour nearest its
colour times the tint; the palette's far ramp (`а` to `ь`) holds those colours
exactly, so the tint stays palette-only. The cut's far props (`far_trestle`, a
step darker, `far_boulder_0`, `far_boulder_1`, `far_rubble` and the lantern's
speck, `far_lantern`) take the same tint (`scenery.FAR_PROPS`). The far mist,
`far_mist_<band>_big_<row>_<col>`, cuts `backdrops.FAR_MISTS`' bands into
tiles for the decorations layer, over the far walls and under the terrain. The
lava's heat haze (`haze`, `haze_small`) is the plume in the palette's warm
translucents (`э` to `Д`), and the river darkens below its first rows
(`river_dark_1`, `river_dark_2`). The rising mist is animated
(`scenery.RISING`): the falls' `plume`, drawn at six phases of one loop,
150 ms a frame, each tile of its cut its own clip (`<name>_big_<row>_<col>`,
frames `_0` to `_5`). The ravine's pool runs out against fallen stones at its
east end, `pool_shore`: four frames of 3 × 2 tiles, each tile with art a clip
(`pool_shore_big_<row>_<col>`).

Cosmetic RNG streams use the first eight bytes of SHA-256 of the asset name,
not Python's process-randomized hash. Changing one asset does not perturb others.
To review art, make a contact sheet in temporary output, outside `assets/`;
review-only files must not enter the generated inventory.

## Editing the level

`level.json` is authoritative. Units are tiles. Platforms specify `left`
(inclusive), `right` (exclusive), surface `row`, exclusive `bottom`, and style.
Solid ground uses full-tile collision. Thin bridges/slabs use generated shallow
static bodies matching the opaque pixel bounds, with those cells excluded from
tile collision. Decoration layers
are independent and cannot collide. Props specify registry IDs, optional
animation and `back`/`world` depth; all pieces of a waterfall start at phase zero.
Emitters specify point positions and distinct deterministic seeds. Their summed
maximum live count is 248, with a global runtime budget of 2048 and at most 16
transient bursts (jumps, landings, explosions) counted at once; at the cap a key
burst (a death, gore, a summon, a shatter or a refusal) takes the oldest
cosmetic burst's slot, whose particles fade uncounted. Each open black hole caps
its particles at 400, so the three that can be open take 1,200.

The map is 430 × 72 tiles with its kill row at 71. `landmarks` are its six
areas, which must tile its columns from 0 to the last; each names its
`ambience` loop and, outdoors, its `backdrop` (the far strip), and takes
`indoor` (a cave: the light dims and the moon goes out, and its open cells
under a roof get cave backs), `night` (the true-black sky), `shoring` (the
columns whose cave backs are the miners' timbers), `horizon` (rows its strips
stand lower than the start's, each times its scroll) and `ridges` and
`woodland` (false where the area's far strip stands without them: the ravine
drops both, the mine and the bridge the woodland). The level's other fields:

| Field | What it places |
| --- | --- |
| `pit` | The ball pit's interior, `left` to `right` and `rim` to `floor`, lined by the pit's tiles |
| `lava` | Each basin, `left` to `right` from its surface `row` down to the terrain: lava props, a static surface body at its skin in physics layer 4 (`LAVA_LAYER`) that only the player's controller meets, a `ContactDamage` touch from 4 px over the skin to the bed (`lava_bed`: 16 fire, a 1,500 push that passes the window, every 0.1 s), a child `ex10_lava_frost_<i>` on the same box whose fire touches only what is frozen and, with `light`, its glow and light |
| `river` | The bridge's river: its surface row, its `rapids` and `rocks` columns, and its depths in the background layer |
| `tunnels` | A 2 × 2 tunnel mouth in the background layer, by `kind` |
| `backs` | Background spans outdoors, a family from a `top` row down to the ground: the ravine's `wet_back` behind the falls and the mine's `cut_back`, with iron on its open faces. A wall's edges follow the outline of all its spans together, so one-column spans give it a jagged top |
| `mists` | A far mist band (`band`, an index into `backdrops.FAR_MISTS`) from its `row` across `left` to `right`, in the decorations layer where no terrain is |
| `spouts` | The channel cells, `[col, row]`, that take `channel_spout`, each with a pour stack under it (01 (k)) |
| `iron` | Markers where a fresh run sets a loose iron block down, its centre |
| `crates`, `kegs` | Each placed pine crate and powder keg (01 (g)), over its `support` platform at column `x`, the box's centre; a `tier` stacks it that many boxes up, each bottom corner over a box of its kind on the tier under it. `placement.prepare` stands each box half a pixel over its support's surface, by its prefab's half height, refuses one with any of its box off its support and lets a solid stair over the support take its place; `level.py` writes a `crate` or `keg` marker at each box's centre, which a fresh run sets down after the iron |
| `pickups` | Each placed health orb by `name`, over its `support` platform at column `x`, its centre `float` tiles above the surface; `placement.prepare` refuses one off its support |
| `tomes` | Each book (01 (l)) by `name` and `kind` (`pact`, `bounding`, `arcane`), placed as an orb is over its `support` at column `x`, sized by its idle frame, and the route platform its double jump starts `from`, which the route test reads; each book's glow counts in the light budget |

`routes` names platform waypoints: `main` runs from the spawn through all six
areas to the bridge's east end, over the pit's chutes and the gap between its
bridge's decks, up the aqueduct's ledges (`aq_ledge_*`), through its first
fallen span onto the channel and over the next two (`channel_mid`,
`channel_east_a`, `channel_east_b`), and down the ravine's jutting rocks
(`rv_ledge_*`) back and forth across the falls. `pit_in_west` and
`pit_in_east` drop down the pit's chutes, `pit_climb` climbs its ten shelves to
the gap and out onto the west deck, `aqueduct_low` takes the aqueduct's floor
over its rubble (`aq_rubble_*`) and the ravine's west steps to the pool, and
`forest_shrine`, `ravine_east` and `cut_upper` are side routes. Routes keep to
fixed platforms, so the route test can run them without the moving decks; the
lifts (up the aqueduct's third span, by the ravine's east ledges, in the
mine) and the aqueduct's ferry are other ways round.
Gameplay geometry is authored, never randomized. Route tests use actual engine input and physics against the
real generated map, searching launch positions without changing gravity or
colliders. Run those tests after any layout change; geometric jump estimates
alone do not prove clearance beneath the solid platforms.

`level.py` emits the embedded Tiled catalog (`firstgid=1`, contiguous local IDs,
zero for empty, no flip bits) and the map's object layers, with the prefabs their
classes name. Edit their source instead of the generated output. The object layers
sit before `collision`, in spawn order, and `World::spawn_map_objects` spawns them
when a run starts; only the objects a save names take the run's next save ID, in
that order (the lava basins, the decks, the dummy, the enemies and the placed
orbs, `D-208`), so a prop or a slab moves none:

| Layer | Objects | Prefab class |
| --- | --- | --- |
| `props` | Every prop, at its tile position | `ex10_prop_<sprite>_<depth>`: its sprite, behind (`back`) or among (`world`) the world's sprites, and its animation |
| `slabs` | One box per thin platform: slab runs that share a row, a depth and a material and touch end to end merge, so a body sunk into a deck meets no join | `ex10_slab_<width>x<height>`: a static body in physics layer 1 (stone); `ex10_wood_<width>x<height>` for the rope bridge's and the scaffolds' wooden decks, in layer 3 |
| `iron` | The mine's iron cladding, merged into the fewest boxes a greedy pass finds (`level.boxes`) | `ex10_iron_<width>x<height>`: a static body in physics layer 2 |
| `hazards` | The lava basins, and the spikes and fireballs `level.json`'s `hazards` places (none today) | `ex10_lava_<n>`, each basin's surface body and its touch to the bed, `hazard.lava` set so cold never douses it, the lake's east pool with its glow; `ex10_spikes`; `ex10_fire_<n>`, each fireball's own: its sine motion on the kit's `oscillator`, starting where the sine stands at time 0. All hurt through the kit's `contact_damage`, the spikes and fireballs 34 a touch, pushing 560 px/s away |
| `platforms` | The moving platforms, and the mine's elevator | `ex10_platform_<n>`, each its own: a kinematic deck on the kit's `path_follower`, ping-ponging between its sine's two ends with a sine easing, half a period a leg, drawn by `lift_deck_<n>`, three deck tiles across its width; deck 4 the elevator (`elevator { rise, speed }`), a cage drawn by `lift_deck_elevator` that the game moves, with the gantry's `lift_sheave` marker for its cable |
| `areas` | `landmarks`, a zone across each one's columns | `ex10_area_<n>`: a sensor body, the kit's `zone` for the player and its `area`: its name, `indoor`, `ambience`, `backdrop`, `night`, `horizon`, `ridges` and `woodland` |
| `emitters` | The ambient emitters | `ex10_ambient_emitter`; each object's `particle_emitter.config` and `particle_emitter.seed` properties set its emitter |
| `markers` | `spawn`, where a fresh run's player stands, an `iron` marker at each loose block's centre, a `crate` or `keg` marker at each placed prop's, and the elevator's `lift_sheave` | None: the game reads them by name |
| `dummies` | The training dummy in the Lantern clearing | `dummy`: each placement's `tags.<n>` properties set its tags (`flesh`, `freezable`, `conductive`) |
| `enemies` | The 26 enemies, after every other object layer but `pickups` and `tomes`, so every earlier object keeps its save ID; a new one goes last, its ID before the orbs' (the save's schema moves theirs and the player's) | Each object's class names its kind's prefab (`wisp`, `crawler`, `beetle`, `bat`, `troll`, `goblin`), its point the sprite's top-left; a `leash` in tiles becomes the `enemy.leash` property, bounding its patrol about its home |
| `pickups` | The eight placed health orbs, after the enemies | `health_orb`, the drops' prefab; the run takes their `lifetime` off |
| `tomes` | The three books, the map's last object layer; no save names a book, so none takes a save ID | `tome_<kind>`, its point the book's top-left |

A parameter that lives in an array (a box's half extents, an oscillator's
amplitude, a path's points) gives its object a prefab of its own; a scalar would
be an object property over a shared prefab. No other runtime paths are used.

Shots meet the walls by these layers (`D-202`): the bullet's `bounce_layers`
are `[2]` and its `pass_layers` `[3]`, so it turns off iron, passes wood and
ends at stone and the tiles (layer 0); the missile banks once off iron. The
enemies' sight casts against the tiles and layers 1 to 3.

Besides `validate_level`'s references, `level.validate_layout` checks the
layout as the generator writes it: the areas tile the columns; the tests' sky
(columns 0–37, rows 0–19) holds no tile or object but the areas' zones; no
92 × 52 window, the 16:9 view at the minimum zoom, holds more than 10 of the
map's lights (`LIGHT_WINDOW`, `LIGHT_BUDGET`), which leaves the player's
lantern, a strike and three missiles under the renderer's 16; each bat hangs
flush under a roof; each fire's sweep stays clear of the terrain; the
pit's shell holds the balls; no iron lies under open air; lava stays in its
basin; and `backs` stay outdoors.

## Runtime and visual checks

Draw order: 01 replaces the engine's sprite base with its backdrop (the sky filling
the viewport, the night areas' true-black sky over it at their share, the moon, and
the strips at their scroll factors with the shake's overhang, each outdoor area's
far strip between the ridges and the woodland, the ravine's mist band, two
translucent cloud layers drifting), then the engine's tilemaps (the background
layer, the decorations with the far mist, the terrain, then the foreground) and
its default sprites by z (props, glows, the toys and the actors). An area's
share is how far the view stands into it within five tiles of each border, so
the far strips, the night sky and the ridges and woodland it drops crossfade
as the view crosses. The strips follow the view up and down but for 30 % of
their parallax (`draw::VERTICAL`) about the start's view, so the deep ravine,
hollow, cut and pit keep their backdrop in view, and each area's `horizon`
lowers them. The toys' and the
elements' effects (flames, shock rings, the vortex, spray wisps, cold pulses) draw
after them, and the cursor last, at 32 screen pixels. The foreground tiles draw
under the sprites.

The kit's platformer controller reads the jump each fixed step, with coyote time
and a jump buffer, and its ground probe decides grounding. The clips and the jump,
air-jump and landing bursts follow its flags, and a fresh run settles onto the
ground silently. A burst's emitter goes once it has drained and its particles are
gone.

Manual acceptance still requires `main`, the pit's chutes and shelves, the lifts and the side routes; deliberate
falls; jump/landing particles; idle/airborne lit frames; animated props; every
spell; a hazard's knockback and shake; and the HUD and the sounds. Inspect resized windows
and both zoom limits for uncovered edges, seams, foreground occlusion and clutter.
Startup smoke alone does not establish these properties. Actual results and
remaining visual checks are listed here; archived implementation plans are historical evidence.

## Gameplay and presentation

Balls never hurt the player, nor does iron but for a block driven into it fast
(see [Iron blocks and crushing](#iron-blocks-and-crushing)). Lava, and any spikes
or moving fire the level's data places, hurt through the kit's `contact_damage`: 34 a touch, pushing 560 pixels/second
away, so the third touch takes Normal's 100 health (Easy has 150, Hard 68). Each
hit plays the hit sound and kicks the camera through the player's `damaged` cues,
knocks it back for 0.18 seconds and opens the kit's 1.1-second window in which
further hits are ignored; the lit wizard shows no hit flash, but a blow reddens
him for 0.12 seconds and spills blood. Falling below the
kill row or wholly out of the world box deals all the health left, window or not.
Game over is the shell's, and its Retry loads the run's last good save. A fireball
bursts a bowling ball with a short particle burst and an expanding ring; both
burst and ring counts are capped at 16. The HUD's bars show the health and the mana.

`hazards` and `moving_platforms` in `level.json` use tile-space center `position`
and `travel` amplitudes, a `period` in seconds, and a `phase` in radians, which the
kit's `oscillator` and `path_follower` take. The kit's controller rides a moving
deck and leaves it on a jump. Static main routes remain available. Route tests run
on the map without hazards or moving platforms; the playthrough test walks `main`
with them through the six areas, jumping the lava. Platform sweeps
are checked for terrain and rider headroom, and a complete physics cycle verifies
support without slipping through the deck.

Scene lights illuminate the normal-mapped terrain and the lit props: lamp posts,
ivy, waystones, gate, oaks, arch, roots, undergrowth, hanging moss and spikes.
Crystals, glowing mushrooms, the waterfall and the fireball stay unlit so they
keep their own glow. The wizard draws lit, with his normal and emissive maps.
Counter-rotating vortex
layers and inward orbiting sparks animate independently of the existing ball
attraction. Camera zoom spans 35–300%.


## Final placement, collision and lighting polish

`placement.py` resolves `support` names in `level.json`. `build_level` makes the
terrain tiles first, since they read only the platforms, and `prepare` stands
props on what those tiles draw (01 (k)): a grounded prop or group starts with
its visible bottom edge on the named surface's row, a hung one (`under`) with
its top tucked `UNDER_TUCK` (6) pixels into its support's underside, a ground
platform's `bottom` or a deck's visible depth, and each then moves by the
smallest 0–12 px (down for a grounded one, up for a hung one) at which it
passes the floating check's own rule below, so it meets the rock families'
ragged tops and undersides and a weathered deck's planks rather than the
cell's edge. Composite `groups` preserve relative piece positions and
validate both feet/trunk against the support width; each takes the pieces
inside its box where `level.json` writes them, and `prepare` refuses a piece
inside two groups' boxes. Wall roots remain attached to cliff faces.

`placement.floating` (01 (k)) lists every piece of the built level that
floats, sinks or states no way it stands, as lines `kind name x y detail`
sorted by x. `build_level` raises on any, naming every line, so `--check`,
the write and the tests fail; `python3 -B tools/generate.py --floating`
builds the level without that stop, prints the lines and a total and exits 1
on any (`no floating scenery` and 0 otherwise). Solid is the drawn terrain layer, a pixel opaque
(alpha 128 or more) in its cell's terrain sprite, slabs and decks included;
liquid is the river, pool, lava and channel-water props. A prop is judged by
its `anchor`, else by its sprite's list in `scenery.py`:

| Class | Rule |
| --- | --- |
| `ground`, a `LEVEL_PROPS` sprite | `MOST` (60 %) of the columns of its bottom two opaque rows have solid within `GAP` (2) px under its bottom row, and none sinks more than `SINK` (6) px into solid |
| `under`, a `HUNG_PROPS` sprite | `MOST` of the columns of its top two opaque rows have solid from `GAP` px over its top row to `UNDER_TUCK` px into it |
| `wall` | an opaque pixel lies within 3 px of solid to its side (the roots) |
| `back` | every opaque pixel lies over a background-layer tile |
| streams (`pour`, `pour_wide`, `pour_foot`, `falls_*`, `lavafall`) | each column of segments is a stack: its top meets a source within `GAP` px (solid, a spill, the gargoyle, the fissure or another column of the stream), each segment's top meets the foot of the one above within `GAP` px, its last segment's foot meets solid or liquid within `GAP` px under most of its columns, and no segment is more than a quarter inside solid |
| exempt | haze, plumes, the far pieces, the water, river, pool and lava props and the spills, which sit in or on what holds them |
| anything else | fails as `unjudged`, so a new piece states how it stands |

A group is judged by its own pieces' composite where they landed, as its
anchor says; a piece inside two groups' boxes where `level.json` writes it,
a group that moves no piece and a non-exempt piece in no group fail. Each
column of `timber_support` pieces ends on solid (it may run on into the
ground behind it). Decorations are judged by their part in a structure: a
pier column (pier top, capital, shafts) runs without a hole to a
`pier_base` over terrain or a `pier_cutwater` over the river; a springing
has a capital or pier top beside it, an arch top its springing under it and
a keystone both arch tops beside it, a gap strip's pieces (`arcade_gap_*`)
taking the parts they replace; a parapet stands as a `ground` prop does; a
background structure over terrain (the cave mouth) fills its bottom row to
its tile's foot; any other decoration fails as `unjudged`.

`SLAB_COLLIDERS` and `DECK_DEPTH` derive from the art's opaque bounds. Current
slabs, bridges and moving decks are 23 pixels deep, rather than the old 64-pixel
tile collision. The runtime and route tests spawn the same shallow bodies;
solid terrain retains native tile collision. Keep platform art and generated
collision together when changing silhouettes.

Press **L** to toggle the staff light, the halo and point light at the staff's orb, which burns mana while lit (01 (e)). Per-frame
anchors on the orb come from the wizard's rig, the mirrored frames' too; both
effects follow facing and animation. A fresh run starts with the
staff light off; L or the pad's North lights it, and a loaded run keeps its save's.
Fireball hazards, which the level's data may place (none today), have a brighter
inner halo and a stronger native light. They face their horizontal travel
(vertical-only movers face the player). Each leaks rising flame wisps from its
seeded `fire_trail` emitter (32 live) and molten drips from a `fireball_drips`
emitter anchored 12 pixels below its center (16 live), 48 particles a fireball.
All emitters remain subject to the global 2048-particle budget.


## Midnight presentation and double jump

The kit's controller allows one air jump after takeoff (or walking off an edge):
release and press Space again to use it; holding Space cannot spend it. It plays the
tuck-and-burst clip until the ascent ends, then the fall clip, and emits a short
18-spark amber burst at the feet with a whoosh (`ex10_air_jump_sfx`). Landing
gives it back. The routes still pass with single jumps.

The controller never pushes dynamic bodies, so a jump press shoves them itself
(`player_unstick`, 01 (f)): bodies over the wizard's head and shoulders fly up
and away at 1,600 pixels/second over their mass, those beside it at 600, and a
jump they blocked launches again the next step at the jump's own speed,
spending no air jump.

The entry clearing is 16 tiles wide. Its first steps, optional overhead platforms,
and lift sit farther from spawn, with fewer foreground props in the launch area.
Support anchors move with the revised layout.

Glows (lamp, fireball, crystal, staff light and moon halos, plus the black
hole's halo) draw through the example's `soft_glow.wgsl` material rather than a
stepped texture. It computes an analytic radial falloff over the quad and
dithers alpha by at most one 8-bit step, so large glows have no rings or banding.

- `ex10_soft_halo` is the wide, faint halo: falloff 1.8, peak alpha 0.1.
- `ex10_soft_flame` is the hot inner glow: falloff 2.2, core boost 0.35, peak alpha 0.6.

Both materials share the shader, and their uniforms live in `generate.py`'s
`MATERIALS`. Without a material registry (headless tests), glows fall back to
the `halo`/`flame_glow` gradients. Those gradients are dithered and use `linear`
filtering, so glow particles are soft too. A plain sprite never merges into a
material batch. The shader's WGSL is Naga-validated by a platformer test.

Stock `GodRays` aimed at the moon was evaluated and rejected. Its full-screen
radial blur turns the moon into a glaring sun and casts a blocky yellow haze,
which fights the restrained night look; the stack stays Bloom plus Vignette.
Stock `Fog` fades toward the top of the screen in screen space, which is not
ground mist. The engine packs at most 16 lights total, keeping directionals first and then
the point lights nearest the camera view; every light inside the view ties, and ties keep
query order. The map holds 10 lights in any window at the widest zoom (`LIGHT_BUDGET`), so
the wizard's staff light stays; past that, with more missiles, flares or summoned wisps, the
staff light can drop out ([known issues](../../../docs/known-issues.md)). That selection lives
in the engine.

Wind motes are fireflies: a soft dot that blinks yellow-green as it wanders.

The palette uses cool night masonry, olive turf and foliage, weathered timber,
teal water and cyan crystal, against layered blue ridges with a ruined keep,
curtain wall and spire (two windows still lit). A separate circular pixel moon
and soft halo keep their aspect ratio on resize;
clouds and landscape can pass in front. The example selects the existing stock
Bloom and Vignette post passes at restrained strengths (0.22 and 0.18), alongside
normal/emissive lighting and a weak neutral moon fill; the shell's Bloom option
takes the Bloom out while it is off. No engine or shader-source
changes are required. The HUD's bars read the health and the mana every frame and
keep their screen size across zoom and camera movement.

## Small balls, the ball pit and particles

Hold `cast` on slot 9 for small balls at 156.25 a second, set down at the cursor.
Their collider radius is 7.5 pixels and render diameter is 16 pixels.

The two balls are deliberately different objects, each drawn at its render size
and stored as 2×2 or 4×4 pixel blocks so that nearest sampling stays exact:

- **Bowling balls** (`bowling_ball*`, 32 pixels, slot 8): dark resin with finger
  holes and an iron sheen, tossed from the hand toward the aim; they land low and
  roll, keep their authored colours and carry no `BallHue`. The bronze rune orbs
  (`ball*`) they replaced are still drawn, unused.
- **Small balls** (`ball_small*`, 16 pixels, slot 9): neutral-grey glass with a
  thin rim, a fixed glint and a turning cat's-eye ribbon. Only these carry
  `BallHue`, so only they cycle through the rainbow; burning and charring tint the
  same greys.

The ball pit is dug into the Lantern clearing east of the start (`level.json`'s
`pit`). Its clear interior spans columns 48–72 and rows 36–68: **24 × 32 tiles,
1,536 × 2,048 pixels**, with an open chute at each end (columns 48–51 and 69–72)
and the rope bridge in two decks over its middle with a four-tile gap between
them. `pit_in_west` and `pit_in_east` drop down the chutes, and `pit_climb`
climbs ten shelves zig-zagging up the middle to the gap. `validate_layout` holds
the interior to at least 3.1 Mpx² (BALL_CAP's 12,000 small balls fill it to about
its rim) and its shell to four tiles. The kill plane lies below the floor. No
hazards occupy the pit. The physics
regression runs 2,048 mixed-size balls for three seconds, including fast wall
impacts, and checks that their centers remain inside the bucket. Capacity is
finite; keep the pile below the open rim when stress-testing.

`SMALL_BALL_IMPACT_SPEED` is 420 pixels/second of pre-resolution relative closing
speed along the contact normal. Faster small-ball contacts emit 18 rainbow sparks,
with hues distributed by launch direction. Tangential/separating contacts and
slower pile motion stay quiet. A 0.12-second cooldown per ball, at most four bursts
per step, the shared 16-transient-emitter cap and global 2,048-particle budget
bound the effect. Finished transient emitters use the existing cleanup.

Black-hole sparks start in a 135–175-pixel annulus, spiral inward with increasing
tangential speed, warm from blue/violet to amber, and disappear at the core.
Each hole's emitters hold 400: 300 sparks and 100 dust. Fire trails use soft flame bodies with
a cream/yellow/orange/red ramp, while torch sparks use the same warm ramp with
smaller sizes and longer lifetimes. Both accelerate upward with varied speeds,
sizes, lifetimes and rotation. The fire-destruction explosion remains a separate
outward burst.

Small balls ignite on contact with moving fire hazards, where the data places any, and spread fire to other
small balls they touch, including settled piles that emit no new physics contacts.
Each ball burns for 10 seconds from its first ignition; contact never resets the
timer. Burned-out balls remain as dark, nonflammable physics bodies.

A burning pile reads as coals under a flickering crest. Every burning ball takes
an ember tint pulsing between deep red and orange at its own rate, darkening to
char over its last two seconds. Only balls with nothing resting on them
(the elements' `flame_exposure`: no ball directly above, nor one above on each side)
draw a flame tongue and a soft glow, so a slope's surface burns too; buried balls
only glow. Flames on every ball of a large pile stacked into one opaque orange
sheet with diagonal banding, which is what this replaced. Each tongue is one of eight
frames drawn on a 32-pixel grid in 2×2 blocks and drawn at 32 pixels, so it
samples exactly; its frame offset, rate (10–18 frames/second), mirror and tint
come from a hash of the ball's id, and a new fire grows in over 0.2 seconds.
Tints multiply the glass in linear light, hence their low green and blue.
Up to 64 surface balls share 128 emitters: each pair (flame wisps and sparks)
stays on one ball for half a second and the pairs hop in turn. Each pair emits
20 wisps and 8 sparks per second with live caps of 12 and 6, so a full pool
stays near half the global 2,048-particle budget and blasts over a burning pit
still get theirs. The pool shrinks as balls burn out or despawn, and remaining
particles finish their short lifetimes.
Burning small balls do not hurt the player or light a bowling ball.

Startup smoke and CPU tests do not verify sprite readability at 16 pixels, the
pit's visual fit during camera travel, or particle appearance and density in
motion. Review those by running the game from its folder (`cargo run`).

## Spells and the HUD

The spells are the player's kit abilities (`content/data/abilities/*.json`, listed
in `content/prefabs/player.json`): each names its mana cost, cooldown and charges,
targets the aim or the player, and grants `casting` (`content/data/effects/casting.json`),
which pauses `mana_regen` for a second. A held spell's cost is one fixed step's, its
cost a second over 60. The mana stats are `content/data/stats/game.json`; the
`casting` tag is in `content/data/tags/game.json`. The values, the slots and the
controls are in [the example's README](../README.md).

The HUD draws the icon tiles (`effects.py`, 36 pixels, nearest filtering) at the
largest whole number of screen pixels per art pixel the UI's scale allows. They
are one family (01 (h), `icon_tile`): each 32-pixel subject lit from the top
left with a rim of its element's light on its far edges, over a dark backing
tinted by the element with a glow behind the subject, in a two-pixel bevelled
frame in the element's colour (`ICON_ELEMENTS`): fire for the missile and the
flamethrower, cold for the ice spray, gravity's purple for the push and the hole,
shock's violet for lightning, bone for the physical toys and the gun, and mana's
blue for each summon type, standing on its ring recoloured from indigo. Slot 0
shows the summon's selected type, the dummy's tile first. The palette's mana
blues (`И Й З Ю`, 01 (h)) draw the summons' frames and ring, the mana orb and the
mana bar's liquid; its violet ones (`Δ Λ Σ`) the black hole's subject and the
lightning's; its gravity ramp (`Ж Ф Ц Ш Щ`, 01 (f)), deep magenta toward indigo,
the force push's wave's fringe and the black hole's collapse ring and
under-layers. The bars fill with `hud_liquid_health` and `hud_liquid_mana`, 128 × 10
strips tileable across that the HUD scrolls by the game clock, and the gun's rounds
are `hud_bullet` cartridges, an empty `hud_bullet_spent` case for each one fired.

A refused cast spends nothing and says why, once a press: `ex10_no_mana_sfx`
for want of mana, `ex10_not_ready_sfx` for a cooldown or no charge left, and a
`fizzle` burst with `ex10_fizzle_sfx` at the cursor where nothing fits (a toy at
its cap, a summon or an iron block that does not fit, no cursor). Choosing
another slot or summon type ticks (`ex10_spell_switch_sfx`).

## Fire missile, extinguishing and sound effects

Slot 1 casts a fire missile toward the cursor through the kit's `spawn` payload: one
a press, at most six alive, 0.25 seconds apart, for 8 mana. It leaves 24 pixels out
from the wizard's centre at 900 pixels/second on the kit's `linear_motion`, falls
at 360 pixels/second² (a tenth of world gravity), is pulled by black holes like a
unit-mass body, and fizzles at the kit's projectile range of 1,890 pixels (2.1
seconds). It is not a physics body; only its blast pushes.

It explodes on the first solid it touches: any collider except the player's
(balls of either size, slab decks, lifts, iron) or a solid collision tile.
The blast spawns 48 flame bodies (`fireball_blast`), 64 fast hot fragments
(`blast_embers`), 24 outward dust puffs (`blast_dust`) and 20 lingering smoke
puffs (`blast_smoke`): 156 particles, subject to the shared emitter and particle
caps. A white-hot flash fades over 0.18 seconds while two shock fronts expand
rapidly toward the blast's push radius. It pushes every dynamic body within
288 pixels straight away from it: 2,500 pixels/second for a
unit-mass body (a ball) at the centre, falling linearly to nothing at the edge and
divided by mass, so an iron block barely moves. Every live enemy within it is
knocked back through the kit's `knock_back`, at the same falloff times its
kind's black-hole pull (`toys::radial_push`); the troll, whose knockback resists
the push whole, stands. The wizard, a kinematic body, stays put. The force push,
slot 5, is a wave instead (01 (j), `toys::wave`): a front from the wizard's hand
toward the cursor at 1,620 pixels/second for 0.3 s, its sector widening from 15°
to 30°, that throws each body and enemy it meets once along the aim, half as hard
at its 486-pixel reach (§3.7a: its throws half again as hard). It draws as `push_wave`, a crescent of bent air turned to
the aim and scaled across to the front's width (sampled linearly), and raises a
`push_dust` puff and a `push_grit` burst where it scrapes a floor. The blast ignites every small ball within 72 pixels through
`elements::ignite`, so spent balls stay spent and bowling balls never burn. It does
not hurt the player.
The camera shakes with trauma 0.68 for a blast within a quarter view width of
the view's centre, less with distance, and
not at all a view width and a half away.
In flight it draws its own clip, `fire_missile` (01 (h), `actors.missile_frame`),
at 40 pixels, turned along its velocity and mirrored flying left: a rocket of fire
with a white-hot nose cone, a body lit along its top with a glowing band, two swept
fins, a dark nozzle and a licking plume shedding embers, ten self-lit 64-pixel
frames of 60 ms. It has a soft flame glow and a dense comet tail as its child, and
carries one warm point light (192-pixel radius). The light fades over 0.3 seconds
where the missile burns out, or flares and fades over 0.5 seconds where it
explodes, flaring to four times its flight intensity with twice the light radius.
Point lights warm the lit terrain, props and iron around the explosion.

A black hole, slot 6, opens at the cursor for 6 seconds; at most three stay open,
and a fourth collapses the oldest. It opens over 0.3 seconds (01 (j)), all
presentation, so it pulls from its first step: its layers grow in from 0.08 of
their size on a cubic ease-out, a ring of its own `photon_ring` art closes on it
from 432 pixels to 24, and a `hole_open` burst of 72 sparks set on a ring 230
pixels out rushes in at 700 pixels/second, with `ex10_black_hole_sfx` tearing it
open. A bowling ball takes 0.625 of its pull, so it lifts within about 200
pixels of the core, while iron, at 0.2, stays down. A flyer in its reach is held
on its own mover rather than flung through: steered toward a ring 40 pixels about
the core at the hole's pull, capped at its speed, eased within 48 pixels of the
ring and circling there at 0.55 of its speed, blended with its own steering by
the pull over its speed, and given its own direction back after the movers. Over
its last 0.5 seconds a hole shrinks to
nothing and closes with a small gravity ring and `ex10_black_hole_collapse_sfx`.
While any is open the holes hum on one shared voice, `ex10_black_hole_hum_sfx`,
louder as the nearest comes near, silent past 900 pixels and under pause. A hole
sweeps up the small balls whose centre comes within 24 pixels of its own,
nearest first, at most 12 a hole a step, every eighth in a `poof`. It puts out
every burning small ball inside its 384-pixel pull radius in the same fixed step,
leaving it spent exactly as after a normal burnout. At most four sampled balls a
step puff an ember-to-steam `extinguish` burst, and the sizzle plays at most every
0.15 seconds.

The black hole draws its soft halo, a hot accretion disk and two counter-rotating
vortex layers, then 48 infalling streaks turned along their spiral, the photon
ring and the horizon, in 01's sprite base between the tiles and the sprites at
the tiles' far plane (`z_norm` 1.0), so actors, props, toys and particles draw
over it; the halo, the disk and the vortex layers take the gravity ramp. Its five sprites (`vortex`,
`vortex_core`, `accretion_disk`, `photon_ring`, `infall_streak`) are white alpha
fields on 96-pixel canvases, an exception to the 64 × 64 rule as the troll's
128-pixel frames are. They are
tinted at extraction, fade to transparent before their edges, and use `linear`
filtering. Its 400-particle budget splits into 300 sparks and 100 dark-gas
`black_hole_dust` particles; the dust emitter is anchored to the hole, and both
follow the same accretion spiral.

Sound effects are authored in `sfx.py`, not recorded. Each sound is a short
function built from integer xorshift noise, one-pole filters, sine oscillators
and exponential envelopes, normalized to a −2 dBFS peak with short fades (or a
crossfaded seam for the sustained ice spray), and
written as 44.1 kHz mono 16-bit PCM WAV with the standard-library `wave` module.
Regeneration is byte-identical, so `--check` covers the WAVs like any other
output. To add a sound: write its function, add it to `build_sounds()`, register
its ID, path and volume in `sounds.json`, and regenerate. Generation rejects a
`sounds.json` path that is neither synthesized nor listed in
`HAND_AUTHORED_SOUNDS` (none since 01 (j)). The one-shot sounds play
through the kit's cues: 01 sends a `Sounded` game event, and
`content/data/cue_rules/sounds.json` plays its sound, throttled by `min_interval`.
The sprays' loops, the small balls' pour and the holes' hum are voices 01 holds;
a spray's loop and the pour stop over 0.08 seconds.

| ID | File | Event |
| --- | --- | --- |
| `ex10_fireball_cast_sfx` | `fireball_cast.wav` | A missile is cast |
| `ex10_fireball_blast_sfx` | `fireball_blast.wav` | A missile explodes, or a bowling ball bursts in fire |
| `ex10_extinguish_sfx` | `extinguish.wav` | A black hole puts out burning balls |
| `ex10_player_hit_sfx` | `player_hit.wav` | The player takes a blow or the killing hit, through the `player_hurt` rule (never a burn or bleed tick) |
| `ex10_staff_gutter_sfx` | `staff_gutter.wav` | The staff light gutters out for want of mana: a starved flame's sputter, slowing, with a few last pops |
| `ex10_iron_crush_sfx` | `iron_crush.wav` | Iron crushes small balls or hurts the player |
| `ex10_ice_beam_sfx` | `ice_beam.wav` | The ice spray starts |
| `ex10_ice_spray_sfx` | `ice_spray.wav` | Cold rushing wind loops while the ice spray is held |
| `ex10_ice_freeze_sfx` | `ice_freeze.wav` | Iron or a creature finishes freezing, with crackles and glassy chimes |
| `ex10_ice_end_sfx` | `ice_end.wav` | A soft pressure release when the ice spray is let go |
| `ex10_ice_shatter_sfx` | `ice_shatter.wav` | Brittle iron fractures under heat or a fast iron impact |
| `ex10_flame_start_sfx` | `flame_start.wav` | The flamethrower catches: a low whoomph opening into a hiss |
| `ex10_flame_spray_sfx` | `flame_spray.wav` | Its roar loops while it is held |
| `ex10_force_push_sfx` | `force_push.wav` | The force push's wave (01 (j)): a soft push-off and a whump of bent air, a rush flying away and falling in pitch, a low warped tone bowing under it and a scuff of grit |
| `ex10_toss_sfx` | `toss.wav` | A bowling ball leaves the hand |
| `ex10_place_sfx` | `place.wav` | An iron block set down |
| `ex10_black_hole_sfx` | `black_hole_open.wav` | A black hole tearing open (01 (j)): an inward suck rising over its 0.3 s opening, a deep whomp falling into its drone, a rumble and a faint glassy shimmer |
| `ui_move`, `ui_select`, `ui_back` | `ui_*.wav` | The shell's menu sounds, by the IDs it plays: a plucked tick, two bells rising, two falling |
| `game_over` | `game_over.wav` | Game over's sting: a falling bell line, then a low toll under a plucked D minor chord |
| `music_title`, `music_over` | `music_*.wav` | The shell's title loop, a twelve-bar lullaby in D minor, and game over's, a four-bar dirge |
| `music_explore`, `music_pause` | `music_*.wav` | A run's exploration loop, and pause's over the quieted run |
| `ex10_gun_shot_sfx` | `gun_shot.wav` | A shot: a crack over a deep punch and a booming body, the slide's tick |
| `ex10_gun_reload_sfx` | `gun_reload.wav` | The magazine whole again: a slide, two clicks and an iron ring |
| `ex10_ricochet_sfx` | `ricochet.wav` | A bullet turns off iron: a ping that whines away |
| `ex10_thunder_sfx` | `thunder.wav` | A lightning strike: a splitting crack and a rolling rumble |
| `ex10_crackle_sfx` | `crackle.wav` | A conduction jump: a short electric buzz |
| `ex10_steam_burst_sfx` | `steam_burst.wav` | Cold on fire, or a fire hazard put out: a wet whoosh into a hiss |
| `ex10_frost_blast_sfx` | `frost_blast.wav` | Thermal shock: fracturing ice, a cold boom and falling shards |
| `ex10_lava_chill_sfx` | `lava_chill.wav` | Lava's freeze starting under the ice spray (01 (k)): a sizzle building with cold glints and creaks, at 0.45 |
| `ex10_lava_crack_sfx` | `lava_crack.wav` | A crust cracking at each of its three stages: a snap, a low split and a fizz, at 0.5 |
| `ex10_lava_melt_sfx` | `lava_melt.wav` | A crust melting: a glug, two bubbles breaking and spatter, at 0.35 |
| `ex10_dummy_hit_sfx` | `dummy_hit.wav` | The dummy takes damage, through its `damaged` cue: a sackcloth thump |
| `ex10_summon_sfx` | `summon.wav` | A dummy summoned: rising bells over a soft swell |
| `ex10_splinter_sfx` | `splinter.wav` | A bullet through a wooden deck, through the `splintered` rule: a sharp crack, a hollow knock and splinters rattling away |
| `ex10_black_hole_hum_sfx` | `black_hole_hum.wav` | A 2-second loop while a hole is open: a deep drone wavering about 46 Hz with a fifth over it, a dark whirl of air and a slow pulse |
| `ex10_black_hole_collapse_sfx` | `black_hole_collapse.wav` | A hole closing: an inward rush rising to a snap, then an implosion under a rumble |
| `ex10_iron_thud_sfx` | `iron_thud.wav` | A block landing hard: a deep thud and knock, a dull clang ringing out, grit settling |
| `ex10_fizzle_sfx` | `fizzle.wav` | A cast with no room: a sputtering hiss falling away, pops, a dull sinking tone |
| `ex10_thunder_snap_sfx` | `thunder_snap.wav` | A bolt landing, under the thunder: a bright snap, a zip falling in pitch, a buzzing sizzle |
| `ex10_ice_crack_sfx` | `ice_crack.wav` | A thaw: a creak under strain, a sharp crack with a glassy ring, meltwater drips |
| `ex10_ball_thud_sfx` | `ball_thud.wav` | A bowling ball landing hard: a low thump under a hollow resin thock and a rattle |
| `ex10_gun_click_sfx` | `gun_click.wav` | The trigger with no round or during a reload: the hammer's light click |
| `ex10_gun_rack_sfx` | `gun_rack.wav` | A reload begun on R: the magazine let go and the slide racked, two clacks and a scrape |
| `ex10_flame_end_sfx` | `flame_end.wav` | The flamethrower let go: the roar dropping into a soft puff, last crackles |
| `ex10_air_jump_sfx` | `air_jump.wav` | The air jump: the cloak snapping open in an upward whoosh, a faint chime |
| `ex10_no_mana_sfx` | `no_mana.wav` | A cast refused for mana: a hollow knock and a tone sinking away |
| `ex10_spell_switch_sfx` | `spell_switch.wav` | Another slot or summon type: a soft click and a short glassy tick |
| `ex10_not_ready_sfx` | `not_ready.wav` | A cast refused on a cooldown or with no charge: two muted low ticks |
| `ex10_orb_pop_sfx` | `orb_pop.wav` | An orb a kill drops: a bubbly rising pop and three small bells |
| `ex10_small_balls_sfx` | `small_balls.wav` | A 2-second loop while small balls pour: a dense patter of glass clicks over a hiss and the pile's roll |
| `ex10_enemy_death_sfx` | `enemy_death.wav` | An enemy killed, through its prefab's `killed` cue with a light shake: a last thwack and crunch, a falling groan, the body's fall |
| `ex10_dummy_thump_sfx` | `dummy_thump.wav` | The dummy knocked out, through its `killed` cue: a heavy sackcloth thump, straw bursting and settling |
| `ex10_crate_ignite_sfx` | `crate_ignite.wav` | A crate catching: a dry whoomph drawing up over a soft thump, the wood's first crackles |
| `ex10_fire_crackle_sfx` | `fire_crackle.wav` | A 2-second loop while a crate burns, one voice by the nearest, silent past 600 pixels: a low roar under a wood fire's crackle and the pops of its sap |
| `ex10_crate_collapse_sfx` | `crate_collapse.wav` | A burnt crate falling in: a creak, a splintering crunch, boards knocking, a gust of sparks |
| `ex10_fuse_hiss_sfx` | `fuse_hiss.wav` | A keg's or a bomb's fuse lit, as long as its 1.5 seconds: a sputtering hiss rising in pitch |
| `ex10_keg_boom_sfx` | `keg_boom.wav` | A keg or a bomb blowing, at most every 0.06 seconds: a sharp crack over a deep boom, a rolling rumble, splinters and grit raining down |
| `ex10_rock_throw_sfx` | `rock_throw.wav` | A goblin's throw: a short, sharp whoosh |
| `ex10_rock_hit_sfx` | `rock_hit.wav` | A goblin's rock landing hard, or crumbling past the cap: a stony clack over a hollow knock, grit skittering away |
| `ex10_goblin_cackle_sfx` | `goblin_cackle.wav` | A goblin's alert: a high, nasal, quick cackle of five syllables falling in pitch |
| `ex10_goblin_stab_sfx` | `goblin_stab.wav` | A goblin's stab: a quick, thin swish of its knife and a bright tick of steel |
| `ex10_troll_swipe_sfx` | `troll_swipe.wav` | A troll's swipe: a heavy whoosh of air as its arm rakes through |
| `ex10_troll_grunt_sfx` | `troll_grunt.wav` | A troll winding up its slam: a deep, short, effortful grunt with a rasp |
| `ambience_clearing` | `ambience_clearing.wav` | The clearing at night: crickets, a breeze in the oaks, an owl |
| `ambience_aqueduct` | `ambience_aqueduct.wav` | The channel's flow swelling once a loop, the pours' patter on the floor, a spill's heavier plop now and then, water trickling and dripping from its leaks |
| `ambience_ravine` | `ambience_ravine.wav` | The falls' roar with slow surges and a hiss of mist |
| `ambience_hollow` | `ambience_hollow.wav` | A low rumble under the basalt and the crackle of embers |
| `ambience_mine` | `ambience_mine.wav` | Wind over the spoil, a creaking timber and a distant clank of iron |
| `ambience_bridge` | `ambience_bridge.wav` | The river under the arches, wind across the bridge, the ropes' creak |

The music loops are 16-bit mono at 22,050 Hz, on the music bus. Each renders a
bar past its end and folds that tail onto its start, so it repeats at its bar
line; generation fails a loop whose length is not its bars exactly or whose seam
jumps further than any step inside it (`sfx.seam_problem`).

The six ambience loops (`sfx.AMBIENCE`) are 6 seconds each at 22,050 Hz, on the
effects bus at volumes 0.15 to 0.2, half their first levels. `sfx.ambience_loop` renders a quarter second
past the end and crossfades it into the head, and `LOOPS` holds each to its
132,300 samples under the same seam check. 01 starts all six silent; the current
area's rises over 1.5 seconds and the others fall, and all are silent under pause
and game over.

## Ice spray and thermal shattering

Hold `cast` on slot 3 to channel the ice spray toward the cursor, for 18 mana a
second; slot 2's flamethrower is the same spray in fire, a shorter reach, lighting
the small balls it meets. The 28-degree cone reaches 582 pixels, 9.1 tiles (01 (h):
30 % further than 01 (f)'s), and the flamethrower's 378, 5.9 tiles (§3.7a: 30 %
further again, its puffs and sparks flying 30 % faster to fill it); seven
rays stop on colliders and solid tiles, so exposed iron at the sides freezes too.
The nozzle follows the interpolated caster and current aim every drawn frame.
Its visuals and sound start on press even before the next fixed physics step;
freezing exposure still uses only simulation time.
Each spray draws as animated, self-lit sprites at z 11 (01 (h), `toys/spray_look.rs`,
`effects.SPRAY_CLIPS`): a muzzle flare where the stream leaves the staff
(`flame_muzzle`, `frost_muzzle`) with a point light at the flame, a stream of puffs
(`flame_puff`, `frost_puff`), the fire's every 12 ms at 900 pixels/second, the ice's
every 18 ms at 1,600, each along a line within the cone that stops where it meets
something, growing and streaking as it flies; a looping splash where the cone
meets something (`flame_splash`, `frost_splash`); and for the ice a bright core
beam tiled down the cone (`frost_core`, seamless across, drawn 1.5 times tall). A
release takes the flare, the light, the splash and the core at once, and the
puffs in flight play out. The particles are accents: crystals and a light mist,
or embers and smoke rising off the flame's tip, and the contact's splashes.
Newborn spray particles rotate into their birth aim, draw at z 11, over the
enemies, the dummy and the wizard, from their second frame, and expire before
blocking surfaces within their own spray's reach. One channel owns three
continuous emitters, the ice spray's capped at 228 particles combined under the
global 2,048 budget and the flamethrower's at 202. Every second damage pulse, from
the first, each live target the cone damages bursts at its contact, `frost_hit`
(12 crystals) or `flame_hit` (14 embers), a cosmetic burst the burst cap may drop;
a surface keeps the splash alone. Completion and shatter bursts share the existing
16-transient-emitter cap.
Each channel varies its seeded particles. Contact splashes prefer the closest
exposed iron surface across the cone, including contact away from the centerline.
Mist remains translucent enough to read the target's frost buildup.

Iron takes 1.5 game seconds of exposure to freeze, independent of draw frame rate.
Partial frost fades at 0.15 of a coating per second without exposure. A completed
freeze persists. Only fully frozen iron becomes brittle and gains reduced
friction. The cold rush starts once per channel, a seamless hiss with crystalline
resonances loops while held, and a crackling chime marks each completed freeze.
Crystals burst on the contacted face, with a brief blue-white glow and broken icy
ring marking the completed coating. Shattering instead sends sharp faceted shards
spinning outward, a faster expanding cold ring, and a distinct glass-fracture sound.
Completion and fracture cues have separate shared 0.1-second sound intervals;
at most 16 cold pulses live at once, lasting 0.24 seconds for freeze completion
and 0.32 seconds for fracture.
Release, death or loss of the caster stops the loop over 0.08 seconds and removes
its emitters, including on frames with no fixed step; existing particles finish
fading. Normal release adds a quiet 0.18-second pressure tail, the flamethrower's
a dying puff (`ex10_flame_end_sfx`); death and caster loss stop quietly.

Iron under the spray gradually gains a pale ice shell, white frost rims, crystalline facets,
branching cracks and hanging icicles. Four unlit `iron_frost_big_*` overlays keep
the coating visible at night over the lit iron plates. The block stays frozen
and brittle. Supported frozen
iron loses horizontal speed at 120 pixels/second², independently of normal friction.
A fireball's thermal blast shatters every frozen iron surface within 72 pixels,
including contact at a block's edge. Each full block becomes exactly 16 physical
30-pixel scrap blocks in a 4×4 grid. Each scrap has 1/16 of a full block's mass
(25) and sliding friction (93.75 pixels/second²), reduced to 7.5 pixels/second²
when frozen, so the sixteen pieces preserve the block's total mass.
They inherit the block's velocity and launch outward at varied speeds of
800–1,200 pixels/second, with a small upward lift. Surface explosions also throw
the fragments away from the contacted face, on top of the normal blast push.
Each piece starts at a different angle and briefly tumbles in either direction,
slowing its visual spin over about a second on the interpolated scene clock.
The matching frost rotates with the metal; physics retains square colliders.
Ice chips accompany
the break. Scraps have four dedicated lit `iron_scrap_*` sprites, with chipped
corners, rough fracture planes, torn bright edges, cracks and rust instead of
the big block's regular riveted plates. They are authored at their 32-pixel
runtime size and have silhouette-matched unlit `iron_scrap_frost_*` coatings.
Scraps receive the same black-hole acceleration as balls: five times a full
block's response, so holes readily gather loose debris. Their mass and sliding
friction stay at 1/16 of full iron.
They collide normally, ride lifts,
and are removed when they escape the world or the run restarts. The thermal shock
leaves them warm; refreezing a scrap makes the next thermal contact destroy it
into particle chips. Warm iron survives fireball blasts.

A full iron block also shatters frozen iron on a head-on impact at 600
pixels/second or faster. Both the striker's speed into the contact and the
relative closing speed must reach that threshold before physics resolution;
resting, separating, glancing and equally moving contacts stay intact. The
striker keeps 90% of its incoming velocity, and the frozen block becomes the same
16 light scraps and ice chips as under thermal shock. Frozen scraps break into
chips under these impacts too. The existing crunch sound cooldown applies.

CPU tests cover held channels, gradual freezing and partial thawing, cone coverage,
occlusion, particle direction and caps, loop start/stop, exposure at 30/60/144 Hz,
pause, immediate feedback between simulation steps, side-contact splashes,
grouped sound cues, cold-pulse rendering and cleanup, friction, thermal surface
contact, scrap count and mass, directional launches and floor-level dispersal
through real physics, dedicated scrap sprites, tumble and matching frost rotation,
stronger black-hole pull for both warm and frozen scraps, and cleanup.
Visual playtest at 1920×1080 checked gradual frost, the completed-freeze pulse,
spray translucency, thermal fracture with cold shards, and control/heart spacing.
The final shatter pass was also checked at 1920×1080: a surface fireball blast
throws the broken-metal chunks in a visible fan, with varied angles and a brief tumble.
The audible mix and resized-window presentation still need human review.

## Elements, weapons and the dummy

01 (b)'s art follows the house style, and `--check` covers it like the rest. The
palette gains `Θ` and `ϑ` (maroon, the bleed's), `ϖ` (the crit yellow, which the
damage numbers' crits use), `ϰ`, `ϱ` and `ς` (translucent ice) and `ϕ` (a
near-white violet). Every new sprite is unlit, drawn over the lit world:

- **Statuses**, generic per size class, never drawn per target: small 32, medium 64
  and large 128 pixels, by the target's sprite. `status_ice_<class>` is an ice
  block over the frame's box, `status_crackle_<class>_<0|1>` a two-frame violet
  crackle, and `status_halo_<class>` the stun's ring above the head. Chilled shows
  a frost tint and `chill_flakes`, bleeding `bleed_drips`, burning today's `fire_*`
  flames. 01 (f) takes the frost toward frozen's tint as the cold bucket fills,
  sets the flames at 0.55 of the side on three spots over a warm halo with
  `burn_embers` and a flickering ember tint, and gives each cube a cracked twin,
  `status_ice_<name>_cracked`, drawn by the cube's own code and held to its box
  by `check_ice_fits`, which a freeze's last 0.75 seconds flickers to at 8 Hz.
  01 (j) frosts a creature that freezes from its first point of cold in four
  stages, `frost_<kind>_<stage>` fitted to the wisp's, beetle's, bat's,
  troll's, goblin's and dummy's hurtboxes as their cubes are and
  `frost_<class>_<stage>` over a class's status box, held within 6 pixels of
  its box by `check_frost_fits`: white rime (`Ψ φ x ϱ`, a sparse `u`) from the
  feet (1) up the legs at the chill (2) and the body (3) to a crust (4), its
  sides fading toward its top. Its tint goes from white toward a whiter
  `FROST_TINT` before the chill and on toward a pale `CHILL_DEEP` from it, and
  its flakes are `chill_flakes_<stage>`, a pulse every 0.45 seconds of one,
  one, two and three white crystals; the wizard's flat chill keeps
  `chill_flakes`, on the same white ramp.
- **The lightning bolt**: eight 24 × 64 segments, `bolt_0` to `bolt_7`, each entering
  at its top centre and leaving `BOLT_EXITS` pixels aside at its bottom, so a strike
  stacks five to eight of them from the sky down, each starting where the last left;
  a violet haze, a glow, a light core and a white spine, every third forking a twig.
- **The ice shell** over a frozen ball, `ice_shell`.
- **The gun**: a three-frame 16-pixel muzzle flash (`muzzle_0` to `muzzle_2`) and a
  32 × 3 tracer along +x, a dim tail to a white-hot head, whose head the lightning's
  arcs and veil also take their white from.
- **The training dummy** (`actors.py`, `dummy`): a straw target on a post, 64 × 64,
  its lowest pixel on row 60 as a 64 px walker's is, drawn unlit with no normal or
  emissive map, so the kit's hit flash shows through `damage_flash`.

Nine particle configs join, over today's sprites: `muzzle_flash` and `ricochet`
(the gun), `steam_burst`, `burn_out`, `poof` and `frost_blast` (the endings and
the dummy's), and `bleed_drips`, `chill_flakes` and `shock_sparks` (the statuses),
each under the shared emitter and particle caps. 01 (e) adds `staff_gutter`, a curl
of smoke off the staff light's last orange as it gutters out, and `frost_breath`,
a crawler's puff of frost as it shrinks from the light. 01 (f) adds `blood_burst`
and `blood_spurt` (a blow on flesh or the wizard, and bleeding's start),
`gore_burst` (iron's kill), `burn_embers`, `warp_burst` (the force push, on
`flame_glow`'s soft glow in the gravity ramp), `strike_sparks` (a bolt's
landing), `thaw_glints`, `iron_dust` (a hard landing) and `fizzle` (a refusal),
and makes `bleed_drips` denser (24 at 14 a second), larger and a brighter maroon.

## Iron blocks and crushing

Slot 7 sets an iron block down at the cursor, for 7.5 mana: three charges, one back
every 2 seconds; at most 24, the oldest going in a poof. A spot where the block
would meet a solid tile, a body that is not dynamic (a deck, a slab, lava's
surface, the elevator, the wizard or an enemy) or another block is refused
before the cast spends anything, with a fizzle; balls in the way are shoved
aside. It is a
120-pixel square collider, four large-ball diameters on a side, drawn from four
64-pixel lit quarters (`iron_brick_big_*`: riveted plates cut along their seams),
so it keeps the terrain's pixel scale. It weighs 400 balls and falls harder:
past 30 pixels/second downward it gains 1,800 pixels/second² more, half the
world's gravity, unless a troll holds or has thrown it. Iron receives 20%
of a ball's black-hole acceleration at the same distance, enough for a nearby
hole to drag it along the floor. Loose scraps receive a ball's full acceleration;
other bodies' pull is divided by mass.
The solver has no friction, so a supported block loses horizontal
speed at 1,500 pixels/second². Lifts carry it like the player and balls. A block
meeting a still body or a tile at 900 pixels/second or more raises `iron_dust`,
a shake a fifth of a blast's and `ex10_iron_thud_sfx`, once a block a step. A
body iron hits takes a push along the closing line, 0.6 times the closing speed
and at most 900 pixels/second, through the hit; a hit that kills a flesh target
bursts it in `gore_burst` in place of its poof.

A block moving at 360 pixels/second or more into a body, taken from its velocity
before the physics step, hurts the player it drives into (34, and 34 more per
further 600 pixels/second, with a push) and smashes the small balls ahead of it (four
per step, and one more per further 60 pixels/second of head-on speed, up to 64,
most head-on first). Physics contacts count even when the solver has already
pushed a marble clear of the block. Smashed balls vanish with up to four sampled
24-chip `ball_smash` bursts, and the block keeps 90% of its speed through them,
so a fast drop plows several layers into a pile. Crushing marbles or damaging
the player plays a heavy crunch, glass crackle and ringing iron clink, at most
once per 0.1 seconds across all blocks. A resting or creeping block does neither,
and large balls are never smashed.

Losing the last health or falling out of bounds ends the run with the shell's game
over: "YOU DIED" on a dark band while the world pixelates and dims, its sting and
its music, then Retry, which loads the run's last good save (a fresh run without
one), Load and Quit to Title.

## The bestiary

01 (c)'s five enemies (`actors.py`), drawn from the art-review sheet the owner
reviewed on 2026-10-21. Each is one rig posed per frame, drawn unlit with no normal or
emissive map, so the kit's hit flash shows through `damage_flash`; the moonlit top
and right edges and the glowing eyes are painted into their colours. Every clip's
frames are `<clip>_<i>`, and `generate.py` mirrors each into a `_left` twin, as it
does the wizard's. `art.py` holds the walkers to their feet's row: the crawler's
and the beetle's lowest pixel on row 60, the troll's on row 124 of its 128 frame
(`WALKER_FEET`), as it holds the wizard's on row 92 of his 96 (`PLAYER_FEET`).

| Enemy | Rig | Clips |
| --- | --- | --- |
| Wisp | A round flame with a face, self-lit like the fireball hazards; its tongues stream up and behind | `wisp_float` (8, flickering, a pixel's bob), `wisp_flare` (6, its alert: wider, eyes and mouth open) |
| Crawler | Five frost-blue plates with a cold glow between, eight high-kneed legs stepping in two sets of four, a wedge head with four glowing eyes, mandibles, swept feelers, ice crystals on its back, a crystal sting | `crawler_walk` (6), `crawler_lunge` (4: rearing, the front leg raised, jaws open), `crawler_scared` (4, looping: recoiled over its planted feet, front low, plates raised, feelers laid flat, jaws shut, eyes a slit, sting raised, a pixel's tremble) |
| Beetle | A rhinoceros beetle of riveted iron: banded wing cases with punctures, a riveted shield and short horn, the long ridged horn, red eyes, spined legs stepping in sets of three, rust along its seams and rivets | `beetle_walk` (6), `beetle_paw` (4: a front foot scraping, dust), `beetle_charge` (4: head down, horn levelled, dust behind) |
| Bat | A near-black furred body, dark purple wings with their finger bones showing, red eyes, fangs | `bat_roost` (4: hanging by its feet, swaying), `bat_fly` (6: the wings turned about their roots, folding a little on the downstroke), `bat_windup` (4: wings raised, the outline flashing white), `bat_dive` (2: wings swept back) |
| Troll | Lanky on a 128 frame, no hump, on a skeleton (`TrollPose`): hips and knees by `two_bone`, the pose's own elbows and wrists, each hand turned with its forearm (open, a fist, or both clasped overhead); a thick neck into the skull, a hooked nose, tusks, an ember eye, an ear with a ring, mossy hair, a hide loincloth on a strap, a scar, ribs, warts, nails | `troll_idle` (4, breathing), `troll_walk` (8: a heavy stride, the arms swinging against the legs), `troll_windup` (3, 133 200 467 ms, 0.8 seconds in all: the arms rising to lock overhead), `troll_slam` (3: fists down to the ground, a crouch, dust), `troll_swipe_windup` (2, 120 180 ms: the arm drawn back), `troll_swipe` (3, 60 70 200 ms: the arm raked under and up in front of it, streaks along the hand's arc), `troll_pickup` (3: bending, then lifting overhead), `troll_throw` (3: flung forward, following through) |
| Goblin | A sapper on a joint skeleton (01 (g)): a skull with a hooked nose, a slit-pupilled yellow eye and fangs, notched leaf ears with a ring, a goggled leather cap, a laced jerkin over a pot belly, a pouch of rocks and a bomb at the belt, clawed feet; its outline's lowest row on 60, its collider's foot | `goblin_idle` (4, 240 200 160 120 ms, looping), `goblin_walk` (6 × 85 ms), `goblin_windup` and `goblin_windup_bomb` (3, 110 140 200 ms: a rock or a lit bomb drawn back), `goblin_throw` (3, 60 70 170 ms), `goblin_stab_windup` (1, 200 ms: its knife drawn back low at its hip), `goblin_stab` (2, 80 140 ms: the knife thrust ahead off a lunging foot, then drawn halfway back), `goblin_hop` (2 × 110 ms, looping: knees tucked, arms flung up). Its rock (`goblin_rock_0`, `_1`) and bomb (`goblin_bomb_0`, `_1`) are 16 pixels, each centred on its frame, which it rolls about |

The frozen look (`effects.py`) is a frosty cube of ice in three-quarter view: a snow
cap, a side in shade, frost thick at the edges, a drift at the foot, frost ferns,
bubbles, cracks, a chipped corner and sparkles, the body showing through a clearer
window. `status_ice_<kind>` fits each kind's hurtbox plus 2 pixels on a full frame
(`HURTBOXES`: the wisp, the crawler, the beetle, the bat, the troll, the goblin and the dummy; `PROP_BOXES`: the crate and the keg),
so what lies outside the hurtbox shows outside the ice; `generate.check_ice_fits`
holds each box to its prefab's hurtbox wherever that prefab exists. The size
classes' `status_ice_<class>` and the ball's `ice_shell` are cubes too. A frozen
target with health shows `frozen_glints` over the 16-pixel `ice_glint`; frozen balls
and iron show none. The wisp's embers are `wisp_embers` over the spark. The drops are
24-pixel orbs, `health_orb` (red) and `mana_orb` (mana's blue), in a dithered halo,
palette pixels only. The summon slot's icons (`icon_wisp` to `icon_goblin`, `icon_crate`, `icon_keg`) are each
enemy's or prop's frame at half size, the troll's at a quarter, on the dummy's ring, which their tiles recolour to mana's blue.

`sfx.py` adds each enemy's sounds, 16-bit mono at 44,100 Hz: `wisp_hum` (a 2-second
crossfaded loop), `wisp_flare`, `crawler_hiss`, `crawler_lunge`, `crawler_scared` (a startled hiss as it shrinks from the light), `beetle_paw`,
`beetle_charge`, `bat_screech`, `bat_dive`, `troll_step`, `troll_roar`, `troll_slam`,
`troll_throw`, `enemy_hit`, `enemy_spotted` (the alert's sting, which each fresh alert sounds through the `alerted` rule since 01 (g)) and `orb_pickup`, registered
as `ex10_<name>_sfx`. `music_combat` is a fifth music loop at 22,050 Hz, sixteen bars
at 128 BPM in step with `music_explore`'s bars and chords, which it plays over while
an enemy sees the wizard: toms on every beat, an eighth-note low pulse, string stabs
on the off-beats and a bell figure in its second half; `LOOPS` holds its length and
seam.

## The tomes

e01tomes' families (`effects.py`), drawn from the review sheet the owner
accepted on 2026-10-26. Each book is closed on its 32-pixel frame, its front cover
facing and its head and fore-edge receding up and to the right (`TOME_LOOKS`,
`tome_parts`), its centre at (16, 16): the pact in cracked hide with iron corner
caps and a horned, eyed sigil, its page edges glowing ember; bounding in sky-blue
tooled leather with gusts and a silver wing clasp, its top sheets lifting; arcane
in violet with gold-leaf fleurons, a gold ring of runes about a cut gem, and a
ribbon. Their idle frames (`tome_<kind>_idle_<f>`, four at 180 ms) are lit; their
glow comes from the glow set alone, and the loop moves a bright band round it.
Each take (`tome_<kind>_take_<f>`, 64 pixels, six at 80 ms, unlit) flares the
book, breaks it into shards of light and opens a ring behind them. The bound's
ring (`bound_ring_<f>`, 64 × 32, six at 50 ms, unlit) is centred on the feet.
`TOME_CLIPS` lists the three clips of each kind and the ring's, which
`generate.spray_clips` registers. The tome tiles (`icon_tome_<kind>`) are each
book's first frame on a square tile in its own metal with a bracket on each
corner, apart from the spells' bevels; `hud_liquid_shield` is the shield's light,
cool blue strip. `generate.particle_configs` adds each book's motes
(`tome_<kind>_motes`, at most 8), which leave the book fast because particles
draw under it, each take's sparks (`tome_<kind>_take_sparks`, 24) and the bound's
(`bound_sparks`, 20; `bound_trail`, 12). `sfx.py` adds `tome_pact`, `tome_bounding`,
`tome_arcane` and `bound`, registered as `ex10_<name>_sfx`.
