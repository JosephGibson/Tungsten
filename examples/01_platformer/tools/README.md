# Twilight ruin authoring

The example runs from checked-in PNGs, JSON and Rust tables. Nothing here runs
from Cargo, engine startup or asset loading. Python 3, Rust's `rustfmt`, and
`Pillow==12.3.0` are needed only to edit/regenerate the artwork and layout.

Paths below are relative to this `tools/` directory unless a command starts at the repository root. For art, read [Editing art](#editing-art); for geometry, [Editing the level](#editing-the-level); for presentation, [Runtime and visual checks](#runtime-and-visual-checks); for spells/audio, [Fireball spell](#fireball-spell-extinguishing-and-sound-effects) and [Ice beam](#ice-beam-and-thermal-shattering). Current Rust wiring is in `src/{setup,systems,gameplay,extract,fireball,burning,ice,brick}.rs`, relative to the example root.

From the repository root:

```sh
python3 -m pip install -r examples/01_platformer/tools/requirements.txt
python3 examples/01_platformer/tools/generate.py
python3 examples/01_platformer/tools/generate.py --check
python3 -B -m unittest discover -s examples/01_platformer/tools -p 'test_*.py'
just level-check
cargo test -p example-01-platformer
cargo test -p tungsten-core --test manifests
just check
just repo-check
just smoke
cargo run -p example-01-platformer
TUNGSTEN_LIGHTING_FIXTURE=on cargo run -p example-01-platformer
```

The local Python tests are separate from `just script-test`. `just level-check` runs
`--check` and them together; it needs Pillow, so CI and the standard-library-only
recipes leave it out. `--check` regenerates
into a temporary directory, compares every owned byte, and fails on missing,
stale or uncovered output. It does not rewrite the working tree. PNG compression
is fixed with no timestamps; JSON is sorted; Rust output goes through `rustfmt`.
All generated files must accompany their sources in a change. `outputs.json` is
the exact ownership inventory. Normal generation removes only files explicitly
listed by the previous inventory (or `legacy_outputs.json` on the first migration).
It never sweeps unrelated files from the assets directory. `sounds.json` is the
sound registration; synthesized WAVs are owned outputs, and `black_hole.ogg` stays
hand-authored (see "Fireball spell, extinguishing and sound effects").

## Editing art

`palette.json` maps one-character symbols to RGBA bytes. `.` is transparent.
Every pixel must use the palette. All art is drawn by code; `art.read_grid`
still validates the text-grid format (one line per pixel row, one character per
pixel) for any hand-authored sprite. The modules:

| Module | Draws |
| --- | --- |
| `pixels.py` | Shared toolkit: masks, silhouette shading, overlap rims, outlines, dithering, whole-period waves |
| `actors.py` | Courier, fireball and both balls |
| `terrain.py` | Masonry, turf, cliff edges, decks, trestle, waterfall backing |
| `scenery.py` | Lamp posts, ivy, waystone, summit gate, oak, arch, roots, undergrowth, crystals, waterfall, spikes |
| `backdrops.py` | Sky, moon, clouds, ridges, woodland |
| `effects.py` | Particle sprites (dust, spark, droplet, firefly mote, cursor), glow gradients, black-hole layers |
| `polish_art.py` | HUD hearts, burning-ball flames, explosion shock ring, iron brick and scraps with frost, snowflakes, ice shards and cold rings |
| `sfx.py` | Synthesized sound effects (WAV) |
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

Backdrops are the only art larger than 64×64. Strips are 1024 pixels wide and
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
zero for empty, no flip bits) and `src/level_layout.rs`. Edit their source instead
of the generated output. No Tiled object layers or extra runtime paths are used.

## Runtime and visual checks

Draw order is parallax, background/decorations, back props, terrain, world props,
particles/actors, foreground, cursor. Tile/prop extraction culls to the viewport
and retains contiguous atlas/filter runs within each layer. Background coverage
fills the actual viewport for the sky and repeats the other strips using their
scroll factors, the current aspect ratio and shake overhang, including vertical
travel. Two translucent cloud layers drift independently. Cursor size remains 32 screen pixels.

Input captures accepted jumps before physics. Current collision contacts and
sleep state determine grounding afterward. Restart clears presentation and
suppresses the initial settling effect. Clip selection and one-shot effects
follow those triggers. The engine retains its single particle count/emit/tick
pass and deferred flush. Cleanup waits for first tick, drained report and zero
owned particles, allowing next-frame counts to catch up.

Manual acceptance still requires both full routes and the switchback; deliberate
falls; jump/landing particles; idle/airborne lit frames; animated props; balls,
black holes, hazard damage flash/shake and audible controls. Inspect resized windows
and both zoom limits for uncovered edges, seams, foreground occlusion and clutter.
Startup smoke alone does not establish these properties. Actual results and
remaining visual checks are listed here; archived implementation plans are historical evidence.

## Gameplay and presentation

LMB balls render at 32 pixels with a 15-pixel collider radius and cause no damage.
Spikes and moving fire use swept relative contact tests after physics. The
player has three health points. Every hit, from a hazard or a falling iron
brick, goes through `gameplay::damage_player`: it plays the hit sound, flashes
the damage material red, kicks the camera (trauma 0.5, a few pixels) and starts
1.1 seconds of immunity in which further hits are ignored and the player blinks,
with brief knockback and loss of control. The last heart opens the death screen
(see "Iron brick, death screen and restart"). Falling below the kill plane or
fully outside the active world bounds also kills the player, even during hit
immunity; **Enter** starts a fresh run. Fire destroys normal balls with a short
particle burst and expanding ring. Those effects cannot hurt the player; both emitter and ring
counts are capped at 16. The HUD shows remaining health.

`hazards` and `moving_platforms` in `level.json` use tile-space center `position`
and `travel` amplitudes, a `period` in seconds, and a `phase` in radians. Platforms
carry supported bodies before the engine physics step; upward jumps release
immediately. Static main routes remain available. Route tests avoid static
spikes; moving-fire timing still needs a human playthrough. Platform sweeps are
checked for terrain and rider headroom, and a complete physics cycle verifies
support without slipping through the deck.

Scene lights illuminate the normal-mapped terrain and the lit props: lamp posts,
ivy, waystones, gate, oaks, arch, roots, undergrowth, hanging moss and spikes.
Crystals, glowing mushrooms, the waterfall and the fireball stay unlit so they
keep their own glow. The player retains the
ordinary damage material and fixture lighting paths. Counter-rotating vortex
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
anchors are generated from the art; both effects follow facing, animation and
squash. A fresh run restores the lantern's default state. Fireballs have a
brighter inner halo and a stronger native light. They face their horizontal travel (vertical-only movers
face the player) and stretch up to 8% with speed. Each leaks rising flame wisps
from its seeded `fire_trail` emitter (32 live) and molten drips from a
`fireball_drips` emitter anchored 12 pixels below its center (16 live). That is
48 particles per fireball, 288 in total. All emitters remain subject to the
global 2048-particle budget.


## Midnight presentation and double jump

One aerial jump is available after takeoff (or walking off an edge). Release and
press Space again to use it; holding Space cannot consume it automatically. The
second jump plays the tuck-and-burst clip until the ascent ends, then the fall
clip, and emits a short 18-spark amber burst at the captured feet position.
Landing and restart replenish it. Ground jump/hold behavior remains unchanged,
and existing routes still pass with single jumps.

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
normal/emissive lighting and a weak neutral moon fill. No engine or shader-source
changes are required. Three red/empty pixel hearts read current HP every frame
and stay at a fixed screen size across zoom and camera movement.

## Small balls, collection pit and particles

Hold MMB for small balls at 156.25/second (five times LMB's 31.25/second).
Their collider radius is 7.5 pixels and render diameter is 16 pixels.

The two balls are deliberately different objects, each drawn at its render size
and stored as 2×2 or 4×4 pixel blocks so that nearest sampling stays exact:

- **LMB orbs** (`ball*`, 32 pixels): heavy bronze spheres with fill light and a
  specular glint. Three glowing teal meridians and rune rings roll around a
  tilted axis. They keep their authored colors; they carry no `BallHue`.
- **MMB marbles** (`ball_small*`, 16 pixels): neutral-grey glass with a thin rim,
  a fixed glint and a turning cat's-eye ribbon. Only these carry `BallHue`, so
  only they cycle through the rainbow; burning and charring tint the same greys. Both buttons have independent spawn timers.

S alone stops audio. The example reapplies its local ball, spell and audio bindings after
shared input hot reloads without writing the shared input file.

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
per frame, the shared 16-transient-emitter cap and global 2,048-particle budget
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
(`burning::flame_exposure`: no ball directly above, nor one above on each side)
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
Normal balls retain their fire-destruction behavior; burning small balls do not
damage the player or ignite normal balls.

Startup smoke and CPU tests do not verify sprite readability at 16 pixels, the
pit's visual fit during camera travel, or particle appearance and density in
motion. Review those by running `cargo run -p example-01-platformer`.

## Fireball spell, extinguishing and sound effects

Press **Mouse 4** (winit `Back`, `"button4"` in input files) to cast a fireball
from the player toward the cursor. The binding is example-local, applied by
`platformer_bindings` like MMB. Each fresh press casts one missile: at most six
alive, 0.2 seconds apart. It leaves the body at 900 pixels/second, falls under a
tenth of world gravity, is pulled by black holes like a unit-mass physics body,
and burns out silently after 2.1 seconds or outside the world
bounds. The missile is example-moved, not a physics body; only its blast pushes.

It explodes on the first solid it touches: any collider except the player's
(balls of either size, slab decks, lifts, iron bricks) or a solid collision tile.
The blast spawns 48 flame bodies (`fireball_blast`), 64 fast hot fragments
(`blast_embers`), 24 outward dust puffs (`blast_dust`) and 20 lingering smoke
puffs (`blast_smoke`): 156 particles, subject to the shared emitter and particle
caps. A white-hot flash fades over 0.18 seconds while two shock fronts expand
rapidly toward the blast's push radius. It pushes every dynamic body within
288 pixels straight away from it: 2,500 pixels/second for a
unit-mass body (a ball, the player) at the centre, falling linearly to nothing at
the edge and divided by mass, so an iron brick barely moves. A pushed player loses
control for up to 0.15 seconds, so a point-blank shove carries several tiles.
The blast ignites every small ball within 72 pixels through `burning::ignite`, so
spent balls stay spent and normal balls never burn. It does not hurt the player.
The camera shakes with trauma 0.68 for a blast within a quarter view width of
the view's centre, less with distance, and
not at all a view width and a half away.
In flight it draws the fireball frames at 40 pixels, turned along its velocity,
with a soft flame glow, a dense `spell_trail` comet tail and molten drips, and it
carries one warm point light (192-pixel radius). The light fades over 0.3 seconds
where the missile burns out, or flares and fades over 0.5 seconds where it
explodes, flaring to four times its flight intensity with twice the light radius.
Point lights warm the lit terrain, props and bricks around the explosion.

A black hole puts out every burning small ball inside its 384-pixel pull radius
in the same frame, leaving it spent exactly as after a normal burnout. At most
four sampled balls per frame puff an ember-to-steam `extinguish` burst, and the
sizzle plays at most every 0.15 seconds.

The black hole draws in two passes around the engine particles. Under them: the
soft halo, a hot accretion disk and two counter-rotating vortex layers. Over
them: 48 infalling streaks turned along their spiral, the photon ring and the
horizon, so sparks visibly vanish into the core. Its five sprites (`vortex`,
`vortex_core`, `accretion_disk`, `photon_ring`, `infall_streak`) are white alpha
fields on 96-pixel canvases, the only exception to the 64 × 64 rule. They are
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
`HAND_AUTHORED_SOUNDS` (currently only `black_hole.ogg`).

| ID | File | Event |
| --- | --- | --- |
| `ex10_fireball_cast_sfx` | `fireball_cast.wav` | A missile is cast |
| `ex10_fireball_blast_sfx` | `fireball_blast.wav` | A missile explodes |
| `ex10_extinguish_sfx` | `extinguish.wav` | A black hole puts out burning balls |
| `ex10_player_hit_sfx` | `player_hit.wav` | The player takes damage |
| `ex10_iron_crush_sfx` | `iron_crush.wav` | A brick crushes marbles or damages the player |
| `ex10_ice_beam_sfx` | `ice_beam.wav` | The ice channel starts |
| `ex10_ice_spray_sfx` | `ice_spray.wav` | Cold rushing wind loops while F is held |
| `ex10_ice_freeze_sfx` | `ice_freeze.wav` | Iron finishes freezing, with crackles and glassy chimes |
| `ex10_ice_end_sfx` | `ice_end.wav` | A soft pressure release when F is released |
| `ex10_ice_shatter_sfx` | `ice_shatter.wav` | Brittle iron fractures under heat or a fast iron impact |

## Ice beam and thermal shattering

Hold **F** to channel an ice flamethrower toward the cursor, or in the facing
direction without a cursor. The 28-degree cone reaches 448 pixels; seven rays
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
quietly. The control hints use three short rows that fit above the hearts.

Iron under the spray gradually gains a pale ice shell, white frost rims, crystalline facets,
branching cracks and hanging icicles. Four unlit `iron_frost_big_*` overlays keep
the coating visible at night over the lit iron plates. The block stays frozen
and brittle. Supported frozen
iron loses horizontal speed at 120 pixels/second², independently of normal friction.
A fireball's thermal blast shatters every frozen iron surface within 72 pixels,
including contact at a block's edge. Each full brick becomes exactly 16 physical
30-pixel scrap blocks in a 4×4 grid. Each scrap has 1/16 of a full brick's mass
(12.5) and sliding friction (93.75 pixels/second²), reduced to 7.5 pixels/second²
when frozen, so the sixteen pieces preserve the block's total mass.
They inherit the brick's velocity and launch outward at varied speeds of
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

## Iron brick, death screen and restart

Press **R** to place an iron brick at the cursor, at most 24. It is a
120-pixel square collider, four large-ball diameters on a side, drawn from four
64-pixel lit quarters (`iron_brick_big_*`: riveted plates cut along their seams),
so it keeps the terrain's pixel scale. It weighs 200 balls. Iron receives 20%
of a ball's black-hole acceleration at the same distance, enough for a nearby
hole to drag it along the floor. Loose scraps receive a ball's full acceleration;
other bodies' pull is divided by mass.
The solver has no friction, so a supported brick loses horizontal
speed at 1,500 pixels/second². Lifts carry it like the player and balls. `KeyR` is a
`tungsten-core` `KeyCode` variant added for this binding.

A brick moving at 360 pixels/second or more into a body, taken from its velocity
before the physics step, hurts the player it drives into (one heart, and one
more per further 600 pixels/second) and smashes the small balls ahead of it (four
per step, and one more per further 60 pixels/second of head-on speed, up to 64,
most head-on first). Physics contacts count even when the solver has already
pushed a marble clear of the brick. Smashed balls vanish with up to four sampled
24-chip `ball_smash` bursts, and the brick keeps 90% of its speed through them,
so a fast drop plows several layers into a pile. Crushing marbles or damaging
the player plays a heavy crunch, glass crackle and ringing iron clink, at most
once per 0.1 seconds across all bricks. A resting or creeping brick does neither,
and large balls are never smashed.

Losing the last heart or falling out of bounds starts the death screen
(`death.rs`). The body leaves physics where it fell, a golden burst marks the
spot, and over 0.9 seconds the frame pixelates to 48-pixel blocks, like the Sprite
Scene pause transition, and dims 80% toward near-black crimson. A large red
YOU DIED title settles into place above a pulsing restart prompt, with the
gameplay HUD hidden. Text scales to the window and draws after the post stack,
so the title stays sharp. Once the frame is dim, **Enter** restarts: the fade
closes to full cover in 0.35 seconds, `setup::restart_world` rebuilds the world under it, and
the cover lifts over 0.5 seconds. The screen runs on real time, so a paused or
scaled game clock cannot hold it.

`restart_world` despawns every entity, puts the per-run engine state back to what
`App::new` inserts (physics buffers, pending commands, the example's events,
camera state and controller, particle counters and the world RNG seed), stops all
sound and runs `seed_world` and the startup hook's `populate_world` again, so
resources, timers, spawn counters, cooldowns, zoom and camera match a launch.
Asset registries, bindings and display settings stay. Freed entity slots are
reused, so entity ids, and the cosmetic phases and emitter seeds derived from
them, can differ from a launch's.
