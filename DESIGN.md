# Tungsten — Design

**Status:** `v0.9.0-alpha` — Phase 3 M12 complete. Branch: `0.9`.  
**Companion docs:** [`AGENTS.md`](AGENTS.md) (operational rules), [`DECISIONS.md`](DECISIONS.md) (rationale by D-NNN).

---

## What it is

Tungsten is a from-scratch Rust 2D game engine. Native only (Linux / macOS / Windows). Three-crate Cargo workspace: `tungsten-core`, `tungsten-render`, `tungsten`. 2D only — no 3D math beyond what `glam` provides, no model formats, no skeletal animation, no PBR.

## Principles

1. **Build from scratch where practical.** No ECS crates, no engine crates, no rendering helpers.
2. **wgpu for rendering.** Modern GPU API at a manageable level.
3. **ECS-first for game state, built by hand.**
4. **Data over code for content.** Engine config, asset registration, and animation definitions live in JSON.

### Dependency philosophy

A dependency is acceptable if at least one of these applies (D-015):

| Rule | Applies to |
|---|---|
| 1. Platform API abstraction | `winit`, `wgpu`, `notify`, `cpal` |
| 2. Well-specified data format | `serde_json`, `image`, `symphonia` |
| 3. Math / primitive (solved problem) | `glam`, `bytemuck`, `pollster`, `rtrb` |

Any crate that would hand over something the project is supposed to build is rejected (`bevy_ecs`, `hecs`, `rapier2d`, `rodio`, etc.). Borderline cases get a `DECISIONS.md` entry.

## Stack

| Concern | Choice | Role |
|---|---|---|
| Windowing | `winit` | OS window and event abstraction |
| Rendering | `wgpu` | GPU API abstraction |
| Math | `glam` | Vectors, matrices, transforms |
| ECS | hand-rolled | Archetypal storage, built in-project |
| Config | `serde` + `serde_json` | JSON schema derive + parsing |
| Image | `image` | PNG decoding to CPU bitmaps |
| Logging | `log` + `env_logger` | Standard facade + basic backend |
| Errors | `thiserror` / `anyhow` | Typed at library boundaries, anyhow at top level |
| Audio device | `cpal` | Platform audio API (WASAPI / CoreAudio / ALSA) |
| Audio decode | `symphonia` | OGG / WAV / MP3 decode |
| File watch | `notify` | Platform file-change events |
| Audio ring | `rtrb` | Wait-free SPSC ring for audio commands |
| Text | `glyphon` + `cosmic-text` | TrueType/OpenType layout + wgpu rasterization |
| wgpu init | `pollster` | Sync wrapper for wgpu's async adapter/device init |
| GPU data | `bytemuck` | Safe `&[T]` → `&[u8]` for vertex/instance buffers |
| Perf micro-benches | `criterion` | Repeatable ECS / physics / render CPU benchmarks |

---

## Architecture

### Frame loop

Single-threaded, fixed-order, synchronous. The `cpal` audio callback and the `notify` watcher are the only background threads. Game logic stays single-threaded throughout.

```
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
    telemetry       → record update/extract/render/audio/hot-reload timings
    render          → extract renderables from World; record + submit draw calls
    present         → swap buffers

shutdown:
    drop World (drops asset registry and opaque handles)
    drop renderer (releases GPU resources)
    tear down wgpu; close window
```

System registration order is execution order. No scheduler, labels, or dependency graph.

### ECS

**Entity** — `Entity { index: u32, generation: u32 }`. Generational IDs catch stale-handle bugs. `entity.id()` returns `index` as `u32` for source compatibility.

**Archetypal storage (M12):** Each archetype is a table of columns — `HashMap<TypeId, Box<dyn AnyColumn>>` plus a parallel `Vec<Entity>` row index. `AnyColumn` is a type-erased interface over `TypedVec<T>(Vec<T>)`. One downcast per archetype per type; elements accessed as contiguous `Vec<T>` slices.

**Archetype graph:** `Vec<Archetype>` indexed by `ArchetypeId` + `HashMap<Box<[TypeId]>, ArchetypeId>` for O(1) lookup by component set. Archetype 0 is the empty archetype; all freshly spawned entities start there. `add_edges` / `remove_edges` are lazy — cached on first `insert<T>` / `remove<T>` from a given archetype; subsequent transitions are O(1) edge lookup.

**Queries:** `query<A>()`, `query2<A,B>()`, `query3<A,B,C>()` plus `_entities` variants — immutable; one downcast per archetype per type, then sequential row access over contiguous `Vec<T>`. Mutable multi-component queries require unsafe split-borrow and are deferred.

**Resources:** singleton state in the `World` accessed by the same mechanism as components. `DeltaTime`, `InputState`, `WindowSize`, `AssetRegistry`, `AudioCommands`, `PhysicsConfig`, `CollisionEvents`, `Camera2D` are all resources.

**Runtime telemetry (M12):** `FrameTimings` is also a `World` resource. `App` measures stage timings inline with `std::time::Instant`, stores the latest frame totals, and records a per-system `Vec<(String, f32)>` in registration order. GPU-facing diagnostics live in `tungsten-render::GpuFrameTimings`, mirrored into the `World` by the umbrella crate after renderer init and each frame.

**ECS error strategy (D-022):** panic on programmer errors (insert on dead entity, wrong downcast); `Option`/`Result` on runtime conditions (entity not found, component absent).

**Render path (D-018):** systems mutate the `World` during `tick`; extract functions receive `&World`, resolve string IDs → `TextureHandle` via `AssetRegistry`, and produce POD slices (`SpriteBatch`, `QuadInstance`, `TextSection`) passed into `render_frame_full`. The renderer may read the asset registry for ID resolution but needs no long-lived mutable `World` access at draw time.

**Core/render seam:** `TextureHandle(u32)` defined in `tungsten-core` — no `wgpu` types appear there. `tungsten` mediates: `AssetRegistry::register_sprite` allocates a handle in core; `renderer.upload_texture(handle, rgba, …)` stores the GPU resource in render's pool under the same key. Core never calls into render. `tungsten-render` may depend on `tungsten-core` types (D-007).

### Data-driven config

Single `tungsten.json` at workspace root, loaded once at startup. Missing → defaults with warning. Invalid → fatal naming the bad field.

```json
{
  "window": { "title": "Tungsten", "width": 1280, "height": 720, "vsync": true },
  "render":  { "clear_color": [0.05, 0.05, 0.08, 1.0] },
  "logging": { "level": "info" }
}
```

### Asset system

**Manifest-driven, ID-referenced (D-009).** `assets/manifest.json` registers every asset by string ID. Game code references assets by ID, never by path. Validation at load time catches missing files and unresolved references.

**Multiple manifests compose by extension, never override (D-017).** IDs must be globally unique across the merged set; duplicates are fatal. Each path resolves relative to its declaring manifest. Merge order is call-site order (D-035).

**Animation format (D-010):** frame-based, per-frame durations, each animation in its own file under `assets/animations/`:

```json
{
  "looping": true,
  "frames": [
    { "sprite": "player_walk_0", "duration_ms": 100 },
    { "sprite": "player_walk_1", "duration_ms": 100 }
  ]
}
```

**Per-sprite filter mode (D-011):** `nearest` (default) or `linear` in the manifest. The renderer creates one sampler per mode and binds the right one per sprite. Enables mixed pixel-art and high-res content in the same frame.

**Directory layout:**

```
assets/
├── manifest.json
├── sprites/
├── animations/
├── fonts/
└── sounds/
```

Examples ship `examples/NN_name/assets/` with a local manifest. The loader takes a manifest path; multiple manifests compose.

**Opaque handles (D-016):** `tungsten-core` stores opaque `TextureHandle(u32)` IDs — no `wgpu` types. `tungsten-render` owns GPU textures, samplers, and pipelines in internal pools keyed by those handles. The registry is the one game-facing lookup path.

---

## Subsystems

### Text — M7

`glyphon` + `cosmic-text` + `swash` handle font parsing, shaping, layout, and GPU rasterization (D-026). Fonts registered in the manifest by ID under `fonts`. `TextSection` is extracted each frame; text deliberately ignores `Camera2D` — it stays screen-space while the world scrolls.

### Audio — M8

`cpal` (D-027) opens the audio device. `symphonia` (D-028) decodes OGG/WAV/MP3 at load time into `Vec<f32>` PCM — no decoder types appear at runtime. A hand-rolled mixer (~150 lines, D-029) runs in the `cpal` callback thread. Game systems write `AudioCommand` values each tick; the callback drains them via an `rtrb` wait-free SPSC ring (D-034, capacity 64). Audio assets are not hot-reloadable — PCM buffers decoded at startup remain fixed for the session.

### Hot reload — M9

`notify` v6 (D-031) runs on a dedicated background thread, sending file-change events to the main thread via `std::sync::mpsc`. A 50ms debounce collapses editor double-writes. At the next frame boundary the main thread resolves file paths → asset IDs, decodes new data, uploads to GPU, and swaps handles in the registry. Covers: sprites, animations, fonts, manifest. **Shaders excluded** — shader changes require a binary rebuild (D-023).

**Do not break the registry-by-ID invariant** — it is what makes hot reload feasible. No game code should hold direct GPU handles.

### Tilemaps — M10

`.tmj` extension, Tiled-compatible schema (D-032). `TilemapData` / `TilemapRegistry` / `TilemapInstance` live in `tungsten-core` (plain data, no wgpu). `extract_tilemaps(&World)` resolves visible tiles into `SpriteBatch`es and returns them in layer order — the sprite pipeline draws them with zero changes (no new wgpu pipeline). `Camera2D` resource (position, zoom) feeds view-projection into sprite and quad pipelines. Visible-AABB culling keeps cost proportional to viewport, not map size. `LayerKind::Collision` layers are accepted by the loader but skipped by extract; physics reads them directly.

### Physics — M11

`tungsten-core::physics` (D-033). Components: `Position`, `Velocity`, `Collider` (AABB or circle, with offset), `RigidBody` (static or dynamic). Resources: `PhysicsConfig` (gravity, substep cap, cell size), `CollisionEvents`. Broad-phase: uniform spatial grid rebuilt per substep (`HashMap<IVec2, Vec<ProxyId>>`, default cell size 32.0). Narrow-phase: AABB/AABB, circle/circle, AABB/circle. MTV resolution with restitution. Tilemap collision layers emitted as transient static AABBs per substep.

**Known limits:** variable-dt with substep cap (D-033) — preferred upgrade is semi-fixed accumulator; tilemap collider budget ≤128×128 tiles per substep.

`physics_step` is a plain system — the user registers it via `app.add_system(physics_step)`. `PhysicsConfig::gravity` defaults to `Vec2::ZERO` so top-down games pay no physics overhead.

### Archetypal ECS — M12

Storage design described in [§ECS](#ecs) above. Decision to proceed: D-036 (cites D-030).

**Benchmark results** (release, Criterion 0.5, 10k entities, naive `HashMap<TypeId, HashMap<u32, Box<dyn Any>>>` baseline):

| Benchmark | Archetypal | Naive | Ratio |
|---|---|---|---|
| `query::<Position>` — 10k entities | 6.8 µs | 43.4 µs | ~6× |
| `query2::<Position, Velocity>` — 10k, 1 archetype | 7.1 µs | 1 424 µs | ~200× |
| `query2::<Position, Velocity>` — 10k, 5 archetypes | 7.5 µs | 1 424 µs | ~190× |
| spawn + 3 inserts × 10k | 4.4 ms | — | — |

Deferred: parallel system scheduling, change detection, command buffers, reactive queries, `BlobVec` raw-byte columns.

### Performance baseline + profiling harness — Phase 3 M12

M12 establishes the baseline for all later Phase 3 work.

- **CPU telemetry:** `App` instruments update, extract, render, audio, hot reload, and total frame time. Per-system timings are available through `FrameTimings::system_timings` and a `slowest_system()` helper.
- **GPU telemetry:** `Renderer::render_frame_full_timed()` uses `wgpu` timestamp queries when `TIMESTAMP_QUERY` is available on the active adapter. The path is opt-in via `TUNGSTEN_GPU_TIMING` because it blocks on GPU completion to read the timestamps back.
- **Canonical scenes:** `example-01-platformer` remains the broad feature scene; `example-02-sprite-stress` is the canonical sprite-throughput scene for perf capture.
- **Bench coverage:** Criterion suites now cover ECS, physics, and CPU-only render-data construction. These are intended as repeatable regression detectors, not exhaustive throughput claims.
- **Capture tooling:** `scripts/perf-capture.sh` and `docs/perf/profiling-workflow.md` define the repeatable Linux profiling workflow: release builds with frame pointers, smoke-frame-bounded runs, optional flamegraph/perf artifacts, and timestamped output directories under `perf-runs/`.

---

## Non-commitments

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
- GPU-compressed texture formats (KTX2, Basis Universal)
- Skeletal animation
- Streaming or async asset loading
- Per-platform asset variants
- Tweened transforms as a separate animation system
