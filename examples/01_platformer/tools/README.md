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
sound registration; synthesized WAVs are owned outputs, and `black_hole.ogg` stays
hand-authored (see "Fireball spell, extinguishing and sound effects").

The shell's assets come with its code: the `shell` theme, Inter and Source
Serif 4 with their licence files, and the `damage_flash` shader and material are
copied byte for byte from `templates/basic/assets/`, with their manifest entries,
so `--check` also catches drift from the template; `just repo-check` holds the
theme to the template's as it holds `src/shell/`. `content/` holds the prefab
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
| `actors.py` | Courier (with its toss and aim frames and the lantern's cast twins), fireball, the small ball, the bowling ball, the training dummy and the five enemies (wisp, crawler, beetle, bat, troll) |
| `terrain.py` | Masonry, turf, cliff edges, decks, trestle, waterfall backing |
| `scenery.py` | Lamp posts, ivy, waystone, summit gate, oak, arch, roots, undergrowth, crystals, waterfall, spikes |
| `backdrops.py` | Sky, moon, clouds, ridges, woodland |
| `effects.py` | Particle sprites (dust, spark, droplet, firefly mote, cursor), glow gradients, black-hole layers, the 32-pixel icons of the nine spells, the gun and the six summon types, the lightning bolt's segments, the status overlays, the fitted ice cubes, the ball's cube, the ice glint, the muzzle flash, the tracer and the drops' orbs |
| `polish_art.py` | Hearts (which the HUD's bars replaced), burning-ball flames, explosion shock ring, iron block and scraps with frost, snowflakes, ice shards and cold rings |
| `sfx.py` | Synthesized sound effects, the shell's menu sounds and game over's sting, and the music loops (WAV) |
| `shaders/soft_glow.wgsl` | Example-local glow material, copied to `assets/shaders/` |

`actors.py` draws the courier and the fireball. The courier is one rig posed per
frame: hips, neck, feet, hands, lantern swing, cloak hem, scarf chain and hood
tip. Each part (legs, boots, satchel, belted cloak, hood with glowing eyes, scarf,
lantern, sleeves) is filled from palette symbols, shaded from its own silhouette,
rimmed where it overlaps the parts behind it and outlined once. No raster is
rotated. Edit a clip by changing its `Pose` list and durations. Generation
rejects a frame unless its lowest pixel sits on row 60 (bounding-box edge 61,
which the extractor anchors to the collider's feet). It also rejects a frame
whose lantern glass is not entirely inside the flame window (x 40–62, y 30–59),
or whose window contains any other `y`/`Y` pixel. Clips:

| Clip | Frames | Motion |
| --- | --- | --- |
| idle | 8, 100–300 ms | breathing, lagging lantern sway, scarf and hood-tip drift, one blink |
| walk | 12, 45 ms | leaning run: 2 px bob, toe-off heel lift, trailing cloak and curling scarf |
| jump | 3, 200 ms total | takeoff stretch, rise, apex (held); must finish within 13 frames |
| fall | 4, 80 ms | loop: cloak, hood tip and scarf streaming upward |
| land | 3, 60 ms | impact squint and bent knees (at most 2 px), recover, settle; runtime squash owns scale |
| double jump | 4, 300 ms total | crouched tuck, then the cloak bursts open and the courier springs up |
| toss | 4, 300 ms total | wind-up, swing, release, follow-through: the free hand throws a bowling ball toward the aim, the lantern hand braced |
| aim | 5 | the free arm and its gun from straight up to straight down by eighths of a turn, drawn over the cloak so a downward aim shows; 01 shows the one nearest a cast's aim, standing |

`generate.py` mirrors every courier and fireball frame into a `_left` twin, since
nothing flips a sprite in place, and gives every courier frame a `_cast` twin
(`actors.cast_frame`): all its lantern glass at the hot core and the frame round it
lit, so its emissive mask widens. 01 shows the twins for 0.3 seconds after a cast.

Palette symbols `O P Q U W` (cloak), `X z +` (scarf), `[ ] (` (trousers and leather)
belong to the courier. `M L K J Z` are the fireball's ember-to-white-hot ramp.

The fireball is an analytic ball of flame. Tongues stream up and behind a
right-moving hazard, plasma bands swirl through the core, and embers peel off.
Its 10 frames loop every 600 ms. It is self-lit: it is drawn unlit at its
authored colors and has no normal or emissive map. The existing `fire_*` frames
remain the burning-ball flames.

Normal and emissive images derive from each final frame's silhouette and glowing
detail colors, including every player clip and all rune rotations. Terrain
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

Art is 64 × 64 unless `art.py`'s size tables say otherwise: the backdrops, the
black hole's 96-pixel layers, the 32-pixel icons, the elements' sizes (the bolt's
segments, the status overlays' small and large classes, the muzzle flash, the
tracer, the glint and the orbs) and the troll, whose frames and fitted ice cube
are 128 pixels, the one enemy larger than a tile (`SIZE_PREFIXES`). Backdrop
strips are 1024 pixels wide and
wrap seamlessly: profiles are whole-period sine sums, and shapes are drawn at x
and x ± 1024. The ridges' and woodland's last row stays opaque, because the
runtime stretches it downward. The sky must keep a mean luminance below 25; the
moon keeps bounds `(7,7,57,57)` and a bright centre. Particle sprites stay 64×64
and render small via config scale.

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
transient jump/landing/explosion emitters. Black holes retain their own 1200-particle cap.

`routes` names platform waypoints. Both main routes start at the lantern apron;
the upper aqueduct joins the waterfall climb, with a switchback to the high
bridge. Recovery and grotto routes climb back to the main path. The tower routes
join at the gate; the high overlook is optional. Gameplay geometry is authored,
never randomized. Route tests use actual engine input and physics against the
real generated map, searching launch positions without changing gravity or
colliders. Run those tests after any layout change; geometric jump estimates
alone do not prove clearance beneath the solid platforms.

`level.py` emits the embedded Tiled catalog (`firstgid=1`, contiguous local IDs,
zero for empty, no flip bits) and the map's object layers, with the prefabs their
classes name. Edit their source instead of the generated output. The object layers
sit before `collision`, in spawn order, and `World::spawn_map_objects` spawns them
when a run starts, each object taking the run's next save ID:

| Layer | Objects | Prefab class |
| --- | --- | --- |
| `props` | Every prop, at its tile position | `ex10_prop_<sprite>_<depth>`: its sprite, behind (`back`) or among (`world`) the world's sprites, and its animation |
| `slabs` | One box per thin platform: slab runs that share a row and a depth and touch end to end merge, so a body sunk into a deck meets no join | `ex10_slab_<width>x<height>`: a static body |
| `hazards` | The spikes and the fireballs | `ex10_spikes`; `ex10_fire_<n>`, each fireball's own: today's sine motion on the kit's `oscillator`, starting where the sine stands at time 0. Both hurt through the kit's `contact_damage` (34 a touch, pushing 560 px/s away) |
| `platforms` | The moving platforms | `ex10_platform_<n>`, each its own: a kinematic deck on the kit's `path_follower`, ping-ponging between its sine's two ends with a sine easing, half a period a leg, drawn by `lift_deck_<n>`, three deck tiles across its width |
| `areas` | `landmarks`, a zone across each one's columns | `ex10_area_<n>`: a sensor body, the kit's `zone` for the player and its `area` name |
| `emitters` | The ambient emitters | `ex10_ambient_emitter`; each object's `particle_emitter.config` and `particle_emitter.seed` properties set its emitter |
| `markers` | `spawn`, where a fresh run's player stands | None: the game reads it by name |
| `dummies` | The training dummy in the Lantern clearing, the map's last object layer, so every earlier object keeps its save ID | `dummy`: each placement's `tags.<n>` properties set its tags (`flesh`, `freezable`, `conductive`) |

A parameter that lives in an array (a box's half extents, an oscillator's
amplitude, a path's points) gives its object a prefab of its own; a scalar would
be an object property over a shared prefab. No other runtime paths are used.

## Runtime and visual checks

Draw order: 01 replaces the engine's sprite base with its backdrop (the sky filling
the viewport, the moon, and the strips at their scroll factors with the shake's
overhang, two translucent cloud layers drifting), then the engine's tilemaps and its
default sprites by z (props, glows, the toys and the actors). The toys' and the
elements' effects (flames, shock rings, the vortex, spray wisps, cold pulses) draw
after them, and the cursor last, at 32 screen pixels. The foreground tiles draw
under the sprites.

The kit's platformer controller reads the jump each fixed step, with coyote time
and a jump buffer, and its ground probe decides grounding. The clips and the jump,
air-jump and landing bursts follow its flags, and a fresh run settles onto the
ground silently. A burst's emitter goes once it has drained and its particles are
gone.

Manual acceptance still requires both full routes and the switchback; deliberate
falls; jump/landing particles; idle/airborne lit frames; animated props; every
spell; a hazard's knockback and shake; and the HUD and the sounds. Inspect resized windows
and both zoom limits for uncovered edges, seams, foreground occlusion and clutter.
Startup smoke alone does not establish these properties. Actual results and
remaining visual checks are listed here; archived implementation plans are historical evidence.

## Gameplay and presentation

Balls never hurt the player, nor does iron but for a block driven into it fast
(see [Iron blocks and crushing](#iron-blocks-and-crushing)). Spikes and moving fire
hurt through the kit's `contact_damage`: 34 a touch, pushing 560 pixels/second
away, so the third touch takes Normal's 100 health (Easy has 150, Hard 68). Each
hit plays the hit sound and kicks the camera through the player's `damaged` cues,
knocks it back for 0.18 seconds and opens the kit's 1.1-second window in which
further hits are ignored; the lit courier shows no hit flash. Falling below the
kill row or wholly out of the world box deals all the health left, window or not.
Game over is the shell's, and its Retry loads the run's last good save. A fireball
bursts a bowling ball with a short particle burst and an expanding ring; both
burst and ring counts are capped at 16. The HUD's bars show the health and the mana.

`hazards` and `moving_platforms` in `level.json` use tile-space center `position`
and `travel` amplitudes, a `period` in seconds, and a `phase` in radians, which the
kit's `oscillator` and `path_follower` take. The kit's controller rides a moving
deck and leaves it on a jump. Static main routes remain available. Route tests run
on the map without hazards or moving platforms; the playthrough test walks the
lower route with them, jumping the spikes and waiting out the fire. Platform sweeps
are checked for terrain and rider headroom, and a complete physics cycle verifies
support without slipping through the deck.

Scene lights illuminate the normal-mapped terrain and the lit props: lamp posts,
ivy, waystones, gate, oaks, arch, roots, undergrowth, hanging moss and spikes.
Crystals, glowing mushrooms, the waterfall and the fireball stay unlit so they
keep their own glow. The courier draws lit, with its normal and emissive maps.
Counter-rotating vortex
layers and inward orbiting sparks animate independently of the existing ball
attraction. Camera zoom spans 35–300%.


## Final placement, collision and lighting polish

`placement.py` resolves `support` names in `level.json`. Grounded props place their
visible bottom edge on the named surface; hanging moss attaches its first pixel
to the deck underside. Composite `groups` preserve relative piece positions and
validate both feet/trunk against the support width. Wall roots remain attached
to cliff faces. Four timber trestles join deck undersides to ground, and the
waterfall has a continuous wet cliff backing, capped by a mossy spring tile
(`cliff_back_top`). One decorative arch remains. Summit
stairs and a grotto stepping stone add geometry without removing existing routes.

`SLAB_COLLIDERS` and `DECK_DEPTH` derive from the art's opaque bounds. Current
slabs, bridges and moving decks are 23 pixels deep, rather than the old 64-pixel
tile collision. The runtime and route tests spawn the same shallow bodies;
solid terrain retains native tile collision. Keep platform art and generated
collision together when changing silhouettes.

Press **L** to toggle the player's lantern halo and point light. Per-frame flame
anchors are generated from the art, the cast twins' and the mirrored frames'
too; both effects follow facing and animation. A fresh run lights the lantern.
Fireballs have a brighter inner halo and a stronger native light. They face their
horizontal travel (vertical-only movers face the player). Each leaks rising flame wisps
from its seeded `fire_trail` emitter (32 live) and molten drips from a
`fireball_drips` emitter anchored 12 pixels below its center (16 live). That is
48 particles per fireball, 288 in total. All emitters remain subject to the
global 2048-particle budget.


## Midnight presentation and double jump

The kit's controller allows one air jump after takeoff (or walking off an edge):
release and press Space again to use it; holding Space cannot spend it. It plays the
tuck-and-burst clip until the ascent ends, then the fall clip, and emits a short
18-spark amber burst at the feet. Landing gives it back. The routes still pass with
single jumps.

The entry clearing is 16 tiles wide. Its first steps, optional overhead platforms,
and lift sit farther from spawn, with fewer foreground props in the launch area.
Support anchors move with the revised layout.

Glows (lamp, fireball, crystal, player-lantern and moon halos, plus the black
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
the point lights nearest the camera view. At the widest zoom some off-centre pools can drop out; that
selection lives in the engine.

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

## Small balls, collection pit and particles

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

Beyond the gate, walk across the four-tile apron to the open collection pit.
Its clear interior spans columns 132–180 and rows 18–46: **48 × 28 tiles,
3,072 × 1,792 pixels**. Solid walls and floor are at least four tiles (256 pixels)
thick; the far wall rises six tiles above the entry rim. The map is 184 × 50 tiles,
and the kill plane lies below the floor. No hazards occupy the pit. The physics
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
The existing 1,200-particle cap remains. Fire trails use soft flame bodies with
a cream/yellow/orange/red ramp, while torch sparks use the same warm ramp with
smaller sizes and longer lifetimes. Both accelerate upward with varied speeds,
sizes, lifetimes and rotation. The fire-destruction explosion remains a separate
outward burst.

Small balls ignite on contact with moving fire hazards and spread fire to other
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

The HUD draws the icons (`effects.py`, 32 pixels, nearest filtering) at the
largest whole number of screen pixels per art pixel the UI's scale allows, each in
its element's border colour: ember orange for fire, ice cyan for cold, violet for
shock (lightning), indigo for mana and the summon, and bone white for the physical
toys and the gun's magazine. Slot 0 shows the summon's selected type, the dummy's
icon. The palette's indigo shades (`Γ Π Ξ`) are the mana's, and its violet ones
(`Δ Λ Σ`) the black hole's icon and the lightning's.

## Fire missile, extinguishing and sound effects

Slot 1 casts a fire missile toward the cursor through the kit's `spawn` payload: one
a press, at most six alive, 0.25 seconds apart, for 8 mana. It leaves 24 pixels out
from the courier's centre at 900 pixels/second on the kit's `linear_motion`, falls
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
divided by mass, so an iron block barely moves. The courier, a kinematic body,
stays put. The blast ignites every small ball within 72 pixels through
`elements::ignite`, so spent balls stay spent and bowling balls never burn. It does
not hurt the player.
The camera shakes with trauma 0.68 for a blast within a quarter view width of
the view's centre, less with distance, and
not at all a view width and a half away.
In flight it draws the fireball frames at 40 pixels, turned along its velocity,
with a soft flame glow, a dense comet tail and molten drips as its children, and it
carries one warm point light (192-pixel radius). The light fades over 0.3 seconds
where the missile burns out, or flares and fades over 0.5 seconds where it
explodes, flaring to four times its flight intensity with twice the light radius.
Point lights warm the lit terrain, props and iron around the explosion.

A black hole, slot 6, opens at the cursor for 4 seconds, one at a time. It puts out
every burning small ball inside its 384-pixel pull radius in the same fixed step,
leaving it spent exactly as after a normal burnout. At most four sampled balls a
step puff an ember-to-steam `extinguish` burst, and the sizzle plays at most every
0.15 seconds.

The black hole draws its soft halo, a hot accretion disk and two counter-rotating
vortex layers, then 48 infalling streaks turned along their spiral, the photon
ring and the horizon, all after the engine's sprites and particles. Its five sprites (`vortex`,
`vortex_core`, `accretion_disk`, `photon_ring`, `infall_streak`) are white alpha
fields on 96-pixel canvases, an exception to the 64 × 64 rule as the troll's
128-pixel frames are. They are
tinted at extraction, fade to transparent before their edges, and use `linear`
filtering. Its 1,200-particle budget splits into 900 sparks and 300 dark-gas
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
`HAND_AUTHORED_SOUNDS` (currently only `black_hole.ogg`). The one-shot sounds play
through the kit's cues: 01 sends a `Sounded` game event, and
`content/data/cue_rules/sounds.json` plays its sound, throttled by `min_interval`.
The sprays' loops are voices of their own.

| ID | File | Event |
| --- | --- | --- |
| `ex10_fireball_cast_sfx` | `fireball_cast.wav` | A missile is cast |
| `ex10_fireball_blast_sfx` | `fireball_blast.wav` | A missile explodes |
| `ex10_extinguish_sfx` | `extinguish.wav` | A black hole puts out burning balls |
| `ex10_player_hit_sfx` | `player_hit.wav` | The player takes damage |
| `ex10_iron_crush_sfx` | `iron_crush.wav` | Iron crushes small balls or hurts the player |
| `ex10_ice_beam_sfx` | `ice_beam.wav` | The ice spray starts |
| `ex10_ice_spray_sfx` | `ice_spray.wav` | Cold rushing wind loops while the ice spray is held |
| `ex10_ice_freeze_sfx` | `ice_freeze.wav` | Iron finishes freezing, with crackles and glassy chimes |
| `ex10_ice_end_sfx` | `ice_end.wav` | A soft pressure release when the ice spray is let go |
| `ex10_ice_shatter_sfx` | `ice_shatter.wav` | Brittle iron fractures under heat or a fast iron impact |
| `ex10_flame_start_sfx` | `flame_start.wav` | The flamethrower catches: a low whoomph opening into a hiss |
| `ex10_flame_spray_sfx` | `flame_spray.wav` | Its roar loops while it is held |
| `ex10_force_push_sfx` | `force_push.wav` | A force push: a deep whump and a rush of air |
| `ex10_toss_sfx` | `toss.wav` | A bowling ball leaves the hand |
| `ex10_place_sfx` | `place.wav` | An iron block or small balls set down |
| `ex10_black_hole_sfx` | `black_hole.ogg` | A black hole opens (hand-authored) |
| `ui_move`, `ui_select`, `ui_back` | `ui_*.wav` | The shell's menu sounds, by the IDs it plays: a plucked tick, two bells rising, two falling |
| `game_over` | `game_over.wav` | Game over's sting: a falling bell line, then a low toll under a plucked D minor chord |
| `music_title`, `music_over` | `music_*.wav` | The shell's title loop, a twelve-bar lullaby in D minor, and game over's, a four-bar dirge |
| `music_explore`, `music_pause` | `music_*.wav` | A run's exploration loop, and pause's over the quieted run |
| `ex10_gun_shot_sfx` | `gun_shot.wav` | A shot: a cracking burst over a low punch and a slide's tick |
| `ex10_gun_reload_sfx` | `gun_reload.wav` | The magazine whole again: a slide, two clicks and an iron ring |
| `ex10_ricochet_sfx` | `ricochet.wav` | A bullet turns off iron: a ping that whines away |
| `ex10_thunder_sfx` | `thunder.wav` | A lightning strike: a splitting crack and a rolling rumble |
| `ex10_crackle_sfx` | `crackle.wav` | A conduction jump: a short electric buzz |
| `ex10_steam_burst_sfx` | `steam_burst.wav` | Cold on fire, or a fire hazard put out: a wet whoosh into a hiss |
| `ex10_frost_blast_sfx` | `frost_blast.wav` | Thermal shock: fracturing ice, a cold boom and falling shards |
| `ex10_dummy_hit_sfx` | `dummy_hit.wav` | The dummy takes damage, through its `damaged` cue: a sackcloth thump |
| `ex10_summon_sfx` | `summon.wav` | A dummy summoned: rising bells over a soft swell |

The music loops are 16-bit mono at 22,050 Hz, on the music bus. Each renders a
bar past its end and folds that tail onto its start, so it repeats at its bar
line; generation fails a loop whose length is not its bars exactly or whose seam
jumps further than any step inside it (`sfx.seam_problem`).

## Ice spray and thermal shattering

Hold `cast` on slot 3 to channel the ice spray toward the cursor, for 18 mana a
second; slot 2's flamethrower is the same spray in fire, half the reach, lighting
the small balls it meets. The 28-degree cone reaches 448 pixels; seven rays
stop on colliders and solid tiles, so exposed iron at the sides freezes too.
The nozzle follows the interpolated caster and current aim every drawn frame.
Its visuals and sound start on press even before the next fixed physics step;
freezing exposure still uses only simulation time.
Snowflake crystals, expanding blue-white mist, soft fluttering wisps and contact
splashes replace the straight beam. Newborn spray particles rotate into their
birth aim and expire before blocking surfaces. One channel owns three continuous
emitters, capped at 292 particles combined under the global 2,048 budget;
completion and shatter bursts share the existing 16-transient-emitter cap.
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
Release, death or loss of the caster immediately stops the loop and removes its
emitters, including on frames with no fixed step; existing particles finish fading.
Normal release adds a quiet 0.18-second pressure tail; death and caster loss stop
quietly.

Iron under the spray gradually gains a pale ice shell, white frost rims, crystalline facets,
branching cracks and hanging icicles. Four unlit `iron_frost_big_*` overlays keep
the coating visible at night over the lit iron plates. The block stays frozen
and brittle. Supported frozen
iron loses horizontal speed at 120 pixels/second², independently of normal friction.
A fireball's thermal blast shatters every frozen iron surface within 72 pixels,
including contact at a block's edge. Each full block becomes exactly 16 physical
30-pixel scrap blocks in a 4×4 grid. Each scrap has 1/16 of a full block's mass
(12.5) and sliding friction (93.75 pixels/second²), reduced to 7.5 pixels/second²
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
  flames.
- **The lightning bolt**: eight 24 × 64 segments, `bolt_0` to `bolt_7`, each entering
  at its top centre and leaving `BOLT_EXITS` pixels aside at its bottom, so a strike
  stacks five to eight of them from the sky down, each starting where the last left;
  a violet haze, a glow, a light core and a white spine, every third forking a twig.
- **The ice shell** over a frozen ball, `ice_shell`.
- **The gun**: a three-frame 16-pixel muzzle flash (`muzzle_0` to `muzzle_2`) and a
  32 × 3 tracer along +x, a dim tail to a white-hot head, whose head the lightning's
  arcs and veil also take their white from.
- **The training dummy** (`actors.py`, `dummy`): a straw target on a post, 64 × 64,
  its lowest pixel on row 60 as the courier's is, drawn unlit with no normal or
  emissive map, so the kit's hit flash shows through `damage_flash`.

Nine particle configs join, over today's sprites: `muzzle_flash` and `ricochet`
(the gun), `steam_burst`, `burn_out`, `poof` and `frost_blast` (the endings and
the dummy's), and `bleed_drips`, `chill_flakes` and `shock_sparks` (the statuses),
each under the shared emitter and particle caps.

## Iron blocks and crushing

Slot 7 sets an iron block down at the cursor, for 15 mana: three charges, one back
every 2 seconds; at most 24, the oldest going. It is a
120-pixel square collider, four large-ball diameters on a side, drawn from four
64-pixel lit quarters (`iron_brick_big_*`: riveted plates cut along their seams),
so it keeps the terrain's pixel scale. It weighs 200 balls. Iron receives 20%
of a ball's black-hole acceleration at the same distance, enough for a nearby
hole to drag it along the floor. Loose scraps receive a ball's full acceleration;
other bodies' pull is divided by mass.
The solver has no friction, so a supported block loses horizontal
speed at 1,500 pixels/second². Lifts carry it like the player and balls.

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
does the courier's. `art.py` holds the walkers to their feet's row: the crawler's
and the beetle's lowest pixel on row 60, as the courier's is, the troll's on row
124 of its 128 frame (`WALKER_FEET`).

| Enemy | Rig | Clips |
| --- | --- | --- |
| Wisp | A round flame with a face, self-lit like the fireball hazards; its tongues stream up and behind | `wisp_float` (8, flickering, a pixel's bob), `wisp_flare` (6, its alert: wider, eyes and mouth open) |
| Crawler | Five frost-blue plates with a cold glow between, eight high-kneed legs stepping in two sets of four, a wedge head with four glowing eyes, mandibles, swept feelers, ice crystals on its back, a crystal sting | `crawler_walk` (6), `crawler_lunge` (4: rearing, the front leg raised, jaws open) |
| Beetle | A rhinoceros beetle of riveted iron: banded wing cases with punctures, a riveted shield and short horn, the long ridged horn, red eyes, spined legs stepping in sets of three, rust along its seams and rivets | `beetle_walk` (6), `beetle_paw` (4: a front foot scraping, dust), `beetle_charge` (4: head down, horn levelled, dust behind) |
| Bat | A near-black furred body, dark purple wings with their finger bones showing, red eyes, fangs | `bat_roost` (4: hanging by its feet, swaying), `bat_fly` (6: the wings turned about their roots, folding a little on the downstroke), `bat_windup` (4: wings raised, the outline flashing white), `bat_dive` (2: wings swept back) |
| Troll | Lanky on a 128 frame, no hump, on a skeleton (`TrollPose`): hips and knees by `two_bone`, the pose's own elbows and wrists, each hand turned with its forearm (open, a fist, or both clasped overhead); a thick neck into the skull, a hooked nose, tusks, an ember eye, an ear with a ring, mossy hair, a hide loincloth on a strap, a scar, ribs, warts, nails | `troll_idle` (4, breathing), `troll_walk` (8: a heavy stride, the arms swinging against the legs), `troll_windup` (3: the arms rising to lock overhead), `troll_slam` (3: fists down to the ground, a crouch, dust), `troll_pickup` (3: bending, then lifting overhead), `troll_throw` (3: flung forward, following through) |

The frozen look (`effects.py`) is a frosty cube of ice in three-quarter view: a snow
cap, a side in shade, frost thick at the edges, a drift at the foot, frost ferns,
bubbles, cracks, a chipped corner and sparkles, the body showing through a clearer
window. `status_ice_<kind>` fits each kind's hurtbox plus 2 pixels on a full frame
(`HURTBOXES`: the wisp, the crawler, the beetle, the bat, the troll and the dummy),
so what lies outside the hurtbox shows outside the ice; `generate.check_ice_fits`
holds each box to its prefab's hurtbox wherever that prefab exists. The size
classes' `status_ice_<class>` and the ball's `ice_shell` are cubes too. A frozen
target with health shows `frozen_glints` over the 16-pixel `ice_glint`; frozen balls
and iron show none. The wisp's embers are `wisp_embers` over the spark. The drops are
24-pixel orbs, `health_orb` (red) and `mana_orb` (indigo), in a dithered halo,
palette pixels only. The summon slot's icons (`icon_wisp` to `icon_troll`) are each
enemy's frame at half size, the troll's at a quarter, on the dummy's ring.

`sfx.py` adds each enemy's sounds, 16-bit mono at 44,100 Hz: `wisp_hum` (a 2-second
crossfaded loop), `wisp_flare`, `crawler_hiss`, `crawler_lunge`, `beetle_paw`,
`beetle_charge`, `bat_screech`, `bat_dive`, `troll_step`, `troll_roar`, `troll_slam`,
`troll_throw`, `enemy_hit`, `enemy_spotted` (the alert) and `orb_pickup`, registered
as `ex10_<name>_sfx`. `music_combat` is a fifth music loop at 22,050 Hz, sixteen bars
at 128 BPM in step with `music_explore`'s bars and chords, which it plays over while
an enemy sees the courier: toms on every beat, an eighth-note low pulse, string stabs
on the off-beats and a bell figure in its second half; `LOOPS` holds its length and
seam.
