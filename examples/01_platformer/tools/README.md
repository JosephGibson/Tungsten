# Twilight ruin authoring

The example runs from checked-in PNGs, JSON and Rust tables. Nothing here runs
from Cargo, engine startup or asset loading. Python 3, Rust's `rustfmt`, and
`Pillow==12.3.0` are needed only to edit/regenerate the artwork and layout.

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
It never sweeps unrelated files from the assets directory. `sounds.json` preserves
the existing sound registration; the OGG remains independently owned.

## Editing art

`palette.json` maps one-character symbols to RGBA bytes. `.` is transparent.
Each line of a `.grid` file is one pixel row, and each character is one pixel.
Player key poses, orb, lantern, marker and vines are native 64×64 grids, not
scaled copies of the old art. `gate.grid` is a 192×192 composition, split into
`gate_big_<row>_<col>` pieces. Edit these text grids directly to move a contour,
add a highlight or change a detail. Every pixel must use the palette. Preserve
a transparent outer border and the courier's foot baseline at pixel edge 61.
The custom extractor anchors that baseline to the collision box's feet.

`art.py` and `polish_art.py` derive the animation frames with integer part shifts, nearest-neighbor
rotation and palette cycling. Landing frames alter folds and eyes; the existing
squash system owns the scale envelope so landing is not squashed twice. Normal
and emissive images derive from each final frame's silhouette and glowing detail
colors, including idle, jump, fall, landing and all rune rotations. Terrain uses
shared perimeter colors with cosmetic variation restricted away from seams.
Backgrounds use palette dithering and periodic silhouettes; only these may be
larger than 64×64. Particle sprites stay 64×64 and render small via config scale.

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
remaining acceptance items are recorded in the implementation plan.

## Gameplay and polish follow-up

Balls render at 32 pixels with a 15-pixel collider radius and cause no damage.
Spikes and moving fire use swept relative contact tests after physics. The
player has three health points, 1.1 seconds of hit immunity, brief knockback,
and a full-health safe respawn. Fire destroys balls with a short particle burst
and expanding ring. Those effects cannot hurt the player; both emitter and ring
counts are capped at 16. The HUD shows remaining health.

`hazards` and `moving_platforms` in `level.json` use tile-space center `position`
and `travel` amplitudes, a `period` in seconds, and a `phase` in radians. Platforms
carry supported bodies before the engine physics step; upward jumps release
immediately. Static main routes remain available. Route tests avoid static
spikes; moving-fire timing still needs a human playthrough. Platform sweeps are
checked for terrain and rider headroom, and a complete physics cycle verifies
support without slipping through the deck.

`polish_art.py` authors twelve walk frames with alternating planted/swing feet,
body bob and lantern motion. It also supplies irregular rock variants, carved
and broken slabs, rope/weathered bridges, oak/arch/root compositions, small
foliage, fire, spikes, clouds, vortex spirals and soft halo/ring assets. Scene
lights illuminate normal-mapped terrain and props; the player retains the
ordinary damage material and fixture lighting paths. Counter-rotating vortex
layers and inward orbiting sparks animate independently of the existing ball
attraction. Camera zoom spans 35–300%.


## Final placement, collision and lighting polish

`placement.py` resolves `support` names in `level.json`. Grounded props place their
visible bottom edge on the named surface; hanging moss attaches its first pixel
to the deck underside. Composite `groups` preserve relative piece positions and
validate both feet/trunk against the support width. Wall roots remain attached
to cliff faces. Four timber trestles join deck undersides to ground, and the
waterfall has a continuous cliff backing. One decorative arch remains. Summit
stairs and a grotto stepping stone add geometry without removing existing routes.

`SLAB_COLLIDERS` and `DECK_DEPTH` derive from the art's opaque bounds. Current
slabs, bridges and moving decks are 23 pixels deep, rather than the old 64-pixel
tile collision. The runtime and route tests spawn the same shallow bodies;
solid terrain retains native tile collision. Keep platform art and generated
collision together when changing silhouettes.

Press **L** to toggle the player's lantern halo and point light. Per-frame flame
anchors are generated from the art; both effects follow facing, animation and
squash. Toggle state survives respawn. Moving fires have a brighter inner halo,
stronger native light and a seeded particle emitter capped at 40 particles per
flame (240 total). All emitters remain subject to the global 2048-particle budget.


## Midnight presentation and double jump

One aerial jump is available after takeoff (or walking off an edge). Release and
press Space again to use it; holding Space cannot consume it automatically. The
second jump restarts the jump pose and emits a short 18-spark amber burst at the
captured feet position. Landing and respawn replenish it. Ground jump/hold
behavior remains unchanged, and existing routes still pass with single jumps.

The entry clearing is 16 tiles wide. Its first steps, optional overhead platforms,
and lift sit farther from spawn, with fewer foreground props in the launch area.
Support anchors move with the revised layout.

The palette uses charcoal stone, muted olive foliage and warm highlights. A
separate circular pixel moon and soft halo keep their aspect ratio on resize;
clouds and landscape can pass in front. The example selects the existing stock
Bloom and Vignette post passes at restrained strengths (0.22 and 0.18), alongside
normal/emissive lighting and a weak neutral moon fill. No engine or shader-source
changes are required. Three red/empty pixel hearts read current HP every frame
and stay at a fixed screen size across zoom and camera movement.
