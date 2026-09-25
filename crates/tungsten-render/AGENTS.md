# tungsten-render

Scoped rules for this crate, on top of the root `AGENTS.md`. The `tungsten-wgpu` skill has deeper wgpu/WGSL guidance.

## Shaders (`D-057`, narrows `D-023`)

- Runtime shaders are manifest-tracked assets under `assets/shaders/`, loaded through `ShaderRegistry` + `ShaderModuleCache`.
- Mirrors: `src/shaders/stock/**` ↔ `assets/shaders/stock/**` and `src/sprite.wgsl` ↔ `assets/shaders/sprite.wgsl` stay byte-equal; edit both copies together. The loader skips a reload when bytes match the compiled-in source. `lit_sprite.wgsl` exists only in `assets/shaders/`. `quad`, `debug_line` and `present_blit` are internal and not manifest-tracked.
- Body edits hot-reload through the umbrella watcher after `wgpu::naga` validation (`validate_wgsl_source`, `Renderer::reload_shader`). Signature or bind-group layout changes need a rebuild.
- LYGIA helpers under `stock/lygia/` keep their MIT headers.

## Frame order

`Scene → PostStack → [SMAA tail → PresentSource] → Text Overlay → Present Blit → Swapchain`. Text always draws after presentation AA, so SMAA never samples it.

Surface acquire decisions (reconfigure, recreate, skip, fail) live in the GPU-free `src/surface_acquire.rs` with transition tests; change the policy there, not inline in `renderer.rs`.

## Features

- **Materials** (`D-058`): manifest `materials` entries get a `MaterialAssetId` and a 256-byte UBO matching `UniformOverrideBlock`. `MaterialPipeline` reuses the sprite layout (groups 0/1) and adds group 2. No per-material files; `uniform_defaults` reload with the manifest. `PostStack::default()` is empty and byte-identical to the pre-M26 frame.
- **SMAA** (`D-059`): three manifest-tracked stage shaders. `area`/`search` LUTs are `include_bytes!` engine content in `src/assets/smaa/` (MIT, not manifest-tracked). `render.post_aa` / `TUNGSTEN_RENDER_POST_AA`; runtime switches go through `tungsten::request_post_aa` at a frame boundary. Changing `msaa` still needs a relaunch.
- **Bloom** (`D-060`): the 18th `PostPass` variant. It records its own threshold, downsample, upsample and composite passes instead of `PassRecorder::begin`. The `Rgba16Float` pyramid lives on `SceneTarget`, sized by `bloom_mip_count_for_size` (`render.bloom_max_mips`, default 6, 1..=8, startup-only). `SceneColor` stays sRGB.
- **Lighting** (`D-061`): `LitSpritePipeline` runs inside the scene pass; `LightingResources` owns a 544-byte `LightUbo` at group 2 (`LIGHT_CAP = 16`). Normal/emissive siblings pack into parallel atlas pages keyed by the albedo `TextureHandle`. `emissive_mask` and `rim_light` are validated helpers with no pipeline.
- **Screenshots/debug** (`D-047`): offscreen `RENDER_ATTACHMENT | COPY_SRC` texture read back through a row-padded `MAP_READ` buffer; debug groups and labels always on.

## Checks

- `cargo test -p tungsten-render`: layout tests plus `tests/shader_coverage.rs`, which Naga-validates every `.wgsl` in both trees and checks each mirror pair byte for byte. A new LYGIA fragment that calls a sibling needs a `FRAGMENT_DEPS` entry.
- GPU changes: `WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh` and the visual test from the root `AGENTS.md`. Naga success doesn't prove pixels; Vulkan success doesn't certify Metal or DX12.
- Don't replace golden images to make a test pass; explain the pixel difference first.
