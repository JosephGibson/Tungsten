---
name: tungsten-wgpu
description: Tungsten renderer, WGSL shader and GPU-resource work in native wgpu-rs — pipelines, bind groups, vertex/instance layouts, sprite/quad/text paths, post-processing, the core/render seam. Not Three.js, TSL or browser WebGPU.
---

# tungsten-wgpu

Read [renderer AGENTS.md](../../../crates/tungsten-render/AGENTS.md) first; it owns shader mirrors, frame order, feature contracts and GPU checks. [DESIGN.md](../../../DESIGN.md#hot-reload--m9) owns the current reload support matrix. This skill adds seam and resource guidance.

## Stack

- Native `wgpu` plus hand-written WGSL. No Three.js, TSL, GLSL or HLSL.
- Entry points: [renderer.rs](../../../crates/tungsten-render/src/renderer.rs) and [lib.rs](../../../crates/tungsten-render/src/lib.rs). Pipelines live in `sprite.rs`, `lit_sprite.rs`, `quad.rs`, `text.rs`, `material.rs`, `debug_line.rs`, `mesh_particle.rs` and `post/`.

## Core/render seam (D-007, D-016, D-018)

- `TextureHandle(u32)` is defined in `tungsten-core`. **No `wgpu` types in `tungsten-core`.** Render may depend on core, never the reverse.
- The umbrella mediates: `Renderer::allocate_texture_handle` mints handles (`D-048`); the loader uploads pixels and passes the same handle, atlas UVs and metadata to `AssetRegistry::register_sprite`. Several sprites can share one atlas handle.
- Extract → draw: systems mutate `World`; extract functions produce plain render data (`QuadInstance`, `SpriteInstance`, `TextSection`) for render. The renderer never takes mutable `World` at draw time.

Resolve asset IDs during extract and pass resolved render data across the seam. A change that needs mutable `World` access at draw time must first reconcile `D-018`.

## Layout and resources

- Bind group layouts are hand-written in Rust; no codegen. Every GPU-uploaded struct is `bytemuck::Pod + Zeroable` (D-020).
- Materials, SMAA and bloom share the 256-byte UBO contract; lighting's `LightUbo` is 544 bytes. Keep Rust and WGSL layouts in lockstep and cover them with a layout test.

## Pacing, timing and checks

[The profiling workflow](../../../docs/perf/profiling-workflow.md#frame-pacing) owns display/render precedence, child-only pacing overrides and capture defaults. Override studies use `just perf run … --present-mode … --max-frame-latency …`; `--gpu-timing off` disables the diagnostic run.

`TUNGSTEN_GPU_TIMING=1` blocks on readback; keep it out of CPU profiles. Renderer changes require the shader/layout tests and GPU checks in renderer AGENTS. Cache validation alone does not establish correct pixels or visible hot reload; use the current support matrix and connected pipeline rebuild path.

## Reference material

Use versioned sources that match `Cargo.lock`: `https://docs.rs/wgpu/<version>/`, the wgpu release notes and changelog on GitHub, and the W3C WGSL specification. Check the locked version with `cargo tree -i wgpu` before relying on an API.

## Dependencies (D-015)

A new graphics dependency (WGSL preprocessor, texture crate, `wgpu-profiler`, …) must satisfy a D-015 rule: it abstracts a platform API, implements a well-specified format, or provides a solved math/primitive. It also needs a new entry in [DECISIONS.md](../../../DECISIONS.md) before the `Cargo.toml` edit. No external rendering or engine crates (`bevy`, `rend3`, …).

## Do not

- Write TSL, GLSL or HLSL, or reach for Three.js patterns (node materials, pass helpers).
- Add mutable `World` access at draw time or `wgpu` types to `tungsten-core`.
- Let a shader mirror drift from its compiled-in copy.
- Add a dependency to fix shader-authoring ergonomics without a D-015 justification.
