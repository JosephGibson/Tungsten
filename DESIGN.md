# Tungsten — Design

## Status

Workspace `v0.39.0`; agent tooling and documentation only, with no engine change: decision and patch hand-off skills, `just physics-release`, perf capture and done-when rules in [profiling-workflow.md](docs/perf/profiling-workflow.md), and a 1.0 criteria discussion draft (`docs/plans/1.0-criteria-draft.md`). It follows 0.38's faster ball extract in example 01 (plan archived at `docs/plans/archive/ball-pit-extract.md`): zipped queries, a sprite cache and view culling. Before that came 0.37's dense-pile collapse fix (`D-094`, plan archived at `docs/plans/archive/physics-dense-pile-collapse.md`): one physics step advances at most `PhysicsConfig::max_step_dt` (1/30 s), so a slow frame no longer softens every contact, and example 01 caps its balls at 12,000. It follows M31, the last Phase 4 milestone (`D-093`, plan archived at `docs/plans/archive/phase4-milestone-31-mesh-particles-transitions.md`): particle configs can draw an instanced triangle mesh instead of a sprite quad, and a state change can run behind a screen transition (fade, radial wipe, dissolve, pixelate). It sits on 0.35's physics and manifest correctness pass (`D-088`–`D-092`, plan archived at `docs/plans/archive/p2-correctness-pass.md`): the frame dt is capped at 0.1 s, manifest roots validate and reload as one merged graph, stock post effects keep per-slot parameters and hot-reload their shaders, and a physics arrival pass stops a pushed body at the gate behind it. 0.34's GPU and render-path performance pass (`D-085`–`D-087`, plan archived at `docs/plans/archive/gpu-perf-pass.md`) came before it; unapproved experiments remain proposals in [benchmarks.md](docs/perf/benchmarks.md#open-proposals).

Phases 3 and 4 are complete (M25–M31; roadmap archived at `docs/plans/archive/phase4.md`). This document describes current architecture. Use [CHANGELOG.md](CHANGELOG.md) for dated release history, [DECISION_INDEX.md](docs/DECISION_INDEX.md) for rationale, and [LLM_INDEX.md](docs/LLM_INDEX.md) for source paths.

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

The app runs synchronously on the main thread; the audio callback and file watcher provide the two engine background paths. `crates/tungsten/src/app.rs` owns the order:

```text
setup:      parse config → construct App/World and resources
startup:    open window → initialize renderer → load merged manifests
            → user startup hook → initialize audio

redraw:     apply pending display settings → update DeltaTime
            → registered systems → particles → tweens
            → flush commands → rotate event queues → hot reload
            → apply pending post-AA request → extract → render/present
            → recycle extract buffers → forward audio commands
            → clear input edges → record telemetry → schedule next redraw
```

Systems run in registration order; there is no scheduler or dependency graph. A smoke run pins `DeltaTime.dt` to 1/60 s. Normal play uses elapsed time, capped at 0.1 s per frame (`D-088`); a frame cap delays the next redraw. Telemetry's `total` measures frame work, while `interval` measures the time between frame starts, including pacing waits.

### ECS

**Entity:** `Entity { index: u32, generation: u32 }`. Generational IDs catch stale-handle bugs. `entity.id()` returns `index` as `u32` for compatibility.

**Archetypal storage (M12, `D-083`):** Each archetype is a table of columns: a `Vec<Box<dyn AnyColumn>>` in the order of the archetype's sorted `TypeId` key, created with the archetype, plus a parallel `Vec<Entity>` row index. `AnyColumn` is a type-erased interface over `TypedVec<T>(Vec<T>)`. A row moves between archetypes value by value without boxing, in one merge walk over the two sorted keys. A column is found by type through a scan of the key (structural changes, query setup) or through the archetype's slot table, 128 one-byte slots that give every key type its own slot (`World::get`, `get_mut` and the command flush). Cost model: one downcast per archetype per type, then contiguous `Vec<T>` slice access.

**Archetype graph:** storage is `Vec<Archetype>` indexed by `ArchetypeId` plus `HashMap<Box<[TypeId]>, ArchetypeId>` for O(1) lookup by component set. Archetype `0` is the empty archetype. Freshly spawned entities start there. `add_edges` / `remove_edges` are lazy: the first `insert<T>` / `remove<T>` from an archetype builds the edge, and later transitions use O(1) cached lookup.

**Queries:** immutable queries are `query<A>()`, `query2<A,B>()`, `query3<A,B,C>()`, their `_opt2` forms with two optional columns, and the `_entities` variants; mutable ones are `query_mut`, `query2_mut`, `query3_mut`, `query3_mut_without` and `query2_opt2_mut`, which borrow their columns disjointly per archetype without `unsafe`. Cost model: one downcast per archetype per type, then sequential row access over contiguous `Vec<T>`.

**Resources:** singleton state lives in the `World` and uses the same access path as components. Examples: `DeltaTime`, `InputState`, `ActionMap`, `WindowSize`, `AssetRegistry`, `AudioCommands`, `PhysicsConfig`, `EventQueue<CollisionEvent>`, `CameraState`, `CameraController`, `DisplayState`, and `DisplayTelemetry`.

**Command buffer (M13, `D-039`, `D-084`):** systems record structural changes in the `CommandBuffer` resource, and `App` flushes it after the systems and before events, hot reload and extract. A buffer keeps one typed queue of values per inserted component type, so recording allocates nothing once the queues have grown, and `App` drains the same buffer every frame (`World::flush_reusing`). The flush spawns every pending entity first, then replays the commands in order; consecutive inserts on one entity apply as one archetype move.

**Event delivery (M14):** `EventQueue<T>` stores `previous` + `current`. Systems send into `current`. Readers normally use `iter()`, which yields `previous` first and then `current`, to avoid order-sensitive missed reads. `App` rotates queues once per frame after `CommandBuffer` flush and before hot reload, extract, and render.

**Runtime telemetry (M12+):** `FrameTimings` is a `World` resource. `App` measures stage timings inline with `std::time::Instant`, stores the latest frame totals, and keeps a per-system `Vec<(String, f32)>` in registration order. Render is split into acquire, encode, and submit/present. GPU-facing diagnostics live in `tungsten-render::GpuFrameTimings`, and the umbrella crate mirrors those diagnostics into the `World` after renderer init and after each frame. Display-facing diagnostics live in `tungsten::DisplayTelemetry`, which tracks the authoritative resolution, display mode, vsync intent, applied present-mode label, max-frame-latency hint, scale mode, and frame-rate cap.

**ECS error strategy (D-022):** panic on programmer errors such as insert on dead entity or wrong downcast; return `Option` / `Result` on runtime conditions such as entity not found or component absent.

**Render path (D-018):** systems mutate the `World` during `tick`. Extract functions receive `&World`, resolve string IDs → `TextureHandle` via `AssetRegistry`, and produce plain render data such as `SpriteBatch`, `QuadInstance`, and `TextSection` for `render_frame_full`. ID resolution happens during extract; the renderer draws resolved batches without mutable `World` access.

**Core/render seam:** `TextureHandle(u32)` lives in `tungsten-core`; no `wgpu` types appear in core. `tungsten` mediates the bridge: `Renderer::allocate_texture_handle` mints the handle (`D-048`); the loader uploads pixels under that key and passes it, atlas UVs and metadata to `AssetRegistry::register_sprite`. Many sprites can share one atlas handle. Core never calls into render. `tungsten-render` may depend on `tungsten-core` types (`D-007`).

**Render components (M15, `D-042`):** four engine-level component types live in `tungsten_core::components`:

- `Transform { position: Vec2, rotation: f32, scale: Vec2 }` — world-space pose. Rotation is in radians, CCW positive, applied around the quad centre by the sprite shader; scale multiplies the sprite's intrinsic pixel size per-axis.
- `Sprite { asset_id: String, color: [u8; 4], z_order: i32, material_id: Option<MaterialAssetId> }` — asset lookup + tint + stable ascending sort key.
- `Visibility { visible: bool }` — explicit render gate.
- `Tag { name: String }` — debug-friendly entity label for find-by-name lookups.

Physics `Position` stays separate (`D-033`). A free-fn `sync_position_to_transform(&mut World)` copies `Position.0` into `Transform.position` one-way; callers register it after `physics_step` when they want the post-physics position to reach the extract stage.

`SpriteInstance` carries the rotation and tint across the core/render seam as a 48-byte GPU-facing POD: position, size, rotation, `Unorm8x4` color, atlas `uv_min`/`uv_size`, normalized depth and padding. The WGSL pipeline multiplies the sampled texel by the tint and rotates around the quad centre. Every sprite path — component-driven, tilemap, and custom extracts alike — uses the same layout.

**Default sprite extract (`D-086`):** `App::run` installs `extract_sprites_default` when no custom sprite extract was set. It requires `Transform + Sprite + Visibility`, skips hidden or unresolved sprites, applies parallax and scale, resolves asset IDs through a small cache, and writes instances plus compact sort keys into reusable `ExtractScratch` buffers. It sorts by `(z_order, entity.id)` only when the query is not already in that order, then groups compatible atlas/filter/material/override/lit keys within each z-order run. Batches follow first-seen key order within the run; this can reorder overlapping sprites with different keys at the same z. It does not cull off-screen sprites. Tilemap extraction culls separately. Custom extracts retain the same render-data seam.

### Data-Driven Config

Config model: single `tungsten.json` at workspace root, loaded once at startup, then environment overrides. Missing file → defaults with warning. Invalid file → fatal error naming the bad field. Runtime display changes use `request_display_settings`; config-file edits require a restart.

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
    "depth_sort": "cpu_stable",
    "post_aa": "off",
    "bloom_max_mips": 6
  },
  "logging": { "level": "info" }
}
```

Display config semantics: checked-in display settings live under `display.*`. `display.present_mode` is authoritative when set to a concrete mode such as `"immediate"` or `"mailbox"`. When absent or `"auto"`, `display.vsync` selects between the auto-vsync and auto-no-vsync families. `display.max_frame_latency` is the requested frames-in-flight hint passed into `wgpu::SurfaceConfiguration`; backends may clamp it, so treat runtime telemetry as the configured hint unless the backend exposes stronger confirmation. Legacy `window.width`, `window.height`, `window.vsync`, `render.present_mode`, and `render.max_frame_latency` remain valid compatibility inputs, but `display.*` wins whenever both specify the same concern.

Parsed `logging.level` and `display.scale_mode` currently have no runtime application; examples initialize `env_logger` themselves. Pacing precedence and overrides are documented in [the profiling workflow](docs/perf/profiling-workflow.md#frame-pacing).

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
├── sounds/
├── tilemaps/
├── particles/
└── shaders/
```

Examples ship `examples/NN_name/assets/` with a local manifest. `App::set_manifest_roots` declares the load order; `load_all_merged` loads the composed result before user startup (`D-052`). Explicit scene files are the path-based exception (`D-046`). Materials and particle meshes (`particle_meshes`, `D-093`) are inline manifest entries.

**Opaque handles (D-016):** `tungsten-core` stores opaque `TextureHandle(u32)` IDs. `tungsten-render` owns GPU textures, samplers, and pipelines in internal pools keyed by those handles. The registry is the one game-facing lookup path.

## Subsystems

### Text — M7

Stack: `glyphon` + `cosmic-text` + `swash`. Responsibilities: font parsing, shaping, layout, and GPU rasterization. Decision: `D-026`. Fonts are registered in the manifest by ID under `fonts`. `TextSection` is extracted each frame. Text ignores the world camera; it stays screen-space while the world scrolls.

### Audio — M8

`cpal` (`D-027`) opens the audio device. `symphonia` (`D-028`) decodes `OGG` / `WAV` (PCM since `D-077`) / `MP3` / `AAC` at load time into `Vec<f32>` PCM, and no decoder types appear at runtime. A hand-rolled mixer (`D-029`) runs in the `cpal` callback thread. Game systems write `AudioCommand` values each tick. The callback drains commands through an `rtrb` wait-free SPSC ring (`D-034`, capacity `64`). Audio assets are not hot-reloadable; PCM buffers decoded at startup stay fixed for the session.

### Hot Reload — M9

`notify` (`D-031`; version pinned by Cargo.lock) runs on a dedicated background thread. File events cross to the main thread through `std::sync::mpsc`. A `50ms` debounce collapses editor double-writes. At the next frame boundary the main thread resolves file paths → asset IDs, decodes new data, uploads to GPU, and updates registry metadata and GPU resources. M25 (`D-057`) brings shaders into the same path: `.wgsl` edits validate through `wgpu::naga` and only commit to the live `ShaderModule` after the dependent pipeline rebuilds. Signature / bind-group-layout changes still require a binary rebuild, narrowing `D-023`. Invariant: do not break the registry-by-ID model; game code must not hold direct GPU handles.

**Supported reload matrix (`D-053`):** applies when the app enables hot reload. The watcher routes every manifest root, and an edit to any of them reloads the merged graph. Material→shader references are validated on the merged graph, so they may cross roots (`D-089`).

| Asset class | Single-file edit | Manifest-add | Manifest-remove |
| --- | --- | --- | --- |
| Sprite (`.png`/`.jpg`/`.jpeg`) | yes — `reload_sprite` with in-place overwrite for shrink/equal, `rebuild_atlas_for_filter` for growth | yes — registered with placeholder then atlas class rebuilt | warn-only; stale entry kept |
| Animation (`.json`) | yes — `reload_animation` replaces entry in `AnimationRegistry` | yes — inserted into `AnimationRegistry` | warn-only; stale entry kept |
| Tilemap (`.tmj`) | yes — `reload_tilemap` replaces entry, rejects unknown tileset sprite IDs | yes — inserted after tileset validation | warn-only; stale entry kept |
| Font (`.ttf`/`.otf`) | yes — `reload_font` swaps face data in `TextPipeline` | yes — added through `renderer.load_font` + `FontRegistry::register` | not applied; existing entries kept |
| Particle (`.json`) | yes — `reload_particle` swaps the `Arc<ParticleConfig>` under the same `AssetId` (`D-050`); a config naming an unregistered sprite or mesh is rejected | yes — inserted into `ParticleConfigRegistry` after sprite or mesh validation | warn-only; stale entry kept |
| Sound (decoded PCM) | **not supported** — mixer owns cloned PCM; session-static | **not supported** — no manifest-add path | n/a |
| Shader (`.wgsl`) | yes (body-edit only) — `reload_shader` re-validates through `wgpu::naga` and rebuilds the sprite pipeline **or** every material pipeline bound to that shader; signature / bind-group-layout changes still need a rebuild (`D-057`, narrowing `D-023`) | **not supported** — restart to register new shader IDs | not applied; existing entries kept |
| Material (`materials` section, `D-058`) | yes — existing `uniform_defaults` reload; changing the shader binding requires restart | yes — manifest reload allocates a new `MaterialAssetId` and calls `upload_material` | warn-only; stale entry kept |
| Particle mesh (`particle_meshes` section, `D-093`) | yes — changed `vertices` / `indices` re-upload under the same `ParticleMeshAssetId`, and live particles draw the new geometry on the next frame; an invalid mesh fails the reload and the last good graph stays | yes — registered and uploaded before the particle configs of the same reload | warn-only; stale mesh kept |
| SMAA stage shaders (M27, `D-059`) | yes (body-only) — `smaa_edge`, `smaa_blend_weights`, `smaa_neighborhood_blend` follow the M25 shader path; `Renderer::reload_shader` re-validates and rebuilds only the affected `SmaaPipeline` stage. SMAA `area` / `search` LUT binaries are explicitly out-of-matrix (engine-internal `include_bytes!`) | n/a (engine-internal stage shaders, fixed set of three) | n/a |
| Bloom stage shaders (M28, `D-060`) | yes (body-only) — `bloom_threshold`, `bloom_downsample`, `bloom_upsample`, `bloom_composite` follow the M25 shader path; `Renderer::reload_shader` re-validates and rebuilds only the affected `BloomPipeline` stage via `rebuild_stage_with_module`. The `BloomPyramid` texture is engine-internal and explicitly out-of-matrix; signature changes still need a rebuild | n/a (engine-internal stage shaders, fixed set of four) | n/a |
| Stock post-effect shaders (M26, `D-058`, `D-091`) | yes (body-only) — the 17 stock effects follow the M25 shader path; `Renderer::reload_shader` re-validates and rebuilds only the affected stock pipeline via `rebuild_stock_with_module`; signature changes still need a rebuild | n/a (engine-internal, fixed set of 17) | n/a |
| Lit sprite shader + helpers (M29, `D-061`) | yes (body-only) — `lit_sprite` rebuilds the `LitSpritePipeline` via `Renderer::reload_shader` → `LitSpritePipeline::rebuild_with_shader`; `emissive_mask` and `rim_light` are validated and cached but bound to no pipeline directly (helpers for material composition) | **not supported** — restart to register new shader IDs | not applied; existing entries kept |
| Sprite normal_map / emissive_mask siblings (M29, `D-061`) | yes — sibling PNG edits route through `reload_sprite` (mapped via reverse path lookup); `write_subtexture_lit` updates the matching cell in the lit atlas pool, full repack on grow | yes for new sprite entries; adding sibling fields to an existing sprite requires restart | warn-only; stale lit page kept |
| `input.json` | yes — `reload_action_map` merges with defaults and swaps `ActionMap` | n/a | n/a |
| `manifest.json` | yes — `reload_manifest` rebuilds the merged graph of every root and walks every class above (`D-089`) | n/a | n/a |

Audio is session-static by design: `AudioSystem::init` reads every decoded `SoundData::samples` into a callback-owned `HashMap<AudioHandle, Vec<f32>>` (`D-027` / `D-029` / `D-034`), and the mixer closure captures that map at startup. Adding a runtime PCM-swap command is a future milestone; until then, "sound hot reload" is explicitly out of scope and the watcher logs at `debug` when a `.ogg`/`.wav`/`.mp3` under the asset tree changes.

### Tilemaps — M10

Extension: `.tmj`. Schema: Tiled-compatible (`D-032`). Core data types `TilemapData`, `TilemapRegistry`, and `TilemapInstance` live in `tungsten-core` as plain data; no `wgpu`. `extract_tilemaps(&World)` resolves visible tiles into `SpriteBatch`es. Tilemaps reuse the sprite pipeline; there is no new `wgpu` pipeline. `CameraState` (`position`, `zoom`, `rotation`) feeds view-projection into sprite and quad pipelines, and `visible_world_aabb()` keeps tile culling proportional to viewport size while over-covering safely under rotation. `LayerKind::Collision` layers are accepted by the loader but skipped by extract; physics reads them directly.

### Physics — M11

Components: `Position`, `Velocity`, `Collider` (AABB or circle, with offset) and `RigidBody` (static or dynamic). Resources: `PhysicsConfig` and `EventQueue<CollisionEvent>`. Register `physics_step` as a system; default gravity is `Vec2::ZERO`.

Each frame gathers bodies and collision tiles once into dense proxy arrays and writes bodies back after the last substep (`D-066`). The spatial grid uses a direct cell table for compact bounds and a prefix-sum hashed table otherwise (`D-080`; default cell size 32 px). Broadphase pairs persist across substeps under travel budgets (`D-075`); exhausted proxies repair their own pairs, while contact wakes or many simultaneous budget trips rebuild the list (`D-081`). A statics-only sweep grid handles the safety-net path (`D-080`).

Signed-distance AABB/AABB, circle/circle and AABB/circle contacts support speculative CCD (`D-064`). After a substep's position integration, an arrival pass re-checks every body whose velocity the solve changed against the neighbours that velocity reaches, clamps it and redoes its move, so a pushed body stops at a wall instead of crossing it (`D-092`). The solver uses warm-started soft constraints, restitution and a configured fixed substep count (default 4; `D-063`, `D-076`), plus deterministic island sleeping (`D-065`, `D-082`). The step remains serial (`D-067`). Collision tile layers become static AABB proxies, gathered by a full-map scan each frame.

Variable frame time, capped at 0.1 s per frame (`D-088`), is split across the configured substeps. One step advances at most `PhysicsConfig::max_step_dt` (1/30 s by default), so below 30 FPS physics runs slow instead of softening its contacts (`D-094`). There is no fixed-step accumulator. Current workload limits and measured costs belong in [benchmarks.md](docs/perf/benchmarks.md), rather than an old stress-scene timing here.

### Archetypal ECS — M12

Storage design is described in [§ECS](#ecs). The table is the historical M12 comparison, not a current benchmark-suite baseline. Decision to proceed: `D-036` (cites `D-030`).

| Benchmark | Archetypal | Naive | Ratio |
| --- | --- | --- | --- |
| `query::<Position>` — 10k entities | `6.8 µs` | `43.4 µs` | `~6×` |
| `query2::<Position, Velocity>` — 10k, 1 archetype | `7.1 µs` | `1 424 µs` | `~200×` |
| `query2::<Position, Velocity>` — 10k, 5 archetypes | `7.5 µs` | `1 424 µs` | `~190×` |
| spawn + 3 inserts × 10k | `4.4 ms` | `—` | `—` |

Command buffers shipped in M13 and were optimized by `D-084`. Parallel scheduling, change detection, reactive queries and raw-byte columns have no current implementation commitment.

### Input Mapping — M19

`tungsten-core::input::action_map::ActionMap` is a `World` resource that maps named actions to one or more `Binding`s (`Key`, `Mouse`, `Scroll`). It loads from `input.json` at the workspace root, merges with `default_map()` so missing actions still resolve, and exposes `is_pressed`, `just_pressed`, and `just_released` against the live `InputState`. `InputState` was extended with cursor position/delta and per-frame line and pixel scroll deltas; scroll edges auto-release on `begin_frame` so `just_pressed("…wheel_up")` fires once per notch.

Engine-owned actions (`engine_toggle_hud`, `engine_toggle_vsync`, `engine_toggle_fullscreen`, `engine_exit`) live in defaults so a missing or partial `input.json` still ships HUD/display/exit controls. Hot reload watches `input.json` through `HotReloadWatcher::extra_files`; on a ready event `asset_loader::reload_action_map` swaps the resource at the frame boundary. `ActionMap::persist` writes the file back through a temp-file + rename, replacing the `actions` JSON object while retaining surrounding source text where possible. Action entries are serialized in sorted order; JSON comments are not supported. Decision: `D-045`.

### Scene / State System — M20

`tungsten::state::StateStack` is a `World` resource driving a `Vec<Box<dyn GameState>>` through deferred `request_push` / `request_pop` / `request_replace` queues. A single engine-owned `state_dispatcher_system`, registered immediately after `__display_input`, drains the pending queue each frame and fires the transition matrix: `push` triggers `old.on_pause` → `new.on_enter`; `pop` triggers `old.on_exit` (after auto-despawn) → `next.on_resume`; `replace` triggers `old.on_exit` (after auto-despawn) → `new.on_enter`. `on_pause` / `on_resume` default to no-op so Pause overlays Gameplay without tearing the scene down. The dispatcher also mirrors the active state id into `HudActiveState` so the M18 state row keeps rendering correctly.

Scene-owned entities carry a `SceneEntity { state_id }` marker. On state exit the dispatcher walks `query::<SceneEntity>()` and enqueues `CommandBuffer::despawn` for each matching entity before the user's `on_exit` runs; the engine's post-systems `CommandBuffer` flush (`D-039`) applies the despawns so the last frame of the exiting state already sees its scene entities gone and the first frame of the next state already sees its scene entities present. `tungsten_core::assets::scene::SceneData` is a minimal JSON schema that reuses the M15 components — each `SceneEntry` maps to `Transform + Sprite? + Visibility + Tag?` and may carry scene-authored tweens. `asset_loader::spawn_scene(world, &data, state_id)` funnels every entry through `CommandBuffer` so the spawn lands at the normal frame boundary; sprite id validation is intentionally deferred to the extract path, matching the tilemap behaviour, and scene tween authoring inserts at most one `Tween` component per spawned entity. `ActionMap::default_map()` now ships `state_start` (`Enter`), `state_pause` (`KeyP`), and `state_back` (`Backspace`) so the example flow works without edits to `input.json`. Decision: `D-046`.

A state change can run behind a screen transition (M31, `D-093`): `request_push_transition`, `request_pop_transition` and `request_replace_transition` take a `Transition` and return `false` when one is already queued or active. The dispatcher applies plain requests first, activates a queued transition, advances the active one by `DeltaTime` and runs its command on the frame the `Out` phase completes, with the same hooks and despawns as a plain request. States keep updating during both phases; `is_transitioning`, `transition_state` and `transition_cover` expose the progress.

### Tween System — M24

`tungsten_core::tween` adds a component-driven animation surface: `Tween` holds one timing model plus a `Vec<TweenChannel>` so position, rotation, scale, and sprite color can animate together on one entity. Easings are a closed `enum` (`Easing`) with a pure `apply(t)` implementation; repeat modes are `Once`, `Loop`, `PingPong`, and `Times(n)`.

`tungsten::tweens::tween_tick_system` advances every live tween from `DeltaTime`, writes `Transform` / `Sprite` fields in place, emits `TweenComplete` through `EventQueue<TweenComplete>`, and defers terminal `Tween` removal through `CommandBuffer` so archetypes never mutate mid-iteration. The frame slot is `particles -> tweens -> flush commands -> flush events`, which keeps tween writes visible to extract/render in the same frame while preserving the fixed frame-boundary mutation/event rules. Scene JSON can author tweens directly, making state-transition fades and simple data-driven motion possible without bespoke example systems. Decisions: `D-054`, `D-055`, `D-056`.

### Materials and Post-Stack — M26

Manifest `materials` bind a shader ID to a 256-byte `MaterialUniformDefaults` block (`D-058`). `Sprite.material_id` selects a material pipeline; `None` uses the built-in sprite pipeline. `UniformOverrideBlock` and `TweenChannel::Uniform*` provide entity-local animated overrides alongside authored defaults, preserving the one-`Tween`-per-entity rule.

Core owns the reorderable `PostStack`/`PostPass` data. Render records 17 ordinary stock effects plus bloom, using pooled ping-pong targets as needed; an empty stack applies no effects. SMAA is a fixed tail, outside the reorderable stack. Since `D-087`, the last full-screen stage writes the swapchain directly, except on screenshot frames. GPU objects are cached by target generation (`D-085`). Each post-stack slot owns its stock-effect parameters, so one effect can repeat with different values (`D-090`). Distinct material override batches currently share uniform storage; per-batch parameter independence needs a renderer fix.

### Game Feel — M30

`ParallaxLayer.scroll_factor` remaps sprite positions during extract against `CameraState.position`, keeping one camera matrix and `Sprite.z_order` as the ordering authority (`D-073`). Tilemaps do not use this remap. `CameraController` stores the trauma envelope (`shake_trauma`, `shake_decay`, `shake_max_offset`) over the existing sine carrier; `CameraState` remains output.

`SpriteSquashStretch` plus `SquashStretchState` apply a symmetric one-shot scale envelope through `game_feel.rs`, independent of `Tween`. Register the trigger system before the squash tick, and `shake_tick_system` before `camera_update_system`. Examples opt into these systems. Both event types are registered by `App::new`.

### Mesh Particles and Screen Transitions — M31

A particle config with `"render": { "kind": "mesh", "mesh": "<id>" }` draws each particle as an instanced triangle mesh from the manifest's inline `particle_meshes` section (`D-093`). The particle stays one entity (`D-051`) and carries `MeshParticle` in place of `Sprite`; emit, tick, budget and despawn are shared with quad particles. `extract_mesh_particles` batches the visible ones per mesh, `Renderer::update_mesh_particles` uploads the instances, and `MeshParticlePipeline` draws one indexed call per mesh inside the scene pass, after the sprites. Mesh particles therefore draw above every sprite, untextured and unlit, with a per-instance color.

`tungsten::transition` holds `Transition` (an effect and a duration per phase, plus an `Easing`) and the closed `TransitionEffect` enum: `Fade`, `WipeRadial`, `Dissolve`, `Pixelate`. Each effect maps a cover in [0, 1] to its stock post pass, because the four shaders share no progress convention. `App::stage_render` appends that pass to a copy of the user's `PostStack` as its last slot, so the transition survives a state that rebuilds the stack and runs before the SMAA tail. Screen-space text draws after the post stack and is not covered; a game fades its own text with `1 − StateStack::transition_cover()`.

### Presentation AA / SMAA — M27

`RenderConfig.post_aa` (`PostAaMode::{Off, SmaaLow, SmaaMedium, SmaaHigh, SmaaUltra}`, `#[non_exhaustive]`) selects an optional SMAA 1x presentation tail. `Off` is the default and produces the byte-identical M26 frame. The render path is:

```
Scene → PostStack → [optional SMAA tail] → Text Overlay
Scene → PostStack → [optional SMAA tail → PresentSource] → Text Overlay → Present Blit → Swapchain   (capture frames)
```

Since `D-087` the last full-screen stage of a frame renders into the swapchain (SMAA's neighborhood pass; without SMAA the last post pass, or bloom's composite; with neither the scene pass, or its MSAA resolve), and the text overlay draws there. A frame with a screenshot armed keeps the second path, so the screenshot reads the source it always read.

When `post_aa != Off` the renderer allocates `SmaaEdges` (`Rg8Unorm`), `SmaaBlend` (`Rgba8Unorm`), and `PresentSource` (matching the swapchain format, `RENDER_ATTACHMENT | TEXTURE_BINDING | COPY_SRC`). `SceneColor`, `PostPing`, and `PostPong` are recreated with a non-sRGB twin in `view_formats` so the SMAA edge/blend/neighborhood passes sample gamma-encoded values; the rest of the frame keeps using the primary view. The text overlay always runs after the SMAA tail, so screen-space text is never sampled by SMAA. On a capture frame the present blit and the screenshot source `PresentSource` when `post_aa != Off` and `SceneColor` / the post-stack final target otherwise; no other frame writes `PresentSource`.

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

The four stage shaders (`bloom_threshold`, `bloom_downsample`, `bloom_upsample`, `bloom_composite`) follow the M25 stock-shader pattern: compile-time `include_str!` mirror under `crates/tungsten-render/src/shaders/stock/`, byte-equal mirror under `assets/shaders/stock/`, manifest-tracked by the four stage names (renderer-local IDs are not a cross-crate contract), body-edit hot-reload through `Renderer::reload_shader` → `BloomPipeline::rebuild_stage_with_module` with `naga` validation and last-known-good pipeline retention on failure. The per-subpass UBO reuses the engine-wide 256-byte `UniformOverrideBlock` layout: `vec4[0]` carries `inv_src_size` (per-mip), `vec4[1]` is the reserved `composite_tint`, the `f32s` block carries `[threshold, knee, intensity, radius]`, and the `i32s` block carries `[mip_count, dst_level, pass_kind, _]`. `SceneColor` stays sRGB — only the pyramid is HDR. With `PostStack` empty and `post_aa = Off` the pyramid exists but is never written to or sampled, so the captured frame matches the M27 baseline. Decision: `D-060`.

### 2D Forward Lighting — M29

`Light { kind, color, intensity }` and the closed `LightKind::{Point { radius, falloff }, Directional { angle }}` enum live in `tungsten-core::components`; `AmbientLight(Vec3)` is a world resource defaulting to `Vec3::ONE`. `LIGHT_CAP = 16` lives in `tungsten-core::lighting`; the render-side mirror is `tungsten-render::LIT_LIGHT_CAP`. The renderer owns one 544-byte `LightUbo` (16 lights × 32 bytes + `vec4<u32>` count_pad + `vec4<f32>` ambient) under `LightingResources` bound at group 2 of a sibling `LitSpritePipeline`. The lit pipeline reuses `SpritePipeline::vertex_layouts()` and binds a parallel albedo + normal + emissive bundle at group 1 — three texture views + one filtering sampler — so vertex/instance buffers stay interchangeable with the unlit and material paths.

Sprites with optional `normal_map` / `emissive_mask` sibling files in the manifest pack into parallel atlas canvases keyed by the same `PackedSprite` placement output: the asset loader runs `pack_shelf` once over the albedo inputs, then fills albedo, flat-normal-default, and emissive canvases page-by-page. Albedo uploads as `Rgba8UnormSrgb`; normal and emissive upload as `Rgba8Unorm` so tangent-space vectors and mask intensities are not gamma-decoded. The `SpriteAsset.lit_atlas: Option<TextureHandle>` marker tags only sprites with a valid normal sibling, and `extract_sprites_default` flips `SpriteBatch.lit` on that axis. Lit + material is intentionally out-of-scope in M29: a sprite carrying both warns and uses lit. `extract_lights` runs every frame, queries `(Transform, Light)`, packs each into `GpuLight`, sorts directional-first then nearest-to-AABB-squared-distance, truncates to `LIGHT_CAP`, and packs the result + ambient into the per-frame `LightUbo`. With no lit sprites and an empty light list the captured frame stays byte-identical to the M28 baseline — the unlit pipeline is unchanged and the lit pipeline never runs.

The lit shader (`assets/shaders/lit_sprite.wgsl`) and helpers (`assets/shaders/stock/emissive_mask.wgsl`, `assets/shaders/stock/rim_light.wgsl`) are manifest-tracked by name, with renderer-local shader IDs. The lit shader is compiled directly from `assets/shaders/lit_sprite.wgsl` with no `src/` mirror; the helpers have byte-equal mirrors in `crates/tungsten-render/src/shaders/stock/`. Lit body edits hot-reload through `Renderer::reload_shader` → `LitSpritePipeline::rebuild_with_shader` with `naga` validation and last-known-good pipeline retention on failure. The helpers are validated only — no pipeline behind them — so material authors and future milestones can fold `emissive_contribution` and `rim_term` into their own WGSL. Decision: `D-061`.

### Performance and Profiling

`FrameTimings` records CPU stages, per-system wall time and the frame interval. Renderer GPU timestamps are opt-in diagnostics and block on readback; they must stay out of CPU timing captures. Criterion measures isolated primitives.

[profiling-workflow.md](docs/perf/profiling-workflow.md) owns capture configuration, telemetry semantics, comparability, verdicts, capacity search and profiling commands. [benchmarks.md](docs/perf/benchmarks.md) owns the six workloads, knobs, guards, dated measurements and open performance proposals (`D-078`).

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
