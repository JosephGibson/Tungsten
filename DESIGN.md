# Tungsten — Design

## Status

Workspace `v0.56.0`; M42, Phase 5's eleventh milestone (W15c, plan archived at `docs/plans/archive/1.0/phase5-milestone-42-additive-extracts.md`): the `Extracts` resource holds the frame's sprite, quad and text channels, each its default or a game's replacement followed by what games and plugins add in registration order; the default sprite channel draws the tilemaps at the far plane, then the `Sprite` entities; `App::set_extract_*` replace a channel's base; `extract_tilemap_layers` and lit tiles let example 01 draw its tile layers through the engine, and the template's text goes through its plugin. The physics hashes, the row digests and the pixel fixtures are unchanged (`D-138`). Before that came M41, Phase 5's tenth milestone (W3b, plan archived at `docs/plans/archive/1.0/phase5-milestone-41-fixed-step-interpolation.md`): `Time` holds a bounded fixed-step accumulator, so `fixed_update` runs zero or more times a frame at 1/60 s with `Time::delta()`, input edges and event queues on the step inside it, set by `tungsten.json`'s `time` section; bodies spawned through `RigidBodyBundle` carry `PrevPosition`, which `physics_prev_snapshot` takes before each step, and `PhysicsPlugin`'s `physics_sync` draws them at `alpha` between their last two steps; example 01 runs on the fixed loop and draws its bodies interpolated. At the pinned 1/60 s every frame runs one step with `alpha` 0, so the physics hashes, the row digests and the pixel fixtures are unchanged (`D-129`, `D-137`). Before that came M40, Phase 5's ninth milestone (W15b, plan archived at `docs/plans/archive/1.0/phase5-milestone-40-tuple-queries-bundles.md`): `tungsten_core::ecs::query` holds tuple queries, `World::query::<(Entity, &A, Option<&B>)>()` with `With`/`Without` filters and the slice forms, which take over `query` and `query_mut` from the thirteen arity-named functions, now deprecated to W4b; `ecs::bundle` holds `Bundle`, `spawn_with` and `insert_bundle`, one archetype move per spawn, with `RigidBodyBundle` for the physics components; `World::resource::<T>()` panics naming a missing resource. The physics hashes, the row digests and the pixel fixtures are unchanged; the lone mutable column's cost is recorded and the default extract reads the slice form (`D-130`, `D-135`). Before that came M39, Phase 5's eighth milestone (W3a, plan archived at `docs/plans/archive/1.0/phase5-milestone-39-game-clock-timers.md`): `tungsten_core::time` holds the `Time` resource, which `App` advances once a frame: a real clock and a game clock with time scale, pause, elapsed time on both and a frame index, beside a plain `Timer`; engine systems read the game clock and screen transitions the real one, and `DeltaTime` is deprecated for W4b to remove (`D-129`, `D-134`). The physics hashes, the row digests and the pixel fixtures are unchanged at scale 1. Before that came M38, Phase 5's seventh milestone (W15a, plan archived at `docs/plans/archive/1.0/phase5-milestone-38-schedule-stages-plugins.md`): `tungsten_core::schedule` holds a `Schedule` of five closed stages with named systems, stage-local `before`/`after` constraints and plugins, which `App` drives and the headless harness steps; the engine registers as `DefaultPlugins`, the template and examples 01, 03 and 04 name no engine system, and example 01's input and contact systems run in `fixed_update` (`D-128`, `D-133`). `WindowSize` moved to core, `InspectorState`'s row functions are deprecated for `InspectRegistry`, and the `particles` benchmark row owns its three named systems. Before that came 0.51's glyph gate and frame-loop gate, run in one owner session and signed on 2026-10-06 ([gate records](docs/plans/1.0/implementation-plan.md#11-gate-records)): the glyph path is several glyphon renderers on one atlas with a deferred trim (T1b), with UI laid out in logical units and drawn at physical size (`D-126`, `D-127`); the frame loop is to get a `Schedule` of five closed stages with plugins in core, a `Time` resource with a bounded fixed step and interpolation on by default, and tuple queries with bundles (`D-128`–`D-130`); the fork-join extract (R1) leaves 1.0 with no worker mechanism (`D-131`), and the W6 and W13 tiers come from the acceptance game (`D-132`). W15, W2 and W3 graduated into their workstream files. No engine code changed. Before that came M37, Phase 5's sixth milestone (W1 M1, plan archived at `docs/plans/archive/1.0/phase5-milestone-37-core-ui-model.md`): the core UI model, one `UiTree` resource with generational widget IDs, roots on layers, roles and labels in node data, a theme-token style model with text-only inheritance, layout solved by Taffy behind Tungsten-owned style types, hit testing and keyboard focus, tested headless through a fixed-advance text double and `UiHarness`; the `TextNodeStore` seam beside `TextMeasure` (`D-124`, `D-125`). Nothing draws until M2. Before that came M36, Phase 5's fifth milestone (W9a with W12a, plan archived at `docs/plans/archive/1.0/phase5-milestone-36-licence-notices-template.md`): every release archive carries the third-party license notices of what its binaries link (`D-122`), and a game lives in its own folder: the engine owns its font, the action map reloads from its own path, and `templates/basic`, the supported game layout, is checked in the workspace and from a copy outside it, with the getting-started guide's first draft ([Standalone Games](#standalone-games-and-the-template--m36), `D-123`). Before that came M35, Phase 5's fourth milestone (W11a with W7a, plan archived at `docs/plans/archive/1.0/phase5-milestone-35-logs-crash-reports.md`): a game names itself in `game.id` and gets a per-user folder, and `App::new` installs one engine logger with a log file per run and a panic hook that writes crash files (`D-119`); release builds ship their debug info in a separate archive that `scripts/crash-report.py` symbolizes against, probed in every release run (`D-120`); Windows release builds open no console, and CI runs the tests on Windows too (`D-121`). Before that came 0.47's roadmap page work: the 1.0 roadmap catalog gives each stop its stages from `tungsten-next`'s Flow table and each session stage a recommended mode, model and effort, which the page draws (`D-118`). Before that came M34, Phase 5's third milestone (W1 M0a, plan archived at `docs/plans/archive/1.0/phase5-milestone-34-text-engine-split.md`): text splits into a device-free `TextEngine` and the GPU `TextPipeline`, core holds the neutral text types and `TextMeasure`, and retained text nodes measure min-content, max-content and definite widths on one shaped buffer (`D-117`); the manifest groups faces into font families with a fallback chain (`D-115`); and text draws with packaged fonts only unless `render.system_fonts` is set (`D-116`). Before that came M33, Phase 5's second milestone (W2 R0, plan archived at `docs/plans/archive/1.0/phase5-milestone-33-interned-asset-ids.md`): sprites name their asset by an interned `SpriteAssetId`, and animation frames hold IDs, while files keep names (`D-113`); the default extract skips stock-pipeline sprites outside the view render projects with, keeping their sort keys so batch order does not change (`D-114`). Before that came 0.44's test-suite overhead pass (plan archived at `docs/plans/archive/test-suite-overhead.md`): example 01's route test replays known launches, redundant unit tests go, dev and test builds keep debuginfo in unpacked `.dwo` files (`D-111`) and CI builds the benchmarks in the dev profile (`D-112`), so `cargo test --workspace` takes 1.9 s instead of 10.9 s. Before that came M32, Phase 5's first milestone (W14a, plan archived at `docs/plans/archive/1.0/phase5-milestone-32-headless-harness.md`): `App::run_frame` is the frame body the window loop runs, and `tungsten::testing::Harness`, behind the umbrella's `testing` feature, runs it with no window, renderer or audio device at a pinned dt, so tests read the world, events, the extract and the audio commands between frames (`D-110`); example 01's frame tests run on it. Before that came 0.42's 1.0 Step 0, the planning tooling every milestone plan needs: `just repo-check` reads the 1.0 plan folder, the on-demand index may hold 12 KiB with `AGENTS.md`'s asset rules moved to `docs/assets.md` (`D-106`), a `tungsten-milestone` skill holds the milestone plan skeleton, and `just api` keeps each library crate's public surface in `api/`, checked at release (`D-107`). The owner signed the 1.0 definition gate, W4, W9, W11, W12 and W14 graduated into their workstream files, sessions now leave work uncommitted until the release commit (`D-105`), each milestone releases from its own run session and two small neighbours may share a release (`D-108`), and the 1.0 roadmap page reads a checked catalog, `docs/plans/1.0/roadmap.json`, from its database (`D-109`). The gate (0.41) defined 1.0 as a complete engine for making games and a stable library, tested by one acceptance game, a top-down survivors-like auto-shooter in its own repository (`D-102`); semver will cover the `tungsten` crate's API, the game file formats and the CLI, not `wgpu` or `winit` types (`D-103`); and `cargo-public-api` snapshots the public surface each milestone (`D-104`). The 1.0 plans live in `docs/plans/1.0/` (start at its README), and 0.41 also stopped the SMAA neighborhood pass encoding the frame to sRGB twice. Before that came the 0.40 QA and cleanup pass (plan archived at `docs/plans/archive/qa-cleanup-0.40.md`): eight bugs fixed (`D-099`–`D-101`), dead API, unused dependencies and probe tests removed, `tungsten-core` built at opt-level 1 in dev (`D-096`), captures under background load marked invalid (`D-095`) and plan work committed locally by agents (`D-097`, `D-098`). Before that came 0.39's agent tooling and perf capture rules, and 0.38's faster ball extract in example 01 (plan archived at `docs/plans/archive/ball-pit-extract.md`): zipped queries, a sprite cache and view culling. Before that came 0.37's dense-pile collapse fix (`D-094`, plan archived at `docs/plans/archive/physics-dense-pile-collapse.md`): one physics step advances at most `PhysicsConfig::max_step_dt` (1/30 s), so a slow frame no longer softens every contact, and example 01 caps its balls at 12,000. It follows M31, the last Phase 4 milestone (`D-093`, plan archived at `docs/plans/archive/phase4-milestone-31-mesh-particles-transitions.md`): particle configs can draw an instanced triangle mesh instead of a sprite quad, and a state change can run behind a screen transition (fade, radial wipe, dissolve, pixelate). It sits on 0.35's physics and manifest correctness pass (`D-088`–`D-092`, plan archived at `docs/plans/archive/p2-correctness-pass.md`): the frame dt is capped at 0.1 s, manifest roots validate and reload as one merged graph, stock post effects keep per-slot parameters and hot-reload their shaders, and a physics arrival pass stops a pushed body at the gate behind it. Unapproved experiments remain proposals in [benchmarks.md](docs/perf/benchmarks.md#open-proposals).

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
| 3. Math / primitive (solved problem) | `glam`, `bytemuck`, `pollster`, `rtrb`, `taffy` (`D-124`) |

Reject crates that would hand over work this project is supposed to build. Examples: `bevy_ecs`, `hecs`, `rapier2d`, `rodio`. Borderline cases require a `DECISIONS.md` entry.

## Stack

| Concern | Choice | Role |
| --- | --- | --- |
| Windowing | `winit` | OS window and event abstraction |
| Rendering | `wgpu` | GPU API abstraction |
| Math | `glam` | Vectors, matrices, transforms |
| UI layout | `taffy` | Flexbox and block solving behind core's own `LayoutStyle` types; never public (`D-124`) |
| ECS | hand-rolled | Archetypal storage built in-project |
| Config | `serde` + `serde_json` | JSON schema derive + parsing |
| Image | `image` | PNG decoding to CPU bitmaps |
| Logging | `log` + `env_logger` | Standard facade; the one engine logger `App::new` installs, to stderr and a log file per run (`D-119`) |
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
            → plugins build the schedule (DefaultPlugins, then the game's)
startup:    resolve the schedule → open window → initialize renderer
            → load merged manifests → user startup hook → initialize audio

redraw:     apply pending display settings → advance Time
            → startup (first frame only) → pre_update
            → fixed_update once per fixed step (zero or more) → update
            → exit check → post_update
            → flush commands → flush events → hot reload
            → apply pending post-AA request → extract → render/present
            → recycle extract buffers → forward audio commands
            → clear input edges → record telemetry → schedule next redraw
```

The redraw arm runs `App::run_frame`, which holds the stages from the display apply through telemetry, then exits on an `engine_exit` press or schedules the next redraw. `tungsten::testing::Harness`, behind the umbrella's `testing` feature, runs the same `run_frame` with no window, renderer, audio device or startup hook (the `Startup` stage runs on its first step), at a pinned dt that advances `Time`'s real clock as given; tests read the world, events, the extract and the audio commands between frames (`D-110`).

Systems run in the five stages a `Schedule` resolves, `startup`, `pre_update`, `fixed_update`, `update` and `post_update`: within a stage a stable topological sort of the systems' `before`/`after` constraints, ties in registration order (`D-128`; the M38 section below). `fixed_update` runs once per fixed step the frame's `Time` owes, zero or more times (`D-129`, `D-137`; the M41 section below). The frame's first step advances `Time` (`D-134`; the M39 section below): a smoke run pins the real clock's dt to 1/60 s, normal play uses elapsed time, capped at 0.1 s per frame (`D-088`), and the game clock is that dt times the scale, zero while paused; a frame cap delays the next redraw. Telemetry's `total` measures frame work, while `interval` measures the time between frame starts, including pacing waits.

### ECS

**Entity:** `Entity { index: u32, generation: u32 }`. Generational IDs catch stale-handle bugs. `entity.id()` returns `index` as `u32` for compatibility.

**Archetypal storage (M12, `D-083`):** Each archetype is a table of columns: a `Vec<Box<dyn AnyColumn>>` in the order of the archetype's sorted `TypeId` key, created with the archetype, plus a parallel `Vec<Entity>` row index. `AnyColumn` is a type-erased interface over `TypedVec<T>(Vec<T>)`. A row moves between archetypes value by value without boxing, in one merge walk over the two sorted keys. A column is found by type through a scan of the key (structural changes, query setup) or through the archetype's slot table, 128 one-byte slots that give every key type its own slot (`World::get`, `get_mut` and the command flush). Cost model: one downcast per archetype per type, then contiguous `Vec<T>` slice access.

**Archetype graph:** storage is `Vec<Archetype>` indexed by `ArchetypeId` plus `HashMap<Box<[TypeId]>, ArchetypeId>` for O(1) lookup by component set. Archetype `0` is the empty archetype. Freshly spawned entities start there. `add_edges` / `remove_edges` are lazy: the first `insert<T>` / `remove<T>` from an archetype builds the edge, and later transitions use O(1) cached lookup.

**Queries (M40, `D-130`, `D-135`):** a query names its data as a type, `&A`, `&mut A`, `Option<&A>`, `Option<&mut A>` or `Entity`, alone or in a tuple of up to eight, and may add a filter as a second type parameter, `With<T>`, `Without<T>` or a tuple of up to four, checked once per archetype: `world.query::<(Entity, &A, Option<&B>)>()` and `query_filtered::<&Transform, With<Player>>()` read over `&self`, `query_mut` and `query_mut_filtered` write over `&mut self`, and the four `_slices` forms yield `(rows, slices)` per matching archetype for a loop written by hand, with `OptionalColumn` zipping an optional column in. Rows come in archetype order, then row order. Every mutable column of an archetype is borrowed through one `slice::get_disjoint_mut` call, and an access check panics with the query's type name when two items name one component, so the module has no `unsafe`; the traits are sealed. The thirteen arity-named functions (`query2`, `query3_mut`, `query_entities` and the rest) are `#[deprecated(since = "0.54.0")]` shims until W4b removes them. Cost model: one downcast per archetype per type, then sequential row access over contiguous `Vec<T>`; a lone mutable column with a trivial body reads about 0.2 ns a row slower than the shims' index loop, and the default sprite extract reads its six-item shape through the slice form because the tuple row iterator spills its state to the stack (`D-135`).

**Resources:** singleton state lives in the `World` and uses the same access path as components. Examples: `Time`, `InputState`, `ActionMap`, `WindowSize`, `AssetRegistry`, `AudioCommands`, `PhysicsConfig`, `EventQueue<CollisionEvent>`, `CameraState`, `CameraController`, `DisplayState`, and `DisplayTelemetry`. `World::resource::<T>()` and `resource_mut` return a resource the World always holds, or panic naming `T`; `get_resource` and `get_resource_mut` are the `Option` forms (`D-135`).

**Command buffer (M13, `D-039`, `D-084`):** systems record structural changes in the `CommandBuffer` resource, and `App` flushes it after the systems and before events, hot reload and extract. A buffer keeps one typed queue of values per inserted component type, so recording allocates nothing once the queues have grown, and `App` drains the same buffer every frame (`World::flush_reusing`). The flush spawns every pending entity first, then replays the commands in order; consecutive inserts on one entity apply as one archetype move. A bundle spawns an entity straight into its archetype (`D-130`, `D-135`): `World::spawn_with((A, B, C))` and `insert_bundle` stage the values in the World's own typed queues and apply them through the flush's `insert_run` as one move, the World untouched until then; `CommandBuffer::spawn_with`, `insert_bundle` and `insert_bundle_pending` record the same run for the flush. A `Bundle` has one method, `put`, implemented for tuples of up to sixteen components, for `Chain` (two bundles joined with `with`) and for game types such as `RigidBodyBundle`; a tuple element is a component, never a bundle.

**Event delivery (M14):** `EventQueue<T>` stores `previous` + `current`. Systems send into `current`. Readers normally use `iter()`, which yields `previous` first and then `current`, to avoid order-sensitive missed reads. A plugin or a game registers a queue through `World::register_event` (idempotent), and `World::flush_events` rotates every registered queue once per frame, which `App` calls after the `CommandBuffer` flush and before hot reload, extract, and render (`D-040`, amended by `D-128`). Inside the frame's fixed steps a queue has a step view (`D-137`): the first step reads it as any stage does, a later step reads only the events sent since the step before it ended, so a contact reader counts each step's collisions once, and outside the steps every reader sees the whole frame. An event sent outside the steps reaches a `fixed_update` reader only in a later step of its frame (`iter_current`) or the next frame's first step (`iter`), and otherwise rotates out unseen, so a fixed system reads what `fixed_update` sends.

**Runtime telemetry (M12+):** `FrameTimings` is a `World` resource. `App` measures stage timings inline with `std::time::Instant`, stores the latest frame totals, and keeps a per-system `Vec<(String, f32)>` in run order across the stages, each engine system under its public name in `tungsten::plugins` (`D-133`); each name appears once a frame, a `fixed_update` system's runs summed over the frame's steps and listed at 0 ms in a frame with no step, and `FrameTimings::fixed_steps` counts the steps (`D-137`). Render is split into acquire, encode, and submit/present. GPU-facing diagnostics live in `tungsten-render::GpuFrameTimings`, and the umbrella crate mirrors those diagnostics into the `World` after renderer init and after each frame. Display-facing diagnostics live in `tungsten::DisplayTelemetry`, which tracks the authoritative resolution, display mode, vsync intent, applied present-mode label, max-frame-latency hint, scale mode, and frame-rate cap.

**ECS error strategy (D-022):** panic on programmer errors such as insert on dead entity or wrong downcast; return `Option` / `Result` on runtime conditions such as entity not found or component absent.

**Render path (D-018):** systems mutate the `World` during `tick`. Extract functions receive `&World`, resolve asset IDs → `TextureHandle` via `AssetRegistry` (an interned `SpriteAssetId` indexes it, `D-113`), and produce plain render data such as `SpriteBatch`, `QuadInstance`, and `TextSection` for `render_frame_full`. ID resolution happens during extract; the renderer draws resolved batches without mutable `World` access. The extract stage calls the three channels of the `Extracts` resource (`crates/tungsten/src/extract.rs`, `D-138`), quads, sprites and text, then adds the engine's own pieces: the HUD, the systems overlay and inspector text, debug draw, lights and mesh particles. A channel draws its base, its default or a game's replacement (`replace_*`, or `App::set_extract_*`), then the contributions games and plugins added (`add_*`) in registration order; `App` inserts the resource before any plugin builds, so engine plugins contribute before a game's. The sprite default draws the tilemaps, then the `Sprite` entities; quads and text have none. A contribution keeps the `z_norm` its closure wrote, so under `gpu_depth` one at 0 draws over the default.

**Core/render seam:** `TextureHandle(u32)` lives in `tungsten-core`; no `wgpu` types appear in core. `tungsten` mediates the bridge: `Renderer::allocate_texture_handle` mints the handle (`D-048`); the loader uploads pixels under that key and passes it, atlas UVs and metadata to `AssetRegistry::register_sprite`. Many sprites can share one atlas handle. Core never calls into render. `tungsten-render` may depend on `tungsten-core` types (`D-007`).

**Render components (M15, `D-042`):** four engine-level component types live in `tungsten_core::components`:

- `Transform { position: Vec2, rotation: f32, scale: Vec2 }` — world-space pose. Rotation is in radians, CCW positive, applied around the quad centre by the sprite shader; scale multiplies the sprite's intrinsic pixel size per-axis.
- `Sprite { asset_id: SpriteAssetId, color: [u8; 4], z_order: i32, material_id: Option<MaterialAssetId> }` (`Copy`) — interned asset ID + tint + stable ascending sort key (`D-113`).
- `Visibility { visible: bool }` — explicit render gate.
- `Tag { name: String }` — debug-friendly entity label for find-by-name lookups.

Physics `Position` stays separate (`D-033`). A free-fn `sync_position_to_transform(&mut World)` copies `Position.0` into `Transform.position` one-way; callers register it after `physics_step` when they want the post-physics position to reach the extract stage.

`SpriteInstance` carries the rotation and tint across the core/render seam as a 48-byte GPU-facing POD: position, size, rotation, `Unorm8x4` color, atlas `uv_min`/`uv_size`, normalized depth and padding. The WGSL pipeline multiplies the sampled texel by the tint and rotates around the quad centre. Every sprite path — component-driven, tilemap, and custom extracts alike — uses the same layout.

**Default sprite extract (`D-086`):** the sprite channel's default runs `extract_sprites_default` after the tilemaps unless the game replaced it (`D-138`). It requires `Transform + Sprite + Visibility`, read through the slice-form query (`D-135`), skips hidden or unresolved sprites, applies parallax and scale, resolves each sprite's `SpriteAssetId` by index, and writes instances plus compact sort keys into reusable `ExtractScratch` buffers. It sorts by `(z_order, entity.id)` only when the query is not already in that order, then groups compatible atlas/filter/material/override/lit keys within each z-order run. Batches follow first-seen key order within the run; this can reorder overlapping sprites with different keys at the same z. It culls (`D-114`): a stock-pipeline sprite wholly outside the view render projects with (the surface size the app passes, else `WindowSize`) keeps its sort key and writes no instance, so batch order and `z_norm` do not depend on the camera; material sprites are always kept. Tilemap extraction culls separately. Custom extracts retain the same render-data seam.

### Data-Driven Config

Config model: one `tungsten.json` in the game's folder, its working directory (the workspace root for the examples, `D-123`), loaded once at startup, then environment overrides. Missing file → defaults with warning. Invalid file → fatal error naming the bad field. Runtime display changes use `request_display_settings`; config-file edits require a restart.

```json
{
  "game": { "id": "tungsten-examples" },
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
    "bloom_max_mips": 6,
    "system_fonts": false
  },
  "logging": { "level": "info" }
}
```

Display config semantics: checked-in display settings live under `display.*`. `display.present_mode` is authoritative when set to a concrete mode such as `"immediate"` or `"mailbox"`. When absent or `"auto"`, `display.vsync` selects between the auto-vsync and auto-no-vsync families. `display.max_frame_latency` is the requested frames-in-flight hint passed into `wgpu::SurfaceConfiguration`; backends may clamp it, so treat runtime telemetry as the configured hint unless the backend exposes stronger confirmation. Legacy `window.width`, `window.height`, `window.vsync`, `render.present_mode`, and `render.max_frame_latency` remain valid compatibility inputs, but `display.*` wins whenever both specify the same concern.

`render.system_fonts` (default `false`) lets text fall back to the machine's fonts after the manifest's (`D-116`).

`game.id` names the game's user folder and `game.version` goes into crash files; `logging.level` filters the engine logger while `RUST_LOG` is unset. `Config::load` and `App::new` both reject an invalid id or level ([Logs and Crash Reports](#logs-and-crash-reports--m35), `D-119`). Parsed `display.scale_mode` has no runtime application yet. Pacing precedence and overrides are documented in [the profiling workflow](docs/perf/profiling-workflow.md#frame-pacing).

### Asset System

**Manifest-driven, ID-referenced (D-009):** `assets/manifest.json` registers every asset by string ID. Game code uses IDs, never paths. Validation at load time catches missing files and unresolved references.

**Interned sprite IDs (D-113):** files name sprites (manifest, scenes, animation files, particle configs); at run time a sprite is a dense `SpriteAssetId`. `AssetRegistry::intern_sprite` gives one name one ID, registered or not, append-only for the registry's life; `load_sprites` interns the manifest's names in sorted order, so a manifest gets the same IDs every run. Animation frames hold IDs, filled by `AnimationData::load`; scenes and particle emitters intern names at spawn, so a sprite a later manifest reload registers starts drawing then.

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

A game keeps `assets/` in its own folder, its working directory, and a game made from `templates/basic` loads that manifest as its only root; no engine feature reads the workspace's root manifest ([Standalone Games](#standalone-games-and-the-template--m36), `D-123`). Examples ship `examples/NN_name/assets/` with a local manifest. `App::set_manifest_roots` declares the load order; `load_all_merged` loads the composed result before user startup (`D-052`). Explicit scene files are the path-based exception (`D-046`). Materials and particle meshes (`particle_meshes`, `D-093`) are inline manifest entries.

**Opaque handles (D-016):** `tungsten-core` stores opaque `TextureHandle(u32)` IDs. `tungsten-render` owns GPU textures, samplers, and pipelines in internal pools keyed by those handles. The registry is the one game-facing lookup path.

## Subsystems

### Text — M7

Stack: `glyphon` + `cosmic-text` + `swash`. Responsibilities: font parsing, shaping, layout, and GPU rasterization. Decision: `D-026`. Fonts are registered in the manifest by ID under `fonts`. `TextSection` is extracted each frame. Text ignores the world camera; it stays screen-space while the world scrolls.

**Engine and GPU halves (`D-117`).** `tungsten-render`'s `text.rs` holds `TextSection`; `text/engine.rs` holds the device-free `TextEngine` (font system, faces, families, the fallback chain, `FontEpoch`, the bounded section cache of `D-085`, retained nodes in `text/nodes.rs`); `text/gpu.rs` holds `TextPipeline`, which owns the engine and glyphon's atlas, renderer, viewport and swash cache. Core's `tungsten_core::text` holds the neutral types (`TextStyle`, `TextLayout`, `StyledText`, `TextMetrics`, `TextNodeId`, `FontEpoch`, …) and the `TextMeasure` trait, and never calls render. `TextSection.layout` (a `TextLayout`) sets alignment, wrap, overflow, hinting, letter spacing (in em) and OpenType features; its default lays out as sections always did. An ellipsis line limit counts lines per paragraph.

**Retained nodes and width modes (`D-117`).** A node holds styled text (spans that may override weight, italic and colour) and a style, shaped once into a buffer. Layout measures it through `TextMeasure`: max-content is the unwrapped width; min-content is the widest word (`Word`, `WordOrGlyph`), the widest glyph (`Glyph`) or the unwrapped width (`None`); a definite width applies the node's overflow. Probes relayout the retained buffer and never reshape, and min- and max-content are cached per text, style and epoch. `commit_layout` lays the buffer out in its final box. A node shaped under an older `FontEpoch` reshapes at its next measure or commit; a stale ID measures zero. `Renderer::text_nodes` lends layout a `TextNodes` facade; nothing public hands out `&mut TextEngine`, since the GPU half keys glyphs by the engine's font IDs.

**Families, the chain and packaged fonts (`D-115`, `D-116`).** The manifest's `font_families` group `fonts` face IDs under a family ID, and `font_fallback` orders families for glyphs a style's family lacks; chains concatenate across roots. A family resolves a weight to its exact face, else the nearest, ties to the heavier; italic exact, else upright. The font database holds only packaged faces, loaded in sorted ID order, with the chain as cosmic-text's fallback, so a fresh start draws the same on every machine; `render.system_fonts` adds the system's faces and platform lists after the chain, a packaged face replacing a system face of the same family, weight and style. A hot reload that adds a face may change fallback ties until the next start.

**The engine font (`D-123`).** `tungsten::ENGINE_FONT_ID`, `engine_mono`, is JetBrains Mono Regular compiled into the umbrella crate (`tungsten/engine_font.rs`; the file and its OFL text sit in `crates/tungsten/assets/fonts/`, a copy of the root font), registered once the renderer starts and before any manifest. The HUD, the systems overlay and the inspector default to it, so they draw in a game whose manifest has no monospace face. A manifest face under that ID replaces it: `load_font` on a registered ID drops that ID's earlier faces once the new data has registered a face. Two faces of one family, weight and style warn that text draws one of them for both, unless their bytes are identical, as the root `mono` and the engine font are.

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
| Font (`.ttf`/`.otf`) | yes — `reload_font` swaps face data in the `TextEngine`, a new `FontEpoch` | yes — added through `renderer.load_font` + `FontRegistry::register` in sorted ID order; `font_families` and `font_fallback` are set again, a new epoch only when they change (`D-115`, `D-116`) | not applied; existing entries kept |
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

Extension: `.tmj`. Schema: Tiled-compatible (`D-032`). Core data types `TilemapData`, `TilemapRegistry`, and `TilemapInstance` live in `tungsten-core` as plain data; no `wgpu`. `extract_tilemaps(&World)` resolves visible tiles into `SpriteBatch`es, per layer one batch for each atlas page and lighting in first-use order; a tile whose sprite has a lit atlas draws lit, as a sprite does (`D-061`, `D-138`). `extract_tilemap_layers(&World, &[&str])` draws only the named render layers, each map's in file order, for a game that puts its own drawing between layers, as example 01 does. Both write `z_norm` 0; the default sprite channel draws the tilemaps under its sprites with their tiles at 1.0, the far plane, so under `gpu_depth` every default sprite still draws over them. Tilemaps reuse the sprite pipeline; there is no new `wgpu` pipeline. `CameraState` (`position`, `zoom`, `rotation`) feeds view-projection into sprite and quad pipelines, and `visible_world_aabb()` keeps tile culling proportional to viewport size while over-covering safely under rotation. `LayerKind::Collision` layers are accepted by the loader but skipped by extract; physics reads them directly.

### Physics — M11

Components: `Position`, `PrevPosition` (the position before the last fixed step, `D-137`), `Velocity`, `Collider` (AABB or circle, with offset) and `RigidBody` (static or dynamic). Resources: `PhysicsConfig` and `EventQueue<CollisionEvent>`. Register `physics_step` as a system; default gravity is `Vec2::ZERO`.

Each frame gathers bodies and collision tiles once into dense proxy arrays and writes bodies back after the last substep (`D-066`). The spatial grid uses a direct cell table for compact bounds and a prefix-sum hashed table otherwise (`D-080`; default cell size 32 px). Broadphase pairs persist across substeps under travel budgets (`D-075`); exhausted proxies repair their own pairs, while contact wakes or many simultaneous budget trips rebuild the list (`D-081`). A statics-only sweep grid handles the safety-net path (`D-080`).

Signed-distance AABB/AABB, circle/circle and AABB/circle contacts support speculative CCD (`D-064`). After a substep's position integration, an arrival pass re-checks every body whose velocity the solve changed against the neighbours that velocity reaches, clamps it and redoes its move, so a pushed body stops at a wall instead of crossing it (`D-092`). The solver uses warm-started soft constraints, restitution and a configured fixed substep count (default 4; `D-063`, `D-076`), plus deterministic island sleeping (`D-065`, `D-082`). The step remains serial (`D-067`). Collision tile layers become static AABB proxies, gathered by a full-map scan each frame.

`physics_step` reads `Time::delta()`: under `PhysicsPlugin` it runs in `fixed_update`, so each call gets the fixed step (1/60 s by default) and a frame runs as many as the accumulator owes, a paused clock none (`D-137`; the M41 section below); a world that wires it elsewhere, as `02_bench` does in `update`, gets the frame's game dt (variable frame time capped at 0.1 s, `D-088`, times the scale, zero while paused). The dt is split across the configured substeps. One call advances at most `PhysicsConfig::max_step_dt` (1/30 s by default), so a step or a frame dt past it runs physics slow instead of softening its contacts (`D-094`); the `time` section refuses a step that long. Current workload limits and measured costs belong in [benchmarks.md](docs/perf/benchmarks.md), rather than an old stress-scene timing here.

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

A state change can run behind a screen transition (M31, `D-093`): `request_push_transition`, `request_pop_transition` and `request_replace_transition` take a `Transition` and return `false` when one is already queued or active. The dispatcher applies plain requests first, activates a queued transition, advances the active one by the real clock's dt (`Time::real_delta()`, so a fade finishes over a paused game; `D-129`) and runs its command on the frame the `Out` phase completes, with the same hooks and despawns as a plain request. States keep updating during both phases; `is_transitioning`, `transition_state` and `transition_cover` expose the progress.

### Tween System — M24

`tungsten_core::tween` adds a component-driven animation surface: `Tween` holds one timing model plus a `Vec<TweenChannel>` so position, rotation, scale, and sprite color can animate together on one entity. Easings are a closed `enum` (`Easing`) with a pure `apply(t)` implementation; repeat modes are `Once`, `Loop`, `PingPong`, and `Times(n)`.

`tungsten::tweens::tween_tick_system` advances every live tween by the game clock's dt (`Time::delta()`, so a pause freezes it), writes `Transform` / `Sprite` fields in place, emits `TweenComplete` through `EventQueue<TweenComplete>`, and defers terminal `Tween` removal through `CommandBuffer` so archetypes never mutate mid-iteration. The frame slot is `post_update`'s `tween_tick`, after `particle_tick` and before the command and event flushes (M38, `D-128`), which keeps tween writes visible to extract/render in the same frame while preserving the fixed frame-boundary mutation/event rules. Scene JSON can author tweens directly, making state-transition fades and simple data-driven motion possible without bespoke example systems. Decisions: `D-054`, `D-055`, `D-056`.

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

`PostPass::Bloom(BloomParams { threshold, knee, intensity, radius })` is the 18th `PostPass` variant. Bloom is a normal reorderable post slot — placement before vs after tone-mapping is the user's choice — but it is the first slot that records multiple sub-passes through the encoder rather than a single fullscreen draw into the slot's auto-opened render pass. The renderer detects the variant before `PassRecorder::begin` and calls `BloomPipeline::record_pass_timed`, which opens its own per-subpass passes:

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

`Light { kind, color, intensity }` and the closed `LightKind::{Point { radius }, Directional { angle }}` enum live in `tungsten-core::components`; `AmbientLight(Vec3)` is a world resource defaulting to `Vec3::ONE`. `LIGHT_CAP = 16` lives in `tungsten-core::lighting`; the render-side mirror is `tungsten-render::LIT_LIGHT_CAP`. The renderer owns one 544-byte `LightUbo` (16 lights × 32 bytes + `vec4<u32>` count_pad + `vec4<f32>` ambient) under `LightingResources` bound at group 2 of a sibling `LitSpritePipeline`. The lit pipeline reuses `SpritePipeline::vertex_layouts()` and binds a parallel albedo + normal + emissive bundle at group 1 — three texture views + one filtering sampler — so vertex/instance buffers stay interchangeable with the unlit and material paths.

Sprites with optional `normal_map` / `emissive_mask` sibling files in the manifest pack into parallel atlas canvases keyed by the same `PackedSprite` placement output: the asset loader runs `pack_shelf` once over the albedo inputs, then fills albedo, flat-normal-default, and emissive canvases page-by-page. Albedo uploads as `Rgba8UnormSrgb`; normal and emissive upload as `Rgba8Unorm` so tangent-space vectors and mask intensities are not gamma-decoded. The `SpriteAsset.lit_atlas: Option<TextureHandle>` marker tags only sprites with a valid normal sibling, and `extract_sprites_default` flips `SpriteBatch.lit` on that axis. Lit + material is intentionally out-of-scope in M29: a sprite carrying both warns and uses lit. `extract_lights` runs every frame, queries `(Transform, Light)`, packs each into `GpuLight`, sorts directional-first then nearest-to-AABB-squared-distance, truncates to `LIGHT_CAP`, and packs the result + ambient into the per-frame `LightUbo`. With no lit sprites and an empty light list the captured frame stays byte-identical to the M28 baseline — the unlit pipeline is unchanged and the lit pipeline never runs.

The lit shader (`assets/shaders/lit_sprite.wgsl`) and helpers (`assets/shaders/stock/emissive_mask.wgsl`, `assets/shaders/stock/rim_light.wgsl`) are manifest-tracked by name, with renderer-local shader IDs. The lit shader is compiled directly from `assets/shaders/lit_sprite.wgsl` with no `src/` mirror; the helpers have byte-equal mirrors in `crates/tungsten-render/src/shaders/stock/`. Lit body edits hot-reload through `Renderer::reload_shader` → `LitSpritePipeline::rebuild_with_shader` with `naga` validation and last-known-good pipeline retention on failure. The helpers are validated only — no pipeline behind them — so material authors and future milestones can fold `emissive_contribution` and `rim_term` into their own WGSL. Decision: `D-061`.

### Logs and Crash Reports — M35

`App::new` first checks `game.id` and `logging.level`, then resolves the game's user folder (`tungsten/user_dir.rs`): none in smoke mode, `TUNGSTEN_USER_DIR` when set (empty means none), none without an id, otherwise the XDG folders on Linux and other Unix, `%APPDATA%` and `%LOCALAPPDATA%` on Windows and `~/Library` on macOS. Only `logs` is created so far; settings and saves create theirs with W11b and W11c (`D-119`).

Then it installs the one engine logger (`tungsten/logging.rs`) unless the game set one first: `env_logger`, filtered by `RUST_LOG` when set and by `logging.level` otherwise, writing to stderr in debug builds and in release builds only while `RUST_LOG` is set, and, with a user folder, to an unbuffered `<logs>/<stem>-<UTC stamp>-<pid>.log`, ten kept per executable. A `Tee` writer fans each record out to the file and stderr; W1 M5's console adds its bounded channel sink there rather than a second logger. `Config::load` keeps its warnings for `App::new` to log once the logger is up (`Config::take_load_warnings`), and `App::new` and `App::run` log the errors they return, so a Windows release build, which opens no console (`windows_subsystem`), still records why it stopped.

With a user folder, `App::new` also installs a panic hook once per process (`tungsten/crash.rs`, behind a `static Once`). It writes `<logs>/<stem>-<stamp>-<pid>-crash.txt` (game and engine versions, target, executable, thread, message, location, an anchor address, the executable's GNU build-id or PDB identity, the backtrace with addresses and, on Linux, the file-backed mappings), names it in the run's log and calls the hook it replaced. Only panics leave a file. `TUNGSTEN_TEST_PANIC` panics in the first frame to check a shipped build. Release builds keep their line tables and ship them beside the player archive in a debug archive (`D-120`); `scripts/crash-report.py symbolize` resolves a crash file against it after checking the build identity, and every release run probes that path with `tools/crash-probe`. Captures log as before: a release build, `RUST_LOG`, stderr only, and no user folder.

### Standalone Games and the Template — M36

A game lives in its own folder, its working directory, which holds its `tungsten.json`, `input.json` and `assets/`; the workspace root is that folder for the examples until W12b gives each its own (`D-123`). `templates/basic` is the supported layout. A workspace member, its `main.rs` loads the config, creates the app, names its one manifest root, turns on hot reload in debug builds, calls `game::register`. `game::register` adds the game's one plugin, `GamePlugin` (`setup` in `startup`, `player_movement` in `update`, the HUD on in debug builds; M38, `D-128`), and its text extract; the headless harness runs the `Startup` stage on its first step, so its tests step one frame instead of calling `game::setup`. Its states, title, gameplay and pause, sit on the state stack (`D-046`), and its `AGENTS.md`, which `CLAUDE.md` imports, holds a game repository's rules. Layer 1 loads each template manifest on its own; `just repo-check` covers its assets and checks its toolchain pin and `rust-version` against the workspace's; `just smoke` runs it from its folder and fails on an unknown font ID. `just template-check`, a release check, copies it to a fresh folder with nothing a game reads above it, points its engine dependency at this checkout, appends the workspace's profiles and lock, and builds, tests and smoke-runs it there, offline, into this checkout's target folder after a free-space check.

The engine owns what its own features draw with: the HUD, the systems overlay and the inspector draw with the embedded engine font ([Text](#text--m7)). The action map reloads only when the watcher reports its own canonical path, so a level's `input.json` under `assets/` no longer replaces the bindings. The sprite and lit shaders still compile in from `assets/shaders/`, and both stock-shader copies stay, until crate publication is settled.

Release archives carry third-party license notices: each build job collects the shipped crates' license data and the standard library's notices, and packaging writes `THIRD-PARTY-NOTICES.txt` and `THIRD-PARTY-NOTICES-rust-std.html` beside the binaries ([license notices](docs/releases.md#license-notices), `D-122`).

### Core UI Model — M37

The UI is one `World` resource, `tungsten_core::ui::UiTree` (`crates/tungsten-core/src/ui/`, `D-125`). Roots sit on `UiLayer`s (`Game`, `GamePopup`, `Debug`, `DebugPopup`), ordered by layer then insertion; nodes are `Panel`s, `Label`s and `Button`s (a `Button` holds one `Label`) behind generational `WidgetId`s that a removed node never revives. Every node carries its `Role`, an accessible name (a label's text by default), its `Visibility` (`Hidden` keeps its box, `Collapsed` leaves layout) and the `interactive`, `focusable`, `focus_scope` and `hit_testable` flags, so accessibility and input routing build on data that exists from the start. Mutation is explicit: each setter marks only what it changes, text and visibility mark the root for layout, enabled and focus mark the node's style for resolution, and `set_text` compares first. `is_layout_dirty` and `is_paint_dirty` are what M2's layout stage and extract read.

Style is a theme of tokens (`Theme { tokens, kinds }`, plain data a game edits and hands to `set_theme`, which bumps a `ThemeEpoch`), per-kind defaults with `focused`, `disabled`, `hovered` and `pressed` variants, and per-node `StyleOverrides`. Resolution runs inside `layout()` for the nodes marked and those under a parent whose text changed: non-text properties are the kind's normal visual, then the state variant, then the node's own, and never inherit; text properties start from the theme's base and pass down the tree, each node's kind and own overrides applied in order, so a label inside a button draws in the button's text colour and weight. No selectors. A resolved text change in any property, the colour included, re-sets the label's text node and relayouts its root in the same pass, since the engine drops a commit on any style change; background, border and radius changes are paint-only.

Layout is Taffy 0.14 behind core's own `LayoutStyle` (`Sizing`, `Edges`, `Direction`, `Align`, `Justify`, `Anchor`, `Placement`; `D-124`): one solver tree per dirty root, built afresh inside a viewport-sized column with the default style, so a root's `Auto` width is the viewport's and a clean root costs nothing. On the main axis `Auto` is content-sized and never shrinks, `Fill` shares the remaining space and `Px`/`Percent` are fixed; on the cross axis `Auto` follows the parent's `align_items` (`Stretch` by default), `Fill` stretches. `Anchored` nodes are absolute, insets on their `Start`/`End` edges and a post-solve shift on a centred axis. Rounding is off: boxes are fractional and the extraction rounds once (w01 §6). Labels are leaves whose measure closure calls `TextNodeStore::measure`; after the solve every label's box is committed once, and `rect(id)` returns viewport-space `Rect`s. The pass reads the store's `font_epoch()` first and relayouts every root with text when it moved. `hit_test` walks layers high to low, roots and siblings latest first, to the deepest visible hit-testable node inside every ancestor's box; disabled nodes block. Focus is one widget, moved by `focus`, `focus_next` and `focus_prev` in depth-first order inside the nearest `focus_scope`, wrapping, skipping disabled, hidden and collapsed nodes and their descendants; focus clears when its widget or an ancestor is removed, hidden, collapsed or disabled, and removing a scope restores the focus it took over.

The seam: `tungsten_core::text::TextNodeStore: TextMeasure` adds the node lifecycle (`font_epoch`, `create_node`, `remove_node`, `set_text`, `commit_layout`, `committed`), which `tungsten_render::text::TextEngine` and the lent `TextNodes` implement by forwarding (`D-117`), so M2's layout stage hands `Renderer::text_nodes()` to `UiTree::layout` unchanged and core never calls render. Tests run headless: `tungsten_core::ui::FixedAdvanceMeasure` is a `TextNodeStore` with no fonts (a glyph advances a fixed fraction of the size, greedy wrapping per `TextWrap`, `lines × line_height` tall), and `tungsten::testing::UiHarness` wraps `Harness`, inserts a `UiTree` when the world has none and lays it out at the `WindowSize` viewport after each frame, so a system reads the previous frame's rects. Nothing draws yet: `App::new` inserts the tree and `run_frame` gains the layout stage with the overlay, the paint list, clips and the theme asset at M2; `UiEvent`, the `ui_*` actions, hover and pressed states at M3; the scale factor at M0b.

### Schedule, Stages and Plugins — M38

`tungsten_core::schedule` (`crates/tungsten-core/src/schedule.rs`, `D-128`) holds the `Schedule` that `App` drives: five closed stages, `Startup` (once, on the first frame, after the manifests have loaded; on the harness's first step, and `last_frame` is reset after it so a slow startup system does not inflate the next frame's dt), `PreUpdate`, `FixedUpdate`, `Update` and `PostUpdate`, each a list of named systems; the engine's exit check runs between `Update` and `PostUpdate`, and its own steps (the command flush, `World::flush_events`, hot reload, extract, render, audio, telemetry) run after `PostUpdate` and take no systems. A system is `system(name, f)` over `FnMut(&mut World)` with `before`/`after` constraints on names in its own stage, required (an unknown name fails `resolve`, naming the other stage when the system sits there) or `_if_present` (ignored when the name is absent). `resolve` sorts each stage once, Kahn's algorithm with the earliest-registered ready system first, so ties keep registration order and `resolved_text` (`stage: a, b, c` per line, `-` when empty) is a snapshot a test pins. A duplicate name, an unknown name or a cycle (reported as its path) is a `ScheduleError` before the first frame, from `App::run` or `Harness::new`; `run_frame` resolves lazily for a test that drives it directly. `fixed_update` runs once per fixed step, zero or more times a frame (M41). `FrameTimings::system_timings`, the `systems:` perf line and the systems overlay list every system in run order across the stages, each name once a frame.

A `Plugin` is a unit of registration: `build(&self, &mut Schedule, &mut World)` adds systems, events (`World::register_event`, idempotent; `World::flush_events` rotates every registered queue once a frame), resources and inspector rows (`InspectRegistry`, a `World` resource that the inspector overlay reads first). `PluginSet::with` adds or replaces a plugin by type, `without` removes one, and the set's order is the registration order. `App::new` is `App::with_plugins(config, DefaultPlugins::set())`; `add_plugin`, `add_plugins`, `add_system_to(stage, desc)`, `schedule()` and `resolve_schedule()` complete the surface, and `add_system` and `add_system_named` register into `Update`. The engine's resources are inserted by `App` whichever plugins run, so a game that leaves a plugin out still finds its resources. `InspectorState::new_with_defaults`, `register` and `registered_len` are deprecated shims to W4b (`D-133`).

`DefaultPlugins` (`crates/tungsten/src/plugins.rs`) is, in order, `DebugPlugin`, `DisplayPlugin`, `StatePlugin`, `PhysicsPlugin` (core's, `crates/tungsten-core/src/physics/plugin.rs`, the kit's shape), `ParticlesPlugin`, `TweensPlugin`, `GameFeelPlugin` and `CameraPlugin`, each registering under a public name constant (`D-133`):

```text
startup: -
pre_update: physics_debug_toggle, systems_overlay_toggle, inspector_toggle, inspector_pick, hud_toggle, display_input, state_dispatcher
fixed_update: physics_prev_snapshot, physics_step
update: -
post_update: physics_sync, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, camera_update
```

A plugin's own chains are required (`inspector_pick` after its toggle, the particle trio, `squash_stretch_tick` after its trigger); every coupling across plugins is if-present (`state_dispatcher` after `display_input`; the chain from `physics_sync` through the particle and tween ticks to the game-feel systems and the camera), so leaving any plugin out never fails startup. A game's `post_update` systems hang on that chain: `after(PHYSICS_SYNC).before(PARTICLE_COUNT_REFRESH)` runs between the sync and every other engine system, `before(CAMERA_UPDATE)` last but for the camera. The opt-in rule: a system that drives bodies or reads input for movement goes in `fixed_update` before `PHYSICS_STEP`, so a press moves the body on the frame of the press; example 01 is the worked example, its old order kept system for system with the stage boundaries drawn through it (`D-133`). The template is one `GamePlugin`. `02_bench` builds from an explicit set without physics, game feel and the camera, wires those systems by hand in each row's own `update` order and registers the events they send, so its digests are the old wiring's; the `particles` row owns the three particle systems it can now see (`D-133`). The kit (W13) and later plugins register the same way through core alone; what a plugin cannot do from core is contribute render data to the extract, which stays the app's until W15c.

### Game Clock and Timers — M39

`tungsten_core::time` (`crates/tungsten-core/src/time.rs`, `D-129`, `D-134`) holds the `Time` resource and its two clocks. The real clock takes the dt the loop gives a frame: elapsed wall time capped at 0.1 s (`D-088`), 1/60 s in a smoke run, the harness's dt as given (`D-110`). The game clock is the real dt times `scale` (1 by default; a negative or non-finite scale counts as zero), zero while paused, saturating at `f32::MAX`. `App::new` inserts `Time`, and `stage_time`, the frame's first step after the display apply, calls `advance_frame` once: both dts, both elapsed times (summed in `f64`) and `frame()`, which counts frames started. A wall frame with no previous frame time advances by zero, so the window loop's first drawn frame gets the time since `resumed` and a smoke run's frames see the dts they always did. `set_scale`, `pause`, `resume` and `set_paused` take effect from the next frame, so a system that pauses the clock leaves the current frame's dt as it was.

| Reader | Reads | So |
| --- | --- | --- |
| `physics_step` and a game's `fixed_update` systems | `Time::delta()`, the fixed step (M41) | They advance in whole steps; a pause runs none, a scale changes how many a frame runs |
| `tween_tick`, `particle_emit`, `particle_tick`, `shake_tick`, `squash_stretch_tick`, `camera_update` and a game's other systems | `Time::delta()`, the game dt | A pause freezes them and a scale slows or speeds them |
| The state dispatcher's transitions | `Time::real_delta()` | A fade finishes over a paused game (`D-093`, as `D-129` amends it) |
| The HUD, overlays, the inspector | `FrameTimings` | They read no dt |
| `DeltaTime.dt` | The app's writes | The step inside `fixed_update`, the game dt elsewhere, deprecated to W4b |

A pause does not freeze everything outright. `camera_update_system` draws the shake offset for the phase it entered with and then advances the phase by dt, so the first paused frame still shows a new offset and the camera holds from then on; a camera at smoothing 1 snaps to a target that moves under a pause (`D-100`); a `Burst` emitter spawned under a pause fires at once, and a `Pulse` emitter with a backlog fires one pulse a frame until it drains (W8a's emission semantics). A paused clock runs no fixed step, so bodies hold (M41); a world that wires `physics_step` outside `fixed_update` hands it dt 0 and it returns before touching a body. Above scale 2 at 60 Hz a frame owes more steps than the default bound of two, so the steps past it are dropped and the game runs slow against its clock.

`Timer` is a plain value, `TimerMode::Once` or `Repeating`, that a component or resource owns and ticks with whichever clock's dt its owner passes; there is no timer system or event queue. `tick(dt)` returns how many times the timer finished, so a long frame cannot swallow a repeat: a repeating timer splits the periods in `f64` with an exact remainder, its count saturating at `u32::MAX` and `elapsed()` staying within the duration. A negative or non-finite dt counts as zero, a negative, non-finite or zero repeating duration panics, and `set_duration` clamps `elapsed()` to the new duration. A world driven by hand, without the app, inserts `Time` and calls `advance_frame(dt)` in the clock stage's place before its direct system calls: engine systems read `Time` alone and no longer fall back to `DeltaTime`, which is `#[deprecated(since = "0.53.0")]` and still written each frame for games that read it. The accumulator is the M41 section's.

### Fixed Step and Interpolation — M41

`Time` (`D-129`, `D-137`) spends game time in whole fixed steps: `fixed_step()` 1/60 s, the smoke pin bit for bit, and at most `max_steps_per_frame()` 2 a frame. `advance_frame` adds the game dt to an `f32` sum that saturates at `f32::MAX` and subtracts one step at a time while a whole step is left, fewer than the bound have run and the subtraction still changes the sum; whole steps past the bound are dropped (`dropped_this_frame()`) and the fraction left is `alpha()`, in `[0, 1)`. A 0.1 s stall runs two steps and drops four (66.7 ms); a pinned 1/60 s frame runs one with `alpha` 0, so a smoke run or a capture runs as it did before the accumulator; 1/144 s runs zero or one, 1/30 s two. A paused clock runs none and holds `alpha`. The step and bound setters take effect at the next frame. `tungsten.json`'s `time` section (`fixed_step_hz`, `max_steps_per_frame`, `interpolate`; 60, 2, true) sets them at `App::new`, which refuses a rate below 30 Hz, since `physics_step` clamps a call to `max_step_dt`'s 1/30 s, and a bound of 0.

The app runs `fixed_update` once per step. Inside each, `Time::delta()` and `DeltaTime.dt` are the step, `InputState`'s edge queries answer from a fixed view that holds every press and release since the last step ended (`ActionMap` reads through them), and event queues have their step view (Events above). After each step the fixed views move on; after the last, `delta()` and `DeltaTime` read the game dt and every reader sees the frame again. So a press reaches exactly one step whatever the frame rate: a frame with no step keeps it for the next, a frame with two shows it to the first. Level queries, the cursor and the deltas stay per frame. A world without `Time` runs the stage once; a world driven by hand brackets its fixed systems with `Time::enter_fixed_step` and `leave_fixed_steps`, running them `fixed_steps_this_frame()` times.

Render interpolation: every `RigidBodyBundle` body carries `PrevPosition` from its spawn, static bodies too, so its archetype, and with it the solve order (`D-066`), is fixed at spawn; a game spawns a tuple for a body without history. `PhysicsPlugin` registers `physics_prev_snapshot` first in `fixed_update`, then `physics_step`, so a step's history spans the gameplay moves and the solve, and `physics_sync` in `post_update` writes `Transform.position = prev + (cur − prev) × alpha` for a body with history while `Time::interpolate()` is on and `Position` otherwise. The drawn body sits up to one step behind the simulation: at the pin it draws its previous step's position. A teleport writes `Position` and `PrevPosition` together, or the body is drawn sliding from the old point. The physics overlay draws `Position` (`D-042`), so it leads an interpolated sprite by up to a step. A game whose extract reads `Position` interpolates there, as example 01 does through one helper, and puts what it draws or emits at a moving body on that drawn point; `sync_position_to_transform` stays for worlds driven by hand. `02_bench` keeps its hand-wired `physics_step` in `update` with no history, so its rows and digests are unchanged. At 12,000 bodies the interpolating sync costs about 1.3 µs a frame over the plain copy (criterion `physics_sync_12k`).

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
