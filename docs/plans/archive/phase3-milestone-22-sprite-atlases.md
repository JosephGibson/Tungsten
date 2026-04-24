---
status: done
milestone: M22
goal: Pack manifest-registered sprites into per-filter atlas textures at load time, store a per-sprite UV rect, keep the game-facing `AssetRegistry` / `Sprite` / `Transform` API unchanged, and measurably reduce live sprite-texture bind count on representative scenes.
non-goals:
  - New runtime dependencies — packer is hand-rolled per [Phase3.md:167-169](Phase3.md#L167-L169)
  - Glyph/text atlas changes (glyphon owns its own atlas — `D-026`)
  - Mipmaps, streaming, eviction, atlas GC beyond rebuild-on-growth
  - Array-texture / multi-layer atlases; overflow handles by opening a second 2D page in the same filter class
  - GPU-compressed texture formats (BC7/BCn, KTX2, Basis Universal) — explicit Phase 4+ deferral per [DESIGN.md:251](../../DESIGN.md#L251); also incompatible with M22's no-new-dep rule
  - Manifest, `tungsten.json`, `input.json`, `scene.json` schema changes
  - Example art changes — M22 must be transparent to game code
files to touch:
  - [crates/tungsten-core/src/assets/atlas.rs](../../crates/tungsten-core/src/assets/atlas.rs) (new)
  - [crates/tungsten-core/src/assets/mod.rs](../../crates/tungsten-core/src/assets/mod.rs)
  - [crates/tungsten-core/src/assets/registry.rs](../../crates/tungsten-core/src/assets/registry.rs)
  - [crates/tungsten-render/src/sprite.rs](../../crates/tungsten-render/src/sprite.rs)
  - [crates/tungsten-render/src/sprite.wgsl](../../crates/tungsten-render/src/sprite.wgsl)
  - [crates/tungsten-render/src/renderer.rs](../../crates/tungsten-render/src/renderer.rs)
  - [crates/tungsten-render/src/lib.rs](../../crates/tungsten-render/src/lib.rs)
  - [crates/tungsten-render/benches/render_bench.rs](../../crates/tungsten-render/benches/render_bench.rs)
  - [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs)
  - [crates/tungsten/src/sprite_extract.rs](../../crates/tungsten/src/sprite_extract.rs)
  - [crates/tungsten/src/tilemap_extract.rs](../../crates/tungsten/src/tilemap_extract.rs)
  - [crates/tungsten/src/app_tests.rs](../../crates/tungsten/src/app_tests.rs)
  - [crates/tungsten/tests/atlas_integration.rs](../../crates/tungsten/tests/atlas_integration.rs) (new)
  - [examples/01_platformer/src/extract.rs](../../examples/01_platformer/src/extract.rs)
  - [examples/02_sprite_stress/src/main.rs](../../examples/02_sprite_stress/src/main.rs)
  - [DECISIONS.md](../../DECISIONS.md) + [docs/DECISION_INDEX.md](DECISION_INDEX.md) (append `D-048`)
  - [docs/LLM_INDEX.md](LLM_INDEX.md) (add atlas row)
  - [docs/plans/Phase3.md](Phase3.md) (status flip + archive pointer on close)

---

# Phase 3 Milestone 22 — Sprite Atlases

## Scope (lifted from Phase3.md M22)

- Pack sprites into atlas textures at load time; in-engine packer; no new dependency.
- Store UV rect per sprite asset.
- Keep sprite ID access unchanged.
- Split atlases by sampler mode (`nearest`, `linear`).
- On hot-reload growth, allow full rebuild and log a warning.

Done-when bar: existing examples render correctly; texture count measurably lower; `nearest` / `linear` parity verified against pre-atlas output.

Dependency: M15 (`D-042`). No blocker for M23/M24.

---

## Seam / Ownership

- `tungsten-core` — CPU atlas data and packer. No `wgpu`. (`D-007`, `D-016`.)
- `tungsten-render` — single `wgpu::Texture` per atlas page in the existing sprite-pipeline pool; updated `SpriteInstance` layout + WGSL; one bind group per atlas (not per sprite).
- `tungsten` umbrella — orchestrates pack-then-upload in `asset_loader::load_sprites`; drives hot-reload rebuild.

No new cross-crate dependencies; existing `tungsten-render → tungsten-core` arrow (`D-007`) is sufficient.

---

## Public API / Data-Layout Deltas

1. `tungsten_core::assets::SpriteAsset` — rename `texture: TextureHandle` → `atlas: TextureHandle`; add `uv: UvRect`. Keep `filter`, `width`, `height`, `path`.
2. `tungsten_core::assets::UvRect` — new: `{ min: [f32;2], max: [f32;2] }`, `Copy`, `PartialEq`. Provide `UvRect::FULL = UvRect { min: [0.0,0.0], max: [1.0,1.0] }` for callers that want an untransformed quad (single-page test path, sprite-stress baseline).
3. `tungsten_core::assets::AssetRegistry::register_sprite` — new signature: `fn register_sprite(&mut self, id: String, filter: FilterMode, width: u32, height: u32, path: PathBuf, atlas: TextureHandle, uv: UvRect)`; the return type drops (caller already holds `atlas`). Internal `next_texture_handle` counter is removed — handles now originate at the renderer.
4. `tungsten_core::assets::AssetRegistry::update_sprite_entry(id, atlas, uv, width, height)` — replaces `update_sprite_dimensions` (same method renamed + extended). Used on hot-reload rebuild.
5. `tungsten_render::SpriteInstance` — add two vertex attributes `uv_min: [f32;2]` at `@location(6)` and `uv_size: [f32;2]` at `@location(7)`. Stride grows 24 B → 40 B. Add constructor `SpriteInstance::whole(position, size, rotation, color) -> Self` that fills `uv_min = [0,0]`, `uv_size = [1,1]` for callers driving the pipeline without an `AssetRegistry` entry (sprite-stress baseline, placeholder uploads).
6. `tungsten_render::Renderer::allocate_texture_handle()` — new, returns a fresh monotonic `TextureHandle`. Handle authority lives with the GPU pool so `AssetRegistry` stops minting.
7. `tungsten_render::Renderer::drop_texture(handle)` — new; removes a pool entry and its bind group. Used when a rebuild shrinks the page count.
8. `tungsten_render::Renderer::max_2d_texture_dimension() -> u32` — passthrough over `device.limits().max_texture_dimension_2d`, clamped to 8192 to keep atlas pages within the portable-core limit.
9. `tungsten_render::Renderer::upload_texture(handle, rgba, w, h, filter)` — add the `filter: FilterMode` parameter; atlases are single-filter, so the correct sampler is baked into the one bind group stored with the texture.
10. `tungsten_render::SpriteBatch.filter` — stays; used as a debug assert against the pool entry’s filter. Mismatch logs a warn and skips the draw (mirrors the existing missing-texture behaviour at [sprite.rs:448-454](../../crates/tungsten-render/src/sprite.rs#L448-L454)).
11. `GpuTexture` (private, in `sprite.rs`) — drop the dual `bind_group_nearest` + `bind_group_linear` pair; replace with `bind_group` + `filter`.

On-disk formats are unchanged. `SpriteInstance` is an in-process POD vertex stream; no persistence.

---

## Ordered steps

Convention: every step must leave `cargo build --workspace` and `cargo test --workspace` green at the step boundary. Verifications listed per step are the ones that *prove* that step.

### Step 1 — Core: atlas module (self-contained)

- Create [crates/tungsten-core/src/assets/atlas.rs](../../crates/tungsten-core/src/assets/atlas.rs):
  ```rust
  pub struct UvRect { pub min: [f32; 2], pub max: [f32; 2] }
  impl UvRect { pub const FULL: Self = Self { min: [0.0,0.0], max: [1.0,1.0] }; }

  pub struct PackInput<'a> { pub id: &'a str, pub width: u32, pub height: u32 }
  pub struct PackedSprite { pub id: String, pub page: u32, pub x: u32, pub y: u32, pub width: u32, pub height: u32 }
  pub struct AtlasPage { pub width: u32, pub height: u32 }

  pub struct PackResult { pub pages: Vec<AtlasPage>, pub sprites: Vec<PackedSprite> }

  pub fn pack_shelf(inputs: &[PackInput<'_>], max_dim: u32, padding: u32) -> PackResult;
  ```
- Algorithm: shelf-next-fit. Sort a stable copy of `inputs` by `(height desc, width desc, id asc)` — deterministic tie-break. Open shelves inside the current `AtlasPage`; when a shelf would overflow `max_dim` on either axis, finalise the page and open a new one. Every sprite gets `padding` on all four sides (padding-in-the-canvas; the UV inset in step 4 keeps samples inside the drawn rect).
- Compute page size per page: `width = next_power_of_two(observed_shelf_extent)` clamped to `max_dim`; `height = next_power_of_two(highest_y_used)` clamped to `max_dim`. Power-of-two avoids odd-dimension allocations without adding a new dep.
- Return inputs to their original order in `result.sprites` for stable iteration.
- Hard-fail when a single sprite exceeds `max_dim - 2*padding` on either axis — atlas overflow at item granularity is unrecoverable without a new strategy; panic with a clear message naming the sprite id and size. Document in the module header.
- Add unit tests in `#[cfg(test)] mod tests`:
  - Empty input → `PackResult { pages: [], sprites: [] }`.
  - Single 16×16 sprite → one page, origin `(padding, padding)`.
  - Two 128×128 sprites with `max_dim=256` → pack both on page 0 (same shelf).
  - Three 128×128 sprites with `max_dim=256` → page 0 has two, page 1 has one.
  - One 200×200 sprite with `max_dim=256, padding=1` → hard-fails (exceeds 254). Use `#[should_panic]`.
  - Determinism: identical input → identical output page count + positions.
- Export: add `pub mod atlas;` + re-exports (`UvRect`, `PackInput`, `PackedSprite`, `AtlasPage`, `PackResult`, `pack_shelf`) to [assets/mod.rs](../../crates/tungsten-core/src/assets/mod.rs).
- Verify: `cargo test -p tungsten-core assets::atlas`.

Workspace stays green — this step adds only.

### Step 2 — Render: `SpriteInstance` layout + shader + pipeline bind-group simplification

- [crates/tungsten-render/src/sprite.rs](../../crates/tungsten-render/src/sprite.rs):
  - Add `uv_min: [f32; 2]` and `uv_size: [f32; 2]` fields to `SpriteInstance`. Extend `ATTRIBS` to six entries, adding `6 => Float32x2, 7 => Float32x2`. Update `desc()` automatically via the `array_stride: size_of::<Self>()` line (already correct).
  - Add `SpriteInstance::whole(position, size, rotation, color)` constructor — fills `uv_min = [0.0, 0.0]`, `uv_size = [1.0, 1.0]`.
  - Replace `GpuTexture { bind_group_nearest, bind_group_linear, ... }` with `GpuTexture { texture, view, bind_group, filter }`.
  - Change `upload_texture(device, queue, handle, rgba, w, h, filter)` — it now selects the sampler once and stores a single bind group. Remove `filter`-based selection in `draw`.
  - `draw`: bind `gpu_tex.bind_group` unconditionally; if `batch.filter != gpu_tex.filter`, `log::warn!("sprite batch filter {:?} != pool filter {:?} for handle {:?}", batch.filter, gpu_tex.filter, batch.texture)` and skip (matches existing missing-texture warn pattern).
  - Add `allocate_texture_handle(&mut self) -> TextureHandle` and `drop_texture(&mut self, handle: TextureHandle)`; store a `next_handle: u32` field on `SpritePipeline` for the allocator (seeded at 0; monotonic across the process lifetime).
  - Update the existing unit tests for `SpriteInstance::desc()` stride / attribute count if any (there are none currently — add one: `assert_eq!(std::mem::size_of::<SpriteInstance>(), 40);`).
- [crates/tungsten-render/src/sprite.wgsl](../../crates/tungsten-render/src/sprite.wgsl):
  - Add `@location(6) inst_uv_min: vec2<f32>` and `@location(7) inst_uv_size: vec2<f32>` to `InstanceInput`.
  - `vs_main`: set `out.tex_coord = instance.inst_uv_min + vertex.uv * instance.inst_uv_size;`.
  - Fragment unchanged.
- [crates/tungsten-render/src/renderer.rs](../../crates/tungsten-render/src/renderer.rs):
  - Thread `filter` through `Renderer::upload_texture`.
  - Add `Renderer::allocate_texture_handle()`, `Renderer::drop_texture(handle)`, `Renderer::max_2d_texture_dimension()` — each is a one-liner over the sprite pipeline / device.
- Verify: `cargo build -p tungsten-render`, `cargo test -p tungsten-render`.

Workspace is **red at this point** because `asset_loader::load_sprites` still calls `upload_texture` without `filter` and constructs `SpriteInstance` struct literals without the new fields. Step 3 fixes all consumers in a single wave.

### Step 3 — Core registry + all `SpriteInstance` / `SpriteAsset` consumers, one wave

Do these in the same commit so the workspace returns to green at the end of the step.

- [crates/tungsten-core/src/assets/registry.rs](../../crates/tungsten-core/src/assets/registry.rs):
  - Rename `SpriteAsset.texture` → `atlas`. Add `uv: UvRect` field.
  - Replace `register_sprite` signature per §API delta 3. Drop `next_texture_handle`.
  - Replace `update_sprite_dimensions` with `update_sprite_entry(id, atlas, uv, width, height)` per §API delta 4.
  - Update the six unit tests already in the file to thread `UvRect::FULL` + a dummy `TextureHandle(0)` through the helper.
- [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs):
  - In `load_sprites`, temporarily (for this step only) pack each sprite into its own one-sprite page: call `renderer.allocate_texture_handle()` per sprite, upload full decoded image at `UvRect::FULL`, and register with `filter`. This keeps behaviour bit-identical to pre-M22 while the rest of the plumbing settles. Step 4 replaces this with real packing.
  - `reload_sprite`: use `update_sprite_entry` and keep the one-sprite-per-atlas mapping; pass `atlas: asset.atlas`, `uv: UvRect::FULL`.
  - `reload_manifest` sprite-addition path: same one-per-atlas behaviour.
- [crates/tungsten/src/sprite_extract.rs](../../crates/tungsten/src/sprite_extract.rs):
  - Rename `asset.texture` → `asset.atlas`.
  - Batching key: `(asset.atlas.0, asset.filter)`.
  - Emit `uv_min: asset.uv.min`, `uv_size: [asset.uv.max[0]-asset.uv.min[0], asset.uv.max[1]-asset.uv.min[1]]` on each `SpriteInstance`.
  - Update in-file tests to pass through the new `register_sprite` signature and assert `uv_min == [0,0]`, `uv_size == [1,1]` in the default (one-sprite-per-atlas interim) case.
- [crates/tungsten/src/tilemap_extract.rs](../../crates/tungsten/src/tilemap_extract.rs):
  - Same rename + same instance-field fill.
- [crates/tungsten/src/app_tests.rs](../../crates/tungsten/src/app_tests.rs):
  - Update any `SpriteInstance { … }` struct literals to use `SpriteInstance::whole(…)` or fill the two new fields explicitly.
- [crates/tungsten-render/benches/render_bench.rs](../../crates/tungsten-render/benches/render_bench.rs):
  - Replace the struct literal at [render_bench.rs:29-34](../../crates/tungsten-render/benches/render_bench.rs#L29-L34) with `SpriteInstance::whole(...)`.
- [examples/01_platformer/src/extract.rs](../../examples/01_platformer/src/extract.rs):
  - Rename `asset.texture` → `asset.atlas` at the four `SpriteBatch { texture: ..., ... }` sites.
  - Rewrite the four `SpriteInstance { … }` struct literals to fill `uv_min = asset.uv.min`, `uv_size = [asset.uv.max[0]-asset.uv.min[0], asset.uv.max[1]-asset.uv.min[1]]`. All four sites already resolve `SpriteAsset` via `assets.get_sprite(...)`, so no new lookups are needed.
- [examples/02_sprite_stress/src/main.rs](../../examples/02_sprite_stress/src/main.rs):
  - Both `extract_baseline_sprites` and `extract_high_load_sprites` already use a placeholder `TextureHandle` (no real asset). Use `SpriteInstance::whole(...)` at both sites ([main.rs:329](../../examples/02_sprite_stress/src/main.rs#L329), [main.rs:771](../../examples/02_sprite_stress/src/main.rs#L771)).
- Verify: `cargo build --workspace`, `cargo test --workspace`, `./scripts/smoke-examples.sh`.

At end of step 3 the engine renders identically to pre-M22 (one sprite per atlas page). All plumbing is in place. Note: texture count is *higher* here than either pre-M22 or post-step-4 (every sprite gets a fresh handle via `allocate_texture_handle`). Do not capture perf data at this boundary — it inverts the optimization target temporarily.

### Step 4 — Actually pack atlases at load time

- [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs):
  - Rewrite `load_sprites`:
    1. Decode every `manifest.sprites` PNG to RGBA; retain the decoded `RgbaImage` plus `(id, path, filter, width, height)`.
    2. `max_dim = renderer.max_2d_texture_dimension()`.
    3. For each `filter ∈ { Nearest, Linear }`, partition → build a `Vec<PackInput>` → call `pack_shelf(inputs, max_dim, 1)`. Skip filter classes with no entries.
    4. For each `AtlasPage` in each partition: `let atlas = renderer.allocate_texture_handle();` build a CPU `Vec<u8>` of `page.width * page.height * 4` filled with `0` (transparent black — `Rgba8UnormSrgb`), memcpy every `PackedSprite` with `packed.page == this_page_index` at `(packed.x, packed.y)`, `renderer.upload_texture(atlas, &pixels, page.width, page.height, filter)`.
    5. For each `PackedSprite`: compute `uv` with half-texel inset:
       ```text
       uv_min = [ (x + 0.5)/W, (y + 0.5)/H ]
       uv_max = [ (x + w - 0.5)/W, (y + h - 0.5)/H ]
       ```
       Register with `AssetRegistry::register_sprite(id, filter, w, h, path, atlas_handle_for_page, uv)`.
  - Log `log::info!("Packed {n_sprites} sprites → {n_pages} atlas pages ({nearest_pages} nearest + {linear_pages} linear)")`.
- Provide a small crate-private helper `build_atlas_for_filter(filter, decoded: &[Decoded], renderer, registry, max_dim) -> Result<(), anyhow::Error>` — step 5 will reuse it for the rebuild path.
- Verify: `cargo test --workspace`, `./scripts/smoke-examples.sh`. All three examples render visually identically to the end-of-step-3 snapshot.

### Step 5 — Hot-reload: rebuild-on-growth

- [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs) `reload_sprite`:
  - Resolve `asset.atlas`, `asset.uv`, and the pre-packed rect from the registry. Decode the new PNG.
  - In-place path: if `new_w <= packed_rect_w` and `new_h <= packed_rect_h`, build a padded sub-buffer the size of the packed rect (new bitmap top-left, transparent fill below/right if shrunk), write it with `queue.write_texture` targeting `(packed_rect_x, packed_rect_y)` on the atlas. UV unchanged. `update_sprite_entry` just updates `width/height`.
  - Growth path: `log::warn!("Sprite '{id}' grew ({ow}x{oh} → {nw}x{nh}); rebuilding {filter:?} atlas")`, then call `rebuild_atlas_for_filter(filter, world, renderer)`.
  - **Between-frames invariant.** Hot-reload events are drained on the main loop between frames (`D-031`), so `drop_texture(handle)` inside `rebuild_atlas_for_filter` never races a `SpriteBatch` built in the *current* frame's extract. Any caller that wires a different drain point must preserve this — otherwise a dropped handle can be bound in a pending draw (UB).
  - `rebuild_atlas_for_filter`:
    1. Re-read every sprite with matching `filter` from disk via `SpriteAsset.path` (the registry already holds every path; `D-031` hot-reload semantics mandate re-reading from disk rather than caching in RAM).
    2. Run `pack_shelf` on the fresh sizes.
    3. Track old atlas pages for this filter class via an internal `AtlasRegistry` resource (step 5a below).
    4. Reuse old `TextureHandle`s for the new page list one-to-one. If the new page count is lower, call `Renderer::drop_texture(handle)` on the excess old handles. If higher, allocate new handles for the tail.
    5. Upload every new page canvas; `update_sprite_entry` every sprite.
    6. On any decode error in the rebuild partition: abandon the rebuild, keep the previous atlas, log an error. Last-known-good discipline per existing [asset_loader.rs:289-297](../../crates/tungsten/src/asset_loader.rs#L289-L297).
- Step 5a — add a `tungsten::AtlasRegistry` resource (private to the umbrella crate; live under `asset_loader.rs`) storing `{ nearest_pages: Vec<TextureHandle>, linear_pages: Vec<TextureHandle>, packed: HashMap<String, PackedSprite> }`. Populated at end of `load_sprites`; consulted during reloads. Kept in sync by `build_atlas_for_filter` and `rebuild_atlas_for_filter`.
- Verify: manual smoke — `cargo run -p example-02-sprite-stress`, swap a loaded PNG for a larger one, confirm warn log + no artifacts; `./scripts/smoke-examples.sh`.

### Step 6 — Manifest hot-reload addition path

- [crates/tungsten/src/asset_loader.rs](../../crates/tungsten/src/asset_loader.rs) `reload_manifest` sprite branch ([asset_loader.rs:444-471](../../crates/tungsten/src/asset_loader.rs#L444-L471)):
  - An added sprite is a growth event. Collect additions, group by filter, call `rebuild_atlas_for_filter` for each filter class that gained at least one entry. No `load_sprites(&additions, ...)` call — that call is removed.
- Verify: `cargo test --workspace`.

### Step 7 — Startup bench: `atlas_pack_startup_200`

- [crates/tungsten-render/benches/render_bench.rs](../../crates/tungsten-render/benches/render_bench.rs) (CPU-only per file header — no GPU). `tungsten-render` already depends on `tungsten-core`, so `pack_shelf` is reachable.
- Add `atlas_pack_startup_200`:
  - Generate 200 `PackInput { id, width, height }` entries with sizes drawn deterministically from a small seeded generator (e.g. a `u64` XorShift seeded with `0xA71A5` yielding sizes in `8..=128`).
  - Bench body: `let r = pack_shelf(&inputs, 8192, 1); black_box(r);`. Use 8192 (the production clamp per §API delta 8), not 4096 — the regression gate needs to reflect shipping behaviour.
- Matches the `atlas pack startup cost (200 sprites)` line in [Phase3.md:185](Phase3.md#L185) and anchors the ≤20 % startup-regression rule at [Phase3.md:202](Phase3.md#L202).
- Capture the first run as the M22 baseline; record in the observable-results block below.
- Verify: `cargo bench -p tungsten-render atlas_pack_startup_200`.

### Step 8 — Integration test: atlas batching property

- Create [crates/tungsten/tests/atlas_integration.rs](../../crates/tungsten/tests/atlas_integration.rs):
  - Build a headless `World`, insert a fake `AssetRegistry` with two sprites sharing the same `atlas: TextureHandle(0)`, same `filter`, same `z_order`, but distinct `uv`.
  - Spawn two entities with `Transform + Sprite + Visibility`.
  - Run `extract_sprites_default(&world)` and assert: exactly one `SpriteBatch` returned; `instances.len() == 2`; the two instances have different `uv_min`.
  - A second case: three sprites across two atlases (two share atlas A, one on atlas B) → two batches, sizes 2 and 1.
- No GPU needed (layer-1 class test); runs as part of `cargo test --workspace`.
- Verify: `cargo test -p tungsten --test atlas_integration`.

### Step 9 — Visual parity captures

- Use `tungsten_render::compare_png` (`D-047`, [image_diff.rs](../../crates/tungsten-render/src/image_diff.rs)).
- Procedure, per example in `{ 01_platformer, 02_sprite_stress, 03_scene_state }`:
  1. `git worktree add ../tungsten-m22-baseline <merge-base>` — capture baseline from a separate worktree so in-flight edits on the feature branch are not disturbed.
  2. In the baseline worktree: `TUNGSTEN_SMOKE_FRAMES=3 cargo run -p example-NN-name`, save final-frame screenshot to `perf-runs/m22-baseline/<name>.png`. Also run `cargo bench -p tungsten-render sprite_extract_batch_build_2k` and record the number — this is the denominator for the ≤10 % gate.
  3. In the feature worktree (post step 4 at minimum), repeat both: screenshot → `perf-runs/m22-post/<name>.png`, and `sprite_extract_batch_build_2k` post-M22 number.
  4. `git worktree remove ../tungsten-m22-baseline` when done.
  5. Compare with `delta_per_channel = 0`. Expected result for `nearest`: byte-identical (point sampling of a 1:1 pixel copy). For `linear`: byte-identical **only because** the atlas copy is 1:1 at pow2 dimensions, the 1 px transparent padding plus half-texel UV inset keeps bilinear samples entirely inside the drawn rect, and no mips exist (mips are a non-goal). If any of those three invariants is relaxed in a future change, expect ≤ 1 LSB per channel and soften this gate accordingly.
- Record `DiffReport` summaries and both bench numbers in the observable-results block.

### Step 10 — DECISIONS.md + indexes

- Append to [DECISIONS.md](../../DECISIONS.md):
  ```
  ## D-048 — M22 sprite atlases (shelf packer, per-filter pages, half-texel inset)
  ```
  Capture only what is not already in Phase3.md: shelf-next-fit ordering rule (`height desc, width desc, id asc`), 1 px padding + half-texel UV inset, one page list per `FilterMode` with overflow to multiple 2D pages at `max_2d_texture_dimension ≤ 8192`, rebuild-on-growth rule (shrink / equal stays in-place), and the choice that the renderer mints `TextureHandle`s (registry stops minting).
- Append a matching one-liner under `Assets / Rendering` in [docs/DECISION_INDEX.md](DECISION_INDEX.md) — the coverage test at the top of that file will fail otherwise.
- Verify: `cargo test --workspace` (the `DECISION_INDEX.md` coverage test runs here).

### Step 11 — LLM_INDEX row

- Insert under `## Subsystem Map` in [docs/LLM_INDEX.md](LLM_INDEX.md), between the existing sprite-extract row and the tilemap row:
  ```
  | Sprite atlases (M22) | [crates/tungsten-core/src/assets/atlas.rs](../crates/tungsten-core/src/assets/atlas.rs), [crates/tungsten/src/asset_loader.rs](../crates/tungsten/src/asset_loader.rs) |
  ```

### Step 12 — Phase3 status + archive

- On close-out only:
  - Flip M22 in [Phase3.md](Phase3.md) to `> **Status: complete** (vX.Y.Z, YYYY-MM-DD)` and add `Detailed implementation plan archived at docs/plans/archive/phase3-milestone-22-sprite-atlases.md`.
  - `git mv docs/plans/phase3-milestone-22-sprite-atlases.md docs/plans/archive/phase3-milestone-22-sprite-atlases.md`.
  - Tick the `[ ] Sprite atlas path is transparent to game code and reduces texture pressure` box in Phase3.md’s close-out list.

---

## Done-when checks

Commands (from [AGENTS.md](../../AGENTS.md) `## Commands`):

- [ ] `cargo fmt --all`
- [ ] `cargo build --workspace`
- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets` (advisory; no new warnings in touched files)
- [ ] `./scripts/smoke-examples.sh`
- [ ] `bash scripts/test-perf-capture.sh`
- [ ] `cargo bench -p tungsten-render atlas_pack_startup_200` within 20 % of the first recorded number on this machine ([Phase3.md:202](Phase3.md#L202))
- [ ] `cargo bench -p tungsten-render sprite_extract_batch_build_2k` within 10 % of the pre-M22 baseline on this machine ([Phase3.md:201](Phase3.md#L201)). Note: the `SpriteInstance` stride grows 24 B → 40 B (+66 %), so batch-build cost is expected to move; the test asserts the engineered-in batch collapse more than compensates.

Milestone acceptance (from M22 body + Phase 3 bench gate):

- [ ] Each example renders byte-identically (image diff ≤ 0) vs the pre-M22 commit (step 9).
- [ ] Startup log shows `Packed N sprites → M atlas pages` with `M ≤ 2` per filter class for every in-tree example.
- [ ] Every sprite with `filter: linear` lands in a `linear` atlas; every `filter: nearest` lands in a `nearest` atlas (verified by pool-filter assertion in `SpritePipeline::draw`).
- [ ] Growth hot-reload: swap a PNG for a larger one at runtime, confirm `rebuilding … atlas` warn + no visual artifacts. Shrink hot-reload stays in-place (no rebuild log).
- [ ] New integration test `atlas_integration` passes.

### Observable results (fill on close-out)

- `atlas_pack_startup_200` baseline: `<ms>`
- `sprite_extract_batch_build_2k` pre-M22: `<ms>` / post-M22: `<ms>`
- Texture count before → after:
  - `01_platformer`: `<a> → <b>`
  - `02_sprite_stress`: `<a> → <b>`
  - `03_scene_state`: `<a> → <b>`
- Image-diff status: `01_platformer` ☐, `02_sprite_stress` ☐, `03_scene_state` ☐

---

## Risks / Open Questions

1. **Sampler bleed on `linear` atlases.** 1 px transparent padding + half-texel UV inset should fully suppress neighbour bleed at non-mip sampling. Mipmaps remain out of scope — comment the constraint in `atlas.rs` so a future mip switch doesn’t silently break parity.
2. **Atlas overflow per filter class.** The data model supports multiple pages per filter via `PackedSprite.page`. No current example exercises this (all sprites fit in one 4096² page). Packer tests in step 1 cover the split; there is no end-to-end test for multi-page until a stress example demands it.
3. **Single-sprite overflow.** Any sprite larger than `max_dim - 2*padding` (≤ 8190 in practice) hard-fails at load time. Documented in the module header; Phase3.md does not forbid a panic here, and the alternative (skipping the sprite, emitting the whole PNG as its own page) adds per-sprite-texture code paths the milestone explicitly targets for removal.
4. **Tilemap parity.** `D-032` reuses the sprite render path. Step 3’s tilemap-extract change must flow UVs the same way gameplay sprites do; the existing tilemap smoke in `example-01-platformer` is the primary proof.
5. **Shrink-in-place semantics.** Phase3.md only mandates a *growth* rebuild. The plan treats `new ≤ old` as in-place overwrite with transparent tail; flag at review if that surprises.
6. **Baseline captures.** Step 9 needs a clean pre-M22 checkout to produce the baseline screenshots *and* the `sprite_extract_batch_build_2k` baseline number. Use `git worktree add` against the branch's merge-base rather than switching the working tree — avoids disturbing in-flight edits if capture is deferred until after step 4 lands.
7. **Custom extract closures.** Only `example-01-platformer` resolves real `SpriteAsset`s in its custom extract; `example-02-sprite-stress` uses placeholder handles. Plan threads UV from `SpriteAsset` in the former and uses `SpriteInstance::whole` in the latter — a constructor helper is required for the placeholder path to avoid making custom extract boilerplate mention UVs.

---

## Reference IDs (grep these in [DECISIONS.md](../../DECISIONS.md))

- `D-007`, `D-016` — core/render seam, no `wgpu` in core.
- `D-009`, `D-011`, `D-017` — manifest model, per-sprite filter, merge semantics.
- `D-018` — extract-plain-data-before-drawing.
- `D-023` — WGSL embedded via `include_str!`; step 2’s shader edit requires a rebuild.
- `D-031` — `notify` watcher drives step 5 + step 6.
- `D-032` — tilemap shares sprite render path; step 3 tilemap edit is mandatory.
- `D-042` — M15 components + default extract.
- `D-047` — screenshot + image-diff helpers used in step 9.
- New: `D-048` (added in step 10).
