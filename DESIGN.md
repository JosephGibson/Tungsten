# Tungsten — Design

## Status

Workspace `v0.17.0` on branch `0.17`. Phase 3 M20 is shipped. Companion docs: [`AGENTS.md`](AGENTS.md) for operational rules, [`DECISIONS.md`](DECISIONS.md) for rationale by `D-NNN`.

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

Single-threaded, fixed-order, synchronous. Only the `cpal` audio callback and `notify` watcher are background threads. All game logic stays single-threaded.

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
  "render": { "clear_color": [0.05, 0.05, 0.08, 1.0], "max_frame_latency": 1, "present_mode": "auto" },
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

`notify` v6 (`D-031`) runs on a dedicated background thread. File events cross to the main thread through `std::sync::mpsc`. A `50ms` debounce collapses editor double-writes. At the next frame boundary the main thread resolves file paths → asset IDs, decodes new data, uploads to GPU, and swaps handles in the registry. Covered asset classes: sprites, animations, fonts, manifest. Exclusion: shaders are excluded, so shader changes require a binary rebuild (`D-023`). Invariant: do not break the registry-by-ID model; game code must not hold direct GPU handles.

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

Scene-owned entities carry a `SceneEntity { state_id }` marker. On state exit the dispatcher walks `query::<SceneEntity>()` and enqueues `CommandBuffer::despawn` for each matching entity before the user's `on_exit` runs; the engine's post-systems `CommandBuffer` flush (`D-039`) applies the despawns so the last frame of the exiting state already sees its scene entities gone and the first frame of the next state already sees its scene entities present. `tungsten_core::assets::scene::SceneData` is a minimal JSON schema that reuses the M15 components — each `SceneEntry` maps to `Transform + Sprite? + Visibility + Tag?`. `asset_loader::spawn_scene(world, &data, state_id)` funnels every entry through `CommandBuffer` so the spawn lands at the normal frame boundary; sprite id validation is intentionally deferred to the extract path, matching the tilemap behaviour. `ActionMap::default_map()` now ships `state_start` (`Enter`), `state_pause` (`KeyP`), and `state_back` (`Backspace`) so the example flow works without edits to `input.json`. Decision: `D-046`.

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
- Texture atlases / sprite sheet packing
- GPU-compressed texture formats (`KTX2`, `Basis Universal`)
- Skeletal animation
- Streaming or async asset loading
- Per-platform asset variants
- Tweened transforms as a separate animation system
