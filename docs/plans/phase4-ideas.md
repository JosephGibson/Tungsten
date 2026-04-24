---
status: draft
goal: Phase 4 ships 8 milestones (M25–M32) covering render foundation, materials, stock post-effects, bloom, 2D lighting, parallax + game-feel, instanced mesh particles + transitions, MSDF text, and a collaborative showcase example.
non-goals:
  - No 3D, scripting, networking, WASM, editor (DESIGN.md §Non-Commitments).
  - No capture tooling (GIF/video/screenshot automation).
  - No deferred lighting, shadow casters, occluder polygons, volumetric lights, GI (Phase 5).
  - No asset-preprocessing pipeline (MSDF bakes at startup, not ahead of time).
  - No cleanup-only milestone; each feature milestone slices the monoliths it touches.
files to touch:
  - `docs/plans/phase4-ideas.md` (this index)
  - `docs/plans/phase4-milestone-25-*.md` through `docs/plans/phase4-milestone-32-*.md`
ordered steps:
  1. Promote each milestone below to its own `phase4-milestone-NN-*.md` plan file.
  2. Execute M25 → M26 → M27 → M28 in order. M29 ↔ M30 order is free. M31 before M32. M32 last.
  3. Each milestone: write plan, implement, produce acceptance artifact, flip status to done.
done-when:
  - All 8 milestones landed on `main`, each with `status: done` in its plan file.
  - `DESIGN.md § Status` updated. `CHANGELOG.md` entry added. This file flipped to `status: done`.
---

## Current Renderer Baseline

- Single pass direct to swapchain, no depth, no MSAA.
- Pipelines: [sprite](crates/tungsten-render/src/sprite.rs), [quad](crates/tungsten-render/src/quad.rs), [debug_line](crates/tungsten-render/src/debug_line.rs), [text](crates/tungsten-render/src/text.rs).
- Text via `glyphon` + `cosmic-text` ([D-026](DECISIONS.md)).
- WGSL embedded via `include_str!`, no hot reload ([D-023](DECISIONS.md)).
- [renderer.rs](crates/tungsten-render/src/renderer.rs) entrypoints: `render_frame`, `render_frame_with_quads`, `render_frame_full`, `render_frame_full_timed`.

Phase 4 adds: render targets, depth, optional MSAA, shader hot reload, user materials, post-stack, bloom, normal-mapped lighting, parallax, screen-shake, squash/stretch, instanced mesh particles, screen transitions, MSDF text, capstone example.

---

## M25 — Render Foundation

**Depends on:** none.

**Adds (crates/tungsten-render/src/):**
- `targets.rs` — `RenderTargetPool`, `SceneTarget { color, depth, msaa? }`.
- `passes/` — `PassDesc`, `PassRecorder`, `PassOrder`. Named targets + ordered pass list (no DAG).
- `shader_hot_reload.rs` — `notify`-backed WGSL watcher, `naga` validation, `ShaderModuleCache` keyed by `ShaderAssetId`.

**Adds (data types):**
- `enum TargetId { SceneColor, SceneDepth, PostPing, PostPong, Swapchain }`.
- `struct ShaderAssetId(u32)`, `struct ShaderRegistry` (resource in `tungsten-core` via opaque ID; WGSL text + compiled `wgpu::ShaderModule` stored render-side, per [D-016](DECISIONS.md) pattern).

**Adds (config — `tungsten.json` `render.*`):**
- `msaa: u32` (1, 2, 4, 8; default 1).
- `depth_enabled: bool` (default true).

**Adds (DECISIONS.md):**
- New entry narrowing [D-023](DECISIONS.md): shaders become manifest-tracked `.wgsl` assets with `notify` + `naga` validation; rebuild still required for signature changes; hot reload on body edits only. Cite `D-053` hot-reload matrix and extend it with `shader` row.

**Touches:**
- [renderer.rs](crates/tungsten-render/src/renderer.rs) — extract surface/config/timing into sibling modules; `render_frame_full` routes through new pass list.
- [asset_loader.rs](crates/tungsten/src/asset_loader.rs) — new shader load path; `reload_shader` handler.
- [manifest.rs](crates/tungsten-core/src/assets/manifest.rs) — `shaders: Vec<ShaderEntry>` section.
- [hot_reload.rs](crates/tungsten/src/hot_reload.rs) — route `.wgsl` edits.

**Optional depth-test path:**
- `Sprite` gains no field. Existing `z_order` CPU-sort stays default.
- `RenderConfig { depth_sort: DepthSortMode::CpuStable | DepthSortMode::GpuDepth }` switches behavior.
- GPU path writes `z_order as f32 / i32::MAX as f32` to `gl_Position.z`; sprite fragment enables depth write.

**Acceptance:** `cargo run -p example-02-sprite-stress` renders through the offscreen pipeline; saving `sprite.wgsl` updates visuals without rebuild; smoke script passes; test layer 2 passes with `TUNGSTEN_SMOKE_FRAMES=3`.

---

## M26 — Materials + Post-Stack + Tween→Material Bridge

**Depends on:** M25.

**Adds (crates/tungsten-render/src/):**
- `material.rs` — `MaterialPipeline`, `MaterialUniforms` (fixed 256-byte UBO; 4 `Vec4` + 4 `f32` + 4 `i32` slots by name).
- `post/` — `PostStack` resource, `PostPass` enum variants (one per stock effect), ping-pong target swap in `passes/`.
- `shaders/stock/` — vendored WGSL:
  - `shaders/stock/lygia/` — cherry-picked LYGIA helpers (noise, hash, srgb, luma) with MIT attribution header.
  - `shaders/stock/tonemap.wgsl`, `vignette.wgsl`, `lut.wgsl`, `chromatic_aberration.wgsl`, `color_adjust.wgsl` (hue/sat/contrast), `tone_mono.wgsl` (sepia/mono/duotone).
  - `shaders/stock/crt.wgsl`, `film_grain.wgsl`, `dither.wgsl`, `pixel_outline.wgsl`.
  - `shaders/stock/fade.wgsl`, `wipe_radial.wgsl`, `dissolve.wgsl`, `glitch.wgsl`, `pixelate.wgsl`.
  - `shaders/stock/fog.wgsl`, `god_rays.wgsl`.

**Adds (data types in tungsten-core):**
- `struct MaterialAssetId(u32)` + `MaterialRegistry` resource.
- `enum PostPass { Tonemap(TonemapParams), Vignette(VignetteParams), Lut(LutParams), ChromaticAberration(f32), ColorAdjust { hue, sat, contrast }, ToneMono(ToneMonoParams), Crt(CrtParams), FilmGrain(f32), Dither(DitherParams), PixelOutline(PixelOutlineParams), Fade(f32), WipeRadial(f32), Dissolve(f32), Glitch(GlitchParams), Pixelate(f32), Fog(FogParams), GodRays(GodRaysParams) }`.
- `struct PostStack(Vec<PostPass>)` resource.

**Adds (Sprite extension):**
- `Sprite.material_id: Option<MaterialAssetId>` — `None` uses built-in `sprite.wgsl`.

**Adds (manifest):**
- `materials: Vec<MaterialEntry { id, shader_asset_id, uniform_defaults }>` section.
- LUT images go under `sprites` section as regular assets.

**Adds (tween bridge — tungsten-core/src/tween.rs):**
- `TweenTarget::Material { entity, uniform_slot: MaterialSlot }`.
- `enum MaterialSlot { Vec4(u8), Scalar(u8), Int(u8) }` — index into the fixed UBO layout.
- `tween_tick_system` writes the resolved value into the entity's `Sprite.material_id`'s per-entity uniform override (new `MaterialUniformOverride` component).

**Stock roster (final — 17 effects in M26):**

| Bucket | Effects |
| --- | --- |
| Color | Tonemap, Vignette, LUT, ChromaticAberration, ColorAdjust, ToneMono |
| Retro | CRT, FilmGrain, Dither, PixelOutline |
| Transition | Fade, WipeRadial, Dissolve, Glitch, Pixelate |
| Environmental | Fog, GodRays |

**Touches:**
- [asset_loader.rs](crates/tungsten/src/asset_loader.rs) — `asset_loader/material.rs` split; `reload_material` handler.
- [sprite.rs](crates/tungsten-render/src/sprite.rs) — per-batch material selection.
- [tweens.rs](crates/tungsten/src/tweens.rs) — material-uniform channel path.

**Acceptance:**
- `examples/04_shader_playground/` — bouncing sprite + on-screen key list that toggles each of the 17 effects individually and cycles a preset stack.
- Gif showing damage-flash uniform driven by a one-shot tween in [examples/01_platformer/](examples/01_platformer/).

---

## M27 — Bloom

**Depends on:** M25, M26.

**Adds (crates/tungsten-render/src/):**
- `post/bloom.rs` — `BloomPipeline { threshold, downsample, upsample, composite }`, mip chain allocator.
- `shaders/stock/bloom_threshold.wgsl`, `bloom_downsample.wgsl`, `bloom_upsample.wgsl`, `bloom_composite.wgsl`.

**Algorithm:**
- Threshold extract from `SceneColor` with soft knee `(brightness, knee)` into mip 0 of `BloomPyramid`.
- 6-level downsample with 13-tap Karis-averaged filter.
- Progressive upsample with 9-tap tent filter, accumulate into mip 0.
- Additive composite into `PostPing` as final `PostPass::Bloom { threshold, knee, intensity, radius }`.

**Adds to PostPass enum (M26):**
- `Bloom(BloomParams)` variant appended; `PostStack` accepts it.

**Config:**
- `render.bloom_max_mips: u32` (default 6, clamped by viewport size).

**Touches:**
- [targets.rs](crates/tungsten-render/src/) from M25 — `BloomPyramid { mips: Vec<TextureView> }` allocator, resized on surface resize.

**Acceptance:** gif of `examples/04_shader_playground/` with an emissive sprite (flat bright quad as placeholder) toggling `Bloom` on/off via HUD; second gif with threshold/intensity/radius slid via keys.

---

## M28 — 2D Lighting (Forward, Normal-Mapped)

**Depends on:** M25, M26. M27 recommended (bloom + emissive).

**Adds (crates/tungsten-core/src/):**
- `components.rs` — `Light { kind: LightKind, color: Vec3, intensity: f32 }`, `enum LightKind { Point { radius: f32, falloff: f32 }, Directional { angle: f32 } }`.
- Resource `AmbientLight(Vec3)` — default `Vec3::ONE`.

**Adds (crates/tungsten-render/src/):**
- `lighting.rs` — `LightUbo { lights: [GpuLight; 16], count: u32, ambient: Vec3 }`, `GpuLight { position, color, params }` as 32-byte POD.
- `shaders/lit_sprite.wgsl` — samples albedo, normal, emissive; N-dot-L accumulation across lights; additive rim term; emissive mask add.
- `shaders/stock/emissive_mask.wgsl`, `rim_light.wgsl` as composable helpers (callable from lit_sprite and from user materials).

**Adds (manifest — sprite entry):**
- `normal_map: Option<AssetId>`.
- `emissive_mask: Option<AssetId>` (single-channel mask, or alpha of normal_map if set and emissive_mask is None).

**Adds (extract):**
- `extract_lights(&World) -> LightUbo` — queries `(Transform, Light)`, culls by `CameraState::visible_world_aabb()`, caps at 16, writes to UBO.
- Lit sprite batch path: if sprite has `normal_map`, routed through `lit_sprite.wgsl` pipeline instead of `sprite.wgsl`.

**Light cull:** distance-to-camera-AABB sort, keep nearest 16.

**Acceptance:** [examples/01_platformer/](examples/01_platformer/) gets normal-mapped character + 2 colored point lights + 1 directional; gif shows lighting response during movement; emissive eyes trigger M27 bloom.

---

## M29 — Parallax + Screen-Shake + Squash/Stretch

**Depends on:** M25 (extract changes share seam).

**Adds (crates/tungsten-core/src/components.rs):**
- `ParallaxLayer { scroll_factor: Vec2, depth_bucket: i32 }`.
- `SpriteSquashStretch { on: SquashTrigger, amount: Vec2, duration: f32 }`.
- `enum SquashTrigger { OnLand, OnHit, OnPickup, Manual }`.

**Adds (crates/tungsten-core/src/camera.rs):**
- `CameraState.shake_offset: Vec2`, `CameraState.shake_trauma: f32`.
- Trauma model: `offset = max_offset * trauma^2 * noise()`, decay `trauma -= trauma_decay * dt`.

**Adds (crates/tungsten-core/src/ecs/event_queue.rs usage):**
- `ShakeEvent { trauma_add: f32 }` — event-driven shake triggers.

**Adds (systems, registered in umbrella crate):**
- `parallax_extract_system` — groups sprite extract by `depth_bucket`, emits one view_proj per bucket (base `CameraState.position * scroll_factor`).
- `shake_tick_system` — reads `EventQueue<ShakeEvent>`, advances trauma decay, updates `shake_offset`.
- `squash_stretch_trigger_system` — on matching event, inserts a one-shot `Tween` (M24) writing `Transform.scale`.

**Touches:**
- [sprite_extract.rs](crates/tungsten/src/sprite_extract.rs) — bucket-aware grouping.
- [camera.rs](crates/tungsten/src/camera.rs) — shake offset applied before view matrix build.

**Acceptance:** platformer gif — 3-layer parallax (sky / mid-hills / near-trees) scrolling horizontally, character lands with visible squash, on-hit screen shake + damage-flash material uniform fires.

---

## M30 — Instanced Mesh Particles + Screen Transitions

**Depends on:** M23 (particles), M26 (materials/post-stack).

**Adds (crates/tungsten-render/src/):**
- `mesh_particle.rs` — `MeshParticlePipeline`, `ParticleMesh { vertices: Vec<Vec2>, indices: Vec<u16> }` uploaded once, instanced per live particle.
- `transition.rs` — `TransitionState { phase: Out | Swap | In, easing, elapsed }`.

**Adds (crates/tungsten-core/src/assets/particle.rs):**
- `ParticleConfig.render: ParticleRender` — `enum ParticleRender { Quad, Mesh { mesh_id: ParticleMeshAssetId } }`.
- `ParticleMeshAssetId(u32)` + `ParticleMeshRegistry` resource.

**Adds (crates/tungsten-core/src/assets/manifest.rs):**
- `particle_meshes: Vec<ParticleMeshEntry { id, vertices, indices }>` — inline JSON or separate `.mesh.json`.

**Adds (crates/tungsten-core/src/):**
- `transitions.rs` — `Transition { out_effect: PostPass, in_effect: PostPass, duration: f32 }`, `RequestTransition { target: StateRequest, transition: Transition }` resource/event.

**State-system integration:**
- [state.rs](crates/tungsten/src/state.rs) — `state_dispatcher_system` checks `RequestTransition`; runs `out_effect` via `PostStack` with driven progress uniform, swaps state at phase boundary, runs `in_effect` on the new state, clears transition.

**Acceptance:**
- Bullet-trail spawn in `examples/04_shader_playground/` using a triangle mesh (instanced).
- [examples/03_scene_state/](examples/03_scene_state/) transitions between states using each of fade, radial wipe, dissolve, pixelate-out.

---

## M31 — MSDF Text

**Depends on:** M25 (shader assets), M26 (material-uniform patterns for outline/glow controls).

**Adds (new dep):**
- `msdfgen` crate + `ttf_parser` crate. Both satisfy [D-015](DECISIONS.md) rule 2 (well-specified format). Add `DECISIONS.md` entry narrowing [D-026](DECISIONS.md): `cosmic-text` retained for layout; rasterizer path split — `glyphon` remains default, MSDF is opt-in.

**Adds (crates/tungsten-render/src/):**
- `msdf_text.rs` — `MsdfTextPipeline`, `MsdfAtlas { texture, glyph_metrics: HashMap<GlyphId, GlyphMetrics> }`, per-frame vertex buffer.
- `shaders/msdf_text.wgsl` — median-of-RGB threshold, optional outline + glow uniforms.

**Adds (crates/tungsten-core/src/):**
- `MsdfText { text: String, font_id: AssetId, px: f32, color: [u8;4], outline: Option<OutlineParams>, glow: Option<GlowParams> }` component.
- `MsdfFontRegistry` — parallel to `FontRegistry`, keyed by font ID.

**Bake pipeline:**
- At startup, for each manifest font with `msdf: true`, `msdfgen` + `ttf_parser` generate a 512×512 atlas (ASCII + Latin-1 Supplement + user-declared extra ranges) on a background thread pool (reuse asset-load path).
- Atlas cached in memory only (no disk cache in M31).

**Extract:**
- `extract_msdf_text(&World)` — queries `(Transform, MsdfText, Visibility)`, resolves via `MsdfFontRegistry`, uses `cosmic-text` `Buffer` for layout, emits per-glyph quad instances referencing atlas UVs.

**Touches:**
- [text.rs](crates/tungsten-render/src/text.rs) — unchanged (`glyphon` path stays); MSDF runs as a sibling pipeline.
- [asset_loader.rs](crates/tungsten/src/asset_loader.rs) — `asset_loader/msdf.rs` split.

**Acceptance:** side-by-side gif — same string at 3 zoom levels rendered via `glyphon` vs `MsdfText`; outline + glow animated via tween→material-uniform bridge.

---

## M32 — Showcase Example (scope-locked at kickoff, not now)

**Depends on:** M25–M31 all done.

**Hard rule:** no `examples/05_showcase/` directory, no asset production, no scope lock until M31 is flipped to `status: done`.

**Locked requirements (only these):**
- Lives in `examples/05_showcase/` with local `assets/manifest.json`.
- Must exercise: materials + ≥5 stock effects (M26), bloom (M27), ≥2 point lights + 1 directional + normal maps (M28), ≥3 parallax layers + shake + squash (M29), ≥1 mesh-instanced particle system + ≥1 screen transition (M30), MSDF title card (M31).
- Acceptance: ~30-second clip + 3 PNG stills committed under `docs/showcase/`.

Shape (game or non-game), assets, and systems are designed in the M32 plan file, not here.

---

## Final Ordering

| # | ID | Title | Hard Deps |
| --- | --- | --- | --- |
| 1 | M25 | Render foundation | — |
| 2 | M26 | Materials + post-stack + tween→material | M25 |
| 3 | M27 | Bloom | M25, M26 |
| 4 | M28 | 2D lighting + emissive + rim | M25, M26 |
| 5 | M29 | Parallax + shake + squash | M25 |
| 6 | M30 | Mesh particles + transitions | M23, M26 |
| 7 | M31 | MSDF text | M25, M26 |
| 8 | M32 | Showcase | M25–M31 |

M29 and M30 may swap. M31 must precede M32.

## Resolved Decisions

- 8 milestones. Locked.
- 17 stock effects in M26; bloom in M27; emissive + rim in M28 lit path. Total Phase 4 effect count: 20.
- LYGIA WGSL snippets vendored under `crates/tungsten-render/src/shaders/stock/lygia/` with header attribution. No crate dependency.
- MSDF narrows [D-026](DECISIONS.md) (does not reverse). `cosmic-text` retained for layout.
- [D-023](DECISIONS.md) narrowed by M25 decision entry. Shader hot reload for body edits only; signature changes require rebuild.
- M26 `04_shader_playground` stays minimal. Heavy example authoring only in M32.
- Depth-test sprite path ships in M25 as opt-in; `z_order` CPU-sort remains default.

## Sources

- [msdfgen — C++ reference implementation](https://github.com/Chlumsky/msdfgen)
- [msdfgen Rust crate](https://docs.rs/msdfgen)
- [awesome-msdf — shader collection](https://github.com/Blatko1/awesome-msdf)
- [SDF Fonts — redblobgames](https://www.redblobgames.com/blog/2024-03-21-sdf-fonts/)
- [LYGIA shader library](https://lygia.xyz/)
- [Godot CanvasItem shader reference](https://docs.godotengine.org/en/stable/tutorials/shaders/shader_reference/canvas_item_shader.html)
- [gdquest-demos/godot-shaders — 2D reference](https://github.com/gdquest-demos/godot-shaders)
- [LearnOpenGL — 2D post-processing](https://learnopengl.com/In-Practice/2D-Game/Postprocessing)
