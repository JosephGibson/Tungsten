---
status: done
goal: "Ship SMAA 1x presentation AA (Low/Medium/High/Ultra) as a renderer-owned fixed tail between the reorderable M26 PostStack and the screen-space text overlay, driven by startup `render.post_aa` / `TUNGSTEN_RENDER_POST_AA` plus a runtime world-resource request path, with `Off` byte-identical to the M26 baseline."
non-goals:
  - "No T2x, 4x, or SMAA with temporal history -- presets are SMAA 1x only."
  - "No new `PostPass::Smaa` variant; SMAA is not reorderable or tween-bridged."
  - "No FXAA, TAA, or MSAA-mode changes; MSAA keeps its M25 relaunch requirement."
  - "No change to `ExtractTextFn` or screen-space text extraction surface."
  - "No manifest entry for the SMAA area/search LUT textures; they ship as `include_bytes!` engine-internal content with attribution."
  - "No runtime hot reload of `tungsten.json`; `render.post_aa` is startup/env config, while runtime changes use `tungsten::request_post_aa`."
  - "No hot reload of SMAA LUT textures or preset constants at runtime."
  - "No WGSL include/preprocessor layer; each SMAA stage shader is a standalone module because the current shader loader validates one file as one WGSL module."
files to touch:
  - "crates/tungsten-core/src/config.rs"
  - "crates/tungsten-core/src/lib.rs"
  - "crates/tungsten-core/src/tests/config.rs"
  - "crates/tungsten-render/src/targets.rs"
  - "crates/tungsten-render/src/passes/order.rs"
  - "crates/tungsten-render/src/passes/mod.rs"
  - "crates/tungsten-render/src/passes/recorder.rs"
  - "crates/tungsten-render/src/post/mod.rs"
  - "crates/tungsten-render/src/post/smaa.rs"
  - "crates/tungsten-render/src/post/smaa_luts.rs"
  - "crates/tungsten-render/src/shaders/stock/smaa_edge.wgsl"
  - "crates/tungsten-render/src/shaders/stock/smaa_blend_weights.wgsl"
  - "crates/tungsten-render/src/shaders/stock/smaa_neighborhood_blend.wgsl"
  - "crates/tungsten-render/src/assets/smaa/area.bin"
  - "crates/tungsten-render/src/assets/smaa/search.bin"
  - "crates/tungsten-render/src/assets/smaa/ATTRIBUTION.md"
  - "crates/tungsten-render/src/lib.rs"
  - "crates/tungsten-render/src/renderer.rs"
  - "crates/tungsten-render/src/tests/passes_order.rs"
  - "crates/tungsten-render/src/tests/smaa.rs"
  - "crates/tungsten/src/app.rs"
  - "crates/tungsten/src/lib.rs"
  - "crates/tungsten/src/post_aa.rs"
  - "crates/tungsten/src/tests/post_aa.rs"
  - "assets/manifest.json"
  - "assets/shaders/stock/smaa_edge.wgsl"
  - "assets/shaders/stock/smaa_blend_weights.wgsl"
  - "assets/shaders/stock/smaa_neighborhood_blend.wgsl"
  - "tungsten.json"
  - "input.json"
  - "examples/04_shader_playground/src/main.rs"
  - "examples/04_shader_playground/assets/manifest.json"
  - "examples/04_shader_playground/assets/alias_checker.png"
  - "scripts/smoke-examples.sh"
  - "docs/showcase/smaa_off_vs_high.png"
  - "docs/showcase/README.md"
  - "DECISIONS.md"
  - "docs/DECISION_INDEX.md"
  - "docs/LLM_INDEX.md"
  - "AGENTS.md"
  - "DESIGN.md"
  - "CHANGELOG.md"
  - "README.md"
  - "docs/plans/phase4.md"
ordered steps:
  - "Add `PostAaMode` (`#[non_exhaustive]`, `is_smaa`, `FromStr`) + env override plumbing to core `RenderConfig` (step 1)."
  - "Extend `RenderTargetPool` + `TargetId` with SMAA working targets, a `PresentSource`, and non-sRGB read views on scene/post targets (step 2)."
  - "Vendor three standalone SMAA WGSL stage shaders plus `area.bin` / `search.bin` LUT binaries under `crates/tungsten-render/src/assets/smaa/` with MIT attribution (step 3)."
  - "Mirror the three SMAA WGSL files into `assets/shaders/stock/` and register the three ids under the root `shaders` manifest section (step 4)."
  - "Build `SmaaPipeline` (3 sub-pipelines + LUT upload + 256-byte preset UBO + record/rebuild helpers) in `post/smaa.rs` (step 5)."
  - "Splice SMAA + overlay routing into `default_pass_order` / `text_overlay_target`; wire renderer fields, `set_post_aa`, shader upload/reload branches, render-frame loop, present-blit source selection, and screenshot source selection (step 6)."
  - "Thread runtime post-AA changes through `tungsten`: `PostAaState`, `PendingPostAa`, public `request_post_aa`, and app frame-boundary apply (step 7)."
  - "Extend `examples/04_shader_playground` with post-AA ActionMap bindings, HUD row, aliasing test sprite, and `TUNGSTEN_POST_AA_FIXTURE` env pin (step 8)."
  - "Add pass-order, SMAA preset/UBO, core config, and umbrella post-AA request tests; extend the smoke script with an M27 post-AA fixture row (step 9)."
  - "Author `D-059`, sync the decision index / LLM index / operational docs / release docs, flip this plan + phase4 M27 row to `done`, archive file (step 10)."
done-when:
  - "`cargo fmt --all && cargo test --workspace` passes on the `0.24` branch."
  - "`./scripts/smoke-examples.sh` passes for all examples, including the existing M26 shader-playground rows and the new `TUNGSTEN_POST_AA_FIXTURE=smaa_high` row."
  - "`WGPU_BACKEND=vulkan cargo run -p example-04-shader-playground` cycles Off -> SmaaLow -> SmaaMedium -> SmaaHigh -> SmaaUltra via hotkeys; HUD mirrors the applied mode; aliasing on the test sprite visibly resolves under SmaaHigh/SmaaUltra."
  - "`WGPU_BACKEND=vulkan TUNGSTEN_RENDER_POST_AA=smaa_high cargo run -p example-03-scene-state` keeps menu / pause text crisp (manual inspection), proving the overlay lands after SMAA."
  - "`cargo test -p tungsten-render passes_order` confirms `post_aa = Off` emits the same `PassDesc` vector as M26 across the msaa x depth x post-stack-length matrix."
  - "`docs/showcase/smaa_off_vs_high.png` committed alongside a regeneration note in `docs/showcase/README.md` using `TUNGSTEN_CAPTURE_FRAME`, `TUNGSTEN_CAPTURE_PATH`, and `TUNGSTEN_POST_AA_FIXTURE`."
  - "`DECISIONS.md` contains `D-059`; `docs/DECISION_INDEX.md` gains the matching row; `docs/LLM_INDEX.md`, `AGENTS.md`, and `DESIGN.md` carry the SMAA frame-order / shader-routing guidance."
  - "`CHANGELOG.md` and `README.md` updated for M27 in the `0.24` section."
  - "`docs/plans/phase4.md` M27 section flipped to `status: done`; this file flipped to `status: done` and moved to `docs/plans/archive/phase4-milestone-27-smaa-presentation-aa.md`."
---

## Context Digest

| Slice | Current state (after M26, on branch `0.24`) |
| --- | --- |
| Frame order | `SceneColor [+ MSAA resolve] -> N x PostPass (ping/pong) -> text overlay (loads final post target) -> present blit -> Swapchain`. Implemented in [`default_pass_order`](../../crates/tungsten-render/src/passes/order.rs) and [`Renderer::render_frame_internal`](../../crates/tungsten-render/src/renderer.rs) around the `post_stack_len + 1` overlay splice. |
| Target pool | [`RenderTargetPool.scene: SceneTarget`](../../crates/tungsten-render/src/targets.rs) holds `color`, `depth?`, `color_msaa?`, `post_ping`, `post_pong`. All sized to surface; `format = surface_config.format` (currently the selected swapchain format, usually sRGB). |
| Stock post pipelines | [`PostStackRenderer`](../../crates/tungsten-render/src/post/mod.rs) owns one `StockPipeline` per `PostPass` variant. Shared [`StockLayouts`](../../crates/tungsten-render/src/post/fullscreen.rs) = `(source BGL: tex + sampler, params BGL: 256-byte UBO)`. Source bind groups rebuild per frame; params UBO is 256 bytes matching [`UniformOverrideBlock`](../../crates/tungsten-core/src/tween.rs). |
| Shader upload/reload | [`load_shaders`](../../crates/tungsten/src/asset_loader.rs) registers every manifest WGSL id in core `ShaderRegistry` and render `shader_ids`. Today `Renderer::upload_shader` / `reload_shader` rebuild sprite and material users only; M27 must add explicit SMAA branches for both initial upload and hot reload instead of assuming a generic stock-post rebuild path exists. |
| Text overlay | Drawn into `text_overlay_target(post_stack_len)` (SceneColor / PostPing / PostPong) via `LoadOp::Load`; present blit samples the same target. `ExtractTextFn` seam is `Box<dyn Fn(&World) -> Vec<TextSection>>` in [`App`](../../crates/tungsten/src/app.rs). |
| Config | [`RenderConfig`](../../crates/tungsten-core/src/config.rs): `clear_color`, `max_frame_latency`, `present_mode`, `msaa`, `depth_enabled`, `depth_sort`. Config is loaded at startup; display runtime changes use request resources, not `tungsten.json` hot reload. |
| Runtime app seam | User systems receive `&mut World`, not `&mut App`; interactive post-AA changes must follow the display pattern (`request_*` writes a pending world resource; `App` applies before render). |
| Core/render invariants | `D-007` (render depends on core, not reverse), `D-016` (no `wgpu` types in core), `D-018` (extract plain POD before draw). `PostPass` + `PostStack` are core-owned POD resources; renderer owns pipelines/targets. |
| Hot-reload matrix | `D-053` + `D-057`: published matrix covers assets and WGSL body edits. SMAA stage WGSL can fit the shader row; LUTs and preset constants stay explicitly out-of-matrix in `D-059`. |
| Screenshot path | `SceneColor`, `post_ping`, `post_pong` all carry `COPY_SRC`. The screenshot/readback pulls from whatever target the present blit sampled. M27 keeps this invariant by routing readback through the SMAA-resolved `PresentSource`. |

### Relevant DECISIONS.md ids

- `D-007`, `D-016`, `D-018` -- core/render seam invariants.
- `D-023` -- shaders embedded; narrowed by `D-057`.
- `D-026` -- `glyphon` + `cosmic-text` text stack (untouched).
- `D-043` -- display config lives in `tungsten.json`, runtime mutation happens through request/apply boundaries.
- `D-053` -- hot-reload matrix (extend for SMAA stage WGSL; declare LUTs out-of-matrix).
- `D-057` -- M25 shader asset rules, `naga` validation gate.
- `D-058` -- M26 materials + post-stack; locks `PostPass` as reorderable art-direction only; SMAA explicitly deferred to M27.

<assumptions>
- Preset knobs (threshold, max search steps, max diag steps, corner rounding) ship as fields of a `SmaaPresetUniform` packed to 256 bytes. Preset switches update the UBO only; they do not recompile or rebuild pipelines.
- Preset values (`Low` / `Medium` / `High` / `Ultra`) match the canonical `SMAA.hlsl` preset block one-to-one: Low = `0.15 / 4 / diag off / corner off`; Medium = `0.10 / 8 / diag off / corner off`; High = `0.10 / 16 / 8 / 25`; Ultra = `0.05 / 32 / 16 / 25`.
- Diag/corner "off" ride sentinel values (`max_search_steps_diag = 0`, `corner_rounding = u32::MAX`) that WGSL branches check.
- SMAA lookup textures are generated from the upstream header arrays: `AreaTex.h` -> `area.bin` (`Rg8Unorm`, 160 x 560) and `SearchTex.h` -> `search.bin` (`R8Unorm`, 64 x 16). Do not add a runtime DDS/parser dependency.
- SMAA intermediate formats: `SmaaEdges = Rg8Unorm`, `SmaaBlend = Rgba8Unorm`, both `RENDER_ATTACHMENT | TEXTURE_BINDING` (no `COPY_SRC`; screenshots sample/copy the resolved `PresentSource`, not the SMAA working set).
- A new `TargetId::PresentSource` (same format as `SceneColor`, `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC`) is allocated only when `post_aa != Off`, keeping the `Off` frame byte-identical to M26.
- `SceneColor` and the post ping/pong pair carry the non-sRGB twin in `view_formats` while SMAA is active so edge/blend/neighborhood stages sample gamma-encoded values without automatic sRGB decode. The rest of the frame keeps using the primary view.
- SMAA WGSL mirrors the stock-shader manifest pattern as three standalone modules: compile-time `include_str!` under `crates/tungsten-render/src/shaders/stock/`, byte-equal mirror under `assets/shaders/stock/`, manifest `shaders` entries, `naga` validation on body-edit reload. There is no `smaa_common.wgsl` module in M27.
- No new runtime or dev dependency. SMAA uses existing `wgpu`, `bytemuck`, and `glam`, so `D-015` is satisfied without a new rule citation.
- Runtime `post_aa` changes apply at an app frame boundary after systems/hot-reload and before extract/render acquire. This matches the existing request/apply shape used for display changes and avoids mid-frame target reallocation.
- `PostAaMode` is `#[non_exhaustive]` so Phase 5 additions (`Fxaa`, `Smaa2x`, `Taa`, etc.) do not break the public match surface.
</assumptions>

---

## Step 1 -- `PostAaMode` in core `RenderConfig`

| File | Edit |
| --- | --- |
| [`crates/tungsten-core/src/config.rs`](../../crates/tungsten-core/src/config.rs) | Add `#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)] #[serde(rename_all = "snake_case")] #[non_exhaustive] pub enum PostAaMode { #[default] Off, SmaaLow, SmaaMedium, SmaaHigh, SmaaUltra }` with `as_str` + `FromStr` mirroring `DepthSortMode`. Parse error: `ParsePostAaModeError`. Constant: `POST_AA_EXPECTED = "one of: off, smaa_low, smaa_medium, smaa_high, smaa_ultra"`. Helper: `pub const fn is_smaa(self) -> bool`. |
| same file `RenderConfig` | Add `#[serde(default)] pub post_aa: PostAaMode,` and wire into `Default::default()`. |
| same file env overrides | Add `RENDER_POST_AA_ENV = "TUNGSTEN_RENDER_POST_AA"`, `apply_post_aa_override`, branch in `apply_env_overrides_from_env` alongside existing `RENDER_DEPTH_SORT_ENV`. |
| [`crates/tungsten-core/src/lib.rs`](../../crates/tungsten-core/src/lib.rs) | Re-export `PostAaMode` under the existing `pub use config::{...}` group. |
| [`crates/tungsten-core/src/tests/config.rs`](../../crates/tungsten-core/src/tests/config.rs) | Add test cases: default is `Off`; every mode parses; `junk` errors with `POST_AA_EXPECTED`; env override `TUNGSTEN_RENDER_POST_AA=smaa_medium` flips the field. |
| [`tungsten.json`](../../tungsten.json) | Document `render.post_aa` by adding it (value `"off"`) so contributors see the key. |

## Step 2 -- Target pool + `TargetId`

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/targets.rs`](../../crates/tungsten-render/src/targets.rs) `TargetId` | Add `SmaaEdges`, `SmaaBlend`, `PresentSource`. Keep existing variants and `#[derive]`s. |
| same file `SceneTarget` | Add `smaa: Option<SmaaTargets>` where `struct SmaaTargets { edges: (Texture, TextureView), blend: (Texture, TextureView), present: (Texture, TextureView), scene_color_linear_view: TextureView, post_ping_linear_view: TextureView, post_pong_linear_view: TextureView }`. The `_linear_view` fields re-view `color`/`post_ping`/`post_pong` through the non-sRGB twin so SMAA samples gamma-space pixels. Allocated only when `post_aa != Off`. |
| same file | `fn create_smaa_edges(device, w, h) -> (Texture, TextureView)` -> `Rg8Unorm`, `RENDER_ATTACHMENT | TEXTURE_BINDING`; `fn create_smaa_blend` -> `Rgba8Unorm`, same flags; `fn create_present_source(device, w, h, format)` -> same flags as `create_resolved_color` (including `COPY_SRC`). |
| same file `create_resolved_color` + `create_post_target` | When `post_aa != Off` is passed down, add `view_formats: &[non_srgb_twin(format)]` (for example `Rgba8UnormSrgb` -> `Rgba8Unorm`, `Bgra8UnormSrgb` -> `Bgra8Unorm`). Helper: `pub fn non_srgb_twin(format: TextureFormat) -> Option<TextureFormat>`. When `None`, the SMAA read views collapse to the primary view. |
| `SceneTarget::new` | Accept `post_aa: PostAaMode`; build the SMAA targets and non-sRGB read views iff `post_aa != Off`. Store `post_aa: PostAaMode` so `RenderTargetPool::resize` can include it in shape checks. |
| `SceneTarget` accessors | Add `smaa_edges_view`, `smaa_blend_view`, `present_source_view`, `present_source_texture`, `scene_color_smaa_read_view`, `post_ping_smaa_read_view`, `post_pong_smaa_read_view`; return `Option<...>` for SMAA-only targets/views. |
| `RenderTargetPool::new` / `resize` | Take `post_aa: PostAaMode`; include it in `shape_changed` so flipping `post_aa` reallocates. Reallocation stays at a frame boundary (called from `Renderer::set_post_aa`). |
| [`crates/tungsten-render/src/passes/recorder.rs`](../../crates/tungsten-render/src/passes/recorder.rs) `resolve_view` | Add match arms for `SmaaEdges`, `SmaaBlend`, `PresentSource`. Panic messages match the existing pattern (`"PresentSource requested but post_aa == Off"`). |

## Step 3 -- Vendored SMAA WGSL + LUT binaries

M27 deliberately uses three standalone WGSL modules. Do not add `smaa_common.wgsl`: the current loader validates manifest WGSL files one-by-one and has no include/preprocessor layer.

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/shaders/stock/smaa_edge.wgsl`](../../crates/tungsten-render/src/shaders/stock/smaa_edge.wgsl) | New standalone module. Luma-based edge detection with local contrast adaptation. `@group(0) binding(0) = source_tex; binding(1) = source_sampler; @group(1) binding(0) = preset_ubo`. Fragment writes edge values into `SmaaEdges`. |
| [`crates/tungsten-render/src/shaders/stock/smaa_blend_weights.wgsl`](../../crates/tungsten-render/src/shaders/stock/smaa_blend_weights.wgsl) | New standalone module. Reads edges + `area` + `search`. `@group(0) binding(0) = edges_tex; binding(1) = edges_sampler; @group(1) binding(0) = area_tex; binding(1) = search_tex; binding(2) = lut_sampler; binding(3) = preset_ubo`. |
| [`crates/tungsten-render/src/shaders/stock/smaa_neighborhood_blend.wgsl`](../../crates/tungsten-render/src/shaders/stock/smaa_neighborhood_blend.wgsl) | New standalone module. Reads source + blend weights. `@group(0) binding(0) = source_tex; binding(1) = source_sampler; @group(1) binding(0) = blend_tex; binding(1) = blend_sampler; binding(2) = preset_ubo`. |
| all three WGSL files | Header attribution: `// SMAA 1x -- Jorge Jimenez, Jose I. Echevarria, Tiago Sousa, Diego Gutierrez. MIT. https://www.iryoku.com/smaa/`. Keep helpers duplicated as needed so each file validates independently. |
| [`crates/tungsten-render/src/assets/smaa/area.bin`](../../crates/tungsten-render/src/assets/smaa/area.bin) | Raw bytes generated from upstream `Textures/AreaTex.h` `areaTexBytes`: `Rg8Unorm`, 160 x 560, length = `160 * 560 * 2`. |
| [`crates/tungsten-render/src/assets/smaa/search.bin`](../../crates/tungsten-render/src/assets/smaa/search.bin) | Raw bytes generated from upstream `Textures/SearchTex.h` `searchTexBytes`: `R8Unorm`, 64 x 16, length = `64 * 16`. |
| [`crates/tungsten-render/src/assets/smaa/ATTRIBUTION.md`](../../crates/tungsten-render/src/assets/smaa/ATTRIBUTION.md) | Citation of the [SMAA paper](https://www.iryoku.com/smaa/downloads/SMAA-Enhanced-Subpixel-Morphological-Antialiasing.pdf), [iryoku/smaa](https://github.com/iryoku/smaa) MIT license text, exact source files (`SMAA.hlsl`, `Textures/AreaTex.h`, `Textures/SearchTex.h`), and the byte-generation note. |
| [`crates/tungsten-render/src/post/smaa_luts.rs`](../../crates/tungsten-render/src/post/smaa_luts.rs) | New. `pub fn upload_area(device, queue) -> (Texture, TextureView)` and `upload_search(device, queue) -> (Texture, TextureView)` using `include_bytes!("../assets/smaa/area.bin")` and `search.bin`; assert byte lengths before upload. Reference guidance says samplers are linear + clamp, so use one linear clamp sampler for area/search unless a measured backend-specific point-sampling optimization is added later with a decision. |

## Step 4 -- Manifest mirror entries

| File | Edit |
| --- | --- |
| [`assets/shaders/stock/smaa_edge.wgsl`](../../assets/shaders/stock/smaa_edge.wgsl) | Byte-equal copy of the `include_str!` source. |
| [`assets/shaders/stock/smaa_blend_weights.wgsl`](../../assets/shaders/stock/smaa_blend_weights.wgsl) | Byte-equal copy. |
| [`assets/shaders/stock/smaa_neighborhood_blend.wgsl`](../../assets/shaders/stock/smaa_neighborhood_blend.wgsl) | Byte-equal copy. |
| [`assets/manifest.json`](../../assets/manifest.json) `shaders` | Append `"smaa_edge"`, `"smaa_blend_weights"`, and `"smaa_neighborhood_blend"` keyed entries. No `smaa_common` entry. |

## Step 5 -- `SmaaPipeline` in `post/smaa.rs`

Mirrors `PostStackRenderer::record_pass`: the renderer frame loop opens each `PassDesc` via `PassRecorder::begin`, then delegates to the matching `record_*_pass` method on `SmaaPipeline`. `SmaaPipeline` never calls `begin_render_pass` itself, keeping one source of truth for pass begin/clear semantics.

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/post/mod.rs`](../../crates/tungsten-render/src/post/mod.rs) | Add `pub mod smaa; pub mod smaa_luts;`. Do not touch `PostPass`; SMAA stays out of `PostStackRenderer`'s reorderable pass enum. |
| [`crates/tungsten-render/src/post/smaa.rs`](../../crates/tungsten-render/src/post/smaa.rs) | New. `struct SmaaPreset { threshold: f32, max_search_steps: u32, max_search_steps_diag: u32, corner_rounding: u32 }` + `fn from_mode(mode: PostAaMode) -> Option<Self>` (`None` for `Off`). |
| same file | `#[repr(C)] #[derive(Pod, Zeroable, Copy, Clone)] struct SmaaPresetUbo { threshold: f32, max_search_steps: f32, max_search_steps_diag: f32, corner_rounding: f32, rt_metrics: [f32; 4], _pad: [f32; 56] }` -- total 256 bytes. `rt_metrics = [1.0/w, 1.0/h, w as f32, h as f32]`. |
| same file | `struct SmaaLayouts { edge_source_bgl, edge_params_bgl, blend_input_bgl, blend_lut_bgl, nbh_input_bgl, nbh_params_bgl }`. Build in a `build_layouts(device)` free fn. |
| same file | `pub struct SmaaPipeline { edge_detect: RenderPipeline, blend_weights: RenderPipeline, neighborhood_blend: RenderPipeline, layouts: SmaaLayouts, preset_ubo: Buffer, preset_bg: BindGroup, area_tex: (Texture, TextureView), search_tex: (Texture, TextureView), linear_sampler: Sampler, edge_shader_id: ShaderAssetId, blend_shader_id: ShaderAssetId, nbh_shader_id: ShaderAssetId }`. |
| same file | `impl SmaaPipeline { pub fn new(device, queue, format, cache: &ShaderModuleCache, ids: SmaaShaderIds) -> Self; pub fn update_preset(&self, queue: &Queue, mode: PostAaMode, size: (u32, u32)); pub fn record_edge_pass(&self, device, render_pass: &mut RenderPass, pool, source: TargetId); pub fn record_blend_weights_pass(&self, device, render_pass: &mut RenderPass, pool); pub fn record_neighborhood_pass(&self, device, render_pass: &mut RenderPass, pool, source: TargetId); pub fn rebuild_stage_with_module(&mut self, device, format, shader_id: ShaderAssetId, module: &ShaderModule); }`. |
| same file | `source: TargetId` inputs to `record_edge_pass` / `record_neighborhood_pass` resolve to the SMAA read views on `SceneTarget`. The neighborhood source must match the edge source. |
| same file | Pipelines build from live `ShaderModuleCache` modules seeded by `Renderer::new` / updated by `upload_shader` and `reload_shader`. Edge pipeline writes `Rg8Unorm`; blend pipeline writes `Rgba8Unorm`; neighborhood pipeline writes `format` into `PresentSource`. |
| same file | `edge_pass_desc`, `blend_pass_desc`, `neighborhood_pass_desc` free fns return `PassDesc` values consumed by [`default_pass_order`](../../crates/tungsten-render/src/passes/order.rs). Edge + blend clear to `Color::TRANSPARENT`; neighborhood loads (fullscreen triangle coverage, no clear). |
| [`crates/tungsten-render/src/lib.rs`](../../crates/tungsten-render/src/lib.rs) | `pub use post::smaa::{SmaaPipeline, SmaaPreset};` if downstream tests/examples need it; otherwise keep it crate-private and export only `SmaaPreset` for tests. |

## Step 6 -- Pass-order splice + frame recording

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/passes/order.rs`](../../crates/tungsten-render/src/passes/order.rs) `default_pass_order` | New signature: `default_pass_order(msaa, depth_sort, depth_enabled, post_stack_len, post_aa)`. When `post_aa == Off`: behavior unchanged. When `post_aa != Off`: after the last post pass, append three SMAA passes -- edge (`SmaaEdges`, clear transparent), blend (`SmaaBlend`, clear transparent), neighborhood (`PresentSource`, no clear). Text overlay target becomes `PresentSource`. Present pass stays `Swapchain`. |
| same file `text_overlay_target` | New signature: `text_overlay_target(post_stack_len, post_aa)`. Returns `PresentSource` when SMAA is active, else existing logic. |
| [`crates/tungsten-render/src/passes/mod.rs`](../../crates/tungsten-render/src/passes/mod.rs) | Re-export the new signatures under the same public names. Internal-crate breaking change only; callers are `renderer.rs` and tests. |
| [`crates/tungsten-render/src/renderer.rs`](../../crates/tungsten-render/src/renderer.rs) `Renderer` fields | Add `post_aa: PostAaMode`, `smaa: Option<SmaaPipeline>`, and `smaa_shader_ids: SmaaShaderIds`. Seed the three SMAA shader names in the existing `shader_ids: HashMap<String, ShaderAssetId>` map so `upload_shader` / `reload_shader` can find them by stable manifest name. |
| same file `Renderer::new` | Read `config.post_aa` from the existing `RenderConfig`; pre-seed the render-side `ShaderModuleCache` from the three compile-time `include_str!` WGSL strings; allocate `SmaaPipeline` when enabled; pass `post_aa` into `RenderTargetPool::new`. Core `ShaderRegistry` registration still happens later in `asset_loader::load_shaders`, not in `Renderer::new`. |
| same file `Renderer::resize` | Forward `post_aa` into `target_pool.resize`; when `smaa.is_some()`, call `update_preset(&self.queue, self.post_aa, new_size)` so `rt_metrics` tracks the viewport. |
| same file `Renderer::set_post_aa` | New. Called only outside `render_frame_internal`. Swaps `self.post_aa`, rebuilds `smaa` (`None` <-> `Some`), updates preset UBO, and calls `target_pool.resize` so SMAA intermediates allocate/drop without relaunch. |
| same file `Renderer::upload_shader` and `Renderer::reload_shader` | After validation, when the shader id matches one of the three SMAA ids, rebuild the affected SMAA stage with the candidate module before committing it to `ShaderModuleCache`. If `post_aa == Off`, commit the module so a later `set_post_aa` builds from the latest source. Keep prior live pipelines on validation failure. |
| same file `render_frame_internal` | Compute `final_source_target = text_overlay_target(post_stack_len, self.post_aa)`. When SMAA is active, derive `smaa_edge_idx = post_stack_len + 1`, `smaa_blend_idx = smaa_edge_idx + 1`, `smaa_nbh_idx = smaa_blend_idx + 1`, `text_overlay_idx = smaa_nbh_idx + 1`; otherwise keep `text_overlay_idx = post_stack_len + 1`. |
| same function | Add branches `is_smaa_edge` / `is_smaa_blend` / `is_smaa_nbh`. Each opens the pass via `PassRecorder::begin` then delegates to the matching `SmaaPipeline::record_*_pass`. Edge source = post-stack final target or `SceneColor` when the stack is empty; neighborhood source matches. |
| same function | Resolve `final_source_view` to `SceneTarget::present_source_view()` when SMAA is active; otherwise keep the current `text_overlay_target(...)` selection. Present-blit bind group rebuilds against this live view every frame. |
| same function | Screenshot readback copies from `SceneTarget::present_source_texture()` when SMAA is active; otherwise keep the current `text_overlay_target`-based source selection. [`crates/tungsten-render/src/screenshot.rs`](../../crates/tungsten-render/src/screenshot.rs) itself does not change. |

## Step 7 -- Runtime post-AA request path in `tungsten`

| File | Edit |
| --- | --- |
| [`crates/tungsten/src/post_aa.rs`](../../crates/tungsten/src/post_aa.rs) | New umbrella module, mirroring the display request/apply shape: `pub struct PostAaState { pub mode: PostAaMode }`, crate-private `PendingPostAa(Option<PostAaMode>)`, public `request_post_aa(world: &mut World, mode: PostAaMode)`, crate-private `take_pending_post_aa(world)`, and tests. |
| [`crates/tungsten/src/app.rs`](../../crates/tungsten/src/app.rs) | Insert `PostAaState { mode: config.render.post_aa }` and `PendingPostAa::default()` in `App::new`. Add `apply_pending_post_aa_request` that calls `renderer.set_post_aa(mode)`, then updates `PostAaState`. Run it after `stage_hot_reload()` and before `stage_extract()` so system requests become visible to extract/HUD/render in the same frame without reallocating mid-render. |
| same file | `Renderer::new(window, &render_config, vsync)` already receives the full `RenderConfig`; no constructor signature change is needed once `Renderer::new` reads `config.post_aa`. |
| same file | Optional convenience: `App::set_post_aa(mode)` may write `PendingPostAa`, but examples/systems must use `request_post_aa(world, mode)` because systems do not receive `&mut App`. |
| [`crates/tungsten/src/lib.rs`](../../crates/tungsten/src/lib.rs) | `pub mod post_aa; pub use post_aa::{request_post_aa, PostAaState};`. |
| explicitly not changed | Do not wire `tungsten.json` into `HotReloadWatcher` for this milestone. `TUNGSTEN_RENDER_POST_AA` and startup `render.post_aa` cover config-driven launch; `request_post_aa` covers runtime changes. |

## Step 8 -- Shader playground hotkeys + capture artifact

| File | Edit |
| --- | --- |
| [`examples/04_shader_playground/src/main.rs`](../../examples/04_shader_playground/src/main.rs) | Parse `TUNGSTEN_POST_AA_FIXTURE=off|smaa_low|smaa_medium|smaa_high|smaa_ultra` after `Config::load` and before `App::new` by mutating `config.render.post_aa`. Unknown values should log/fail loudly like the existing config env path. |
| same file | Reuse the existing `ActionMap`-driven input pattern (`cycle_input_system`). Add actions `playground_cycle_post_aa` (Tab) and explicit `playground_post_aa_off` / `low` / `medium` / `high` / `ultra` (digits 0-4). Toggle via `tungsten::request_post_aa(world, mode)`, not `App::set_post_aa`. |
| [`input.json`](../../input.json) | Add the `playground_post_aa_*` bindings so checked-in controls match the example. If the example should work with missing `input.json`, inject fallback bindings into the runtime `ActionMap` during startup with `ActionMap::replace_bindings` (no persistence). |
| same file | Gate the interactive cycle behind the existing `TUNGSTEN_POST_STACK_FIXTURE` convention plus `TUNGSTEN_POST_AA_FIXTURE` so smoke runs stay deterministic. Default (no fixture): `PostAaMode::Off`. |
| same file | Extend the on-screen HUD row list to show `post_aa: <mode>` by reading `PostAaState`, so it reflects the applied renderer state rather than only the last keypress. |
| [`examples/04_shader_playground/assets/manifest.json`](../../examples/04_shader_playground/assets/manifest.json) | Add a high-contrast aliasing test sprite under `sprites`. Reuse an existing 1 px-feature sprite if one appears; otherwise add `alias_checker.png`. |
| [`docs/showcase/smaa_off_vs_high.png`](../showcase/smaa_off_vs_high.png) | 2-up PNG composed offline from two screenshots: `post_aa = off` left, `post_aa = smaa_high` right, captured from `example-04-shader-playground` on the checker scene at the default viewport. |
| [`docs/showcase/README.md`](../showcase/README.md) | New or extended index entry linking the capture and how to regenerate with `TUNGSTEN_CAPTURE_FRAME`, `TUNGSTEN_CAPTURE_PATH`, `TUNGSTEN_CAPTURE_RESOLUTION`, and `TUNGSTEN_POST_AA_FIXTURE`. Do not mention F10; screenshot capture is currently env-driven. |

## Step 9 -- Tests

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/tests/passes_order.rs`](../../crates/tungsten-render/src/tests/passes_order.rs) | Add `post_aa_off_matches_m26_baseline` over `msaa in {1,4}` x `depth_sort in {CpuStable,GpuDepth}` x `post_stack_len in {0,1,3}`. Add `post_aa_smaa_inserts_three_passes` for `post_aa = SmaaHigh` and `post_stack_len = 2`, asserting labels/indices/targets/clears. Add `text_overlay_target_smaa_returns_present_source`. Add `non_srgb_twin_roundtrips` for `Rgba8UnormSrgb -> Rgba8Unorm` and `Bgra8UnormSrgb -> Bgra8Unorm`; linear-in -> `None`. |
| [`crates/tungsten-render/src/tests/smaa.rs`](../../crates/tungsten-render/src/tests/smaa.rs) | New. `preset_mode_off_returns_none`. `preset_from_mode_matches_reference` with Low `threshold = 0.15`, Medium `threshold = 0.10`, High `max_search_steps = 16`, Ultra `max_search_steps = 32`. `preset_ubo_size_is_256`. LUT byte-length tests for `area.bin` and `search.bin`. No GPU required. |
| [`crates/tungsten-core/src/tests/config.rs`](../../crates/tungsten-core/src/tests/config.rs) | Step 1 cases: default `Off`, each mode parses, unknown errors with `POST_AA_EXPECTED`, `TUNGSTEN_RENDER_POST_AA=smaa_medium` flips the field. |
| [`crates/tungsten/src/tests/post_aa.rs`](../../crates/tungsten/src/tests/post_aa.rs) | New. `request_post_aa_replaces_pending`, `take_pending_post_aa_clears`, and `post_aa_state_tracks_initial_config` if the state initializer is factored for testing. |
| [`scripts/smoke-examples.sh`](../../scripts/smoke-examples.sh) | Extend the shader-playground matrix with an M27 row, at minimum `TUNGSTEN_POST_STACK_FIXTURE=empty TUNGSTEN_POST_AA_FIXTURE=smaa_high`. Keep default Off covered by the ordinary all-examples smoke run and/or the existing `fixture=empty` row. |
| `cargo test --workspace` | Runs all non-GPU tests above. |

## Step 10 -- Decision entry + doc sync

| File | Edit |
| --- | --- |
| [`DECISIONS.md`](../../DECISIONS.md) | Append `## D-059 -- M27 SMAA presentation AA as a renderer-owned tail`. **Decision:** SMAA 1x is a renderer-owned presentation stage exposed through `render.post_aa` (`Off | SmaaLow | SmaaMedium | SmaaHigh | SmaaUltra`). It runs as a fixed three-pass tail (edge detect -> blend weights -> neighborhood blend) after the reorderable `PostStack` and before screen-space text. Lookup textures `area` / `search` ship as `include_bytes!` engine content with attribution (not manifest-tracked). SMAA WGSL uses three standalone manifest-tracked stage shaders; body-edit reload goes through `Renderer::upload_shader` / `reload_shader` + `SmaaPipeline` stage rebuild, leaving prior pipelines live on validation failure. Preset knobs ride a 256-byte UBO, so switching presets neither rebuilds nor recompiles. `SceneColor` and post ping/pong targets carry non-sRGB twin views while SMAA is active so edge detection sees gamma-encoded values. `post_aa = Off` is byte-identical to the M26 frame. Flipping `post_aa` reallocates intermediates at a frame boundary -- no relaunch required, unlike `msaa`. No new runtime dependency. **Why:** presentation AA must see fully shaded post output and must not smooth screen-space text; making it a reorderable `PostPass` violated both constraints. Narrows `D-058`, extends `D-053`, and narrows `D-023` the same way `D-057` did. |
| [`docs/DECISION_INDEX.md`](../DECISION_INDEX.md) | Add a row to Assets / Rendering for `D-059`: fixed SMAA tail after `PostStack`, before text overlay; `PostAaMode` in `RenderConfig`; lookup textures `include_bytes!` only; preset via 256-byte UBO; runtime request path; `Off` byte-identical to M26; no new dependency. |
| [`docs/LLM_INDEX.md`](../LLM_INDEX.md) | Add an M27 post-AA row pointing to `crates/tungsten-core/src/config.rs`, `crates/tungsten/src/post_aa.rs`, `crates/tungsten-render/src/post/smaa.rs`, `crates/tungsten-render/src/targets.rs`, and `crates/tungsten-render/src/passes/order.rs`. |
| [`AGENTS.md`](../../AGENTS.md) | Under the shader/hot-reload block, add a bullet: SMAA stage shaders follow the stock/mirror pattern and register with `ShaderRegistry`; `area` / `search` LUT binaries ship as `include_bytes!` engine content, not manifest-tracked. Frame-order bullet: presentation AA is a renderer-owned tail; text overlay always runs after it. |
| [`DESIGN.md`](../../DESIGN.md) | Update the frame-order description to `Scene -> PostStack -> [optional SMAA tail -> PresentSource] -> Text Overlay -> Present Blit -> Swapchain`, and list `render.post_aa` in the render-config sample/table. |
| [`CHANGELOG.md`](../../CHANGELOG.md) | New entry under the `0.24` section: "M27 -- SMAA 1x presentation AA (`Off` default, `SmaaLow` / `SmaaMedium` / `SmaaHigh` / `SmaaUltra`). `render.post_aa` config key; renderer-owned three-pass tail between `PostStack` and text overlay; runtime changes through `request_post_aa`; no new runtime deps. See `D-059`." |
| [`README.md`](../../README.md) | Status block: mark M27 shipped. |
| [`docs/plans/phase4.md`](phase4.md) | Flip M27 section status to `done -- shipped in 0.24`. Reference this plan's archive path. |
| [`docs/plans/phase4-milestone-27-smaa-presentation-aa.md`](phase4-milestone-27-smaa-presentation-aa.md) | Flip front-matter `status: draft` -> `status: done`; move file to `docs/plans/archive/phase4-milestone-27-smaa-presentation-aa.md` on ship. |

## Sources

- [SMAA official site](https://www.iryoku.com/smaa/) -- paper, reference implementation link, and mode history.
- [SMAA.hlsl](https://github.com/iryoku/smaa/blob/master/SMAA.hlsl) -- preset macro values, sampler guidance, and three-stage algorithm shape.
- [Textures/AreaTex.h](https://github.com/iryoku/smaa/blob/master/Textures/AreaTex.h) and [Textures/SearchTex.h](https://github.com/iryoku/smaa/blob/master/Textures/SearchTex.h) -- LUT dimensions, formats, and source bytes.
- [wgpu TextureDescriptor `view_formats`](https://docs.rs/wgpu/latest/wgpu/struct.TextureDescriptor.html#structfield.view_formats) -- needed to expose a non-sRGB twin view while still using the sRGB format for scene/present attachments.
- [wgpu TextureFormat](https://docs.rs/wgpu/latest/wgpu/enum.TextureFormat.html) -- `Rg8Unorm` / `R8Unorm` portability for SMAA LUTs + edges target.
- [wgpu RenderPipelineDescriptor](https://docs.rs/wgpu/latest/wgpu/struct.RenderPipelineDescriptor.html) -- fullscreen-triangle pipeline pattern already used by `post/fullscreen.rs`.
