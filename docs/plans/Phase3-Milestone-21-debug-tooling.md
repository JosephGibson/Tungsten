---
status: draft
milestone: M21
branch: 0.18
version-target: 0.18.0
depends-on: M14 complete, M15 complete, M18 complete, M19 complete
unblocks: ship/debug quality — collider visualisation, per-system timing review, deterministic visual regression checks
---

# Phase 3 Milestone 21 — Debug Tooling

## Goal

Ship the M21 deliverables verbatim from `docs/plans/Phase3.md`:

- `DebugDraw` primitive with `draw_aabb`, `draw_circle`, `draw_line`; cleared each frame.
- Physics debug overlay toggled by the `engine_toggle_physics_debug` action (default `F1`).
- Per-system timing overlay with rolling average, toggled by `engine_toggle_systems_overlay` (default `F2`).
- Text-only entity inspector with opt-in `Inspectable` trait, toggled by `engine_toggle_inspector` (default `F3`).
- Screenshot capture plus baseline image-diff helper driving a deterministic visual regression check for at least one scene.

Plus (scope-in for M21 after plan review):

- Reuse `QuadPipeline` for axis-aligned AABB outlines (no new pipeline for that path); a minimal `DebugLinePipeline` handles arbitrary-angle lines and circle polylines and shares `QuadPipeline`'s camera bind group layout so only one camera uniform lives on the GPU.
- RenderDoc / GPU-marker hints: encoder- and render-pass debug groups (`push_debug_group` / `pop_debug_group`) plus consistent `label:` fields on pipelines, buffers, and textures so a RenderDoc capture names every stage of the frame without engineers guessing.

## Non-goals

- Beyond `DebugLinePipeline` (oriented-line quads for lines + circle polylines) and `QuadPipeline` reuse (axis-aligned AABB edges), no new render pipeline. No SDF, no MSAA change, no depth buffer.
- Graphical / property-editing inspector UI; `egui` / `imgui` / `hud-rs` stay out.
- Persisting overlay toggle state, inspector selection, or baseline image paths to `tungsten.json`.
- Full UI layout engine; overlays reuse the existing `glyphon` text pipeline plus flat quad/line primitives only.
- Interactive colour / thickness editing of `DebugDraw` commands at runtime.
- Continuous visual diff across every example or every frame; M21 ships one fixed-frame fixture under `example-02-sprite-stress` (`02_sprite_stress`).
- Replacing `DebugHud` (`M18`). The systems-timing overlay (`F2`) is a separate resource so its state is independent of the HUD toggle (`F4`).
- Automated RenderDoc capture, adapter feature gates beyond the existing `TIMESTAMP_QUERY` path (`M12`), or `wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES`. GPU debug-group markers are in scope; programmatic capture is not.
- Cross-platform PNG save semantics beyond what `image::save_buffer` already provides; no HDR, no tone-mapping.
- Windows/macOS screenshot smoke-test coverage; the visual-regression check runs on Linux only and stays opt-in behind `TUNGSTEN_CAPTURE_FRAME`. Other platforms rely on `cargo test --workspace`.
- Scoping the `Inspectable` registration API so it can diff component values over time; M21 renders current snapshot only.
- Any change to `D-007` / `D-016` / `D-018`; `DebugDraw` stays in `tungsten-core` as pure POD and crosses the seam the same way `QuadInstance` does.

## Guardrails

- `D-007` / `D-016`: `tungsten-core` stays free of `wgpu` / `winit` types. `DebugDraw` is pure POD; the new `DebugLinePipeline` lives in `tungsten-render` and reuses `QuadPipeline`'s camera bind group layout.
- `D-018`: extract runs on the main thread with `&World`; `DebugDraw` commands are drained into `QuadInstance` (AABB edges) plus `DebugLineInstance` (lines, circle polylines) POD before the render stage.
- `D-038`: `FrameTimings` remains the single CPU-timing resource; the `F2` overlay reads it and keeps its own EWMA scratch map — no parallel timer.
- `D-039`: overlays emit `DebugDraw` commands during the systems stage; the extract stage reads and clears them. No mutation of `DebugDraw` from extract/render.
- `D-042`: physics overlay uses `Position` + `Collider` (not `Transform`) so the draw reflects authoritative collision geometry, not render state.
- `D-044`: the `F2` system-timing overlay is a distinct resource; it does NOT toggle with `DebugHud.enabled`. `HudRow` entries are only used for custom HUD rows, not the M21 overlays.
- `D-045`: every new engine toggle routes through `ActionMap`. Hardcoded `KeyCode` branches in user-facing paths are prohibited.
- Frame order invariant unchanged: `systems -> flush commands -> flush events -> hot-reload -> extract -> render`. `DebugDraw` is populated during `systems`, drained during `extract`, and cleared before the next frame.
- Camera uniform reuse: only one `view_proj` buffer ships in `tungsten-render`. `DebugLinePipeline` binds the same `camera_bind_group` instance owned by `QuadPipeline`; it does not allocate a second uniform. `QuadPipeline::new` now returns an object whose camera bind group layout (and bound group) can be borrowed by sibling pipelines.
- When every overlay is disabled, per-frame cost collapses to four `ActionMap::just_pressed` checks and one `DebugDraw::clear` on an empty vector. The extra `QuadPipeline::draw` call (debug AABB channel) short-circuits on empty-slice and draws zero instances.
- GPU debug groups are always-on and cheap. Every render pass opens a named group around each stage (`quads`, `sprites`, `debug_quads`, `debug_lines`, `text`); the command encoder wraps the frame with a `frame` group. Pipelines, buffers, textures, and bind groups carry explicit `label:` values so RenderDoc captures are self-describing.
- Screenshot capture must be off by default; enabling it requires `TUNGSTEN_CAPTURE_FRAME=<n>` or an explicit API call from a test. No capture path runs in release-mode examples unless requested.
- No new workspace dependency. `image = { workspace = true }` already exists (used by `asset_loader`); `tungsten-render` gains `image = { workspace = true }` with no version bump.
- Do not read `docs/plans/archive/`.

## Read before coding

1. `AGENTS.md`
2. `docs/LLM_INDEX.md`
3. `docs/plans/Phase3.md` §`M21 - Debug Tooling`
4. `crates/tungsten/src/app.rs` (frame order, engine-system registration, `FrameTimings` population, text extract slot, renderer call sites)
5. `crates/tungsten/src/debug_hud.rs` (`DebugHud`, `HudActiveState`, compose pattern, `HudRowProvider`)
6. `crates/tungsten/src/telemetry.rs` (`FrameTimings`, `RenderCounts`)
7. `crates/tungsten/src/display.rs` (`engine_display_input_system` as the template for engine-owned action consumers)
8. `crates/tungsten-core/src/input.rs` (`KeyCode` enum — add `F1`, `F2`, `F3`)
9. `crates/tungsten-core/src/input/key_serde.rs` (`KEYCODE_NAMES` round-trip table)
10. `crates/tungsten-core/src/input/action_map.rs` (`default_map`, merging with `merged_with_defaults`)
11. `crates/tungsten/src/input_bridge.rs` (`translate_key` — add `F1` / `F2` / `F3` arms)
12. `crates/tungsten-core/src/components.rs` (`Transform`, `Tag`)
13. `crates/tungsten-core/src/physics/components.rs` (`Position`, `Velocity`, `Collider`, `Shape::{Aabb, Circle}`)
14. `crates/tungsten-render/src/lib.rs`, `renderer.rs`, `quad.rs` (`Renderer::render_frame_full`, `QuadInstance`, existing `camera_bind_group_layout`/`camera_bind_group` fields to expose for sharing, pipeline pattern for a new `DebugLinePipeline`)
15. `crates/tungsten-render/src/text.rs` (`TextSection` shape, unknown-font silent fallback)
16. `crates/tungsten-core/src/ecs/world.rs` (`query2`, `query3`, `entity_count`)
17. `crates/tungsten/src/asset_loader.rs` (only for the `image::` import pattern — screenshot encode)
18. `input.json`
19. `examples/01_platformer/src/{main,setup,systems,extract,state}.rs`
20. `examples/02_sprite_stress/src/main.rs` (target scene for the visual regression fixture)
21. `docs/DECISION_INDEX.md` (new `D-047` one-liner; grep `D-007|D-016|D-018|D-038|D-039|D-042|D-044|D-045`)
22. `scripts/smoke-examples.sh`, `scripts/perf-capture.sh`

## Files to touch

| File | Change |
| --- | --- |
| `crates/tungsten-core/src/input.rs` | add `KeyCode::F1`, `KeyCode::F2`, `KeyCode::F3` |
| `crates/tungsten-core/src/input/key_serde.rs` | add round-trip entries for `F1` / `F2` / `F3` |
| `crates/tungsten-core/src/input/action_map.rs` | add defaults for `engine_toggle_physics_debug` (`F1`), `engine_toggle_systems_overlay` (`F2`), `engine_toggle_inspector` (`F3`); extend `default_map_registers_engine_actions` test |
| `crates/tungsten/src/input_bridge.rs` | add `WinitKeyCode::F1/F2/F3 -> KeyCode::F1/F2/F3` |
| `crates/tungsten-core/src/debug_draw.rs` | **new** — `DebugDraw` resource, `DebugShape` enum (`Aabb { min, max }`, `Circle { center, radius }`, `Line { a, b }`), `DebugCommand { shape, color, thickness }`, `DebugDraw::{new, clear, draw_aabb, draw_circle, draw_line, drain}`; unit tests |
| `crates/tungsten-core/src/inspect.rs` | **new** — `Inspectable` trait with `fn inspect_rows(&self) -> Vec<(&'static str, String)>`, blanket impls for `Tag`, `Transform`, `Visibility`, `Position`, `Velocity`, `Sprite`; unit tests |
| `crates/tungsten-core/src/lib.rs` | `pub mod debug_draw;` + `pub use debug_draw::{DebugDraw, DebugShape, DebugCommand};` and `pub mod inspect; pub use inspect::Inspectable;` |
| `crates/tungsten-render/Cargo.toml` | add `image = { workspace = true }` (workspace dep already exists) |
| `crates/tungsten-render/src/quad.rs` | expose `pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout` and `pub fn camera_bind_group(&self) -> &wgpu::BindGroup` so `DebugLinePipeline` can reuse the same uniform; add explicit `label:` strings on the pipeline, vertex buffer, and bind group (already labelled today — verify parity). No change to `QuadInstance` layout. |
| `crates/tungsten-render/src/debug_line.rs` | **new** — `DebugLineInstance { a, b, thickness, _pad, color }` POD + `DebugLinePipeline` drawing oriented quads for lines and circle polylines (NOT axis-aligned AABBs — those reuse `QuadPipeline`); constructor takes `&wgpu::BindGroupLayout` borrowed from `QuadPipeline::camera_bind_group_layout()`; `debug_line.wgsl` embedded via `include_str!` |
| `crates/tungsten-render/src/debug_line.wgsl` | **new** — vertex shader that expands `(a, b, thickness)` into an oriented quad in world space using `view_proj` from the shared camera uniform |
| `crates/tungsten-render/src/lib.rs` | `pub mod debug_line;` + `pub use debug_line::{DebugLineInstance, DebugLinePipeline};` |
| `crates/tungsten-render/src/renderer.rs` | own a `debug_line_pipeline: DebugLinePipeline` built from `QuadPipeline::camera_bind_group_layout()`; extend `render_frame_full` + `render_frame_full_timed` signatures to take `debug_quads: &[QuadInstance]` and `debug_lines: &[DebugLineInstance]`; inside the existing `main_pass` draw order is `quads -> sprites -> debug_quads (via QuadPipeline) -> debug_lines -> text`; wrap the command encoder with `push_debug_group("frame")` / `pop_debug_group`, and wrap each stage inside the render pass with its own named group |
| `crates/tungsten-render/src/screenshot.rs` | **new** — `Renderer::capture_frame(path: &Path) -> Result<(), ScreenshotError>`; copy-to-buffer path; `image::save_buffer` encoding; `ScreenshotError` via `thiserror` |
| `crates/tungsten-render/src/image_diff.rs` | **new** — `pub fn compare_png(lhs: &Path, rhs: &Path, tolerance: u8) -> Result<DiffReport, ImageDiffError>`; `DiffReport { max_delta, mean_delta, pixels_above_tolerance, width, height }`; `ImageDiffError::{Io, Decode, DimensionMismatch}`; unit tests round-trip fabricated RGBA `ImageBuffer`s through `std::env::temp_dir()` (no new dep, explicit cleanup) |
| `crates/tungsten/src/physics_debug.rs` | **new** — `PhysicsDebugOverlay { enabled, color_aabb, color_circle, thickness }` resource, `physics_debug_emit_system`, `physics_debug_toggle_system` |
| `crates/tungsten/src/systems_overlay.rs` | **new** — `SystemTimingOverlay { enabled, ewma: BTreeMap<String, f32>, alpha, refresh_interval_ms, cached_section, time_since_refresh_ms }`, `systems_overlay_toggle_system`, `compose_systems_overlay_text_section` (`pub(crate)`) |
| `crates/tungsten/src/inspector.rs` | **new** — `InspectorState { enabled, selected: Option<Entity>, registered: Vec<InspectFn> }`, `InspectFn = Box<dyn Fn(&World, Entity) -> Vec<(&'static str, String)>>`, `inspector_toggle_system`, `inspector_pick_system` (LMB-to-pick while enabled), `compose_inspector_text_section` |
| `crates/tungsten/src/app.rs` | insert new resources (`DebugDraw`, `PhysicsDebugOverlay`, `SystemTimingOverlay`, `InspectorState`) in `App::new`; register toggle systems at the head of the engine chain and `physics_debug_emit_system` at the start of the extract stage (before the `DebugDraw` drain); wire the extract stage to drain `DebugDraw` into `debug_quads: Vec<QuadInstance>` (AABB edges) and `debug_lines: Vec<DebugLineInstance>` (lines, circle polylines) and extend overlay text; pass both channels through to `renderer.render_frame_full[_timed]`; register default `Inspectable` bindings for `Tag`, `Transform`, `Visibility`, `Position`, `Velocity`, `Sprite`; add `App::register_inspectable::<T: 'static + Inspectable>(label: &'static str)`; honour `TUNGSTEN_CAPTURE_FRAME` / `TUNGSTEN_CAPTURE_PATH` by counting rendered frames and calling `renderer.capture_frame` on the target frame |
| `crates/tungsten/src/lib.rs` | re-exports: `DebugDraw`, `DebugShape`, `PhysicsDebugOverlay`, `SystemTimingOverlay`, `InspectorState`, `Inspectable` |
| `examples/02_sprite_stress/src/main.rs` | no code change required — `TUNGSTEN_CAPTURE_FRAME` / `TUNGSTEN_CAPTURE_PATH` are honoured generically by `App` (step 8); overlays-on perf capture is driven via `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` parsed by the example's setup to flip `.enabled = true` before `App::run` |
| `examples/02_sprite_stress/tests/fixtures/baseline-sprite-stress.png` | **new** — checked-in reference image captured on the reference machine |
| `examples/02_sprite_stress/tests/visual_regression.rs` | **new** — cargo test gated on `TUNGSTEN_VISUAL_REGRESSION=1` (early-return when unset) that shells out to the example with `TUNGSTEN_SMOKE_FRAMES=8 TUNGSTEN_CAPTURE_FRAME=5 TUNGSTEN_CAPTURE_RESOLUTION=1280x720`, then compares the produced PNG to the baseline via `image_diff::compare_png` (tolerance=2, zero pixels above) |
| `examples/01_platformer/src/main.rs` | extend header `Controls:` block with `F1`, `F2`, `F3` action rows |
| `examples/01_platformer/src/setup.rs` | no structural change; `Tag::new("player")` is already present |
| `input.json` | add `engine_toggle_physics_debug`, `engine_toggle_systems_overlay`, `engine_toggle_inspector` entries bound to `F1` / `F2` / `F3` |
| `scripts/smoke-examples.sh` | no change required; new toggles stay off by default |
| `docs/LLM_INDEX.md` | add subsystem row `Debug tooling (M21)` → debug_draw / physics_debug / systems_overlay / inspector / screenshot / image_diff files; task row `Fix a debug overlay or screenshot check` → same files + `docs/DECISION_INDEX.md` for `D-047` |
| `docs/DECISION_INDEX.md` | add `D-047` one-liner under `ECS / Runtime Flow` |
| `DECISIONS.md` | add full `D-047` entry covering (a) `DebugDraw` lives in core as POD and crosses the seam via `QuadInstance` (AABB edges) + `DebugLineInstance` (lines, circle polylines) — no second AABB pipeline, `DebugLinePipeline` shares `QuadPipeline`'s camera bind group layout; (b) overlays are independent action-toggled resources, not `DebugHud` rows; (c) screenshot path renders the capture frame into an offscreen `RENDER_ATTACHMENT \| COPY_SRC` texture, reads back via row-padded `MAP_READ` buffer, encodes with `image::save_buffer` — swapchain is never `COPY_SRC`; (d) image-diff is per-pixel RGBA with a tolerance threshold, no perceptual metric; (e) GPU debug groups + explicit wgpu labels are always-on (no feature gate) so RenderDoc captures self-document frame structure |
| `docs/plans/Phase3.md` | flip `M21` status to `in progress` on start; to `complete` on land with `v0.18.0` + date; link this plan once archived |
| `perf-runs/M21-debug-tooling/README.md` | **new** — sprite-stress 300-frame capture pair: all overlays off (baseline) vs. all overlays on; record `total_ms` mean / p95 / p99, flag any regression > 5 % |

## Public surface after M21

- `tungsten_core::{DebugDraw, DebugShape, DebugCommand, Inspectable}`
- `tungsten_core::input::KeyCode::{F1, F2, F3}`
- `tungsten_render::{DebugLineInstance, DebugLinePipeline, ScreenshotError, image_diff::{compare_png, DiffReport}}`
- `tungsten_render::QuadPipeline::camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout`
- `tungsten_render::QuadPipeline::camera_bind_group(&self) -> &wgpu::BindGroup`
- `tungsten_render::Renderer::capture_frame(&mut self, path: &Path)`
- `tungsten::{PhysicsDebugOverlay, SystemTimingOverlay, InspectorState}`
- `tungsten::App::register_inspectable::<T>(label: &'static str)`
- Action map defaults: `engine_toggle_physics_debug`, `engine_toggle_systems_overlay`, `engine_toggle_inspector`

Keep private:

- `physics_debug_toggle_system`, `physics_debug_emit_system`, `systems_overlay_toggle_system`, `inspector_toggle_system`, `inspector_pick_system` — registered as engine systems in `App::new` via `add_engine_system`.
- `compose_systems_overlay_text_section`, `compose_inspector_text_section` — `pub(crate)`; called only from `app.rs` using the `remove_resource`/`insert_resource` borrow dance.
- `DebugLinePipeline` internals (buffers, bind groups, wgsl module).

## Ordered steps

### 1. Core primitives: `KeyCode` + `DebugDraw` + `Inspectable`

- `crates/tungsten-core/src/input.rs`: add `F1`, `F2`, `F3` to `KeyCode` adjacent to `F4` / `F9` / `F11`. Preserve alphabetical / numeric grouping with existing `F*` entries.
- `crates/tungsten-core/src/input/key_serde.rs`: add `(KeyCode::F1, "F1")`, `(KeyCode::F2, "F2")`, `(KeyCode::F3, "F3")` to `KEYCODE_NAMES`. Existing `keycode_names_round_trip` test covers the new rows.
- `crates/tungsten-core/src/debug_draw.rs` — new module:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum DebugShape {
      Aabb { min: Vec2, max: Vec2 },
      Circle { center: Vec2, radius: f32, segments: u16 },
      Line { a: Vec2, b: Vec2 },
  }

  #[derive(Debug, Clone, Copy)]
  pub struct DebugCommand {
      pub shape: DebugShape,
      pub color: [f32; 4],
      pub thickness: f32,
  }

  #[derive(Debug, Default)]
  pub struct DebugDraw { cmds: Vec<DebugCommand> }

  impl DebugDraw {
      pub fn new() -> Self { Self::default() }
      pub fn draw_aabb(&mut self, min: Vec2, max: Vec2, color: [f32; 4], thickness: f32) { /* push */ }
      pub fn draw_circle(&mut self, center: Vec2, radius: f32, color: [f32; 4], thickness: f32) { /* push with default segments=24 */ }
      pub fn draw_line(&mut self, a: Vec2, b: Vec2, color: [f32; 4], thickness: f32) { /* push */ }
      pub fn clear(&mut self) { self.cmds.clear(); }
      pub fn drain(&mut self) -> std::vec::Drain<'_, DebugCommand> { self.cmds.drain(..) }
      pub fn is_empty(&self) -> bool { self.cmds.is_empty() }
      pub fn len(&self) -> usize { self.cmds.len() }
  }
  ```
  Unit tests: `draw_*` pushes one command each; `clear` empties; `drain` empties and returns iterator.
- `crates/tungsten-core/src/inspect.rs`:
  ```rust
  pub trait Inspectable {
      fn inspect_rows(&self) -> Vec<(&'static str, String)>;
  }
  ```
  Blanket impls for `Tag`, `Transform`, `Visibility`, `Position`, `Velocity`, `Sprite` (returns one-to-three labelled rows each, e.g. `Transform` → `[("pos", "(x, y)"), ("rot", "...")]`). Unit tests verify each impl's row count and label stability.
- `crates/tungsten-core/src/lib.rs`: add `pub mod debug_draw;`, `pub mod inspect;` and re-exports.
- Exit: `cargo test -p tungsten-core` passes; no change to existing serialization round-trips.

### 2. Input bridge + action-map defaults

- `crates/tungsten/src/input_bridge.rs`: add three arms mapping `WinitKeyCode::F1/F2/F3` to `KeyCode::F1/F2/F3`.
- `crates/tungsten-core/src/input/action_map.rs::default_map`: insert bindings
  ```rust
  actions.insert("engine_toggle_physics_debug".into(),    vec![Binding::Key { code: KeyCode::F1 }]);
  actions.insert("engine_toggle_systems_overlay".into(),  vec![Binding::Key { code: KeyCode::F2 }]);
  actions.insert("engine_toggle_inspector".into(),        vec![Binding::Key { code: KeyCode::F3 }]);
  ```
  Update the comment at the top of `default_map` from `F4/F9/F11/Escape` to `F1/F2/F3/F4/F9/F11/Escape`. Extend the existing engine-action assertion test to cover the three new actions.
- `input.json`: append the three new entries in the same style as `engine_toggle_hud`; preserve JSON formatting.
- Exit: `cargo test -p tungsten-core` + `cargo build -p tungsten` clean; `ActionMap::merged_with_defaults` round-trips new actions.

### 3. Render seam: `QuadPipeline` reuse + `DebugLinePipeline` + GPU debug groups

- `crates/tungsten-render/Cargo.toml`: add `image = { workspace = true }`.
- `crates/tungsten-render/src/quad.rs`: promote `camera_bind_group_layout` and `camera_bind_group` to stored fields (they already exist as locals in `new`), add public accessors:
  ```rust
  pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout { &self.camera_bind_group_layout }
  pub fn camera_bind_group(&self) -> &wgpu::BindGroup { &self.camera_bind_group }
  ```
  Verify every `create_*` call site in `QuadPipeline::new` still carries an explicit `label: Some(..)` — the current file already labels the shader, buffer, layout, bind group, pipeline layout, pipeline, vertex buffer; no functional change, just a guardrail so RenderDoc groups stay named.
- `crates/tungsten-render/src/debug_line.rs`:
  ```rust
  #[repr(C)]
  #[derive(Debug, Clone, Copy, Pod, Zeroable)]
  pub struct DebugLineInstance {
      pub a: [f32; 2],
      pub b: [f32; 2],
      pub thickness: f32,
      pub _pad: f32,
      pub color: [f32; 4],
  }

  pub struct DebugLinePipeline {
      pipeline: wgpu::RenderPipeline,
      vertex_buffer: wgpu::Buffer, // unit-quad corners, shared shape, labelled "debug_line_unit_quad"
  }

  impl DebugLinePipeline {
      pub fn new(
          device: &wgpu::Device,
          surface_format: wgpu::TextureFormat,
          camera_bind_group_layout: &wgpu::BindGroupLayout,
      ) -> Self { ... }

      pub fn draw(
          &self,
          device: &wgpu::Device,
          render_pass: &mut wgpu::RenderPass<'_>,
          camera_bind_group: &wgpu::BindGroup,
          instances: &[DebugLineInstance],
      ) { /* empty slice -> early return */ }
  }
  ```
  The pipeline uses the caller-supplied `camera_bind_group_layout` (borrowed from `QuadPipeline`); it does NOT allocate its own camera uniform. `draw` accepts `camera_bind_group` so `Renderer` can pass `QuadPipeline::camera_bind_group()` at call time. Pipeline renders one oriented quad per instance; the vertex shader transforms `a` and `b` via the shared `view_proj`, offsets each endpoint perpendicular to the line tangent by `thickness / 2` in screen space (clip-space offset converted to NDC via viewport dims from a push constant — or, if push constants add complexity, via a tiny `ViewportSize` uniform packaged with the camera in a second binding; prefer the first if `wgpu::Features::PUSH_CONSTANTS` is already enabled, otherwise ship viewport via uniform). `ALPHA_BLENDING`, no depth.
- `crates/tungsten-render/src/debug_line.wgsl`: minimal vertex + fragment pair; follow `quad.wgsl`'s shape; use `@group(0) @binding(0)` for the camera matrix to match the `QuadPipeline` layout exactly.
- `crates/tungsten-render/src/renderer.rs`:
  - Store `debug_line_pipeline: DebugLinePipeline`, constructed with `quad_pipeline.camera_bind_group_layout()`.
  - Change `render_frame_full` / `render_frame_full_timed` signatures:
    ```rust
    pub fn render_frame_full(
        &mut self,
        view_proj: &glam::Mat4,
        quads: &[QuadInstance],
        sprite_batches: &[SpriteBatch],
        debug_quads: &[QuadInstance],
        debug_lines: &[DebugLineInstance],
        text_sections: &[TextSection],
    ) -> Result<(), wgpu::SurfaceError>;
    ```
  - Draw order inside the main render pass: `quads -> sprites -> debug_quads -> debug_lines -> text`. Debug AABB edges are drawn by calling `self.quad_pipeline.draw(&self.device, &mut render_pass, debug_quads)` — the same pipeline runs twice, which is cheaper than maintaining a second AABB-only pipeline.
  - GPU debug groups:
    ```rust
    encoder.push_debug_group("tungsten_frame");
    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("tungsten_main_pass"),
            /* ... */
        });
        render_pass.push_debug_group("quads"); quad_pipeline.draw(...); render_pass.pop_debug_group();
        render_pass.push_debug_group("sprites"); sprite_pipeline.draw(...); render_pass.pop_debug_group();
        render_pass.push_debug_group("debug_quads"); quad_pipeline.draw(...debug_quads); render_pass.pop_debug_group();
        render_pass.push_debug_group("debug_lines"); debug_line_pipeline.draw(...); render_pass.pop_debug_group();
        render_pass.push_debug_group("text"); text_pipeline.render(...); render_pass.pop_debug_group();
    }
    encoder.pop_debug_group();
    ```
    `push_debug_group` / `pop_debug_group` calls are near-free on non-RenderDoc backends; no feature gate required.
- `crates/tungsten-render/src/lib.rs`: `pub mod debug_line;` + re-exports.
- Unit tests: `DebugLineInstance` size/alignment checked via `std::mem::size_of`; runtime draw path is exercised via `./scripts/smoke-examples.sh`.
- Exit: `cargo build -p tungsten-render` clean; `cargo test -p tungsten-render` passes.

### 4. Screenshot capture + image-diff

- `crates/tungsten-render/src/screenshot.rs`:
  ```rust
  impl Renderer {
      pub fn capture_frame(&mut self, path: &Path) -> Result<(), ScreenshotError> { ... }
  }
  ```
  - Approach: render every frame through an offscreen `wgpu::Texture` only when `capture_frame` is armed; the normal path still targets the swapchain directly (zero cost when capture is not requested). `capture_frame(path)` sets `pending_capture = Some(path)` on the renderer; on the next `render_frame_full`, the renderer additionally begins a second render pass against an offscreen `Texture` (format = surface format, usage = `RENDER_ATTACHMENT | COPY_SRC`, label = `"tungsten_screenshot_target"`) using the same draw calls, then issues `copy_texture_to_buffer` into a row-padded `Buffer` (label `"tungsten_screenshot_readback"`, usage = `COPY_DST | MAP_READ`), submits, blocks on `device.poll(Wait)`, maps the buffer, strips the `bytes_per_row` padding, encodes via `image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8)`, and clears `pending_capture`.
  - Blocking map is explicitly documented as dev-tool only; not safe for production frame-critical paths.
  - `ScreenshotError`: `thiserror` enum covering `Io`, `Encode` (pass-through `image::ImageError`), `DeviceLost`, `Surface`, `MapFailed`.
  - All new wgpu resources carry `label: Some(..)` so RenderDoc captures of the screenshot frame show the readback pipeline as a named stage.
- `crates/tungsten-render/src/image_diff.rs`:
  ```rust
  pub struct DiffReport {
      pub width: u32,
      pub height: u32,
      pub max_delta: u8,
      pub mean_delta: f32,
      pub pixels_above_tolerance: u32,
  }
  pub fn compare_png(lhs: &Path, rhs: &Path, tolerance: u8) -> Result<DiffReport, ImageDiffError>;
  ```
  - Decode both PNGs via `image::open`; reject mismatched dimensions with `ImageDiffError::DimensionMismatch`.
  - Walk RGBA byte arrays; per-pixel delta is `max(|r1-r2|, |g1-g2|, |b1-b2|, |a1-a2|)`; accumulate mean and max; count pixels above `tolerance`.
  - Unit tests build in-memory RGBA `ImageBuffer`s and round-trip them through `std::env::temp_dir()` + unique-name paths (no new dep; explicit cleanup on drop). Assert: identical images yield `max_delta == 0, pixels_above_tolerance == 0`; a single flipped red channel yields `max_delta == 255, pixels_above_tolerance == 1`; mismatched dimensions return `ImageDiffError::DimensionMismatch`.
- Exit: `cargo test -p tungsten-render` passes; capture path is linked but unexercised without the env-var wiring from step 8.

### 5. Physics debug overlay (`F1`)

- `crates/tungsten/src/physics_debug.rs`:
  ```rust
  pub struct PhysicsDebugOverlay {
      pub enabled: bool,
      pub color_aabb: [f32; 4],
      pub color_circle: [f32; 4],
      pub thickness: f32,
  }
  impl Default for PhysicsDebugOverlay {
      fn default() -> Self { Self { enabled: false, color_aabb: [0.0, 1.0, 0.0, 0.9], color_circle: [0.0, 0.8, 1.0, 0.9], thickness: 1.5 } }
  }

  pub(crate) fn physics_debug_toggle_system(world: &mut World) { /* action_map.just_pressed("engine_toggle_physics_debug") -> toggle */ }
  pub(crate) fn physics_debug_emit_system(world: &mut World) {
      // if !enabled: return early
      // for (entity, pos, collider) in world.query2::<Position, Collider>():
      //   match collider.shape {
      //     Shape::Aabb { half_extents } => draw_aabb(pos.0 + offset - half, pos.0 + offset + half, color_aabb, thickness),
      //     Shape::Circle { radius } => draw_circle(pos.0 + offset, radius, color_circle, thickness),
      //   }
  }
  ```
  Unit tests with a `World` containing two bodies assert `DebugDraw::len()` equals the colliders count when enabled, zero when disabled.
- Exit: `cargo test -p tungsten` passes.

### 6. System timing overlay (`F2`)

- `crates/tungsten/src/systems_overlay.rs`:
  ```rust
  pub struct SystemTimingOverlay {
      pub enabled: bool,
      pub alpha: f32,               // EWMA alpha; default 0.1
      pub refresh_interval_ms: f32, // default 250.0
      pub position: [f32; 2],       // screen-space; default [12.0, 12.0]
      pub font_id: String,          // default "mono"
      pub font_size: f32,           // default 18.0
      pub line_height: f32,         // default 22.0
      pub color: [u8; 4],
      pub outline_color: [u8; 4],
      pub outline_px: f32,
      ewma: BTreeMap<String, f32>,  // name -> smoothed ms
      cached_section: Option<TextSection>,
      time_since_refresh_ms: f32,
  }
  ```
  Behaviour:
  - Each frame while enabled, iterate `FrameTimings::system_timings` and update `ewma` entries by name; drop stale entries whose names no longer appear this frame.
  - Compose a single `TextSection` listing every system as `"{name:>30}  {ms:>6.2}ms"` sorted descending by smoothed ms; refresh throttling mirrors the `DebugHud` compose pattern.
  - `pub(crate) fn compose_systems_overlay_text_section(overlay: &mut SystemTimingOverlay, world: &World, viewport: (u32, u32), frame_ms: f32) -> Vec<TextSection>` — returns `Vec::new()` when disabled.
- Unit tests: feed 200 frames of constant timings for two systems, assert EWMA within tolerance; `enabled = false` returns empty section; stale system name is dropped after one frame.
- Exit: `cargo test -p tungsten` passes.

### 7. Entity inspector (`F3`)

- `crates/tungsten/src/inspector.rs`:
  ```rust
  pub type InspectFn = Box<dyn Fn(&World, Entity) -> Vec<(&'static str, String)> + 'static>;

  pub struct InspectorState {
      pub enabled: bool,
      pub selected: Option<Entity>,
      registered: Vec<(&'static str, InspectFn)>, // (component_type_name, fn)
      pub position: [f32; 2],
      pub font_id: String,
      pub font_size: f32,
      pub line_height: f32,
      pub color: [u8; 4],
      pub outline_color: [u8; 4],
      pub outline_px: f32,
  }

  impl InspectorState {
      pub fn register<T: 'static + Inspectable>(&mut self, label: &'static str) {
          self.registered.push((label, Box::new(|world: &World, e: Entity| {
              world.get::<T>(e).map(|c| c.inspect_rows()).unwrap_or_default()
          })));
      }
  }

  pub(crate) fn inspector_toggle_system(world: &mut World) { /* action_map.just_pressed("engine_toggle_inspector") */ }
  pub(crate) fn inspector_pick_system(world: &mut World) {
      // when enabled && InputState::mouse_just_pressed(Left):
      //   resolve cursor in world space via CameraState + WindowSize
      //   walk (entity, Transform) entities, choose closest by squared distance
      //   update InspectorState.selected
  }
  pub(crate) fn compose_inspector_text_section(state: &mut InspectorState, world: &World, viewport: (u32, u32)) -> Vec<TextSection>;
  ```
  - `App::register_inspectable::<T: 'static + Inspectable>(label: &'static str)` delegates to `InspectorState::register` on the stored resource.
  - `App::new` auto-registers `Tag`, `Transform`, `Visibility`, `Position`, `Velocity`, `Sprite` via their `Inspectable` impls.
  - When `selected` is `None`, the overlay renders `"inspector: no selection — LMB to pick"`.
  - When `selected` is a stale entity (despawned), clear `selected` next frame and render the no-selection message.
- Unit tests: register a component, spawn an entity, call `inspector_pick_system` with a synthetic `InputState`, assert `selected == Some(e)`; call compose and assert the rendered string includes the component's labelled rows.
- Exit: `cargo test -p tungsten` passes.

### 8. `App` wiring

- `crates/tungsten/src/app.rs`, in `App::new` after existing `world.insert_resource(...)` calls:
  ```rust
  world.insert_resource(DebugDraw::new());
  world.insert_resource(PhysicsDebugOverlay::default());
  world.insert_resource(SystemTimingOverlay::default());
  world.insert_resource(InspectorState::new_with_defaults()); // auto-registers Tag/Transform/Visibility/Position/Velocity/Sprite
  ```
- Register engine toggle / pick systems at the head of the engine chain, before `__hud_toggle`, so toggles that share an input frame observe `just_pressed` first:
  ```rust
  app.add_engine_system("__physics_debug_toggle",   physics_debug_toggle_system);
  app.add_engine_system("__systems_overlay_toggle", systems_overlay_toggle_system);
  app.add_engine_system("__inspector_toggle",       inspector_toggle_system);
  app.add_engine_system("__inspector_pick",         inspector_pick_system);
  app.add_engine_system("__hud_toggle",             hud_toggle_system);
  app.add_engine_system("__display_input",          engine_display_input_system);
  app.add_engine_system("__state_dispatcher",       state_dispatcher_system);
  ```
  `physics_debug_emit_system` is NOT registered via `add_engine_system`. It is called inline from the extract stage (see below) so emit → drain ordering is locked and auditable in one place.
- `RedrawRequested` handler, in order:
  1. Run the existing systems + flush + hot-reload stages unchanged.
  2. Enter the extract stage. After the user extract closure (and after `RenderCounts` is populated) call `physics_debug_emit_system(&mut self.world)`, then drain into two POD channels:
     ```rust
     let mut debug_quads: Vec<QuadInstance>      = Vec::new();
     let mut debug_lines: Vec<DebugLineInstance> = Vec::new();
     if let Some(dd) = self.world.get_resource_mut::<DebugDraw>() {
         for cmd in dd.drain() {
             match cmd.shape {
                 DebugShape::Aabb { min, max }         => expand_aabb(&mut debug_quads, min, max, cmd.color, cmd.thickness),
                 DebugShape::Circle { center, radius, segments } => expand_circle(&mut debug_lines, center, radius, segments, cmd.color, cmd.thickness),
                 DebugShape::Line { a, b }             => debug_lines.push(DebugLineInstance { a: a.to_array(), b: b.to_array(), thickness: cmd.thickness, _pad: 0.0, color: cmd.color }),
             }
         }
     }
     ```
     `expand_aabb` produces four thin axis-aligned `QuadInstance`s (top / bottom / left / right edges, inset by `thickness / 2` so the outline tracks the AABB interior). `expand_circle` produces `segments` `DebugLineInstance`s forming a closed polyline.
  3. Compose system-timing and inspector overlays using the same `remove_resource`/`insert_resource` borrow dance used for `DebugHud`. Append returned `TextSection`s to the existing `text` vector.
  4. Call `renderer.render_frame_full[_timed](view_proj, quads, sprite_batches, &debug_quads, &debug_lines, &text)`.
- Screenshot hook: on `App::new`, read `TUNGSTEN_CAPTURE_FRAME` (parse `u32`) and `TUNGSTEN_CAPTURE_PATH` (fallback `./actual.png`) once, store on a new `CaptureConfig { target_frame: u32, path: PathBuf, captured: bool }` field. Each `RedrawRequested` increments an internal `frames_rendered: u64` counter. After the `render_frame_full` call, if `!captured && frames_rendered == target_frame`, arm `renderer.capture_frame(&path)` for the *next* frame (the renderer's `pending_capture` flag triggers the offscreen pass on the subsequent draw). Set `captured = true`. No double-render; no panic on error — log via `tracing::warn!` and continue.
- Exit: `cargo build --workspace` clean; `cargo test --workspace` passes; `./scripts/smoke-examples.sh` passes.

### 9. Visual-regression fixture

- No code change to `examples/02_sprite_stress/src/main.rs`. `TUNGSTEN_CAPTURE_FRAME` / `TUNGSTEN_CAPTURE_PATH` are handled generically in `App` (step 8), so every example inherits the capture path for free.
- Capture target resolution: the reference baseline is generated at `tungsten.json`'s startup resolution on the reference machine. To pin the fixture's resolution independent of `tungsten.json`, honour an additional env var `TUNGSTEN_CAPTURE_RESOLUTION=1280x720` inside `App::new` that overrides the startup window size only when set. This keeps fixtures reproducible across machines with different monitor DPIs.
- `examples/02_sprite_stress/tests/fixtures/baseline-sprite-stress.png`:
  - Generate on the reference machine:
    ```bash
    TUNGSTEN_SMOKE_FRAMES=8 \
    TUNGSTEN_CAPTURE_FRAME=5 \
    TUNGSTEN_CAPTURE_RESOLUTION=1280x720 \
    TUNGSTEN_CAPTURE_PATH=examples/02_sprite_stress/tests/fixtures/baseline-sprite-stress.png \
    cargo run -p example-02-sprite-stress --release
    ```
  - Commit the resulting PNG alongside a `README.md` noting the reference machine and GPU driver version.
- `examples/02_sprite_stress/tests/visual_regression.rs`:
  ```rust
  #[test]
  fn sprite_stress_matches_baseline() {
      if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() { return; } // opt-in
      let actual = std::env::temp_dir().join("tungsten-visual-regression-actual.png");
      let status = std::process::Command::new(env!("CARGO_BIN_EXE_example-02-sprite-stress"))
          .env("TUNGSTEN_SMOKE_FRAMES", "8")
          .env("TUNGSTEN_CAPTURE_FRAME", "5")
          .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
          .env("TUNGSTEN_CAPTURE_PATH", &actual)
          .status().expect("run sprite-stress");
      assert!(status.success());
      let report = compare_png(
          Path::new("tests/fixtures/baseline-sprite-stress.png"),
          &actual,
          /* tolerance */ 2,
      ).expect("compare");
      assert_eq!(report.pixels_above_tolerance, 0, "{report:?}");
  }
  ```
- CI is not a release gate here per `D-002`; this test is opt-in for local pre-release validation.
- Exit: run the opt-in test locally on the reference machine; report captured.

### 10. Platformer demo wiring + docs

- `examples/01_platformer/src/main.rs` header `Controls:` block extend (alphabetical with existing `F4`/`F9`/`F11`):
  ```
  F1              engine_toggle_physics_debug
  F2              engine_toggle_systems_overlay
  F3              engine_toggle_inspector
  ```
  No other gameplay change; overlays stay off by default.
- `docs/LLM_INDEX.md`:
  - Subsystem row `Debug tooling (M21)` → `crates/tungsten-core/src/debug_draw.rs`, `crates/tungsten-core/src/inspect.rs`, `crates/tungsten-render/src/debug_line.rs`, `crates/tungsten-render/src/screenshot.rs`, `crates/tungsten-render/src/image_diff.rs`, `crates/tungsten/src/physics_debug.rs`, `crates/tungsten/src/systems_overlay.rs`, `crates/tungsten/src/inspector.rs`, `crates/tungsten/src/app.rs`.
  - Task row `Fix a debug overlay or screenshot check` → the same files plus `docs/DECISION_INDEX.md` for `D-047`.
- `docs/DECISION_INDEX.md`: add `D-047` under `ECS / Runtime Flow`:
  `| D-047 | Debug tooling: DebugDraw is core POD drained into QuadInstance (AABB) + DebugLineInstance (lines/circles); overlays are independent action-toggled resources; screenshots use an offscreen COPY_SRC texture + row-padded readback; always-on GPU debug groups + labels. |`
- `DECISIONS.md` add `D-047` with subsections covering the four decisions called out in the "Files to touch" table.
- `docs/plans/Phase3.md`: flip `M21` status to `in progress` on start and to `complete` on land; link this plan once it is archived under `docs/plans/archive/`.
- Exit: `D-047` resolvable from `docs/DECISION_INDEX.md`; `docs/LLM_INDEX.md` task row passes fresh-agent navigation.

### 11. Perf capture

- Add a small startup block to `examples/02_sprite_stress/src/main.rs` that parses the env var `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` (comma-separated, any subset) and flips the matching resource's `.enabled = true` after `App::new` but before `App::run`. This replaces any manual-code-edit workflow and makes perf captures reproducible from the command line alone.
- Two runs on the reference machine with `scripts/perf-capture.sh`:
  ```bash
  ./scripts/perf-capture.sh sprite-stress 300                                       # overlays off
  TUNGSTEN_OVERLAYS_ON=physics,systems,inspector \
      ./scripts/perf-capture.sh sprite-stress 300                                   # overlays on
  ```
- Record both output dirs plus `perf-runs/M21-debug-tooling/README.md` summarising `total_ms` mean / p95 / p99 for each run, the `render_ms` delta attributable to `debug_quads` + `debug_lines` draws, and the `update_ms` delta attributable to `physics_debug_emit_system`.
- Flag any regression > 5 % with explicit rationale under `DECISIONS.md` (`D-047`). A regression > 10 % blocks milestone close.
- Exit: `perf-runs/M21-debug-tooling/README.md` committed; both runs reproducible purely from env vars.

### 12. Validate

Run in order:

1. `cargo fmt --all`
2. `cargo test --workspace`
3. `./scripts/smoke-examples.sh`
4. `cargo run -p example-01-platformer`:
   - `F1` toggles the physics overlay; AABBs and circle outlines align with player / ball / tile colliders.
   - `F2` toggles the system timing overlay; rows appear and stabilise under EWMA smoothing; refresh throttle honours the 250 ms cadence.
   - `F3` toggles the inspector; LMB picks the nearest entity; rows appear for `Tag`, `Transform`, `Visibility`, `Position`, `Velocity`, `Sprite` when present; stale selection is cleared cleanly.
   - `F4` (existing HUD) and `F9`/`F11` (display) still work unchanged.
5. Visual regression: run the opt-in `sprite_stress_matches_baseline` test on the reference machine; assert zero pixels above tolerance against the committed baseline.
6. Perf capture per step 11.

Manual spot checks:

- With all overlays off, `FrameTimings::render_ms` and `update_ms` are within 1 % of pre-M21 numbers.
- With all overlays on, `render_ms` climbs by the expected amount (bounded by one extra `QuadPipeline::draw` for AABB edges and the `DebugLinePipeline` draw) and nothing else regresses.
- Screenshot capture on an arbitrary frame produces a PNG with the expected resolution and non-zero content.
- `image_diff::compare_png` against a deliberately corrupted baseline returns a non-zero `pixels_above_tolerance`.
- RenderDoc (or equivalent: `VK_LAYER_LUNARG_api_dump` / `METAL_DEVICE_WRAPPER`) capture on a single platformer frame shows the named groups `tungsten_frame`, `quads`, `sprites`, `debug_quads`, `debug_lines`, `text` nested correctly, and every tracked resource (camera buffer, vertex buffer, bind group, screenshot target) carries its intended label.

## Cross-cutting concerns

| Concern | Impact |
| --- | --- |
| Frame order | `systems -> flush commands -> flush events -> hot-reload -> extract -> render`. M21 additions: (1) four engine toggle / pick systems run at the head of the systems stage, (2) `physics_debug_emit_system` runs at the start of the extract stage (before `DebugDraw::drain`), (3) `DebugDraw` is drained and split into `Vec<QuadInstance>` (AABB edges) + `Vec<DebugLineInstance>` (lines, circle polylines) inside the extract stage, (4) both channels plus composed overlay text flow into `renderer.render_frame_full[_timed]`. No change to command / event flush ordering. |
| Render pass order | Inside the single `tungsten_main_pass`: `quads -> sprites -> debug_quads -> debug_lines -> text`. `debug_quads` reuses `QuadPipeline` (no second pipeline for axis-aligned overlays). `debug_lines` uses the new `DebugLinePipeline`, which binds the same camera uniform owned by `QuadPipeline`. |
| GPU debug groups | Encoder wraps the frame in `push_debug_group("tungsten_frame")` / `pop_debug_group`. Inside the main pass, each stage opens its own named group. All wgpu resources (pipelines, buffers, bind groups, render targets) carry explicit `label:` fields. Always-on; no feature flag. RenderDoc captures are self-describing. |
| Input bindings | New action defaults: `engine_toggle_physics_debug` (`F1`), `engine_toggle_systems_overlay` (`F2`), `engine_toggle_inspector` (`F3`). Merged into `input.json` via `ActionMap::merged_with_defaults`. Mouse LMB is read by `inspector_pick_system` only while `InspectorState.enabled` — no new binding; uses `InputState::mouse_just_pressed`. |
| Config keys | None. `tungsten.json` is not touched. Env vars only (capture + overlays-on). |
| Feature flags | None at the crate level. The visual-regression test is opt-in via `TUNGSTEN_VISUAL_REGRESSION=1`; screenshot capture is opt-in via `TUNGSTEN_CAPTURE_FRAME=<n>` + optional `TUNGSTEN_CAPTURE_PATH=<path>` / `TUNGSTEN_CAPTURE_RESOLUTION=<WxH>`; perf overlays-on via `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector`. |
| Telemetry / HUD fields added | None on `FrameTimings`. `SystemTimingOverlay` owns its own `BTreeMap<String, f32>` EWMA store; does not pollute `FrameTimings`. `RenderCounts` unchanged. |
| Test layers | Layer 1 (`cargo test --workspace`) covers `DebugDraw`, `Inspectable`, EWMA smoothing, `compare_png` on in-memory PNG fixtures, and action-map defaults. Layer 2 (`./scripts/smoke-examples.sh`) covers the new render path end-to-end because every example now renders through `render_frame_full(..., debug_quads, debug_lines, ...)`. Perf-capture layer re-runs `bash scripts/test-perf-capture.sh`. The visual-regression layer is opt-in per step 9. |
| Dependencies | `image = { workspace = true }` moves into `tungsten-render` — no new crate, no new version. `D-015` rule 2 ("data format parsing") already covers it. |

## Done-when checks

- `KeyCode::F1`, `KeyCode::F2`, `KeyCode::F3` exist in `tungsten-core`; `winit`'s `F1`/`F2`/`F3` translate via `input_bridge`; key_serde round-trip covers them.
- `ActionMap::default_map` registers `engine_toggle_physics_debug`, `engine_toggle_systems_overlay`, `engine_toggle_inspector`; `input.json` mirrors them; merging with defaults preserves user overrides.
- `DebugDraw` is inserted in `App::new`, populated via `draw_aabb` / `draw_circle` / `draw_line`, and drained + cleared once per frame by the extract stage.
- AABB commands expand into four axis-aligned `QuadInstance` edges drawn via the existing `QuadPipeline`; no second AABB pipeline ships.
- `DebugLinePipeline` renders one instance per expanded line / circle-polyline-segment command and binds the same camera bind group owned by `QuadPipeline` (one camera uniform on the GPU).
- `Renderer::render_frame_full` and `Renderer::render_frame_full_timed` accept `debug_quads: &[QuadInstance]` and `debug_lines: &[DebugLineInstance]` parameters; both short-circuit on empty slice.
- GPU debug groups are present at both encoder and render-pass scope (`tungsten_frame`, `quads`, `sprites`, `debug_quads`, `debug_lines`, `text`); every new pipeline / buffer / texture / bind group carries an explicit `label:` value verified via a RenderDoc capture on the reference machine.
- Physics overlay (`F1`) outlines every `Position + Collider` entity while enabled; collider visuals align with world bounds in the platformer.
- System-timing overlay (`F2`) renders an EWMA-smoothed table sourced from `FrameTimings::system_timings`; throttled redraw mirrors `DebugHud`'s 250 ms cadence; stale system names are dropped after one absent frame.
- Entity inspector (`F3`) picks the nearest entity by cursor on LMB while enabled; renders rows from every registered `Inspectable` on the selected entity; clears stale selection without panicking.
- `App::register_inspectable::<T: Inspectable>()` wires a new component type into the inspector; `App::new` auto-registers `Tag`, `Transform`, `Visibility`, `Position`, `Velocity`, `Sprite`.
- `Renderer::capture_frame(path)` writes an RGBA PNG matching the current surface resolution; encode path runs through `image::save_buffer`.
- `image_diff::compare_png(lhs, rhs, tolerance)` returns a populated `DiffReport`; identical images yield zero `pixels_above_tolerance`; mismatched dimensions return `ImageDiffError::DimensionMismatch`.
- Opt-in `sprite_stress_matches_baseline` test passes against the committed baseline on the reference machine.
- When every overlay is disabled, no debug commands are emitted, `DebugDraw` stays empty, and the debug-line pipeline draws zero instances.
- `perf-runs/M21-debug-tooling/README.md` records `total_ms` mean / p95 / p99 for overlays-off vs. overlays-on; any regression > 5 % carries rationale in `DECISIONS.md`.
- `cargo fmt --all` clean; `cargo test --workspace` passes; `./scripts/smoke-examples.sh` passes.
- `docs/LLM_INDEX.md`, `docs/DECISION_INDEX.md`, `docs/plans/Phase3.md`, and `DECISIONS.md` (`D-047`) match the shipped shape.

## Open Questions

- `DebugLinePipeline` line-thickness conversion: screen-space thickness requires the viewport size in the vertex shader. Preferred path is `wgpu::Features::PUSH_CONSTANTS` (4-byte push constant holding `[viewport.x, viewport.y]`); the fallback is a second uniform. Resolve during step 3 by checking the adapter features reported in `Renderer::new` against what the existing `sprite_pipeline` already requests. If push constants are not universally available on our target backends, fall back to the uniform path — add a `viewport_uniform` field to `DebugLinePipeline`. Either way, no change to `QuadPipeline`.
- Visual-regression tolerance floor: shipping with `tolerance = 2` and `pixels_above_tolerance == 0`. If the Linux Vulkan path jitters at that floor during the first baseline run on the reference machine, raise to `pixels_above_tolerance < 16` (empirically small enough to catch a single-element regression, large enough to absorb driver-level noise) and record the decision under `D-047`.

Resolved during this plan pass (recorded so the decision is auditable without re-deriving it):

- Pipeline structure: `QuadPipeline` draws both gameplay quads and debug AABB edges (two `draw` calls per frame). A minimal `DebugLinePipeline` handles oriented lines + circle polylines and shares `QuadPipeline`'s camera bind group layout.
- Inspector registration: user-supplied `&'static str` labels, not `std::any::type_name::<T>()` — stable across compiler churn.
- Capture resolution: `TUNGSTEN_CAPTURE_RESOLUTION=<WxH>` pins the startup window size for fixtures; unset means use `tungsten.json` resolution.
- Capture trigger: generic `App`-level env-var hook — no per-example code. `examples/02_sprite_stress` needs only the small `TUNGSTEN_OVERLAYS_ON` parser for perf runs.
- GPU markers: in scope for M21 (always-on encoder + render-pass debug groups, explicit labels on every wgpu resource).
