---
name: tungsten-wgpu
description: Tungsten renderer, WGSL shader and GPU-resource work in native wgpu-rs — pipelines, bind groups, vertex/instance layouts, sprite/quad/text paths, post-processing, the core/render seam. Not Three.js, TSL or browser WebGPU.
---

# tungsten-wgpu

Read `crates/tungsten-render/AGENTS.md` first; it holds the crate's shader, frame-order and feature rules. This skill adds working guidance.

## Stack

- Native `wgpu` plus hand-written WGSL. No Three.js, TSL, GLSL or HLSL.
- Entry points: [renderer.rs](../../../crates/tungsten-render/src/renderer.rs) and [lib.rs](../../../crates/tungsten-render/src/lib.rs). Pipelines live in `sprite.rs`, `lit_sprite.rs`, `quad.rs`, `text.rs`, `material.rs`, `debug_line.rs` and `post/`.

## Shaders (D-057 to D-061)

- Manifest-tracked shaders live under `assets/shaders/` and hot-reload on body edits after `wgpu::naga` validation (`validate_wgsl_source`). Signature or bind-group layout changes still need a rebuild. `D-023`'s "every shader edit rebuilds" rule is narrowed, not current.
- Stock effects are `include_str!`ed from `crates/tungsten-render/src/shaders/stock/` and mirrored byte-equal in `assets/shaders/stock/`; `src/sprite.wgsl` mirrors `assets/shaders/sprite.wgsl`. Edit both copies of a pair together.
- Internal-only shaders (`quad.wgsl`, `debug_line.wgsl`, `shaders/present_blit.wgsl`) aren't manifest-tracked and still need a rebuild.
- Validate new or changed WGSL with `cargo test -p tungsten-render`. Naga success doesn't prove pixels; run the smoke and visual checks on a GPU.

## Core/render seam (D-007, D-016, D-018)

- `TextureHandle(u32)` is defined in `tungsten-core`. **No `wgpu` types in `tungsten-core`.** Render may depend on core, never the reverse.
- The umbrella mediates: `AssetRegistry::register_sprite` allocates a handle and stores metadata in core, then `renderer.upload_texture(handle, rgba, …)` stores the GPU texture under the same key.
- Extract → draw: systems mutate `World`; extract functions produce POD slices (`QuadInstance`, `SpriteInstance`, `TextSection`) for render. The renderer never takes mutable `World` at draw time.

If a change needs the renderer to mutate `World` or handle string IDs at draw time, stop: the seam is breaking.

## Layout and resources

- Bind group layouts are hand-written in Rust; no codegen. Every GPU-uploaded struct is `bytemuck::Pod + Zeroable` (D-020).
- Materials, SMAA and bloom share the 256-byte UBO contract; lighting's `LightUbo` is 544 bytes. Keep Rust and WGSL layouts in lockstep and cover them with a layout test.

## Present mode and GPU timing

- `display.present_mode` in `tungsten.json` wins when set (it falls back to `render.present_mode`); defaults are `"auto"` with `max_frame_latency = 1`. The Vulkan pacing matrix is in [profiling-workflow.md](../../../docs/perf/profiling-workflow.md). Don't change shipped defaults to win a benchmark; record `--telemetry-only` override rows.
- `TUNGSTEN_GPU_TIMING=1` enables timestamp queries (`GpuFrameTimings::frame_gpu_ms`, `None` without adapter support). The readback blocks, so never enable it during CPU profiling.

## Reference material

Use versioned sources that match `Cargo.lock`: `https://docs.rs/wgpu/<version>/`, the wgpu release notes and changelog on GitHub, and the W3C WGSL specification. Check the locked version with `cargo tree -i wgpu` before relying on an API.

## Dependencies (D-015)

A new graphics dependency (WGSL preprocessor, texture crate, `wgpu-profiler`, …) must satisfy a D-015 rule: it abstracts a platform API, implements a well-specified format, or provides a solved math/primitive. It also needs a new entry in [DECISIONS.md](../../../DECISIONS.md) before the `Cargo.toml` edit. No external rendering or engine crates (`bevy`, `rend3`, …).

## Do not

- Write TSL, GLSL or HLSL, or reach for Three.js patterns (node materials, pass helpers).
- Add mutable `World` access at draw time or `wgpu` types to `tungsten-core`.
- Let a shader mirror drift from its compiled-in copy.
- Add a dependency to fix shader-authoring ergonomics without a D-015 justification.
