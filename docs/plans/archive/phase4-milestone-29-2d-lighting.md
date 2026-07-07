---
status: done
milestone: M29
goal: "Ship M29 forward 2D normal-mapped lighting: a `Light` component + `AmbientLight` resource feeding a 16-light `LightUbo`, a `lit_sprite.wgsl` pipeline that samples albedo + normal + emissive, manifest-tracked `sprites.<id>.normal_map` / `emissive_mask` siblings packed into per-filter parallel atlas pages keyed off the existing `SpriteAsset.uv` rect, and renderer routing that picks the lit pipeline for any sprite carrying a lit atlas bundle. Empty light list + no normal-mapped sprites keep the M28 frame byte-identical."
non-goals:
  - "No deferred lighting, shadow casters, occluder polygons, volumetric lights, GI, baked GI, light volumes, or stencil shadows. Phase 5 owns those."
  - "No HDR `SceneColor`. Lit fragments stay LDR-clamped pre-write; emissive headroom rides M28 bloom (D-060) thresholding only."
  - "No per-light culling shape beyond distance-to-camera-AABB sort + top-16 keep. No spotlights, no cones, no IES profiles."
  - "No material × lit composition. A sprite with `material_id = Some(_)` cannot also use the lit pipeline in M29 — extract logs `WARN` and uses the lit path. M33 may revisit."
  - "No specular term, no anisotropy, no PBR. Lit shading is N·L diffuse + additive rim + emissive add."
  - "No animated normal-map flipbooks. Each `walk_N` sprite carries its own static normal/emissive sibling."
  - "No new dependency. Existing `wgpu`, `bytemuck`, `glam` cover pipeline + UBO + math (`D-015` already satisfied for these)."
  - "No runtime light cap reconfiguration. `LIGHT_CAP = 16` is a `const`; raising it is a recompile + UBO re-pack."
  - "No screenshot/GIF automation; acceptance artifacts are committed PNGs."
files to touch:
  - "crates/tungsten-core/src/components.rs                       — `Light`, `LightKind` (`Point { radius, falloff }` / `Directional { angle }`)"
  - "crates/tungsten-core/src/lib.rs                              — re-export `Light`, `LightKind`, `AmbientLight`"
  - "crates/tungsten-core/src/lighting.rs                         — new module: `AmbientLight(Vec3)` resource, `LIGHT_CAP = 16`"
  - "crates/tungsten-core/src/tests/components.rs                 — `Light` default coverage; `LightKind` variants"
  - "crates/tungsten-core/src/tests/lighting.rs                   — `AmbientLight` default = `Vec3::ONE`; `LIGHT_CAP == 16`"
  - "crates/tungsten-core/src/assets/manifest.rs                  — extend `SpriteEntry` / `ResolvedSprite` with `normal_map: Option<String>`, `emissive_mask: Option<String>` (sibling files, not parallel sprite ids)"
  - "crates/tungsten-core/src/assets/registry.rs                  — `SpriteAsset` gains `normal_path`, `emissive_path`, and `lit_atlas: Option<TextureHandle>`; `register_sprite` signature kept by `#[allow(clippy::too_many_arguments)]` per M22 precedent; reverse path lookup includes sibling files for hot reload"
  - "crates/tungsten-core/tests/manifests.rs                      — keep coverage; new root `assets/sprites/walk_*` normal/emissive siblings cover the new fields"
  - "crates/tungsten-render/src/lighting.rs                       — new module: `LIT_LIGHT_CAP = 16`, `GpuLight` (32-byte POD), `LightUbo` (`Std140`-friendly: `[GpuLight; 16]` + `count_pad: [u32; 4]` + `ambient: [f32; 4]`, 544 bytes), `LightingResources { ubo, bind_group, bind_group_layout }`, `pack_lights(...)`, `cull_to_cap(...)`"
  - "crates/tungsten-render/src/lit_sprite.rs                     — new `LitSpritePipeline { pipeline, pipeline_layout }` reusing `SpritePipeline::vertex_layouts()` + camera + sampler + lit textures bind group + `LightingResources`"
  - "crates/tungsten-render/src/sprite.rs                         — `SpritePipeline` grows a parallel `lit_textures: HashMap<TextureHandle, GpuLitTextures>` pool + `upload_lit_texture(...)` + `lit_texture_bind_group_layout()` accessor; `SpriteBatch` gains `lit: bool` (default false) so the renderer can pick pipeline by batch flag without re-reading world data"
  - "assets/shaders/sprite.wgsl                                   — unchanged"
  - "assets/shaders/lit_sprite.wgsl                               — new compile-time + manifest source: samples albedo (group 1.0), normal (1.1), emissive (1.2); reads `LightUbo` (group 2.0); per-fragment N·L sum across `count` lights with optional rim + emissive add; depth-write parity with the unlit sprite path"
  - "crates/tungsten-render/src/shaders/stock/emissive_mask.wgsl  — new helper (callable by user materials too); sample mask, multiply by tunable `emissive_strength`"
  - "crates/tungsten-render/src/shaders/stock/rim_light.wgsl      — new helper; `rim_color * pow(1 - max(0, n_dot_v), rim_power)` style additive contribution"
  - "crates/tungsten-render/src/renderer.rs                       — own `LightingResources`, `LitSpritePipeline`, `LIT_SPRITE_SHADER_NAME`, `EMISSIVE_MASK_SHADER_NAME`, `RIM_LIGHT_SHADER_NAME`; pre-seed three new ids in `ShaderModuleCache` after bloom; route `upload_shader` / `reload_shader` to `LitSpritePipeline::rebuild_with_shader` for the lit shader; `upload_lit_texture(...)` mirrors `upload_texture`; `update_lights(&LightUbo)` writes the UBO; lit/unlit batches share `record_main_draws` but bind group 2 differs"
  - "crates/tungsten-render/src/lib.rs                            — re-export `LIT_SPRITE_SHADER_NAME`, `LitSpritePipeline`, `LightUbo`, `GpuLight`, `LIT_LIGHT_CAP`"
  - "crates/tungsten-render/src/tests/lighting.rs                 — new: `LightUbo` 544-byte payload check (16×32 + 16 count/pad + 16 ambient) and `cull_to_cap` ordering"
  - "crates/tungsten-render/src/tests/lit_sprite.rs               — new: `LIT_SPRITE_SHADER_NAME` constant, lit batch keying split"
  - "crates/tungsten-render/src/tests/passes_order.rs             — assert lit sprite slot does not change pass list shape (still scene → post → text → present)"
  - "crates/tungsten-render/src/tests/sprite.rs                   — extend with lit-batch routing test"
  - "crates/tungsten/src/sprite_extract.rs                        — `BatchKey` grows `lit: bool`; lit batches set `SpriteBatch.lit = true` when the resolved `SpriteAsset.lit_atlas.is_some()`; warn when `material_id` and `lit_atlas` collide on one sprite (lit wins)"
  - "crates/tungsten/src/light_extract.rs                         — new: `extract_lights(&World, &CameraState, viewport_w, viewport_h) -> LightUbo`; queries `(Transform, Light)`; culls by visible-AABB-distance; caps at 16; reads `AmbientLight` resource (defaults `Vec3::ONE`)"
  - "crates/tungsten/src/asset_loader.rs                          — `load_sprites` decodes optional normal/emissive siblings and packs them into parallel per-filter atlas canvases using the exact albedo `PackedSprite` placements; ID-stable so `SpriteAsset.lit_atlas` shares the UV rect of the matching `SpriteAsset.atlas`; `reload_sprite` mirrors aux paths; `reload_manifest` warn-on-removal"
  - "crates/tungsten/src/lib.rs                                   — `pub mod light_extract; pub use light_extract::extract_lights;`"
  - "crates/tungsten/src/app.rs                                   — `extract_lights` runs in `stage_extract` between sprite extract and render; result fed to `Renderer::update_lights` before draw"
  - "crates/tungsten/src/tests/light_extract.rs                   — new: ambient default; cull-cap; directional + point ordering"
  - "assets/sprites/walk_0_n.png .. walk_3_n.png                  — new normal maps (tangent-space; flat 0.5/0.5/1 baseline)"
  - "assets/sprites/walk_0_e.png .. walk_3_e.png                  — new emissive masks (single-channel-as-alpha eyes glow)"
  - "assets/manifest.json                                         — `walk_0..3` entries gain `normal_map` + `emissive_mask` siblings; append `shaders.lit_sprite`, `shaders.emissive_mask`, `shaders.rim_light`"
  - "examples/01_platformer/src/setup.rs                          — spawn 2 `Light::point` (warm + cool) tracking the ball arc, 1 `Light::directional` from upper-right; insert `AmbientLight(Vec3::splat(0.35))`"
  - "examples/01_platformer/src/systems.rs                        — orbit_lights_system advancing the two point-light positions over time"
  - "examples/01_platformer/src/extract.rs                        — custom platformer extract opts the player batch into `SpriteBatch.lit` when `TUNGSTEN_LIGHTING_FIXTURE=on`; lit wins over the existing damage-flash material in that fixture only"
  - "examples/01_platformer/src/setup.rs                          — register `orbit_lights_system` in `RUNTIME_SYSTEM_ORDER`"
  - "assets/shaders/stock/emissive_mask.wgsl                      — byte-equal mirror"
  - "assets/shaders/stock/rim_light.wgsl                          — byte-equal mirror"
  - "tungsten.json                                                — no new key (lighting has no startup config in M29); document defaults in `DESIGN.md`"
  - "scripts/smoke-examples.sh                                    — append `TUNGSTEN_LIGHTING_FIXTURE=on` row over `example-01-platformer` (smoke-frame variant; matches existing post-AA / bloom row pattern)"
  - "docs/showcase/lighting_off_vs_on.png                         — 2-up still capture; left `TUNGSTEN_LIGHTING_FIXTURE=off` custom-unlit path, right normal-mapped + 2 point + 1 directional"
  - "docs/showcase/README.md                                      — M29 regen recipe (capture env vars + ImageMagick `+append`)"
  - "DECISIONS.md                                                 — append `D-061` (M29 lighting)"
  - "docs/DECISION_INDEX.md                                       — one-line `D-061` row under Assets / Rendering"
  - "docs/LLM_INDEX.md                                            — Lighting (M29) row"
  - "AGENTS.md                                                    — add lighting bullet to the asset-rules paragraph"
  - "DESIGN.md                                                    — frame-order + hot-reload-matrix updates for lit sprite + normal/emissive sibling files"
  - "CHANGELOG.md                                                 — `0.27` section: M29 lighting"
  - "README.md                                                    — flip M29 status row"
  - "docs/plans/phase4.md                                         — flip M29 row to `done — shipped in 0.27` and reference the archived plan path"
ordered steps:
  - "Step 1 — extend `tungsten-core`: add `Light` + `LightKind` to `components.rs`; add `lighting.rs` with `AmbientLight(Vec3)` resource and `pub const LIGHT_CAP: usize = 16`; re-export through `lib.rs`. Cover with `tests/components.rs` + `tests/lighting.rs`."
  - "Step 2 — extend `SpriteEntry` / `ResolvedSprite` with `normal_map: Option<String>` (path relative to the manifest, like `path`) and `emissive_mask: Option<String>`; resolve into absolute `PathBuf` in `ResolvedManifest::load`. `serde` defaults to `None` so existing manifests stay valid. The workspace manifest integration test covers the new root `walk_*` siblings after Step 14."
  - "Step 3 — extend `SpriteAsset` with `normal_path: Option<PathBuf>`, `emissive_path: Option<PathBuf>`, and `lit_atlas: Option<TextureHandle>`; thread through `register_sprite` (kept under `#[allow(clippy::too_many_arguments)]`) and `update_sprite_entry`. Register both sibling paths in `path_to_sprite_id` so hot reload can map a changed normal/emissive PNG back to its sprite id. Default `None` keeps the M28 hot-reload + atlas paths byte-identical."
  - "Step 4 — `crates/tungsten-render/src/lighting.rs`: define `GpuLight` (32 bytes, `[f32; 8]` slots), `LightUbo` (`std140`-friendly: `[GpuLight; 16]` + `[u32; 4]` for `(count, _pad, _pad, _pad)` + `[f32; 4]` ambient; total 544 bytes). Provide `pack_lights(slice: &[GpuLight], ambient: Vec3) -> LightUbo` and `cull_to_cap(camera_aabb, lights: &[(Vec2, Light)]) -> Vec<GpuLight>` (distance-to-AABB sort, keep nearest 16). Build `LightingResources::new(device)` that allocates the UBO + a `lighting_bgl` (group 2) and the matching bind group."
  - "Step 5 — vendor `assets/shaders/lit_sprite.wgsl` as the single compile-time + manifest-tracked lit sprite source, matching the existing `assets/shaders/sprite.wgsl` pattern. Add helper shaders under `crates/tungsten-render/src/shaders/stock/` with byte-equal mirrors under `assets/shaders/stock/`. Register the three shader ids in the workspace `assets/manifest.json` so manifest-driven hot reload picks up edits."
  - "Step 6 — `crates/tungsten-render/src/lit_sprite.rs`: build `LitSpritePipeline::new(device, format, sample_count, depth_write, sprite_pipeline.camera_bind_group_layout(), lit_texture_bgl, lighting_bgl)` against the shared sprite vertex+instance layout. Add `rebuild_with_shader(device, module, ...)` mirroring `SpritePipeline::rebuild_with_shader`."
  - "Step 7 — `crates/tungsten-render/src/sprite.rs`: introduce `GpuLitTextures { albedo_view, normal_view, emissive_view, bind_group, filter }` keyed by the albedo atlas `TextureHandle` in a sibling `lit_textures` pool. Add `upload_lit_texture(device, queue, handle, albedo_rgba, normal_rgba, emissive_rgba, w, h, filter)` (one bind group, three views, one sampler) and `drop_lit_texture(handle)`. Albedo remains `Rgba8UnormSrgb`; normal and emissive textures are `Rgba8Unorm` so normal vectors and masks are not gamma-decoded. `SpriteBatch` grows `lit: bool` defaulting `false` so the M28 batch shape is byte-equal."
  - "Step 8 — `crates/tungsten-render/src/renderer.rs`: pre-seed three new ids in `ShaderModuleCache` after bloom (8/9/10), bump `next_shader_id = 11`, build `LitSpritePipeline` + `LightingResources` in `Renderer::new`, expose `update_lights(&LightUbo)` (one `queue.write_buffer`; resize does not invalidate the lighting bind group), `upload_lit_texture(...)`, route `upload_shader` / `reload_shader` to rebuild the lit pipeline on `LIT_SPRITE_SHADER_NAME` (and material rebuild branch keeps the lit/emissive/rim names off the fall-through)."
  - "Step 9 — update `record_main_draws` / sprite draw to walk `sprite_batches` in their existing extracted order and branch per batch (`batch.lit`, `material_id`, or built-in). Do not globally partition unlit and lit batches; that would break the current `(z_order, entity_id)` painter ordering across lit/unlit overlaps. Lit batches carry `material_id = None` after Step 11's collision warn; bloom slot stays untouched."
  - "Step 10 — `crates/tungsten/src/asset_loader.rs`: extend `Decoded` to carry optional normal/emissive paths and decoded RGBA. Run `pack_shelf` once per filter for the albedo inputs, then use the returned `PackedSprite` placements to fill albedo, flat-normal, and emissive canvases page-by-page. Do not repack a lit subset. Upload the unlit albedo atlas through `upload_texture` and upload the lit bundle for the same page handle through `upload_lit_texture`; `SpriteAsset.lit_atlas = Some(asset.atlas)` only for sprites with a valid normal sibling. On `reload_sprite`, decode and write the changed albedo/normal/emissive cell through `write_subtexture` / `write_subtexture_lit`; if dimensions grow, rebuild the whole filter class. `reload_manifest` warns when an existing sprite's `normal_map` / `emissive_mask` is removed (last-known-good)."
  - "Step 11 — `crates/tungsten/src/sprite_extract.rs`: extend `BatchKey` with `lit: bool`, set on the resolved `SpriteAsset.lit_atlas.is_some()` axis. When a sprite carries both `material_id = Some` and `lit_atlas = Some`, log `WARN` and force the lit path. Do not use a global `OnceLock`/static warning cache; global mutable state is forbidden, and this collision should be rare enough that a direct warn is acceptable in M29."
  - "Step 12 — `crates/tungsten/src/light_extract.rs`: implement `extract_lights(&World, &CameraState, viewport_w, viewport_h) -> LightUbo`. Queries `(Transform, Light)`, culls by `CameraState::visible_world_aabb`, sorts by AABB-clamped distance, keeps the nearest 16 with `Directional` always retained; reads optional `AmbientLight` resource (defaults `Vec3::ONE`)."
  - "Step 13 — `crates/tungsten/src/app.rs`: in `stage_extract`, run `extract_lights` after sprite extract; pass result into a new `Renderer::update_lights(&LightUbo)` call inside `stage_render` before `render_frame_full*`. Add `pub mod light_extract;` + re-export. Cover with `tests/light_extract.rs`."
  - "Step 14 — extend `examples/01_platformer`: ship root `assets/sprites/walk_*_n.png` and `walk_*_e.png` siblings, append `normal_map` + `emissive_mask` to each root `walk_N` manifest entry, spawn a warm + cool `Light::Point` (radius 6.0 / falloff 1.5), one `Light::Directional` from upper-right, and insert `AmbientLight(Vec3::splat(0.35))`. Add `orbit_lights_system` to `RUNTIME_SYSTEM_ORDER` in `setup.rs`. Parse `TUNGSTEN_LIGHTING_FIXTURE=on|off` at startup into an example resource; when off, skip light entity spawn, reset `AmbientLight` to `Vec3::ONE`, and keep the custom platformer player batch on the existing material/unlit path. When on, `examples/01_platformer/src/extract.rs` must set `SpriteBatch.lit = true` for `walk_*` player batches and clear `material_id` so the non-goal material×lit collision is explicit instead of accidental."
  - "Step 15 — `scripts/smoke-examples.sh`: append a `TUNGSTEN_LIGHTING_FIXTURE=on` row over `example-01-platformer`. Capture `docs/showcase/lighting_off_vs_on.png` (2-up; left = lighting fixture off, right = on); update `docs/showcase/README.md` regen recipe."
  - "Step 16 — author `D-061`, sync `docs/DECISION_INDEX.md`, `docs/LLM_INDEX.md`, `AGENTS.md`, `DESIGN.md` (frame-order paragraph: lit sprite is a sibling pipeline inside the scene pass; hot-reload matrix gains `lit_sprite`/`emissive_mask`/`rim_light` shader rows + `normal_map`/`emissive_mask` sibling-file rows under sprites); update `CHANGELOG.md` and `README.md` for `0.27`."
  - "Step 17 — flip `docs/plans/phase4.md` M29 row to `done — shipped in 0.27`; flip this plan's `status: draft → done`; move file to `docs/plans/archive/phase4-milestone-29-2d-lighting.md`."
done-when:
  - "`cargo fmt --all && cargo test --workspace` passes on the M29 integration branch."
  - "`./scripts/smoke-examples.sh` passes including the new `TUNGSTEN_LIGHTING_FIXTURE=on` row and existing M26/M27/M28 rows."
  - "`cargo test -p tungsten-core lighting` covers `AmbientLight::default() == Vec3::ONE`, `LIGHT_CAP == 16`, `Light::point/Light::directional` constructors."
  - "`cargo test -p tungsten-render lighting` covers `LightUbo` byte size and `cull_to_cap` (top-16 by distance, `Directional` always retained)."
  - "`cargo test -p tungsten-render lit_sprite` covers `LIT_SPRITE_SHADER_NAME == \"lit_sprite\"` and pipeline-routing batch keying."
  - "`cargo test -p tungsten-render passes_order` is unchanged: `default_pass_order(...)` shape stays scene → post → optional SMAA → text → present (lit sprites do not introduce a new `PassDesc`)."
  - "`cargo test -p tungsten-core --test manifests` parses every workspace manifest, including the new `walk_*` normal/emissive siblings."
  - "`cargo test -p tungsten light_extract` covers cull-cap and directional retention."
  - "`WGPU_BACKEND=vulkan cargo run -p example-01-platformer` shows normal-mapped character lighting response when running near each colored point light; emissive eyes glow visibly when the platformer fixture enables a low-threshold `PostPass::Bloom(_)` or a manual run inserts equivalent bloom params."
  - "Empty light list + sprite without normal/emissive keeps the captured frame byte-identical to the M28 baseline. Verify via the existing image-diff regression on `example-02-sprite-stress`; the platformer `TUNGSTEN_LIGHTING_FIXTURE=off` capture is a showcase comparator, not the engine-wide byte-identity proof."
  - "Manual hot-reload smoke: editing `assets/shaders/lit_sprite.wgsl` (body-only) while example-01 runs updates lit shading within ~200 ms; validation failure logs `shader 'lit_sprite' validation failed: ...` and keeps the prior pipeline + frame."
  - "`docs/showcase/lighting_off_vs_on.png` committed; `docs/showcase/README.md` regen recipe references `TUNGSTEN_CAPTURE_FRAME`, `TUNGSTEN_CAPTURE_PATH`, and `TUNGSTEN_LIGHTING_FIXTURE`."
  - "`DECISIONS.md` contains `D-061`; `docs/DECISION_INDEX.md`, `docs/LLM_INDEX.md`, `AGENTS.md`, `DESIGN.md`, `CHANGELOG.md`, and `README.md` updated in the same change."
  - "`docs/plans/phase4.md` M29 row flipped to `status: done — shipped in 0.27`; this file flipped to `status: done` and moved to `docs/plans/archive/phase4-milestone-29-2d-lighting.md`."
---

## Context Digest

| Slice | Current state (after M28, on `0.26`) |
| --- | --- |
| Sprite path | [`SpritePipeline`](../../crates/tungsten-render/src/sprite.rs) draws all sprites through one camera (group 0) + texture (group 1) layout; M26 added per-batch material pipelines bound at group 2 reusing the same vertex/instance layout. `SpriteBatch { texture, filter, instances, material_id, uniform_overrides }` — no `lit` flag yet. |
| Sprite atlasing | [`load_sprites`](../../crates/tungsten/src/asset_loader.rs#L139) packs decoded RGBA into per-filter atlases via [`pack_shelf`](../../crates/tungsten-core/src/assets/atlas.rs); each `SpriteAsset` records its packed `UvRect` + `atlas: TextureHandle`. No sibling/aux texture concept exists today. |
| Manifest | [`SpriteEntry { path, filter }`](../../crates/tungsten-core/src/assets/manifest.rs#L61). M29 widens this to optional sibling paths, not a separate sprite id, so UV stays in lockstep with the albedo. |
| Frame order | `Scene [+ MSAA resolve] → N × PostPass (ping/pong) → [optional SMAA edge → blend → neighborhood → PresentSource] → text overlay → present blit → Swapchain` — implemented in [`default_pass_order`](../../crates/tungsten-render/src/passes/order.rs) and [`Renderer::render_frame_internal`](../../crates/tungsten-render/src/renderer.rs#L931). M29 stays inside the scene pass and adds no new `PassDesc`. |
| Camera | [`CameraState::visible_world_aabb(viewport_w, viewport_h)`](../../crates/tungsten-core/src/camera.rs#L189) is the cull AABB. M29 calls it from `extract_lights`. |
| Tween / override | M26 [`UniformOverrideBlock`](../../crates/tungsten-core/src/tween.rs) is unrelated to lighting; lit sprite controls are global (per-frame `LightUbo`), not per-entity. The override block stays available to user materials. |
| Hot reload | `D-053` matrix + `D-057` shader bodies + `D-058` material refresh + `D-059` SMAA stage shaders + `D-060` bloom stage shaders — M29 extends with three more shader ids and two new sprite-sibling file rows. |
| 04_shader_playground env pins | `TUNGSTEN_POST_STACK_FIXTURE`, `TUNGSTEN_POST_AA_FIXTURE`, `TUNGSTEN_BLOOM_FIXTURE`. M29 uses `TUNGSTEN_LIGHTING_FIXTURE` over `example-01-platformer`, not the playground (the lit acceptance happens in the platformer). |
| Screenshot path | Already routes through final post / present source; lit fragments live inside `SceneColor` so no readback change is needed. |

### Relevant `D-0xx` ids

- `D-007`, `D-016`, `D-018` — core/render seam invariants. `Light` lives in core; UBO + bind group live render-side.
- `D-009`, `D-017`, `D-035` — manifest is source of truth; sibling files extend `SpriteEntry`, no new asset section. ID-based references only (existing `SpriteAsset.path` discipline).
- `D-023` narrowed by `D-057` — shaders are manifest-tracked; `lit_sprite.wgsl` follows the same body-edit reload contract as the stock shaders.
- `D-053` — hot-reload matrix; M29 extends with `lit_sprite`/`emissive_mask`/`rim_light` shader rows + sprite-sibling file rows.
- `D-054` — closed-enum precedent. `LightKind` is closed: `Point` + `Directional`. Adding spotlight is a follow-up milestone.
- `D-058`/`D-060` — `PostStack` + bloom; emissive bright pixels feed bloom thresholding without an extra wire-up.
- `D-042` — `Sprite` extract is opt-in via `Visibility`; `Light` extract follows the same explicit pattern (no `Visibility` gate — lights are always-on while present).

### Lit-sprite path overview

```
extract_sprites (lit batch when SpriteAsset.lit_atlas.is_some())
extract_lights (camera AABB cull, top-16, ambient resource)

scene pass:
  Renderer.update_lights(&light_ubo)        // queue.write_buffer + group2 stays bound
  for batch in batches:
    if batch.lit:
      pipeline = lit_sprite                 // group0=camera, group1=lit textures, group2=light ubo
    else:
      pipeline = sprite                     // unlit / material path unchanged
```

Lit fragments in `SceneColor` flow through every later stage byte-equally to unlit fragments. Bloom (`D-060`) thresholds emissive headroom; SMAA (`D-059`) sees the same gamma values.

### Light UBO (`std140`-friendly, 544 bytes)

| Offset | Bytes | Field | Notes |
| --- | --- | --- | --- |
| 0     | 16    | `lights[0].position_radius`        | `vec4<f32>(pos.x, pos.y, radius, _)` for `Point`; `(dir.x, dir.y, 0, _)` for `Directional` |
| 16    | 16    | `lights[0].color_intensity`        | `vec4<f32>(rgb * intensity, kind_tag)` — `kind_tag = 0` Point, `1` Directional |
| ...   | ...   | `lights[1..16]`                    | 16 entries × 32 bytes = 512 bytes total |
| 512   | 16    | `count + _pad`                     | `vec4<u32>(count, 0, 0, 0)` keeps `std140` alignment for the trailing `vec4<f32>` |
| 528   | 16    | `ambient`                          | `vec4<f32>(rgb, 1.0)` |

Total: **544 bytes**. WGSL should declare the same explicit surface (`lights: array<GpuLight, 16>; count_pad: vec4<u32>; ambient: vec4<f32>;`) rather than relying on implicit padding after a lone `u32`. Pack with `bytemuck::Pod` so `to_bytes()` is a single copy.

```rust
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Default)]
pub struct GpuLight {
    pub position_radius: [f32; 4], // (x, y, radius_or_0, _pad)
    pub color_intensity: [f32; 4], // (r*int, g*int, b*int, kind_tag)
}

#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct LightUbo {
    pub lights: [GpuLight; 16],
    pub count_pad: [u32; 4],
    pub ambient: [f32; 4],
}
```

### Light cull (`extract_lights`)

```text
let aabb = camera.visible_world_aabb(viewport_w, viewport_h);
let mut buf: Vec<(f32, GpuLight)> = world.query2::<Transform, Light>()
    .map(|(_, t, l)| (distance_to_aabb_sq(t.position, aabb), pack(l, t)))
    .collect();
buf.sort_by(|a, b| {
    // Directional always wins over distance; otherwise nearest first.
    let a_dir = is_directional(a.1);
    let b_dir = is_directional(b.1);
    b_dir.cmp(&a_dir).then(a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal))
});
buf.truncate(LIGHT_CAP);
pack_lights(&buf.iter().map(|(_, g)| *g).collect::<Vec<_>>(), ambient)
```

`distance_to_aabb_sq` is the squared distance from the light's world position to the AABB rectangle (0 inside, monotonic outside). Directional lights sort to the head and never get culled until `LIGHT_CAP` is exceeded by directionals alone (then keep first 16 in registration order).

### Why a parallel atlas instead of separate sprite ids

Albedo / normal / emissive must share UV; a parallel atlas keyed by the same `id` reuses the same `pack_shelf` placement output. This avoids a second packer call, keeps the seam (`SpriteAsset.uv`) untouched, and means `reload_sprite` only needs to additionally re-decode/upload the aux pixels for the same packed cell. A separate sprite-id approach would force the extract layer to look up two UV rects and split batches — strictly more code and more edge cases for no benefit.

### Lit sprite shader sketch (`lit_sprite.wgsl`)

```wgsl
// Group 0: camera (matches sprite.wgsl)
// Group 1: lit textures + sampler
// @binding(0) albedo_tex
// @binding(1) normal_tex
// @binding(2) emissive_tex
// @binding(3) sampler
// Group 2: light UBO

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let albedo = textureSample(albedo_tex, samp, in.tex_coord) * in.tint;
    let n_ts = textureSample(normal_tex, samp, in.tex_coord).xyz * 2.0 - 1.0;
    // 2D shading uses world-axis normal (z = 1 means flat, +x = right-facing).
    var rgb = albedo.rgb * lighting.ambient.rgb;
    for (var i: u32 = 0u; i < lighting.count_pad.x; i = i + 1u) {
        let l = lighting.lights[i];
        if (l.color_intensity.w == 1.0) {
            // Directional: l.position_radius.xy = direction.
            let l_dir = normalize(vec3<f32>(l.position_radius.xy, 1.0));
            let n_dot_l = max(0.0, dot(n_ts, l_dir));
            rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l;
        } else {
            let to_light = vec3<f32>(l.position_radius.xy - in.world_pos, 0.0);
            let dist = length(to_light.xy);
            let radius = max(l.position_radius.z, 0.0001);
            let attenuation = clamp(1.0 - dist / radius, 0.0, 1.0);
            // Squared falloff knob lives in `radius` for M29; refine in M33.
            let l_dir = normalize(vec3<f32>(to_light.xy, 1.0));
            let n_dot_l = max(0.0, dot(n_ts, l_dir));
            rgb = rgb + albedo.rgb * l.color_intensity.rgb * n_dot_l * attenuation * attenuation;
        }
    }
    let emissive = textureSample(emissive_tex, samp, in.tex_coord).rgb;
    rgb = rgb + emissive;
    return vec4<f32>(rgb, albedo.a);
}
```

The lit vertex shader uses the same instance layout as `sprite.wgsl` but adds a `world_pos: vec2<f32>` varying for point-light vectors. Rim term (helper `rim_light.wgsl`) ships as an unused-by-default helper so M33 / showcase can compose it; do not wire it to `falloff` in M29 unless a real tuning surface lands with it.

<assumptions>
- One `LightKind` enum, two variants. `Point { radius, falloff }` carries radius (world units, drives attenuation start) and falloff (reserved for M33; ignored by the M29 shader). `Directional { angle }` is a 2D direction (radians from +x), packed as `(cos, sin, 0)` into `position_radius.xy`. Spotlight / area come later.
- `LIGHT_CAP = 16` is `pub const`. Raising it requires a UBO re-pack (`std140` rules) and a shader literal change; the WGSL loop is bounded by the `count` field, not the array length, so under-cap frames cost only the writes.
- Tangent-space normals only. The 2D shader treats `(n_ts.xy, n_ts.z)` as world-axis normals because there is no per-fragment tangent frame; sprite normals are authored as if the sprite plane is the world plane. `(0.5, 0.5, 1.0)` in 8-bit is the flat default that matches an unlit albedo when ambient = white.
- Aux atlas pages share filter and dimensions with the albedo page. Decode failure (e.g. dimensions mismatch between albedo/normal) is logged `ERROR` and the sprite skips lit registration (last-known-good = unlit). Normal maps are uploaded as linear `Rgba8Unorm`, not sRGB. Emissive masks are expanded into RGB contribution data before upload so alpha-only masks render consistently. Manifest authoring discipline is to keep the three siblings the same size, which is the natural authoring workflow anyway.
- `extract_lights` runs every frame regardless of light count. With zero lights the UBO write still happens but `count = 0` and the lit pipeline shading collapses to ambient + emissive.
- `Renderer::update_lights` writes one UBO per frame; the bind group is built once at startup against that UBO and stays bound for every lit batch. Resize does not invalidate it.
- A sprite carrying both `material_id = Some(_)` and `lit_atlas = Some(_)` is allowed but lit wins in M29 — the material UBO is not bound, and a `WARN` is logged. Removing the warning requires a future merged lit+material pipeline (M33 or Phase 5 work).
- Bloom integration is implicit: emissive multiplied by `1.0` lives inside `[0, 1]` LDR range, so the M28 threshold knob (default 1.0) needs to be lowered for visible bloom from emissive eyes; the platformer lighting fixture can insert a low-threshold `PostPass::Bloom(_)` when capture/smoke wants the glow visible.
- `extract_sprites_default` becomes the default batch-keying authority for `lit`. User overrides via `App::set_extract_sprites` may bypass the lit path; that is intentional (D-018: extract is the user's seam). The M29 platformer acceptance is a custom-extract exception and must update `examples/01_platformer/src/extract.rs` explicitly.
- No new dependency. `glam::Vec2`/`Vec3`, `bytemuck::Pod`, `wgpu` already cover the math + GPU surface (`D-015` rule 3 / rule 1 already satisfied for those crates).
- Empty light list and sprites without aux atlases keep the captured frame byte-identical to M28: `LightUbo` writes `count = 0` plus ambient every frame, but no lit pipeline draws read it; the unlit pipeline is unchanged.
</assumptions>

---

## Step 1 — `Light`, `LightKind`, `AmbientLight`, `LIGHT_CAP`

| File | Edit |
| --- | --- |
| [`crates/tungsten-core/src/components.rs`](../../crates/tungsten-core/src/components.rs) | Add `pub struct Light { pub kind: LightKind, pub color: Vec3, pub intensity: f32 }` and closed `pub enum LightKind { Point { radius: f32, falloff: f32 }, Directional { angle: f32 } }`. Provide `Light::point(color, radius)` / `Light::directional(color, angle)` constructors with `intensity = 1.0`, `falloff = 1.0`. Derive `Debug, Clone, Copy, PartialEq`. |
| [`crates/tungsten-core/src/lighting.rs`](../../crates/tungsten-core/src/lighting.rs) | New module. `pub const LIGHT_CAP: usize = 16;` and `#[derive(Debug, Clone, Copy, PartialEq)] pub struct AmbientLight(pub Vec3); impl Default for AmbientLight { fn default() -> Self { Self(Vec3::ONE) } }`. |
| [`crates/tungsten-core/src/lib.rs`](../../crates/tungsten-core/src/lib.rs) | `pub mod lighting;` + `pub use lighting::{AmbientLight, LIGHT_CAP};` and extend the existing `pub use components::{...}` block with `Light, LightKind`. |
| [`crates/tungsten-core/src/tests/components.rs`](../../crates/tungsten-core/src/tests/components.rs) | New cases: `light_point_constructor_intensity_one`, `light_directional_constructor_angle`. |
| [`crates/tungsten-core/src/tests/lighting.rs`](../../crates/tungsten-core/src/tests/lighting.rs) | New file: `ambient_default_is_one`, `light_cap_is_sixteen`. |

Verify: `rg -n "AmbientLight|LIGHT_CAP|LightKind" crates/tungsten-core/src && cargo test -p tungsten-core lighting`.

## Step 2 — Manifest: optional `normal_map` + `emissive_mask` siblings

| File | Edit |
| --- | --- |
| [`crates/tungsten-core/src/assets/manifest.rs`](../../crates/tungsten-core/src/assets/manifest.rs) | Extend `SpriteEntry` with `#[serde(default)] pub normal_map: Option<String>` and `#[serde(default)] pub emissive_mask: Option<String>`. Extend `ResolvedSprite` with `pub normal_path: Option<PathBuf>` and `pub emissive_path: Option<PathBuf>`. In `ResolvedManifest::load`, resolve each optional field against `base_dir` and assert the file exists with a new `MissingNormalMapFile { id, path }` / `MissingEmissiveMaskFile { id, path }` `ManifestError` variant. |
| [`crates/tungsten-core/tests/manifests.rs`](../../crates/tungsten-core/tests/manifests.rs) | No code change — already iterates every workspace `manifest.json` and calls `ResolvedManifest::load`. The new platformer fixture covers the new fields. |

Verify: `cargo test -p tungsten-core --test manifests` after Step 14 lands the new platformer entries.

## Step 3 — `SpriteAsset` aux atlas slots

| File | Edit |
| --- | --- |
| [`crates/tungsten-core/src/assets/registry.rs`](../../crates/tungsten-core/src/assets/registry.rs) | Add `pub normal_path: Option<PathBuf>`, `pub emissive_path: Option<PathBuf>`, and `pub lit_atlas: Option<TextureHandle>` to `SpriteAsset`. Extend `register_sprite(...)` (already `#[allow(clippy::too_many_arguments)]`) with optional sibling paths + lit atlas trailing args. Insert sibling paths into `path_to_sprite_id` so `App::process_hot_reload` can route normal/emissive PNG edits through `reload_sprite`. Provide `update_sprite_lit_atlas(id, lit_atlas: Option<TextureHandle>)` for atlas rebuilds. |

Verify: `rg -n "register_sprite|normal_path|emissive_path|lit_atlas" crates/tungsten-core/src crates/tungsten/src` covers all call sites.

## Step 4 — `LightUbo` + `LightingResources`

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/lighting.rs`](../../crates/tungsten-render/src/lighting.rs) | New module. Define `pub const LIT_LIGHT_CAP: usize = 16;` (asserts equal to `tungsten_core::LIGHT_CAP`). `GpuLight` + `LightUbo` POD structs as above. `pack_lights(slice: &[GpuLight], ambient: glam::Vec3) -> LightUbo` zero-fills the unused tail. `cull_to_cap(camera_aabb: (Vec2, Vec2), entries: &[(Vec2, Light)]) -> Vec<GpuLight>` implements the directional-first sort + truncate. `LightingResources::new(device)` allocates a 544-byte `Buffer` with `UNIFORM | COPY_DST`, builds `lighting_bgl` (one uniform binding) and the bind group. Provide `LightingResources::write(queue, ubo: &LightUbo)`. |
| [`crates/tungsten-render/src/tests/lighting.rs`](../../crates/tungsten-render/src/tests/lighting.rs) | New: `light_ubo_byte_size_is_544`, `cull_to_cap_keeps_directional_first`, `cull_to_cap_truncates_to_sixteen`, `pack_lights_zeros_unused_tail`. |

Verify: `cargo test -p tungsten-render lighting` and `rg -n "LIT_LIGHT_CAP|pack_lights|cull_to_cap" crates/tungsten-render/src`.

## Step 5 — Lit shader vendoring

| File | Edit |
| --- | --- |
| [`assets/shaders/lit_sprite.wgsl`](../../assets/shaders/lit_sprite.wgsl) | New WGSL: groups 0/1/2 as in the sketch above, vertex shader uses the same instance layout as `sprite.wgsl` and adds a `world_pos` varying, fragment shader does ambient + N-light loop bounded by `count` + emissive add. Header comment cites the references (no copied code, no MIT attribution). |
| [`crates/tungsten-render/src/shaders/stock/emissive_mask.wgsl`](../../crates/tungsten-render/src/shaders/stock/emissive_mask.wgsl) | New helper. Standalone fragment-only WGSL exposing `fn emissive_contribution(uv: vec2<f32>, mask_tex: texture_2d<f32>, samp: sampler, strength: f32) -> vec3<f32>`; not bound into a pipeline by itself in M29 (manifest-tracked so users can edit it for material composition). |
| [`crates/tungsten-render/src/shaders/stock/rim_light.wgsl`](../../crates/tungsten-render/src/shaders/stock/rim_light.wgsl) | New helper. `fn rim_term(n: vec3<f32>, view_dir: vec3<f32>, color: vec3<f32>, power: f32) -> vec3<f32>`. Same standalone status. |
| [`assets/shaders/stock/emissive_mask.wgsl`](../../assets/shaders/stock/emissive_mask.wgsl) | Mirror. |
| [`assets/shaders/stock/rim_light.wgsl`](../../assets/shaders/stock/rim_light.wgsl) | Mirror. |
| [`assets/manifest.json`](../../assets/manifest.json) | Append three `shaders` entries: `lit_sprite`, `emissive_mask`, `rim_light`. |

Verify: `rg -n "lit_sprite|emissive_mask|rim_light" assets/manifest.json crates/tungsten-render/src/shaders`.

## Step 6 — `LitSpritePipeline`

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/lit_sprite.rs`](../../crates/tungsten-render/src/lit_sprite.rs) | New module mirroring `material.rs`'s shape: `LitSpritePipeline { pipeline, pipeline_layout, lit_texture_bgl }` plus `build_lit_sprite_pipeline(device, module, camera_bgl, lit_texture_bgl, lighting_bgl, surface_format, sample_count, depth_write) -> RenderPipeline`. Reuse `SpritePipeline::vertex_layouts()`. `rebuild_with_shader(device, module, surface_format, sample_count, depth_write)` keeps last-known-good on caller-side validation failure. |
| [`crates/tungsten-render/src/lib.rs`](../../crates/tungsten-render/src/lib.rs) | `pub mod lighting; pub mod lit_sprite;` + re-exports `LitSpritePipeline`, `LightUbo`, `GpuLight`, `LightingResources`, `LIT_LIGHT_CAP`, `LIT_SPRITE_SHADER_NAME`. |

Verify: `cargo build -p tungsten-render`.

## Step 7 — `SpriteBatch.lit` + lit texture pool

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/sprite.rs`](../../crates/tungsten-render/src/sprite.rs) | `SpriteBatch` gains `pub lit: bool` (default `false`). `SpritePipeline` gains a parallel `lit_textures: HashMap<TextureHandle, GpuLitTextures>` field where `struct GpuLitTextures { albedo_view, normal_view, emissive_view, bind_group, filter }`. New methods `lit_texture_bind_group_layout(&self)`, `upload_lit_texture(device, queue, handle, albedo_rgba, normal_rgba, emissive_rgba, w, h, filter)`, `write_subtexture_lit(...)`, `drop_lit_texture(handle)`. Lit texture bind group binds the three views + one shared sampler at group 1 (slots 0/1/2/3). Use sRGB only for the albedo texture; normal/emissive use linear `Rgba8Unorm`. |
| same `draw` | Keep one stable draw walk over `batches`; branch per batch to built-in, material, or lit pipeline. Bind group 2 is the material UBO for material batches and the lighting UBO for lit batches. Do not partition the full slice by lit/unlit because that changes cross-pipeline painter order. |
| [`crates/tungsten-render/src/tests/sprite.rs`](../../crates/tungsten-render/src/tests/sprite.rs) | Add a `sprite_batch_default_lit_false` regression. |

Verify: `cargo test -p tungsten-render sprite::tests`.

## Step 8 — Renderer wiring + hot-reload routes

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/renderer.rs`](../../crates/tungsten-render/src/renderer.rs) `Renderer` fields | Add `lit_sprite: LitSpritePipeline`, `lighting: LightingResources`, `lit_sprite_shader_id: ShaderAssetId`. |
| same `Renderer::new` | After bloom seeding (ids 4..=7), pre-seed `lit_sprite` (id 8), `emissive_mask` (id 9), `rim_light` (id 10) into `ShaderModuleCache` from compile-time `include_str!`. Build `LightingResources::new(&device)` + `LitSpritePipeline::new(&device, format, sample_count, depth_attached, sprite_pipeline.camera_bind_group_layout(), sprite_pipeline.lit_texture_bind_group_layout(), &lighting.bind_group_layout)`. Insert all three new ids into `shader_ids`; bump `next_shader_id = 11`. Keep this seeding logic testable through a device-free helper (for example `well_known_shader_ids()`) rather than a `Renderer::new` unit test, because `cargo test --workspace` must not require GPU/display. |
| same `upload_shader` and `reload_shader` | Add a branch alongside the bloom branch: `if name == LIT_SPRITE_SHADER_NAME { self.lit_sprite.rebuild_with_shader(&self.device, &module, self.surface_config.format, self.sample_count, matches!(self.depth_sort, DepthSortMode::GpuDepth)); }`. The `emissive_mask` / `rim_light` helpers are not bound to a pipeline directly; they pass through the cache only. Material rebuild guard expands to skip `LIT_SPRITE_SHADER_NAME | EMISSIVE_MASK_SHADER_NAME | RIM_LIGHT_SHADER_NAME` so material pipelines do not rebuild on a lit-shader edit. |
| same | Add `pub fn upload_lit_texture(&mut self, handle: TextureHandle, albedo: &[u8], normal: &[u8], emissive: &[u8], w: u32, h: u32, filter: FilterMode)` delegating to `SpritePipeline::upload_lit_texture`. |
| same | Add `pub fn update_lights(&mut self, ubo: &LightUbo) { self.lighting.write(&self.queue, ubo); }`. |
| same `record_main_draws` | Pass `lighting.bind_group` into `SpritePipeline::draw`; it binds group 2 only for lit batches. The unlit branch uses the existing two-group pipeline, and the material branch continues to bind the material UBO at group 2. |
| [`crates/tungsten-render/src/tests/lit_sprite.rs`](../../crates/tungsten-render/src/tests/lit_sprite.rs) | New file. `lit_sprite_shader_name_constant`, `well_known_lit_shader_ids_are_after_bloom`, and any pure batch-key helper coverage. No test should construct `Renderer` directly. |
| [`crates/tungsten-render/src/tests/passes_order.rs`](../../crates/tungsten-render/src/tests/passes_order.rs) | Confirm `default_pass_order(...)` shape unchanged when lit sprites are present (no new `PassDesc`). |

Verify: `cargo test -p tungsten-render lit_sprite passes_order`.

## Step 9 — `record_main_draws` lit routing

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/renderer.rs`](../../crates/tungsten-render/src/renderer.rs) `record_main_draws` | Thread `&self.lighting.bind_group` and `&self.lit_sprite` into the sprite draw path and draw batches in the exact order extract emitted them. Adjacent batches may switch between built-in, material, and lit pipelines; preserving order is more important than reducing pipeline binds in M29. |

Verify: `cargo test -p tungsten` and `WGPU_BACKEND=vulkan TUNGSTEN_LIGHTING_FIXTURE=on TUNGSTEN_SMOKE_FRAMES=3 cargo run -p example-01-platformer`.

## Step 10 — `load_sprites` aux pages

| File | Edit |
| --- | --- |
| [`crates/tungsten/src/asset_loader.rs`](../../crates/tungsten/src/asset_loader.rs) `load_sprites` | Decode albedo plus optional `normal_path` / `emissive_path` into one `Decoded` record per sprite. Run `pack_shelf` once for the full filter-class albedo list, then use each returned `PackedSprite { x, y, w, h, page }` to copy into three page canvases: albedo, flat-normal default, and emissive default. Normal siblings must match albedo dimensions or the sprite stays unlit. Emissive siblings may be authored as alpha/luma masks; expand alpha/luma into RGB contribution bytes before upload. Upload the regular albedo page with `renderer.upload_texture(handle, ...)` and the optional lit bundle for the same page handle with `renderer.upload_lit_texture(handle, albedo_canvas[p], normal_canvas[p], emissive_canvas[p], ...)`. Sprites without a valid normal sibling keep `SpriteAsset.lit_atlas = None` so extract stays unlit even though placeholder texels may exist on a mixed page. |
| same | Atlas-handle bookkeeping: no separate aux handle vectors are needed in M29. `lit_textures` is a sibling renderer pool keyed by the existing albedo page handle, and `drop_texture(handle)` must also drop `lit_textures[handle]`. `AtlasRegistry::packed` remains the source of packed-cell geometry for albedo and aux writes. |
| same `reload_sprite` | Because `path_to_sprite_id` now maps albedo, normal, and emissive paths, any sibling PNG edit calls `reload_sprite(id, changed_path, ...)`. The function should consult the live `SpriteAsset` to decode all known siblings for that id, then call `renderer.write_subtexture(...)` for albedo and `renderer.write_subtexture_lit(...)` for the lit bundle. When the sprite is in-place shrink, all three siblings shrink/pad in lockstep. |
| same `reload_manifest` | When an existing sprite's `normal_map` / `emissive_mask` is missing in the new manifest, log `WARN` and keep the last-known-good aux pages (mirrors the sprite-removal warn-only behavior). New sprite additions with siblings trigger an aux atlas rebuild via the same gain-flag path as the albedo. |

Verify: `cargo test -p tungsten asset_loader` (extend the existing tests under `crates/tungsten/src/tests/asset_loader.rs` with an aux-aware fixture).

## Step 11 — `BatchKey.lit` + sprite extract routing

| File | Edit |
| --- | --- |
| [`crates/tungsten/src/sprite_extract.rs`](../../crates/tungsten/src/sprite_extract.rs) | `BatchKey = (u32, FilterMode, Option<MaterialAssetId>, Option<u64>, bool)` — append `lit`. When `asset.lit_atlas.is_some()`: `lit = true`; force `material_id = None` for batch keying (lit wins) and log `WARN` when the original sprite carried `material_id = Some(_)`. Do not add a static warning cache; it would violate the project rule against global mutable state. Set `SpriteBatch.lit = true` on the batch struct. |
| [`crates/tungsten/src/tests/sprite_extract.rs`](../../crates/tungsten/src/tests/sprite_extract.rs) | New cases: `lit_batch_routed_when_lit_atlas_present`, `lit_collides_with_material_warns_and_keeps_lit`, `unlit_path_byte_identical_with_no_aux`. |

Verify: `cargo test -p tungsten sprite_extract`.

## Step 12 — `extract_lights`

| File | Edit |
| --- | --- |
| [`crates/tungsten/src/light_extract.rs`](../../crates/tungsten/src/light_extract.rs) | New module. `pub fn extract_lights(world: &World, camera: &CameraState, viewport_w: f32, viewport_h: f32) -> LightUbo`. Implementation: query `(Transform, Light)` via `world.query2::<Transform, Light>()`; pack each into a `(distance_to_camera_aabb_sq: f32, GpuLight, is_directional: bool)` triple via `pack_one_light`; sort directional-first then nearest-first; truncate to `LIGHT_CAP`; call `pack_lights`. Read optional `AmbientLight` resource (defaults to `Vec3::ONE`). With no lights, return `count = 0` and the ambient default, not a fully zeroed UBO. |
| [`crates/tungsten/src/lib.rs`](../../crates/tungsten/src/lib.rs) | `pub mod light_extract;` + `pub use light_extract::extract_lights;`. |
| [`crates/tungsten/src/tests/light_extract.rs`](../../crates/tungsten/src/tests/light_extract.rs) | New: `extract_lights_returns_ambient_when_no_lights`, `extract_lights_caps_at_sixteen`, `extract_lights_keeps_directional_under_pressure`. |

Verify: `cargo test -p tungsten light_extract`.

## Step 13 — App wiring

| File | Edit |
| --- | --- |
| [`crates/tungsten/src/app.rs`](../../crates/tungsten/src/app.rs) `stage_extract` | After sprite extract, read `WindowSize` + `CameraState` from the world and build the light UBO: `let light_ubo = light_extract::extract_lights(&self.world, &camera, vw, vh);`. Stash it on `FrameExtract` (new field `light_ubo: LightUbo`). |
| same `stage_render` | Before `render_frame_full*`, call `renderer.update_lights(&extract.light_ubo)`. |
| `FrameExtract` struct | Add `pub light_ubo: tungsten_render::LightUbo`. |

Verify: `cargo test -p tungsten` and `WGPU_BACKEND=vulkan cargo run -p example-01-platformer` (manual: lit response moves with the player).

## Step 14 — Platformer normal/emissive assets + lights

| File | Edit |
| --- | --- |
| [`assets/sprites/walk_0_n.png`](../../assets/sprites/walk_0_n.png) | New tangent-space normal map; flat baseline `(128, 128, 255)` over the silhouette, beveled chest/face for shading variation. |
| `walk_1_n.png .. walk_3_n.png` | Same authoring as `walk_0_n.png`. |
| `walk_0_e.png .. walk_3_e.png` | Single-eye emissive masks (white over the eye region, black elsewhere). |
| [`assets/manifest.json`](../../assets/manifest.json) | Each root `walk_N` entry gains `"normal_map": "sprites/walk_N_n.png"` and `"emissive_mask": "sprites/walk_N_e.png"`. Do not add duplicate `walk_N` ids to the platformer-local manifest. |
| [`examples/01_platformer/src/setup.rs`](../../examples/01_platformer/src/setup.rs) | After the `Player` spawn block, spawn three light entities when `TUNGSTEN_LIGHTING_FIXTURE != off`: warm point (color `(1.0, 0.85, 0.6)`, radius `8.0 * TILE`), cool point (color `(0.5, 0.7, 1.0)`, radius `8.0 * TILE`), and a directional light from upper-right (`Light::directional(Vec3::splat(0.9), -std::f32::consts::FRAC_PI_4)`). Insert `AmbientLight(Vec3::splat(0.35))` when on and `AmbientLight(Vec3::ONE)` when off. Also insert a small example resource recording the fixture mode so `extract.rs` can decide whether to set `SpriteBatch.lit`. |
| [`examples/01_platformer/src/systems.rs`](../../examples/01_platformer/src/systems.rs) | New `orbit_lights_system(world: &mut World)` that advances the warm + cool point lights along sine/cosine arcs centered on the camera target each frame. |
| [`examples/01_platformer/src/setup.rs`](../../examples/01_platformer/src/setup.rs) | Insert `("orbit_lights_system", orbit_lights_system)` into `RUNTIME_SYSTEM_ORDER` before `camera_update_system`. |
| [`examples/01_platformer/src/extract.rs`](../../examples/01_platformer/src/extract.rs) | The platformer uses a custom extract, so default `sprite_extract` changes do not light the player. When the lighting fixture resource is on and a `walk_*` asset has `lit_atlas`, set `batch.lit = true` and clear the existing `PlayerMaterial`/`UniformOverrideBlock` material fields for that batch. When off, keep the current material path so damage-flash behavior and baseline captures remain comparable. |

Verify: `WGPU_BACKEND=vulkan TUNGSTEN_LIGHTING_FIXTURE=on cargo run -p example-01-platformer` shows orbiting colored lights and emissive eyes.

## Step 15 — Smoke + capture

| File | Edit |
| --- | --- |
| [`scripts/smoke-examples.sh`](../../scripts/smoke-examples.sh) | After the M27 `post_aa_*` block and the M28 `bloom_*` block, append a `lighting_pass=()`/`lighting_fail=()` block running `example-01-platformer` once with `TUNGSTEN_LIGHTING_FIXTURE=on TUNGSTEN_SMOKE_FRAMES=$SMOKE_FRAMES`. Echo the row count alongside the existing matrices. |
| [`docs/showcase/lighting_off_vs_on.png`](../showcase/lighting_off_vs_on.png) | New 2-up still: left captured with `TUNGSTEN_LIGHTING_FIXTURE=off` (custom extract keeps player unlit), right with `TUNGSTEN_LIGHTING_FIXTURE=on` plus the platformer bloom fixture. Use the existing `TUNGSTEN_CAPTURE_FRAME` / `TUNGSTEN_CAPTURE_PATH` env path. |
| [`docs/showcase/README.md`](../showcase/README.md) | Append M29 section mirroring the M27/M28 sections: regen recipe with the env triple + `convert _lighting_off.png _lighting_on.png +append lighting_off_vs_on.png`. |

Verify: `bash -n scripts/smoke-examples.sh && WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh`.

## Step 16 — Decision entry + doc sync

| File | Edit |
| --- | --- |
| [`DECISIONS.md`](../../DECISIONS.md) | Append `## D-061 — M29 2D forward lighting`. **Decision:** `Light { kind, color, intensity }` + `LightKind::{ Point, Directional }` live in `tungsten-core`; `AmbientLight(Vec3)` is a world resource defaulting to `Vec3::ONE`. Render-side `LightingResources` owns one 544-byte `LightUbo` (16 lights + count pad + ambient) bound at group 2 of a new `LitSpritePipeline`. Sprites carrying a `normal_map` / `emissive_mask` sibling in the manifest pack into parallel atlas canvases keyed by the same packed rect; `extract_sprites_default` flips `SpriteBatch.lit` on the resolved `SpriteAsset.lit_atlas.is_some()` axis. Light extract runs every frame, culls by camera-AABB distance, keeps `Directional` first, caps at `LIGHT_CAP = 16`. **Why:** sibling files share UV with the albedo for free (no second packer call, no UV plumbing), keeping the `D-009` ID-based seam intact. The lit pipeline is a sibling rather than a `material_id` extension because lit shading is a renderer-owned global (per-frame UBO, not per-entity), so squeezing it into the per-material `UniformOverrideBlock` would force every material author into the lighting model. Forward shading with 16-light cap fits 2D scenes (and clamps UBO size at 544 bytes, comfortably under wgpu's 16 KiB minimum spec). Directional lights always retained because losing them under heavy point-light pressure would dim the entire scene noticeably, while losing the farthest point light is locally invisible. **Consequences:** `SpriteAsset` grows optional sibling paths plus a lit-atlas marker; `SpriteEntry` grows two optional sibling paths; `extract_sprites_default` grows a `lit` keying axis (still byte-equal output when no sprite carries aux atlases). New shader id triple (`lit_sprite`, `emissive_mask`, `rim_light`) extends `D-053`'s body-edit reload table. Lit + material composition is intentionally out-of-scope: a sprite carrying both warns and uses lit. No new dependency. Narrows neither `D-058` nor `D-060`; M28 bloom thresholding picks up emissive headroom without further changes. |
| [`docs/DECISION_INDEX.md`](../DECISION_INDEX.md) | Add Assets / Rendering row for `D-061`: `M29 2D lighting: Light/LightKind in core, LightUbo (16 lights + ambient) on render, lit sprites pick parallel normal/emissive atlas pages keyed by SpriteAsset.lit_atlas; lit + material warns and lit wins.` |
| [`docs/LLM_INDEX.md`](../LLM_INDEX.md) | Add a Lighting (M29) row pointing to `crates/tungsten-core/src/components.rs` + `crates/tungsten-core/src/lighting.rs` + `crates/tungsten-render/src/lighting.rs` + `crates/tungsten-render/src/lit_sprite.rs` + `crates/tungsten/src/light_extract.rs`. |
| [`AGENTS.md`](../../AGENTS.md) | Under the materials/SMAA/bloom shader bullets, add: lit sprite shader (`lit_sprite`) + `emissive_mask` / `rim_light` helpers follow the stock-shader pattern; manifest-tracked, body-edit hot-reload via `Renderer::reload_shader` + `LitSpritePipeline::rebuild_with_shader` (helpers are validated only — no pipeline behind them). Asset-rules table grows two optional sibling fields under sprites: `normal_map` and `emissive_mask`. Frame-order bullet stays the same — lit sprites are a sibling pipeline inside the scene pass. |
| [`DESIGN.md`](../../DESIGN.md) | Status block: note `Light` / `AmbientLight` and `LitSpritePipeline` are live in `0.27`. Hot-reload matrix: gain three shader rows (`lit_sprite`, `emissive_mask`, `rim_light`) plus two sprite-sibling-file rows (`sprites.<id>.normal_map`, `sprites.<id>.emissive_mask`). Frame-order paragraph: note that lit sprites are a sibling draw path inside the scene pass and the renderer preserves extracted batch order while switching between built-in, material, and lit pipelines. |
| [`CHANGELOG.md`](../../CHANGELOG.md) | New entry under `0.27`: `M29 — 2D forward normal-mapped lighting (Light/LightKind components, AmbientLight resource, 544-byte LightUbo with 16-light cap, lit_sprite pipeline + emissive_mask/rim_light helpers, manifest-tracked normal_map/emissive_mask sibling files for sprites, parallel atlas pages keyed by the existing SpriteAsset.uv rect, sprite_extract lit-batch keying, light_extract camera-AABB cull). No new runtime deps. See D-061.` |
| [`README.md`](../../README.md) | Status block: mark M29 shipped. |
| [`docs/plans/phase4.md`](phase4.md) | Flip M29 row to `done — shipped in 0.27`; reference the archived plan path. |
| [`docs/plans/phase4-milestone-29-2d-lighting.md`](phase4-milestone-29-2d-lighting.md) | Flip front-matter `status: draft` → `status: done`; move file to `docs/plans/archive/phase4-milestone-29-2d-lighting.md` on ship. |

## Risks / Unknowns

- **Aux atlas reuse vs. new pool.** Treating the lit pool as overlay state on the same `TextureHandle` (Step 7/10) keeps lookups cheap but means an unlit batch sampling that handle reads only the albedo view while a lit batch samples three views. The bind-group-per-batch structure already permits this; risk is hot-reload lifecycle (a `drop_texture` while a `lit_textures` entry is live). Mitigation: `drop_texture` also drops the matching `lit_textures` entry.
- **Aux atlas for partially-lit pages.** If page `p` mixes lit and unlit sprites, the lit pool entry covers the entire page even though the unlit sprites' aux pixels are placeholders. Cost is a few KiB of mostly-blank texture per page; acceptable for desktop. If memory pressure shows in M30 telemetry, split lit/unlit packs into separate pages (small post-M29 follow-up).
- **Tangent-space approximation.** The 2D shader treats `(n_ts.xy, n_ts.z)` as world-axis; sprite rotation is *not* rotated into the normal sample. For M29 the platformer character does not rotate, so this is invisible. Rotated lit sprites are a documented M33 follow-up.
- **Light count ABI.** Bumping `LIGHT_CAP` requires a UBO size change + WGSL literal change + `std140` re-pack. The plan keeps it at 16 to avoid dragging this into M29; deferred lighting in Phase 5 will replace this entirely.
- **Capture parity.** `LightUbo` is uploaded every frame even when `count = 0`. This costs one `queue.write_buffer(544 bytes)` per frame; well below noise but worth noting against the M28 byte-identical baseline. The captured frame stays byte-equal because the lit pipeline never runs with no lit batches.

## Sources

- [LearnOpenGL — Multiple Lights](https://learnopengl.com/Lighting/Multiple-lights) — forward N-light loop reference (3D Phong; M29 collapses to 2D N·L diffuse).
- [LearnOpenGL — Normal Mapping](https://learnopengl.com/Advanced-Lighting/Normal-Mapping) — tangent-space normal sample conventions and the `* 2.0 - 1.0` decode used by `lit_sprite.wgsl`.
- [Godot CanvasItem normal-map docs](https://docs.godotengine.org/en/stable/tutorials/2d/2d_lights_and_shadows.html) — 2D normal-map sibling file authoring conventions and per-sprite normal-texture wiring.
- [Aseprite — Normal Map for 2D sprites](https://www.aseprite.org/docs/normal-mapping/) — authoring workflow for sprite + normal sibling.
- [LWJGL3 SilenceEngine — 2D forward lighting](https://github.com/silenceengine/silenceengine/wiki/Lighting) — 2D N-light cap reference; informs the `LIGHT_CAP = 16` choice.
- [wgpu 29.0.1 `BindGroupLayoutEntry`](https://docs.rs/wgpu/29.0.1/wgpu/struct.BindGroupLayoutEntry.html) and [`Buffer::min_binding_size`](https://docs.rs/wgpu/29.0.1/wgpu/struct.BindingType.html#variant.Buffer.field.min_binding_size) — UBO binding layout used by `LightingResources`.
- [WGSL spec — std140 rules](https://www.w3.org/TR/WGSL/#address-space-layout-rules) — 544-byte `LightUbo` layout justification (vec4 alignment for `count_pad` and trailing `ambient`).
- Local source/docs researched for project fit: `docs/LLM_INDEX.md`, `docs/plans/phase4.md`, `docs/DECISION_INDEX.md`, `DECISIONS.md` entries `D-053`, `D-054`, `D-057`, `D-058`, `D-059`, `D-060`; `crates/tungsten-render/src/{renderer.rs,sprite.rs,material.rs,lit_sprite.rs (new),lighting.rs (new),targets.rs,passes/order.rs}`; `crates/tungsten-core/src/{components.rs,assets/manifest.rs,assets/registry.rs,assets/atlas.rs,camera.rs,post.rs,tween.rs,lib.rs}`; `crates/tungsten/src/{asset_loader.rs,sprite_extract.rs,app.rs}`; `examples/01_platformer/src/setup.rs`; `assets/manifest.json`; `scripts/smoke-examples.sh`.
