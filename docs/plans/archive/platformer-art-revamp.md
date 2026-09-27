---
status: done
goal: "Revamp example 01 with reproducible 64×64 pixel art, a larger branching level, ambient and event particles, and player/environment animation using existing engine capabilities."
non-goals:
  - "No changes to crates/tungsten-core, crates/tungsten-render, or crates/tungsten."
  - "No changes to other examples and no examples/05_showcase directory."
  - "No M31/M32 implementation, new runtime dependencies, engine asset preprocessing, enemy AI, combat/progression system, or new audio production."
files to touch:
  - "docs/plans/platformer-art-revamp.md"
  - "docs/plans/phase4.md — narrow exception amendment after approval"
  - "examples/01_platformer/tools/ — generator, palette, char grids, level source, validation, and regeneration instructions"
  - "examples/01_platformer/assets/manifest.json"
  - "examples/01_platformer/assets/sprites/*.png — generated albedo, normal, emissive, and backdrop images"
  - "examples/01_platformer/assets/animations/*.json"
  - "examples/01_platformer/assets/particles/*.json"
  - "examples/01_platformer/assets/tilemaps/level.tmj"
  - "examples/01_platformer/src/{main,state,setup,systems,extract,gameplay}.rs"
  - "examples/01_platformer/src/level_layout.rs — generated constants and prop/emitter placements"
  - "examples/01_platformer/src/tests/main.rs"
ordered steps:
  - "Obtain approval; amend the Phase 4 exception and mark this plan in progress."
  - "Build deterministic art/level authoring tools and validate the palette, silhouettes, and tile seams."
  - "Generate and commit the complete art roster, animation files, manifest, map, and layout metadata."
  - "Integrate the new grid, movement scale, scrolling camera, depth ordering, and animated props."
  - "Implement player animation transitions and bounded ambient/event particle effects."
  - "Update example tests, run all acceptance checks, and play through both routes."
done-when:
  - "Regeneration is deterministic, all intended generated outputs are committed, and generator validation passes."
  - "All tiles, characters, props, particle sprites, and their auxiliary maps are 64×64; only background/parallax images may exceed that size."
  - "The 128×48 level has traversable lower/upper routes, vertical climbs, gaps, and distinct decoration/foreground depth."
  - "Idle, walk, jump, fall, landing, ball spin, and environment animations play correctly; ambient and event particles have bounded lifecycles."
  - "just check passes, including updated example-01-platformer tests."
  - "cargo test -p tungsten-core --test manifests passes without changing the core test."
  - "just repo-check reports no uncovered asset files."
  - "just smoke passes, including example-01-platformer with TUNGSTEN_LIGHTING_FIXTURE=on."
  - "cargo run -p example-01-platformer runs successfully and manual visual/playability checks pass."
---

## Context and approval boundary

The current example has 64 registered sprites at 32×32, two animations, one black-hole particle config, and an 84×32 map. There are also normal/emissive PNGs and an unregistered legacy 16×16 `player.png`; the resolution migration must cover these too. Grid constants are in `src/state.rs`; setup, movement, extraction, and camera tests also embed scale assumptions. The camera currently fits the entire map height, and custom extraction puts all tilemap layers behind actors. Existing particle configs support bursts, continuous emission, curves, seeded variation, and budgets. Public tilemap data permits example-local layer extraction. No engine change is currently required.

Approved by the user on 2026-09-26, including the scoped `phase4.md` exception. Existing unrelated working-tree changes belong to the user and must be preserved. Approval authorizes the scoped implementation below, not release/publication work.

## Phase 4 exception

This is a deliberate user-authorized exception to `phase4.md`'s M33 hard rule, “no asset production … until M32 is flipped to `status: done`,” and its resolved rule, “Heavy example authoring only in M33.” It neither completes M32 nor starts M33.

**Yes, `phase4.md` needs an amendment.** After approval, read the whole document and add a narrow cross-reference beside the M33 hard rule and heavy-authoring rule: this plan may author assets and gameplay presentation solely in `examples/01_platformer/` before M32. Clarify its asset-preprocessing non-goal to allow this example's offline authoring generator with checked-in outputs. The M32→M33 ordering, M33 acceptance requirements, and prohibition on `examples/05_showcase/` remain intact. The generator does not run during engine startup, Cargo builds, or asset loading. No canonical decision reversal or new runtime dependency is proposed.

## Visual direction and reproducible authoring

Use a **twilight moss-covered ruin**: indigo sky, muted blue mountains, teal foliage, cool stone, and warm amber lanterns. The player is a small hooded lantern courier; existing bouncing balls become animated rune orbs and retain their physics/demo controls. Their artwork follows the character/hero-prop char-grid path; new enemy AI is outside scope. Give traversable surfaces bright, consistent top edges, subdued scenery behind them, and sparse dark foreground silhouettes that never conceal landing edges.

Implement `tools/generate.py` with Python's standard library and Pillow, pinned to locally available `Pillow==12.3.0` in `tools/requirements.txt`. Keep palette, source grids, and level authoring data under `tools/`, outside manifest-scanned `assets/`. Add `tools/README.md` describing format, seeds, source/output ownership, commands, and how to edit art. Split helpers under `tools/` only when they improve readability.

| Asset family | Authoring method | Outputs |
| --- | --- | --- |
| Sky and parallax | Fully procedural palette ramps, dithered bands, seeded noise, and layered silhouettes | Sky plus distant ridges and near woodland; initial sizes 1024×512 for sky and 1024×256 for repeating silhouette strips |
| Terrain | Shared hand-tuned palette and procedural base shapes | 64×64 moss/stone tops, fills, left/right edges, convex/concave corners, isolated ledges, bridge tiles, and at least three texture variants |
| Player and rune orbs | Hand-placed palette-index char grids for key poses and details | 64×64 frames; derive intermediate poses with integer shifts, nearest-neighbor squash/rotation, and palette cycling |
| Hero props | Hand-placed char grids for lanterns, ruin gate, carved markers, and vegetation details | 64×64 sprites or assembled `*_big_<row>_<col>` pieces; animate flame, cloth/foliage, and glow |
| Water and particles | Procedural palette patterns and simple masks | 64×64 waterfall segments, dust, sparks, droplets, and motes; render particles small through config scale |

Each char-grid canvas is genuinely authored at 64×64 with one character per pixel, a transparent symbol, and a validated symbol-to-RGBA palette. Large props use a composed grid whose dimensions are multiples of 64, then split into the existing row/column convention. This is new detailed art, not a nearest-neighbor enlargement of the old PNGs. Nearest resampling and palette quantization preserve hard pixel edges in derived frames. Keep foot/prop anchors stable and prevent frame clipping.

Generate matching 64×64 normal and emissive maps for **every player and orb frame**, deriving auxiliary maps from the same geometry/detail masks and transforming them consistently with albedo. This includes idle, airborne, and landing frames, not just walk. Preserve the fixture's lit-batch route and the ordinary damage-flash material route. Use warm animated pixels for default lantern glow; fixture point lights continue to use existing `Light` APIs.

The generator owns an explicit output inventory and uses stable per-asset seeds, sorted serialization, and fixed PNG settings without timestamps. Add `--check` to regenerate into a temporary directory and compare all owned outputs without rewriting tracked files. Generate the local manifest from the inventory while retaining the existing sound entry. Every new ID starts with `ex10_`; preserve established IDs when their role survives. Repurpose the legacy `sprites/player.png` as the registered idle starting frame at 64×64, eliminating reliance on its coverage exception. Remove obsolete generated assets/references together, never broad-delete the asset directory. Commit PNGs and data alongside the source; no generation prerequisite for running tests or the example.

## Level and scale

**Regenerate `level.tmj` from a hand-authored declarative layout**, rather than hand-editing thousands of GIDs or randomly generating gameplay. Store platforms, terrain regions, route waypoints, set pieces, and prop/emitter placements in `tools/level.json`. Seed only cosmetic variation. Emit both the Tiled file and a small `src/level_layout.rs` containing dimensions, spawn, and typed placement tables with registry IDs. `state.rs` derives constants from this module; game code loads no new filesystem paths. Document that generated files are changed through their source.

Target **128 columns × 48 rows at 64px**, or **8192×3072 world units**. This grows the tile count as well as physical size and remains within D-033's 128×128 tilemap budget.

| Columns | Landmark and route geometry |
| --- | --- |
| 0–23 | Safe lantern clearing, flat spawn apron at surface row 36, then short stepped terraces introducing height changes |
| 24–55 | Broken aqueduct: a broad lower path and an upper bridge/ledge route, with clearly visible gaps and recovery shelves |
| 56–91 | Waterfall ravine: switchback climb from roughly row 36 to row 18, dry ledges, recessed grotto, and distinct near/far decoration |
| 92–127 | Ruined tower and overlook: staggered platforms, routes reconnecting at the lit gate, and an optional high overlook |

Keep a complete primary route with ordinary solid tile AABBs, headroom, and generous landing areas. Use gaps and falls as hazards, with a kill plane below playable geometry and reset to the safe spawn; no new sensor/collision capability is needed. Existing rune orbs provide moving interference. Falling to recovery shelves must allow climbing back. Waterfall art is decorative, not simulated fluid. No slopes, one-way platforms, or Tiled object-layer runtime support are assumed.

Rebuild the embedded tileset from a stable sprite-ID catalog: contiguous local tile IDs, `firstgid = 1`, GID `1 + local_id`, zero for empty, updated `tilecount`, image paths/sizes, and map/tileset tile dimensions of 64. Use explicit variant images rather than unsupported Tiled flip bits. Validate every GID and `sprite_id` reference. Keep `background`, `decorations`, `foreground`, and `collision`; add a `terrain` render layer so foreground can be ornamental while collision stays independent. The collision layer retains its `kind = collision` property.

Proposed scale values, refined only through playability checks:

| Quantity | Derivation / initial value |
| --- | --- |
| `TILE`, `MAP_COLS`, `MAP_ROWS` | 64, 128, 48 from the generated layout |
| `PLAYER_HALF` | `(20, 28)`, fitting a roughly 40×56 silhouette within the 64×64 canvas |
| `PLAYER_SPAWN` | Center of column 6: `(6.5 × TILE, 36 × TILE - PLAYER_HALF.y - 0.5)`, above the safe apron |
| Move / jump / gravity | 560 units/s, 1280 units/s, 3600 units/s²; preserves original tile-relative movement and airtime |
| Ball geometry | Radius 30, visual diameter 64; center extraction by rendered diameter |
| World cleanup bounds | `(-2×TILE, -8×TILE)` through `((MAP_COLS+2)×TILE, (MAP_ROWS+8)×TILE)`; gameplay fall reset occurs earlier |
| Camera | Base zoom `window_height / (18 × TILE)`, existing user multiplier retained; bounds span the whole map and follow works on both axes |

At those movement settings, unobstructed jump rise is approximately 3.56 tiles and same-height travel approximately 6.22 tiles. Initially limit mandatory rises to 2 tiles and gaps to 3; alternate-route rises up to 3 and gaps up to 4 require actual physics validation, especially combined rise/gap jumps. Use waypoint traversal tests and a manual run, not these estimates alone.

Audit every remaining pixel-valued assumption: initial ball positions/speeds, jitter, black-hole radius/force/core size, particle speeds/scales, broadphase cells, lighting radius/orbit, camera shake, cursor display size, and backdrop placement. Express world distances in tile units where useful; do not blindly scale timers or screen-space HUD sizes. Backdrop coverage must be derived from camera bounds, parallax factors, minimum zoom, viewport aspect, and shake overhang, including vertical travel.

## Runtime integration

Keep all new components, tables, and systems in this example. Resolve assets through registries in startup/runtime code; retain existing manifest-root bootstrap constants only. No new asset paths in gameplay. Asset-dependent prop/emitter initialization belongs in the startup callback after manifests load.

The shared `extract_tilemaps` has no layer-selection parameter. Implement a small example-local extractor using public `TilemapData`, `TilemapRegistry`, camera bounds, and `SpriteBatch`; skip collision and cull tiles to the viewport. Preserve layer order and deterministic atlas/filter runs instead of merging across depth boundaries. Draw parallax → background/decorations → terrain → world props and actors/effects → foreground → cursor; order particles within the appropriate stage. Extend the custom extractor to include animated prop entities, which it currently does not render. Retain player bottom anchoring, squash scale, lighting/material selection, and camera-remapped parallax. This uses the existing extract seam, with no renderer API changes.

Minimum animation roster (frame counts are initial targets):

| Registry ID | Frames and behavior |
| --- | --- |
| `ex10_player_idle` | 4, looping breathing/blink/lantern movement |
| `ex10_player_walk` | 8, looping contact/passing poses |
| `ex10_player_jump` | 2, non-looping launch into held rising pose |
| `ex10_player_fall` | 2, looping airborne fall pose |
| `ex10_player_land` | 3, non-looping settling pose |
| `ex10_ball_spin` | 6, looping rune rotation/palette cycle |
| `ex10_torch_flicker` | 6, looping flame and emissive variation |
| `ex10_waterfall_flow` | 4, looping tiled water pattern |
| `ex10_vines_sway` | 4, looping subtle foliage motion |

Add an example-local player presentation state for facing, selected animation, and landing lock time. Select after physics/ground detection: rising jump or falling pose while airborne, a short landing clip on the grounded transition, otherwise walk/idle. Interrupt landing on a new jump; change/reset `AnimationState` only when the selected clip changes. Flip facing in the custom extract without changing the collider; keep the feet stable. Coordinate the landing frames with existing `SpriteSquashStretch` to avoid double squash. Props use existing `AnimationState` plus position/depth metadata, with frame selection updated by the example animation system. For multi-tile animated props, synchronize pieces to one phase.

Preserve the damage-flash tween slot, ball controls, black-hole controls, audio controls, cursor picking, and lighting fixture. Update the declared runtime system order: capture accepted jumps before physics; detect landings after physics; select/tick animations and spawn effects after their triggers; synchronize transforms before presentation; keep squash trigger before squash tick and shake tick before camera update. Respect the existing command/event flush timing and do not register a second particle simulation loop.

## Particle roster and lifecycle

Each row has its own JSON config under `assets/particles/` and local manifest entry. Use existing sprite particles, alpha/color/scale curves, and bounded lifetimes; no M31 mesh particles or new blend modes.

| Registry ID / file | Trigger and initial budget |
| --- | --- |
| `ex10_black_hole` / `black_hole.json` | Retain continuous attraction effect; retune 64px spark scale, retain max 1200 |
| `ex10_landing_dust` / `landing_dust.json` | One 12-particle burst at feet on airborne→grounded transition; lifetime ≤0.5s |
| `ex10_jump_puff` / `jump_puff.json` | One 8-particle burst on an accepted grounded jump; lifetime ≤0.35s |
| `ex10_torch_embers` / `torch_embers.json` | Continuous upward drift at selected lanterns, about 8/s and max 24 per emitter |
| `ex10_waterfall_spray` / `waterfall_spray.json` | Continuous spray at waterfall foot, about 32/s and max 64 per emitter |
| `ex10_wind_motes` / `wind_motes.json` | Sparse distributed ambient streams, about 4/s and max 16 per emitter |

Particles spawn from point emitters in the current API. Distribute several authored mote emitters instead of assuming an area-emission shape. Give placements distinct deterministic seeds. Bound authored ambient emitter counts (target ≤256 ambient live particles total), cap transient burst emitters (e.g. 16), and set an example-level global particle budget of 2048. Do not allow repeated grounded frames or repeated collision contacts to create duplicate landing bursts. Keep short bursts at their event position rather than attached to a moving player.

Tag transient emitters for example-local cleanup after `ParticleEmitterState` reports drained and no active particles remain, respecting the engine's next-frame count refresh. Do not despawn emitters before their first emission tick. Ambient emitters live for the level; particle entities age out through the existing command buffer. Reset presentation/grounding state on respawn so teleporting does not generate a stale jump or landing effect.

## Ordered implementation and validation

1. **After approval:** mark this plan in progress and make the narrow `phase4.md` amendment. Recheck the working tree and public APIs. If implementation needs an unavailable engine capability, stop and report the specific blocker rather than modifying an engine crate.
2. Build the generator and source formats. Validate strict char-grid dimensions/symbols, palette membership, transparent borders/anchors, terrain edge continuity, and derived frame bounds. Preview a contact sheet in temporary output before expanding to the full roster; do not put review-only images in `assets/`.
3. Author the full roster and deterministic level, then generate PNGs, auxiliary maps, animations, manifest, TMJ, and Rust placement data. Validate dimensions, nearest filters, reference coverage, unique IDs across loaded manifests, GID ranges, array lengths, safe spawn, and route/collider agreement. Run generation twice and `--check`; compare every owned output, including metadata and Rust formatting.
4. Integrate layout constants, movement and bounds, scrolling camera, ordered extraction, and animated props. Preserve lighting and gameplay demo features. Add player animation selection and particle triggers/cleanup with the system ordering above.
5. Update `src/tests/main.rs`: replace old fixed coordinates and full-map-height zoom expectations; test safe spawn and real map geometry, horizontal/vertical camera clamps and cursor inversion, traversable route segments with real physics, fall reset, animation transitions/completion/facing, one burst per jump/landing, emitter cleanup/budget behavior, and foreground extraction order. Preserve existing grounding, ball, black-hole, audio, and bootstrap coverage. Add focused generator validation tests under `tools/` for malformed grids, invalid GIDs/references, and deterministic outputs. No edits to core tests or shared QA scripts.
6. Run `python3 examples/01_platformer/tools/generate.py --check`, generator tests, `cargo test -p example-01-platformer`, and `cargo test -p tungsten-core --test manifests`. Finish substantial implementation with `just check`, `just repo-check`, and `just smoke`; explicitly inspect the platformer lighting-fixture result. Report any pre-existing unrelated failure separately without modifying unrelated work. The new local generator tests are a separate command documented in `tools/README.md`; existing `just script-test` does not discover them automatically.
7. Launch `cargo run -p example-01-platformer` and the same command with `TUNGSTEN_LIGHTING_FIXTURE=on`. Traverse both routes and the vertical climb; test deliberate falls, landing/jump effects, idle/airborne lighting frames, prop animation, ball spawning, black holes, damage flash/shake, and audio controls. Inspect default and resized windows plus zoom limits for seams, uncovered edges, foreground occlusion, consistent pixel edges, and particle clutter. Smoke startup alone does not establish these properties. If a GPU/display or interactive verification is unavailable, record the unchecked acceptance items rather than declaring completion.
8. Record actual validation results and any approved deviations here. Once all acceptance items pass, mark done and archive according to `docs/plans/README.md` during implementation handoff. Until approval, leave this document as a draft and make no implementation changes.

## Implementation record — 2026-09-26

The user approved this plan and its Phase 4 exception. Implementation is present
in the working tree. All code and asset changes are confined to
`examples/01_platformer/`; the only documentation changes outside it are this
plan and the approved narrow `phase4.md` amendment. No engine capability blocker,
runtime dependency, M31/M32 work, release or publication was needed. The initial
working tree contained only this untracked plan; unrelated files were preserved.

### Delivered

- Offline Pillow 12.3.0 authoring sources, palette, native character/prop grids,
  declarative layout, explicit ownership inventory and regeneration instructions.
  There are 157 owned outputs: 76 registered sprites, 138 PNGs including auxiliary
  maps, nine animations, six particle configs, manifest, map and formatted Rust
  placement data. The existing OGG and sound registration are retained. The old
  unregistered `player.png` is now the registered 64×64 idle starting frame.
- The 128×48 map has lower/upper aqueduct routes, waterfall switchbacks, a grotto,
  recovery shelves, the tower gate and optional overlook. Rendering/collision share
  authored occupancy; decoration and foreground remain independent. Typed tables
  initialize props and ten ambient point emitters after manifest loading.
- Camera follow/zoom on both axes, viewport-culling tile/prop extraction, ordered
  foreground, parallax coverage from camera bounds/aspect/minimum zoom/overhang,
  player facing and bottom anchoring, all player/orb lit frames, and animated props.
- Jump/landing presentation and event particles, transient cleanup after the
  engine's drained report, 16 transient emitters maximum, 248 authored ambient
  particles maximum and a 2048 global cap. No second particle simulation loop.
  Existing ball/black-hole/audio controls and damage flash/shake remain wired.

### Refinements within the approved scope

Real-physics traversal exposed insufficient grotto recovery clearance; the
layout now includes an additional return ledge at row 30. Both routes and every
recovery/overlook connection pass the waypoint traversal test with the proposed
560/1280/3600 movement values. The tests exercise actual input, engine physics and
the generated collision map; individual segments begin on their named platform.
They do not constitute an uninterrupted human playthrough.

Grounding now reads current contacts, rejects separating launch contacts and
retains support while physics bodies sleep. Material lookup moved to startup,
after the manifests register `damage_flash`. Jump puffs retain the feet position
captured before physics. The 61-pixel art foot baseline stays anchored while the
runtime squash envelope supplies the landing scale; frames do not double squash.
Live inspection prompted a subtler sky ramp and vertically flowing waterfall
streaks whose four frames wrap exactly across both animation and tile boundaries.

### Validation and remaining acceptance

Validation results are recorded below after the final run. The plan remains
`in progress` until the remaining interactive acceptance checks are confirmed;
it has not been marked done or archived merely because automated checks pass.

Observed interactive runs rendered successfully in ordinary and lighting-fixture
modes. Inspected views include the waterfall/climb, default zoom at 1920×1080 and
50% zoom at 3840×2040. The lit airborne courier and rune orbs were visible, with
hard pixel edges, ordered scenery and continuous background coverage in those
sampled views. Review images were kept in temporary output, outside assets.

Still unverified: uninterrupted manual completion of both full routes and the
vertical climb; subjective jump/landing and ambient-effect feel/clutter; every
prop/animation transition across a full playthrough; deliberate fall/reset and
black-hole/damage-flash/shake interactions in live play; audible music/SFX and
volume/stop controls; exhaustive resized-window and zoom-limit visual inspection.
CPU tests cover the relevant geometry, transitions, commands, budgets, extraction
and camera math, but do not replace these manual checks. The optional request for
live-play feedback has not yet produced a result to record.

Final automated results (after the takeoff-origin and waterfall-loop refinements):

| Check | Result |
| --- | --- |
| Generate twice / `generate.py --check` | Pass; all 157 owned files match, including PNGs, metadata, manifest and formatted Rust |
| Local Python authoring tests | 7 passed, including malformed grids/palettes, GID/reference failures, anchors/seams, waterfall loop and all-output byte determinism |
| `cargo test -p example-01-platformer` | 35 passed; final workspace rerun also passed all 35 |
| `cargo test -p tungsten-core --test manifests` | 2 passed, with the core test unchanged |
| `just check` | Pass: formatting, workspace clippy with warnings denied, workspace tests and doctests |
| `just repo-check` | Pass: zero repository errors and no uncovered assets; manifest/index checks pass |
| `just smoke` | All 15 rows passed: four examples and all fixture matrices; platformer lighting fixture explicitly passed with an empty error log |
| Ordinary interactive `cargo run` | Rendered successfully; exited 0 |
| `TUNGSTEN_LIGHTING_FIXTURE=on cargo run` | Rendered successfully; exited 0 |
| `git diff --check` / scope inspection | Pass; no changes outside the example and the two approved plan documents |

There were no unrelated pre-existing validation failures. Changes, including all
generated outputs, remain together in the working tree for handoff. Manual items
listed above remain open, so completion/archive has intentionally not occurred.

## Approved follow-up — gameplay and polish

The user's follow-up authorizes the following example-local work. Existing
uncommitted revamp work is retained. The earlier no-combat/progression non-goal
still excludes enemies/progression; simple hazard damage/respawn is now requested.

1. Extend `src/gameplay.rs` with hazard overlap/swept contact, three-hit player
   health and brief immunity/knockback, moving fire, ball explosions, prescribed
   moving platforms with rider carry, and vortex particle steering. Keep the
   engine's single physics/particle loops and public component APIs.
2. Update `state.rs`, `setup.rs`, `systems.rs`, `extract.rs`, `main.rs`: halve orb
   collision/render size, remove ball damage, add hazard/platform/scene systems,
   animated vortex extraction, light halos, cloud parallax and 35–300% zoom.
3. Extend the offline authoring sources and generated inventory with a 12-frame
   walk cycle, hazard/vortex art, clouds, varied terrain, larger scenery groups,
   new placements and optional moving-platform routes. Retain static traversable
   routes, clear landing edges and a safe spawn.
4. Validate harmless ball contact, damage cooldown/respawn, moving-flame swept
   contact, one bounded explosion per ball, platform carry/jump-off, zoom limits,
   art/loop determinism, route geometry and inventory coverage. Run generator
   tests, example/manifest tests, `just check`, `just repo-check`, and GPU smoke;
   launch and inspect ordinary/lighting modes, recording manual limitations.

All new code and assets remain in `examples/01_platformer/`. No engine or runtime
API/dependency addition is planned. The existing Phase 4 exception covers this
continuation of the platformer's asset and gameplay presentation work.


## Follow-up implementation and validation results

Implemented the approved gameplay/polish follow-up without engine changes or
new runtime dependencies. Balls now have a 15-pixel collision radius and
32-pixel rendered diameter, and never trigger player damage. Six spike traps
and six moving flames damage a three-hit player with immunity, knockback, flash,
shake and safe respawn. Moving fire uses relative swept contact and destroys
balls once, with bounded particle bursts and expanding rings. Three prescribed
platforms carry supported bodies and release jumps through public physics APIs.

The level now has 38 static platform regions, three moving platforms and 299
prop pieces/placements. Lower aqueduct steps, a forest shrine/perch and a wet
recovery shelf add traversal options. New rock/moss variants, slab and bridge
styles, large oak/arch/root groups, small plants and crystals reduce uniformity.
Native scene lights and soft halos illuminate normal-mapped terrain and props.
The background includes a viewport-sized twilight sky, layered ridges/woodland,
and two independently drifting parallax cloud strips. The vortex has rotating
spiral/core layers, inward orbiting sparks and steered particles. The walk cycle
has twelve distinct frames with planted/swing feet, body bob and lantern motion.
Zoom now spans 35–300%.

Final follow-up checks:

| Check | Result |
| --- | --- |
| Regeneration / `generate.py --check` | Pass; 311 owned outputs match, including 150 sprites, 10 animations and 7 particle configs |
| Python authoring tests | 9 passed; deterministic bytes, anchors, distinct walk frames, palette/seams, references and motion/spawn validation |
| Platformer tests in `just check` | 43 passed; includes spike-free route connections, damage/immunity/respawn, harmless ball collision, moving-fire swept contact, bounded explosions, complete platform carry cycle, jump release, terrain/headroom sweeps, animated extraction and 35–300% zoom |
| `just check` | Pass: workspace format, strict clippy, tests and doctests |
| `just repo-check` | Pass: zero errors/uncovered assets; unchanged manifest tests pass (2 tests) |
| `just smoke` | All 15 example/fixture rows passed, including platformer lighting and parallax |
| Ordinary and lighting-mode launches | Both rendered successfully and closed with exit 0; inspected 1920×1080 screenshots for the starting area, background/scenery, platform movement, lighting, player and smaller balls |
| Diff/scope check | Pass; code/assets only under the platformer; only the two approved plan documents changed outside it |

Visual inspection prompted softer halo opacity, higher background layers and
flatter, fainter clouds. Initial clippy findings (wildcard import, integer parity
idiom and helper placement after the test module) were fixed before the passing
workspace check. No unresolved automated failures or missing engine capability
were found.

Remaining manual acceptance: a full human playthrough of both routes and all
moving-fire timing windows; subjective walk/vortex/explosion motion quality in
active play; audible audio controls; exhaustive resized-window/zoom and scenery
occlusion review. Automated route checks reject static spikes but do not prove
a human can complete the entire course against every moving-fire phase. The
pixel comparison suite was not explicitly enabled on a reference machine; GPU
smoke and inspected screenshots are the visual evidence here. The plan remains
in progress for these manual acceptance items. All implementation and generated
outputs remain together in the working tree, with no commit created.

## Approved final polish pass

1. Anchor scenery to named support surfaces using actual pixel bounds; attach
   hanging vegetation/supports to visible platforms and ground the waterfall
   source. Retain one ruin arch, replace repeated arches with rock shelves and
   timber supports, and preserve traversable routes.
2. Generate shallow colliders matching the opaque slab/bridge bounds; remove
   those cells from full-tile collision. Keep solid terrain tile collision and
   share generated collider placement with runtime and route tests.
3. Add the player's warm lantern halo and native light, following facing,
   animation and squash, with an example-local L toggle for both. Add bounded
   fire emitters, stronger warm illumination and an inner flame glow.
4. Validate scenery supports, collision silhouettes/undersides, lantern toggles,
   fire particle budgets, routes, determinism, workspace checks and GPU smoke.
   All code/assets remain example-local; existing working-tree work is retained.

### Final polish results

Scenery now resolves named supports through `tools/placement.py`, using opaque
pixel bounds rather than transparent canvas borders. Composite feet/trunks are
validated against support width. Hanging moss starts at the visible deck
underside; lantern embers follow corrected prop placement. Six repeated arches
became one deliberate landmark, four continuous timber trestles replace floating
masonry columns, and a cliff face backs the waterfall source. Summit stairs and
a grotto stepping stone bring the level to 41 platform regions and 272 prop
pieces/placements.

Thin platforms now use 157 generated static rectangles with a 23-pixel depth,
excluded from full-tile collision. Moving decks share the generated visible
depth. Real-physics regression coverage confirms passage through the previously
blocked transparent area and contact with the visible underside. Main and new
optional route connections remain traversable with static-spike avoidance.

The player's lantern has a visible warm halo and native light; L toggles both.
Generated flame anchors follow each animation frame, facing and squash, and the
preference persists through respawn. Moving flames now have a brighter inner
glow, stronger light and seeded particle trails capped at 40 per fire/240 total,
within the existing global budget.

| Final polish validation | Result |
| --- | --- |
| Generator / byte comparison | Pass; all 315 owned outputs match (153 sprites, 10 animations, 8 particle configs) |
| Python authoring tests | 11 passed, including support anchors and opaque bounds versus collision |
| `just check` | Pass; format, strict clippy and all workspace tests, including 46 platformer tests |
| `just repo-check` | Pass; zero repository errors, manifest and decision-index checks pass |
| `just smoke` | All 15 example/fixture rows passed, including lighting and parallax |
| Ordinary launch | Successful; inspected starting-area scenery/supports and visible player lantern glow |
| Diff/scope checks | Pass; code/assets remain example-local; unrelated working-tree work retained |

The first lantern extraction test placed the player outside its test camera;
that fixture was corrected. Generated fractional coordinates also needed numeric
separators for strict clippy; generation now emits them deterministically. Both
issues are resolved in the passing checks above. No engine changes were needed.
Full human playthrough, subjective fire/animation review, audio, and exhaustive
visual acceptance remain as previously documented. The plan stays open for
those manual checks; implementation remains uncommitted in the working tree.

## Approved midnight and double-jump follow-up

1. Add one fresh-press aerial jump per landing/respawn, with a distinct bounded
   amber burst and jump animation restart; preserve existing ground movement.
2. Retune the palette to dark charcoal, subdued olive foliage and warm highlights.
   Add a separately drawn glowing pixel moon, restrained existing Bloom and
   Vignette post passes, and retain native normal/emissive lighting.
3. Widen the starting clearing from 12 to 16 tiles, move overhead platforms/lift
   away from spawn, reduce nearby scenery clutter and update all support anchors.
4. Replace numeric HP with three camera-independent pixel hearts driven directly
   by Health. Test jump limits/reset/effects, heart state/zoom, opening clearance,
   shader settings, routes and deterministic assets; run full checks and GPU smoke.
   All code/assets remain inside example 01; no engine/API additions are needed.

### Midnight/double-jump implementation and checks

Implemented one fresh-press aerial jump with an 18-spark amber burst, animation
restart, and landing/respawn reset. Holding jump cannot spend the second jump;
third presses are rejected. Existing grounded hold behavior is retained.
The entry clearing grew from 12 to 16 tiles; nearby platforms/lift, support
anchors and plants moved outward, while established routes remain traversable
with single jumps.

Charcoal stone, subdued olive foliage and a darker sky replace the blue-heavy
palette. A separately rendered circular pixel moon has restrained halos, and
stays round across aspect/zoom changes. Stock Bloom (intensity 0.22) and Vignette
(strength 0.18), plus a weak native moon fill, improve highlight separation while
keeping edges readable. Existing normal/emissive and damage material paths are
retained; no shader-source or engine changes were needed. Three red/empty pixel
hearts now read current HP immediately and remain fixed in screen size/position.

| Check | Result |
| --- | --- |
| Generator / `--check` | Pass; 319 owned outputs match |
| Python authoring tests | 13 passed, including courtyard clearance, sky/moon brightness and heart artwork |
| Platformer tests | 49 passed; fresh aerial input, third-jump rejection, one burst, landing/respawn reset, every HP value at all zooms, stock post settings, round moon, and all existing route/physics tests |
| `just check` | Pass: formatting, strict workspace clippy, tests and doctests |
| `just repo-check` | Pass: zero errors, unchanged manifest and index checks pass |
| `just smoke` | All 15 example/fixture rows passed, including lighting/parallax with the new post stack |
| Ordinary GPU launch | Successful; inspected the midnight palette, open clearing, three hearts, readable ledges, lantern and glowing moon at 1920×1080 |
| Scope / diff checks | Pass; code/assets remain inside example 01; only the two approved plan documents differ outside it |

Visual inspection led to raising the moon above the starting-area bridge and
softening its outer halo. The initial test compile used an unavailable read-only
App accessor; the fixture now uses the existing world_mut API. The old no-air-jump
assertion now checks an exhausted aerial charge. All final checks pass. The final
ordinary build is left running for review.

Full human double-jump traversal, subjective in-motion particle/night readability,
audible audio, exhaustive resized visual acceptance and reference-machine pixel
comparison remain unverified. These do not require additional implementation
changes identified by the automated checks. The active plan remains open for
manual acceptance, and all work remains uncommitted together in the working tree.

## Closing note — 0.29.0

Closed at the user's instruction when release 0.29.0 was cut. Every automated
acceptance check above passes and the generator reproduces all 319 owned outputs
byte for byte. The manual acceptance items listed above — an uninterrupted human
playthrough of both routes against every moving-fire phase, subjective motion and
night-readability review, audible audio controls, exhaustive resized-window and
zoom inspection, and reference-machine pixel comparison — remain unverified and
are not claimed by this status change.
