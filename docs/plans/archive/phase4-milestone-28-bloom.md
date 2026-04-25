---
status: done
milestone: M28
goal: "Ship M28 Bloom: a `PostPass::Bloom(BloomParams { threshold, knee, intensity, radius })` variant that runs a Karis-averaged 13-tap downsample + 9-tap tent upsample mip-chain against an `Rgba16Float` `BloomPyramid`, then writes `src + bloom` into the `PostPing`/`PostPong` slot the post stack assigns it. Default-empty `PostStack` keeps the M27 frame byte-identical; mip count clamps by `render.bloom_max_mips` (default 6) and viewport size."
non-goals:
  - "No HDR `SceneColor`. The scene attachment stays swapchain-sRGB; the pyramid is the only `Rgba16Float` allocation. Phase 5 may revisit when emissives outgrow the LDR ceiling."
  - "No shared optimization for multiple `PostPass::Bloom` slots. Multiple instances remain legal, but each invocation overwrites/rebuilds the pyramid for its own slot."
  - "No 2D lighting (M29), parallax/shake (M30), mesh particles/transitions (M31), or MSDF text (M32)."
  - "No new `TargetId` for the pyramid — `BloomPyramid` lives on `SceneTarget` and exposes `mip_view(level)` accessors; pass routing for the bloom slot still names `PostPing` / `PostPong` per the M26 ladder."
  - "No reorderable bloom sub-passes (threshold/downsample/upsample/composite are an internal implementation detail, never user-visible)."
  - "No SMAA-style fixed tail — bloom remains a reorderable `PostPass` so users can place it before tonemap (correct for HDR-style stacks) or after (cheap stylized stacks)."
  - "No new dependency. `wgpu`, `bytemuck`, `glam` already cover the pipeline + UBO + math surface (`D-015` satisfied without a new rule)."
  - "No bloom hot reload of `bloom_max_mips` — startup config + env-only, like `msaa`."
  - "No GIF/screenshot automation; acceptance artifacts are committed PNGs."
files to touch:
  - "crates/tungsten-core/src/config.rs                          — add `bloom_max_mips: u32`, `BLOOM_MAX_MIPS_EXPECTED`, `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` env override"
  - "crates/tungsten-core/src/post.rs                            — `BloomParams`, `PostPass::Bloom(BloomParams)`, `kind_name` arm, default presets"
  - "crates/tungsten-core/src/lib.rs                             — re-export `BloomParams`"
  - "crates/tungsten-core/src/tests/post.rs                      — `Bloom` serde round-trip + `kind_name` coverage"
  - "crates/tungsten-core/src/tests/config.rs                    — default + parse + env override for `bloom_max_mips`"
  - "crates/tungsten-render/src/targets.rs                       — `BloomPyramid` struct, allocator, accessors, `BLOOM_PYRAMID_FORMAT`, resize / shape-change wiring"
  - "crates/tungsten-render/src/passes/recorder.rs               — no `TargetId` arm for bloom mips; document that pyramid views are addressed directly by `BloomPipeline`"
  - "crates/tungsten-render/src/post/mod.rs                      — `pub mod bloom;`, `bloom: BloomPipeline` field on `PostStackRenderer`, encoder-level `record_bloom_slot` sibling"
  - "crates/tungsten-render/src/post/bloom.rs                    — `BloomPipeline { threshold, downsample, upsample, composite }`, `UniformOverrideBlock` packing helper, `record_pass` (encoder-level, multi-subpass)"
  - "crates/tungsten-render/src/shaders/stock/bloom_threshold.wgsl"
  - "crates/tungsten-render/src/shaders/stock/bloom_downsample.wgsl"
  - "crates/tungsten-render/src/shaders/stock/bloom_upsample.wgsl"
  - "crates/tungsten-render/src/shaders/stock/bloom_composite.wgsl"
  - "crates/tungsten-render/src/renderer.rs                      — `bloom_max_mips`, `bloom_shader_ids`, pre-seed `ShaderModuleCache` with the four bloom WGSL strings, route `upload_shader`/`reload_shader` to `BloomPipeline::rebuild_stage_with_module`, pre-branch `PostPass::Bloom` before opening the slot render_pass"
  - "crates/tungsten-render/src/tests/post.rs                    — keep `plan_targets` / final-target tests aligned with 18-pass roster"
  - "crates/tungsten-render/src/tests/bloom.rs                   — new; UBO size / packing, Karis weight numerics, mip-count clamp, shader-id seeding"
  - "crates/tungsten-render/src/tests/passes_order.rs            — assert `PostPass::Bloom` slot still emits one `PassDesc` writing to `PostPing`/`PostPong` per the M26 ladder"
  - "crates/tungsten/src/app.rs                                  — no new resource (Bloom rides `PostStack`); thread `config.render.bloom_max_mips` through to `Renderer::new`"
  - "assets/shaders/stock/bloom_threshold.wgsl                   — byte-equal mirror of compile-time include"
  - "assets/shaders/stock/bloom_downsample.wgsl                  — mirror"
  - "assets/shaders/stock/bloom_upsample.wgsl                    — mirror"
  - "assets/shaders/stock/bloom_composite.wgsl                   — mirror"
  - "assets/manifest.json                                        — append `shaders.bloom_threshold/downsample/upsample/composite` entries"
  - "tungsten.json                                               — document `render.bloom_max_mips` (default `6`)"
  - "input.json                                                  — add `playground_toggle_bloom`, `playground_bloom_threshold_inc/dec`, `playground_bloom_intensity_inc/dec`, `playground_bloom_radius_inc/dec` action bindings"
  - "examples/04_shader_playground/src/main.rs                   — `TUNGSTEN_BLOOM_FIXTURE=on|off` env pin, Bloom toggle key, threshold/intensity/radius live-tune keys, HUD row, bright emissive quad sprite for the demo"
  - "examples/04_shader_playground/assets/manifest.json          — register the bright `emissive_quad` sprite"
  - "examples/04_shader_playground/assets/emissive_quad.png      — small fully-white sprite (acts as the bloom source for the LDR demo fixture)"
  - "scripts/smoke-examples.sh                                   — append `TUNGSTEN_BLOOM_FIXTURE=on TUNGSTEN_POST_STACK_FIXTURE=bloom_only` row alongside the existing post-stack and post-aa fixture rows"
  - "docs/showcase/bloom_off_vs_on.png                           — 2-up still capture (off vs on with demo fixture params) committed under showcase"
  - "docs/showcase/README.md                                     — extend with M28 regeneration recipe (capture env vars + ImageMagick `+append`)"
  - "DECISIONS.md                                                — append `D-060` (M28 bloom)"
  - "docs/DECISION_INDEX.md                                      — one-line `D-060` row under Assets / Rendering"
  - "docs/LLM_INDEX.md                                           — Bloom subsystem row"
  - "AGENTS.md                                                   — add bloom shader bullet to the stock-shader paragraph"
  - "DESIGN.md                                                   — frame-order + hot-reload-matrix bullets for bloom; PostPass roster grows to 18"
  - "CHANGELOG.md                                                — `0.25` section: M28 bloom"
  - "README.md                                                   — flip M28 status row"
  - "docs/plans/phase4.md                                        — flip M28 section to `done — shipped in 0.25` and reference the archived plan path"
ordered steps:
  - "Step 1 — extend `RenderConfig` with `bloom_max_mips: u32` (default 6, range 1..=8), `BLOOM_MAX_MIPS_EXPECTED`, env override `TUNGSTEN_RENDER_BLOOM_MAX_MIPS`, `is_supported_bloom_max_mips`."
  - "Step 2 — extend `tungsten-core/src/post.rs` with `BloomParams { threshold, knee, intensity, radius }` (`#[serde(default)]` on the struct, matching existing params), append `PostPass::Bloom(BloomParams)`, add `kind_name` arm `\"bloom\"`."
  - "Step 3 — vendor four standalone WGSL stages under `crates/tungsten-render/src/shaders/stock/bloom_*.wgsl` (threshold + downsample + upsample + composite); mirror byte-equal under `assets/shaders/stock/`; register the four ids in `assets/manifest.json`."
  - "Step 4 — add `BloomPyramid` + `BLOOM_PYRAMID_FORMAT = Rgba16Float` to `targets.rs`; allocate mips on `SceneTarget::new` from `bloom_mip_count_for_size(width, height, bloom_max_mips)`; thread `bloom_max_mips` through `RenderTargetPool::new` / `resize` shape-change checks; add `mip_view(level)`, `mip_extent(level)`, `mip_count` accessors."
  - "Step 5 — implement `BloomPipeline` in `post/bloom.rs`: four `RenderPipeline`s built off the seeded `ShaderModuleCache`, `BloomShaderIds`, a `pack_params(...) -> UniformOverrideBlock` helper, and `record_pass(&mut encoder, pool, params, src: TargetId, dst: TargetId)` driving one threshold pass into mip 0, `mip_count - 1` downsample passes, `mip_count - 1` additive upsample passes (blend `One+One`), and one replace-blend composite into `dst`."
  - "Step 6 — wire `bloom: BloomPipeline` into `PostStackRenderer` as an encoder-level sibling to the 17 single-pass stock effects; in `render_frame_internal`, detect a bloom slot before `PassRecorder::begin`, call `record_bloom_slot(...)`, and `continue` the loop. All other slots stay on the existing open-render-pass path."
  - "Step 7 — extend `Renderer::new` to seed the four bloom shader ids in `shader_ids` + `ShaderModuleCache` from compile-time `include_str!` (ids 4–7 after sprite + SMAA, then bump `next_shader_id` to 8); route `upload_shader` and `reload_shader` to `BloomPipeline::rebuild_stage_with_module` for those names before the material rebuild branch. Validation failure keeps the prior pipeline live."
  - "Step 8 — rely on the existing manifest-driven `asset_loader::load_shaders` loop: adding the four `assets/manifest.json` entries is enough for `ShaderRegistry` path lookup and `.wgsl` watcher routing. Do not add a parallel stock-shader registration list."
  - "Step 9 — extend `examples/04_shader_playground`: parse `TUNGSTEN_BLOOM_FIXTURE=on|off` and `TUNGSTEN_POST_STACK_FIXTURE=bloom_only`; use demo-tuned bloom params for fixtures so the LDR scene has a visible halo; add `playground_toggle_bloom` (KeyL) + threshold/intensity/radius live-tune actions to `input.json`; add an emissive bright quad to the local manifest and spawn it; extend the HUD row list to render `bloom: <on/off> thr=… int=… rad=…`."
  - "Step 10 — add tests: `bloom_params_serde_round_trip`, `bloom_pack_writes_expected_slots`, `bloom_pyramid_mip_count_clamp`, `uniform_override_block_payload_is_256_bytes`, `bloom_shader_ids_seeded`, and a pass-order regression proving a one-slot post stack still writes to `PostPing`. Extend `scripts/smoke-examples.sh` with the new `TUNGSTEN_BLOOM_FIXTURE=on TUNGSTEN_POST_STACK_FIXTURE=bloom_only` row."
  - "Step 11 — author `D-060`, sync `docs/DECISION_INDEX.md`, `docs/LLM_INDEX.md`, `AGENTS.md`, `DESIGN.md` (frame-order paragraph + hot-reload matrix row + 18th `PostPass`); update `CHANGELOG.md` and `README.md` for `0.25`; capture `docs/showcase/bloom_off_vs_on.png` and document regen recipe."
  - "Step 12 — flip `docs/plans/phase4.md` M28 row to `done`; flip this plan's `status: draft → done`; move file to `docs/plans/archive/phase4-milestone-28-bloom.md`."
done-when:
  - "`cargo fmt --all && cargo test --workspace` passes on `0.25`."
  - "`./scripts/smoke-examples.sh` passes including the new `TUNGSTEN_BLOOM_FIXTURE=on TUNGSTEN_POST_STACK_FIXTURE=bloom_only` row and the existing M26/M27 rows."
  - "`cargo test -p tungsten-render passes_order` shows that an M27 baseline (`Off`, post stack `[]`) emits the exact `PassDesc` vector M27 shipped — bloom does not change the pass-list shape when bloom is not in the stack."
  - "`cargo test -p tungsten-render bloom` covers `UniformOverrideBlock` bloom packing (256-byte payload), Karis weighted-average normalization, mip-count clamp (`bloom_max_mips=6` on a 64×64 viewport clamps to 5), and `BloomShaderIds` seeding."
  - "`cargo test -p tungsten-core post` covers `PostPass::Bloom` serde round-trip and `kind_name == \"bloom\"`."
  - "`cargo test -p tungsten-core config` covers `bloom_max_mips` default = 6, valid range 1..=8, env override via `TUNGSTEN_RENDER_BLOOM_MAX_MIPS`, and rejection of `0` / `9` with `BLOOM_MAX_MIPS_EXPECTED`."
  - "`WGPU_BACKEND=vulkan cargo run -p example-04-shader-playground` toggles bloom with `L`, sliders update live, HUD reflects the active params; the emissive bright quad shows a visible halo with the demo-tuned fixture params."
  - "`WGPU_BACKEND=vulkan TUNGSTEN_BLOOM_FIXTURE=on cargo run -p example-04-shader-playground` boots, draws the haloed emissive quad, and exits cleanly under `TUNGSTEN_SMOKE_FRAMES=3`."
  - "Opt-in image-diff regression on the reference GPU: `TUNGSTEN_VISUAL_REGRESSION=1 cargo test -p example-02-sprite-stress --test visual_regression -- --nocapture` still passes. Empty `PostStack` and `PostAaMode::Off` keep the pass list and captured frame aligned with the M25/M26/M27 baseline."
  - "Manual hot-reload smoke: editing `assets/shaders/stock/bloom_upsample.wgsl` (body-only) while example-04 runs with `Bloom` in the stack updates the visible halo within ~200 ms; validation failure logs `shader 'bloom_upsample' validation failed: ...` and keeps the prior pipeline + frame."
  - "`docs/showcase/bloom_off_vs_on.png` committed; `docs/showcase/README.md` regen recipe references `TUNGSTEN_CAPTURE_FRAME`, `TUNGSTEN_CAPTURE_PATH`, and `TUNGSTEN_BLOOM_FIXTURE`."
  - "`DECISIONS.md` contains `D-060`; `docs/DECISION_INDEX.md` gains the matching one-line row; `docs/LLM_INDEX.md`, `AGENTS.md`, `DESIGN.md`, `CHANGELOG.md`, and `README.md` updated in the same change."
  - "`docs/plans/phase4.md` M28 row flipped to `status: done — shipped in 0.25`; this file flipped to `status: done` and moved to `docs/plans/archive/phase4-milestone-28-bloom.md`."
---

## Context Digest

| Slice | Current state (after M27, on `0.25`) |
| --- | --- |
| Frame order | `Scene [+ MSAA resolve] → N × PostPass (ping/pong) → [optional SMAA edge → blend → neighborhood → PresentSource] → text overlay → present blit → Swapchain`. Implemented in [`default_pass_order`](../../crates/tungsten-render/src/passes/order.rs) and [`Renderer::render_frame_internal`](../../crates/tungsten-render/src/renderer.rs#L814). |
| Post stack | [`PostStack(Vec<PostPass>)`](../../crates/tungsten-core/src/post.rs#L376) is a reorderable resource; 17 stock variants today. Each variant maps to one [`StockPipeline`](../../crates/tungsten-render/src/post/mod.rs#L63) inside [`PostStackRenderer`](../../crates/tungsten-render/src/post/mod.rs#L90). `record_pass` writes one fullscreen triangle per slot. |
| Target pool | [`SceneTarget`](../../crates/tungsten-render/src/targets.rs#L52) holds `color`, `depth?`, `color_msaa?`, `post_ping`, `post_pong`, optional `smaa: Option<SmaaTargets>`. All viewport-sized. `RenderTargetPool::resize` shape-checks `(size, msaa, depth_enabled, post_aa)`. |
| Shader path | `ShaderRegistry` (core) + `ShaderModuleCache` (render, [`shader_hot_reload.rs`](../../crates/tungsten-render/src/shader_hot_reload.rs)). `asset_loader::load_shaders` iterates `manifest.shaders`; there is no separate stock-shader registration list. Today `Renderer::upload_shader` / `reload_shader` validate via `naga`, then dispatch by name to sprite, SMAA stage, or material rebuild. M28 adds the bloom-stage dispatch branch. |
| Hot-reload matrix | `D-053` + `D-057` + `D-058` + `D-059`: shaders + materials body-only; LUTs out-of-matrix; manifest-add for sprites/animations/fonts/tilemaps/particles/shaders/materials. |
| Runtime app seam | User systems get `&mut World`; runtime renderer-state changes flow through `request_*` writers + `App::apply_pending_*` at frame boundary (`D-043` for display, `D-059` for `post_aa`). Bloom in M28 rides `PostStack` mutation directly — no new request path. |
| 04_shader_playground env pins | `TUNGSTEN_POST_STACK_FIXTURE` (`empty` / `all` / `retro_arcade` / `dreamy` / `glitch_boss`) + `TUNGSTEN_POST_AA_FIXTURE` (M27). M28 adds `TUNGSTEN_POST_STACK_FIXTURE=bloom_only` for smoke and `TUNGSTEN_BLOOM_FIXTURE=on|off` for side-by-side capture; if both are set, `bloom_only` owns the stack shape and the bloom fixture only tunes params. |
| Screenshot path | Reads back from the text-overlay target (`SceneColor` / final post target / `PresentSource`). Bloom does not change this — composite writes into the slot's `dst` (`PostPing` or `PostPong`), and a later overlay/present samples it as today. |

### Relevant `D-0xx` ids

- `D-007`, `D-016`, `D-018` — core/render seam invariants.
- `D-023` — shaders embedded; narrowed by `D-057`.
- `D-053` — hot-reload matrix; bloom shaders extend the matrix the same way SMAA stage shaders did.
- `D-054` — closed-enum precedent (extending `PostPass` is a four-point change: variant + pipeline + stock WGSL + roster row).
- `D-057` — shader assets + body-edit reload; bloom shaders follow the stock-shader pattern verbatim.
- `D-058` — `PostStack` is reorderable art-direction; bloom is a `PostPass`, not a fixed tail (contrast with `D-059`'s SMAA).
- `D-059` — SMAA tail precedent for vendoring + LUT/UBO conventions; bloom needs no LUT but reuses the 256-byte UBO contract.

### Bloom Algorithm (COD/Karis-style, single source)

```
                  threshold + soft knee
src slot sampled as linear color ─────► pyramid.mip[0]   (Rgba16Float)
                                  │
              13-tap Karis-avg downsample (×N-1)
                                  ▼
        pyramid.mip[1] → mip[2] → … → mip[N-1]
                                  │
              9-tap tent upsample, additive (One+One)
                                  ▼
        mip[N-1] → mip[N-2] → … → mip[0]
                                  │
                composite (lerp on `radius`, scale `intensity`)
                                  ▼
                              src + bloom → dst (PostPing/PostPong)
```

- **Karis average**: each downsample tap weight is `1 / (1 + luma(rgb))`, then the accumulated RGB is divided by the accumulated weight; this suppresses fireflies vs. a raw 13-tap box.
- **Knee**: COD-style soft-knee curve with the knee term clamped before squaring; handle `knee == 0` with a small epsilon so the shader never divides by zero.
- **Radius**: lerp factor between source slot and the upsampled mip 0 in the composite — `0` means scene-only, `1` means full bloom; default `1.0` in `BloomParams::default`.
- **Tent filter** on upsample is a 3×3 Hat with corner weight 1, edge 2, center 4 (sum 16).

### Pyramid Sizing

```
bloom_mip_count_for_size(width, height, max_mips):
    size_limit = floor(log2(max(1, min(width, height)))).saturating_sub(1).max(1)
    mip_count = max_mips.max(1).min(size_limit)
mip[i].extent = ((width  >> (i+1)).max(1), (height >> (i+1)).max(1))   // mip 0 is half resolution
```

`mip[0]` is half the viewport on each axis; this halves bandwidth versus a full-res pyramid and matches the COD/Frostbite "downsample by 2 starting at half" convention. The `+1` shift keeps mip 0 at half resolution; mip `N-1` at `1/(2^N)` resolution.

### Bloom UBO Packing (`UniformOverrideBlock`, 256 bytes)

| Slot | Field | Use |
| --- | --- | --- |
| `vec4[0]` | `inv_src_size: vec4<f32>` | `(1/src_w, 1/src_h, 0, 0)` per sub-pass |
| `vec4[1]` | `composite_tint: vec4<f32>` | `(1, 1, 1, 1)` reserved for tinted bloom (defaults to white) |
| `f32s[0]` | `threshold: f32` | bright-pass threshold |
| `f32s[1]` | `knee: f32` | soft-knee width |
| `f32s[2]` | `intensity: f32` | composite scale |
| `f32s[3]` | `radius: f32` | composite mix factor |
| `i32s[0]` | `mip_count: i32` | pyramid depth, ≤ `bloom_max_mips` |
| `i32s[1]` | `dst_level: i32` | downsample/upsample level being written |
| `i32s[2]` | `pass_kind: i32` | 0 threshold / 1 downsample / 2 upsample / 3 composite |

Use `UniformOverrideBlock::default()` / `to_bytes()` rather than introducing a second POD layout. The bloom pack helper writes only the slots above and leaves the reserved tail zeroed.

### Slot Recording — Why Bloom Bypasses the Auto-opened Render Pass

Every other `PostPass` is a single fullscreen triangle into one open `wgpu::RenderPass`. Bloom needs `1 + 2·(mip_count - 1) + 1` sub-passes — 2 when `mip_count = 1`, 12 when `mip_count = 6` — each into a different attachment (pyramid mip view or composite dst). That cannot share one open render pass.

Renderer change: in `render_frame_internal`, classify a post slot as bloom before calling `PassRecorder::begin`; if it is bloom, call `BloomPipeline::record_pass(&mut encoder, pool, params, src, dst)` and `continue`. The bloom routine opens and closes its own per-sub-pass `RenderPass`es. Pass-order labels for the bloom slot keep their `tungsten_post_pass_{i}` name on the outer slot description; sub-passes carry `tungsten_bloom_{stage}_{level}` debug groups for capture readability.

<assumptions>
- Pyramid format = `Rgba16Float` so bloom intermediates avoid 8-bit sRGB quantization during downsample/upsample even though the source scene stays LDR. Because `mip[0]` starts at half resolution, the 6-mip pyramid costs about one-third of a full-resolution `Rgba16Float` texture (≈ 5.5 MiB at 1920×1080, ≈ 22 MiB at 4K), which is acceptable for desktop M28.
- Threshold samples the slot source through its normal view. For sRGB scene/post attachments, wgpu's texture sampling already returns linear values; do not use the M27 non-sRGB SMAA twin or add a second manual `srgb_to_linear` conversion. If the source format is already linear, the same shader path works.
- Bloom slot mints a fresh sub-pass per mip; sub-passes use `LoadOp::Clear(Color::TRANSPARENT)` for downsample writes and `LoadOp::Load` for additive upsample passes (target is the previous mip).
- Composite uses replace blending and writes `src + bloom * intensity * radius` (or the equivalent `mix(src, src + bloom * intensity, radius)`) into `dst`; do not use target blending here, or stale contents in `PostPing`/`PostPong` can accumulate across frames. Upsample remains the only additive-blend stage.
- `bloom_max_mips` is startup-only. Runtime mutation would force a pyramid reallocation the request/apply seam isn't wired for in M28; users wanting fewer mips edit `tungsten.json` and relaunch, like `msaa`.
- `BloomPyramid` is allocated unconditionally on `SceneTarget::new` because `bloom_max_mips` is validated to `1..=8`. The pre-allocation removes the runtime allocation gate; bloom-not-in-stack frames still pay the bounded pyramid memory but skip every sub-pass.
- Allowing two `PostPass::Bloom` slots in the same stack is a non-feature: each invocation reuses the same pyramid and overwrites mip levels mid-frame. The plan does not guard against it; the second slot just produces a second bloom pass with whatever knobs it carried. M33's showcase will be reviewed with this in mind.
- Karis weight + knee curve constants ship as WGSL literals; no Rust-side constant table. The `UniformOverrideBlock` payload carries only per-frame knobs.
- Image-diff regression keeps M25 baseline because the empty-stack code path is unchanged: with `PostStack` empty and `post_aa = Off`, `BloomPyramid` exists but is never sampled or written.
- `PostPass` stays a closed enum after `Bloom` is appended (`D-054` reasoning continues to hold). M29+ may add further variants on the same precedent.
- No new dependency. `wgpu`, `bytemuck`, `glam` cover sub-pipelines + UBO packing + math (`D-015` satisfied via existing rule citations on those crates).
</assumptions>

---

## Step 1 — `bloom_max_mips` in `RenderConfig`

| File | Edit |
| --- | --- |
| [`crates/tungsten-core/src/config.rs`](../../crates/tungsten-core/src/config.rs) | Add private `fn default_bloom_max_mips() -> u32 { 6 }`, private `BLOOM_MAX_MIPS_EXPECTED = "an integer in 1..=8"`, public `pub const fn is_supported_bloom_max_mips(n: u32) -> bool`, and private `RENDER_BLOOM_MAX_MIPS_ENV = "TUNGSTEN_RENDER_BLOOM_MAX_MIPS"`. |
| same `RenderConfig` | `#[serde(default = "default_bloom_max_mips")] pub bloom_max_mips: u32,` and matching `Default::default()` row. |
| same `apply_env_overrides_from_env` | Branch: parse `RENDER_BLOOM_MAX_MIPS_ENV` into `u32`, validate via `is_supported_bloom_max_mips`, return `ConfigError::InvalidEnvOverride { var, value, expected: BLOOM_MAX_MIPS_EXPECTED }` on failure. Cite the exact existing pattern from the `RENDER_MSAA_ENV` branch. |
| same `Config::load` | Validate `bloom_max_mips` after JSON parse the same way `msaa` is validated; same `ConfigError::InvalidEnvOverride` shape. |
| [`crates/tungsten-core/src/tests/config.rs`](../../crates/tungsten-core/src/tests/config.rs) | Cases: default = 6; `bloom_max_mips = 4` parses; `bloom_max_mips = 0` and `9` reject; env `TUNGSTEN_RENDER_BLOOM_MAX_MIPS=3` flips the field; env `TUNGSTEN_RENDER_BLOOM_MAX_MIPS=junk` errors with `BLOOM_MAX_MIPS_EXPECTED`. |
| [`tungsten.json`](../../tungsten.json) | Add `"bloom_max_mips": 6` under `render` so contributors discover the key. |

## Step 2 — `BloomParams` + `PostPass::Bloom`

| File | Edit |
| --- | --- |
| [`crates/tungsten-core/src/post.rs`](../../crates/tungsten-core/src/post.rs) | Add `#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)] #[serde(default)] pub struct BloomParams { pub threshold: f32, pub knee: f32, pub intensity: f32, pub radius: f32 }`, matching the existing param structs. `impl Default` → `threshold = 1.0, knee = 0.5, intensity = 0.7, radius = 1.0`; the playground may override these for a stronger LDR demo. |
| same enum `PostPass` | Append `Bloom(BloomParams)` as variant 18; `kind_name` arm returns `"bloom"`. |
| [`crates/tungsten-core/src/lib.rs`](../../crates/tungsten-core/src/lib.rs) | Re-export `BloomParams` under the existing `pub use post::{...}` group. |
| [`crates/tungsten-core/src/tests/post.rs`](../../crates/tungsten-core/src/tests/post.rs) | Add `bloom_serde_round_trip` (`{ "kind": "bloom", "params": { "threshold": 1.2, "knee": 0.4, "intensity": 0.6, "radius": 0.85 } }`) and `bloom_kind_name`. |

## Step 3 — Vendored bloom WGSL + manifest mirrors

Standalone WGSL modules — no preprocessor, no shared `bloom_common.wgsl` (matches the M27 SMAA approach for the same loader-validates-one-file reason). Implement the shaders from scratch and cite the public bloom references in comments; do not claim vendored code or MIT attribution unless code is actually copied from an MIT source.

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/shaders/stock/bloom_threshold.wgsl`](../../crates/tungsten-render/src/shaders/stock/bloom_threshold.wgsl) | Standalone; samples source through the normal filterable view (sRGB attachments decode to linear automatically), applies the soft-knee bright-pass curve, writes mip 0. `@group(0) source_tex + sampler; @group(1) params_ubo`. Output format `Rgba16Float`. |
| [`crates/tungsten-render/src/shaders/stock/bloom_downsample.wgsl`](../../crates/tungsten-render/src/shaders/stock/bloom_downsample.wgsl) | 13-tap Karis-averaged box; reads `inv_src_size` from `vec4[0]`. Same bind layout as threshold. |
| [`crates/tungsten-render/src/shaders/stock/bloom_upsample.wgsl`](../../crates/tungsten-render/src/shaders/stock/bloom_upsample.wgsl) | 9-tap tent filter; outputs additive contribution (pipeline blend state does the `One+One`). |
| [`crates/tungsten-render/src/shaders/stock/bloom_composite.wgsl`](../../crates/tungsten-render/src/shaders/stock/bloom_composite.wgsl) | Reads slot `src` + `bloom_pyramid.mip[0]` via second bind group `@group(2)`; writes `mix(src, src + bloom*intensity, radius)` into `dst`. |
| All four | Header: `// Bloom — 13-tap Karis-weighted downsample + 9-tap tent upsample, implemented for Tungsten M28. References: Jimenez/COD 2014 and Karis firefly weighting.` |
| [`assets/shaders/stock/bloom_threshold.wgsl`](../../assets/shaders/stock/bloom_threshold.wgsl) | Byte-equal mirror of the compile-time include. |
| [`assets/shaders/stock/bloom_downsample.wgsl`](../../assets/shaders/stock/bloom_downsample.wgsl) | Mirror. |
| [`assets/shaders/stock/bloom_upsample.wgsl`](../../assets/shaders/stock/bloom_upsample.wgsl) | Mirror. |
| [`assets/shaders/stock/bloom_composite.wgsl`](../../assets/shaders/stock/bloom_composite.wgsl) | Mirror. |
| [`assets/manifest.json`](../../assets/manifest.json) | Append `"bloom_threshold"`, `"bloom_downsample"`, `"bloom_upsample"`, `"bloom_composite"` under `shaders`, paths `shaders/stock/bloom_*.wgsl`. |

## Step 4 — `BloomPyramid` allocator

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/targets.rs`](../../crates/tungsten-render/src/targets.rs) | Add `pub const BLOOM_PYRAMID_FORMAT: TextureFormat = TextureFormat::Rgba16Float;`. |
| same | `pub struct BloomPyramid { texture: wgpu::Texture, mip_views: Vec<wgpu::TextureView>, mip_extents: Vec<(u32, u32)> }`. `mip_views[i]` views level `i` only; composite samples `mip_views[0]`, not a whole-chain view. |
| same | `pub fn bloom_mip_count_for_size(width: u32, height: u32, max_mips: u32) -> u32` — clamp with saturating arithmetic (`floor_log2(min).saturating_sub(1).max(1)`) so tiny viewports never underflow; usage `RENDER_ATTACHMENT | TEXTURE_BINDING`; sample count 1; one texture with `mip_level_count = mip_count`. |
| same `SceneTarget` | Add `pub bloom_pyramid: BloomPyramid` and `pub bloom_max_mips: u32`. Do not use `Option`: config rejects `0`, and future disable behavior can add a new field/enum when it is real scope. |
| same `SceneTarget::new` | Take `bloom_max_mips: u32`; build `BloomPyramid` unconditionally after validation. |
| same `RenderTargetPool::new` / `resize` | Take `bloom_max_mips`; include it in `shape_changed` so a config change reallocates. Existing `post_aa` shape check stays. |
| same | Accessors: `bloom_mip_view(level: u32) -> Option<&TextureView>`, `bloom_mip_count() -> u32`, `bloom_mip_extent(level: u32) -> Option<(u32, u32)>`. |
| [`crates/tungsten-render/src/passes/recorder.rs`](../../crates/tungsten-render/src/passes/recorder.rs) | No `TargetId` arm needed — bloom mip views are addressed by `BloomPipeline::record_pass` directly (encoder-level), not via `PassRecorder::begin`. Document this with a one-line comment near the existing `resolve_view` match. |

## Step 5 — `BloomPipeline` in `post/bloom.rs`

```rust
// crates/tungsten-render/src/post/bloom.rs (new)

pub struct BloomShaderIds {
    pub threshold: ShaderAssetId,
    pub downsample: ShaderAssetId,
    pub upsample: ShaderAssetId,
    pub composite: ShaderAssetId,
}

pub struct BloomPipeline {
    threshold:  RenderPipeline,
    downsample: RenderPipeline,
    upsample:   RenderPipeline,
    composite:  RenderPipeline,
    layouts:    BloomLayouts,
    ubo: Buffer, ubo_bg: BindGroup,
    sampler: Sampler,
    shader_ids: BloomShaderIds,
}
```

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/post/bloom.rs`](../../crates/tungsten-render/src/post/bloom.rs) | New module with the structs above, `pub const BLOOM_THRESHOLD_SHADER_NAME = "bloom_threshold"` etc. (four constants). |
| same | `BloomLayouts { source_bgl, params_bgl, composite_bgl }` built in `build_layouts(device)` free fn; `composite_bgl` binds the bloom mip-0 view in addition to the slot-source group so the composite shader reads `src + pyramid.mip[0]`. |
| same | `impl BloomPipeline { pub fn new(device, format, cache: &ShaderModuleCache, ids: BloomShaderIds) -> Self; pub fn rebuild_stage_with_module(&mut self, device, format, id: ShaderAssetId, module: &ShaderModule); pub fn record_pass(&self, device, queue, encoder, pool, params: &BloomParams, src: TargetId, dst: TargetId); }`. |
| same `pack_params` | Return a `UniformOverrideBlock` with `vec4[0] = inv_src_size`, `vec4[1] = composite_tint`, `f32s = [threshold, knee, intensity, radius]`, and `i32s = [mip_count, dst_level, pass_kind, 0]`. Reuse `UniformOverrideBlock::to_bytes()` for the 256-byte upload. |
| same `record_pass` | Algorithm: (1) write threshold UBO + open render_pass into `pool.scene.bloom_mip_view(0)`, draw 3 verts; (2) for `level in 1..mip_count`: write downsample UBO with `inv_src_size = 1.0 / mip_extent(level - 1)`; open render_pass into mip `level`; sample mip `level - 1` via source bind; draw; (3) for `level in (0..mip_count - 1).rev()`: write upsample UBO with `inv_src_size = 1.0 / mip_extent(level + 1)`; open additive-blend render_pass into mip `level`; sample mip `level + 1`; draw; (4) write composite UBO; open render_pass into `dst` view (`PostPing` / `PostPong`) with replace blending; bind slot `src` + `pyramid.mip_view(0)`; draw. Each sub-pass tagged `encoder.push_debug_group("bloom_<stage>_<level>")` for capture clarity. |
| same | Threshold/downsample/upsample pipelines write `Rgba16Float`. Composite pipeline writes the swapchain format (matches `SceneColor`). Upsample pipeline blend state: `One + One` on color, `One + Zero` on alpha; threshold/downsample/composite blend state: replace. |
| [`crates/tungsten-render/src/post/mod.rs`](../../crates/tungsten-render/src/post/mod.rs) | `pub mod bloom;`. Add `pub(crate) bloom: BloomPipeline` to `PostStackRenderer`. Initialize in `new` (signature grows to take `&ShaderModuleCache, BloomShaderIds`; renderer pre-seeds the cache before constructing the post stack — see Step 7). |
| same `pipeline_for` | No change — bloom doesn't fit `StockPipeline`; it routes outside the per-pass dispatch in `record_pass`. |
| same `record_pass` | Existing signature takes `&mut RenderPass`. Keep it for the 17 stock variants. Add a sibling crate-private fn `pub(crate) fn record_bloom_slot(&self, device, queue, encoder, pool, params, src, dst)` that delegates to `self.bloom.record_pass`. The renderer calls one or the other depending on the variant. |

## Step 6 — Slot routing in `render_frame_internal`

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/renderer.rs`](../../crates/tungsten-render/src/renderer.rs) `render_frame_internal` post-loop | Before opening the slot render_pass for `post_index`, peek `post_stack.0[pi]`. When the variant is `PostPass::Bloom`, skip `PassRecorder::begin` for that index and call `self.post_stack.record_bloom_slot(&self.device, &self.queue, &mut encoder, &self.target_pool, p, src, dst)`. The slot's outer `PassDesc` from `default_pass_order` becomes a debug-only label — no render_pass is opened against it. |
| same | Pass-list indexing stays unchanged. The bloom slot still consumes one entry in `default_pass_order`'s `post_stack_len` count, so `text_overlay_idx` math in M27 stays correct. |
| [`crates/tungsten-render/src/passes/order.rs`](../../crates/tungsten-render/src/passes/order.rs) | No code change. Add a doc comment on the post-loop noting that the bloom variant is handled by the renderer at frame time, not by `default_pass_order`. |

## Step 7 — Renderer wiring + hot-reload routes

| File | Edit |
| --- | --- |
| [`crates/tungsten-render/src/renderer.rs`](../../crates/tungsten-render/src/renderer.rs) `Renderer` fields | Add `bloom_max_mips: u32` and `bloom_shader_ids: BloomShaderIds`. `BloomPipeline` itself lives inside `PostStackRenderer.bloom`. |
| same `Renderer::new` | Read `config.bloom_max_mips`; pre-seed the four bloom WGSL strings into `ShaderModuleCache` from compile-time `include_str!("shaders/stock/bloom_*.wgsl")` (same idiom as M27's SMAA pre-seed). Assign stable ids `ShaderAssetId(4..=7)` after sprite + SMAA, insert them into `shader_ids`, pass `bloom_max_mips` into `RenderTargetPool::new`, pass cache + ids into `PostStackRenderer::new`, and initialize `next_shader_id = 8`. |
| same `Renderer::resize` | Forward `self.bloom_max_mips` into `target_pool.resize`. |
| same `upload_shader` and `reload_shader` | After the existing SMAA branch and before the material rebuild, add: `if matches!(name, BLOOM_THRESHOLD_SHADER_NAME | BLOOM_DOWNSAMPLE_SHADER_NAME | BLOOM_UPSAMPLE_SHADER_NAME | BLOOM_COMPOSITE_SHADER_NAME) { self.post_stack.bloom.rebuild_stage_with_module(&self.device, self.surface_config.format, id, &module); }` — keeping the live pipeline on validation failure (last-known-good). |
| same | Material rebuild gate stays the same; bloom shader names are added to the `name != SPRITE_SHADER_NAME && !matches!(name, SMAA_* | BLOOM_*)` guard so editing a bloom shader does not trigger material rebuilds. |
| [`crates/tungsten/src/asset_loader.rs`](../../crates/tungsten/src/asset_loader.rs) | No code change expected: `load_shaders` already iterates every manifest shader entry, calls `registry.allocate(id, path)`, and forwards source to `renderer.upload_shader`. Revisit only if implementation discovers a hidden stock list. |
| [`crates/tungsten/src/app.rs`](../../crates/tungsten/src/app.rs) | No new resource. `Renderer::new` already takes `&RenderConfig`; bloom rides `PostStack` mutation through the existing world-resource path. |

## Step 8 — Tests

| File | Edit |
| --- | --- |
| [`crates/tungsten-core/src/tests/post.rs`](../../crates/tungsten-core/src/tests/post.rs) | `bloom_params_serde_round_trip` (with non-default values), `bloom_kind_name`, `bloom_default_matches_reference` (`threshold = 1.0, knee = 0.5, intensity = 0.7, radius = 1.0`). |
| [`crates/tungsten-core/src/tests/config.rs`](../../crates/tungsten-core/src/tests/config.rs) | `bloom_max_mips_default_is_six`, `bloom_max_mips_parses_in_range`, `bloom_max_mips_rejects_zero_and_nine`, `bloom_max_mips_env_override`. |
| [`crates/tungsten-render/src/tests/bloom.rs`](../../crates/tungsten-render/src/tests/bloom.rs) | New file. `uniform_override_block_payload_is_256_bytes`, `bloom_pyramid_clamps_max_mips_by_viewport` (`(64, 64, 6) → 5`, `(1024, 1024, 6) → 6`, `(2, 2, 6) → 1`), `bloom_shader_ids_are_stable`, `karis_weighted_average_renormalizes`, `bloom_pack_writes_expected_slots` (drives `BloomParams { threshold: 1.2, knee: 0.4, intensity: 0.6, radius: 0.85 }` and asserts the f32 slot bytes match). |
| [`crates/tungsten-render/src/tests/passes_order.rs`](../../crates/tungsten-render/src/tests/passes_order.rs) | Keep the existing `post_stack_one_splices_post_then_text_overlay_on_ping` coverage and add/rename a regression noting a bloom-only stack has the same `post_stack_len = 1` pass shape: one `tungsten_post_pass_0` writing into `PostPing`, then overlay + present. |
| [`scripts/smoke-examples.sh`](../../scripts/smoke-examples.sh) | Add a `bloom_pass=()`/`bloom_fail=()` block (mirror the M27 `post_aa_*` block) running `example-04-shader-playground` once with `TUNGSTEN_BLOOM_FIXTURE=on TUNGSTEN_POST_STACK_FIXTURE=bloom_only`. |
| `cargo test --workspace` | All non-GPU tests above run from the standard layer-1 path. |
| `./scripts/smoke-examples.sh` | Picks up the new fixture row; the M26 `all` row already covers the post-stack matrix containing `Bloom` once it's appended. |

## Step 9 — Shader-playground hotkeys + capture artifact

| File | Edit |
| --- | --- |
| [`examples/04_shader_playground/src/main.rs`](../../examples/04_shader_playground/src/main.rs) | Parse `TUNGSTEN_BLOOM_FIXTURE=on|off` (default `off`) and extend `TUNGSTEN_POST_STACK_FIXTURE` with `bloom_only`. When bloom is fixture-enabled, push `PostPass::Bloom(demo_bloom_params())` where `demo_bloom_params()` lowers threshold enough for an LDR white sprite to bloom visibly (for example `threshold = 0.85`, `knee = 0.35`, `intensity = 1.0`, `radius = 1.0`). |
| same | Reuse the existing `cycle_input_system` shape but avoid the current `KeyB` conflict (`post_prev`). Bind `playground_toggle_bloom` to `KeyL`; live-tune actions: `playground_bloom_threshold_inc/dec` (`Y/H`), `playground_bloom_intensity_inc/dec` (`U/J`), `playground_bloom_radius_inc/dec` (`I/K`). Each action mutates the in-stack `BloomParams` step ±0.05 and clamps threshold/intensity/radius to non-negative ranges. |
| same | Extend the HUD row list with `bloom: <on/off> thr=… int=… rad=…`. Reuse the existing HUD line-emit path. |
| [`input.json`](../../input.json) | Append the new `playground_bloom_*` action bindings. |
| [`examples/04_shader_playground/assets/manifest.json`](../../examples/04_shader_playground/assets/manifest.json) | Add `"emissive_quad": { "path": "emissive_quad.png", "filter": "linear" }` under `sprites`. |
| [`examples/04_shader_playground/assets/emissive_quad.png`](../../examples/04_shader_playground/assets/emissive_quad.png) | New 32×32 fully-white PNG. The example draws it at ordinary white against a dim background; the demo fixture lowers threshold because the scene is intentionally LDR and values above 1.0 are clipped before bloom in M28. |
| [`docs/showcase/bloom_off_vs_on.png`](../showcase/bloom_off_vs_on.png) | 2-up still PNG: `TUNGSTEN_BLOOM_FIXTURE=off` left, `TUNGSTEN_BLOOM_FIXTURE=on` right, captured from `example-04-shader-playground` via the existing `TUNGSTEN_CAPTURE_*` env path. |
| [`docs/showcase/README.md`](../showcase/README.md) | Add an M28 section mirroring the M27 SMAA section: regen recipe with `TUNGSTEN_CAPTURE_FRAME`, `TUNGSTEN_CAPTURE_PATH`, `TUNGSTEN_CAPTURE_RESOLUTION`, `TUNGSTEN_BLOOM_FIXTURE`; `convert _bloom_off.png _bloom_on.png +append bloom_off_vs_on.png`. |

## Step 10 — Decision entry + doc sync

| File | Edit |
| --- | --- |
| [`DECISIONS.md`](../../DECISIONS.md) | Append `## D-060 — M28 bloom`. **Decision:** `BloomParams { threshold, knee, intensity, radius }` is the 18th `PostPass` variant; bloom runs as a multi-sub-pass slot inside the reorderable post stack. The pyramid is `Rgba16Float`, sized by `bloom_mip_count_for_size(width, height, bloom_max_mips)`, and allocated unconditionally on `SceneTarget::new` because `bloom_max_mips` is validated to `1..=8`. The bloom slot is detected before `PassRecorder::begin` and records its own threshold + N×downsample + (N-1)×additive upsample + replace-blend composite chain through the encoder. Composite reads the slot `src` and writes `src + bloom` modulated by `radius` and `intensity`. **Why:** keeping bloom inside `PostStack` preserves art-direction reorderability (e.g. before vs after tonemap) — unlike SMAA's algorithmic constraint that locked it to a fixed tail in `D-059`. The pyramid is `Rgba16Float` because downsample/upsample through 8-bit sRGB targets quantizes badly; starting mip 0 at half resolution keeps the default 6-mip cost near one-third of a full-res `Rgba16Float` target (≈ 5.5 MiB at 1080p, ≈ 22 MiB at 4K). `D-058`'s "no HDR `SceneColor` in M26" stays — only the pyramid is HDR-capable. The encoder-level slot record is the smallest deviation from the per-slot single-render-pass model that fits a multi-stage algorithm. `bloom_max_mips` is startup-only because pyramid reallocation has no request/apply seam in M28; the runtime cost of editing it is the same as `msaa`. No new dependency. **Consequences:** `PostPass` grows to 18 variants; the M26 `D-058` "closed-enum" claim continues to hold (next addition follows the same four-point pattern). Extends `D-053` with four new body-edit-reloadable shader rows. Narrows neither `D-058` nor `D-059`; bloom is a `PostPass`, not a fixed tail. |
| [`docs/DECISION_INDEX.md`](../DECISION_INDEX.md) | Add Assets / Rendering row for `D-060`: `M28 bloom: BloomParams as 18th PostPass; Rgba16Float pyramid sized by bloom_max_mips × viewport; encoder-level slot recording for multi-pass; no HDR scene; no new dep.` |
| [`docs/LLM_INDEX.md`](../LLM_INDEX.md) | Add a Bloom (M28) row pointing to [`crates/tungsten-render/src/post/bloom.rs`](../../crates/tungsten-render/src/post/bloom.rs), [`crates/tungsten-render/src/targets.rs`](../../crates/tungsten-render/src/targets.rs), [`crates/tungsten-core/src/post.rs`](../../crates/tungsten-core/src/post.rs). |
| [`AGENTS.md`](../../AGENTS.md) | Under the materials/SMAA shader bullets, add: bloom stage shaders (`bloom_threshold`, `bloom_downsample`, `bloom_upsample`, `bloom_composite`) follow the stock-shader pattern; manifest-tracked, body-edit hot-reload via `Renderer::reload_shader` + `BloomPipeline::rebuild_stage_with_module`. Frame-order bullet stays the same — bloom is a reorderable `PostPass`. |
| [`DESIGN.md`](../../DESIGN.md) | Status block: note `Bloom` is the 18th `PostPass` and bloom is live in `0.25`. Hot-reload matrix: gain four `bloom_*` shader rows (body-only). Frame-order paragraph: optionally call out that the bloom slot internally records multiple sub-passes through the encoder. |
| [`CHANGELOG.md`](../../CHANGELOG.md) | New entry under `0.25`: `M28 — Bloom (PostPass::Bloom(BloomParams { threshold, knee, intensity, radius })) with Rgba16Float pyramid, 13-tap Karis downsample + 9-tap tent upsample. render.bloom_max_mips config (default 6). No new runtime deps. See D-060.` |
| [`README.md`](../../README.md) | Status block: mark M28 shipped. |
| [`docs/plans/phase4.md`](phase4.md) | Flip M28 row to `done — shipped in 0.25`; reference the archived plan path. |
| [`docs/plans/phase4-milestone-28-bloom.md`](phase4-milestone-28-bloom.md) | Flip front-matter `status: draft` → `status: done`; move file to `docs/plans/archive/phase4-milestone-28-bloom.md` on ship. |

## Risks / Unknowns

- **Slot bypass shape**. The encoder-level bloom slot is the first `PostPass` that does not record a single fullscreen pass into the slot's auto-opened `RenderPass`. If a future `PostPass` variant needs the same pattern, the renderer's slot dispatch should be lifted into a shared trait or enum. M28 keeps it inline; the comment in `render_frame_internal` flags the precedent.
- **Pyramid reallocation cost**. Resize → `BloomPyramid::new` allocates a fresh `Rgba16Float` texture with `mip_count` levels. At 4K + 6 half-res mips the steady allocation is ~22 MiB; continuous resize can churn that amount repeatedly. Acceptable for desktop; revisit if perf telemetry flags it under M30.
- **LDR ceiling**. Without HDR `SceneColor`, the threshold pass only sees values in roughly `0..=1` after sampling the sRGB target. The default API remains future-HDR-shaped, but the M28 playground fixture must lower threshold/intensify bloom to show a visible halo from ordinary white sprites.
- **Two-bloom stacks**. `[Bloom, Bloom]` is allowed; behavior is double-application with whatever knobs each carries. Document this in the `D-060` entry's "Consequences" so future authors know the cost is theirs.

## Sources

- [Jorge Jimenez — "Next Generation Post Processing in Call of Duty: Advanced Warfare"](https://www.iryoku.com/next-generation-post-processing-in-call-of-duty-advanced-warfare/) — Karis-averaged 13-tap downsample, tent upsample, additive accumulation.
- [Brian Karis — "Tone Mapping" GDC 2013 / SIGGRAPH 2014 notes](http://graphicrants.blogspot.com/2013/12/tone-mapping.html) — fireflies discussion behind the `1 / (1 + luma)` weighting.
- [Frostbite — Moving Frostbite to PBR](https://www.ea.com/frostbite/news/moving-frostbite-to-pb) — production PBR/bloom context and course-note pointer.
- [Unity-Technologies PostProcessing Bloom](https://github.com/Unity-Technologies/PostProcessing/wiki/Bloom) — artist-facing threshold / soft-knee / intensity vocabulary.
- [LearnOpenGL — Physically Based Bloom](https://learnopengl.com/Guest-Articles/2022/Phys.-Based-Bloom) — secondary reference for comparable GLSL structure; do not treat as canonical over project decisions.
- [wgpu 29.0.1 `BlendState`](https://docs.rs/wgpu/29.0.1/wgpu/struct.BlendState.html) — `BlendState::REPLACE` overwrites output; custom `One + One` blend is only for upsample accumulation.
- [wgpu 29.0.1 `TextureFormat`](https://docs.rs/wgpu/29.0.1/wgpu/enum.TextureFormat.html#variant.Rgba16Float) and [wgpu-types format feature table](https://wgpu.rs/doc/src/wgpu_types/texture/format.rs.html#989) — `Rgba16Float` is available as render attachment + texture binding in wgpu 29.
- Local source/docs researched for project fit: `docs/LLM_INDEX.md`, `docs/plans/phase4.md`, `docs/DECISION_INDEX.md`, `DECISIONS.md` entries `D-053`, `D-054`, `D-057`, `D-058`, `D-059`; `crates/tungsten-render/src/renderer.rs`, `targets.rs`, `post/mod.rs`, `passes/order.rs`; `crates/tungsten-core/src/post.rs`, `config.rs`; `examples/04_shader_playground/src/main.rs`; `scripts/smoke-examples.sh`.
