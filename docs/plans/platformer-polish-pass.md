# Platformer polish pass

status: in progress
goal: raise the art quality of `examples/01_platformer/` (first the player and the fireball enemy, then everything else) through its Python generators, and refactor its Rust and Python for clarity, with gameplay, physics, collision and level layout behavior-identical.
non-goals: new gameplay features; anything in `docs/plans/archive/platformer-fire-intensity.md` (burn duration, burning-ball flame layering, `fire_*` frames, burn particle configs); other examples; library crates (none need changes; stop and amend this plan if one does); ID, size or filter changes beyond decisions D1 and D4–D7.
files to touch: `examples/01_platformer/tools/*` (art modules, `palette.json`, grids, `level.py`, `generate.py` wiring, `test_generate.py` imports, `README.md`); regenerated `examples/01_platformer/assets/**`, `src/level_layout.rs` and `tools/outputs.json`; `examples/01_platformer/src/**` except `burning.rs`, `tests/burning.rs`, `tests/ball_pit.rs`; `docs/LLM_INDEX.md` rows 16, 62 and 63.
ordered steps: 0 baseline; 1 audit (recorded below); 2 byte-identical Python split; 3 legacy sprite removal; 4 compact `level_layout.rs`; 5 normal/emissive derivation; 6 player; 7 fireball; 8–12 remaining art; 13 visual review; 14 test split; 15 extract split; 16 systems/gameplay split; 17 setup split; 18 unwrap and dead-code pass; 19 docs; 20 final checks and summary.
done-when: `python3 examples/01_platformer/tools/generate.py --check`, `python3 -B -m unittest discover -s examples/01_platformer/tools -p 'test_*.py'`, `just check`, `just repo-check` and `just smoke` pass; the sorted test-name list equals the step-0 list plus only the tests added here (61 Rust and 15 Python at baseline); gameplay tables and collision mask equal the step-0 snapshot.

## Context digest

- Art is generated from `tools/`. `generate.py` calls `art.build_art()`, which calls `polish_art.enrich_art()`; `polish_art` overwrites some `art.py` outputs (sky, ridges, woodland, walk frames). `level.py` and `placement.py` build `level.tmj` and `src/level_layout.rs` (rustfmt-expanded, 2,325 lines). `outputs.json` is the owned inventory. `--check` regenerates to a temp dir and compares bytes.
- Every pixel must come from `palette.json`. Sprites are 64×64 except the five backdrops; all filters are `nearest`. The `ex10_` ID prefix is legacy and stays.
- Invariants enforced by tests or validation: player foot baseline at 61; transparent border on actor/prop frames; 12 distinct walk frames; the land clip lasts 180 ms, matching `landing_lock`. Platform-style bounding box is exactly `(0,0,64,23)`, so `DECK_DEPTH` and `SLAB_COLLIDERS` stay 23 px. Also enforced: sky mean luminance below 25; moon bounding box `(7,7,57,57)`; waterfall period 32 px with an 8 px/frame offset; 6 distinct lit small-ball frames.
- `LANTERN_ANCHORS` comes from the `y`/`Y` pixels inside x 40–62, y 30–59 of each player frame. Grounded prop `y` comes from each prop's bounding-box bottom.
- The player moves at a constant 560 px/s, so the "walk" clip reads as a run. Runtime squash owns the landing scale envelope. The fire hazard hitbox is 36×48, centred, and set in Rust. The six fire hazards drift at up to ~100 px/s. The `fire_*` frames are shared with burning small balls (fire-intensity work).
- Terrain tiles are lit in the default mode. Balls and the player go through the lit path only with `TUNGSTEN_LIGHTING_FIXTURE=on`. Fire hazards are lit.
- Entity spawn order feeds seeds, glow phases and sort keys (`e.id()`), so refactors must keep spawn and system order exactly.
- Uncommitted ball-pit and fire-intensity work is in the tree. Baseline on it (2026-09-29): `--check` OK (341 outputs), 15 Python tests OK, 61 Rust tests OK.
- Review images are in the session scratchpad: contact sheets, animation strips with frame diffs, a flat level render, live captures, and `proto/fireball_*.png` (a prototype fireball against the current fire).

## Owner direction (2026-09-29)

- **Player:** redo the sprite model entirely, keeping the hooded lantern-courier theme, and make walking and movement more dynamic. This approves D5 and D6.
- **Fireball:** redo it as a dynamic ball of flame that leaks fire particles. This replaces D4's face and prototype: no face. The hazard's `ex10_fire_trail` emitter keeps its ID and is retuned for the fireball, and `ball_burn` keeps its current values by being derived before the retune. A new `ex10_fireball_drips` emitter hangs under each fireball; trail cap plus drip cap is 48 per fire.
- **Order:** steps 6 and 7 run now, ahead of steps 2–5. They land in a new `tools/actors.py`, which is the step-2 target module. Everything else in this plan is still awaiting approval: D1–D3, D7 and steps 2–5 and 8–20.
- **Steps 6–7 done (2026-09-29).**
  - **Player:** new rig with clips idle ×8, walk ×12, jump ×3, fall ×4, land ×3 and double jump ×4, plus the tuck-clip flag in `PlayerPresentation`.
  - **Fireball:** ×10 frames, drawn unlit. It flips to face its travel and stretches with speed. `fire_trail` was retuned, and `fireball_drips` is anchored under each fireball.
  - **Removed:** `fire_dance` and the six player key-pose grids.
  - **Tests:** the only test edit is the fixture rename `fire_dance` → `fireball`. The jump clip was retimed to 200 ms so the existing rise-finish test holds. Two tests were added: the tuck clip, and fireball facing plus drip anchoring.
  - **Unchanged:** every gameplay table, every tile layer and the collision mask. The `ball_burn*` configs are byte-identical.
  - **Checks:** `--check` (386 outputs), 15 Python tests, 63 Rust tests, `just check`, `just repo-check` and `just smoke` pass.
  - **Still to review by playing:** run and jump feel in motion, and fireball particle density at both zoom limits.
- **Owner direction, second pass (2026-09-29):** "totally redesign all the scenery, ground tiles and background assets." Steps 8–10 and 12 are done; step 11 (balls) is not part of this pass.
  - **Modules:**
    - `pixels.py`: shared toolkit, lifted out of `actors.py` with byte-identical output.
    - `terrain.py`: periodic masonry; 8+8 seam-safe variants; darker deep fill; `cliff_left`/`cliff_right` edges; slab, rope-bridge, weathered-bridge and lift decks at exactly 23 px; twin-post X-braced trestle; wet cliff plus a new `cliff_back_top` spring cap.
    - `backdrops.py`: dithered sky with star tiers and a milky band; cratered moon; lobed translucent clouds; cusped three-layer ridges with a keep, curtain wall and spire; two-row woodland. All strips wrap.
    - `scenery.py`: lamp post and flicker; clinging ivy; runed waystone; summit gate with keystone rune and portcullis; oak; ruined arch; root curtains; fern, grass, flowers, glowing mushrooms, rubble, crystals and hanging moss; looping waterfall; spikes within the hitbox span.
    - `polish_art.py` now holds only the hearts, the burning-ball flames and the black-hole and glow sprites. The `gate`, `lantern`, `marker` and `vines` grids were deleted.
  - **D1 resolved by restyling:** no IDs were removed; the legacy names are copies of the new family. New IDs: `cliff_left`, `cliff_right`, `cliff_back_top`.
  - **`level.py`:** exposed wall faces use the cliff tiles, and the variant draw order is kept.
  - **`placement.py`:** the top row of the waterfall column uses `cliff_back_top`.
  - **Normals:** relief normals (luminance as height, no tile tilt) for terrain and `*_big_*` pieces. Most props are now lit.
  - **Palette:** new symbols for masonry, wood, water, crystal and backdrop tones; the wood mid tone is `ω` so `?` stays an unknown symbol for the grid test.
  - **Unchanged:** every gameplay constant, collision and layer occupancy. Only derived prop and torch-emitter y positions moved.
- **Owner direction, third pass (2026-09-29):** "redo both ball assets, make them unique; don't have both hue shift." Step 11 is done.
  - **LMB orb:** a bronze sphere drawn at its 32 px render size, with teal meridians and rune rings rolling on a tilted axis. It has no hue; the untinted fallback is now white instead of magenta.
  - **MMB marble:** neutral-grey glass drawn at 16 px, with a turning cat's-eye ribbon. It is the only ball with `BallHue`.
  - **Source:** both are generated by `actors.build_balls`. `orb.grid` and the dead `grid`/`shift_part` helpers are removed.
  - **Tests:** two assertions changed from "9/5 balls have hue" to "0", because the owner reversed that behavior. A new test covers the hue split and the rendered tint.
- **Owner direction, fourth pass (2026-09-29):** a final asset review, smoother lighting, and whether a shader would help.
  - **Asset sweep:** every manifest path exists and is generator-owned, with no stray files. The only unreferenced sprites are the 16 legacy aliases.
  - **D7 resolved, with a shader:**
    - New example-local `soft_glow.wgsl` material, with `ex10_soft_halo` and `ex10_soft_flame` materials.
    - All glows now draw as analytic, dithered falloffs.
    - `halo`/`flame_glow` gradients are dithered and linear-filtered (fallback and particles).
    - `push_instance` no longer merges into material batches.
    - New tests: Naga validation, and the glow material batch.
  - **Redrawn:** the moon (solid maria, round craters); `dust`, `droplet` and `mote` (fireflies with a blinking config), moved to `effects.py`.
  - **Evaluated and rejected:** stock `GodRays` (glare and a blocky haze) and `Fog` (screen-space top haze).
  - **Engine limit to report:** 16-light packing at the widest zoom.
- **Still awaiting approval:** D2 (partly used), D3 and Rust steps 13–18.
- **Shipped in 0.31.0:** the four owner-directed passes above (steps 6–12, D1 by restyling, D7). Later work from this plan goes under `[Unreleased]`.

## Decisions for approval

- **D1: drop 16 never-placed legacy sprite IDs (24 files).** The IDs are `ground_1..3`, `corner_convex_left/right`, `corner_concave_left/right`, `fill_1/2`, `platform`, `platform_1/2`, `ledge`, `stone_wall`, `bridge` and `stone_platform`. No level, Rust or doc references them; `stone_wall`, `bridge` and `stone_platform` remain in-memory bases for `cliff_back` and the platform variants. The tileset renumbers GIDs; the collision mask is unchanged. Recommended; if rejected, restyle them to match the new terrain instead.
- **D2: add palette entries.** Hot fire ramp; brighter scarf and rim light; trousers and boots; deep stone, wood, bark and moss tips; extra glow alpha steps. Existing symbols keep their RGBA.
- **D3: `level_layout.rs` stays generated** but is emitted with short `const fn` constructors and an identical literal sequence.
- **D4: a dedicated fireball set for the hazard.** Add `fireball_0..7` and the `ex10_fireball` animation. `fire_*` stays untouched for burning balls, and `fire_dance` is dropped because nothing else uses it; the test fixture list renames `"fire_dance"` to `"fireball"`. It gets a face (slit eyes and brows) so it reads as an enemy, and the sprite flips to face its horizontal travel direction; vertical-only movers face the player. This needs a presentation-only change in obstacle extraction. Recommended, with the face; the alternative is a plain fireball.
- **D5: new player frame IDs**: `player_idle_4..7`, `player_jump_2` and `player_fall_2..3`. Existing IDs, clip IDs and the 12-frame walk stay.
- **D6: a dedicated double-jump clip** (`ex10_player_double_jump`: 4 tuck frames, which then hand over to the fall clip). This needs a presentation-only flag in `PlayerPresentation`; no current assertion changes. Recommended.
- **D7: glow sprites `halo` and `flame_glow` switch to a finer alpha ramp and the `linear` filter.** Today they show stepped concentric rings when scaled 2–6×. This also softens the burning-ball glow and flame particles, which is a visual change to the fire-intensity look, not to its behavior. Recommended.

## Steps

### 0. Baseline (done during planning)
Record `rtk proxy cargo test -p example-01-platformer -- --list` and the Python test IDs. Snapshot `SLAB_COLLIDERS`, `DECK_DEPTH`, `HAZARDS`, `MOVING_PLATFORMS`, `PLATFORMS`, `ROUTES`, spawn/kill constants and the `level.tmj` collision non-zero mask. Regenerate review images in the scratchpad, never under `assets/`. Live captures use `TUNGSTEN_CAPTURE_FRAME` with `TUNGSTEN_DISPLAY_RESOLUTION=3840x1080` to include the first fires.

### 1. Audit: weakest art (done during planning)

Player and fireball (priority):

| ID | Problem | Category | Fix |
| --- | --- | --- | --- |
| `player` (all frames) | Coat greens match the moss, trees and woodland, so the figure sinks into the background. The grey boots float below the hem as separate blocks. The hand does not grip the lantern, so the lantern floats. The scarf is a flat rectangle and the backpack an unattached box | readability, silhouette | Lift coat values and add a warm rim light on the lantern side; make the scarf a brighter accent; join trousers and boots to the hem; hand grips the lantern bail; backpack strap across the chest |
| `player`, `player_idle_1..3` | A rigid 1 px bob of the whole block; a blink every 0.96 s; no secondary motion | animation | 8 frames with varied durations: breathing, a lagging lantern sway, scarf flutter, one blink per ~2.4 s cycle |
| `player_walk_0..11` | An upright walk at a 560 px/s run speed, so the feet slide. Legs are 3 px lines; the torso is rigid; hem, scarf and backpack never move | animation | 12-frame run at ~50 ms: forward lean, knee lift, flight frames, 2 px bob; hem and scarf trail; lantern swings as a lagging pendulum; backpack bounces |
| `player_jump_0/1` | The idle body plus a scarf shaped like an "E"; the only change between frames is a 1 px lantern jitter | animation, silhouette | 3 frames: takeoff stretch, rise, apex tuck (held) |
| `player_fall_0/1` | Idle body; the scarf is a straight stick; the loop's only motion is the 1 px lantern jitter | animation, silhouette | 4-frame flutter loop: hem, hood and scarf lifted, arms up, legs dangling |
| `player_land_0..2` | Only the eyes and foot spacing change; the last frame copies idle | animation | Impact (squint, pooled hem, lantern swings down), recover, settle. No vertical compression, because runtime squash owns scale; total stays 180 ms |
| double jump | Restarts the ground-jump pose | animation | D6 tuck clip |
| `fire_0..7` as hazard | A static campfire body in which only the tip moves. The dusty red rim (`r`) reads as a burgundy blob in game; thin slivers appear in frames 0, 1, 5–7; nothing marks it as an enemy; the outer body is not emissive | readability, animation, emissive | D4 fireball: white-hot core, three phased tongues streaming backward, ember underside, face, whole body emissive, inside the hitbox (tongue tips up to 6 px over) |
| `halo`, `flame_glow` | 8 discrete alpha rings, nearest-sampled at 2–6× scale, show as stepped target rings around fires and lanterns | lighting | D7 |

Everything else:

| ID | Problem | Category | Fix |
| --- | --- | --- | --- |
| `rock_0..7` | Variants 0/3/6 use tone `d`, the rest `b`; random placement reads as a checkerboard quilt. Stones are clipped at tile edges, so the 64 px grid shows | palette, readability | One tone family; masonry courses cross edges at fixed seam positions; variation only in the interior |
| `moss_rock_0..7` | Uniform 3 px drips at a 5 px pitch read as a comb or barcode | silhouette | Continuous turf lip with a lit top row; irregular clumps and fine drips; rows 0–5 shared across variants |
| `ground_left/right` | Brick masonry at every platform lip, next to irregular stone | consistency | Derive from the new stone family, with a shaded outer column and turf overhang; fully opaque |
| terrain `_n` maps | Opaque tiles get only the `(x/w-0.5)*0.25` position gradient, so light pools step at tile seams | normal map | Height-from-tone normals with no position term; seam-safe edges |
| `bridge_rope`, `bridge_weathered` | Flat orange boxes under a grass strip; the weathered cracks read as a repeated "7" | readability | Shaded planks, gaps, nails, a side beam or rope rail; weathered: grey, split planks. Bbox `(0,0,64,23)` |
| `slab_carved`, `slab_broken`, `lift_deck` | Diamonds read as UI glyphs; the cracks are invisible; the lift deck is a bridge with glyphs | readability | Bevelled slab with an inset rune groove (only the rune emissive); chipped corners and visible cracks; timber/iron lift deck with rivets. Same bbox |
| `timber_support` | A V-brace repeated per tile stacks into a fishbone | silhouette | Posts with an X-brace per tile, forming a trestle lattice |
| `distant_ridges` | The profile does not wrap, leaving a hard step at every strip seam. Towers read as mailboxes | seam, silhouette | Periodic profile; crenellated, broken towers |
| `near_woodland` | A tree is sliced at column 1023; trees are lollipop circles | seam, silhouette | Trees placed mod 1024 and drawn at x and x±1024; irregular crowns |
| `clouds_far/near` | Flat lens shapes that read as saucers | silhouette | Stacked lobes, flatter base, lit crown, wisps; same alphas |
| `ball_0..5` | 60° nearest-neighbour raster rotation breaks the rim and runes into speckles | animation frames | Static symmetric rim; runes redrawn analytically per angle |
| `ball_small_0..5` | Reads as a donut once tinted at 16 px; the whole rim glows | readability, emissive | Shaded sphere with rotating glints; only the glints emissive; sphere normals for both balls |
| `marker` | The glyph reads as the digit 3 | readability | Edit `marker.grid` to a non-digit rune; bbox unchanged |
| `crystal` | Rounded lozenges read as pickles | silhouette | Angular prisms with facet planes |
| `fern`, `grass`, `hanging_moss` | Dark crosshatch scribbles | readability | Stemmed fronds, light-tipped blades, drooping strands |
| `rubble` | Flat dark blobs | lighting | Stones with lit tops and shadowed sides |
| `oak_big_*` | The trunk reads as a thin orange stick; the crown is lollipop circles | silhouette | Bark trunk with one-sided highlight; clumped crown; feet stay within support validation |
| `dust` | A grey rectangle inside the puff | silhouette | Rounded puff with a shaded underside |

Acceptable as-is: lantern prop, gate, arch, roots, vines, waterfall, sky, moon, hearts, spark, mote, droplet, vortex, cursor.

### 2. Python structure (byte-identical outputs)
Split by domain into `art.py` (palette, grids, canvas and part helpers, `auxiliary`, `validate_art`, `BACKDROPS`, `build_art` orchestration), `actors.py` (player, balls), `terrain.py`, `scenery.py` (props, composites, waterfall, hazards), `backdrops.py` and `effects.py` (particle, HUD and black-hole sprites); delete `polish_art.py`. Remove dead code: `art.backdrop()`, the overwritten 8-frame walk and its `player_contact`/`player_passing` grids, the unused `ImageChops` import, and the duplicate backdrop-name tuples. Replace the `clip()` ternary chain with a first-frame-name map. Write one statement per line, under 100 columns. `particle_configs()` stays untouched. Check: `--check` passes **without regenerating**; the Python tests pass.

### 3. Legacy sprites (D1)
Stop emitting the 16 IDs and move the seam validation to the new terrain family. Check: only those 24 files, their manifest entries and the `level.tmj` GIDs change; the collision mask equals the snapshot.

### 4. Compact `level_layout.rs` (D3)
`const fn` constructors, still through rustfmt. Check: the ordered string/number literal sequence is identical; Rust tests pass.

### 5. Normal and emissive derivation
`auxiliary(im, height=None, emissive=None, dome=True)`. Tiling textures pass a tone-derived height map and `dome=False`; balls get hemisphere normals. The emissive symbol set is chosen per family, defaulting to `yYCc`, so untouched sprites keep byte-identical `_n`/`_e`.

### 6. Player (D2, D5, D6)
Source: part grids in `tools/grids/player/` (hood and torso, scarf states, lantern-arm swing states, leg poses, hem states, eyes). A per-frame composition table in `actors.py` applies integer offsets; no raster rotation. Frames and timing as in the audit. The fall clip hands over from jump and double-jump unchanged in Rust.

The D6 Rust change is confined to `player_presentation_system` and `PlayerPresentation`: the double-jump clip plays while rising after an aerial jump, then fall takes over. Add a test for that clip selection. Invariants: baseline 61; lantern flame pixels stay inside the anchor region (x 40–62, y 30–59), and no other `y`/`Y` pixels fall inside it; walk stays 12 distinct frames; land stays 180 ms.

Review: strips with frame diffs at 6×; a live capture with the fixture both off and on; before/after at 100% and 35% zoom. The `player_lantern…` and `all_player_orb_frames…` tests must pass unchanged.

### 7. Fireball (D4, D7)
Generate `fireball_0..7` (≈70 ms loop) from the scratch prototype, refined with bigger eyes and brows. Emissive covers the whole flame. Keep the core and body inside the hitbox (x 14–49, y 8–55), with tongue tips up to 6 px above. Point `spawn_obstacles` at `ex10_fireball_0` and `ex10_fireball`. Obstacle extraction flips the hazard by the sign of its horizontal motion (`cos` of its motion phase); vertical-only movers face the player. Add a test for the facing rule.

D7: rebuild `halo` and `flame_glow` with the finer alpha ramp and the `linear` filter. The glow, point light and `fire_trail` configs stay as they are.

### 8–12. Remaining art (one step each)
8 terrain (`rock`, `moss_rock`, `ground_left/right`, `cliff_back` tint); 9 platforms and `timber_support`; 10 backdrops; 11 balls; 12 props, `dust`, `oak`. After each step: regenerate, run the Python tests and `cargo test -p example-01-platformer`, and diff the gameplay snapshot; only prop `y` and `LANTERN_ANCHORS` may move. Every platform bbox stays `(0,0,64,23)`, and spikes stay within x 5–59, y 34–62.

### 13. Visual review
Regenerate the contact sheets, the flat level render and live captures at spawn, the fires, the upper aqueduct, the gate and the pit, with the fixture both off and on. Iterate on anything still weak. Record the remaining limits: readability in motion, zoom extremes.

### 14. Rust tests: move only
`tests/main.rs` keeps the shared fixtures and `mod` lines. Tests move to `wiring.rs`, `player.rs`, `camera.rs`, `balls.rs`, `routes.rs`, `presentation.rs` and `hazards.rs`; `ball_pit.rs` and `burning.rs` stay untouched. Check: the sorted leaf-name list is unchanged; no assertion edits.

### 15. Extract split
`extract.rs` keeps the orchestrator and shared helpers (`instance`, `push_instance*`, `view_bounds`, `uv_size`, a facing-flip helper). Submodules:
- `extract/backdrop.rs`: parallax, moon.
- `extract/world.rs`: tiles, props, obstacles.
- `extract/actors.rs`: particles, player, balls, burning flames moved verbatim, vortices.
- `extract/overlay.rs`: hearts, cursor, text.

Replace the hand-built `SpriteInstance` literals, and rebuild `rainbow_rgba` on `hsv_to_rgb` (check with a temporary hue-sweep equality test, then delete it).

### 16. Systems and gameplay split
Replace `systems.rs` and `gameplay.rs` with `input.rs`, `player.rs`, `balls.rs`, `black_hole.rs`, `hazards.rs`, `lights.rs`, `effects.rs`, `camera.rs` and `hud.rs`. `move_obstacles` and `scene_effects` stay the registered systems and delegate to named helpers in their original order; `RUNTIME_SYSTEM_ORDER` is unchanged. Shared helpers: `delta_seconds`, `scene_time`, action reading, `cursor_world_position`, `play_sfx`.

### 17. Setup split
Split `seed_world` into helpers for the post stack, lighting, the player and the seed balls. One ball component set is shared with `spawn_balls`. Entity spawn order and component values stay exact. Startup checks stay fail-fast, with messages.

### 18. Unwrap and dead-code pass
Replace post-query `get_mut().unwrap()` with `query_mut` or `let … else`. Remove the `DeltaTime` unwrap in `animation_system` and the `unwrap` in `push_instance`. Also remove `Vec2X3`, the unconstructed `CycleMode::None` with its `allow`, the stale `OrbitLight` doc and the needless clippy `allow`. `burning.rs` is out of scope.

### 19. Docs
`tools/README.md`: player part grids, fireball, height/emissive rules, glow filter, backdrop wrap rule, D1. Also update the `docs/LLM_INDEX.md` platformer rows. `CHANGELOG.md` is left to release finalize.

### 20. Final checks and summary
Run all done-when checks. The summary lists each art change by ID and each refactor by file with what moved. Set `status: done` and archive the plan. Report the fire-intensity done-when results without changing that plan.
