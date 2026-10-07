# Phase 5 milestone 42: additive extracts (W15c) — draft

- **status:** done
- **goal:** Make the extract additive. The default sprite channel draws tilemaps, then the `Sprite` entities. Engine plugins and games append sprite, quad and text contributions through an umbrella `Extracts` resource, in registration order, and a game replaces the default only explicitly. Example 01 draws its tile layers through the engine's tilemap extract with the same output, and the template's plugin contributes its text through the new path.
- **non-goals:** Ordering tile layers among sprites by z (a backlog row, Q2). Moving example 01's own drawing (parallax, props, player, balls, flames, hearts, cursor) onto `Sprite` entities (Q3). Turning the engine-owned extract pieces (HUD, systems overlay, inspector text, debug draw, lights, mesh particles) into plugin contributions (Q5). `02_bench`'s explicit extracts, its digests and its wiring. Tilemaps at `z_norm` 0 under `gpu_depth` outside the default channel, which is W8a's (Q9). The tile and light cull size (known issues P3). Names or `before`/`after` constraints on contributions (Q6). Examples 03 and 04, whose `set_extract_text` keeps working unchanged (Q8). Anything in `tungsten-core` or `tungsten-render`.
- **files to touch:** Step 1: `crates/tungsten/src/extract.rs` (new), `crates/tungsten/src/tests/extract.rs` (new), `crates/tungsten/src/lib.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/tilemap_extract.rs`, `crates/tungsten/src/tests/tilemap_extract.rs`, `crates/tungsten/src/tests/app.rs` (two tests removed; amended in step 1), `crates/tungsten/src/sprite_extract.rs` (doc comments only), `templates/basic/src/game.rs`, `templates/basic/AGENTS.md`, `api/tungsten.txt`. Step 2: `examples/01_platformer/src/extract.rs`, `examples/01_platformer/src/setup.rs` (a comment), `examples/01_platformer/src/tests/presentation.rs`. Step 3: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/getting-started.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w15-authoring-api.md`, `docs/plans/1.0/backlog-1.x.md`, `crates/tungsten/src/extract.rs` (the module doc's decision ID), `docs/known-issues.md` (only for an out-of-scope finding). Step 4: nothing in the tree (evidence in `perf-runs/`). Step 5: the release step's list.
- **ordered steps:** (1) `Extracts`, the default channel with tilemaps, the tilemap layer form and lit tiles, and the template; stop for the API review. (2) Example 01's tile layers through the engine. (3) `D-138`, docs and records. (4) The capture sitting and the GPU gates. (5) Release 0.56.
- **done-when:** Every step's evidence row is quoted; workflow §5 is closed; the release checks pass after the cut, with `just visual` and the step 4 sitting run by their steps; the preflight prints the command block and the post-merge block.

Candidate W15c ([implementation plan](../../1.0/implementation-plan.md) §3 card, level C), milestone M42, release 0.56. Written 2026-10-07 at `cc3ded9` (0.55 merged, branch `0.56`). Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. Level C: the run stops after step 1 for the owner's review of the API sketch (the `api/tungsten.txt` diff) and the template diff.

## Context digest

W15c is Track C's fifth candidate and the last before the Phase 5 QA pass (amendment 18). Its scope is [w15](../../1.0/w15-authoring-api.md) step 3: the default extract draws sprites and tilemaps, games and plugins append, and a game replaces the default only explicitly. W15a's plugins build with `&mut Schedule, &mut World` (`D-128`), so a plugin can only reach `World`: contributions have to sit in an umbrella `World` resource. Core cannot hold them, because `SpriteBatch`, `QuadInstance` and `TextSection` are render types (`D-007`, `D-016`, `D-018`).

The audit changes w15's example 01 sketch. Example 01 interleaves its tile layers by name with its own drawing: the parallax backdrop, then background and decorations, then back props, then terrain, then the world, then foreground, then hearts and cursor (A7). Its parallax entities and the engine's particles are `Sprite` entities that it draws itself (A8). A default followed by appended contributions therefore cannot reproduce its frame. Example 01 keeps an explicit replace and calls a new engine function, `extract_tilemap_layers`, at its three depths. For that to draw the same frame, the engine's tilemap extract must honour lit atlases as example 01 does (A4, A9). The `integrated` bench row shows the same need (A11) and keeps its own extract. Tiles at `z_norm` 0 would hide the default's sprites under `gpu_depth` (A16), so the default channel writes its tiles at the far plane.

Decisions cited: `D-018` and `D-128` (the extract after the stages), `D-042` (the default sprite extract, which `D-138` amends), `D-061` (lit wins), `D-086`, `D-113` and `D-114` (the extract's buffers, IDs and culling), and `D-135` (`resource_mut`). Later candidates expect `Extracts` as the way a plugin draws: W13's kit, W16's prefabs and the acceptance game through the template.

## 1. Audit

Read at `cc3ded9`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `App` holds three `Option` extract closures. Each `set_extract_*` overwrites its slot. `install_default_extracts` fills only the sprite slot with `extract_sprites_default`, and only when it is empty, so tilemaps are never in the default. `App::run` and `Harness::new` call it | `crates/tungsten/src/app.rs:90-92`, `:373-386`, `:418`, `:446-450`; `crates/tungsten/src/testing.rs:73` |
| A2 | The three closure types are public aliases, `ExtractQuadsFn`, `ExtractSpritesFn` and `ExtractTextFn` | `app.rs:64-71`; `api/tungsten.txt:61-63` |
| A3 | `stage_extract` calls each slot, then composes the HUD, systems overlay and inspector text, debug draw, lights and mesh particles itself. These pieces are engine-owned and no setter reaches them | `app.rs:981-1075` |
| A4 | `extract_tilemaps` draws every render layer, map by map. It makes one batch per atlas page per layer, keyed by the atlas alone, and builds its batches with `SpriteBatch::new`, so `lit` is false whatever the asset. Its instances carry `z_norm` 0, and it culls at `WindowSize` | `crates/tungsten/src/tilemap_extract.rs:78-202`, `:160-179`, `:190`, `:89-97` |
| A5 | The default sprite extract sets `lit` from `asset.lit_atlas.is_some()`. Lit wins over material (`D-061`) | `crates/tungsten/src/sprite_extract.rs:328-342` |
| A6 | The loader gives a sprite a lit atlas only when it has a normal map, so one page can hold both lit and unlit sprites | `crates/tungsten/src/asset_loader/atlas.rs:299-303` |
| A7 | Example 01 replaces the sprite and text extracts. Its sprite extract composes parallax, `["background", "decorations"]`, back props, `["terrain"]`, world props, obstacles, vortices, particles, vortex cores, player, balls, flames, fireballs, `["foreground"]`, hearts and cursor. Its `extract_tile_layers` sorts maps by entity ID and keeps contiguous runs keyed on atlas, filter, lit and material, with `lit` from the lit atlas | `examples/01_platformer/src/setup.rs:636-637`; `examples/01_platformer/src/extract.rs:381-608`, `:386-389`, `:566`, `:277-334`, `:115-139` |
| A8 | Example 01's parallax layers are `Sprite + Transform + Visibility + ParallaxLayer` entities. Its own `extract_parallax` draws them with a stretched sky, drifting clouds and tiling. Engine particles spawn with `Visibility` and a `Sprite`, and the example draws those itself too. The default would draw both a second time, differently | `setup.rs:448-452`; `extract.rs:188-270`, `:394-425`; `crates/tungsten/src/particles.rs:200`, `:536` |
| A9 | Example 01's tile sprites have `normal_map` and `emissive_mask`, so its tile batches are lit. `level.tmj` has the layers `background`, `decorations`, `terrain`, `foreground` and `collision`, with one tileset | `examples/01_platformer/assets/manifest.json` (`ex10_arch_big_0_0`, …); `examples/01_platformer/assets/tilemaps/level.tmj` |
| A10 | The presentation test asserts the composed layer order (`[1, 2, 6, 3, 7, 5, 4, 8]`) and culls through `crate::extract::extract_tile_layers` | `examples/01_platformer/src/tests/presentation.rs:333`, `:337` |
| A11 | Examples 03 and 04 and the template set only text, so they draw through the default sprite extract, and none of them spawns a `TilemapInstance`. Every `02_bench` row sets its own sprite extract: `gpu` draws tilemaps then the default, and `integrated` splits the default at the parallax page to put the tilemap between | `examples/03_scene_state/src/main.rs:117`; `examples/04_shader_playground/src/main.rs:230`; `templates/basic/src/game.rs:36`; `examples/02_bench/src/gpu.rs:446-450`, `integrated.rs:405-420`, `particles.rs:429`, `physics.rs:278-282`, `ecs.rs:202-206`, `churn.rs:184-188` |
| A12 | `App::with_plugins` inserts the engine's resources, `ExtractScratch` among them, before it builds the plugin set. `Plugin::build` takes `&mut Schedule, &mut World` | `app.rs:231`, `:279`; `crates/tungsten-core/src/schedule.rs:487-489` |
| A13 | The template's `GamePlugin` builds its systems, and `register` adds the plugin and then calls `set_extract_text`. Its test checks that the frame's text holds a section in the game's font | `templates/basic/src/game.rs:23-37`; `templates/basic/tests/game.rs:149-170` |
| A14 | `FrameDraw::sprites` and `Harness::new`'s docs describe "the default extract unless the app set its own" and "the default extracts installed" | `testing.rs:60-73`, `:251-256` |
| A15 | `just visual` covers `02_bench`'s `gpu-visual.png` (tilemaps, then the default), the post-stack test in 04 and the transition test in 03, but none of them is byte-level. `gpu-visual` allows 2 per channel. The post and transition tests compare captures of one build with each other. The bench workloads are deterministic, and their `TUNGSTEN_CAPTURE_*` screenshots of a pixel-identical build are byte-equal. 03 and 04 run on the wall clock. No pixel test covers example 01; its harness tests seed the real `level.tmj` without GPU atlases | `justfile:45-48`; `examples/02_bench/tests/visual_regression.rs:26-38`, `:66`; `examples/03_scene_state/tests/transition_regression.rs:58`; `app.rs:1603-1621`; `examples/01_platformer/src/tests/main.rs:146-170` |
| A16 | Tilemaps at `z_norm` 0 cover every sprite under `gpu_depth` (W8a's card). The depth test is `LessEqual` against a 1.0 clear, and default sprites take `z_norm` in [0, 1), the last drawn at 0. With tilemaps in the default, a game with a map and `gpu_depth` would reach this bug without writing an extract | `docs/plans/1.0/implementation-plan.md:80`; `tilemap_extract.rs:190`; `sprite_extract.rs:89-91`; `crates/tungsten-render/src/sprite.rs:1052-1056` |
| A17 | `ExtractBuffers::tile_batch_lens` is one capacity hint shared by every tilemap call in a frame. Three layer calls overwrite each other's hints; this affects performance only | `sprite_extract.rs:164-165`; `tilemap_extract.rs:169-170`, `:198-199` |
| A18 | No `02_bench` row draws through the default channel, so the rows' extract work is unchanged and only the stage's dispatch changes. `stage_extract` is frame-loop code, so every CPU row is captured (workflow §5 item 2) | A11; `app.rs:981` |
| A19 | `tungsten` has no `missing_docs` lint, so rustdoc on new items is checked per file. The `docs/LLM_INDEX.md` index is 10,721 B of its 12,288 B budget. The W15c stop's `plan` stage recommends Opus 5.5 at max, not Fable 5.1, so the critique runs at its default effort | `crates/tungsten/src/lib.rs`; `just ctx`; `scripts/roadmap.py catalog` |

## 2. Design

### 2.1 API sketch (reviewed after step 1)

```rust
// crates/tungsten/src/extract.rs: `tungsten::extract`, `Extracts` re-exported at the crate root.

/// A `World` resource `App` always inserts: the frame's sprite, quad and text
/// channels. Each channel is its default, or the replacement a game set, then
/// the contributions in the order they were added.
pub struct Extracts { /* three private channels */ }

impl Default for Extracts { /* sprites: tilemaps, then `extract_sprites_default`; quads and text: nothing */ }

impl Extracts {
    pub fn add_sprites(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static);
    pub fn add_quads(&mut self, f: impl Fn(&World) -> Vec<QuadInstance> + 'static);
    pub fn add_text(&mut self, f: impl Fn(&World) -> Vec<TextSection> + 'static);
    /// Replaces the channel's default; contributions stay. The last call wins.
    pub fn replace_sprites(&mut self, f: impl Fn(&World) -> Vec<SpriteBatch> + 'static);
    pub fn replace_quads(&mut self, f: impl Fn(&World) -> Vec<QuadInstance> + 'static);
    pub fn replace_text(&mut self, f: impl Fn(&World) -> Vec<TextSection> + 'static);
}

// crates/tungsten/src/tilemap_extract.rs, re-exported beside `extract_tilemaps`.
/// The named render layers of every tilemap, map by map, each map's layers in
/// file order; a name no map has draws nothing; collision layers never draw.
pub fn extract_tilemap_layers(world: &World, layers: &[&str]) -> Vec<SpriteBatch>;

// App: `set_extract_quads`, `set_extract_sprites` and `set_extract_text` keep
// their signatures and forward to `replace_*` (Q1).
```

A plugin draws through `world.resource_mut::<Extracts>().add_sprites(f)` (`D-135`'s accessor, since `App` always inserts the resource). A game with an `App` uses `app.world_mut()`, or the plugin form.

### 2.2 Channel semantics

- **Order.** Each channel is its base, the default or the replacement, followed by its contributions in registration order. `App::with_plugins` inserts `Extracts::default()` beside `ExtractScratch` before building the plugin set (A12). Engine plugins therefore contribute before a game's plugins, and `App::set_extract_*` replaces the base whenever it is called.
- **Defaults.** The sprite default is `extract_tilemaps`, then `extract_sprites_default`: tiles under every sprite, the `gpu` row's order. It sets its tile instances' `z_norm` to 1.0, the far plane. Under `gpu_depth` a tile then passes `LessEqual` against the 1.0 clear, and every default sprite (`z_norm` < 1) still passes over it (A16). Under the CPU sort there is no depth attachment, so the value changes nothing. With no map it returns the sprite extract's vector unchanged. Quads and text have no default; the engine-owned pieces still follow the channels in `stage_extract` (Q5).
- **Frame.** `stage_extract` reads `Extracts` through `get_resource` once a frame and calls each channel with `&self.world`, a shared borrow, as the slots were called. A channel with no contributions returns its base's vector without copying it. Without the resource, which only a removal causes, the channels draw nothing (Q7). `FrameExtract`, recycling and `RenderCounts` are unchanged.
- **Depth.** A contribution's instances keep the `z_norm` its closure wrote; the channel never renormalizes across sources. Under the CPU sort, order is what draws. Under `gpu_depth`, an instance at `z_norm` 0, the value every custom extract in the tree writes, passes `LessEqual` against anything drawn before it, so an appended contribution draws over the default. A nonzero `z_norm` is the caller's depth and meets the default's in the depth test. The rustdoc and `D-138` say so (Q12).
- **Replace.** `replace_*` and the `App` setters replace only the base, and contributions stay. A game that wants nothing else drops the plugin that contributes, as with any engine feature (`D-128`).
- **Gone.** The three `App` fields and `install_default_extracts` are removed, and `Harness::new` no longer installs anything. The three `Extract*Fn` aliases stay as the channels' boxed types.

### 2.3 Tilemap extract

- `extract_tilemaps` and `extract_tilemap_layers` share one body with an optional layer filter. The filter is checked once per layer, never per tile. The all-layers path runs the same tile loop as before.
- A tile's `TileSprite` carries `lit = asset.lit_atlas.is_some()`, and a layer's batch lookup keys on `(atlas, lit)` (A6). Each batch's `lit` is set from that key, so a tile with a normal map draws on the lit pipeline as a sprite does (`D-061`; Q4). An unlit tile keeps today's batches and bytes.
- Both public functions keep `z_norm` 0. A custom extract that calls them still meets W8a's bug, and only the default channel's own pass raises its tiles to 1.0 (§2.2, Q9). That pass touches no bench row. Culling stays at `WindowSize` (known issues P3).

### 2.4 Example 01

- `extract_sprites` keeps its composition and its explicit replace (Q3). Its three `extract_tile_layers` calls become `tungsten::extract_tilemap_layers` with the same names, and `extract_tile_layers` and its tile imports go.
- Same output: within one layer, tiles sit on distinct grid cells, so their draw order changes no pixel. Layer order is the same on both paths. Instance bytes, texture, filter and `lit` match per tile once §2.3 lands. Step 2 checks all three on the real level (A15).
- Map order: the example sorts maps by entity ID while the engine follows query order. The example has one map.

### 2.5 Template and guide

- `GamePlugin::build` adds the text: `world.resource_mut::<Extracts>().add_text(text)`. `register` only adds the plugin. `main.rs` and the tests are unchanged, since the text test already reads the frame's text (A13).
- The template's `AGENTS.md` gains one rule: draw through `Sprite` entities and tilemaps, which the engine draws by default; add the game's own batches, quads and text in its plugin through `Extracts`; replace a default only on purpose.
- The guide gains a short "Drawing" section with the same rule and the plugin snippet (step 3, after `D-138`).

### 2.6 Seam and surface

- Only the umbrella changes. Core and render are untouched, so their snapshots must not move. The extract still reads `&World` after the stages (`D-018`, `D-128`).
- New surface: `tungsten::extract` (`Extracts`, `Default`, six methods), `extract_tilemap_layers` and the two root re-exports.
- Nothing is removed, so no compile break. Two behaviour changes take ledger rows: the default sprite channel draws tilemaps, and tile batches go lit for lit tiles.

## 3. Steps

### Step 1: `Extracts`, the default channel, the tilemap layer form and the template

- **Files:** `crates/tungsten/src/extract.rs` (new), `crates/tungsten/src/tests/extract.rs` (new), `crates/tungsten/src/lib.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/tilemap_extract.rs`, `crates/tungsten/src/tests/tilemap_extract.rs`, `crates/tungsten/src/tests/app.rs` (amended), `crates/tungsten/src/sprite_extract.rs` (docs only), `templates/basic/src/game.rs`, `templates/basic/AGENTS.md`, `api/tungsten.txt`
- **Change:** In this order:
  1. The tilemap extract per §2.3, with tests: `layer_form_draws_named_render_layers_in_map_order`, `lit_tiles_batch_apart_from_unlit_on_one_page` and `every_render_layer_name_matches_extract_tilemaps`.
  2. `Extracts` per §2.1 and §2.2, each item with rustdoc.
  3. `app.rs`: the insertion, the forwarding setters and their docs, `stage_extract`, and the removals.
  4. `testing.rs`: `Harness::new` and the `FrameDraw` docs. `lib.rs`: the module and the re-exports.
  5. `tests/extract.rs` on the harness:
     - `default_draws_tilemaps_then_sprites`. A 2×1 map and one sprite: `draw().sprites` equals `extract_tilemaps` with its tiles' `z_norm` set to 1.0, followed by `extract_sprites_default`, as draw lists. With no map it equals the sprite extract alone, byte for byte.
     - `default_tiles_stay_under_sprites_under_gpu_depth`. A map under two overlapping sprites, on the default channel: every tile instance has `z_norm` 1.0, both sprites have less, and the later-drawn sprite has the lower value.
     - `contributions_follow_the_default_in_registration_order`. Two test plugins each add a sprite, quad and text contribution in `build`, and then the game adds one of each. Sprites come out as the default's, A's, B's, then the game's. Quads and text come out as A's, B's, the game's, with the HUD off. Each contribution's instances come out byte for byte as its closure wrote them; one of them writes a nonzero `z_norm` to show the channel keeps it.
     - `replace_keeps_contributions`. `set_extract_sprites(f)` alone gives only `f`'s batches. With A present it gives `f`'s, then A's.
     - `last_replace_wins`.
  6. The template per §2.5.
  7. `just api`, then the checks.

  A "draw list" is the flattened sequence of (texture, filter, lit, material, override bytes, instance bytes), so batch boundaries do not count.
- **Done-when:**
  - `cargo test -p tungsten --lib extract` → passes, with the eight new tests named in its output.
  - `cargo test -p tungsten-template-basic` → passes, and `git diff --quiet -- templates/basic/tests templates/basic/src/main.rs` → exit 0.
  - `rg -n 'set_extract' templates/basic` → nothing. `rg -n 'add_text' templates/basic/src/game.rs` → one line, inside `GamePlugin::build`.
  - `rg -n 'install_default_extracts|extract_(quads|sprites|text): Option' crates/tungsten/src` → nothing.
  - `just api`, then `git diff --stat -- api/` → only `api/tungsten.txt` changes. `git diff -U0 -- api/tungsten.txt | rg '^-pub'` → nothing. The added lines show `tungsten::extract::Extracts` with its six methods and `Default`, `extract_tilemap_layers`, and the two root re-exports.
  - `cargo rustdoc -p tungsten --lib -- -W missing-docs 2>&1 | rg 'src/(extract|tilemap_extract)\.rs'` → nothing.
  - `just visual` → passes, within the tests' own tolerances (A15). The byte-level evidence is elsewhere: for 03, 04 and the template, the no-map case above (the renderer gets the sprite extract's own batches); for the bench, step 4's captures. `git diff --quiet -- examples/02_bench/tests/fixtures` → exit 0.
  - `just check` → passes.
- **Moves:** none
- **Then:** stop for the API review. Show the `api/tungsten.txt` diff, §2.1 as built, and `git diff -- templates/basic`.

### Step 2: Example 01's tile layers through the engine

- **Files:** `examples/01_platformer/src/extract.rs`, `examples/01_platformer/src/setup.rs` (the comment above `install_runtime`'s two setters), `examples/01_platformer/src/tests/presentation.rs`
- **Change:**
  1. **Equivalence first.** Before removing anything, add a temporary test to `presentation.rs` and run it. It uses the real `level.tmj`, with the tileset's sprites registered on two synthetic atlas pages that alternate by tileset index, and a lit atlas on every third sprite. It runs at three camera positions: spawn, mid-level, and zoomed out over the whole map. Write `ex` for the example's `extract_tile_layers` and `en` for `tungsten::extract_tilemap_layers`. It checks:
     1. For each function, the draw list of `["background", "decorations"]` equals the concatenation of the two single-layer draw lists.
     2. For each of the four render layers, the sorted draw lists of `ex` and `en` are equal, and no two instances in `en`'s list share a position.
     3. It records whether the unsorted draw lists are also equal per layer.
  2. Copy the test and its output to `perf-runs/<date>-m42-additive-extracts/step2/`, then remove the test from the tree.
  3. Make the change from §2.4. `presentation.rs:337` calls `tungsten::extract_tilemap_layers`. The `setup.rs` comment explains the explicit replace: the parallax layers and particles are `Sprite` entities the example draws itself.
- **Done-when:**
  - The temporary test's output, quoted, shows checks 1 and 2 passing at all three positions, and check 3's result.
  - `rg -n 'Tilemap|LayerKind|tileset|fn extract_tile_layers' examples/01_platformer/src/extract.rs` → nothing.
  - `rg -n 'extract_tilemap_layers\(' examples/01_platformer/src/extract.rs` → three lines, with the same layer names as before.
  - `rg -n 'extract_tile_layers' crates examples templates` → nothing.
  - `cargo test -p example-01-platformer` → passes. `git diff -U0 -- examples/01_platformer/src/tests/presentation.rs` → only the function path changes, and `[1, 2, 6, 3, 7, 5, 4, 8]` stays.
  - `just smoke` → passes, example 01's plain, lighting and parallax rows among them.
  - `git status --porcelain -- examples/01_platformer` → the three files.
- **Moves:** none

### Step 3: `D-138`, docs and records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `crates/tungsten/src/extract.rs` (its module doc cites `D-138`), `DESIGN.md`, `docs/getting-started.md`, `docs/LLM_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w15-authoring-api.md`, `docs/plans/1.0/backlog-1.x.md`, and `docs/known-issues.md` only for an out-of-scope finding
- **Change:**
  1. `D-138` through [tungsten-decision](../../../../.claude/skills/tungsten-decision/SKILL.md). Recheck the free ID first. It records the approved API, amends `D-042` choice 4, and records Q2–Q9 as answered.
  2. Then the docs:
     - `DESIGN.md`: the render-path and default-sprite-extract paragraphs and the tilemaps section. Not the status line, which is the release's.
     - The guide's "Drawing" section.
     - `docs/LLM_INDEX.md`: `tungsten/extract.rs` and `D-138` join the default sprite extract's row.
  3. Then the records:
     - Two ledger rows (§2.6), landed in 0.56.
     - w15 step 3's landed note and its "Decisions" line.
     - A backlog row for tile layers by z (Q2).
- **Done-when:**
  - `rg -n '^## D-138' DECISIONS.md` → one line. ``rg -n '^\| `D-138`' docs/DECISION_INDEX.md`` → one line. `rg -n 'Amended by D-138' DECISIONS.md` → `D-042`'s marker line.
  - `rg -c 'Extracts' DESIGN.md docs/getting-started.md` → at least 1 in each.
  - `rg -n 'tungsten/extract.rs' docs/LLM_INDEX.md` → one line.
  - `rg -n 'M42' docs/plans/1.0/w04-api-freeze.md` → two rows reading landed (`D-138`).
  - `rg -n 'D-138' docs/plans/1.0/w15-authoring-api.md docs/plans/1.0/backlog-1.x.md` → at least one line in each.
  - `just ctx`, `just repo-check` and `just api-check` → pass.
  - `just check` → passes.
- **Moves:** none

### Step 4: The capture sitting and the GPU gates

- **Files:** none in the tree. Evidence goes in `perf-runs/<date>-m42-additive-extracts/`.
- **Change:** Follow the M38/M39 sitting shape:
  1. Check that `pgrep -af scripts/bench.py` shows no capture, that `nxcodec.bin` is absent, and that free disk is over 10 GB.
  2. Baseline: `git archive cc3ded9 | tar -x -C target/m42-base`. Candidate: the tree.
  3. Prebuild four binaries: plain, and aligned with `-C llvm-args=-align-all-functions=6 -C llvm-args=-align-all-nofallthru-blocks=6` in their own `CARGO_TARGET_DIR`. Rerun each build to show `Finished`, and use `nm -C <bin> | rg -c 'extract::Extracts'` to tell them apart: 0 on the baseline, at least 1 on the candidate.

     Prebuild a fifth plain binary, the channel probe, from a scratch copy of the candidate (`target/m42-probe`; no tree file changes). In that copy, `examples/02_bench/src/gpu.rs` drops its `set_extract_sprites` call, so the row draws through the default channel: the same tiles and sprites, plus the far-plane pass and the dispatch. It also adds two sprite contributions, one returning an empty vector and one returning a single one-instance batch. The probe's `nm` count of `extract::Extracts` is at least 1, as on the candidate.
  4. One background sitting that waits for 60 s without the encoder, then runs six suites with `--repeat 5`: base, cand, base-again, cand-again, base-aligned, cand-aligned. It logs other sessions' PIDs and CPU, and the `git status --porcelain` count before and after.
  5. After the six suites, the same sitting runs `gpu-throughput` (`--repeat 5`) on the candidate, then on the probe. Compares: direct (base → cand), direct-again, A/A base, A/A cand, aligned, and probe (cand `gpu-throughput` → probe), then a summary script.
  6. Gates as one job, with an encoder check before and after: `just smoke`, then `just visual`, then byte-level captures. The captures use the plain binaries, each run from its own tree's root with `visual_regression.rs`'s environment (`TUNGSTEN_SMOKE_FRAMES=8`, `TUNGSTEN_CAPTURE_FRAME=5`, `TUNGSTEN_CAPTURE_RESOLUTION=1280x720`). They cover `TUNGSTEN_BENCH=gpu` with `TUNGSTEN_BENCH_PRESET=visual`, and `TUNGSTEN_BENCH=integrated` at its default preset, the two rows that call `extract_tilemaps`. Each is captured twice from the baseline and once from the candidate.
  7. Delete the scratch directories.

  Drift list (the owner's earlier rulings and the profiling workflow's A/A note): `gpu-throughput` `render_encode` p50/p95; the `ecs` per-run-mode rows (`stats_decay`, `regen`, `follow`, `buffs`, `team_bags`, `accelerate`, `integrate`); `particles` `animate_sprites` p95 and `particle_count_refresh` p50/p95; `gpu` `extract` p95.
- **Done-when:**
  - Every suite exits 0, every compare is comparable, and the digests match 8 of 8 in each.
  - `gpu` and `gpu-throughput` `extract` p50 read not `regressed` on direct, direct-again and aligned. `gpu` `extract` p95 may read `regressed` on one of the three only when the other two read it not `regressed`.
  - Every other owned metric of every row reads not `regressed` on direct and direct-again, with three ways out:
    - A drift-list metric `regressed` on one pair passes when the other direct pair and the aligned pair read it not `regressed`.
    - An `ecs` per-run-mode row also passes when the suite behind the reading holds at least three mode runs of five and a pair without that suite reads it not `regressed`.
    - An `ecs` system row or `churn` `flush` `regressed` on both direct pairs passes as a reading this plan accepts (Q10) when the aligned pair reads it not `regressed`. It is recorded as accepted with the aligned pair quoted, not as proven placement; the regression policy's padded-baseline control does not run.
  - Both A/A pairs: no owned `regressed` or `improved` reading outside the drift list. Readings on the list are quoted.
  - The probe pair: `gpu-throughput` `extract` p50 and p95 read not `regressed`, so the default channel and two contributions cost what the row's own closure costs (Q12). The probe draws one more batch than the candidate, so its `render_encode` is reported, not judged.
  - `just smoke` and `just visual` → pass.
  - `cmp` of the two baseline captures → identical for each row, so the capture reproduces. `cmp` of baseline and candidate → identical for each row. If the baseline pair itself differs, record that, and baseline against candidate is reported as `compare_png` counts at tolerance 0, not as identity.
  - `ls target/m42-base target/m42-probe` → no such file, for both.
- **Moves:** none

### Step 5: Release 0.56

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README "Now" lines, this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** Close workflow §5 and this step's evidence row with `status: done`. Then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.56.0` once, then every release check once after the cut. Step 4 already ran `just visual`, and no gate by change is left. Then the preflight with `--message`.
- **Done-when:**
  - [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report, since the plan is archived before the cut.
  - `just release-preflight 0.56.0 --repo JosephGibson/Tungsten --message 'Update 0.56: additive extracts (W15c, D-138)'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt. That is the plan prompt for the Phase 5 QA pass, which the register puts after W15c (amendment 18); `/tungsten-next` names the order against W8a and W1 M0b with W2 R2.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | What becomes of `App::set_extract_*` (w15 step 3's question)? | Kept as the explicit replace, with the same signatures, forwarding to `Extracts::replace_*`. Nothing is removed and no ledger row is needed. The resource's pair is the plugin form |
| Q2 | Where do tilemaps sit in the default sprite channel? | Under every `Sprite`: `extract_tilemaps`, then `extract_sprites_default`, the `gpu` row's order. Tile layers ordered by z among sprites (as `integrated` and example 01 need) become a 1.x backlog row that W16b's tilemap builder may take |
| Q3 | Does example 01's extract shrink to "rainbow balls, outlined text, glows" (w15)? | No. It keeps the explicit replace, because its parallax and particles are `Sprite` entities it draws differently (A8), and draws its tile layers through `extract_tilemap_layers`. Moving the rest onto `Sprite` would change its pixels and is not planned. w15's checkable lines hold: no tilemap code in `extract.rs`, and the same output |
| Q4 | Should the tilemap extract honour lit atlases? | Yes (§2.3). A tile with a normal map draws lit, as a sprite does. This is a behaviour change with a ledger row. Step 2's equivalence check needs it, and no stock pixel fixture has a lit tile |
| Q5 | Do the HUD, overlay, inspector text, debug draw, lights and mesh particles become plugin contributions? | No. They stay engine-owned in `stage_extract`, after the channels. W13's plan moves them if the kit needs to |
| Q6 | Do contributions carry names or `before`/`after` constraints? | No: registration order only. A named form can be added later without a break |
| Q7 | What happens to the channels when the `Extracts` resource has been removed? | They draw nothing. No panic in the frame loop; the type's rustdoc says so |
| Q8 | Do examples 03 and 04 move off `set_extract_text`? | No. With no text default, `set_extract_text` draws exactly what it drew before. The template and the guide show the plugin form |
| Q9 | Who fixes tilemaps at `z_norm` 0 under `gpu_depth`, which the default would otherwise reach (A16)? | The default channel sets its own tiles to 1.0 (§2.2), so no game reaches the bug through the default. The general fix, for custom extracts that call `extract_tilemaps` or the layer form, stays W8a's, whose card lists it |
| Q10 | Step 4's acceptance rules (the drift list, the per-run-mode rule, the aligned-pair rule): are they right? | As written. A reading the aligned-pair rule passes is accepted by this plan, not proven to be placement. A reading outside the rules stops the release for the owner |
| Q11 | Are the names right: `tungsten::extract::Extracts`, `add_*`/`replace_*` and `extract_tilemap_layers`? | As sketched. The API review after step 1 may rename them, and `D-138` records the final names |
| Q12 | Two round-2 critique findings were applied without a third round (§9, 2.1 and 2.2). Are they right: contributions keep caller-owned depth, and a scratch `gpu-throughput` probe measures the default channel? | Yes, both as written. If the probe reads `extract` `regressed`, the release stops for the owner. The likely cause is the far-plane pass, which can move into the tile loop as a parameter |

Approved 2026-10-07 with the stated defaults.

Amended 2026-10-07 during step 1, by the owner's answer to the scope stop: `crates/tungsten/src/tests/app.rs` joins step 1's files. Its `default_sprite_extract_installed_when_not_set` and `user_extract_sprites_overrides_default` read the removed slots and `install_default_extracts`, so they are deleted; step 1's `default_draws_tilemaps_then_sprites`, `replace_keeps_contributions` and `last_replace_wins` cover the same behaviour on the harness.

API review 2026-10-07: approved as built after step 1, with no renames (Q11).

## 5. Decisions expected

- `D-138`: additive extracts (W15c). The `Extracts` resource and its channels, the default sprite channel (tilemaps at the far plane, then sprites), replace versus add, `extract_tilemap_layers`, and lit tiles. It amends `D-042` (choice 4) and cites `D-061` and `D-128`. The ID was free at `cc3ded9`; recheck before step 3.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests (8 of 8), `gpu-visual.png` (the file, with the `gpu` and `integrated` captures byte-equal in step 4), and the post and transition regressions (passing). Also fixed:
  - The resolved-schedule snapshots of `DefaultPlugins`, the template and examples 01, 03 and 04 (no system changes).
  - Example 01's per-layer tile output (step 2) and its layer order (`[1, 2, 6, 3, 7, 5, 4, 8]`).
  - Every `examples/02_bench` file.
  - `api/tungsten_core.txt` and `api/tungsten_render.txt`.

## 7. Stop conditions

- **Failed check.** A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that do not depend on it. Never restore an earlier step's files once a later step has built on them. Steps 2 and 3 build on step 1. A check that fails in step 4 or at the release leaves the tree as it is, records the failure in the evidence log, and stops before the release.
  - Steps 2–5 depend on step 1, so a failed step 1 ends the run.
  - A skipped step 2 lets steps 3 and 4 run, with step 3's records written for what landed. The release then stops for the owner, because w15's example check is unmet.
- **API review.** The run stops after step 1, as level C requires, whatever its result.
- **Pixels.** `just visual` fails in step 1: restore step 1's files and stop, reporting the failing test. `just visual` fails, or a baseline-against-candidate capture differs, in step 4: keep the tree, record the differing row, and stop before the release.
- **Example 01 output.** Step 2's check 1 or 2 fails at any position: restore the step's files and stop the release. A tile property the engine does not carry yet is the owner's call.
- **Perf.** A reading outside step 4's rules: stop before the release. The owner accepts it under this plan, as with M39's Q7 and M41's Q13/Q14, or asks for a fix.
- **Scope.** A file outside §files to touch needs a change: stop and ask. The exception is a test that only needs a renamed path, which is recorded in the evidence row.
  - `api/tungsten_core.txt` or `api/tungsten_render.txt` changes: stop.
- **Shared machine.** `D-138` was taken by another session: use the next free ID and record it. The encoder, a capture or another session's `cargo`: wait, and never capture beside them.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1, 2, 4, release | `just visual` in 1 and 4; `just smoke` in 2 and 4; every CPU row and the byte-level bench captures in 4. The release runs the release checks once, after the cut. Layer 1 and the physics release tests are untouched, since no manifest or physics change is made |
| 3 Invariants | 1, 2, 4 | §6 |
| 4 Decisions | 3 | `D-138` before the guide and `DESIGN.md` cite it |
| 5 API (break ledger, rustdoc, `just api` snapshot) | 1, 3 | `just api` and rustdoc in 1, the two behaviour-change ledger rows in 3, and `just api-check` in 3 and the release |
| 6 Template and guide | 1, 3 | The template's plugin and `AGENTS.md` in 1; the guide's "Drawing" section in 3 |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 3 | The default sprite extract's row gains `tungsten/extract.rs` |
| 8 Design | 3 | The render path, the default sprite extract and the tilemaps sections |
| 9 Records (known issues, backlog, gap log, register, workstream file, README "Now") | 3, release | The backlog row (Q2); w15 step 3; the register and "Now" lines at release. No gap row is open for extracts |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight. The session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo` (gpt-6.1-sol, OpenAI family), 2026-10-07, on the plan at `cc3ded9` with the tree clean; gpt-6.1-sol, effort xhigh, 181.0 s, tokens in 370,683 (cached 288,256), out 5,686. It changed step 1's and step 4's done-when checks, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] The new default can hide sprites under `gpu_depth`: tiles keep `z_norm` 0 while default sprites take decreasing depths with `LessEqual` | `crates/tungsten/src/sprite_extract.rs:89-91` (`z_norm` in [0, 1), the last drawn at 0); `crates/tungsten-render/src/sprite.rs:1052-1056` (depth write, `LessEqual`); `tilemap_extract.rs:190` | Accepted | The default channel writes its tiles at `z_norm` 1.0 (§2.2, §2.3), with step 1's test `default_tiles_stay_under_sprites_under_gpu_depth`; Q9, A16 and follow-up 2 rewritten. The public functions keep 0; that general fix stays W8a's |
| 2 | [MEDIUM] `just visual` cannot establish byte equality: `gpu-visual` allows tolerance 2, and the transition test compares within one build | `examples/02_bench/tests/visual_regression.rs:66`; `examples/03_scene_state/tests/transition_regression.rs:58`; the capture variables at `app.rs:1603-1621` | Accepted | Step 1 claims only a pass, and puts the byte-level evidence in the no-map extract test. Step 4 adds baseline-against-candidate captures of `gpu` (visual preset) and `integrated`, compared with `cmp` after a baseline pair; A15 and §6 rewritten |
| 3 | [MEDIUM] The aligned pair does not prove that a regression on both direct pairs is placement | `docs/perf/profiling-workflow.md:210` (the padded-baseline control) | Accepted | The rule stays, but such a reading is recorded as accepted by this plan (Q10) with the aligned pair quoted, not as proven placement |
| 4 | [MEDIUM] A step 4 pixel failure restores step 1's files, which step 2 already builds on | §7 against step 2's change | Accepted | §7: never restore an earlier step once a later one builds on it. A failure in step 4 or at the release keeps the tree, records it and stops before the release |

Round 2: the same reviewer and effort, 2026-10-07, on the revised plan (tree clean but for this plan); gpt-6.1-sol, effort xhigh, 262.0 s, tokens in 531,187 (cached 444,288), out 9,384. There is no third round. Both findings were checked and applied, and Q12 puts the applied changes to the owner.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Registration order does not guarantee composition under `gpu_depth`: an appended sprite at depth 0.5 is rejected where the default's last sprite sits at 0 | `sprite_extract.rs:89-91`; `crates/tungsten-render/src/sprite.rs:1056`; every custom extract in the tree writes `z_norm` 0 (`examples/01_platformer/src/extract.rs:92-107`, `tilemap_extract.rs:190`) | Accepted | §2.2's "Depth" bullet: contributions keep caller-owned `z_norm`; at 0 an appended instance draws over the default under `gpu_depth`. Step 1's order test checks that the channel keeps a nonzero `z_norm`. The rustdoc and `D-138` state it (Q12) |
| 2.2 | [MEDIUM] The sitting never measures the default channel or contributions: every bench row installs its own extract | `examples/02_bench/src/gpu.rs:446`, `integrated.rs:369`, `particles.rs:429`; A11, A18 | Accepted | Step 4 adds a scratch probe: `gpu-throughput` drawn through the default channel with two contributions, against the candidate. `extract` p50/p95 must read not `regressed`; bench files in the tree are unchanged (Q12) |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-07 | Done; stopped for the API review (level C). One scope stop, answered by the owner (the amendment under §4) | `cargo test -p tungsten --lib extract`: 58 passed, the eight new tests among them. Template: 7 passed; `templates/basic/tests` and `src/main.rs` unchanged; no `set_extract` in the template; one `add_text`, in `GamePlugin::build`. No removed slot or `install_default_extracts` left. `just api`: only `api/tungsten.txt`, +36/−0, no `-pub`; `api/tungsten-core.txt` and `api/tungsten-render.txt` unchanged. rustdoc `-W missing-docs`: nothing in `extract.rs` or `tilemap_extract.rs`. `just visual`: 2, 5 and 1 passed; bench fixtures unchanged. `just check`: 33 suites, 1266 passed, 0 failed, 5 ignored | `perf-runs/20261007-m42-additive-extracts/step1/` |
| 2 | 2026-10-07 | Done | Equivalence test on the real `level.tmj` (184 tileset entries on pages 100/101 by index, 62 lit, one UV rect each): checks 1 and 2 pass at spawn, mid-level and the whole map (zoom 0.1); `terrain` 110, 28 and 1975 instances, `background` 144 and `foreground` 7 on the whole map, no two engine instances on one cell. Check 3: the unsorted lists differ for `terrain` at every position (engine 4, 2 and 4 batches against the example's 76, 4 and 1282 runs) and match elsewhere. The level's `decorations` layer holds no tile (`jq`: 144, 0, 1975, 7, collision 1820), so check 1's pair reduces to `background`. The `Tilemap\|LayerKind\|tileset` grep finds nothing; `extract_tilemap_layers(` is on three lines, rustfmt wrapping the first call's arguments (`&["background", "decorations"]` on the next lines); no `extract_tile_layers` anywhere. `cargo test -p example-01-platformer`: 72 passed; the presentation diff is the function path only, `[1, 2, 6, 3, 7, 5, 4, 8]` kept. `just smoke`: 4/4 examples, the template, the lighting fixture, the parallax backdrop, 15/15 benchmark rows. `git status`: the three files | `perf-runs/20261007-m42-additive-extracts/step2/` |
| 3 | 2026-10-07 | Done | `D-138` was free (rechecked) and is written, with `D-042`'s marker line and both index rows; `extract.rs`'s module doc cites it. The greps: one `## D-138` heading, one index row, the marker on `D-042`; `Extracts` in `DESIGN.md` 1 and the guide 2; one `tungsten/extract.rs` row in the LLM index (10,774 B of 12,288); two w04 rows reading landed (`D-138`); `D-138` in w15 (2) and the backlog (1). Two out-of-scope findings went to known issues (follow-ups 3 and 5). `just ctx` OK, `just repo-check` 0 errors, `just api-check` current for 3 crates, `just check` 33 suites, 1266 passed, 0 failed, 5 ignored | `perf-runs/20261007-m42-additive-extracts/step3/` |
| 4 | 2026-10-07 | Done | Five prebuilt binaries (`nm` `extract::Extracts`: baseline 0, candidate, aligned candidate and probe 4). Sitting 14:05–14:33 UTC after 60 s without the encoder; the first baseline attempt read invalid (`nxcodec.bin` in `gpu-throughput` runs 4–5), was set aside and rerun valid; every final suite valid, names and `workload_version` equal. Owned verdicts (regressed / improved / unchanged / noisy): direct 0/0/47/11, direct-again 0/0/44/14, baseline A/A 0/0/45/13, tree A/A 0/0/50/8, aligned 0/2/44/12 (`churn` `flush` p50 and `particle_count_refresh` p95 improved); digests 8 of 8 in every compare. `extract` p50: `gpu` 0.628 → 0.638, 0.634 → 0.642, 0.626 → 0.636 ms and `gpu-throughput` 12.076 → 12.104, 12.086 → 12.076, 12.148 → 12.070 ms (direct, again, aligned), all `unchanged`; `gpu` p95 `noisy` in all three. A/A readings other than `unchanged` are all `noisy`, none `regressed` or `improved`: on the drift list `ecs` `stats_decay`, `follow`, `buffs` and `team_bags`, `gpu` `extract` p95, `gpu-throughput` `render_encode` p50/p95 and `particle_count_refresh` p50/p95; off it `ecs` `age_phase`, `gpu` `render_encode` p50/p95, `churn` `flush` p50 and `churn_spawn` p50. Probe: `extract` p50 12.014 → 12.014 (Δ +0.000, τ 0.360) and p95 12.400 → 12.386, `unchanged`; `render_encode` 2.048 → 1.608 / 2.560 → 2.072 `noisy`, reported. `just smoke` passed, `just visual` 2, 5 and 1 passed; the byte-level captures (`gpu` visual and `integrated`, twice from the baseline, once from the candidate) are `cmp`-identical. `ls target/m42-base target/m42-probe`: no such file or directory, for both | `perf-runs/20261007-m42-additive-extracts/` (`README.md`, `summary.md`, `sitting.log`, `gates.log`, `pixels/`) |
| 5 | 2026-10-07 | Done (the cut and the release checks follow the archive, in the release report) | Workflow §5 closed. Invariants: `just physics-release` passed (`pinned_step_state_is_unchanged`, `physics_step_is_bit_identical_across_runs`, the tunneling tests); no change under `examples/02_bench`, `api/tungsten-core.txt`, `api/tungsten-render.txt` or any snapshot or fixture path; digests 8 of 8 and the `gpu` visual and `integrated` captures byte-identical in step 4. Records: `DESIGN.md`'s status line, one `CHANGELOG.md` `[Unreleased]` entry, the register's W15c row and change-log line, the README's Now lines; `just release-check` consistent with `[Unreleased]` filled. `status: done`, archived before `just release-cut 0.56.0` | `perf-runs/20261007-m42-additive-extracts/step5/` |

## Follow-ups

1. **Tile layers ordered among sprites by z.** Example 01 and `integrated` would keep the default with this. The home is a 1.x backlog row (step 3), unless W16b's tilemap builder takes it (Q2).
2. **Tilemaps at `z_norm` 0 under `gpu_depth`** in custom extracts that call `extract_tilemaps` or the layer form (A16). The default channel is safe (Q9). The home is W8a, whose card lists it.
3. **One `tile_batch_lens` hint per frame** shared by several tilemap calls (A17). It affects performance only; C2 or the Phase 5 QA pass takes it if a row shows it. Recorded in known issues (step 3).
4. **Engine-owned extract pieces as plugin contributions** (Q5). The home is W13's plan, if the kit needs them; `D-138`'s "Not decided here" names it.
5. **Private intra-doc links in `sprite_extract.rs`'s module doc.** Its two `[`ExtractScratch`]` links, which predate M42, warn on every `just api` (`rustdoc::private_intra_doc_links`). Recorded in known issues (step 3).
