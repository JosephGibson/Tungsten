# Tungsten — Design

## Status

Workspace `v0.26.0` on branch `0.26`. Phase 3 is complete; all milestones `M12`–`M24` shipped and the rollout plan is archived at [`docs/plans/archive/phase3.md`](docs/plans/archive/phase3.md). Phase 4 scope is tracked in [`docs/plans/phase4.md`](docs/plans/phase4.md). M25 (`D-057`) is live: offscreen `SceneTarget` + ordered named-pass list (`scene` → `present`), optional MSAA (1/2/4/8) and opt-in GPU depth-test sprite path, WGSL shaders are manifest-tracked with body-edit hot reload via `wgpu::naga` validation. M26 (`D-058`) is live: manifest-tracked materials (WGSL pipeline + 256-byte UBO) selectable per `SpriteBatch`, a reorderable `PostStack` of 17 stock effects ping-ponging between `PostPing` / `PostPong` offscreen targets before the present blit, and `UniformOverrideBlock` + `TweenChannel::Uniform*` wiring a single entity-local animation surface shared with the future M32 MSDF outline/glow. M27 (`D-059`) is live: `render.post_aa` selects optional SMAA 1x presentation AA, fixed tail passes run after the post stack and before text overlay, and `post_aa = Off` keeps the default frame byte-identical to M26. M28 (`D-060`) is live: `PostPass::Bloom(BloomParams { threshold, knee, intensity, radius })` is the 18th `PostPass` variant, runs as a multi-subpass slot against an `Rgba16Float` `BloomPyramid` sized by `render.bloom_max_mips` (default 6), and keeps the empty-stack frame byte-identical to M27. M29 (`D-061`) is live: `Light` / `LightKind` components and an `AmbientLight(Vec3)` resource feed a 544-byte `LightUbo` (cap 16) bound at group 2 of a sibling `LitSpritePipeline`; sprites with manifest-tracked `normal_map` / `emissive_mask` siblings pack into parallel atlas pages keyed by the existing albedo `TextureHandle`, and an empty light list + no lit sprites keeps the captured frame byte-identical to the M28 baseline. Companion docs: [`AGENTS.md`](AGENTS.md) for operational rules, [`DECISIONS.md`](DECISIONS.md) for rationale by `D-NNN`.

## What It Is

Tungsten is a from-scratch Rust 2D game engine for native targets (`Linux`, `macOS`, `Windows`). The workspace has three crates: `tungsten-core`, `tungsten-render`, and `tungsten`. Scope limit: 2D only; no 3D math beyond what `glam` provides, no model formats, no skeletal animation, no PBR.

## Principles

1. Build from scratch where practical. No ECS crates, engine crates, or rendering helpers.
2. Use `wgpu` for rendering. Modern GPU API at a manageable level.
3. Keep game state ECS-first. Build the ECS in-project.
4. Keep content data-driven. Engine config, asset registration, and animation definitions live in JSON.

### Dependency Philosophy

A dependency is acceptable only if at least one D-015 rule applies.

| Rule | Applies To |
| --- | --- |
| 1. Platform API abstraction | `winit`, `wgpu`, `notify`, `cpal` |
| 2. Well-specified data format | `serde_json`, `image`, `symphonia` |
| 3. Math / primitive (solved problem) | `glam`, `bytemuck`, `pollster`, `rtrb` |

Reject crates that would hand over work this project is supposed to build. Examples: `bevy_ecs`, `hecs`, `rapier2d`, `rodio`. Borderline cases require a `DECISIONS.md` entry.

## Stack

| Concern | Choice | Role |
| --- | --- | --- |
| Windowing | `winit` | OS window and event abstraction |
| Rendering | `wgpu` | GPU API abstraction |
| Math | `glam` | Vectors, matrices, transforms |
| ECS | hand-rolled | Archetypal storage built in-project |
| Config | `serde` + `serde_json` | JSON schema derive + parsing |
| Image | `image` | PNG decoding to CPU bitmaps |
| Logging | `log` + `env_logger` | Standard facade + basic backend |
| Errors | `thiserror` / `anyhow` | Typed at library boundaries, anyhow at top level |
| Audio device | `cpal` | Platform audio API (`WASAPI` / `CoreAudio` / `ALSA`) |
| Audio decode | `symphonia` | `OGG` / `WAV` / `MP3` decode |
| File watch | `notify` | Platform file-change events |
| Audio ring | `rtrb` | Wait-free SPSC ring for audio commands |
| Text | `glyphon` + `cosmic-text` | `TrueType`/`OpenType` layout + `wgpu` rasterization |
| `wgpu` init | `pollster` | Sync wrapper for async adapter/device init |
| GPU data | `bytemuck` | Safe `&[T]` → `&[u8]` conversion for GPU buffers |
| Perf micro-benches | `criterion` | Repeatable ECS / physics / render CPU benchmarks |

## Architecture

### Frame Loop

Single-threaded, fixed-order, synchronous. Only the `cpal` audio callback and `notify` watcher are background threads.

```text
init:
    parse tungsten.json → EngineConfig
    open window (winit)
    init wgpu (instance, adapter, device, queue, surface)
    build renderer (pipelines, samplers, GPU resource pools)
    load assets/manifest.json → validate → decode assets → upload to GPU
    build World; insert Resources: DeltaTime, InputState, Assets, WindowSize

loop:
    poll events     → drain winit events into InputState resource
    tick            → update DeltaTime; run systems in declared order
    flush           → apply CommandBuffer, then rotate EventQueue resources
    hot reload      → apply ready asset/manifest changes at the frame boundary
    telemetry       → record update/extract/render/audio/hot-reload timings
    render          → extract renderables from World; record + submit draw calls
    present         → swap buffers

shutdown:
    drop World (drops asset registry and opaque handles)
    drop renderer (releases GPU resources)
    tear down wgpu; close window
```

Execution order is registration order. There is no scheduler, label system, or dependency graph.

### ECS

**Entity:** `Entity { index: u32, generation: u32 }`. Generational IDs catch stale-handle bugs. `entity.id()` returns `index` as `u32` for compatibility.

**Archetypal storage (M12):** Each archetype is a table of columns, `HashMap<TypeId, Box<dyn AnyColumn>>`, plus a parallel `Vec<Entity>` row index. `AnyColumn` is a type-erased interface over `TypedVec<T>(Vec<T>)`. Cost model: one downcast per archetype per type, then contiguous `Vec<T>` slice access.

**Archetype graph:** storage is `Vec<Archetype>` indexed by `ArchetypeId` plus `HashMap<Box<[TypeId]>, ArchetypeId>` for O(1) lookup by component set. Archetype `0` is the empty archetype. Freshly spawned entities start there. `add_edges` / `remove_edges` are lazy: the first `insert<T>` / `remove<T>` from an archetype builds the edge, and later transitions use O(1) cached lookup.

**Queries:** immutable queries are `query<A>()`, `query2<A,B>()`, `query3<A,B,C>()`, plus `_entities` variants. Cost model: one downcast per archetype per type, then sequential row access over contiguous `Vec<T>`. Mutable multi-component queries remain deferred because they require unsafe split-borrow.

**Resources:** singleton state lives in the `World` and uses the same access path as components. Examples: `DeltaTime`, `InputState`, `ActionMap`, `WindowSize`, `AssetRegistry`, `AudioCommands`, `PhysicsConfig`, `EventQueue<CollisionEvent>`, `CameraState`, `CameraController`, `DisplayState`, and `DisplayTelemetry`.

**Event delivery (M14):** `EventQueue<T>` stores `previous` + `current`. Systems send into `current`. Readers normally use `iter()`, which yields `previous` first and then `current`, to avoid order-sensitive missed reads. `App` rotates queues once per frame after `CommandBuffer` flush and before hot reload, extract, and render.

**Runtime telemetry (M12+):** `FrameTimings` is a `World` resource. `App` measures stage timings inline with `std::time::Instant`, stores the latest frame totals, and keeps a per-system `Vec<(String, f32)>` in registration order. Render is split into acquire, encode, and submit/present. GPU-facing diagnostics live in `tungsten-render::GpuFrameTimings`, and the umbrella crate mirrors those diagnostics into the `World` after renderer init and after each frame. Display-facing diagnostics live in `tungsten::DisplayTelemetry`, which tracks the authoritative resolution, display mode, vsync intent, applied present-mode label, max-frame-latency hint, scale mode, and frame-rate cap.

**ECS error strategy (D-022):** panic on programmer errors such as insert on dead entity or wrong downcast; return `Option` / `Result` on runtime conditions such as entity not found or component absent.

**Render path (D-018):** systems mutate the `World` during `tick`. Extract functions receive `&World`, resolve string IDs → `TextureHandle` via `AssetRegistry`, and produce POD slices such as `SpriteBatch`, `QuadInstance`, and `TextSection` for `render_frame_full`. The renderer may read the asset registry for ID resolution but does not need long-lived mutable `World` access at draw time.

**Core/render seam:** `TextureHandle(u32)` lives in `tungsten-core`; no `wgpu` types appear in core. `tungsten` mediates the bridge: `AssetRegistry::register_sprite` allocates the handle in core, and `renderer.upload_texture(handle, rgba, …)` stores the GPU resource in render under the same key. Core never calls into render. `tungsten-render` may depend on `tungsten-core` types (`D-007`).

**Render components (M15, `D-042`):** four engine-level component types live in `tungsten_core::components`:

- `Transform { position: Vec2, rotation: f32, scale: Vec2 }` — world-space pose. Rotation is in radians, CCW positive, applied around the quad centre by the sprite shader; scale multiplies the sprite's intrinsic pixel size per-axis.
- `Sprite { asset_id: String, color: [u8; 4], z_order: i32 }` — asset lookup + tint + stable ascending sort key.
- `Visibility { visible: bool }` — explicit render gate.
- `Tag { name: String }` — debug-friendly entity label for find-by-name lookups.

Physics `Position` stays separate (`D-033`). A free-fn `sync_position_to_transform(&mut World)` copies `Position.0` into `Transform.position` one-way; callers register it after `physics_step` when they want the post-physics position to reach the extract stage.

`SpriteInstance` carries the rotation and tint across the core/render seam as a 24-byte GPU-facing POD (`position`, `size`, `rotation` as `f32`, `color` as `Unorm8x4`). The WGSL pipeline multiplies the sampled texel by the tint and rotates around the quad centre. Every sprite path — component-driven, tilemap, and custom extracts alike — uses the same layout.

**Default sprite extract:** if the user does not call `App::set_extract_sprites`, the engine installs `tungsten::extract_sprites_default` at the start of `App::run`. It iterates `query3::<Transform, Sprite, Visibility>`, resolves each sprite through `AssetRegistry`, filters out entities where `visible == false`, sorts entries stably by `z_order` ascending, and batches by `(texture, filter)` within each z-order run so painter ordering is preserved. `Visibility` is required: entities with `Transform + Sprite` but no `Visibility` are never emitted. There is no implicit fallback.

### Data-Driven Config

Config model: single `tungsten.json` at workspace root, loaded once at startup. Missing file → defaults with warning. Invalid file → fatal error naming the bad field.

```json
{
  "window": { "title": "Tungsten", "width": 1280, "height": 720, "vsync": false },
  "display": {
    "resolution": { "width": 1280, "height": 720 },
    "display_mode": "windowed",
    "vsync": false,
    "present_mode": "auto",
    "max_frame_latency": 1,
    "scale_mode": "stretch",
    "frame_rate_cap": null
  },
  "render": {
    "clear_color": [0.05, 0.05, 0.08, 1.0],
    "max_frame_latency": 1,
    "present_mode": "auto",
    "msaa": 1,
    "depth_enabled": true,
    "depth_sort": "cpu_stable"
  },
  "logging": { "level": "info" }
}
```

Display config semantics: checked-in display settings live under `display.*`. `display.present_mode` is authoritative when set to a concrete mode such as `"immediate"` or `"mailbox"`. When absent or `"auto"`, `display.vsync` selects between the auto-vsync and auto-no-vsync families. `display.max_frame_latency` is the requested frames-in-flight hint passed into `wgpu::SurfaceConfiguration`; backends may clamp it, so treat runtime telemetry as the configured hint unless the backend exposes stronger confirmation. Legacy `window.width`, `window.height`, `window.vsync`, `render.present_mode`, and `render.max_frame_latency` remain valid compatibility inputs in M17, but `display.*` wins whenever both specify the same concern.

### Asset System

**Manifest-driven, ID-referenced (D-009):** `assets/manifest.json` registers every asset by string ID. Game code uses IDs, never paths. Validation at load time catches missing files and unresolved references.

**Multiple manifests compose by extension, never override (D-017):** IDs must be globally unique across the merged set, duplicates are fatal, each path resolves relative to its declaring manifest, and merge order is call-site order (`D-035`).

**Animation format (D-010):** frame-based, per-frame durations, one animation per file under `assets/animations/`.

```json
{
  "looping": true,
  "frames": [
    { "sprite": "player_walk_0", "duration_ms": 100 },
    { "sprite": "player_walk_1", "duration_ms": 100 }
  ]
}
```

**Per-sprite filter mode (D-011):** manifest values are `nearest` (default) or `linear`. The renderer creates one sampler per mode. Mixed pixel-art and high-res content can coexist in one frame.

**Directory layout:**

```text
assets/
├── manifest.json
├── sprites/
├── animations/
├── fonts/
└── sounds/
```

Examples ship `examples/NN_name/assets/` with a local manifest. The loader takes a manifest path. Multiple manifests compose.

**Opaque handles (D-016):** `tungsten-core` stores opaque `TextureHandle(u32)` IDs. `tungsten-render` owns GPU textures, samplers, and pipelines in internal pools keyed by those handles. The registry is the one game-facing lookup path.

## Subsystems

### Text — M7

Stack: `glyphon` + `cosmic-text` + `swash`. Responsibilities: font parsing, shaping, layout, and GPU rasterization. Decision: `D-026`. Fonts are registered in the manifest by ID under `fonts`. `TextSection` is extracted each frame. Text ignores the world camera; it stays screen-space while the world scrolls.

### Audio — M8

`cpal` (`D-027`) opens the audio device. `symphonia` (`D-028`) decodes `OGG` / `WAV` / `MP3` at load time into `Vec<f32>` PCM, and no decoder types appear at runtime. A hand-rolled mixer (~150 lines, `D-029`) runs in the `cpal` callback thread. Game systems write `AudioCommand` values each tick. The callback drains commands through an `rtrb` wait-free SPSC ring (`D-034`, capacity `64`). Audio assets are not hot-reloadable; PCM buffers decoded at startup stay fixed for the session.

### Hot Reload — M9

`notify` v6 (`D-031`) runs on a dedicated background thread. File events cross to the main thread through `std::sync::mpsc`. A `50ms` debounce collapses editor double-writes. At the next frame boundary the main thread resolves file paths → asset IDs, decodes new data, uploads to GPU, and swaps handles in the registry. M25 (`D-057`) brings shaders into the same path: `.wgsl` edits validate through `wgpu::naga` and only commit to the live `ShaderModule` after the dependent pipeline rebuilds. Signature / bind-group-layout changes still require a binary rebuild, narrowing `D-023`. Invariant: do not break the registry-by-ID model; game code must not hold direct GPU handles.

**Supported reload matrix (`D-053`):**

| Asset class | Single-file edit | Manifest-add | Manifest-remove |
| --- | --- | --- | --- |
| Sprite (`.png`/`.jpg`/`.jpeg`) | yes — `reload_sprite` with in-place overwrite for shrink/equal, `rebuild_atlas_for_filter` for growth | yes — registered with placeholder then atlas class rebuilt | warn-only; stale entry kept |
| Animation (`.json`) | yes — `reload_animation` replaces entry in `AnimationRegistry` | yes — inserted into `AnimationRegistry` | warn-only; stale entry kept |
| Tilemap (`.tmj`) | yes — `reload_tilemap` replaces entry, rejects unknown tileset sprite IDs | yes — inserted after tileset validation | warn-only; stale entry kept |
| Font (`.ttf`/`.otf`) | yes — `reload_font` swaps face data in `TextPipeline` | yes — added through `renderer.load_font` + `FontRegistry::register` | warn-only; stale entry kept |
| Particle (`.json`) | yes — `reload_particle` swaps the `Arc<ParticleConfig>` under the same `AssetId` (`D-050`) | yes — inserted into `ParticleConfigRegistry` after sprite validation | warn-only; stale entry kept |
| Sound (decoded PCM) | **not supported** — mixer owns cloned PCM; session-static | **not supported** — no manifest-add path | n/a |
| Shader (`.wgsl`) | yes (body-edit only) — `reload_shader` re-validates through `wgpu::naga` and rebuilds the sprite pipeline **or** every material pipeline bound to that shader; signature / bind-group-layout changes still need a rebuild (`D-057`, narrowing `D-023`) | M26: new stock / user shaders register on next manifest reload | warn-only; stale entry kept |
| Material (`materials` section, `D-058`) | yes (body-only) — `reload_material` re-uploads the 256-byte UBO against the shader's live module and swaps the `MaterialPipeline` entry; validation failure keeps the prior pipeline | yes — manifest reload allocates a new `MaterialAssetId` and calls `upload_material` | warn-only; stale entry kept |
| SMAA stage shaders (M27, `D-059`) | yes (body-only) — `smaa_edge`, `smaa_blend_weights`, `smaa_neighborhood_blend` follow the M25 shader path; `Renderer::reload_shader` re-validates and rebuilds only the affected `SmaaPipeline` stage. SMAA `area` / `search` LUT binaries are explicitly out-of-matrix (engine-internal `include_bytes!`) | n/a (engine-internal stage shaders, fixed set of three) | n/a |
| Bloom stage shaders (M28, `D-060`) | yes (body-only) — `bloom_threshold`, `bloom_downsample`, `bloom_upsample`, `bloom_composite` follow the M25 shader path; `Renderer::reload_shader` re-validates and rebuilds only the affected `BloomPipeline` stage via `rebuild_stage_with_module`. The `BloomPyramid` texture is engine-internal and explicitly out-of-matrix; signature changes still need a rebuild | n/a (engine-internal stage shaders, fixed set of four) | n/a |
| Lit sprite shader + helpers (M29, `D-061`) | yes (body-only) — `lit_sprite` rebuilds the `LitSpritePipeline` via `Renderer::reload_shader` → `LitSpritePipeline::rebuild_with_shader`; `emissive_mask` and `rim_light` are validated and cached but bound to no pipeline directly (helpers for material composition) | M29: new lit-shader / helper ids register on next manifest reload | warn-only; stale entry kept |
| Sprite normal_map / emissive_mask siblings (M29, `D-061`) | yes — sibling PNG edits route through `reload_sprite` (mapped via reverse path lookup); `write_subtexture_lit` updates the matching cell in the lit atlas pool, full repack on grow | yes — manifest reload picks up new sibling fields and rebuilds the affected filter-class atlas | warn-only; stale lit page kept |
| `input.json` | yes — `reload_action_map` merges with defaults and swaps `ActionMap` | n/a | n/a |
| `manifest.json` | yes — `reload_manifest` walks every class above | n/a | n/a |

Audio is session-static by design: `AudioSystem::init` reads every decoded `SoundData::samples` into a callback-owned `HashMap<AudioHandle, Vec<f32>>` (`D-027` / `D-029` / `D-034`), and the mixer closure captures that map at startup. Adding a runtime PCM-swap command is a future milestone; until then, "sound hot reload" is explicitly out of scope and the watcher logs at `debug` when a `.ogg`/`.wav`/`.mp3` under the asset tree changes.

### Tilemaps — M10

Extension: `.tmj`. Schema: Tiled-compatible (`D-032`). Core data types `TilemapData`, `TilemapRegistry`, and `TilemapInstance` live in `tungsten-core` as plain data; no `wgpu`. `extract_tilemaps(&World)` resolves visible tiles into `SpriteBatch`es. Tilemaps reuse the sprite pipeline; there is no new `wgpu` pipeline. `CameraState` (`position`, `zoom`, `rotation`) feeds view-projection into sprite and quad pipelines, and `visible_world_aabb()` keeps tile culling proportional to viewport size while over-covering safely under rotation. `LayerKind::Collision` layers are accepted by the loader but skipped by extract; physics reads them directly.

### Physics — M11

Module: `tungsten-core::physics` (`D-033`). Components: `Position`, `Velocity`, `Collider` (AABB or circle, with offset), `RigidBody` (static or dynamic). Resources: `PhysicsConfig`, `EventQueue<CollisionEvent>`. Broad-phase: uniform spatial grid rebuilt per substep, `HashMap<IVec2, Vec<ProxyId>>`, default cell size `32.0`. Narrow-phase: `AABB/AABB`, `circle/circle`, `AABB/circle`. Response: MTV resolution with restitution. Tilemap collision layers become transient static AABBs per substep.

Known limits: variable-dt with substep cap (`D-033`), with a semi-fixed accumulator as the preferred upgrade; tilemap collider budget `<= 128×128` tiles per substep.

`physics_step` is a plain system. User registration path: `app.add_system(physics_step)`. `PhysicsConfig::gravity` defaults to `Vec2::ZERO`, so top-down games pay no gravity overhead by default.

### Archetypal ECS — M12

Storage design is described in [§ECS](#ecs). Decision to proceed: `D-036` (cites `D-030`).

| Benchmark | Archetypal | Naive | Ratio |
| --- | --- | --- | --- |
| `query::<Position>` — 10k entities | `6.8 µs` | `43.4 µs` | `~6×` |
| `query2::<Position, Velocity>` — 10k, 1 archetype | `7.1 µs` | `1 424 µs` | `~200×` |
| `query2::<Position, Velocity>` — 10k, 5 archetypes | `7.5 µs` | `1 424 µs` | `~190×` |
| spawn + 3 inserts × 10k | `4.4 ms` | `—` | `—` |

Deferred work: parallel system scheduling, change detection, command buffers, reactive queries, `BlobVec` raw-byte columns.

### Input Mapping — M19

`tungsten-core::input::action_map::ActionMap` is a `World` resource that maps named actions to one or more `Binding`s (`Key`, `Mouse`, `Scroll`). It loads from `input.json` at the workspace root, merges with `default_map()` so missing actions still resolve, and exposes `is_pressed`, `just_pressed`, and `just_released` against the live `InputState`. `InputState` was extended with cursor position/delta and per-frame line and pixel scroll deltas; scroll edges auto-release on `begin_frame` so `just_pressed("…wheel_up")` fires once per notch.

Engine-owned actions (`engine_toggle_hud`, `engine_toggle_vsync`, `engine_toggle_fullscreen`, `engine_exit`) live in defaults so a missing or partial `input.json` still ships HUD/display/exit controls. Hot reload watches `input.json` through `HotReloadWatcher::extra_files`; on a ready event `asset_loader::reload_action_map` swaps the resource at the frame boundary. `ActionMap::persist` writes the file back through a temp-file + rename, patching only the changed lines so user-authored ordering and comments survive. Decision: `D-045`.

### Scene / State System — M20

`tungsten::state::StateStack` is a `World` resource driving a `Vec<Box<dyn GameState>>` through deferred `request_push` / `request_pop` / `request_replace` queues. A single engine-owned `state_dispatcher_system`, registered immediately after `__display_input`, drains the pending queue each frame and fires the transition matrix: `push` triggers `old.on_pause` → `new.on_enter`; `pop` triggers `old.on_exit` (after auto-despawn) → `next.on_resume`; `replace` triggers `old.on_exit` (after auto-despawn) → `new.on_enter`. `on_pause` / `on_resume` default to no-op so Pause overlays Gameplay without tearing the scene down. The dispatcher also mirrors the active state id into `HudActiveState` so the M18 state row keeps rendering correctly.

Scene-owned entities carry a `SceneEntity { state_id }` marker. On state exit the dispatcher walks `query::<SceneEntity>()` and enqueues `CommandBuffer::despawn` for each matching entity before the user's `on_exit` runs; the engine's post-systems `CommandBuffer` flush (`D-039`) applies the despawns so the last frame of the exiting state already sees its scene entities gone and the first frame of the next state already sees its scene entities present. `tungsten_core::assets::scene::SceneData` is a minimal JSON schema that reuses the M15 components — each `SceneEntry` maps to `Transform + Sprite? + Visibility + Tag?` and may carry scene-authored tweens. `asset_loader::spawn_scene(world, &data, state_id)` funnels every entry through `CommandBuffer` so the spawn lands at the normal frame boundary; sprite id validation is intentionally deferred to the extract path, matching the tilemap behaviour, and scene tween authoring inserts at most one `Tween` component per spawned entity. `ActionMap::default_map()` now ships `state_start` (`Enter`), `state_pause` (`KeyP`), and `state_back` (`Backspace`) so the example flow works without edits to `input.json`. Decision: `D-046`.

### Tween System — M24

`tungsten_core::tween` adds a component-driven animation surface: `Tween` holds one timing model plus a `Vec<TweenChannel>` so position, rotation, scale, and sprite color can animate together on one entity. Easings are a closed `enum` (`Easing`) with a pure `apply(t)` implementation; repeat modes are `Once`, `Loop`, `PingPong`, and `Times(n)`.

`tungsten::tweens::tween_tick_system` advances every live tween from `DeltaTime`, writes `Transform` / `Sprite` fields in place, emits `TweenComplete` through `EventQueue<TweenComplete>`, and defers terminal `Tween` removal through `CommandBuffer` so archetypes never mutate mid-iteration. The frame slot is `particles -> tweens -> flush commands -> flush events`, which keeps tween writes visible to extract/render in the same frame while preserving the fixed frame-boundary mutation/event rules. Scene JSON can author tweens directly, making state-transition fades and simple data-driven motion possible without bespoke example systems. Decisions: `D-054`, `D-055`, `D-056`.

### Presentation AA / SMAA — M27

`RenderConfig.post_aa` (`PostAaMode::{Off, SmaaLow, SmaaMedium, SmaaHigh, SmaaUltra}`, `#[non_exhaustive]`) selects an optional SMAA 1x presentation tail. `Off` is the default and produces the byte-identical M26 frame. The render path is:

```
Scene → PostStack → [optional SMAA tail → PresentSource] → Text Overlay → Present Blit → Swapchain
```

When `post_aa != Off` the renderer allocates `SmaaEdges` (`Rg8Unorm`), `SmaaBlend` (`Rgba8Unorm`), and `PresentSource` (matching the swapchain format, `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC`). `SceneColor`, `PostPing`, and `PostPong` are recreated with a non-sRGB twin in `view_formats` so the SMAA edge/blend/neighborhood passes sample gamma-encoded values; the rest of the frame keeps using the primary view. The text overlay always runs after the SMAA tail, so screen-space text is never sampled by SMAA. The present blit and screenshot path source `PresentSource` when `post_aa != Off` and `SceneColor` / the post-stack final target otherwise.

Stage shaders (`smaa_edge`, `smaa_blend_weights`, `smaa_neighborhood_blend`) are manifest-tracked, follow the stock-shader pattern (compile-time `include_str!` mirror under `crates/tungsten-render/src/shaders/stock/` + byte-equal mirror under `assets/shaders/stock/`), and hot-reload through `Renderer::reload_shader` with `naga` validation; failure leaves the live pipelines untouched. The `area` and `search` lookup textures are `include_bytes!`-embedded engine content under `crates/tungsten-render/src/assets/smaa/` and explicitly **not** manifest-tracked. Preset knobs (threshold, max search steps, max diag steps, corner rounding) ride a single 256-byte UBO matching `UniformOverrideBlock`; switching presets only rewrites the UBO. Switching `post_aa` itself re-creates SMAA intermediate targets and the `SmaaPipeline` at a frame boundary — no relaunch (unlike `msaa`).

`tungsten.json` carries `render.post_aa`; the env override is `TUNGSTEN_RENDER_POST_AA`; runtime mutation goes through `tungsten::request_post_aa(world, mode)`, applied by `App::apply_pending_post_aa_request` between hot-reload and extract. Decision: `D-059`.

### Bloom — M28

`PostPass::Bloom(BloomParams { threshold, knee, intensity, radius })` is the 18th `PostPass` variant. Bloom is a normal reorderable post slot — placement before vs after tone-mapping is the user's choice — but it is the first slot that records multiple sub-passes through the encoder rather than a single fullscreen draw into the slot's auto-opened render pass. The renderer detects the variant before `PassRecorder::begin` and calls `BloomPipeline::record_pass`, which opens its own per-subpass passes:

```
src slot ─► threshold (write mip 0) ─► N-1 13-tap Karis-weighted downsamples
                                       (write mip 1..N-1, replace blend)
                                                            │
            additive 9-tap tent upsamples ◄────────────────┘
            (write mip N-2..0, blend One+One)
                              │
              composite ◄─────┘
              (read src + pyramid.mip[0]; write dst slot, replace blend)
```

The `BloomPyramid` lives on `SceneTarget` as a single `Rgba16Float` texture with N mip levels and per-level views; mip 0 starts at half resolution to halve bandwidth and match the COD/Frostbite convention. `bloom_mip_count_for_size(width, height, render.bloom_max_mips)` clamps the chain by `floor(log2(min(width, height))) - 1`, with `bloom_max_mips` configured in `tungsten.json` (default 6, range 1..=8) and overridable via `TUNGSTEN_RENDER_BLOOM_MAX_MIPS`. The pyramid is allocated unconditionally because the validated range never collapses to zero; bloom-not-in-stack frames still pay the bounded pyramid memory but skip every sub-pass. `bloom_max_mips` is startup-only — runtime mutation has no request/apply seam in M28, the same constraint as `msaa`.

The four stage shaders (`bloom_threshold`, `bloom_downsample`, `bloom_upsample`, `bloom_composite`) follow the M25 stock-shader pattern: compile-time `include_str!` mirror under `crates/tungsten-render/src/shaders/stock/`, byte-equal mirror under `assets/shaders/stock/`, manifest-tracked at stable shader ids `4..=7` (after sprite + SMAA `0..=3`), body-edit hot-reload through `Renderer::reload_shader` → `BloomPipeline::rebuild_stage_with_module` with `naga` validation and last-known-good pipeline retention on failure. The per-subpass UBO reuses the engine-wide 256-byte `UniformOverrideBlock` layout: `vec4[0]` carries `inv_src_size` (per-mip), `vec4[1]` is the reserved `composite_tint`, the `f32s` block carries `[threshold, knee, intensity, radius]`, and the `i32s` block carries `[mip_count, dst_level, pass_kind, _]`. `SceneColor` stays sRGB — only the pyramid is HDR. With `PostStack` empty and `post_aa = Off` the pyramid exists but is never written to or sampled, so the captured frame matches the M27 baseline. Decision: `D-060`.

### 2D Forward Lighting — M29

`Light { kind, color, intensity }` and the closed `LightKind::{Point { radius, falloff }, Directional { angle }}` enum live in `tungsten-core::components`; `AmbientLight(Vec3)` is a world resource defaulting to `Vec3::ONE`. `LIGHT_CAP = 16` lives in `tungsten-core::lighting`; the render-side mirror is `tungsten-render::LIT_LIGHT_CAP`. The renderer owns one 544-byte `LightUbo` (16 lights × 32 bytes + `vec4<u32>` count_pad + `vec4<f32>` ambient) under `LightingResources` bound at group 2 of a sibling `LitSpritePipeline`. The lit pipeline reuses `SpritePipeline::vertex_layouts()` and binds a parallel albedo + normal + emissive bundle at group 1 — three texture views + one filtering sampler — so vertex/instance buffers stay interchangeable with the unlit and material paths.

Sprites with optional `normal_map` / `emissive_mask` sibling files in the manifest pack into parallel atlas canvases keyed by the same `PackedSprite` placement output: the asset loader runs `pack_shelf` once over the albedo inputs, then fills albedo, flat-normal-default, and emissive canvases page-by-page. Albedo uploads as `Rgba8UnormSrgb`; normal and emissive upload as `Rgba8Unorm` so tangent-space vectors and mask intensities are not gamma-decoded. The `SpriteAsset.lit_atlas: Option<TextureHandle>` marker tags only sprites with a valid normal sibling, and `extract_sprites_default` flips `SpriteBatch.lit` on that axis. Lit + material is intentionally out-of-scope in M29: a sprite carrying both warns and uses lit. `extract_lights` runs every frame, queries `(Transform, Light)`, packs each into `GpuLight`, sorts directional-first then nearest-to-AABB-squared-distance, truncates to `LIGHT_CAP`, and packs the result + ambient into the per-frame `LightUbo`. With no lit sprites and an empty light list the captured frame stays byte-identical to the M28 baseline — the unlit pipeline is unchanged and the lit pipeline never runs.

The lit shader (`assets/shaders/lit_sprite.wgsl`) and helpers (`assets/shaders/stock/emissive_mask.wgsl`, `assets/shaders/stock/rim_light.wgsl`) follow the M25 stock-shader pattern: compile-time `include_str!` mirror under `crates/tungsten-render/src/shaders/` and `crates/tungsten-render/src/shaders/stock/`, byte-equal mirror under `assets/shaders/`, manifest-tracked at stable shader ids `8..=10` (after bloom `4..=7`), body-edit hot-reload through `Renderer::reload_shader` → `LitSpritePipeline::rebuild_with_shader` with `naga` validation and last-known-good pipeline retention on failure. The helpers are validated only — no pipeline behind them — so material authors and future milestones can fold `emissive_contribution` and `rim_term` into their own WGSL. Decision: `D-061`.

### Performance Baseline + Profiling Harness — Phase 3 M12

M12 defines the baseline for later Phase 3 work.

- CPU telemetry: `App` instruments update, extract, render, audio, hot reload, and total frame time. Render is split into surface acquire, CPU encode, and submit/present timing. Per-system timings live in `FrameTimings::system_timings`, plus `slowest_system()`.
- GPU telemetry: `Renderer::render_frame_full_timed()` uses `wgpu` timestamp queries when `TIMESTAMP_QUERY` is available on the active adapter. The path is opt-in via `TUNGSTEN_GPU_TIMING`, blocks on GPU completion to read timestamps back, and exposes backend, adapter, present mode, and max-frame-latency metadata through `GpuFrameTimings`.
- Canonical scenes: `example-02-sprite-stress` with `STRESS_SCENE=ecs-high-load` is the primary full-system stress scene (ECS, physics grid, steering, camera follow, render); the same binary under `STRESS_SCENE=baseline` remains the render-hot-path sprite-throughput baseline. `example-01-platformer` is the broad feature scene and is no longer part of the canonical perf matrix.
- Bench coverage: Criterion suites cover ECS, physics, and CPU-only render-data construction. They are regression detectors, not exhaustive throughput claims.
- Capture tooling: `scripts/perf-capture.sh` and `docs/perf/profiling-workflow.md` define the repeatable Linux workflow: release builds with frame pointers, smoke-frame-bounded runs, parsed renderer metadata, `p50` / `p95` / `p99` summaries, optional flamegraph / `perf` artifacts, and timestamped output directories under `perf-runs/`.

## Non-Commitments

Not scoped without an explicit decision:

- Networking, multiplayer
- Scripting
- Editor tooling
- Asset preprocessing / build pipeline
- 3D rendering
- WASM / browser support
- Hot reload of config (assets have it; config does not)
- Save / load
- GUI library
- GPU-compressed texture formats (`KTX2`, `Basis Universal`)
- Skeletal animation
- Streaming or async asset loading
- Per-platform asset variants
