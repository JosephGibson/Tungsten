# tungsten-render

Rules over the root `AGENTS.md`; the `tungsten-wgpu` skill adds more.

## Shaders (`D-057`, narrows `D-023`)

- Runtime shaders are manifest-tracked assets under `assets/shaders/`, loaded through `ShaderRegistry` + `ShaderModuleCache`.
- Mirrors: `src/shaders/stock/**` ↔ `assets/shaders/stock/**` stay byte-equal; edit both copies together. The loader skips a reload when bytes match the compiled-in source. `sprite.wgsl` and `lit_sprite.wgsl` exist only in `assets/shaders/`, which the renderer compiles in. `quad`, `debug_line`, `mesh_particle`, `ui` and `present_blit` are internal, not manifest-tracked.
- Body edits use the umbrella watcher and Naga validation; signature/layout changes rebuild. Pipeline coverage and manifest-add limits are in [the reload matrix](../../DESIGN.md#hot-reload--m9).
- LYGIA helpers under `stock/lygia/` keep their MIT headers.

## Frame order

`Scene → PostStack → [SMAA tail] → Overlay` (`D-087`, `D-147`, `D-161`), declared in `src/graph/frame.rs` (`D-305`): a pass names what it samples and writes (`Clear`, `Whole`, `Over`). Compiling derives the swapchain's writer, loads, stores, pooled targets and the acquire split. `Whole` is only a full-screen pass on the engine's own shader, which never discards; the swapchain's first one loads `DontCare`, the crate's one `unsafe` (`D-306`). Add passes there, not in `renderer.rs`. A stack's last two stages draw as one pass when the last only mixes its pixel (`post/compose.rs`, `D-307`). The overlay (timing label `text`) draws world labels, the UI paint list, then text, after presentation AA.

A capture frame (`Renderer::capture_frame`, `TUNGSTEN_CAPTURE_FRAME`) is that declaration with its last version marked copied-from: it stays offscreen and a blit shows it. A new stage must pass `graph_frame`'s matrix. `TUNGSTEN_CAPTURE_DIRECT=1` captures the direct path.

Surface acquire policy lives in the GPU-free `src/surface_acquire.rs`; change it there.

## Features

- **Materials** (`D-058`): manifest `materials` entries get a `MaterialAssetId`; each batch in a frame gets a 256-byte UBO slot matching `UniformOverrideBlock` (`D-101`). `MaterialPipeline` reuses the sprite layout (groups 0/1) and adds group 2.
- **SMAA** (`D-059`): three manifest-tracked stage shaders. `area`/`search` LUTs are `include_bytes!` engine content in `src/assets/smaa/` (MIT, not manifest-tracked). `render.post_aa` / `TUNGSTEN_RENDER_POST_AA`; runtime switches go through `tungsten::request_post_aa` at a frame boundary. `msaa` changes need a relaunch.
- **Bloom** (`D-060`): a `PostPass` variant declared as its own passes over a pooled `Rgba16Float` pyramid, sized by `render.bloom_base` (`D-301`) and `bloom_max_mips`, startup-only.
- **HDR** (`D-309`): under `render.hdr` (startup-only) the scene and every colour version before the last are float; the last full-screen pass writes 8 bits through the composer's tail. Default `off`: sRGB.
- **Lighting** (`D-061`): `LitSpritePipeline` runs in the scene pass; `LightingResources` owns a 544-byte `LightUbo` at group 2 (`LIGHT_CAP = 16`; `lights_view` drops lights lighting no pixel). Normal/emissive siblings pack into parallel atlas pages keyed by the albedo `TextureHandle`. `emissive_mask` and `rim_light` are validated helpers, no pipeline.
- **Depth sort** (`D-297`): under `gpu_depth` a batch's depth is its place and solid batches draw first; `fs_solid` must draw what `fs_main` draws.
- **Screenshots/debug** (`D-047`): an offscreen `COPY_SRC` target read back through a row-padded `MAP_READ` buffer; debug groups always on.

## Checks

- `cargo test -p tungsten-render`: layout tests plus `tests/shader_coverage.rs`, which Naga-validates every `.wgsl` in both trees and compares each mirror pair's bytes. A new LYGIA fragment that calls a sibling needs a `FRAGMENT_DEPS` entry.
- GPU changes: `just smoke` and `just visual` ([validation](../../docs/validation.md)). Naga success doesn't prove pixels; Vulkan success doesn't certify Metal or DX12.
- Never replace a golden image to pass a test; explain the pixel difference first.
