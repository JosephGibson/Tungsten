# Twilight ruin authoring

The example runs from checked-in PNGs, JSON and Rust tables. Nothing here runs
from Cargo, engine startup or asset loading. Python 3, Rust's `rustfmt`, and
`Pillow==12.3.0` are needed only to edit/regenerate the artwork and layout.

Paths below are relative to this `tools/` directory unless a command starts at the repository root. For art, read [Editing art](#editing-art); for geometry, [Editing the level](#editing-the-level); for presentation, [Runtime and visual checks](#runtime-and-visual-checks); for spells/audio, [Fireball spell](#fireball-spell-extinguishing-and-sound-effects). Current Rust wiring is in `src/{setup,systems,gameplay,extract,fireball,burning}.rs`, relative to the example root.

From the repository root:

```sh
python3 -m pip install -r examples/01_platformer/tools/requirements.txt
python3 examples/01_platformer/tools/generate.py
python3 examples/01_platformer/tools/generate.py --check
python3 -B -m unittest discover -s examples/01_platformer/tools -p 'test_*.py'
cargo test -p example-01-platformer
cargo test -p tungsten-core --test manifests
just check
just repo-check
just smoke
cargo run -p example-01-platformer
TUNGSTEN_LIGHTING_FIXTURE=on cargo run -p example-01-platformer
```

The local Python tests are separate from `just script-test`. `--check` regenerates
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
| `polish_art.py` | HUD hearts, burning-ball flames, explosion shock ring |
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
sleep state determine grounding afterward. Respawn clears presentation and
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
player has three health points, 1.1 seconds of hit immunity, brief knockback,
and a full-health safe respawn. Fire destroys normal balls with a short particle burst
and expanding ring. Those effects cannot hurt the player; both emitter and ring
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
squash. Toggle state survives respawn. Fireballs have a brighter inner halo and a
stronger native light. They face their horizontal travel (vertical-only movers
face the player) and stretch up to 8% with speed. Each leaks rising flame wisps
from its seeded `fire_trail` emitter (32 live) and molten drips from a
`fireball_drips` emitter anchored 12 pixels below its center (16 live). That is
48 particles per fireball, 288 in total. All emitters remain subject to the
global 2048-particle budget.


## Midnight presentation and double jump

One aerial jump is available after takeoff (or walking off an edge). Release and
press Space again to use it; holding Space cannot consume it automatically. The
second jump plays the tuck-and-burst clip until the ascent ends, then the fall
clip, and emits a short 18-spark amber burst at the captured feet position. Landing and respawn replenish it. Ground jump/hold
behavior remains unchanged, and existing routes still pass with single jumps.

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
timer. Burned-out balls remain as dark, nonflammable physics bodies. Every burning
ball has two large, flickering animated flame tongues, a pulsing glow and warm
tint. Up to 64 sampled balls share 128 emitters, rotating through the population
every 0.125 seconds. Each sampled ball emits 48 flame particles and 24 fast sparks
per second (six times the former total emission rate), with live caps of 40 and
24 respectively and the existing global 2,048-particle limit. The pool shrinks as
balls burn out or despawn, and remaining particles finish their short lifetimes.
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
tenth of world gravity, is pulled by black holes with the same force and falloff
as physics bodies, and burns out silently after 1.4 seconds or outside the world
bounds. The missile is example-moved, not a physics body: it cannot push the
player or balls.

It explodes on the first solid it touches: any collider except the player's
(balls of either size, slab decks, lifts) or a solid collision tile. The blast
spawns a flame bloom (`fireball_blast`), the existing spark burst and shock ring,
and ignites every small ball within 72 pixels through `burning::ignite`, so spent
balls stay spent and normal balls never burn. It does not hurt the player.
In flight it draws the fireball frames at 40 pixels, turned along its velocity,
with a soft flame glow, a dense `spell_trail` comet tail and molten drips.

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
and exponential envelopes, normalized to a −2 dBFS peak with short fades, and
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
