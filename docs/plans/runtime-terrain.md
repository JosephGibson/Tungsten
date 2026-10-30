# Runtime terrain (runtime-terrain)

- **status:** in progress
- **goal:** The first of redo 7.8's two plans (phase 7's Step 5 and §2.8; the redo's 7.8 row and §5.1's runtime half): the terrain that physics and chemistry stand on, on today's tile grid. Tile properties (material, health, the tile a broken cell becomes, one-way, auto-tile styles) and a collision layer's physics layers, read from the Tiled file; tiles edited in place through the registry, with a journal its consumers read; collision traced from the solid region's outline per 16 × 16 chunk and re-traced locally after an edit; one-way edges; the auto-tiler in Rust; region queries and sensors that follow an edit locally; the outline in the physics overlay; a `terrain` benchmark row with its counter guard. It lands P7-1.
- **non-goals:**
  - The second plan of 7.8, the redo's §5.6 physics on this terrain: friction, contact speeds with begin and end, impulses and radial impulses, sensor exit, slopes, the controller pushed and riding, `Shape` made `#[non_exhaustive]`, the cheaper query index, and every other row Step 5 names (§2.1).
  - 7.12's authoring format, terrain shapes, commands and editor; 7.9's saves, which read §2.4's edits; 7.10's tile statuses; 01 and the template on the new properties (7.13).
  - A `TilemapRegistry::get_mut` (Q3) and `remove` (P7-16, 7.12); slope tiles (the physics plan); 8-bit blob auto-tiling (Q4); seamless joins between two collision layers (Q2); tile cells in contact and sensor events (Q7); dropping through a one-way edge (Q8).
- **files to touch:**
  - Step 1: `crates/tungsten-core/src/assets/tilemap.rs`, `crates/tungsten-core/src/assets/mod.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/assets/tilemap.rs`, the in-crate `TilemapData { … }` literals (`crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/query.rs`), `docs/assets.md`, `api/`.
  - Step 2: `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/query.rs`, `crates/tungsten-core/src/physics/components.rs`, `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/query.rs`, `crates/tungsten-kit/src/overlap/projectile.rs`, `crates/tungsten-kit/src/tests/projectile.rs`, `DESIGN.md`, `api/`.
  - Step 3: `crates/tungsten-core/src/assets/terrain.rs` (new), `crates/tungsten-core/src/tests/assets/terrain.rs` (new), `crates/tungsten-core/src/assets/tilemap.rs`, `crates/tungsten-core/src/assets/mod.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/query.rs`, `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/query.rs`, `DESIGN.md`, `docs/LLM_INDEX.md`, `api/`.
  - Step 4: `crates/tungsten-core/src/assets/terrain.rs`, `crates/tungsten-core/src/assets/tilemap.rs`, `crates/tungsten-core/src/tests/assets/terrain.rs`, `crates/tungsten-core/src/tests/assets/tilemap.rs`, `docs/assets.md`, `DESIGN.md`, `api/`.
  - Step 5: `crates/tungsten-core/src/physics/query.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/query.rs`, `DESIGN.md`.
  - Step 6: `crates/tungsten-core/src/physics/step/outline.rs` (new), `crates/tungsten-core/src/tests/physics/outline.rs` (new), `crates/tungsten-core/src/physics/collision.rs`, `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/pairs.rs`, `crates/tungsten-core/src/physics/step/sleep.rs`, `crates/tungsten-core/src/physics/query.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/query.rs`, `examples/01_platformer/tests/game/guards/pit.json`, any `examples/01_platformer/tests/game/*.rs` the rule in §6 admits, `DESIGN.md`, `api/`.
  - Step 7: `crates/tungsten-core/src/physics/collision.rs`, `crates/tungsten-core/src/physics/step/solver.rs`, `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/step/sweep.rs`, `crates/tungsten-core/src/physics/query.rs`, `crates/tungsten-core/src/tests/physics/step.rs`, `crates/tungsten-core/src/tests/physics/query.rs`, `crates/tungsten-kit/src/tests/platformer.rs`, `docs/assets.md`, `DESIGN.md`.
  - Step 8: `crates/tungsten-core/src/physics/step/mod.rs`, `crates/tungsten-core/src/physics/mod.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten/src/physics_debug.rs`, `crates/tungsten/src/tests/physics_debug.rs`, `api/`.
  - Step 9: `examples/02_bench/src/terrain.rs` (new), `examples/02_bench/src/main.rs`, `examples/02_bench/src/gen.rs`, `examples/02_bench/src/counters.rs`, `examples/02_bench/src/tests/guards.rs`, `examples/02_bench/src/tests/guards/terrain.json` (new), `scripts/perf-rows.json`, `docs/perf/benchmarks.md`.
  - Step 10: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/LLM_INDEX.md`, `ROADMAP.md`, `docs/known-issues.md`.
  - Step 11: this plan (deleted), `ROADMAP.md`, `docs/plans/phase-7.md`.
- **ordered steps:** (1) Tile properties and layer physics. (2) Tiles in their layer's physics layers. (3) Edits in place, the journal and cell health. (4) The auto-tiler. (5) Region queries and sensors on the grids. (6) The outline: traced per chunk, collided and cast against. (7) One-way edges. (8) The outline in the physics overlay. (9) The `terrain` row and guard. (10) The decision and the records. (11) Close.
- **done-when:** every step's done-when passes, quoted in the evidence log; `just check` passes; 01 runs on the outline through a `tungsten run --script` demo, with a `tungsten render` still of its first area showing the outline; `just perf run terrain` is valid; P7-1 reads `landed` in phase 7's table with this plan's part-1 evidence row; the plan deleted with its citations and `runtime-terrain` out of Now.

Item `runtime-terrain` in [ROADMAP.md](../../ROADMAP.md), from [phase 7](phase-7.md)'s Step 5 and §2.8 and the [roadmap redo](roadmap-redo.md)'s 7.8 row, §5.1 and §5.6. Written 2026-10-30 at `64a8a993`. Runs under the [tungsten-plan skill](../../.claude/skills/tungsten-plan/SKILL.md) and the [operating rules](../operating-rules.md), and differs in one way: Step 5 has two plans, so this one writes a part-1 evidence row in phase 7's log at its close and leaves the step's row and its other rows to the physics plan (Q9).

## Context digest

- 7.5 closed on 2026-10-30 with the seam register triaged into phase 7's tables; 7.8 is next on the main track, and phase 7's §2.8 splits it: runtime terrain, then physics with slopes and the controller last.
- Today a tile changes only by cloning its map, `set_tile` and `insert`; each step compares every collision layer whole and, on any difference, rescans every layer of every map, re-staging the pair grid, the query index and every sensor's tile pairs (A2, A3, A7). Tiles are boxes per solid cell with seam face masks (`D-064`), in layer 0 (`D-178`), with no material, health or one-way side (A1, A4, A9).
- It stands on `D-032` (Tiled maps), `D-064` and `D-066` (masks, the gather), `D-154` (sensors and queries), `D-178` (layers), `D-203` (the kept tile proxies and sensor pairs this plan makes local), `D-226` (counter guards in `just check`) and `D-225` (the demo's `run --script` and `render`).
- Later items expect: the physics plan, friction by `material`, slopes on this outline, the controller on one-way edges, events naming a tile cell; 7.9, the edits and health to save; 7.10, cells to put statuses on and blasts that damage them; 7.12, its format on this terrain; 8.2, navigation on the grid with the journal for local repaths.
- The physics pins stand: the determinism and containment piles have no tiles (A16), so this plan moves only scenes with tiles.

## 1. Audit

Read at `64a8a993`: the tile data, the physics paths tiles feed, their consumers, and the rows' evidence.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The loader reads only `sprite_id` from a tileset tile and `kind` from a tile layer; no tile or layer carries material, health, one-way or physics layers | `crates/tungsten-core/src/assets/tilemap.rs:142-176`, `:362-372`, `:434-441` |
| A2 | The registry has `insert`, `insert_with_path`, `get`, `iter`, `ids`, `id_for_path`: no edit in place, so a cell changes by clone, `TilemapData::set_tile` and `insert` (P7-1) | `tilemap.rs:648-689`, `:298-310` |
| A3 | Every step compares each map's collision layers whole with the cache (`layer.tiles == *tiles`, O(cells) a frame); any difference rescans every collision layer of every map and bumps one version | `crates/tungsten-core/src/physics/step/gather.rs:127-176`, `:181-219` |
| A4 | One tile proxy per solid cell: a key hashed from its centre, a face mask of its exposed faces (off-map faces exposed), layer 0 meeting every layer; interior cells (mask 0) are proxies too, though no contact comes from them | `gather.rs:21-26`, `:120-124`, `:231-322`; `crates/tungsten-core/src/physics/collision.rs:116`, `:135` |
| A5 | Masked contacts: the smallest exposed overlap wins, a fully internal cell gives none, a corner gap's normal clamps to the exposed axis; the safety net skips internal faces, the arrival pass reuses the narrow phase, which dispatches by shape | `collision.rs:83-173`, `:275-330`; `step/sweep.rs:157-171`; `step/arrival.rs:275`; `step/solver.rs:127-172` |
| A6 | `Proxy` is 80 bytes, pinned by a test; per-proxy data beyond it lives in parallel arrays (layers, pair flags) | `step/mod.rs:139-172`, `:210-262`; `crates/tungsten-core/src/tests/physics/step.rs:2770` |
| A7 | Every tile consumer keys on the one cache version: the pair grid's tile part, the query index's tile entries and each sensor's kept tile pairs, which a version bump re-tests against every tile under the sensor (that pass was 64 % of 01's step before `D-203` kept the pairs) | `step/pairs.rs:73-87`; `crates/tungsten-core/src/physics/query.rs:303-325`; `step/mod.rs:1035-1060`; `DECISIONS.md:3002` |
| A8 | Casts honour face masks and square a corner beside a covered face; `overlap` lists each overlapped tile as `None`, faces playing no part; `nearest` takes the closest box surface, 0 inside; `SpatialHit` names a tile `entity: None`, with no cell or layers | `query.rs:1-17`, `:149-164`, `:436-557`, `:559-582`, `:780-813` |
| A9 | Tiles are in layer 0 and meet every layer, as `RigidBody`'s and `SpatialFilter::with_mask`'s docs say; the kit projectile hard-codes it to decide whether tiles are passed or bounce | `crates/tungsten-core/src/physics/components.rs:111-122`; `query.rs:120-125`; `crates/tungsten-kit/src/overlap/projectile.rs:627`, `:740-749` |
| A10 | While anything sleeps, every static and tile proxy is a support compared each frame by key, box and layers; a change wakes the islands within the slack (`D-156`) | `step/sleep.rs:62-77`, `:94-125`, `:270-300` |
| A11 | The extract walks each visible render layer's cells every frame, so a written cell draws the next frame; collision layers never draw | `crates/tungsten/src/tilemap_extract.rs:76-120` |
| A12 | Hot and manifest reloads replace a map with `insert` | `crates/tungsten/src/asset_loader/reload.rs:192-229`, `:422-468` |
| A13 | No auto-tiler in Rust; 01's generator picks each terrain sprite in Python by a 4-bit open-side mask (1 up, 2 right, 4 down, 8 left), its variant from a stream seeded by the cell's place | `examples/01_platformer/tools/level.py:368-449` (the mask at `:406`) |
| A14 | 01's map: 430 × 72 cells of 64 px, one collision layer whose 10,093 solid cells use one marker tile, tiles with `sprite_id` only; no runtime tile edit (a test clones and inserts to add an object) | `examples/01_platformer/assets/tilemaps/level.tmj` (by `jq`); `examples/01_platformer/tests/game/saves.rs:4697-4730` |
| A15 | The physics overlay draws `Position` + `Collider` bodies only | `crates/tungsten/src/physics_debug.rs:63-110` |
| A16 | The physics pins have no tiles: the determinism pile and the pinned containment pile stand on static boxes and slabs; the kit's determinism pin has no tilemap; the containment pin case has a tile wall, its two runs compared | `crates/tungsten-core/tests/integration/physics_determinism.rs:43-85`; `physics_containment.rs:109-160`, `:257-272`, `:733-805`; `crates/tungsten/src/tests/kit_recipes/determinism.rs` |
| A17 | 15 benchmark rows, none with tile collision (`gpu`'s tilemap is render-only); guards read `bench:` counters (`counter_min`, `_max`, `_const`, `_eq`, `_band`); 01's pit guard pins the full pit on its tiles | `scripts/perf-rows.json`; `scripts/perf_stats.py:191-222`; `examples/01_platformer/tests/game/guards.rs`, `guards/pit.json` |
| A18 | `TilemapData { … }` literals: the loader and core's tests (`tests/physics/step.rs` ×9, `tests/physics/query.rs:38`, `tests/assets/tilemap.rs:5`); `TilemapLayer { … }`: 20 sites across core, the umbrella, 01 and 02 | `rg -n 'TilemapData \{\|TilemapLayer \{' crates examples` |
| A19 | The LLM index holds 12,229 of its 12,288 bytes | `docs/LLM_INDEX.md`; `scripts/check-agent-context.py:25` |
| A20 | Tests pinning today's tiles: seams (`step.rs:588`, `:629`; `query.rs:118`, `:369`), sensors on tiles (`step.rs:2880`, `:3065`, `:3084`), a removed support (`:3512`), the cache (`:4183`, `:4277`, `:4351`), the kinematic pass (`:2426`) | `crates/tungsten-core/src/tests/physics/` |

## 2. Design

### 2.1 Split and rows

- This plan closes P7-1. The other rows of Step 5 (P7-2 (physics), P7-3 (sensor exit), K7, K14, K23, K30, S-60, S-16, S-26, S-65, S-77, S-99, S-112, S-132, S-172, S-175) are the redo's §5.6 physics and stay open for the physics plan, which reads these: S-26's friction takes a tile's `material` (§2.2); S-175's named layers add names beside §2.2's layer numbers; K14's index keeps §2.7's run boxes; P7-3 and the contact events name a tile through §2.4's `TileHit`.
- Step 5's friction and slope piles are the physics plan's rows; this plan adds `terrain` (§2.9), since each system step adds its row and guard (phase 7 §3).

### 2.2 Tile properties and layer physics

Tileset tile properties, beside `sprite_id`:

| Property | Tiled type | Meaning |
| --- | --- | --- |
| `material` | string, not empty | a name game code reads, and the physics plan's friction |
| `health` | float, finite, > 0 | a cell of the tile breaks when damage brings it to 0 (§2.4) |
| `broken` | string, a sprite ID of the same tileset | the tile a broken cell becomes, empty when absent; needs `health` |
| `one_way` | bool | in a collision layer, solid through its top face only, from above (§2.8) |
| `style` | string | its auto-tile style (§2.5) |
| `mask` | int, 0–15 | a style variant's open sides; needs `style`; a styled tile without one is the style's brush |
| `weight` | int, ≥ 1 | a variant's draw weight, 1 when absent; needs `mask` |

A collision layer's properties, beside `kind`: `layers` and `mask`, strings of comma-separated layer numbers 0–31 (Tiled has no list type), the layers its tiles are in and meet, as `rigid_body`'s lists are (`D-178`): `"0"` and every layer when absent, as today; `mask` `""` meets none. Either on a render layer, a number past 31, a malformed list, a property of the wrong type or outside its range, or a dependency missing fails the load with `TilemapError::Invalid`, naming the tile ID or the layer and the property. Unknown properties stay ignored.

`TileProps` (`#[non_exhaustive]`, `Default`; `broken` as a `TileIndex`) per tileset entry and `LayerPhysics { layers, mask }` per layer live in private `TilemapData` fields, read through `tile_props(tile)` and `layer_physics(layer)` and written through `set_tile_props` and `set_layer_physics`, which check the rules above and panic as the builder's setters do; a tile or layer they lack reads the defaults, so a map built in code and a bulk write to the public fields behave as today.

### 2.3 Tiles in their layer's physics layers

- A collision layer's tiles are in its `layers` and meet its `mask`: the gather gives its tile proxies those layers (`layered` set when any layer is off the defaults, `gather.rs:120-124`) and the index its entries (`step/mod.rs:500-520`), so the pair rule, the safety net, the arrival pass, a sensor (a tile in a layer its mask holds) and a filter's `with_mask` treat a tile as a static body in those layers. The sleep record's supports carry layers already.
- `SpatialHit::layers`: the layers of the body or tile met. `SpatialFilter::without_layers(mask)`: the filter, skipping what is in any layer `mask` holds, beside `with_mask`'s intersection (`query.rs:131-146`).
- The kit projectile treats a tile as it treats a body (`projectile.rs:787`): a wall in any of its `pass_layers` is passed. One cast over tiles `without_layers(pass_layers)` finds its stop (a bounce when `bounce_layers` holds a layer of the tile), and, when `pass_layers` is not empty, one over tiles `with_mask(pass_layers)` finds the tile it passes, its start overlap read under that mask; `TILE_LAYERS` goes. A map without the properties meets shots as today.
- The docs that put tiles in layer 0 change with it: `RigidBody`'s and `SpatialFilter::with_mask`'s rustdoc, the query module's, the kit's `bounce_layers` and `pass_layers` field docs, `DESIGN.md`'s layers paragraph.

### 2.4 Edits in place

On `TilemapRegistry`, in a new `crates/tungsten-core/src/assets/terrain.rs`:

- `set_tile(id, layer, col, row, tile) -> bool`: false for a map it lacks; otherwise the data's `set_tile` checks and panics, the cell's damage cleared, the auto-tiler run (§2.5), each written cell journaled.
- `damage_tile(id, layer, col, row, amount) -> TileDamage` (`#[non_exhaustive]`): `Unbreakable` for an empty cell or a tile without `health`; `Damaged { health }`, the health left, stored sparsely by cell; `Broken { tile }` at 0, the cell set to its tile's `broken` (or empty) through `set_tile`. A non-finite or non-positive amount changes nothing.
- `health(id, layer, col, row) -> Option<f32>`: a breakable cell's health, its tile's `health` until damaged.
- `cursor(id) -> Option<MapCursor>` and `changes_since(id, &MapCursor) -> Option<TileChanges<'_>>`: `Unchanged`; `Cells`, an iterator over the `CellChange { layer, col, row }`s journaled after the cursor, oldest first, a cell possibly repeated; or `Replaced`, after an `insert`, past the journal's bound (4,096 entries a map, Q6), or for a cursor whose fingerprint at its revision is not this map's (another registry's, or before a replace). The fingerprint covers the whole map but its objects: a 64-bit mix of the sizes, each layer's name, kind and physics and the tileset's sprites and properties, fixed at `insert` (nothing changes them in place), XORed with a mix of (layer, col, row, tile) per non-empty cell, updated per write, each journal entry keeping the one after it. Equal fingerprints mean equal maps, so a cursor from another history reads `Unchanged` only for a map that is the same; a cursor holds a revision and that fingerprint.
- `edited_cells(id)` and `damaged_cells(id)`: each cell the game wrote since the map's `insert` (by `set_tile` or a break, the auto-tiler's re-picks of neighbours not among them) with its last tile as written, a brush staying a brush, and each damaged cell with its health, in (layer, row, col) order, for 7.9's saves: replaying them through `set_tile` and `damage_tile` on the authored map rebuilds the terrain, auto cells and their neighbours' variants included (§2.5).
- `insert` and `insert_with_path` start a map's history again: a new revision and fingerprint, an empty journal, no damage or edits. `TilemapData::set_tile` stays the raw builder. A hot reload is an `insert`, so it drops a map's runtime edits (Q10).
- `SpatialHit::tile: Option<TileHit>` (`TileHit { map, layer, col, row }`, `#[non_exhaustive]`, `map` the `TilemapInstance`'s entity) for a tile a cast or `nearest` meets: the cell met, which step 6 keeps (§2.7).
- The step's tile cache keeps each map's cursor and rescans only a map whose `changes_since` is not `Unchanged`, or that was added, moved, resized or removed; the rest keep their proxies, and the per-frame whole-layer compare goes (A3). Step 6 makes the rescan local to chunks.

### 2.5 The auto-tiler

- A tile with `style` and no `mask` is that style's brush; with a `mask`, a variant. Any tile layer may hold them. A brush cell resolves to a variant of its style for its open sides: a side is closed when the neighbour holds any tile of the same style or is off the map (01's rule, A13, but the map's top edge closed too), the mask summing 1 up, 2 right, 4 down, 8 left for the open ones. Among that mask's variants one is drawn by weight from a 64-bit mix of (layer, col, row), so a cell keeps its variant while its mask holds; no variant for the mask draws the brush.
- `TilemapData` remembers which cells are auto (a bit per cell of a layer that held brushes). An authored variant or an unstyled tile is an override: it counts for its neighbours' masks and is never re-picked.
- `TilemapData::auto_tile` resolves every brush; `load` and `insert` call it, so brushes never reach the registry. The registry's `set_tile` with a brush makes the cell auto and resolves it; any write then re-picks the cell's auto neighbours whose masks changed, each change journaled. Clearing a cell re-tiles the four around it.
- Two brushes for one style fail the load.

### 2.6 Region queries and sensors on the grids

- The cache keeps each collision layer's cells as of its last refresh, as it keeps them today to compare (A3). `overlap`, `nearest` and the sensor pass read tiles from those cells, so they see the last step's tiles as they see its bodies: the cells under the query box, each tested as a box exactly as today. Order and results stay today's: bodies, then tiles by map in query order, layer, row and column, then sensors. The index's tile part serves casts alone, ready for step 6.
- A sensor whose identity, box and mask are the last pass's keeps its tile pairs and re-tests only the cells this step's refresh changed (all of a map that was replaced, added, moved or resized): a cell newly solid under it sends `SensorEntered`, a cleared one drops its pair. Any other sensor tests every cell under it, as today.

### 2.7 The outline

- **Runs.** Per collision layer and 16 × 16 chunk (Q5), today's per-cell face masks (a face exposed when its neighbour is off the map, empty or one-way) become collision boxes: a cell whose exposed faces are all top or bottom joins its row neighbours of the same mask into one box, and one whose exposed faces are all left or right joins its column neighbours of the same mask, each run broken at a chunk border and where its cells' `material` or `one_way` changes; a cell exposed on both axes (a corner) keeps its own box; an interior cell (no exposed face, so no contact today, A4) has none. Each box keeps its cells' mask, so every masked face is backed by a solid cell, as on today's cells. Masks and runs read the chunk's cells and a one-cell apron around it.
- **Why runs (Q1).** A run's box is the union of its cells, with their face lines, their masked faces and their centre along the exposed axis, so the masked contacts (`collision.rs:83-173`, `:275-386`), the safety net, the arrival pass, a cast's start overlap and its corner squaring (`query.rs:895-929`) give for it the values its cells give, at any size and depth: a body meets the same set of distinct contacts as on today's cells, and a cast the same first hit. `Shape` stays `Copy` with no primitive added, as the redo's §5.1 asks. The half-cell slabs of the first draft squared corners no solid backs (§7, 2.1); the physics plan's slopes bring a diagonal primitive, and flat runs may move onto segments then.
- **Hit point and ties.** The flat list runs in each box's first cell's (map, layer, row, column) order, today's tile order. A cast's hit on a run is taken against the first of its cells the cast touches in that order, and a tie between two tile hits goes to the lower `TileHit` in that order, the cell's rank where today's tie took the cell's index, so the hit's `point` (`query.rs:953`), its `TileHit` and every tie (`query.rs:584-601`) are today's, a cast meeting a floor and a wall at one instant included.
- **What moves.** One contact per run where a body spans several cells, so fewer contacts and events and other solver sums: tile scenes' trajectories and 01's pit pin (step 6). An edit wakes the sleepers on the runs it changes, up to a chunk wide, where today it woke those on the cell (`D-156`).
- **Keys.** A box's key mixes (map, layer, first cell, last cell, mask), bit 63 clear, so an unchanged box keeps its warm starts across edits.
- **The cache.** Per map, per collision layer, per chunk: its boxes and keys, and the layer's cells (§2.6), updated cell by cell from the journal. `Cells` re-trace each changed cell's chunk and the chunks within one cell of it, deduplicated and sorted; a map replaced, added, moved or resized re-traces whole; a removed one drops. The flat list is rebuilt and the version bumps, so the pair grid and the index restage over the boxes.
- `PhysicsBuffers::terrain_traces()`: chunks traced since the buffers were made, a diagnostic the guards read. The per-cell gather stays under `#[cfg(test)]` as the oracle.

### 2.8 One-way edges

- A one-way cell (`one_way` on its tile) is solid only through its top face, from above. Its neighbours count it as empty for their masks (§2.7); its own mask holds its top face when the cell above is empty or off the map, plus each side where the next cell is not one-way, so its corners round as a corner cell's do, and one with its top covered has no box. One-way cells in a row join into runs as other cells do. The one-way bit is bit 4 of the step's face mask (`FACE_ONE_WAY`, crate-private; the public `FACE_*` and masked functions ignore it), so `Proxy` stays 80 bytes.
- **Contacts.** The narrow phase admits a one-way box's contact only when its normal pushes the body up and the body's bottom lies above the top face within `4 · linear_slop`; the contact build (`step/mod.rs:796`) keeps it within a quarter cell while the pair carries an impulse from the last substep, so a resting or stacked body does not sink through. The safety net takes a one-way box only through its top face, for a body whose previous bottom was above it; the arrival pass inherits the narrow phase's rule.
- **Casts.** A ray or shape meets a one-way box only moving down, its start's bottom at or above the top face, with a hit normal pointing up (the top face or a rounded top corner); a start overlap never hits one. `overlap`, `nearest` and sensors leave one-way cells out: they are surfaces, not solids.
- The platformer controller moves by casts (`D-158`), so it jumps up through a one-way edge and lands on it; dropping through waits for the physics plan's controller (Q8).

### 2.9 Overlay, row and guard

- **Overlay.** `PhysicsBuffers::terrain_faces()` yields each collision box's exposed faces as world segments (start, end, outward normal, one-way flag); the physics overlay draws them, one-way faces in the sensor colour, beside the bodies.
- **The `terrain` row** (`examples/02_bench/src/terrain.rs`): a seeded cave map in a collision layer and a styled art layer (16 generated variants and a brush), bodies (circles) falling on it, and each frame `edits` cells of the surface broken on a seeded walk, in both layers, and the cells broken 60 frames before refilled, so the solid count holds; a body below the map respawns above it, so the row never settles; four sensors the map's height. Presets: `default` (512 × 192 cells of 16 px, 1,200 bodies, 32 edits) and `min` for the guard (64 × 32, 60 bodies, 4 edits, one sensor). `bench:` counters `edits`, `traced` (chunks traced that frame) and `awake`. The registry row owns `system.physics_step` p50 and p95 and `stage.update` p95; its guards are `edits` `counter_const` and `traced` `counter_max` at 8 × `edits`, since each of a frame's 2 × `edits` collision writes re-traces at most the four chunks within one cell of it.
- **The guard** `guards_terrain` (`min`): the bodies and edits asserted first, then a 60-frame window pinned in `guards/terrain.json`; core's locality test (step 6) holds the exact chunk count.

### 2.10 What stays

- Scenes without tiles step bit for bit as today: the physics pins hold (A16).
- On a tile map every body meets today's distinct contacts and every cast today's first hit, its distance, normal, point and `TileHit` (step 6's oracles); `overlap` and `nearest` give today's results; the render path and its counters are untouched; 01's saves keep their schema.

## 3. Steps

Each code step also passes `cargo fmt --all -- --check` and `just lint` → exit 0, and a step that changes the public surface runs `just api`, then `just api-check` and `just docs-ratchet` → exit 0. A test named here exists in the file listed (`rg -q 'fn <name>\(' <file>` → exit 0) and runs under the filter given.

### Step 1: Tile properties and layer physics

- **Files:** step 1's list.
- **Change:** §2.2: the loader reads the tile and layer properties with their rules; `TileProps`, `LayerPhysics`, their accessors and setters; the loader's and the in-crate literals take the new fields; `docs/assets.md`'s tilemap row names the properties. Nothing reads them yet. Sets `status: in progress`.
- **Done-when:** `cargo test -p tungsten-core --lib --locked assets::tilemap::` → exit 0, with `tile_properties_load_from_the_tileset`, `a_collision_layers_physics_layers_load_from_its_properties`, `each_tile_property_rule_fails_the_load_naming_the_tile` and `a_map_built_in_code_takes_tile_properties_and_layer_physics` in `crates/tungsten-core/src/tests/assets/tilemap.rs`; `cargo test -p tungsten-core --test integration --locked manifests::` and `cargo test -p tungsten --lib --locked asset_loader::` → exit 0; `just repo-check` → exit 0.
- **Commit:** `feat(core): read tile properties and a collision layer's layers`
- **Moves:** none.

### Step 2: Tiles in their layer's physics layers

- **Files:** step 2's list.
- **Change:** §2.3: tile proxies and index entries in their layer's layers and mask; `SpatialHit::layers`; the kit projectile on the hit's layers; the docs that put tiles in layer 0.
- **Done-when:** `cargo test -p tungsten-core --lib --locked physics::` → exit 0, with `a_tile_layer_meets_bodies_by_its_layers_and_mask` in `tests/physics/step.rs` and `filters_see_tiles_by_their_collision_layers` (`with_mask` and `without_layers` on tiles of two layers and of one layer in two) in `tests/physics/query.rs`; `cargo test -p tungsten-kit --lib --locked overlap::projectile::` → exit 0, with `shots_meet_tiles_by_their_layers` (a tile in a pass layer passed, one in a bounce layer bounced, and a tile in layers 0 and 1 under `pass_layers` 0 passed, the wall in layer 2 behind it stopping the shot) in `crates/tungsten-kit/src/tests/projectile.rs`; `cargo test -p tungsten-kit --locked` and `cargo test -p tungsten --locked` → exit 0; `just repo-check` → exit 0 (the kit components snapshot).
- **Commit:** `feat(core): put tiles in their collision layer's physics layers`
- **Moves:** none (default layers keep every scene).

### Step 3: Edits in place, the journal and cell health

- **Files:** step 3's list.
- **Change:** §2.4: the registry's edits, cursors, journal, fingerprints, health and edit lists; `TileHit` on hits; the cache's per-map refresh by cursor in place of the whole-layer compare; `DESIGN.md`'s tilemap section; the LLM index's tilemap row gains `core/assets/terrain.rs`.
- **Done-when:** `cargo test -p tungsten-core --lib --locked assets::` → exit 0, with `set_tile_journals_each_written_cell`, `damage_breaks_a_cell_into_its_broken_tile`, `a_cursor_from_another_history_reads_replaced` (another registry's map with the same cells but another layer mask, layer kind or tile property reads `Replaced`; one with the same content reads `Unchanged`), `a_journal_past_its_bound_reads_replaced` and `edited_and_damaged_cells_list_what_changed` in `crates/tungsten-core/src/tests/assets/terrain.rs`; `cargo test -p tungsten-core --lib --locked physics::` → exit 0, with `an_edited_cell_collides_at_the_next_step` and `an_edit_rescans_its_map_alone` in `tests/physics/step.rs`, `the_tile_cache_matches_a_fresh_scan_after_every_change` extended to registry edits, a break and a replaced registry of equal and of different content, and `a_hit_names_the_tile_cell_it_met` in `tests/physics/query.rs`; `just ctx` and `just repo-check` → exit 0.
- **Commit:** `feat(core): edit tiles in place with a journal and cell health`
- **Moves:** `D-203`'s rescan, from every map to the changed map.

### Step 4: The auto-tiler

- **Files:** step 4's list.
- **Change:** §2.5, with `docs/assets.md` (`style`, `mask`, `weight`) and `DESIGN.md`.
- **Done-when:** `cargo test -p tungsten-core --lib --locked assets::` → exit 0, with `brushes_resolve_to_their_masks_variant_at_load`, `variants_draw_by_weight_seeded_by_place`, `an_edit_retiles_only_its_auto_neighbours`, `a_mask_without_a_variant_draws_the_brush`, `two_brushes_for_one_style_fail_the_load` and `a_replay_of_the_edits_rebuilds_auto_cells` (a brush and an explicit variant painted, `edited_cells` replayed on a fresh copy of the map: the same cells and auto cells, and a neighbouring edit after it re-tiles both copies alike) in `tests/assets/terrain.rs`; `cargo test -p tungsten --lib --locked tilemap_extract::` → exit 0 (the extract untouched); `just repo-check` → exit 0.
- **Commit:** `feat(core): auto-tile styled tiles by their open sides`
- **Moves:** none.

### Step 5: Region queries and sensors on the grids

- **Files:** step 5's list.
- **Change:** §2.6, with `DESIGN.md`'s queries and sensors sentences.
- **Done-when:** `cargo test -p tungsten-core --lib --locked physics::` → exit 0, with `overlap_and_nearest_read_tiles_from_the_grid_as_before` (a brute force over every solid cell on 200 seeded maps and queries) in `tests/physics/query.rs`, `kept_sensor_tile_pairs_are_what_testing_every_tile_finds` extended with registry edits, and `a_sensor_retests_only_the_cells_an_edit_changed` (a test-only count) in `tests/physics/step.rs`; `cargo test -p tungsten-kit --locked` → exit 0.
- **Commit:** `feat(core): keep sensors' tile pairs across tile edits`
- **Moves:** a sensor's re-test after an edit, from every cell under it to the changed ones.

### Step 6: The outline: traced per chunk, collided and cast against

- **Files:** step 6's list.
- **Change:** §2.7, in one step, since a tracer the step does not call yet would fail `just lint` on dead code: the tracer as a function of a chunk and its one-cell apron (`FACE_ONE_WAY` beside the face bits, set on one-way cells' boxes); the cache by chunk, re-traced from the journal; run boxes in the step; the index's tile part, hit points and `TileHit` from them; `terrain_traces`; the per-cell gather kept as the test oracle; the existing tile tests adapted where they count proxies or keys; 01's pit pin rewritten with `TUNGSTEN_GUARDS_WRITE=1`, its old and new counts in the evidence; `DESIGN.md`'s physics section.
- **Done-when:** `cargo test -p tungsten-core --lib --locked physics::step::outline::` → exit 0, with `outline_fixtures_trace_their_runs` (a cell, an L, a step, a one-cell pillar, a floor across a chunk border, a material change, one-way cells beside a wall, over a solid cell and stacked, the map's border, a block's interior) and `every_exposed_face_is_on_exactly_one_box` (on 200 seeded 40 × 40 maps: the boxes' exposed faces are the per-cell masks' exposed faces, each once; each box's cells share its mask; no box holds an interior cell) in `crates/tungsten-core/src/tests/physics/outline.rs`; `cargo test -p tungsten-core --lib --locked physics::` → exit 0, with `run_boxes_give_the_cells_contacts` (on 200 seeded maps of 16 and 64 px cells with mixed materials and no one-way cells, which today's cells do not know, boxes and circles of 0.1 to 2.5 cells at any depth, against every box their own box overlaps within the margin: the same set of distinct normals and penetrations, compared as values, as today's cells give), `an_edit_retraces_its_chunk_and_neighbours_only` (exact chunk counts for a chunk's interior, edge and corner cell) and `a_box_slides_a_tile_floor_across_chunk_borders_without_a_bump` in `tests/physics/step.rs`, `the_tile_cache_matches_a_fresh_scan_after_every_change` comparing the incremental boxes with a fresh trace, and `casts_on_run_boxes_give_the_cells_hits` (on the same maps, rays, and boxes and circles of 0.1 to 2.5 cells, starting outside and inside the terrain: the same first hit, its distance, normal, point and `TileHit`, as today's cells give) in `tests/physics/query.rs`; `cargo test -p tungsten-core --test integration --locked physics_` → exit 0; `just physics-release` → exit 0 (the determinism and containment hashes unchanged); `cargo test -p tungsten-kit --locked`, `cargo test -p tungsten --locked` and `cargo test -p example-01-platformer --locked` → exit 0.
- **Commit:** `feat(core): collide and cast against tile outlines`
- **Moves:** contacts with tiles (one per run, fewer events), so tile scenes' trajectories and 01's pit pin; every 01 test changed by §6's rule, listed with its old and new value.

### Step 7: One-way edges

- **Files:** step 7's list.
- **Change:** §2.8, with `docs/assets.md` (`one_way`) and `DESIGN.md`.
- **Done-when:** `cargo test -p tungsten-core --lib --locked physics::` → exit 0, with `a_body_lands_on_a_one_way_edge_and_passes_it_from_below` (a ball dropped, one launched up through, a box moving sideways through) and `a_stack_rests_on_a_one_way_edge` (ten boxes, 600 steps, none below the face) in `tests/physics/step.rs` and `casts_meet_a_one_way_edge_from_above_only` in `tests/physics/query.rs`; `cargo test -p tungsten-kit --lib --locked motion::platformer::` → exit 0, with `the_controller_jumps_up_through_a_one_way_edge_and_lands_on_it` in `crates/tungsten-kit/src/tests/platformer.rs`; `just physics-release` → exit 0; `cargo test -p example-01-platformer --locked` → exit 0.
- **Commit:** `feat(core): add one-way tile edges`
- **Moves:** none (no map has `one_way`).

### Step 8: The outline in the physics overlay

- **Files:** step 8's list.
- **Change:** §2.9's overlay.
- **Done-when:** `cargo test -p tungsten --lib --locked physics_debug::` → exit 0, with `the_overlay_draws_the_tile_outline` (a floor and a one-way ledge: one line per exposed face of each box, the one-way ones in the sensor colour) in `crates/tungsten/src/tests/physics_debug.rs`.
- **Commit:** `feat(debug): draw the tile outline in the physics overlay`
- **Moves:** what the physics overlay draws over a tile map.

### Step 9: The `terrain` row and guard

- **Files:** step 9's list.
- **Change:** §2.9's row, its registry entry and guard pin, and `docs/perf/benchmarks.md` (the row's section, the guard table, Tab cycling six).
- **Done-when:** `cargo test -p example-02-bench --locked tests::guards::` → exit 0, with `guards_terrain` in `examples/02_bench/src/tests/guards.rs` and its pin written once with `TUNGSTEN_GUARDS_WRITE=1`; `just perf list` → lists `terrain`; `just perf-test` → exit 0; `just perf run terrain --repeat 1` → exit 0, valid, both guards ok (local, GPU and display); `just repo-check` → exit 0.
- **Commit:** `test(bench): add the terrain row and its counter guard`
- **Moves:** none (a new row and pin).

### Step 10: The decision and the records

- **Files:** step 10's list.
- **Change:** with the tungsten-decision skill, one entry at the next free ID (recheck it and `git status` first): the format (§2.2, §2.5), edits through the registry's journal and cursors (§2.4), collision as the outline's runs by chunk, re-traced locally (§2.7), the one-way rule (§2.8), sensors and region queries on the grids (§2.6); it amends `D-032`, `D-033`, `D-064`, `D-066`, `D-178` and `D-203`, each with its marker line and index row. The LLM index's tilemap and physics rows cite it. Q2, Q4 and Q8's lines go to ROADMAP.md's Later; a limit the run met goes to known issues.
- **Done-when:** `rg -c '^## D-22[8] — ' DECISIONS.md` → 1 (a later ID if that one is taken, the patterns changed to match); `rg -c '^\*\*Amended by D-22[8]:\*\*' DECISIONS.md` → the number of decisions the entry amends, at least the six named; `cargo test -p tungsten-core --test integration --locked decision_index` → exit 0; `just ctx` and `just repo-check` → exit 0.
- **Commit:** `docs(decisions): record the runtime terrain`
- **Moves:** none.

### Step 11: Close

- **Files:** step 11's list, and any follow-up's home.
- **Change:** the checks and the demo; P7-1 `landed <sha>` in phase 7's §2.2 and its part-1 evidence row (Q9); follow-ups to known issues or Later; the plan deleted with its citations; `runtime-terrain` out of Now.
- **Done-when:** `just check` → exit 0; `just physics-release`, `just api-check`, `just docs-ratchet`, `just perf-test`, `just ctx`, `just repo-check` and `git diff --check` → exit 0; `just smoke` → passes and `just perf counters` → ok (local); from `examples/01_platformer`, `tungsten run --script` on a demo script (`start_run`, the player walked across the clearing to frame 300, a probe asserting it stands on the ground in "Lantern clearing") → exit 0 with a passing report, and `tungsten render --after start_run --at 640,2240 --view 288 --overlay physics --frame 120 -o <evidence>/area.png` → exit 0, the still showing a line on every exposed face and none inside; `just perf run terrain --repeat 1` → valid; `rg -q 'runtime-terrain\.md' --glob '!docs/plans/archive/**' .; test $? -eq 1` → exit 0.
- **Ends with:** the commits, the evidence, what the owner should try (01 as before; `TUNGSTEN_BENCH=terrain TUNGSTEN_OVERLAYS_ON=physics cargo run -p example-02-bench`, the holes opening and the outline following), the next prompt: the physics plan's, tungsten-next's `step` prompt for 7.8's second half.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The redo's §5.1 asks for one-sided segment chains; collision as runs of today's masked boxes instead, or a segment primitive now? | Runs (§2.7): contacts and casts keep today's values, so tiles change only by contact multiplicity, where two critique rounds found half-cell slabs breaking today's corners; the physics plan's slopes bring the diagonal primitive, and may move flat runs onto it |
| Q2 | Faces between two collision layers' cells stay exposed on both sides, as today | Yes; a face that collides only with bodies not meeting the other layer goes to Later |
| Q3 | A `get_mut` for bulk edits in place? | No: it bypasses the journal; `insert` replaces a map, refreshing it alone |
| Q4 | The auto-tiler's mask | 4-bit open sides (16), 01's; the 8-bit blob (47) to Later |
| Q5 | Chunk size | 16 × 16 cells |
| Q6 | Journal bound | 4,096 entries a map; past it, `Replaced` |
| Q7 | `CollisionEvent` and `SensorEntered` naming a tile cell | The physics plan, which reshapes both (P7-2, P7-3) |
| Q8 | Dropping through a one-way edge | The physics plan's controller |
| Q9 | Phase 7's evidence for a step of two plans | This plan's close writes a "5 (7.8), part 1" row; the physics plan's close writes the step's |
| Q10 | A map's hot reload drops its runtime edits | Yes: a reload is an `insert` |
| Q11 | Layer numbers or names in `layers` and `mask` | Numbers; S-175's names (physics plan) add to them |

Each question takes its stated default; the owner reviews the plan on their own time.

## 5. Invariants

- **May move:** the number of tile contacts and events, so tile scenes' trajectories, 01's pit pin and the 01 tests §6 admits (step 6); `D-203`'s rescans and sensor re-tests, by step (3, 5, 6); the sleepers an edit wakes (6); the physics overlay over tiles (8); `e01-*` digests, information only.
- **Must not move:** the determinism and containment hashes (`just physics-release`); the kit's determinism pin; the 02 guard pins but the new `terrain` one; `size_of::<Proxy>()` at 80; the distinct contacts a body meets on a tile map and every cast's first hit (step 6's oracles); `overlap` and `nearest` results (step 5's oracle); the render extract and `just perf counters`; 01's save schema; the visual references.

## 6. Stop conditions

- A done-when check fails: restore the step's files from the last commit, mark it skipped, and continue with the steps that don't depend on it. Step 6 depends on 3; 7 and 8 on 6; 9 on 3, 4 and 6 (its `traced` guard).
- Step 6's contact or cast oracle disagrees anywhere: stop the step; no 01 test is changed to hide it.
- An 01 test fails at step 6: a settled value within the test's own claim is re-derived and listed with its old and new value; a body through a wall, a body caught at a seam, or a failed route or chase stops the step.
- A physics pin moves, or `Proxy` would grow: stop the step.
- The LLM index has no room for step 3's or 10's bytes: tighten the touched rows' words in the same commit (`just ctx`).

## 7. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra` at xhigh, 2026-10-30, reading the tree at `64a8a993` with this draft uncommitted. Footer: `401.2 s, tokens in 451460 (cached 382208), out 8572 (reasoning 5286), commands 18, output 257.5 KB (largest 40 KB), usage 5h 0% → 15%, week 46% → 48%`. Four findings, all accepted; they changed done-when checks, so a second round runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] The slabs cannot meet the cast and contact equivalence: a cell beside a covered face squares its corner, so a radius-18 circle over 16 px cells from (−1, −20) hits at 2 straight up, where a slab rounds the end (20 − √323, angled); at (−1, −17.9) the cells' clamped axial contact goes too | `enter_rounded_box` rounds a corner only with both faces exposed (`query.rs:895-929`); the circle contact clamps a vertex to its exposed axis (`collision.rs:366-380`); worked through, the square reaches past a run's convex end only for a circle wider than a cell, and in the pairs the pair grid admits a box's clamped corner always duplicates a real face's contact | accepted | Round 1 narrowed the slabs' equivalence to circles up to a cell and named the wide circle's ledge as a change; round 2 (2.1) withdrew that with the slabs: runs keep today's corners, the wide circle's included |
| 2 | [HIGH] The projectile's complement mask makes a multi-layer passable tile block shots: the filter tests an intersection, so a tile in layers 0 and 1 under `pass_layers` 0 matches the stop cast | `SpatialFilter::admits` (`query.rs:131-146`); a body wall in any pass layer is passed (`projectile.rs:787`) | accepted | §2.3: `SpatialFilter::without_layers`, the stop cast over tiles without the pass layers, the passed cast with them; step 2's tests take a two-layer passable tile with a wall behind it |
| 3 | [MEDIUM] The fingerprint covers cells only, so another registry's map with the same cells and other layer masks, kinds or tile properties reads unchanged | §2.4's fingerprint as drafted | accepted | The fingerprint covers the sizes, layers' names, kinds and physics and the tileset's sprites and properties too, so equal fingerprints mean equal maps; step 3's test takes another registry's map differing only there |
| 4 | [MEDIUM] `edited_cells` keeps the resolved tile, so a brush cell and an explicit variant save alike and replay as overrides | §2.4 and §2.5 as drafted | accepted | `edited_cells` lists the cells the game wrote with the tile as written, a brush staying a brush, the auto-tiler's re-picks left out; step 4 adds the replay round trip with a neighbouring edit after it |

Round 2: the same reviewer at xhigh, 2026-10-30, reading the same tree with round 1's changes, the Critique section and the log cut and a focus note on round 1's changes. Footer: `514.5 s, tokens in 485099 (cached 421760), out 10278 (reasoning 7585), commands 17, output 195.1 KB (largest 32.8 KB), usage 5h 15% → 31%, week 48% → 50%`. Three findings, all accepted; the run boxes that answer 2.1 had no third round, so step 6's exact oracles and its stop condition are their check.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [HIGH] Half-cell slabs bring phantom ledges for circles under a cell: an isolated 16 px tile's right slab, `[8, 16] × [0, 16]`, squares its top-left corner beside its masked inner face, so a radius-10 circle from (−1, −20) hits at 10 straight up where the tile gives 20 − √99 at an angle, and at (−1, −9) the slab adds an axial contact of 1 | `enter_rounded_box` squares a corner beside any masked face (`query.rs:895-929`) and the circle contact clamps a vertex to the exposed axis (`collision.rs:366-380`); a slab's cut face is masked with no solid behind it past a thin feature | accepted | §2.7: runs of today's masked cell boxes in place of slabs (cells merged only where their masks match on one axis, corners kept single, interior cells dropped), every masked face backed by a solid cell as today, so contacts and casts keep today's values at any size and depth; step 6's oracles compare every size and depth exactly; Q1 records the departure from the redo's segment chains |
| 2.2 | [HIGH] A cast starting inside terrain loses its start overlap: a box below a half-cell slab's centre picks the masked bottom face, so the cast passes into the terrain | The side test by box centre (`collision.rs:108-121`) and the cast's start overlap (`query.rs:646-700`, `:709-748`) | accepted | Answered by the runs, whole cells along their exposed axis; step 6's cast oracle starts casts inside the terrain too |
| 2.3 | [MEDIUM] A merged face moves a box cast's hit point and `TileHit`: a box over x 20–60 on an 80 px floor of 16 px cells hits column 1 at x 26 today, its tie going to the earlier cell, and a merged box at x 40 | Ties by index (`query.rs:584-601`) and the contact point on the shared face (`query.rs:953`) | accepted | §2.7: the boxes run in their first cell's tile order and a hit on a run is taken against the first of its cells the cast touches, so the point, `TileHit` and ties are today's; the cast oracle compares them |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-30 | passed | `assets::tilemap::` 27 passed, the four named tests among them; `manifests::` 5 and `asset_loader::` 54 passed; `cargo fmt --check`, `just lint`, `just api` (+153 lines, `api/tungsten-core.txt`), `just api-check`, `just docs-ratchet` (core 875, no rise) and `just repo-check` exit 0 | `crates/tungsten-core/src/assets/tilemap.rs` |
| 2 | 2026-10-30 | passed | core `physics::` 174 passed, `a_tile_layer_meets_bodies_by_its_layers_and_mask` and `filters_see_tiles_by_their_collision_layers` among them; kit `overlap::projectile::` 13 passed with `shots_meet_tiles_by_their_layers`; `cargo test -p tungsten-kit` 786 and `-p tungsten` 770 passed; fmt, lint, api (+4 core lines, the components snapshot's `bounce_layers` text), api-check, docs-ratchet (no rise) and repo-check exit 0; `physics/mod.rs` needed no change | `crates/tungsten-core/src/physics/step/gather.rs`, `crates/tungsten-kit/src/overlap/projectile.rs` |

## Follow-ups
