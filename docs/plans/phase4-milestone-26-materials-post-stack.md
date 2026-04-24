---
status: done
milestone: M26
goal: "Ship M26 Materials + Post-Stack + Tween→Material Bridge: manifest-tracked WGSL materials selectable per sprite batch, a reorderable `PostStack` of 17 stock effects ping-ponging between offscreen targets before the present blit, and an entity-local `UniformOverrideBlock` that `Tween` channels can drive. Under an empty `PostStack` and no `material_id`, output is byte-identical to the M25 `0.22` baseline (image-diff asserted)."
non-goals:
  - "No SMAA / post AA (M27 ships SMAA as a fixed presentation tail, not a `PostPass` variant)."
  - "No bloom (M28), no lighting (M29), no parallax/shake (M30), no mesh particles/transitions (M31), no MSDF text (M32)."
  - "No HDR scene color in M26: `SceneColor` stays swapchain sRGB; M28 introduces the HDR sibling when bloom actually needs it."
  - "No cross-entity `TweenTarget`; `D-055` one-`Tween`-per-entity stays intact. `TweenChannel` grows uniform-slot variants, nothing else."
  - "No preprocessor / include system. Stock shaders remain one `.wgsl` = one module, `include_str!`'d at compile time and mirrored into `assets/shaders/stock/` so the manifest can reference them and hot reload covers body edits."
  - "No cross-pipeline shared uniform buffer. Each `PostPass` owns its own UBO; materials use their own per-material UBO."
  - "No LYGIA crate dependency — helper snippets are vendored with attribution (`D-015` rule 3: replace what we would otherwise write in a day)."
  - "No shader variant permutations or `override`-driven combinatorial builds for M26; each stock effect is one pipeline."
  - "No capture tooling / GIF automation; acceptance clips are manual."
files to touch:
  - "crates/tungsten-core/src/assets/material.rs (new; `MaterialAssetId`, `MaterialRegistry`, `MaterialUniformDefaults`)"
  - "crates/tungsten-core/src/assets/manifest.rs (`materials` keyed section + `MaterialEntry` / `ResolvedMaterial`)"
  - "crates/tungsten-core/src/assets/mod.rs (re-export material types)"
  - "crates/tungsten-core/src/tween.rs (extend `TweenChannel` with uniform-slot variants; add slot enums)"
  - "crates/tungsten-core/src/components.rs (add `Sprite.material_id: Option<MaterialAssetId>` + `UniformOverrideBlock` component)"
  - "crates/tungsten-core/src/post.rs (new; `PostPass` enum + `PostStack` resource + every stock param struct)"
  - "crates/tungsten-core/src/lib.rs (re-export `UniformOverrideBlock`, `Vec4Slot`, `ScalarSlot`, `IntSlot`, `PostStack`, `PostPass`)"
  - "crates/tungsten-core/src/tests/assets/manifest.rs (materials keyed section + cross-ref to shaders)"
  - "crates/tungsten-core/src/tests/assets/material.rs (new; registry + path reverse + defaults serde)"
  - "crates/tungsten-core/src/tests/post.rs (new; `PostStack` ordering, param serde)"
  - "crates/tungsten-core/src/tests/tween.rs (uniform-slot channel tick tests)"
  - "crates/tungsten-render/src/material.rs (new; `MaterialPipeline`, per-material UBO, sprite-compatible pipeline layout)"
  - "crates/tungsten-render/src/post/mod.rs (new; `PostStackRenderer`, ping-pong routing)"
  - "crates/tungsten-render/src/post/fullscreen.rs (new; shared fullscreen-triangle vertex path + bind-group layout)"
  - "crates/tungsten-render/src/post/tonemap.rs, vignette.rs, lut.rs, chromatic_aberration.rs, color_adjust.rs, tone_mono.rs, crt.rs, film_grain.rs, dither.rs, pixel_outline.rs, fade.rs, wipe_radial.rs, dissolve.rs, glitch.rs, pixelate.rs, fog.rs, god_rays.rs (new; one file per stock effect pipeline)"
  - "crates/tungsten-render/src/shaders/stock/lygia/{noise.wgsl,hash.wgsl,srgb.wgsl,luma.wgsl} (new; vendored with MIT header)"
  - "crates/tungsten-render/src/shaders/stock/{tonemap,vignette,lut,chromatic_aberration,color_adjust,tone_mono,crt,film_grain,dither,pixel_outline,fade,wipe_radial,dissolve,glitch,pixelate,fog,god_rays}.wgsl (17 new; each `include_str!`'d + mirrored under workspace `assets/shaders/stock/`)"
  - "crates/tungsten-render/src/targets.rs (add `PostPing`, `PostPong` `TargetId` variants; pool allocation + resize)"
  - "crates/tungsten-render/src/passes/desc.rs (extend `TargetId` re-export; no new builder surface)"
  - "crates/tungsten-render/src/passes/order.rs (extend `default_pass_order` to splice the active `PostStack` into the scene→present chain)"
  - "crates/tungsten-render/src/passes/recorder.rs (resolve the new `PostPing` / `PostPong` views)"
  - "crates/tungsten-render/src/renderer.rs (thread `PostStack` into `render_frame_internal`; expose `upload_material` / `reload_material`; rebuild present_blit source alternation)"
  - "crates/tungsten-render/src/sprite.rs (per-`SpriteBatch` material pipeline selection; no change to `SpriteInstance` layout)"
  - "crates/tungsten-render/src/lib.rs (re-export material + post types)"
  - "crates/tungsten-render/src/tests/post.rs (new; device-free pass-order splicing tests)"
  - "crates/tungsten-render/src/tests/material.rs (new; uniform-bytes packing test)"
  - "crates/tungsten/src/asset_loader.rs (split into module; add `load_materials`, `reload_material`)"
  - "crates/tungsten/src/asset_loader/mod.rs (new; re-exports)"
  - "crates/tungsten/src/asset_loader/material.rs (new; extracted material load + reload)"
  - "crates/tungsten/src/sprite_extract.rs (emit `SpriteBatch.material_id` + per-batch uniform overrides; batch on effective material state within z-runs)"
  - "crates/tungsten/src/tweens.rs (apply uniform-slot channels against `UniformOverrideBlock`)"
  - "crates/tungsten/src/app.rs (add `materials` hot-reload branch; init `PostStack` resource; render-call threads `PostStack`)"
  - "crates/tungsten/src/tests/asset_loader.rs (material load + reload last-known-good)"
  - "crates/tungsten/src/tests/sprite_extract.rs (material-aware batching determinism)"
  - "crates/tungsten/src/tests/tweens.rs (slot-channel tick drives `UniformOverrideBlock`)"
  - "assets/shaders/stock/... (17 files + lygia helpers; mirror of compile-time sources so manifest hot reload works)"
  - "assets/manifest.json (register the 17 stock shaders + shared `damage_flash` shader/material entry)"
  - "input.json (workspace-root shader-playground action bindings; D-045 keeps input config here, not under example-local assets)"
  - "Cargo.toml (add `examples/04_shader_playground` as a workspace member)"
  - "examples/04_shader_playground/ (new example crate + local manifest; bouncing sprite, key-driven effect toggles, preset cycle)"
  - "examples/04_shader_playground/assets/manifest.json (local `materials` entries for the stock-effect demo)"
  - "examples/01_platformer/src/setup.rs, extract.rs, systems.rs (damage-flash uniform-tween demo on the existing custom extract path)"
  - "other direct `SpriteBatch` constructor sites that must initialize new material/uniform fields (for example umbrella helper extracts, benches, or example-local extractors)"
  - "scripts/smoke-examples.sh (add the explicit post-stack fixture rows; the baseline per-example loop already auto-discovers workspace example packages)"
  - "DECISIONS.md (new `D-0NN` — materials + post-stack design; narrows D-023/D-055/D-057 in stated ways; does not reverse any)"
  - "docs/DECISION_INDEX.md (one-line row for the new D-0NN)"
  - "docs/LLM_INDEX.md (add materials + post-stack subsystem row)"
  - "AGENTS.md (Asset Rules: new `materials` manifest section + `assets/shaders/stock/` vendored rule)"
  - "DESIGN.md (Status + Hot Reload matrix row `material` — body-edit only)"
  - "CHANGELOG.md (Unreleased bullet)"
ordered steps:
  - "Add the core-side material/post data model: manifest `materials`, `MaterialRegistry`, `PostPass`/`PostStack`, `UniformOverrideBlock`, and tween/scene serde support."
  - "Extend extract/render seams for M26: `Sprite.material_id`, `SpriteBatch` material/uniform state, `PostPing`/`PostPong`, material pipelines, stock post pipelines, and `render_frame_internal` post-stack routing."
  - "Add umbrella loader and hot-reload wiring for materials while keeping manifest-driven composition, shader reload, and last-known-good behavior consistent with M25/D-053/D-057."
  - "Update affected custom extract/example paths that construct `SpriteBatch` directly, especially `examples/01_platformer` and the new `examples/04_shader_playground`."
  - "Register root/example assets and input bindings using the existing workspace conventions (`assets/manifest.json`, example-local manifests, workspace-root `input.json`, workspace `Cargo.toml`)."
  - "Land tests, smoke-fixture coverage, image-diff parity checks, and the required docs/decision-index sync; then flip this plan to `done`."
done-when:
  - "`cargo test --workspace` green. New unit tests exist for: `MaterialRegistry` allocate/path-reverse (mirrors `ShaderRegistry` test shape), `ResolvedManifest::materials` parse + merge + missing-file / unknown-shader cross-ref, `PostPass` + `PostStack` serde round-trip, uniform-slot `TweenChannel` tick against `UniformOverrideBlock`, `default_pass_order` splicing for stacks of length 0 / 1 / 2 / 17, material/uniform-aware batching determinism."
  - "`./scripts/smoke-examples.sh` green. `example-04-shader-playground` is registered and runs under `TUNGSTEN_SMOKE_FRAMES=3`. Matrix includes `TUNGSTEN_POST_STACK_FIXTURE=all` for `example-04` which walks the 17-effect preset in 3 frames without panic."
  - "Image-diff assertion: under default config (`PostStack` empty, no `material_id`), frame 60 of `example-02-sprite-stress` matches `docs/showcase/m25-sprite-stress-baseline.png` within `image_diff::assert_within(1)`. Byte-identity with M25 is a hard gate on this milestone — a non-empty post stack must be opt-in, not default."
  - "Manual: editing `assets/shaders/stock/vignette.wgsl` (body-only; tweak inner radius expression) while `example-04-shader-playground` runs with `Vignette` in the stack updates visuals within ~200 ms, no rebuild. Parse/validation failure logs `material 'vignette' validation failed: ...` (or shader-level equivalent) and keeps the prior pipeline + live frame."
  - "Manual: on-screen HUD in `example-04-shader-playground` lists all 17 effects with their toggle key and current on/off state; every effect can be toggled independently; preset cycle (key `P`) steps through ≥3 named combinations."
  - "Manual: in `example-01-platformer`, a player hit from an existing `Ball` entity fires a one-shot tween that drives the damage-flash material uniform slot and returns to baseline in ≤ 250 ms. Saved clip under `docs/showcase/m26-damage-flash.gif`."
  - "`DECISIONS.md` has a new `D-0NN` entry (resolve via `rg -n '^## D-0' DECISIONS.md | tail -1`); `docs/DECISION_INDEX.md` has the matching row; `AGENTS.md`, `DESIGN.md`, `CHANGELOG.md`, and `docs/LLM_INDEX.md` are updated in the same change."
  - "Plan file flipped to `status: done`."
---

# Phase 4 Milestone 26 — Materials + Post-Stack + Tween→Material Bridge

## Context Digest

M25 (shipped in `0.22`) replaced the direct-to-swapchain forward pass with an ordered pass list against a `SceneColor` offscreen target, added `RenderTargetPool` with depth + MSAA, and moved WGSL sprite shaders onto the manifest graph with body-edit hot reload through `ShaderModuleCache` + `wgpu::naga` validation. `SceneColor` format equals the swapchain sRGB format (`D-057`). The M25 frame is `SceneColor → present_blit → Swapchain` — one scene pass, one fullscreen-triangle blit ([renderer.rs:574-623](crates/tungsten-render/src/renderer.rs#L574-L623)).

M26 splices a reorderable post-processing stack between `SceneColor` and `Swapchain`, and opens the sprite pipeline up to user-authored WGSL materials. Architecture:

```
tungsten-core                           tungsten                              tungsten-render
-------------                           --------                              ---------------
MaterialAssetId(u32)                    asset_loader/material.rs              material.rs
MaterialRegistry                          load_materials                        MaterialPipeline (per id)
MaterialUniformDefaults                   reload_material                       per-material 256-byte UBO
ResolvedManifest.materials              sprite_extract (emits material_id)    sprite.rs (per-batch pipeline select)
PostPass (closed enum, 17 variants)     world resource / example systems      post/
PostStack(Vec<PostPass>)                render call site threads              post/<effect>.rs (17 files, one per variant)
UniformOverrideBlock                    sprite/custom extract emits overrides  post/fullscreen.rs (shared triangle vs)
TweenChannel::Uniform* variants         tweens.rs (writes override block)     targets.rs (+PostPing, +PostPong)
Sprite.material_id / custom extract keys per-batch material select           passes/order.rs (scene → N×post → present)
```

Seam constraints from phase4.md hold: manifest stays keyed `HashMap<String, Entry>` (D-017); `Tween` stays one-per-entity (D-055); post-AA is NOT a `PostPass` and is deferred to M27; the `UniformOverrideBlock` is the single per-entity animation surface both material uniforms (M26) and MSDF outline/glow (M32) will share. Stock shaders are vendored, not a crate dependency (D-015 rule 3). Hot reload body-only extends the M25 matrix (`D-057`); signature-level changes still require a rebuild.

### Scene → Post → Present Target Flow

| Post stack length | Pass chain | Notes |
| --- | --- | --- |
| 0 | `SceneColor → present_blit → Swapchain` | Byte-identical to M25 baseline. Image-diff gate. |
| 1 | `SceneColor → post_0(read=SceneColor, write=PostPing) → present_blit(src=PostPing)` | One ping allocation. |
| N ≥ 2 | `SceneColor → post_0(→PostPing) → post_1(→PostPong) → post_2(→PostPing) → … → present_blit(src=last)` | Alternate ping/pong; last write is the present blit source. |

`PostPing` and `PostPong` both use the same format as `SceneColor` (`surface_config.format`). Both are allocated on first non-empty stack activation and freed on resize to match the current viewport (pool handles this in one place).

### Stock Effect Roster (locked — 17 effects)

| # | Variant | File | Uniform layout (subset of 256-byte UBO) |
| --- | --- | --- | --- |
| 1 | `Tonemap(TonemapParams)` | `post/tonemap.rs` + `shaders/stock/tonemap.wgsl` | `mode: u32` (reinhard/aces_approx/aces_fitted), `exposure: f32`, `white_point: f32` |
| 2 | `Vignette(VignetteParams)` | `vignette.rs` + `vignette.wgsl` | `inner: f32`, `outer: f32`, `strength: f32`, `color: vec4<f32>` |
| 3 | `Lut(LutParams)` | `lut.rs` + `lut.wgsl` | `mix: f32`, `lut_sprite_id: u32 (resolved atlas slot)` |
| 4 | `ChromaticAberration(f32)` | `chromatic_aberration.rs` + `chromatic_aberration.wgsl` | `strength: f32` |
| 5 | `ColorAdjust { hue, sat, contrast }` | `color_adjust.rs` + `color_adjust.wgsl` | three `f32` |
| 6 | `ToneMono(ToneMonoParams)` | `tone_mono.rs` + `tone_mono.wgsl` | `mode: u32 (sepia/mono/duotone)`, `tint_a: vec4`, `tint_b: vec4`, `amount: f32` |
| 7 | `Crt(CrtParams)` | `crt.rs` + `crt.wgsl` | `scanline_strength: f32`, `curvature: f32`, `mask: u32`, `bleed: f32` |
| 8 | `FilmGrain(f32)` | `film_grain.rs` + `film_grain.wgsl` | `strength: f32`, `time_seed: f32` |
| 9 | `Dither(DitherParams)` | `dither.rs` + `dither.wgsl` | `mode: u32 (bayer4/bayer8/blue_noise)`, `levels: u32`, `strength: f32` |
| 10 | `PixelOutline(PixelOutlineParams)` | `pixel_outline.rs` + `pixel_outline.wgsl` | `color: vec4`, `thickness_px: f32`, `alpha_threshold: f32` |
| 11 | `Fade(f32)` | `fade.rs` + `fade.wgsl` | `progress: f32`, `color: vec4` |
| 12 | `WipeRadial(f32)` | `wipe_radial.rs` + `wipe_radial.wgsl` | `progress: f32`, `center: vec2`, `softness: f32` |
| 13 | `Dissolve(f32)` | `dissolve.rs` + `dissolve.wgsl` | `progress: f32`, `noise_scale: f32`, `edge_color: vec4` |
| 14 | `Glitch(GlitchParams)` | `glitch.rs` + `glitch.wgsl` | `block_strength: f32`, `shift_px: f32`, `time_seed: f32` |
| 15 | `Pixelate(f32)` | `pixelate.rs` + `pixelate.wgsl` | `block_px: f32` |
| 16 | `Fog(FogParams)` | `fog.rs` + `fog.wgsl` | `density: f32`, `color: vec4`, `height_falloff: f32` |
| 17 | `GodRays(GodRaysParams)` | `god_rays.rs` + `god_rays.wgsl` | `center: vec2`, `density: f32`, `decay: f32`, `weight: f32`, `samples: u32` |

All 17 UBOs fit inside one shared 256-byte binding; the `MaterialUniforms` layout from phase4.md M26 (4×`vec4` + 4×`f32` + 4×`i32`, 80 bytes of usable payload + 176 bytes of slack for alignment and future growth) is the **same** 256-byte shape — same bind group layout, same struct in WGSL. Each `PostPass` stamps its own param values into that shape; each material uses the same slots driven by `UniformOverrideBlock`.

### Uniform Override Block (shared surface for M26 + M32)

```rust
// tungsten-core/src/tween.rs (new)
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UniformOverrideBlock {
    pub vec4: [[f32; 4]; 4], // slots V0..V3
    pub f32s: [f32; 4],      // slots F0..F3
    pub i32s: [i32; 4],      // slots I0..I3
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Vec4Slot { V0, V1, V2, V3 }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarSlot { F0, F1, F2, F3 }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntSlot { I0, I1, I2, I3 }
```

GPU packing matches `std140`-friendly WGSL layout; the 256-byte material UBO places vec4 slots first, then scalars, then ints — identical layout between `MaterialUniforms`, `PostPass` params, and `UniformOverrideBlock::to_bytes()`. M32 MSDF reuses the same block with named slot constants (e.g. `OUTLINE_COLOR: Vec4Slot = V0`).

### Tween Channel Extensions

```rust
// tungsten-core/src/tween.rs (extended)
pub enum TweenChannel {
    // ... existing Position/Rotation/Scale/Color variants unchanged ...
    UniformVec4Lane { slot: Vec4Slot, lane: u8, from: f32, to: f32 }, // lane ∈ {0,1,2,3}
    UniformScalar   { slot: ScalarSlot,        from: f32, to: f32 },
    UniformInt      { slot: IntSlot,           from: i32, to: i32 }, // stepped, not lerped
}
```

No new `TweenTarget`, no second `Tween` per entity, no cross-entity target. `tween_tick_system` writes into `UniformOverrideBlock` when present; when absent, the channel variant is a logged warning and skipped. The renderer reads `UniformOverrideBlock` at extract time and, if present, uses those values for the per-entity material UBO; otherwise it falls back to `ResolvedMaterial::uniform_defaults`.

### Hot Reload Matrix Delta

| asset | M25 | M26 |
| --- | --- | --- |
| material (`.json` / section) | — | body-only (uniform_defaults) + shader pointer re-validate |
| shader (`.wgsl`) | sprite only | any manifest-tracked shader, including stock effects and user materials |

Signature-level changes (bind-group layout, instance attributes) still require a rebuild — narrowing, not reversing, `D-057`.

## Ordered Steps

### 1. Core data types: `UniformOverrideBlock`, slot enums, `TweenChannel` extensions
- Files: [crates/tungsten-core/src/components.rs](crates/tungsten-core/src/components.rs), [crates/tungsten-core/src/tween.rs](crates/tungsten-core/src/tween.rs); new cases for [crates/tungsten-core/src/tests/tween.rs](crates/tungsten-core/src/tests/tween.rs).
- Action:
  - Add `UniformOverrideBlock` as an entity component in [crates/tungsten-core/src/components.rs](crates/tungsten-core/src/components.rs); keep `Vec4Slot`, `ScalarSlot`, and `IntSlot` alongside `TweenChannel` in [crates/tungsten-core/src/tween.rs](crates/tungsten-core/src/tween.rs). `UniformOverrideBlock` implements `Default` → all zeros, and `to_bytes(&self) -> [u8; 256]` via `bytemuck::bytes_of` with an explicit `#[repr(C, align(16))]` padded layout matching WGSL std140 expectations (vec4 slots first; zero-padded tail).
  - Extend `TweenChannel` with the three `Uniform*` variants above. Update `serde(rename_all = "snake_case")` on `SceneTweenChannel` later (step 11) to keep scene-asset compatibility.
  - Do **not** alter `Tween` itself (`D-055` one-per-entity stays).
- Verify: `cargo test -p tungsten-core tween::` — new cases: (a) `UniformOverrideBlock::default().to_bytes() == [0; 256]`; (b) writing `vec4[0][2] = 1.0` lands at bytes 8..12; (c) `TweenChannel::UniformScalar { slot: F1, from: 0.0, to: 1.0 }` round-trips through serde.

### 2. Core data types: `MaterialAssetId`, `MaterialRegistry`, `MaterialUniformDefaults`
- Files: [crates/tungsten-core/src/assets/material.rs](crates/tungsten-core/src/assets/material.rs) (new); [crates/tungsten-core/src/assets/mod.rs](crates/tungsten-core/src/assets/mod.rs); [crates/tungsten-core/src/tests/assets/material.rs](crates/tungsten-core/src/tests/assets/material.rs) (new).
- Action:
  - Mirror the shape of [crates/tungsten-core/src/assets/shader.rs](crates/tungsten-core/src/assets/shader.rs): `pub struct MaterialAssetId(pub u32)`; `pub struct MaterialRegistry { next, ids, paths, reverse, names }`; identical `allocate / get / id_for_path / name_for_id / path_for_id / iter` API.
  - Add `pub struct MaterialUniformDefaults { pub vec4: [[f32; 4]; 4], pub f32s: [f32; 4], pub i32s: [i32; 4] }` with `#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]`. The struct mirrors `UniformOverrideBlock` — it is the manifest-author-facing serialization of the same shape.
  - No `wgpu` types (seam D-016).
- Verify: `cargo test -p tungsten-core assets::material::` — allocate/lookup/double-allocate, path-reverse, `MaterialUniformDefaults` JSON round-trip.

### 3. Manifest `materials` keyed section
- Files: [crates/tungsten-core/src/assets/manifest.rs](crates/tungsten-core/src/assets/manifest.rs); [crates/tungsten-core/src/tests/assets/manifest.rs](crates/tungsten-core/src/tests/assets/manifest.rs).
- Action:
  - Add `MaterialEntry { shader: String, #[serde(default)] uniform_defaults: MaterialUniformDefaults }` to `RawManifest` and `ResolvedMaterial { shader_asset_id_name: String, uniform_defaults: MaterialUniformDefaults }` to `ResolvedManifest` — we resolve to the shader's manifest ID (not `ShaderAssetId`) in core because ID allocation is a runtime, not a parse-time, concern; `asset_loader::load_materials` does the ID resolution.
  - Add `ManifestError::MaterialShaderMissing { id: String, shader: String }` for the unresolved-shader-cross-ref case, and `ManifestError::DuplicateId { id }` already covers collision (D-017).
  - Extend `merge` to handle `materials` alongside every existing section.
- Verify: `cargo test --workspace` — Layer 1 picks up the new section. New unit tests: (a) a manifest with `materials.damage_flash.shader = "damage_flash"` parses and the referenced `shaders.damage_flash` entry resolves; (b) a manifest that references a shader id with no matching `shaders` entry returns `MaterialShaderMissing` from a new post-parse validation step (run inside `ResolvedManifest::load` after both sections have populated); (c) duplicate material IDs across merged manifests are fatal.

### 4. `Sprite.material_id` + sprite extract batch key
- Files: [crates/tungsten-core/src/components.rs](crates/tungsten-core/src/components.rs); [crates/tungsten-render/src/sprite.rs](crates/tungsten-render/src/sprite.rs) (`SpriteBatch`); [crates/tungsten/src/sprite_extract.rs](crates/tungsten/src/sprite_extract.rs); [crates/tungsten/src/tests/sprite_extract.rs](crates/tungsten/src/tests/sprite_extract.rs); plus any custom extract sites that construct `SpriteBatch` directly.
- Action:
  - Add `pub material_id: Option<MaterialAssetId>` to `Sprite`. `Sprite::new` sets it to `None`. No change to `SpriteInstance` GPU layout.
  - `SpriteBatch` gets `pub material_id: Option<MaterialAssetId>` plus `pub uniform_overrides: Option<UniformOverrideBlock>`, so the render path does not have to recover entity-local data after extraction.
  - `extract_sprites_default`: batch key becomes the effective material state inside each z-run, not just `(atlas, filter)`. Different `material_id` values or different uniform-override payloads must split batches so per-entity material animation cannot alias through one UBO upload. Same-z ties stay `(z_order, entity.id())`.
  - Any direct `SpriteBatch` constructor outside the default extract must initialize the new fields explicitly, even when they stay `None`.
- Verify: existing `sprite_extract` tests still pass. New tests:
  - `material_aware_batches_split` — two sprites with the same `(atlas, filter, z_order)` but different `material_id` produce two `SpriteBatch`es in deterministic entity-id order.
  - `uniform_override_batches_split` — two sprites with the same `(atlas, filter, z_order, material_id)` but different override payloads do not alias into one batch.
  - `z_run_reset_clears_material_key` — a new z-run does not share material-batching state with the previous z-run.

### 5. `PostPass` enum + `PostStack` resource + `post.rs`
- Files: [crates/tungsten-core/src/post.rs](crates/tungsten-core/src/post.rs) (new); [crates/tungsten-core/src/lib.rs](crates/tungsten-core/src/lib.rs); [crates/tungsten-core/src/tests/post.rs](crates/tungsten-core/src/tests/post.rs) (new).
- Action: define the 17 `PostPass` variants with the param structs from the stock roster table above. `#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]` on each; outer enum `#[serde(tag = "kind", content = "params", rename_all = "snake_case")]`. `PostStack(pub Vec<PostPass>)` as a resource with `push`, `clear`, `len`, `reorder(&mut self, from: usize, to: usize)`. Default is empty.
- Verify: `cargo test -p tungsten-core post::` — (a) empty-stack default; (b) JSON serde of each of the 17 variants round-trips; (c) `reorder` preserves length and moves the targeted element.

### 6. Render-side target pool: `PostPing` / `PostPong`
- Files: [crates/tungsten-render/src/targets.rs](crates/tungsten-render/src/targets.rs); [crates/tungsten-render/src/passes/desc.rs](crates/tungsten-render/src/passes/desc.rs); [crates/tungsten-render/src/passes/recorder.rs](crates/tungsten-render/src/passes/recorder.rs).
- Action:
  - Extend `enum TargetId` with `PostPing` and `PostPong`.
  - Add `post_ping: Option<(Texture, TextureView)>` and `post_pong: Option<(Texture, TextureView)>` to `SceneTarget`; allocate both on `RenderTargetPool::new` / `resize` whenever the configured max stack length > 0 (for M26: always, since a user can push into the stack at runtime). Format equals `SceneColor` format. Usage: `RENDER_ATTACHMENT | TEXTURE_BINDING`. Sample count 1 (post never multisamples — scene pass already resolved).
  - `PassRecorder::begin` gains view resolution for the two new variants; add matching panics for the "requested target not allocated" case identical to the existing pattern.
- Verify: keep unit coverage on pure-data pass/target-planning helpers, and rely on `cargo build -p tungsten-render` plus Layer 2 smoke to exercise the device-backed `RenderTargetPool` allocation path for `PostPing` / `PostPong`.

### 7. `default_pass_order` splicing
- Files: [crates/tungsten-render/src/passes/order.rs](crates/tungsten-render/src/passes/order.rs); [crates/tungsten-render/src/tests/passes_order.rs](crates/tungsten-render/src/tests/passes_order.rs).
- Action: change the signature to `pub fn default_pass_order(msaa: u32, depth_sort: DepthSortMode, depth_enabled: bool, post_stack_len: usize) -> PassOrder`. Logic:
  1. Start with the existing `scene` pass (unchanged).
  2. For `i in 0..post_stack_len`: push a `post_{i}` `PassDesc` that writes into `PostPing` (even `i`) or `PostPong` (odd `i`), with no clear (full-screen write). Label format `"tungsten_post_pass_{i}"`.
  3. Push the existing `present` pass.
  - Keep `tungsten-render` world-agnostic (`D-018`/`D-016` seam). The renderer owns the current `post_stack_len` value and forwards it into `default_pass_order`; the umbrella crate updates that renderer-side value from the `PostStack` resource before each frame.
- Verify: new unit tests in `tests/passes_order.rs` assert the pass chain for `post_stack_len ∈ {0, 1, 2, 17}`. The length-0 case must be byte-identical to M25's pass order (regression gate for the image-diff baseline).

### 8. `MaterialPipeline` + per-material UBO bind group
- Files: [crates/tungsten-render/src/material.rs](crates/tungsten-render/src/material.rs) (new); [crates/tungsten-render/src/lib.rs](crates/tungsten-render/src/lib.rs); [crates/tungsten-render/src/tests/material.rs](crates/tungsten-render/src/tests/material.rs) (new).
- Action:
  - `pub struct MaterialPipeline { pipeline: RenderPipeline, ubo: Buffer, bind_group: BindGroup, name: String, shader_id: ShaderAssetId }`. Pipeline layout: group 0 camera (shared with sprite), group 1 texture+sampler (shared with sprite), group 2 the material 256-byte UBO. Vertex + instance layouts must stay byte-identical to the sprite path; if `material.rs` cannot reach `SpriteVertex::desc()` directly because that helper is private today, factor a crate-private shared layout helper out of [crates/tungsten-render/src/sprite.rs](crates/tungsten-render/src/sprite.rs) instead of duplicating a second layout by hand.
  - `impl MaterialPipeline`: `new(&Device, shader: &ShaderModule, format, sample_count, depth_write, name, shader_id) -> Self`; `write_uniforms(&self, &Queue, &UniformOverrideBlock | &MaterialUniformDefaults)` using a single 256-byte upload.
  - Storage: `Renderer` gets `materials: HashMap<MaterialAssetId, MaterialPipeline>`. Null id ⇒ fall through to the built-in sprite pipeline.
  - New `Renderer::upload_material(&mut self, id: MaterialAssetId, name: &str, shader_id: ShaderAssetId, defaults: MaterialUniformDefaults)` and `reload_material(&mut self, id: MaterialAssetId, ...)` that rebuild the pipeline from the current cached `ShaderModule`. Follow the M25 last-known-good pattern: validate-then-commit, leave prior entry untouched on failure.
- Verify: `cargo test -p tungsten-render material::` — device-free test that `UniformOverrideBlock { vec4[0] = [1.0, 2.0, 3.0, 4.0], f32s[1] = 0.5, i32s[2] = 7, .. }` packs into the first 64 + 16 + 16 = 96 bytes in the documented slot order.

### 9. 17 stock effect pipelines
- Files: `crates/tungsten-render/src/post/{fullscreen,tonemap,vignette,lut,chromatic_aberration,color_adjust,tone_mono,crt,film_grain,dither,pixel_outline,fade,wipe_radial,dissolve,glitch,pixelate,fog,god_rays}.rs` (18 new files: `fullscreen` + 17 effects); `crates/tungsten-render/src/shaders/stock/{...}.wgsl` (17 WGSL files + 4 lygia helpers); mirrors at `assets/shaders/stock/...`.
- Action:
  - `post/fullscreen.rs` defines the shared fullscreen-triangle vertex shader (or just a `@builtin(vertex_index)`-only vs_main), the shared bind-group layout `(@group(0) source_texture + sampler, @group(1) params UBO, @group(2) [optional LUT for Lut variant])`, and the `FullscreenBindings::new(...)` helper that every effect module uses.
  - Per-effect module: one `struct <Effect>Pipeline { pipeline, params_ubo, bind_group }`, one `new(&Device, scene_format: TextureFormat, source_view, source_sampler) -> Self`, one `record(&self, &mut RenderPass, params: &<Effect>Params)` that writes the UBO and issues `draw(0..3, 0..1)`.
  - Each WGSL file has:
    ```wgsl
    // LYGIA snippets vendored under MIT; see crates/tungsten-render/src/shaders/stock/lygia/LICENSE
    ```
    header when it pulls in vendored helper text. M26 does not implement a WGSL preprocessor; helper composition, where needed, stays as explicit Rust-side source concatenation inside the pipeline module.
  - Lygia files: cherry-pick `noise.wgsl` (simplex 2D), `hash.wgsl` (pcg hash), `srgb.wgsl` (linear↔srgb), `luma.wgsl` (bt709 luma) — minimum set used by CRT, film grain, dissolve, glitch, color_adjust, tone_mono, fog, god_rays. Add a top-of-file MIT attribution block citing `https://lygia.xyz`.
- Verify: `cargo build --workspace` compiles every stock pipeline; the Layer-2 smoke fixture (step 19) exercises each pass-chain permutation once with 3 frames.

### 10. Wire `PostStack` through `render_frame_internal`
- Files: [crates/tungsten-render/src/renderer.rs](crates/tungsten-render/src/renderer.rs); [crates/tungsten-render/src/post/mod.rs](crates/tungsten-render/src/post/mod.rs) (new).
- Action:
  - `post/mod.rs`: `pub struct PostStackRenderer { tonemap: TonemapPipeline, vignette: VignettePipeline, ... }` — one live pipeline per stock effect. Allocated once at `Renderer::new` against `self.surface_config.format`. Exposes `record(&self, &mut CommandEncoder, &PostStack, &RenderTargetPool, &Queue) -> TargetId /* last write */`.
  - Inside `record`, loop through `PostStack.0`: for each `PostPass`, pick `src_view` / `dst_view` from `SceneColor`, `PostPing`, `PostPong` per the alternation table; begin a new render pass via `PassRecorder::begin`; dispatch the matching effect pipeline's `record`.
  - In `render_frame_internal`:
    - Before the current present pass, call `post_stack_renderer.record(...)` iff `post_stack_len > 0`.
    - Update `blit_bind_group` to sample from the final post target when `post_stack_len > 0`, otherwise from `SceneColor` (current M25 behavior).
    - Screenshot readback: copy from the same "final scene source" (either `SceneColor` or the last post target). Byte-identity with M25 at `post_stack_len == 0` is the hard test.
    - `default_pass_order` gets `post_stack_len` forwarded from the new `Renderer::last_post_stack_len: usize` field that `render_frame_full` updates via a new `set_post_stack_len(&mut self, len: usize)` API; the umbrella crate calls this with the `PostStack` resource length each frame before `render_frame_full`.
- Verify: `cargo build -p tungsten-render`; headless unit test for `PostStackRenderer::plan_targets(stack_len) -> Vec<(src: TargetId, dst: TargetId)>` matches the ping-pong alternation table; covers `len ∈ {0, 1, 2, 3, 17}`.

### 11. Scene tween serde for uniform channels
- Files: [crates/tungsten-core/src/assets/scene.rs](crates/tungsten-core/src/assets/scene.rs); [crates/tungsten-core/src/tests/assets/scene.rs](crates/tungsten-core/src/tests/assets/scene.rs).
- Action: add matching `SceneTweenChannel::UniformVec4Lane`, `UniformScalar`, `UniformInt` variants and a `From<SceneTweenChannel> for TweenChannel` branch for each. Scene JSON remains forward-compatible — existing scene files without the new variants keep loading.
- Verify: `cargo test -p tungsten-core scene::` — new test loads a scene with `{ "kind": "uniform_scalar", "slot": "f0", "from": 0.0, "to": 1.0 }` and confirms it converts into `TweenChannel::UniformScalar { slot: ScalarSlot::F0, from: 0.0, to: 1.0 }`.

### 12. Tween system applies uniform channels
- Files: [crates/tungsten/src/tweens.rs](crates/tungsten/src/tweens.rs); [crates/tungsten/src/tests/tweens.rs](crates/tungsten/src/tests/tweens.rs).
- Action: extend `apply_channels` with three new match arms — `UniformVec4Lane / UniformScalar / UniformInt` — each `world.get_mut::<UniformOverrideBlock>(entity)` and writes the interpolated value. If the component is absent, `log::debug!` and continue (do not panic — matches the existing "channel references a missing component" behavior). `UniformInt` steps by `k >= 0.5 ? to : from` (integer tweens are a common Godot-style UX pattern; no lerp).
- Verify: unit tests drive a one-second linear `Tween` with a `UniformScalar { slot: F1, from: 0.0, to: 1.0 }` channel against a fresh `UniformOverrideBlock`; after 0.5 s the scalar is within `±1e-5` of 0.5; after 1.0 s it is 1.0 and a `TweenComplete` event fires.

### 13. Split `asset_loader.rs` into module; add `load_materials` / `reload_material`
- Files: [crates/tungsten/src/asset_loader.rs](crates/tungsten/src/asset_loader.rs) → [crates/tungsten/src/asset_loader/mod.rs](crates/tungsten/src/asset_loader/) (new module); new [crates/tungsten/src/asset_loader/material.rs](crates/tungsten/src/asset_loader/material.rs); [crates/tungsten/src/tests/asset_loader.rs](crates/tungsten/src/tests/asset_loader.rs).
- Action:
  - Convert the flat file into a module. Keep every existing public fn (`load_sprites`, `load_animations`, `load_fonts`, `load_sounds`, `load_tilemaps`, `load_shaders`, `load_particles`, `load_all`, `load_all_merged`, every `reload_*`) re-exported from `mod.rs` so external callers remain source-compatible.
  - Keep the existing shader logic in `mod.rs` for M26 unless the split becomes necessary while implementing materials; the milestone-specific extraction here is `asset_loader/material.rs`, matching [docs/plans/phase4.md](docs/plans/phase4.md).
  - `load_materials(manifest: &ResolvedManifest, world: &mut World, renderer: &mut Renderer) -> anyhow::Result<()>`: for each `ResolvedMaterial`, look up the `ShaderAssetId` via `ShaderRegistry` (fatal if missing — it should already be resolved by `load_shaders` which runs first), allocate a `MaterialAssetId`, and call `renderer.upload_material(id, name, shader_id, uniform_defaults)`. Store the `MaterialRegistry` as a world resource.
  - `reload_material(id: &str, world: &mut World, renderer: &mut Renderer) -> anyhow::Result<()>`: resolve `MaterialAssetId` from `MaterialRegistry`, look up the `ResolvedMaterial` in the current `LoadedManifest`, re-invoke `renderer.reload_material(...)`. Validation failure logs and keeps the previous pipeline.
  - Load order in `load_all`: sprites → animations → fonts → **shaders → materials** → sounds → tilemaps → particles.
- Verify:
  - Unit tests in `asset_loader` module exercise the new `load_materials` path end-to-end against an in-memory `ResolvedManifest` (no live renderer; stub the renderer behind a trait if needed) — pattern mirrors the existing particle-load tests.
  - Integration test: `example-04-shader-playground` boots cleanly with `TUNGSTEN_SMOKE_FRAMES=3`.

### 14. Hot-reload routing for `materials` + stock-shader `.wgsl`
- Files: [crates/tungsten/src/app.rs](crates/tungsten/src/app.rs) (`process_hot_reload`); [crates/tungsten/src/hot_reload.rs](crates/tungsten/src/hot_reload.rs) (no change expected; `.wgsl` already routes through the existing recursive watch).
- Action:
  - Existing `"wgsl"` branch already dispatches to `reload_shader`. Verify it covers the 17 stock shader ids (they're manifest-registered) plus any user shader (e.g. `damage_flash`). No new code — only verification.
  - Material edits in M26 still flow through manifest reload, because `materials` live in the manifest graph, not in standalone JSON files. Add the concrete `reload_material` / manifest-add handling needed for the new `materials` section, but do not add speculative support for a future per-material JSON file layout in this milestone.
  - Manifest-add path (`reload_manifest`): on a successful manifest reload that introduces a new `materials` entry, call `renderer.upload_material(...)` for the new id. Mirrors the existing particle-add path and keeps `LoadedManifest` current (`D-053`).
- Verify: manual — `touch assets/shaders/stock/vignette.wgsl` while `example-04` is running logs `shader 'vignette' reloaded`. Editing the `uniform_defaults` field of a `materials` entry in `assets/manifest.json` and saving triggers `Hot-reloaded manifest` + rebuild of the affected material.

### 15. Sprite pipeline per-batch material selection
- Files: [crates/tungsten-render/src/sprite.rs](crates/tungsten-render/src/sprite.rs) — `SpritePipeline::draw` only.
- Action:
  - Add a `material_pipelines: &HashMap<MaterialAssetId, MaterialPipeline>` parameter to `SpritePipeline::draw` (the umbrella pipeline-registry reference lives on `Renderer`, so `record_main_draws` threads it through).
  - Inside the existing batch loop, if `batch.material_id.is_some()`, pick the matching `MaterialPipeline::pipeline`; else use `self.pipeline`. Re-bind only when the pipeline handle changes across adjacent batches (avoid redundant `set_pipeline`).
  - Material UBO bind (group 2) is updated once per batch from `batch.uniform_overrides` or the material defaults. Do not infer the value from a "first entity in the batch" at draw time; the extract step must already have split batches on effective material state so the renderer can stay world-free and deterministic.
- Verify: `cargo test -p tungsten-render sprite::` — existing tests pass unchanged. Add a test that constructs two `SpriteBatch`es with different material ids and asserts the draw loop would call `set_pipeline` exactly twice (use a mock render pass recorder; follow the pattern from the existing sprite pipeline tests).

### 16. Registers `PostStack`, renders the shader playground
- Files: [examples/04_shader_playground/](examples/04_shader_playground/) (new crate; Cargo workspace member); [crates/tungsten/src/app.rs](crates/tungsten/src/app.rs) (`App::new` inserts an empty `PostStack` resource; `App::register_event` already covers `TweenComplete`).
- Action:
  - Scaffold the example: one bouncing 64×64 sprite, a HUD listing 17 effect rows with toggle keys (`1`–`9`, `0`, `q`–`u`; 17 slots) and an on/off indicator next to each, a preset cycle key `P` with 3+ named presets (`"retro_arcade"` = Crt + FilmGrain + ColorAdjust; `"dreamy"` = Bloom (not in M26, skip) → use `Fade + Vignette + ToneMono`; `"glitch_boss"` = Glitch + ChromaticAberration + Dither).
  - Action map: register input actions for each toggle and the preset cycle using the existing `ActionMap` (`D-045`). Because Tungsten currently loads and hot-reloads one workspace-root `input.json`, add the new bindings there rather than inventing an example-local input file path for M26.
  - One-shot tween demo: pressing `T` spawns a `Tween { duration: 0.25, channels: [UniformVec4Lane { slot: V0, lane: 0, from: 1.0, to: 0.0 }] }` on the sprite entity to illustrate the uniform-tween path.
  - `TUNGSTEN_POST_STACK_FIXTURE` env var (read in `main.rs`) lets the smoke script pre-populate the stack with either `"all"` (17 effects) or `"empty"` for the byte-identity gate.
- Verify: `cargo run -p example-04-shader-playground` renders the bouncing sprite; toggling each effect updates the scene; preset cycle steps through the 3 named combos. Layer 2 smoke (step 19) exercises the `"all"` fixture.

### 17. Damage-flash demo in platformer
- Files: [examples/01_platformer/src/setup.rs](examples/01_platformer/src/setup.rs), [examples/01_platformer/src/extract.rs](examples/01_platformer/src/extract.rs), [examples/01_platformer/src/systems.rs](examples/01_platformer/src/systems.rs); [assets/manifest.json](assets/manifest.json); one new WGSL asset under `assets/shaders/damage_flash.wgsl` (shared under workspace assets because it is a sprite material a game could reuse).
- Action:
  - Add a `damage_flash` WGSL sprite material that reads `material.vec4[0]` as an overlay color and mixes over the sampled sprite texture by `material.f32s[0]`.
  - Register it in `assets/manifest.json` (`shaders.damage_flash` + `materials.damage_flash` with zero defaults).
  - Because the platformer player renders through `CurrentSprite` + the custom [examples/01_platformer/src/extract.rs](examples/01_platformer/src/extract.rs) path rather than the default `Sprite` component, thread the player's `MaterialAssetId` and `UniformOverrideBlock::default()` through that existing custom extract instead of assuming `Sprite.material_id` is present on the entity.
  - On `CollisionEvent` between the player and an existing `Ball` entity, queue a one-shot `Tween` with:
    - `channels = [UniformVec4Lane { slot: V0, lane: 0, from: 0.0, to: 1.0 }, UniformScalar { slot: F0, from: 0.8, to: 0.0 }]`
    - `duration = 0.25`, `easing = QuadOut`.
- Verify: manual run; saved clip at `docs/showcase/m26-damage-flash.gif`.

### 18. Minimal workspace-root manifest registrations
- Files: [assets/manifest.json](assets/manifest.json); [assets/shaders/stock/](assets/shaders/stock/) (mirrors of the 17 compile-time sources).
- Action: register the 17 stock shaders under the existing `shaders` section (`tonemap`, `vignette`, `lut`, `chromatic_aberration`, `color_adjust`, `tone_mono`, `crt`, `film_grain`, `dither`, `pixel_outline`, `fade`, `wipe_radial`, `dissolve`, `glitch`, `pixelate`, `fog`, `god_rays`) and their paths under `shaders/stock/<id>.wgsl`. Register one workspace-scoped `damage_flash` material for the platformer demo; per-effect stock materials are registered locally by `examples/04_shader_playground/assets/manifest.json`.
- Verify: `cargo test --workspace` — Layer 1 (`manifests.rs`) resolves every new entry; byte-equality short-circuit for stock shaders keeps the boot fast.

### 19. Smoke matrix + post-stack fixture
- Files: [scripts/smoke-examples.sh](scripts/smoke-examples.sh).
- Action: keep the existing per-example loop package-discovered via `cargo metadata`; once `examples/04_shader_playground` is added to the workspace, it is picked up automatically. Add only the explicit post-stack fixture rows: run `example-04` once with `TUNGSTEN_POST_STACK_FIXTURE=all TUNGSTEN_SMOKE_FRAMES=3` and once with `TUNGSTEN_POST_STACK_FIXTURE=empty TUNGSTEN_SMOKE_FRAMES=3`. Both must exit 0. Preserve the M25 `{msaa, depth_sort}` matrix rows.
- Verify: `./scripts/smoke-examples.sh` exits 0 with every matrix row reported pass.

### 20. Image-diff regression gate (M25 baseline parity)
- Files: nothing new — reuse [docs/showcase/m25-sprite-stress-baseline.png](docs/showcase/m25-sprite-stress-baseline.png) + the existing `image_diff` machinery.
- Action: run the image-diff assertion under `PostStack` empty (default). Frame 60 of `example-02-sprite-stress` must still match the M25 baseline within tolerance 1.
- Verify:
  ```bash
  TUNGSTEN_CAPTURE_FRAME=60 TUNGSTEN_CAPTURE_PATH=/tmp/m26-default.png \
      cargo run -p example-02-sprite-stress
  # → diff against docs/showcase/m25-sprite-stress-baseline.png via image_diff
  ```

### 21. New decision entry `D-0NN`
- Files: [DECISIONS.md](DECISIONS.md), [docs/DECISION_INDEX.md](docs/DECISION_INDEX.md).
- Action: resolve the next decision number via `rg -n '^## D-0' DECISIONS.md | tail -1`. Write a new entry with these claims:
  - Materials are manifest-tracked, keyed by stable IDs, and render-side pipelines live alongside the built-in sprite pipeline with the same vertex/instance layouts; `@group(2)` is the 256-byte material UBO.
  - `PostStack` is a reorderable `Vec<PostPass>` resource; the 17 stock effects are a **closed enum** (matches `D-054` reasoning for tween easings). Extension requires a new enum variant + a new render-side pipeline. Advantage: no trait-object indirection, no runtime-variant registry, and serde is trivial.
  - `UniformOverrideBlock` is one component shared between M26 material animation and M32 MSDF outline/glow; `TweenChannel` grows uniform-slot variants. This narrows `D-055` (one-`Tween`-per-entity still holds; multi-property is still `Vec<TweenChannel>`) and does not introduce a `TweenTarget`.
  - Stock shaders are vendored (`D-015` rule 3), mirrored at compile time via `include_str!` and on-disk under `assets/shaders/stock/` so the manifest + M25 hot reload already cover them. No WGSL preprocessor.
  - SMAA is not a `PostPass`: see M27's decision for that split.
- Add the matching one-line row to `docs/DECISION_INDEX.md`.
- Verify: `rg -n "D-0NN" DECISIONS.md docs/DECISION_INDEX.md` returns the two hits (replace with resolved number).

### 22. AGENTS / DESIGN / CHANGELOG / LLM_INDEX sync
- Files: [AGENTS.md](AGENTS.md), [DESIGN.md](DESIGN.md), [CHANGELOG.md](CHANGELOG.md), [docs/LLM_INDEX.md](docs/LLM_INDEX.md).
- Action:
  - `AGENTS.md` §Asset Rules: add `Material` row alongside Sprite/Animation/Font/Sound (stable ID, required `shader` field). Add a note that `assets/shaders/stock/` mirrors vendored LYGIA-derived sources kept byte-equal with compile-time `include_str!` bodies.
  - `DESIGN.md` §Hot Reload matrix: add `material` row (body-only; manifest-add covered). §Status: note `PostStack` + material pipeline are live.
  - `CHANGELOG.md` Unreleased: "M26: material pipelines, 17-effect post-stack, tween→uniform bridge."
  - `docs/LLM_INDEX.md`: add a subsystem row pointing to `crates/tungsten-render/src/material.rs`, `crates/tungsten-render/src/post/`, `crates/tungsten-core/src/assets/material.rs`, and `crates/tungsten-core/src/post.rs`.
- Verify: `rg -n 'material|PostStack' AGENTS.md DESIGN.md CHANGELOG.md docs/LLM_INDEX.md` returns the new rows; spot-check `cargo test --workspace` that the `DECISION_INDEX` coverage test (see `D-053` precedent) still passes.

### 23. Flip plan status
- File: [docs/plans/phase4-milestone-26-materials-post-stack.md](docs/plans/phase4-milestone-26-materials-post-stack.md).
- Action: `status: draft` → `status: done` once steps 1–22 land on `0.23`.
- Verify: `rg -n "^status:" docs/plans/phase4-milestone-26-materials-post-stack.md` shows `done`.

## Verification Commands Summary

```bash
cargo fmt --all
cargo build --workspace
cargo test --workspace                          # Layer 1 + unit (incl. new material/post/tween tests)
cargo clippy --workspace --all-targets          # advisory
./scripts/smoke-examples.sh                     # Layer 2, includes post-stack + msaa matrix

# M25 byte-identity regression (post stack must stay empty by default):
TUNGSTEN_CAPTURE_FRAME=60 TUNGSTEN_CAPTURE_PATH=/tmp/m26-default.png \
    cargo run -p example-02-sprite-stress
#  → image_diff against docs/showcase/m25-sprite-stress-baseline.png (tolerance 1)

# 17-effect fixture smoke:
TUNGSTEN_POST_STACK_FIXTURE=all TUNGSTEN_SMOKE_FRAMES=3 \
    cargo run -p example-04-shader-playground

# Material hot-reload smoke (with example-04 running):
sed -i 's/strength\s*=\s*0\.5/strength = 0.9/' assets/shaders/stock/vignette.wgsl
#  → live visual change, log line "shader 'vignette' reloaded"
git restore assets/shaders/stock/vignette.wgsl
```

## Sources

- phase4.md §M26 (canonical scope)
- [docs/plans/archive/phase4-milestone-25-render-foundation.md](docs/plans/archive/phase4-milestone-25-render-foundation.md) (target pool / pass-order / shader-cache shape)
- [renderer.rs:1-300](crates/tungsten-render/src/renderer.rs#L1-L300), [renderer.rs:520-750](crates/tungsten-render/src/renderer.rs#L520-L750)
- [sprite.rs:1-180](crates/tungsten-render/src/sprite.rs#L1-L180), [sprite.rs:556-600](crates/tungsten-render/src/sprite.rs#L556-L600)
- [sprite_extract.rs](crates/tungsten/src/sprite_extract.rs)
- [examples/01_platformer/src/extract.rs](examples/01_platformer/src/extract.rs)
- [tween.rs](crates/tungsten-core/src/tween.rs), [tweens.rs](crates/tungsten/src/tweens.rs)
- [manifest.rs:36-365](crates/tungsten-core/src/assets/manifest.rs#L36-L365)
- [shader.rs](crates/tungsten-core/src/assets/shader.rs), [shader_hot_reload.rs](crates/tungsten-render/src/shader_hot_reload.rs)
- [passes/order.rs](crates/tungsten-render/src/passes/order.rs), [passes/desc.rs](crates/tungsten-render/src/passes/desc.rs), [targets.rs](crates/tungsten-render/src/targets.rs)
- [app.rs:353-489](crates/tungsten/src/app.rs#L353-L489) (hot-reload routing)
- [asset_loader.rs:308-380](crates/tungsten/src/asset_loader.rs#L308-L380) (load_all order)
- `DECISIONS.md` `D-016`, `D-017`, `D-042`, `D-053`, `D-054`, `D-055`, `D-056`, `D-057` (grep, do not read serially)
- [LYGIA shader library](https://lygia.xyz/) (MIT) — vendored under `crates/tungsten-render/src/shaders/stock/lygia/`
