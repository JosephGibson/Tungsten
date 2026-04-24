---
status: done
milestone: M25
goal: "Ship M25 Render Foundation: offscreen `SceneTarget` (color+depth, optional MSAA), ordered named-pass list, WGSL hot reload with `naga` validation, opt-in GPU depth-test sprite path. Under the default config (`msaa=1`, `depth_sort=cpu_stable`) output is byte-identical to the `0.21` baseline (image-diff asserted)."
non-goals:
  - "No post-stack, materials, bloom, lighting (M26–M28)."
  - "No DAG pass graph; named targets + linear ordered `PassOrder`."
  - "No shader signature hot reload; body-edit only. Signature changes still require rebuild."
  - "No disk-baked shader cache; `ShaderModuleCache` is in-memory per session."
  - "No shader preprocessor/include system; one `.wgsl` = one `ShaderAssetId`."
  - "No HDR scene target in M25. `SceneColor` format equals the swapchain sRGB format; M27 adds a sibling HDR target for bloom input."
  - "No new global mutable state, no new background threads beyond the existing `notify` channel."
files to touch:
  - "crates/tungsten-render/src/renderer.rs (split surface/config/timing; route through passes)"
  - "crates/tungsten-render/src/surface.rs (new; extracted from renderer)"
  - "crates/tungsten-render/src/timing.rs (new; extracted CPU/GPU timing bookkeeping)"
  - "crates/tungsten-render/src/targets.rs (new)"
  - "crates/tungsten-render/src/passes/mod.rs (new)"
  - "crates/tungsten-render/src/passes/desc.rs (new)"
  - "crates/tungsten-render/src/passes/recorder.rs (new)"
  - "crates/tungsten-render/src/passes/order.rs (new)"
  - "crates/tungsten-render/src/shader_hot_reload.rs (new)"
  - "crates/tungsten-render/src/shaders/present_blit.wgsl (new; engine-internal, `include_str!`)"
  - "crates/tungsten-render/src/lib.rs (re-exports)"
  - "crates/tungsten-render/src/sprite.rs (sample_count arg; optional depth-stencil; `z_norm` instance attr)"
  - "crates/tungsten-render/src/sprite.wgsl (deleted; source of truth moves to workspace assets/)"
  - "crates/tungsten-render/src/quad.rs (sample_count arg)"
  - "crates/tungsten-render/src/debug_line.rs (sample_count arg)"
  - "crates/tungsten-render/src/text.rs (thread `MultisampleState` to glyphon `TextRenderer::new`)"
  - "crates/tungsten-core/src/assets/shader.rs (new; `ShaderAssetId`, `ShaderRegistry`)"
  - "crates/tungsten-core/src/assets/manifest.rs (add `shaders` keyed section)"
  - "crates/tungsten-core/src/assets/mod.rs (re-export)"
  - "crates/tungsten-core/src/config.rs (`render.msaa`, `render.depth_enabled`, `render.depth_sort`, env overrides for smoke matrix)"
  - "crates/tungsten-core/src/tests/config.rs (render-config parse + env-override coverage)"
  - "crates/tungsten/src/asset_loader.rs (`load_shaders`, `reload_shader`, bridge `ShaderRegistry` ↔ renderer)"
  - "crates/tungsten/src/app.rs (dispatch `.wgsl` to `reload_shader`; init order)"
  - "crates/tungsten/src/tests/asset_loader.rs (shader loader/reload last-known-good tests)"
  - "crates/tungsten/src/tests/sprite_extract.rs (deterministic depth-order tests)"
  - "tungsten.json (add `render.msaa`, `render.depth_enabled`, `render.depth_sort` with defaults)"
  - "assets/manifest.json (register `sprite` shader under new `shaders` section)"
  - "assets/shaders/sprite.wgsl (new; byte-identical to current `crates/tungsten-render/src/sprite.wgsl`)"
  - "examples/01_platformer/src/setup.rs (shared-assets hot-reload audit)"
  - "crates/tungsten-core/src/tests/assets/manifest.rs (unit test for `shaders` section)"
  - "crates/tungsten-render/src/tests/renderer.rs (device-free render-path helpers such as `default_pass_order`)"
  - "scripts/smoke-examples.sh (add msaa=4 and depth_sort=gpu_depth matrix runs)"
  - "docs/showcase/m25-sprite-stress-baseline.png (committed reference frame for image-diff)"
  - "DECISIONS.md (new `D-0NN` narrowing D-023; cross-link D-053; records `SceneColor = swapchain sRGB format` for M25)"
  - "docs/DECISION_INDEX.md (one-line row for new D-0NN)"
  - "AGENTS.md (Asset Rules: shaders become manifest-tracked, wgsl in recursive watch scope)"
  - "DESIGN.md (Status + Hot Reload matrix row: `shader` / body-edit only; `SceneColor` format note)"
  - "CHANGELOG.md (Unreleased bullet)"
done-when:
  - "`cargo test --workspace` green (Layer 1 picks up `shaders` manifest section; new unit tests for `ShaderRegistry`, `ResolvedManifest::shaders`, `default_pass_order`)."
  - "`./scripts/smoke-examples.sh` green under default config (`msaa=1`, `depth_enabled=true`, `depth_sort=cpu_stable`)."
  - "Image-diff assertion: `example-02-sprite-stress` frame 60 under default config matches the committed `docs/showcase/m25-sprite-stress-baseline.png` within `image_diff` tolerance (`<= 1` pixel mean abs diff per channel). Capture reuses the existing `TUNGSTEN_CAPTURE_FRAME` / `TUNGSTEN_CAPTURE_PATH` hooks in `App`."
  - "Under `render.depth_sort = \"gpu_depth\"`, a component-driven capture fixture (current candidate: `example-03_scene_state`, which uses the default sprite extract and real `z_order` values) is image-diff-equal to the `cpu_stable` reference frame at the same scene frame, and unit tests lock down deterministic same-`z_order` tie behavior."
  - "MSAA matrix smoke: running `example-02-sprite-stress` under all four `{msaa ∈ {1,4}} × {depth_sort ∈ {cpu_stable, gpu_depth}}` combinations exits cleanly (3 frames, no panics) via the extended `scripts/smoke-examples.sh`."
  - "Editing `assets/shaders/sprite.wgsl` (body-only, e.g. swap `color.rgb` for `vec3<f32>(1.0) - color.rgb`) while `example-01-platformer` runs updates rendered output within < ~200 ms, no rebuild."
  - "Editing `assets/shaders/sprite.wgsl` to introduce a parse/validation or pipeline-rebuild error logs `shader 'sprite' validation failed: ...` (or equivalent rebuild failure) and keeps the old `ShaderModule` + live pipeline; app does not panic."
  - "`DECISIONS.md` has the new `D-0NN` entry (number resolved via `rg -n '^### D-0' DECISIONS.md | tail -1`); `docs/DECISION_INDEX.md` has the matching row; `AGENTS.md`, `DESIGN.md`, and `CHANGELOG.md` are updated in the same change."
- "Plan file flipped to `status: done`."
---

# Phase 4 Milestone 25 — Render Foundation

## Context Digest

Tungsten `0.22` currently renders directly to the swapchain through one forward pass, matching the `0.21` release baseline: `render_frame_full` clears the surface, then draws quads → sprites → debug quads → debug lines → text in order, all inside one `begin_render_pass` ([renderer.rs:471-494](crates/tungsten-render/src/renderer.rs#L471-L494)). No depth, no MSAA, no offscreen target. Call sites at [app.rs:775-793](crates/tungsten/src/app.rs#L775-L793).

Shaders are source-embedded via `include_str!` at pipeline construction ([sprite.rs:151](crates/tungsten-render/src/sprite.rs#L151), [quad.rs:84](crates/tungsten-render/src/quad.rs#L84), [debug_line.rs:80](crates/tungsten-render/src/debug_line.rs#L80)); text uses `glyphon`. `D-023` captures this. `D-053` publishes the hot-reload matrix. M25 adds a new row (`shader` — body-edit only) that narrows `D-023` rather than reversing it.

Manifest shape is the keyed `HashMap<String, Entry>` pattern ([manifest.rs:36-49](crates/tungsten-core/src/assets/manifest.rs#L36-L49)); merge is duplicate-ID fatal (`D-017`). Hot reload: `notify`, 50 ms debounce, recursive watches over `assets/` plus per-example `assets/` ([hot_reload.rs:24-107](crates/tungsten/src/hot_reload.rs#L24-L107), [app.rs:310-312](crates/tungsten/src/app.rs#L310-L312)). Extension routing at [app.rs:359-466](crates/tungsten/src/app.rs#L359-L466). M25 should reuse this existing umbrella-owned watcher path rather than create a second watcher in `tungsten-render`.

`RenderConfig` holds `clear_color`, `present_mode`, `max_frame_latency` ([config.rs:121-137](crates/tungsten-core/src/config.rs#L121-L137)). Phase 4 adds `msaa: u32`, `depth_enabled: bool`, `depth_sort: DepthSortMode` (`CpuStable` default, `GpuDepth` opt-in). If Layer 2 needs a launch matrix without rewriting tracked config files, the existing render-env-override pattern in `config.rs` should be extended for these new knobs. `Sprite.z_order: i32` already exists ([components.rs:47](crates/tungsten-core/src/components.rs#L47)); no component change needed.

Core/render seam (`D-007`, `D-016`): no `wgpu` in `tungsten-core`. `ShaderAssetId(u32)` + `ShaderRegistry` live core-side; compiled `wgpu::ShaderModule` + live WGSL text live render-side in `ShaderModuleCache`, keyed by the same `ShaderAssetId`. `naga` is reachable as `wgpu::naga` (wgpu 29 re-export) — no separate dep needed.

## Architecture

```
tungsten-core                      tungsten                         tungsten-render
-------------                      --------                         ---------------
ShaderAssetId(u32) ─────────────── asset_loader::load_shaders ───── ShaderModuleCache
ShaderRegistry     (id ↔ path)        → renderer.upload_shader(id)     (id → (text, Module))
RawManifest.shaders                asset_loader::reload_shader       shader_hot_reload
RenderConfig { msaa,               app::process_hot_reload routes   targets.rs
  depth_enabled, depth_sort }        *.wgsl → reload_shader           SceneTarget { color, depth, msaa? }
                                   (existing recursive watch        passes/ { PassDesc, Recorder, Order }
                                    catches assets/shaders/*.wgsl)  surface.rs  timing.rs  (split out)
                                                                  shader_hot_reload = cache + validation only
```

### Scene Target Formats (M25)

| Target | Format | Rationale |
| --- | --- | --- |
| `SceneColor` | swapchain sRGB (e.g. `Bgra8UnormSrgb`) | byte-identical to 0.21 baseline when blitted to swapchain; trivial present pass; no tonemap yet |
| `SceneColorMsaa` (when `msaa>1`) | same as `SceneColor`, `sample_count = msaa` | resolves into `SceneColor` via `resolve_target` |
| `SceneDepth` (when `depth_enabled`) | `Depth32Float` | portable; matches wgpu limits |
| `Swapchain` | surface format | present blit target |

M27 will add an HDR sibling (`Rgba16Float`) as the bloom input; M25 does not allocate it.

### Pass List (default `depth_sort=cpu_stable`, `msaa=1`)

| # | Pass | Inputs | Output | Notes |
| --- | --- | --- | --- | --- |
| 1 | `scene` | quads, sprites, debug_quads, debug_lines, text | `SceneColor` | single load-clear; no depth writes |
| 2 | `present` | `SceneColor` | `Swapchain` | fullscreen-triangle blit, `present_blit.wgsl` |

Under `depth_sort=gpu_depth`, pass 1 binds `SceneDepth`; sprite pipeline writes `ndc.z = z_norm` from the same deterministic painter ordering used by the CPU path (stable `z_order` with an explicit tie-break) and depth-test is `LessEqual`. Under `msaa>1`, `SceneColorMsaa` is the color attachment and resolves into `SceneColor`.

### Hot Reload Matrix Delta

| asset | current | M25 |
| --- | --- | --- |
| shader (`.wgsl`) | rebuild required (D-023) | body-edit hot, signature/bind-group change needs rebuild |

## Ordered Steps

### 1. Config surface + override hooks
- File: `crates/tungsten-core/src/config.rs`, `crates/tungsten-core/src/tests/config.rs`, `tungsten.json`.
- Action:
  - Do **not** add a second `notify` dependency or watcher in `tungsten-render`; reuse the existing umbrella hot-reload path. Do **not** add a separate `naga` dep; use the existing `wgpu::naga` re-export for WGSL parse + semantic validation.
  - In `RenderConfig`:
    ```rust
    #[derive(Debug, Clone, Copy, Default, Deserialize)]
    #[serde(rename_all = "snake_case")]
    pub enum DepthSortMode { #[default] CpuStable, GpuDepth }
    ```
    Extend `RenderConfig`:
    ```rust
    #[serde(default = "default_msaa")]        pub msaa: u32,           // 1 | 2 | 4 | 8
    #[serde(default = "default_depth_on")]    pub depth_enabled: bool, // true
    #[serde(default)]                         pub depth_sort: DepthSortMode,
    ```
    Add validation for `msaa ∈ {1, 2, 4, 8}` and unknown-variant → parse error coverage.
  - Extend the existing env-override surface with `TUNGSTEN_RENDER_MSAA`, `TUNGSTEN_RENDER_DEPTH_ENABLED`, and `TUNGSTEN_RENDER_DEPTH_SORT` so the smoke matrix can vary render settings without rewriting tracked `tungsten.json`.
  - Mirror defaults in `tungsten.json` `render` block (strings for `depth_sort`: `"cpu_stable"` / `"gpu_depth"`).
- Verify: `cargo build -p tungsten-core -p tungsten-render`; `cargo test -p tungsten-core config::`; new tests assert `serde_json::from_str::<RenderConfig>("{}")` returns defaults, `"depth_sort": "gpu_depth"` parses, unsupported `msaa` is rejected, and the new env overrides land on `RenderConfig`.

### 2. `ShaderAssetId` + `ShaderRegistry` (core)
- File: `crates/tungsten-core/src/assets/shader.rs` (new); re-export in `crates/tungsten-core/src/assets/mod.rs`.
- Action: `pub struct ShaderAssetId(pub u32)` (`Copy, Eq, Hash`). `pub struct ShaderRegistry { next: u32, ids: HashMap<String, ShaderAssetId>, paths: HashMap<ShaderAssetId, PathBuf>, reverse: HashMap<PathBuf, ShaderAssetId>, names: HashMap<ShaderAssetId, String> }` with `allocate(&mut self, id: &str, path: PathBuf) -> ShaderAssetId`, `get(&self, id: &str) -> Option<ShaderAssetId>`, `id_for_path(&self, p: &Path) -> Option<ShaderAssetId>`, `name_for_id(&self, ShaderAssetId) -> Option<&str>`. No `wgpu` types. Shape parallels `AnimationRegistry` / `ParticleConfigRegistry`.
- Verify: `cargo test -p tungsten-core assets::shader::` — unit tests cover allocate-then-lookup, path reverse, and double-allocate returning the same id.

### 3. Manifest `shaders` section
- File: `crates/tungsten-core/src/assets/manifest.rs`; `crates/tungsten-core/src/tests/assets/manifest.rs`.
- Action: add `pub struct ShaderEntry { pub path: String }` and `pub struct ResolvedShader { pub path: PathBuf }`. Add `shaders: HashMap<String, _>` to both `RawManifest` and `ResolvedManifest`, with the same missing-file / duplicate-ID / merge treatment as other sections (new error variant `MissingShaderFile { id, path }`). Update `ResolvedManifest::load` and `merge`.
- Verify: `cargo test --workspace` (Layer 1 rewalks `assets/manifest.json`); new unit `load_shaders_resolves_paths` next to existing manifest tests.

### 4. Move `sprite.wgsl` to asset path, register in root manifest
- File: `assets/shaders/sprite.wgsl` (new, byte-identical copy of `crates/tungsten-render/src/sprite.wgsl`); `assets/manifest.json`; `crates/tungsten-render/src/sprite.rs` (update `include_str!` path).
- Action:
  1. Copy `crates/tungsten-render/src/sprite.wgsl` to `assets/shaders/sprite.wgsl`.
  2. Change the `include_str!` in `sprite.rs:151` to `include_str!("../../../assets/shaders/sprite.wgsl")`, then delete the now-redundant in-src copy so compile-time default and manifest-tracked runtime source come from one file.
  3. Append to `assets/manifest.json`:
     ```json
     "shaders": {
       "sprite": { "path": "shaders/sprite.wgsl" }
     }
     ```
  4. `quad.wgsl` and `debug_line.wgsl` stay source-embedded under `crates/tungsten-render/src/` for M25; M26 migrates them when the materials work touches `quad.rs`.
- Verify: `cargo build --workspace` compiles with the new include path; `cargo test -p tungsten-core --test manifests` resolves the root manifest; the raw bytes of the embedded-at-compile text and the manifest-loaded text are equal (asserted by step 11's short-circuit test).

### 5. `ShaderModuleCache` + validation
- File: `crates/tungsten-render/src/shader_hot_reload.rs` (new); `crates/tungsten-render/src/lib.rs` re-exports.
- Action: `pub struct ShaderModuleCache { modules: HashMap<ShaderAssetId, Entry> }` with `Entry { text: String, module: wgpu::ShaderModule }`. API:
  - `upload(&mut self, &wgpu::Device, id: ShaderAssetId, name: &str, wgsl: String)` — validates via `wgpu::naga::front::wgsl::parse_str` + `wgpu::naga::valid::Validator`; returns `Err(ShaderError::Validation { name, report })` on fail; never mutates cache on fail.
  - `reload(...)` — same validation, but only commits the new `Entry` after the dependent pipeline rebuild succeeds; on validation or rebuild failure it logs and leaves the existing entry/pipeline untouched.
  - `get(&self, id) -> Option<&wgpu::ShaderModule>`.
  - `text(&self, id) -> Option<&str>` — used by step 11 for byte-equality short-circuit.
- `SpritePipeline` stores `Option<ShaderAssetId>` and a rebuild hook; the renderer stages `SpritePipeline::rebuild_with_shader(...)` before swapping the live cache entry so last-known-good remains intact on failure.
- Verify: `cargo test -p tungsten-render shader_hot_reload::` — unit tests: (a) valid WGSL populates cache; (b) malformed WGSL returns `Err` and the pre-existing `Entry` is untouched; (c) semantic-validation failure is rejected; (d) `text()` round-trips the source.

### 6. `SceneTarget` + `RenderTargetPool`
- File: `crates/tungsten-render/src/targets.rs`.
- Action: `pub enum TargetId { SceneColor, SceneDepth, SceneColorMsaa, Swapchain }`. `pub struct SceneTarget { pub color: (Texture, TextureView), pub depth: Option<(Texture, TextureView)>, pub color_msaa: Option<(Texture, TextureView)>, pub size: (u32, u32), pub sample_count: u32, pub format: TextureFormat }`. `pub struct RenderTargetPool { scene: SceneTarget }` with `new(&Device, size, format, msaa, depth_enabled)` + `resize(...)`. Do not reserve future post-stack targets in M25; M26 can extend `TargetId` when it actually allocates `PostPing` / `PostPong`.
- Verify: no headless-device harness exists, so **Layer-2 only**: step 13's MSAA matrix exercises all three allocation shapes.

### 7. `passes/` scaffolding
- File: `crates/tungsten-render/src/passes/{mod,desc,recorder,order}.rs`.
- Action:
  - `PassDesc { label: &'static str, color: TargetId, color_resolve: Option<TargetId>, depth: Option<TargetId>, clear: Option<wgpu::Color>, depth_clear: Option<f32> }`.
  - `PassOrder(Vec<PassDesc>)`.
  - `PassRecorder::begin<'a>(encoder: &'a mut CommandEncoder, desc: &PassDesc, pool: &'a RenderTargetPool, swap_view: &'a TextureView) -> RenderPass<'a>` — resolves `TargetId` to views; sets `resolve_target` iff `color_resolve.is_some()`.
  - `pub fn default_pass_order(msaa: u32, depth_sort: DepthSortMode) -> PassOrder`.
- Verify: unit test (no device needed — `PassOrder` is pure data): `default_pass_order(4, GpuDepth)` produces exactly `[scene{color=SceneColorMsaa, resolve=SceneColor, depth=SceneDepth, clear=Some, depth_clear=Some(1.0)}, present{color=Swapchain, resolve=None, depth=None, clear=None}]`.

### 8. Split `renderer.rs` into surface + timing modules
- File: `crates/tungsten-render/src/renderer.rs` → source into `surface.rs` + `timing.rs` siblings.
- Action:
  - Move `resolve_present_mode`, `resolve_max_frame_latency`, `present_mode_label`, `requested_present_mode_label`, `choose_auto_*`, `available_present_mode_labels`, the `SurfaceConfiguration` build, and `reconfigure_surface_pacing` body into `surface.rs` under `pub fn build_surface_config(...) -> Result<(SurfaceConfiguration, GpuFrameTimings), RenderError>` + `pub fn reconfigure_pacing(...)`.
  - Move `CpuFrameTimings`, `GpuFrameTimings`, timestamp query plumbing (from `render_frame_full_timed`) into `timing.rs` as `FrameTimer` + `GpuTimestampQuery`.
  - Re-export both from `renderer.rs` so external callers remain source-compatible.
- Verify: `cargo build -p tungsten-render`; `cargo test -p tungsten-render` + `cargo test -p tungsten`; `rg -n "resolve_present_mode|reconfigure_surface_pacing" crates/tungsten-render/src/renderer.rs` prints only re-export lines.

### 9. Route main frame through `PassOrder` + `present_blit.wgsl`
- File: `crates/tungsten-render/src/renderer.rs`; `crates/tungsten-render/src/shaders/present_blit.wgsl` (new, `include_str!`); `crates/tungsten-render/src/lib.rs`.
- Action:
  - Allocate `RenderTargetPool` in `Renderer::new`; store on `self`; `resize` forwards to pool resize. `SceneColor` format equals `self.surface_config.format` (swapchain sRGB) — see Architecture §Scene Target Formats.
  - Build a `PresentBlitPipeline { module, pipeline, bind_group }` that copies `SceneColor` to `Swapchain` with exact texel fetches (`textureLoad`-style, no filtering) so the default path can stay image-diff-equal to the `0.21` baseline. The shader is a single fullscreen triangle (no vertex buffer; `@builtin(vertex_index)`-driven). It stays source-embedded (`include_str!`) — **not** manifest-tracked; `present_blit.wgsl` is engine-internal.
  - Replace the `begin_render_pass` block in `render_frame_full` (currently [renderer.rs:471-494](crates/tungsten-render/src/renderer.rs#L471-L494)) with a loop over `default_pass_order(msaa, depth_sort).0`:
    - `scene` → `record_main_draws(&mut pass, quads, sprite_batches, debug_quads, debug_lines)` + `text_pipeline.render(&mut pass)`.
    - `present` → bind `SceneColor` view, draw 3 vertices.
  - Screenshot path: remove the duplicate main render pass at [renderer.rs:496-545](crates/tungsten-render/src/renderer.rs#L496-L545). Replace it with copy/readback directly from `SceneColor` where formats allow, and fall back to one exact-load blit only when a capture-format reinterpretation is required.
- Verify: `./scripts/smoke-examples.sh` at defaults; **image-diff assertion** — `capture_frame` frame 60 of `example-02-sprite-stress` matches `docs/showcase/m25-sprite-stress-baseline.png` within `image_diff::assert_within(tolerance = 1)` (uses existing `crates/tungsten-render/src/image_diff.rs`). Capture is driven by the already-shipped `TUNGSTEN_CAPTURE_FRAME=60` + `TUNGSTEN_CAPTURE_PATH=<path>` hooks in `App`. Commit the baseline PNG as part of this step.

### 10. MSAA propagation across all scene pipelines
- File: `crates/tungsten-render/src/sprite.rs`, `quad.rs`, `debug_line.rs`, `text.rs`.
- Action: add `sample_count: u32` to each constructor. Plumb `wgpu::MultisampleState { count: sample_count, mask: !0, alpha_to_coverage_enabled: false }` into the `RenderPipelineDescriptor` at all four sites (`sprite.rs:243`, `quad.rs:150`, `debug_line.rs:115`, plus `TextRenderer::new(&mut atlas, device, MultisampleState { count, .. }, None)` in `text.rs:72`). `Renderer::new` reads `msaa` from `RenderConfig` and passes it to each.
- Rebuild hooks: when `RenderTargetPool` is resized under a different `msaa`, each pipeline is rebuilt — M25 uses the simple path of rebuilding all four on `RenderConfig` changes; live msaa swap is not supported (documented as "requires relaunch" in the DECISIONS entry).
- Verify: step 13's MSAA smoke matrix must not panic on `msaa=4`; under `msaa=1` the matrix is a no-op beyond the default path already covered by `./scripts/smoke-examples.sh`.

### 11. Umbrella `asset_loader` shader path + byte-equal short-circuit
- File: `crates/tungsten/src/asset_loader.rs`; `crates/tungsten/src/app.rs` (init order).
- Action:
  - `pub fn load_shaders(manifest: &ResolvedManifest, world: &mut World, renderer: &mut Renderer) -> anyhow::Result<()>` — iterate `manifest.shaders`, allocate `ShaderAssetId` in `ShaderRegistry`, read WGSL bytes, call `renderer.upload_shader(id, name, text)`.
  - **Byte-equal short-circuit:** `Renderer::upload_shader` pre-seeds the cache at `Renderer::new` with the compile-time `include_str!` bytes keyed by the well-known id `"sprite"`. `load_shaders` then diffs incoming text against the cached text. If equal, the call is a no-op and no pipeline rebuild happens — critical for the "byte-identical to 0.21 baseline" done-when and avoids a first-frame stall.
  - `pub fn reload_shader(id: &str, path: &Path, world: &mut World, renderer: &mut Renderer) -> anyhow::Result<()>` — look up `ShaderAssetId`, read WGSL, call `renderer.reload_shader(id, text)`. `naga` validation errors are logged at `log::error!` and swallowed; caller returns `Ok(())` so hot-reload keeps running.
  - Call order in `load_all`: sprites → animations → fonts → **shaders** → sounds → tilemaps → particles. App init order: `Renderer::new` (builds pipelines from `include_str!` defaults) → `asset_loader::load_all` (runs `load_shaders`, which no-ops when bytes match) → first frame.
- Verify: add a unit test in `asset_loader::tests` driving `load_shaders` twice against the same bytes — second call reports "unchanged" via a counter on `ShaderModuleCache`. Manual: start `example-01-platformer`, edit `assets/shaders/sprite.wgsl`, confirm visual change without rebuild.

### 12. Hot-reload routing + example audit
- File: `crates/tungsten/src/app.rs` (`process_hot_reload`); `examples/01_platformer/src/setup.rs`.
- Action:
  - Extend the extension match in `process_hot_reload` (around [app.rs:380](crates/tungsten/src/app.rs#L380)) with `"wgsl" => { reload_shader(...) }`. Look up the id via `ShaderRegistry::id_for_path(&canon)` identically to the existing particle/animation path.
  - **Example audit**: audit every *current* hot-reload-enabled example. Today that means [01_platformer/src/setup.rs:58](examples/01_platformer/src/setup.rs#L58), which already passes both `ASSETS_ROOT` and `ASSETS_LOCAL` — good. If M25 turns on hot reload in any additional example, ensure the workspace root `assets/` directory is included there in the same change.
- Verify: `rg -n "enable_hot_reload" examples/*/src/` still points at the intended call sites, and each such site includes `ASSETS_ROOT`. Manual: launch `example-01-platformer`, `touch assets/shaders/sprite.wgsl`, confirm `shader 'sprite' reloaded` log line.

### 13. MSAA + depth-sort smoke matrix
- File: `scripts/smoke-examples.sh`.
- Action: keep the current "all examples once" loop, then append four targeted `example-02-sprite-stress` runs using the new env overrides from step 1: `{TUNGSTEN_RENDER_MSAA=1, TUNGSTEN_RENDER_DEPTH_SORT=cpu_stable}`, `{4, cpu_stable}`, `{1, gpu_depth}`, `{4, gpu_depth}`. Each runs under `TUNGSTEN_SMOKE_FRAMES=3` and the existing per-example timeout. This avoids editing tracked config files during the smoke script.
- Verify: `./scripts/smoke-examples.sh` exits 0 with all four matrix rows reported pass.

### 14. GPU depth-test sprite path (image-diff verification)
- File: `crates/tungsten-render/src/sprite.rs`; `assets/shaders/sprite.wgsl`; `crates/tungsten-core/src/components.rs` (no change — `z_order` stays `i32`); `crates/tungsten/src/sprite_extract.rs` (emit `z_norm`); `crates/tungsten/src/tests/sprite_extract.rs`.
- Action:
  - Add `z_norm: f32` to `SpriteInstance`.
  - Make same-`z_order` painter order explicit in `extract_sprites_default` by sorting on `(z_order, entity.id())` rather than relying on implicit query order. Under `CpuStable`, keep the existing z-run batching behavior; under `GpuDepth`, derive a monotonic `z_norm` from that same deterministic painter ordering so the depth buffer reproduces the CPU-visible order instead of approximating only by raw `z_order`.
  - WGSL: switch path via a pipeline constant (`override use_depth: bool = false;`) — sprite pipeline has two variants built at startup, selected per frame by `depth_sort`. Avoids runtime branching.
  - Under `GpuDepth`: `depth_stencil: Some(DepthStencilState { format: Depth32Float, depth_write_enabled: true, depth_compare: LessEqual, stencil: Default::default(), bias: Default::default() })`.
  - Add/extend `sprite_extract` tests so same-`z_order` overlaps stay deterministic and `GpuDepth` uses the exact same tie-break contract as `CpuStable`.
- Verify: image-diff — capture a component-driven scene under `depth_sort=cpu_stable` and `depth_sort=gpu_depth` (current candidate: `example-03_scene_state`, because it uses the default sprite extract and real `Sprite.z_order` data). Both must match within `image_diff::assert_within(1)`. Separately, unit tests prove the exact same-`z_order` tie behavior that the image-diff capture cannot guarantee by itself.

### 15. DECISIONS + docs sync
- File: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `AGENTS.md`, `DESIGN.md`, `CHANGELOG.md`.
- Action:
  - Resolve the new decision number: `rg -n '^### D-0' DECISIONS.md | tail -1` → increment the last digit. Call this `D-0NN` below.
  - New `D-0NN` narrowing `D-023`: "Shader WGSL is manifest-tracked and hot-reloadable on body edits; signature / bind-group layout changes still require rebuild. Validation uses `wgpu::naga::front::wgsl::parse_str` + `wgpu::naga::valid::Validator`, and failed reloads or pipeline rebuilds retain the prior `ShaderModule` + live pipeline. `SceneColor` format equals the swapchain sRGB format in M25; M27 will add an HDR sibling. MSAA sample-count changes require relaunch. Cites `D-053`."
  - `docs/DECISION_INDEX.md`: one-line row.
  - `AGENTS.md` §Asset Rules: rewrite the shaders bullet — manifest-tracked, recursive `assets/` watch picks up `.wgsl`, body edits only, signature change still needs rebuild.
  - `DESIGN.md` §Hot Reload matrix: add `shader` row. §Status: note offscreen `SceneTarget` and pass list are live.
  - `CHANGELOG.md` Unreleased: "M25: render targets, ordered pass list, WGSL hot reload, opt-in GPU depth-test sprite path."
- Verify: `rg -n "D-0NN" DECISIONS.md docs/DECISION_INDEX.md` returns the two decision-doc hits (replace `D-0NN` with the resolved number); then spot-check `AGENTS.md`, `DESIGN.md`, and `CHANGELOG.md` for the policy/status sync. `cargo test --workspace` still green.

### 16. Flip plan status
- File: `docs/plans/phase4-milestone-25-render-foundation.md`.
- Action: `status: draft` → `status: done` once steps 1–15 land on `0.22`.
- Verify: `rg -n "^status:" docs/plans/phase4-milestone-25-render-foundation.md` shows `done`.

## Verification Commands Summary

```bash
cargo fmt --all
cargo build --workspace
cargo test --workspace                          # Layer 1 + unit tests
cargo clippy --workspace --all-targets          # advisory
./scripts/smoke-examples.sh                     # Layer 2, includes msaa × depth_sort matrix

# Image-diff baseline capture (one-time, pre-step-9):
TUNGSTEN_CAPTURE_FRAME=60 TUNGSTEN_CAPTURE_PATH=docs/showcase/m25-sprite-stress-baseline.png \
    cargo run -p example-02-sprite-stress

# Image-diff assertion (post-step-9, post-step-14):
TUNGSTEN_CAPTURE_FRAME=60 TUNGSTEN_CAPTURE_PATH=/tmp/m25-default.png \
    cargo run -p example-02-sprite-stress
#  → diff against docs/showcase/m25-sprite-stress-baseline.png via image_diff

# GPU-depth parity capture on a component-driven scene:
TUNGSTEN_CAPTURE_FRAME=60 TUNGSTEN_CAPTURE_PATH=/tmp/m25-depth-cpu.png \
TUNGSTEN_RENDER_DEPTH_SORT=cpu_stable cargo run -p example-03_scene_state
TUNGSTEN_CAPTURE_FRAME=60 TUNGSTEN_CAPTURE_PATH=/tmp/m25-depth-gpu.png \
TUNGSTEN_RENDER_DEPTH_SORT=gpu_depth cargo run -p example-03_scene_state
#  → diff /tmp/m25-depth-cpu.png vs /tmp/m25-depth-gpu.png via image_diff

# Hot-reload smoke (in another terminal while any example runs):
sed -i 's/color\.rgb/(vec3<f32>(1.0) - color.rgb)/' assets/shaders/sprite.wgsl
#  → live visual inversion, log line "shader 'sprite' reloaded"
git restore assets/shaders/sprite.wgsl
```

## Sources

- phase4.md §M25 (canonical scope)
- [renderer.rs:200-603](crates/tungsten-render/src/renderer.rs#L200-L603)
- [manifest.rs:36-329](crates/tungsten-core/src/assets/manifest.rs#L36-L329)
- [hot_reload.rs:24-145](crates/tungsten/src/hot_reload.rs#L24-L145)
- [app.rs:290-467](crates/tungsten/src/app.rs#L290-L467), [app.rs:760-819](crates/tungsten/src/app.rs#L760-L819)
- [01_platformer/src/setup.rs:56-62](examples/01_platformer/src/setup.rs#L56-L62) (hot-reload call shape)
- [sprite_extract.rs:1-62](crates/tungsten/src/sprite_extract.rs#L1-L62), [sprite_extract.rs tests:1-152](crates/tungsten/src/tests/sprite_extract.rs#L1-L152)
- `DECISIONS.md` `D-016`, `D-017`, `D-023`, `D-053` (grep, do not open serially)
