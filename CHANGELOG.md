# Changelog

Records all notable project changes. Versioned entries preserve historical filenames and commands; use [the source index](docs/LLM_INDEX.md) for current locations. Plan status statements describe their release-time scope; later retirement does not imply every proposed step shipped. Earlier `docs/plans/Phase3.md` references now correspond to the [archived Phase 3 plan](docs/plans/archive/phase3.md).

Format reference: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.58.0] - 2026-10-08

Summary: a small interactive feature with no plan: example 02's bench window moves to the next benchmark on Tab. No engine, decision, dependency, asset or manifest change; captures, smoke runs and the row digests are unchanged.

### Added

- **Tab cycles the benchmarks in example 02**: in a window run (`TUNGSTEN_SMOKE_FRAMES` unset or 0) Tab starts the binary on the next benchmark in `BENCHES` order (`physics`, `ecs`, `churn`, `gpu`, `particles`, `integrated`, then `physics` again) and closes the old window. The preset carries over when the next benchmark has one of that name, else it falls back to `default`; `TUNGSTEN_BENCH_SCALE` carries over and `TUNGSTEN_BENCH_SET` is dropped, since its knobs belong to the benchmark left. The title ends with `- Tab: next benchmark`. Under `TUNGSTEN_SMOKE_FRAMES` no `bench_cycle` system registers, so capture `systems:` lines keep their rows. `physics-sparse` and `gpu-throughput` are knob modes, not cycle entries. The binary still runs one benchmark per launch (`D-078`); `docs/perf/benchmarks.md` (Harness) describes the key, and `next_bench_visits_every_bench_and_wraps` pins the order.

## [0.57.1] - 2026-10-08

Summary: a maintenance release with no plan, for what example 01 still showed at uncapped frame rates after 0.57: the physics debug overlay outlines bodies where their sprites are drawn, and the example's scene animations move in every frame, not only in frames that step (`D-137`, `D-139`). Engine behaviour outside the `F1` overlay is unchanged. No new dependency, asset, manifest or decision.

### Fixed

- **Physics debug overlay follows interpolation** (`D-137`): `F1` outlines a body that carries a `PrevPosition` at `prev + (cur - prev) * alpha` while `Time::interpolate()` is on, the point `physics_sync` draws it at, and every other collider at its `Position`. Since 0.55 it drew the step's `Position`, up to a step ahead of the sprite and moving only at 60 Hz, so at uncapped frame rates the outline lurched against the smoothly following camera and showed as two copies of the player's box.
- **Example 01's scene animations move every frame** (`D-139`'s known issue): the cloud drift, glow and light flicker, the black hole's rings and the burning emitters read `extract::drawn_scene_time`, `SceneTime − (1 − alpha) × step` while interpolation is on, the time the bodies are drawn at. Since 0.55 they read `SceneTime`, which `move_obstacles` advances once a step, so at uncapped frame rates they moved only in frames that stepped (about two frames in five at 150 fps) and stuttered against the scrolling camera. Gameplay (`move_obstacles`, `motion_velocity`) stays on the step's clock. `at_144_hz_the_scene_clock_is_drawn_forward_in_every_frame` pins it.

## [0.57.0] - 2026-10-07

Summary: a fixed-step health pass outside the register (plan archived at `docs/plans/archive/fixed-step-health-pass.md`) for what example 01 showed at uncapped frame rates since 0.55: a frame with no fixed step holds its event queues for the next step, example 01's HUD counts each step's contacts and its slab platforms are one collider each, and the step defaults stay 60 Hz and two steps a frame (`D-139`). The determinism and pinned containment hashes, the row digests and the pixel fixtures are unchanged. No new dependency, asset or manifest change.

### Fixed

- **Fixed-step health pass** (`D-139`; plan archived at `docs/plans/archive/fixed-step-health-pass.md`): above the 60 Hz step most frames run no step, and their flush dropped a step's collision events before the next step's earlier readers ran (0 of 512 at 1/144 and 1/1000 s). A frame with no step now holds its event queues while the game clock advances (`World::hold_events_for_fixed_step`, new), so the next frame's first step reads them through `iter`, `len` and `is_empty`; at most 256 frames, and a pause drops them; the frame view and `iter_current` are unchanged. Example 01's HUD counts each step's own contacts after the contact readers (it read 0, and two steps' events or 0 at 1/30 s), and each slab platform is one collider (155 slabs, 27 colliders), so the player no longer stops at x = 1580.25 on a join; the solver's ghost collision between touching static boxes stays in known issues. Measured on a 3,000-ball pile, 120 and 240 Hz cost the fixed systems 2× and 4× per game second at the same 0.32 ms a step, so the windowed defaults stay 60 Hz and 2. A regime test runs example 01 at 1/30, 1/60, 1/144 and 1/1000 s.

## [0.56.0] - 2026-10-07

Summary: Phase 5's eleventh milestone, M42 (W15c, plan archived at `docs/plans/archive/1.0/phase5-milestone-42-additive-extracts.md`). Additive extracts: the `Extracts` resource holds the frame's sprite, quad and text channels, which games and plugins add to in registration order; the default sprite channel draws the tilemaps at the far plane, then the `Sprite` entities; `App::set_extract_*` stay as the explicit replace; `extract_tilemap_layers` and lit tiles let example 01 draw its tile layers through the engine, and the template's text goes through its plugin (`D-138`). The determinism and pinned containment hashes, the row digests and the pixel fixtures are unchanged. No new dependency, asset or manifest change.

### Added

- **`Extracts`** (`D-138`, M42). `tungsten::extract::Extracts`, also at the crate root: a `World` resource that `App` inserts before any plugin builds, with sprite, quad and text channels. `add_sprites`, `add_quads` and `add_text` append a contribution, drawn after the channel's base in registration order, engine plugins before a game's; `replace_sprites`, `replace_quads` and `replace_text` replace the base and keep the contributions, the last call winning. A contribution keeps the `z_norm` its closure wrote.
- **`extract_tilemap_layers`** (`D-138`): the named render layers of every tilemap, each map's in file order, for a game that draws between its layers.

### Changed

- **The default sprite extract draws tilemaps** (`D-138`, amending `D-042`; the break ledger's row): the sprite channel's default draws `extract_tilemaps` with its tiles at `z_norm` 1.0, the far plane, then `extract_sprites_default`, so under `gpu_depth` every sprite still draws over the tiles. Quads and text have no default.
- **Lit tiles** (`D-138`; the break ledger's row): `extract_tilemaps` keys a layer's batches on atlas page and lighting, so a tile whose sprite has a normal map draws lit, as a sprite does (`D-061`); an unlit tile's batches and bytes are unchanged.
- **`App::set_extract_*` forward to `replace_*`** with the same signatures: `App`'s three extract slots and `install_default_extracts` are gone, and `Harness::new` installs nothing.
- **Example 01's tile layers through the engine**: its extract keeps the explicit replace, since its parallax layers and the engine's particles are `Sprite` entities it draws itself, and calls `extract_tilemap_layers` at its three depths; its own tile extract is gone, with the same per-layer output on the real level.
- **The template and the guide**: `GamePlugin` adds its text through `Extracts` and `register` only adds the plugin; the template's `AGENTS.md` and the getting-started guide's new "Drawing" section give the rule. `DESIGN.md`'s render path, default extract and tilemaps paragraphs follow, and tile layers ordered among sprites by z is a 1.x backlog row.
- **Perf**: no benchmark row draws through the default channel, so only the stage's dispatch moved. Every owned metric of every CPU row reads not `regressed` on both direct pairs and the aligned pair, both A/A pairs read 0 regressed and 0 improved, digests 8 of 8; `gpu` `extract` p50 0.628 → 0.638 ms and `gpu-throughput` 12.076 → 12.104 ms read `unchanged`, and a scratch probe of `gpu-throughput` on the default channel with two contributions reads `extract` p50 12.014 → 12.014 ms. The `gpu` visual and `integrated` captures are byte-identical to 0.55's.

## [0.55.0] - 2026-10-07

Summary: Phase 5's tenth milestone, M41 (W3b, plan archived at `docs/plans/archive/1.0/phase5-milestone-41-fixed-step-interpolation.md`). A bounded fixed-step accumulator in `Time` that runs `fixed_update` zero or more times a frame, with `Time::delta()`, `DeltaTime.dt`, input edges and event queues on the step inside it; render interpolation on by default for bodies spawned through `RigidBodyBundle`, which now carry `PrevPosition`, written by `PhysicsPlugin`'s `physics_sync`; `tungsten.json`'s `time` section; example 01 on the fixed loop (`D-129`, `D-137`). At the pinned 1/60 s every frame runs one step with `alpha` 0, so the determinism and pinned containment hashes, the row digests and the pixel fixtures are unchanged. No new dependency, asset or manifest change. The roadmap's model tiers change beside it (`D-136`).

### Added

- **Fixed step** (`D-129`, `D-137`, M41). `Time::fixed_step()` (1/60 s, `1.0_f32 / 60.0`, the smoke pin bit for bit) and `max_steps_per_frame()` (2), with `set_fixed_step` and `set_max_steps_per_frame` (panicking on a non-finite or non-positive step and a bound of 0), `fixed_steps_this_frame()`, `dropped_this_frame()`, `alpha()`, `interpolate()` and `set_interpolate`, and `enter_fixed_step` and `leave_fixed_steps`. `advance_frame` adds the game dt to a saturating `f32` sum, runs whole steps up to the bound, drops the whole steps past it and keeps the fraction as `alpha`; a paused frame runs none. The app runs `fixed_update` once per step with `Time::delta()` and `DeltaTime.dt` the step inside it and the game dt outside; a world without `Time` runs it once.
- **Step views for input edges and events** (`D-137`, amending `D-040`). `InputState::set_fixed_view` and `end_fixed_step`: inside a step the edge queries, and `ActionMap` with them, answer from a fixed view, so a press reaches exactly one step and a frame with no step keeps it for the next; level queries, the cursor and the deltas stay per frame. `World::set_fixed_event_view` and `end_fixed_step_events`: a later step reads only the events sent since the step before it, and outside the steps a queue reads the whole frame; an event sent outside the steps reaches a `fixed_update` reader only in a later step of its frame or the next frame's first step.
- **Render interpolation** (`D-137`). `PrevPosition(pub Vec2)`, re-exported at `physics` and the crate root; `physics_prev_snapshot` (`PHYSICS_PREV_SNAPSHOT`) heads `fixed_update` and copies `Position` into it; `physics_sync` writes `Transform.position` at `prev + (cur − prev) × alpha` for a body with history while interpolation is on, `Position` otherwise, on the slice forms. A teleport writes both; the physics overlay keeps drawing `Position`.
- **The `time` section** (`D-137`). `TimeConfig { fixed_step_hz, max_steps_per_frame, interpolate }` at `Config.time` (60, 2, true; `#[non_exhaustive]`), validated by `Config::load` and `App::new`, which refuse `time.fixed_step_hz` below 30 and `time.max_steps_per_frame` of 0, and applied to `Time` by `App::new`. No checked-in `tungsten.json` carries it.
- **`FrameTimings::fixed_steps`** (`D-137`): the frame's step count. `system_timings` lists each name once a frame, a later step's runs added to the first step's entries, and a frame with no step lists each `fixed_update` system at 0 ms.
- **`physics_sync_12k`**: a criterion group in `physics_bench` over 12,000 bodies, `sync_position_to_transform`, `physics_sync` at `alpha` 0.5 and `physics_prev_snapshot`.

### Changed

- **`RigidBodyBundle` puts `PrevPosition`** (`D-137`; the break ledger's row): a static body has four components and a dynamic one five, `PrevPosition` at the spawn point; a game spawns a tuple for a body without history.
- **`PhysicsPlugin` steps and interpolates** (`D-137`): `physics_prev_snapshot`, then `physics_step`, in `fixed_update`, and `physics_sync` under `PHYSICS_SYNC` in `post_update` in place of `sync_position_to_transform`, which stays public for worlds driven by hand. The five schedule snapshots gain `physics_prev_snapshot` at the head of `fixed_update`.
- **`FrameTimings` gains a field** (the break ledger's row): a struct literal needs `..Default::default()`.
- **Example 01 on the fixed loop** (`D-137`): its extract draws each body with history at its interpolated point (`drawn_position`, `lerp_drawn`), its anchored emitters follow the parent's `Transform` in `anchor_emitters` after the sync, `pending_effect` is set only on an accepted jump, a grounded jump takes the buffered press, the ball budget counts the balls the frame's earlier steps queued, and its fixed systems scale by the step. Its bodies and attached visuals draw one step behind at the pin, and its schedule snapshot gains `anchor_emitters`.
- **The guide and the template teach the step**: the getting-started guide the stage rule, the step dt, the edge and event views, interpolation and a custom extract's lerp, the teleport rule and the `time` section, and the template's `AGENTS.md` the stage rule, the step dt, the views and the teleport rule. `DESIGN.md` has an M41 section, and the deep-pile lever, `contact_hertz` 60 on the constant step, is a 1.x backlog row.
- **Roadmap tiers** (`D-136`, amending `D-118`): `scripts/roadmap.py` recommends Fable 5.1 only for level C releases, gates and spikes; level A and B releases take Opus 5.5 at max.
- **Perf**: no capture runs a step's new work beyond the clock arithmetic and the view calls, since `02_bench` keeps its hand-wired step in `update`. Every owned metric of every CPU row reads not `regressed` on both direct pairs, digests 8 of 8, every `systems:` name and `workload_version` unchanged; `particles` `particle_count_refresh` p50 and p95 read `regressed` in the tree's own A/A pair, one low first suite, and were accepted as A/A drift (M41 Q14). `physics_sync` reads 4.66 µs against the copy's 3.35 µs at 12,000 bodies in criterion, an accepted cost (M41 Q13).

## [0.54.0] - 2026-10-06

Summary: Phase 5's ninth milestone, M40 (W15b, plan archived at `docs/plans/archive/1.0/phase5-milestone-40-tuple-queries-bundles.md`). Tuple queries with `With`/`Without` filters in `tungsten-core`, which take over `World::query` and `query_mut` from the thirteen arity-named functions, now deprecated; bundles, `spawn_with`, `insert_bundle` and `RigidBodyBundle`, which spawn an entity into its archetype in one move; a panicking resource accessor; every engine, example and template call site on the new forms (`D-130`, `D-135`). No new dependency, asset or manifest change; the determinism and pinned containment hashes, the row digests and the pixel fixtures are unchanged; the `ecs` row's `bounds_wrap` carries `D-130`'s budgeted cost on the block-aligned pair.

### Added

- **Tuple queries** (`D-130`, `D-135`, M40). `tungsten_core::ecs::query`: a query names its data as a type, `&A`, `&mut A`, `Option<&A>`, `Option<&mut A>` or `Entity`, alone or in a tuple of up to eight, with a filter `With<T>`, `Without<T>`, `()` or a tuple of up to four as a second type parameter, checked once per archetype. `World::query`, `query_filtered`, `query_mut` and `query_mut_filtered`, and `query_slices`, `query_slices_filtered`, `query_mut_slices` and `query_mut_slices_filtered`, which yield `(rows, slices)` per archetype for a loop written by hand, with `OptionalColumn` for an optional column. Rows come in archetype order then row order; every mutable column is borrowed through one `slice::get_disjoint_mut` call per archetype, no `unsafe`; a mutable query naming one component twice panics with the query's type name. `With`, `Without` and `OptionalColumn` are re-exported at both crate roots.
- **Bundles** (`D-130`, `D-135`). `tungsten_core::ecs::bundle::Bundle`, with one method, `put`, implemented for tuples of one to sixteen components, for `Chain` (two bundles joined with `Bundle::with`) and for game types. `World::spawn_with((A, B, C))` and `insert_bundle` apply the components as one archetype move through the command-buffer flush's `insert_run`, with a run of inserts' result (a type named twice keeps the later value, a type the entity has is overwritten) and the World untouched until the move, so a panic in a game's `put` leaves it consistent; `CommandBuffer::spawn_with`, `insert_bundle` and `insert_bundle_pending` record the same run. A tuple element is a component, never a bundle. `tungsten_core::physics::RigidBodyBundle` (`dynamic`, `r#static`, `with_velocity`, `with_body`) carries `Position`, `Velocity` when there is one, `Collider` and `RigidBody`.
- **`World::resource::<T>()` and `resource_mut`** (`D-135`): the resource, or a panic naming the type; `get_resource` and `get_resource_mut` stay as the `Option` forms.
- **`tuple_query_bench`**: a criterion bench of thirteen shape pairs, the tuple forms against the arity functions, built under `CARGO_TARGET_DIR=target/criterion-native`.

### Changed

- **`World::query::<T>()` and `query_mut::<T>()` take query data** (`D-130`, `D-135`; the break ledger's two rows): a component type there no longer compiles, and callers write `query::<&T>()` or `query::<(Entity, &T)>()`. Every engine, example and template call site reads the tuple forms, body-preserving; the `ecs` and `churn` benchmark systems take the access each needs, with every digest unchanged.
- **The eleven other arity-named query functions are deprecated** (`D-135`): `query_entities`, `query2`, `query2_entities`, `query3`, `query3_entities`, `query2_opt2`, `query3_opt2`, `query2_opt2_mut`, `query2_mut`, `query3_mut` and `query3_mut_without` are `#[deprecated(since = "0.54.0")]` shims whose notes give their tuple form; W4b removes them (the break ledger's row).
- **Example 01 and the template spawn through bundles**: the player in one `spawn_with` of `RigidBodyBundle::dynamic(..).with_body(..).with((..))` in place of thirteen inserts, the platform colliders through `RigidBodyBundle::r#static`, the seeded balls through `dynamic(..).with_velocity(..).with_body(..).with(..)`; every body keeps its components, so the solve order is unchanged (a 165-body order probe read the same hash before and after). The template's `spawn_player` is one `spawn_with`, its `player_movement` one `query_mut_filtered::<&mut Transform, With<Player>>()` over `resource::<Time>()`, and its `AGENTS.md` and the getting-started guide show a tuple query, a bundle spawn and the accessor.
- **The default sprite extract reads the slice form** (`D-135`): `query_slices` over its six-item shape, each archetype's rows zipped through `OptionalColumn` with `for_each`, since the tuple row iterator in that shape spills its state to the stack (+163% a row in criterion, +12% on `gpu` `stage.extract` p50 in the first capture); on the slice form both `extract` rows read `unchanged`.
- **Perf**: `ecs` `bounds_wrap` p50 reads `unchanged` on the direct pairs and +0.046 ms `regressed` on the block-aligned pair, LLVM's layout of the lone mutable column's loop, `D-130`'s budgeted cost recorded in `D-135`; `churn` `churn_scan` p50 +0.028 ms on the direct pairs is placement (`unchanged` aligned, the function's address moving 0 → 16 modulo 64). `ecs` `update` and every other owned row read not `regressed`, digests 8 of 8.

## [0.53.0] - 2026-10-06

Summary: Phase 5's eighth milestone, M39 (W3a, plan archived at `docs/plans/archive/1.0/phase5-milestone-39-game-clock-timers.md`). A `Time` resource in `tungsten-core` with a real clock and a game clock, time scale, pause, elapsed time on both clocks and a frame index, and a plain `Timer`; every engine, example and template reader of the frame's dt reads `Time`, transitions run on the real clock, and `DeltaTime` is deprecated for W4b to remove (`D-129`, `D-134`). No new dependency, asset or manifest change; the determinism and pinned containment hashes, the row digests and the pixel fixtures are unchanged.

### Added

- **Game clock and timers** (`D-129`, `D-134`, M39). `tungsten_core::time::Time`, re-exported at the crate root with `Timer` and `TimerMode`: `advance_frame(real_dt)`, `delta()` (the game dt a system reads), `game_delta()`, `real_delta()`, `elapsed()` and `real_elapsed()` (summed in `f64`), `frame()` (frames started), `scale` and `set_scale`, and `pause`, `resume`, `set_paused` and `is_paused`, the setters taking effect from the next frame. A negative or non-finite dt or scale counts as zero, and the game dt saturates at `f32::MAX`. `Timer::new(duration, mode)`, `once` and `repeating`: `tick(dt)` returns how many times the timer finished, saturating at `u32::MAX` with `elapsed()` kept within the duration, beside `finished`, `just_finished`, `times_finished_this_tick`, `elapsed`, `remaining`, `fraction`, `duration`, `mode`, `reset` and `set_duration`, which clamps `elapsed()` to the new duration. A negative, non-finite or zero repeating duration panics.

### Changed

- **The frame advances `Time`** (`D-134`): `App::new` inserts it, and `stage_time`, the frame's first step in place of `stage_delta_time`, advances both clocks by the pinned dt as given (`D-110`) or the wall frame's capped dt (`D-088`), zero for a wall frame with no previous frame time, so a smoke run's frames see the dts they saw before; it then writes the game dt into `DeltaTime`.
- **Engine systems read `Time` alone** (`D-134`): `physics_step`, `tween_tick`, `particle_emit`, `particle_tick`, `shake_tick`, `squash_stretch_tick` and `camera_update` read `Time::delta()`, so a pause freezes them and a scale slows or speeds them, and the state dispatcher advances transitions by `Time::real_delta()`, so a fade finishes over a paused game (`D-093` as `D-129` amends it). Nothing falls back to `DeltaTime`: a world built by hand inserts `Time` and calls `advance_frame` before its direct system calls, as the engine's, the examples' and the template's tests and benches now do. Under a pause the camera's shake shows one new offset on the first paused frame, a camera at smoothing 1 still snaps to a moving target, and pending burst and pulse emissions still fire (W8a's emission semantics).
- **`DeltaTime` is deprecated** (`D-134`): `#[deprecated(since = "0.53.0")]`, and the app writes the game dt into it each frame until W4b removes it (the break ledger's row).
- **Examples, template and docs on `Time`**: examples 01–04 and the template's `player_movement` read `Time::delta()`; the template gains `clock_half_scale_moves_the_player_half_as_far` and an `AGENTS.md` rule on which clock a system reads; the getting-started guide gains "Time and timers", `DESIGN.md` the clock stage and "Game Clock and Timers — M39", and `docs/LLM_INDEX.md` a route to `time.rs`. The API snapshot gains `Time`, `Timer` and `TimerMode` and loses nothing (`D-107`). In the milestone's sitting (`perf-runs/20261006-m39-clock/`, machine-local) every compare matched the digests 8 of 8 and the second direct pair read 0 `regressed`; the first read `ecs` `buffs` and `stats_decay` p50 `regressed` against a tree suite that caught the `ecs` per-run mode in five runs of five, which the plan's Q7 accepts.

## [0.52.0] - 2026-10-06

Summary: Phase 5's seventh milestone, M38 (W15a, plan archived at `docs/plans/archive/1.0/phase5-milestone-38-schedule-stages-plugins.md`). A `Schedule` of five closed stages with named systems, stage-local `before`/`after` constraints and plugins in `tungsten-core`, which `App` drives and the headless harness steps; the engine registers as `DefaultPlugins`, and the template and examples 01, 03 and 04 name no engine system (`D-128`, `D-133`). No new dependency, asset or manifest change; the determinism hash, the row digests and `gpu-visual.png` are unchanged.

### Added

- **Schedule, stages and plugins** (`D-128`, `D-133`, M38). `tungsten_core::schedule` holds `Schedule` over `Stage::{Startup, PreUpdate, FixedUpdate, Update, PostUpdate}`, `system(name, f)` with `before`/`after` constraints on names in the system's own stage (required, or `_if_present`), `ScheduleError` (duplicate, unknown, cycle with its path) and `Plugin` with `PluginSet::{with, without}`; `resolve` sorts each stage once and keeps registration order for ties, and `resolved_text` is the snapshot the tests pin. `tungsten::plugins` holds `DefaultPlugins` (`DebugPlugin`, `DisplayPlugin`, `StatePlugin`, `PhysicsPlugin`, `ParticlesPlugin`, `TweensPlugin`, `GameFeelPlugin`, `CameraPlugin`) and fifteen public system-name constants. `App::new` is `App::with_plugins(config, DefaultPlugins::set())`, and `add_plugin`, `add_plugins`, `add_system_to`, `schedule` and `resolve_schedule` join it; `add_system` registers into `Update`; the engine's resources stay `App`'s, so leaving a plugin out never fails startup. `PhysicsPlugin` lives in core (`physics/plugin.rs`) with `PHYSICS_STEP` and `PHYSICS_SYNC`; `World` gains `register_event`, `has_event`, `registered_event_count` and `flush_events`, and `InspectRegistry` (a `World` resource, read first by the inspector overlay) with `InspectFn`. `Harness::new` builds the same schedule and times every engine system under its public name. `WindowSize` moves to `tungsten_core::display` and stays re-exported at `tungsten_core::WindowSize`, `tungsten::WindowSize` and `tungsten::app::WindowSize`, so no caller changes.

### Changed

- **The frame runs the five stages** (`D-128`): `startup` once on the first frame (the clock is reset after it, so a slow startup system does not inflate the next frame's dt), `pre_update`, `fixed_update` (once a frame until W3b), `update`, the exit check, `post_update`, then the engine's own steps. The seven engine systems, particles and tweens are no longer a flat list ahead of the game's systems plus two untimed steps; each is a named, timed system in its plugin's stage.
- **Template and examples on stages** (`D-133`): the template is one `GamePlugin` (`startup: setup`, `update: player_movement`); examples 03 and 04 register through `fn configure(app: &mut App)`; example 01's `RUNTIME_SYSTEM_ORDER` is a table of stage slots that `install_runtime` registers, with its input, spawners, forces and the three riders in `fixed_update` before `PHYSICS_STEP`, the five contact systems after it, six in `update` and four in one `post_update` group ahead of the engine's chain. Each has a schedule snapshot test, and example 01 gains `a_press_moves_the_player_on_the_next_step`. `02_bench` builds from an explicit set without physics, game feel and the camera and wires those systems by hand, so its digests are the old wiring's.
- **The `particles` row owns its three named systems** (`D-133`): `system.particle_count_refresh`, `system.particle_emit`, `system.particle_tick` (the bottleneck) and `system.animate_sprites`, each at p50 and p95, in place of `stage.unattributed`, which now holds the event flush alone; compare reports the change as a schema change, so the row's baseline restarts at 0.52, and `workload_version` stays 1. In the milestone's sitting (`perf-runs/20261006-m38-schedule/`, machine-local) every tracked CPU row read 0 `regressed` on both direct pairs with digests 8 of 8, and `stage.unattributed` p50 fell 2.19 → 0.00 ms into the three new systems (`particle_tick` 1.46, `particle_count_refresh` 0.60, `particle_emit` 0.10 ms p50) with `total` unchanged. The benchmarks guide's T1 proposal is closed, since the stage is timed under its names.
- **`InspectorState`'s row API is deprecated** (`D-133`): `new_with_defaults`, `register` and `registered_len` are `#[deprecated(since = "0.52.0")]` shims that keep their behaviour until W4b removes them (the break ledger's row); `App::new` fills the registry from `tungsten::inspector::default_inspect_registry`.
- **Docs and rustdoc**: `DESIGN.md` gains the frame-loop block and "Schedule, Stages and Plugins — M38"; the getting-started guide and the template's `AGENTS.md` gain the stage rules, including the `fixed_update` opt-in rule; `docs/LLM_INDEX.md`, the profiling workflow, the benchmarks guide, known issues and w15 follow. `display.rs`'s older public items and `Inspectable::inspect_rows` gain the rustdoc `missing_docs` wants at the freeze. The API snapshots gain `schedule`, `InspectRegistry`, `PhysicsPlugin`, `plugins` and the new `App` methods; nothing leaves (`D-107`).

## [0.51.0] - 2026-10-06

Summary: the glyph gate and the frame-loop gate (`D-126`–`D-132`, recorded in `docs/plans/1.0/implementation-plan.md` §11), run in one owner session on 2026-10-06 and signed, every item on its default, outside the register and with no plan file; W15, W2 and W3 graduated into their workstream files the same day. Plans and decisions only: no engine code, dependency, asset or manifest change.

### Changed

- **Glyph gate and frame-loop gate** (`docs/plans/1.0/`, `D-126`–`D-132`): both 1.0 gates ran on 2026-10-06 in one owner session and are signed, every item on its default. The glyph path is T1b, several glyphon renderers on one atlas with a deferred trim, with the owned path (T2) and W2 R3 in 1.x and the damage numbers popping per label (`D-126`); the DPI model is logical layout drawn at physical size with hinting off, and `display.scale_mode` leaves before the freeze (`D-127`). The frame-loop decision set: a `Schedule` of five closed stages with named systems and plugins in core (`D-128`, amends `D-018`, `D-040`); a `Time` resource with a fixed step bounded at two steps a frame, interpolation on by default and the clock table (`D-129`, amends `D-088`, `D-093`, `D-094`); tuple queries on `query`/`query_mut` with bundles (`D-130`); R1 dropped after its spike, no worker mechanism and no `Send`/`Sync` bounds in 1.0 (`D-131`, amends `D-102`); the W6 and W13 tiers confirmed from the acceptance game, bodies on roots, the tilemap builder in W16b and world labels as W1 WL (`D-132`). Phase 6's rows and cards, the backlog, the break ledger and the roadmap catalog follow. Plans and decisions only.
- **W15 graduated** (`docs/plans/1.0/w15-authoring-api.md`): criteria §8.6 moved into the workstream file on 2026-10-06 with the frame-loop gate's decisions and implementation plan amendments 7, 8, 16 and 18 folded in, and its three candidates (W15a stages and plugins, W15b tuple queries and bundles, W15c additive extracts) written as steps with a done-when each; criteria keeps a placement paragraph, and the roadmap's W15 stops read the file.
- **W2 graduated** (`docs/plans/1.0/w02-multi-core-rendering.md`): criteria §5 moved into the workstream file on 2026-10-06, its subsections as headings there, with R0's decisions, the glyph and frame-loop gates' multi-core answers and implementation plan amendment 7 folded in; its two 1.0 candidates written as steps with a done-when each, R0 (landed in 0.45) and R2, the late-acquire split, which W1 M0b's plan carries; criteria keeps a placement paragraph, the plan files that cited its subsections link the new headings, and the roadmap's W2 stops read the file.
- **W3 graduated** (`docs/plans/1.0/w03-frame-loop.md`): criteria §6 moved into the workstream file on 2026-10-06 with the frame-loop gate's decisions (`D-128`, `D-129`, `D-131`) and implementation plan amendments 7, 16 and 18 folded in, and its two candidates (W3a `Time` and `Timer`, W3b the fixed step and interpolation) written as steps with a done-when each; criteria keeps a placement paragraph, the implementation plan's Track C row and W3b card drop the seam change, the break ledger cites the file, and the roadmap's W3 stops read the file.

## [0.50.0] - 2026-10-05

Summary: Phase 5's sixth milestone, M37 (W1 M1, plan archived at `docs/plans/archive/1.0/phase5-milestone-37-core-ui-model.md`). The core UI model: one `UiTree` resource in `tungsten-core` with generational widget IDs, roots on layers, a closed set of widget kinds, roles and labels in node data, a theme-token style model with text-only inheritance, a layout pass solved by Taffy behind Tungsten-owned style types, hit testing and keyboard focus, all tested headless through a fixed-advance text double and the `UiHarness` on the headless harness (`D-124`, `D-125`). Nothing draws yet. One new dependency: `taffy` 0.14 (`D-015` rule 3).

### Added

- **The core UI model** (`D-124`, `D-125`, M37). `tungsten_core::ui::UiTree` is a `World` resource: `add_root` on a `UiLayer` (`Game`, `GamePopup`, `Debug`, `DebugPopup`), `panel`, `label` and `button` (a `Button` holding one `Label`) behind generational `WidgetId`s, with `Role`, an accessible name, `Visibility` (`Hidden` keeps its box, `Collapsed` leaves layout) and the `interactive`, `focusable`, `focus_scope` and `hit_testable` flags in node data; each setter marks only what it changes layout- or paint-dirty, and `set_text` compares first. `LayoutStyle` (`Sizing`, `Edges`, `Direction`, `Align`, `Justify`, `Anchor`, `Placement`) is the only layout surface; `taffy` 0.14 solves it, one tree per dirty root inside a viewport-sized parent, rounding off, after the M37 spike passed every case and a second solve measured nothing (`perf-runs/20261005-taffy-layout-spike/`). `Theme` (tokens, per-kind defaults with `focused`, `disabled`, `hovered` and `pressed` variants) and per-node `StyleOverrides` resolve inside `layout()`; text properties inherit down the tree, nothing else does, and a resolved text change relayouts its root. `layout(viewport, &mut dyn TextNodeStore)` drives labels through the new `tungsten_core::text::TextNodeStore` seam (`font_epoch`, `create_node`, `remove_node`, `set_text`, `commit_layout`, `committed`), which `TextEngine` and `TextNodes` implement by forwarding; `rect`, `dump_layout`, `hit_test` (layers high to low, latest sibling first, disabled widgets block), `focus`, `focus_next` and `focus_prev` (tree order inside the nearest focus scope, wrapping; focus clears when its widget or an ancestor leaves the traversal, and removing a scope restores the focus it took over). `tungsten_core::ui::FixedAdvanceMeasure` is a `TextNodeStore` with no fonts for headless tests, and `tungsten::testing::UiHarness` wraps `Harness`, inserts a `UiTree` when the world has none and lays it out at the `WindowSize` viewport after each frame. 62 `ui::` tests and 6 harness tests; `DESIGN.md` gains "Core UI Model — M37".

### Changed

- **`DECISIONS.md`:** `D-110` and `D-117` are amended by `D-125` (the `UiHarness` beside `Harness`; the `TextNodeStore` seam beside `TextMeasure`). The API snapshots gain the `ui` module, the seam, render's two trait impls and the harness; nothing is removed (`D-107`). `licenses/clarifications.json` gains Taffy's upstream MIT text (`licenses/taffy/LICENSE`), since the package ships no licence file (`D-122`).

## [0.49.0] - 2026-10-05

Summary: Phase 5's fifth milestone, M36 (W9a with W12a, plan archived at `docs/plans/archive/1.0/phase5-milestone-36-licence-notices-template.md`). Every release archive carries the third-party license notices of what its binaries link: each statically linked crate's license texts, the Rust standard library's notices and the works the engine compiles in (`D-122`). A game lives in its own folder: the engine owns its font, the action map reloads from its own path, and `templates/basic` is the supported game layout, built, tested and smoke-run in the workspace and from a copy outside it, with the getting-started guide's first draft (`D-123`). No new dependency.

### Added

- **Third-party license notices, the engine font and the game template** (`D-122`, `D-123`, M36). Each build job runs `release.py licenses <target>`, which records the crates `cargo tree -e normal` reaches from the examples and the launcher, with their license, notice and font files, and the toolchain's standard-library notices; `package` and `release.py notices` write `THIRD-PARTY-NOTICES.txt` (the embedded works, an index of crates with the license each is used under, each text once) and `THIRD-PARTY-NOTICES-rust-std.html` into every archive and fail on a crate with no license text. `licenses/clarifications.json` supplies four crates' upstream texts, `sctk-adwaita`'s bundled Cantarell font (OFL 1.1, Linux) and reviewed font files, and `just notices` joins the release checks. `tungsten::ENGINE_FONT_ID` (`engine_mono`), JetBrains Mono Regular compiled into `tungsten` with its OFL text, registers before the manifests and is the default font of the HUD, the systems overlay and the inspector; a face loaded under a registered ID now replaces that ID's faces, and byte-identical faces no longer warn. The action map reloads only from its own canonical path. `templates/basic` (title, gameplay and pause states, a player, harness tests, `AGENTS.md`) joins the workspace, layer 1, `just repo-check` and `just smoke`; `just template-check` builds, tests and smoke-runs a copy outside the repository, and `docs/getting-started.md` is the guide's first draft.

## [0.48.0] - 2026-10-05

Summary: Phase 5's fourth milestone, M35 (W11a with W7a, plan archived at `docs/plans/archive/1.0/phase5-milestone-35-logs-crash-reports.md`). A game names itself in `tungsten.json`'s `game.id` and gets a per-user folder; `App::new` installs one engine logger, which applies `logging.level` and writes a log file per run, and a panic hook that writes a crash file (`D-119`). Release builds keep their line tables in a separate debug archive per platform, and `scripts/crash-report.py` symbolizes a crash file against it (`D-120`). Windows release builds open no console, and CI gains a CPU-only Windows test job (`D-121`). No new crate: `env_logger` moves from the examples into `tungsten`. The `physics` row digest, `gpu-visual.png`, the post and transition regressions and a capture's logging are unchanged.

### Added

- **User folder, engine logger, crash reports, release symbols, a Windows test job** (`D-119`–`D-121`, M35). `Config.game` (`GameConfig`: `id`, `version`, `validate`, `#[non_exhaustive]`) names the user folder: XDG folders on Linux, `%APPDATA%` and `%LOCALAPPDATA%` on Windows, `~/Library` on macOS, `TUNGSTEN_USER_DIR` to override, none in smoke mode or without an id; an invalid `game.id` or `logging.level` is fatal, from the file or set in code. `App::new` installs `env_logger` unless the game set a logger (`RUST_LOG`, else `logging.level`; stderr in debug builds and while `RUST_LOG` is set; `<logs>/<stem>-<UTC stamp>-<pid>.log`, ten kept), logs `Config::take_load_warnings()` and the errors it and `App::run` return, and installs a once-per-process panic hook that writes `<stem>-<stamp>-<pid>-crash.txt` (versions, target, the GNU build-id or PDB identity, an anchor address, the backtrace with addresses and, on Linux, the mappings) and then calls the previous hook; `TUNGSTEN_TEST_PANIC` panics in the first frame. The examples drop `env_logger::init()` and, like the launcher, open no console in Windows release builds; the root `tungsten.json` names `tungsten-examples`. `release.yml` keeps line tables (`-Wl,--build-id` on Linux), `release.py package` splits Linux binaries with `objcopy` (equal build-ids and `.debug_line` checked; `.debug_gdb_scripts` dropped) and matches each Windows PDB to its CodeView record, into `tungsten-debug-<tag>-<target>` archives that extract over the player archives (`sha256sum -c --ignore-missing` for player-only downloads); `scripts/crash-report.py` checks a crash file's build identity, then symbolizes it, and the release run probes `tools/crash-probe` on both runners; CI's `windows-tests` job runs `cargo test --workspace --locked` on `windows-2025`; its first run failed only in `tungsten-render`'s shader-coverage test, whose messages printed Windows paths with `\`, and that checker now shows `/`-separated paths everywhere. Both reports are informational (`D-070`). `Config` can no longer be built by literal outside core (break ledger).

## [0.47.0] - 2026-10-04

Summary: the 1.0 roadmap page's stages and session recommendations (`D-118`), outside the 1.0 register and with no plan file: the roadmap catalog gives each stop its stages from `tungsten-next`'s Flow table and each session stage a mode, model and effort, and the page draws them. No engine code, dependency, asset or manifest change.

### Changed

- **Roadmap page: every stop's stages, a session recommendation per stage, Dracula on black** (`D-118`): `scripts/roadmap.py catalog` reads each stop's stages from `prompts.md`'s Flow table by kind and level and gives each session stage a mode, model and effort from the stop's complexity and effort (Fable 5.1 xhigh for architectural stops, Opus 5.5 max at complexity 2–3, plan mode for gates), as catalog schema 2 (a 55.4 KB payload, from 30.3 KB), and `just repo-check` fails when a stop matches no Flow row; the page draws each stop's stages with the current one marked, on OLED black with Dracula accents that each mean one thing; the Next panel adds the headline, the step's recommendation, plan progress, the stop's open questions, "After that" and the tree at sync time (`meta/now` drops `flow`, gains `next.rec`, `recovery[].rec` and `tree`); `/tungsten-next`'s new-session label shows the recommendation; expanded stop bodies no longer print "[object HTMLSpanElement]" for sources and questions. Process and tooling only.

## [0.46.0] - 2026-10-04

Summary: Phase 5's third milestone, M34 (W1 M0a, plan archived at `docs/plans/archive/1.0/phase5-milestone-34-text-engine-split.md`). Text splits into a device-free engine and a GPU half, with neutral text types in core and retained nodes that layout measures (`D-117`); font families and a fallback chain join the manifest (`D-115`); text draws with packaged fonts only unless a game sets `render.system_fonts` (`D-116`). One new direct dependency, `unicode-script` 0.5.8, which cosmic-text already locked. `gpu-visual.png` moves on purpose, since the old fixture showed a system font; the row digests, the post and transition regressions and the physics hashes are unchanged.

### Changed

- **Text engine split, font families, packaged-only fonts** (`D-115`–`D-117`, M34). `tungsten_render::text` splits into `TextEngine` (device-free: fonts, families, the fallback chain, `FontEpoch`, the section cache, retained nodes) and `TextPipeline` (the GPU half, its methods unchanged, plus `with_font_source`); `tungsten_core::text` adds `TextStyle`, `TextLayout`, `StyledText`, `TextMetrics`, `TextNodeId`, `FontEpoch` and the `TextMeasure` trait. Nodes measure min-content, max-content and definite widths on one shaped buffer and commit a final box, and `Renderer::text_nodes` lends them to layout. `TextSection` gains `layout` (alignment, wrap, ellipsis, hinting, letter spacing in em, OpenType features) and `Default`, so literals need `..Default::default()`. The manifest gains `font_families` and `font_fallback` (`ManifestError` gains two variants); the shared manifest groups `sans` with `sans_bold`, and `mono`. The font database holds only packaged faces, loaded in sorted ID order with the chain as fallback, and `render.system_fonts` (`RenderConfig::system_fonts`) adds the system's after them, so startup no longer scans system fonts. `gpu-visual.png` is regenerated: the old fixture showed JetBrains Mono 2.304 installed on the reference machine instead of the packaged 2.211 (2155 HUD pixels). `gpu` and `integrated` read 0 regressed against both A/A captures.

## [0.45.0] - 2026-10-04

Summary: Phase 5's second milestone, M33 (W2 R0, plan archived at `docs/plans/archive/1.0/phase5-milestone-33-interned-asset-ids.md`). Sprites name their asset by an interned `SpriteAssetId` (`D-113`, a public API break and W4's first ledger row), and the default extract culls stock-pipeline sprites outside the view (`D-114`). No dependency, asset or manifest change; the row digests, the pinned extract output, the pixel tests and the physics hashes are unchanged.

### Changed

- **Interned sprite IDs and extract culling** (`D-113`, `D-114`, M33). `Sprite.asset_id` is a `SpriteAssetId`, `Copy`, that `AssetRegistry::intern_sprite` mints: one ID per name, registered or not, append-only, sorted at manifest load. Animation frames hold IDs (`AnimationData::load` takes the registry, and `AnimationData` and `AnimationFrame` drop `Deserialize`). `current_sprite` and `advance` return IDs, `spawn_particle_via` takes one, and `sprite_ids()` and `sprite_id_for_path` become `sprite_names()` and `sprite_name_for_path`. Files keep names. The default extract indexes the registry by ID, and it now skips a stock-pipeline sprite wholly outside the view render projects with (the surface size the app passes, else `WindowSize`). A culled sprite keeps its sort key, so batch order and `z_norm` do not change, and material sprites are never culled. `particles` `total` p50 6.50 → 4.44 ms, `gpu-throughput` `extract` p50 13.25 → 12.16 ms, `integrated` `total` p50 8.59 → 7.93 ms. Accepted: `particles` `unattributed` p50 +0.12 ms (15 runs a side) and `gpu` `extract` p50 +0.06 ms from culling.

## [0.44.0] - 2026-10-04

Summary: a test-suite overhead pass (plan archived at `docs/plans/archive/test-suite-overhead.md`): example 01's route test replays known launches, redundant unit tests go, dev and test builds keep debuginfo out of the test binaries (`D-111`) and CI builds the benchmarks in the dev profile (`D-112`), taking `cargo test --workspace` from 10.9 s to 1.9 s. No runtime code, dependency, asset or manifest change.

### Changed

- **Test-suite overhead** (`D-111`, `D-112`; plan archived at `docs/plans/archive/test-suite-overhead.md`): example 01's route test replays a table of the 45 known launches and searches only for a stale or missing row (9.7 s → 0.5 s); 14 redundant unit tests go, five of them as compile-time checks (one already existed), and two lighting tests now also check the lit shader's light array and the manifest's shader keys (941 → 927 tests); dev and test builds keep debuginfo in unpacked `.dwo` files instead of linking it into every test binary, taking a core-edit test build from 18.1 s to 4.8 s and a cold target directory from 10.6 GB to 5.2 GB (medians); CI builds the benchmarks in the dev profile instead of the thin-LTO bench profile; `just level-check` runs the platformer generator's `--check` and unittests locally. Together, by median: `cargo test --workspace` 10.9 s → 1.9 s and a core-edit `just check` 19.8 s → 8.2 s. No runtime behavior, asset or hash changes; gating the GPU pixel tests behind a feature saved under 1 s per core edit and was not adopted.

## [0.43.0] - 2026-10-04

Summary: Phase 5's first milestone, M32 (W14a, plan archived at `docs/plans/archive/1.0/phase5-milestone-32-headless-harness.md`): a headless test harness that runs the window loop's own frame body (`D-110`), with example 01's frame tests moved onto it. No dependency, asset or manifest change; the row digests, pixel tests and physics hashes are unchanged.

### Added

- **Headless test harness** (`D-110`, M32): `App::run_frame` is the frame body that the window loop runs and that `tungsten::testing::Harness` steps with no window, renderer or audio device. The harness sits behind the umbrella's off-by-default `testing` feature and steps at a pinned dt, written as given (amends `D-088`). Tests inject actions and the cursor between steps, then read the world, the frame's events, its extract (`FrameDraw`) and the audio commands it would have played. Example 01's frame tests run on it, its 2,366-line test file is split into topic modules, and the API snapshot lists the feature (amends `D-107`). Row digests, the pixel tests and the physics release tests are unchanged, and the suite reads no regression against the `w14a-pre` baseline.

### Changed

- **Critique rounds record their run** (`tungsten-milestone` skeleton §9): a round's header line also takes the model, effort, wall time and tokens from the user-level `critique` skill's footer line.

## [0.42.0] - 2026-10-04

Summary: 1.0 Step 0, the planning tooling every milestone plan needs (`D-106`, `D-107`; `docs/plans/1.0/workflow.md` §8), the owner's sign-off of the definition gate with W4, W9, W11, W12 and W14 graduated and the gate's amendments folded into the 1.0 plans, sessions that leave work uncommitted until the release commit (`D-105`), a road with fewer owner prompts: each milestone releases from its own run session, and small neighbours may share a release (`D-108`), and a roadmap page fed from a checked catalog of the road to 1.0 (`D-109`; plan `docs/plans/roadmap-artifact-rework.md`, which closes after the post-merge sync). No engine code, dependency, asset or manifest change.

### Added

- **`tungsten-next` skill** (`.claude/skills/tungsten-next/`, linked from `.agents/skills/`): reads where the road to 1.0 stands from the repo (git and release state, other agent sessions in the tree, uncommitted edit groups, what the release commit would take, the 1.0 README's "Now" lines, the open register rows and Step 0's items), flags records that lag the tree, gives the owner the next session prompt and whether it runs in a new or the same session, and syncs the roadmap artifact. It writes nothing in the repo; `docs/agent-setup.md` lists it with `tungsten-milestone`, and `.gitignore` tracks both.

### Changed

- **Sessions don't commit before the release commit** (`D-105`, superseding `D-097`'s and `D-098`'s commit clauses and amending `D-104`): plan, execution, gate, graduation and Step 0 work stays uncommitted on the milestone branch, and the release block's `git add -A` and one-line commit make the release's only commit; a session ends with the list of files it changed, and pushes, tags and merges stay with the owner. `docs/plans/README.md`, `docs/plans/1.0/workflow.md` and the implementation plan follow. Process and docs only.
- **1.0 definition gate signed, workstreams graduated** (`docs/plans/1.0/`): the owner signed the gate record on 2026-10-03. W4, W9, W11, W12 and W14 graduated into their workstream files with amendments 4, 5, 6, 9, 11, 14, 16, 17 and 19 folded in; the remaining amendments (1–3, 7, 8, 10, 13, 15–18) and the gate's answers went into `criteria.md`, w01 and the skeletons of W2, W3, W5–W8, W10, W13 and W15. Criteria §9 now links the implementation plan as the only order, and the release checklist and the 1.x backlog cite the graduated files. Plans only.
- **1.0 Step 0, planning tooling** (`D-106`, `D-107`; `docs/plans/1.0/workflow.md` §8): `just repo-check` also reads `docs/plans/<program>/*.md`, skipping the archive and each README, and names `docs/plans/archive/<program>/` for a finished plan; the `docs/LLM_INDEX.md` budget is 12 KiB and `AGENTS.md`'s asset table moved to `docs/assets.md` (`AGENTS.md` 6,127 → 5,332 B); a `tungsten-milestone` skill holds the milestone plan skeleton and the execution rules plans cite; `just api` writes the library crates' public API to `api/<crate>.txt` with `cargo-public-api` 0.52.0 from the pinned toolchain's rustdoc JSON, and `just api-check` joins the release checks.
- **Milestone plans are critiqued last** (`tungsten-milestone` step 7, `docs/plans/1.0/workflow.md` §2; plan archived at `docs/plans/archive/critique-in-planning.md`): once a plan is complete and before its approval line, the user-level `critique` skill reviews it from another model family, in the background, with `codex:rescue` as the read-only fallback and never a self-review in its place; each finding is checked against the repo or the docs it cites before anything changes and is recorded in the plan; one round, a second only when the first changed a done-when check or the step order. Process only.
- **Fewer owner prompts per 1.0 candidate** (`D-108`; `docs/plans/1.0/workflow.md` §1–§3, §9): a milestone plan's last step is its release, run in the session that ran the plan, so the cut happens once and the checks run once, after it; a separate release session stays for resumes and plan-less releases. `just release-preflight` also prints a post-merge block that starts the next milestone branch (`git switch -c 0.<NN+1> --no-track origin/main`, `git push -u`), with a case in `scripts/test-release-preflight.py`. Each session ends with the next one's prompt, so `tungsten-next` runs once per release; the `tungsten-milestone` skill and its skeleton gain the release step. Two small adjacent level A or B candidates that touch different files may share one plan and one release, revising the definition gate's one release per candidate: W9a with W12a and W1 M0b with W2 R2 join W11a with W7a. Process, docs and release tooling only.
- **Roadmap page fed from a checked catalog** (`D-109`): `docs/plans/1.0/roadmap.json` holds the road to 1.0's 47 stops and 24 owner questions, and `just repo-check` fails when its rows differ from the register's, a level from its card, a source path is missing or a question id is unknown, with cases in `scripts/test-check-repo.py`; `scripts/roadmap.py`, tested by `scripts/test-roadmap.py` under `just script-test`, writes the catalog payload and derives each stop's status from the tree. `tungsten-next`'s `prompts.md` becomes the one prompt home, with keys and a Flow table, and `docs/plans/1.0/workflow.md` §3 links it. The roadmap artifact's source moved into the repo as `.claude/skills/tungsten-next/page.html`, a 29.4 KB viewer (from 198.8 KB) that reads `meta/catalog`, `meta/now` and `stops/*` from its database; `/tungsten-next` writes them with no hand edits, and the `questions` collection is gone. Process, docs and tooling only.

## [0.41.0] - 2026-10-03

Summary: the 1.0 definition gate (`D-102`–`D-104`, recorded in `docs/plans/1.0/implementation-plan.md` §11, awaiting the owner's sign-off), a review of the 1.0 drafts in `docs/plans/1.0/` against the 0.40 tree, and an SMAA fix. No dependency, asset or manifest change.

### Changed

- **1.0 definition gate** (`D-102`, `D-103`, `D-104`): 1.0 means definitions A and B of `docs/plans/1.0/criteria.md` §1, a complete engine for making games and a stable library, tested by one acceptance game, a top-down survivors-like auto-shooter in its own GitHub repository on this repository's git tags, using the public API and the kit only (`D-102`). Sliders and checkboxes (W1 BC), gamepad play and save slots are in; text fields, IME and the pipelined render thread (R4) move to `docs/plans/1.0/backlog-1.x.md`. From 1.0, semver covers the `tungsten` crate's API with the kit re-export, the game file formats and the CLI; direct core/render/kit dependencies and `wgpu`/`winit` types are outside, MSRV rises only in a 1.x minor and deprecated items stay until 2.0 (`D-103`, amends `D-069`). `cargo-public-api` will snapshot each library crate's public surface per milestone (`D-104`). Criteria questions Q1, Q4, Q8, Q9, Q11, Q16 and Q24 are answered; the implementation plan, workflow and release checklist move from draft to in progress; `acceptance-game.md` gains the genre, a proposed pitch, screens and mechanics. Decisions and plans only: no code change.
- **1.0 drafts reviewed against the 0.40 tree** (`docs/plans/1.0/`): Q12 moves to the feature gate, W12a no longer waits for Q2, W1 M2 also waits for W2 R2, the follow-up table gains capture provenance and `cargo shear`, the definition gate agenda gains the release count and the acceptance game's repository, `validate_wgsl_source` leaves criteria §7, the release checklist gains RC-A10 (display and present modes), and the register records the 0.40 release. Plans only.

### Fixed

- **SMAA washed out the frame** (`crates/tungsten-render/src/renderer.rs`): the neighborhood-blending pass, which writes the sRGB swapchain, sampled the non-sRGB view of its source and so encoded the frame to sRGB twice. It now samples the source's sRGB view, decoded to linear on read; edge detection keeps the non-sRGB view.

## [0.40.0] - 2026-10-03

Summary: the 0.40 QA and cleanup pass (plan archived at `docs/plans/archive/qa-cleanup-0.40.md`) and the 1.0 planning drafts in `docs/plans/1.0/`. The pass fixes eight bugs (B1–B8, `D-099`–`D-101`), removes dead API, unused dependencies, a duplicate shader and probe tests, splits `physics/step.rs` and `tungsten::asset_loader` into modules, builds `tungsten-core` at opt-level 1 in dev (`D-096`) and guards perf captures against background load (`D-095`). Agents now commit plan work locally (`D-097`, `D-098`). Both physics hashes, every benchmark digest and `gpu-visual.png` are unchanged.

### Added

- **Perf runner background-load guard and digest comparison** (`D-095`, 0.40 QA step 1): `scripts/bench.py` scans `/proc` before, once a second during and once after each measured run for `nxcodec.bin` and for `cargo`/`rustc` outside its own process tree, rechecks the commit and dirty-tree hash after each run, records both in the capture's provenance and marks the run invalid (`background load: <name>`, `background load: tree changed`) unless `--allow-background` makes it a note. Compare prints whether the first-run digests match, suite compare lists the rows whose digests differ, and `run --compare` takes a suite's row of the same name. Runner and docs only: no engine change, hash or digest moved.
- **Pinned determinism hash** (0.40 QA step 2): `physics_step_is_bit_identical_across_runs` (`crates/tungsten-core/tests/physics_determinism.rs`) now also asserts the run hash equals `0x088ec07a73c1b168`, the value `just physics-release` printed on the 0.40 tree at `afbc330`, so a physics change that moves it fails instead of only printing. Test only: the hash and digests are unchanged.
- **Repo check for missing plan citations** (0.40 QA step 11): `scripts/check-repo.py` (`just repo-check`) reads every tracked `*.rs`, `*.py`, `*.sh`, `*.md`, `*.toml` and `*.yml` file outside `docs/plans/` and fails on a `docs/plans/<name>.md` citation whose file is missing. Archive citations are notes and are never opened; `CHANGELOG.md`, `DECISIONS.md` and the script tests' fixtures are exempt. It caught `DESIGN.md`'s status line citing `docs/plans/1.0-criteria-draft.md`, which moved to `docs/plans/1.0/criteria.md`. Tooling only.

### Changed

- **Faster debug physics for the gate** (`D-096`, 0.40 QA step 3): dev and test builds compile `tungsten-core` at opt-level 1 (`[profile.dev.package.tungsten-core]` in the workspace `Cargo.toml`), which takes the platformer's `authored_routes_and_recovery_shelves_traverse_with_real_physics` from 81.0 s to 8.9 s and a warm `just check` from 89.7 s to 11.1 s with the same 928 tests passing. Release, bench and perf-runner builds are unchanged: no hash, digest or reference image moved.
- **Shader test coverage** (0.40 QA step 5): `crates/tungsten-render/tests/shader_coverage.rs` also Naga-validates each example's `assets/shaders/` (70 paths, 39 distinct contents and 31 mirror pairs, up from 68 and 37), which puts `examples/02_bench/assets/shaders/bench_heavy.wgsl` under a test for the first time. The ten tests it already covered are gone: the bloom and SMAA per-shader Naga and mirror tests in `crates/tungsten-render/src/tests/{bloom,smaa}.rs` and the platformer's `soft_glow_shader_passes_naga_validation`. Tests only: no hash, digest or image moved.
- **Stale comments and plan citations** (0.40 QA step 10): comments that cited `docs/plans/physics-scale-and-ccd.md`, the archived benchmark-suite plan, "the M26 plan", "see plan" or a "Step-9 invariant" now cite the decisions that hold the rationale (`D-058`, `D-059`, `D-061`, `D-063`–`D-067`, `D-078`). `PostPass` is documented as 18 stock effects, the speculative sweep's threshold comment no longer names the removed substep picker, the material doc names `tungsten::asset_loader::load_materials`, `Entities::free` says its stale-handle check is debug-only, and `CameraState::view_projection` calls its full view extents `view_w`/`view_h`. Comments and one local rename: no hash, digest or image moved.
- **Script, CI and justfile follow-ups** (0.40 QA step 12): `scripts/check-repo.py` drops the stale deletion-candidate entry for the platformer's `player.png`, which its manifest registers, and the test that covered it; `just --list` shows `quick`'s comment whole on one line; `just ctx` runs `python3 -B` like every other recipe; both workflows stop installing `libudev-dev`, which no locked crate links (the next CI run checks the workflows, `D-070`). The three `docs/known-issues.md` entries are gone. Tooling only.
- **Render module layout** (0.40 QA step 14): `STOCK_SHADERS` in `crates/tungsten-render/src/post/mod.rs` lists each stock effect's manifest ID and `include_str!` path directly, replacing seventeen four-line `post/<effect>.rs` modules, and the inline tests of `targets.rs`, `timing.rs` and `post/smaa_luts.rs` moved to `src/tests/` behind `#[path]` like the crate's other unit tests. Same 917 tests; no hash, digest or image moved.
- **1.0 planning** (`docs/plans/1.0/`): the 1.0 criteria draft moved, revised, from `docs/plans/1.0-criteria-draft.md` to `docs/plans/1.0/criteria.md` and the UI draft from `docs/plans/ui-text-suite-draft.md` to `w01-ui-text-suite.md`, beside an implementation plan (phases 5–7, gates, Phase 5 candidate cards, the definition gate agenda), a Claude Code workflow, a release checklist, an acceptance game, a 1.x backlog and skeletons for workstreams W2–W16. Every file is a draft; start at `docs/plans/1.0/README.md`. No code, decision or dependency change.
- **Agents commit plan work locally** (`D-097`, `D-098`): on a milestone branch the agent stages only the paths a plan touched and commits once per plan or phase, while push, tags, merges and history rewrites stay with the human and release pull requests stay squash-merged. Plan evidence rows shrink to one line and a plan adds one changelog line. The per-step patch hand-off is gone: the `tungsten-patch-handoff` skill, `scripts/patch-series.py` and its tests. Process, docs and tooling only.

### Removed

- **Physics probe tests** (0.40 QA step 4): `crates/tungsten-core/tests/physics_timing.rs` (two debug-build timing probes that asserted nothing) and `substep_probe.rs` (two ignored dense-pile diagnostics of a finished plan) are gone; the `physics` and `integrated` benchmark rows, `benches/physics_bench.rs`, `physics_containment.rs` and the sleep tests in `src/tests/physics/step.rs` cover what they printed. `physics_determinism.rs` and `physics_containment.rs` now share `spawn_pile` and a `spawn_static_box` that takes the wall thickness from `tests/common/mod.rs`. Tests only: both physics hashes and the digests are unchanged.
- **Unused dependencies** (0.40 QA step 6): `image` from `tungsten-core`, `wgpu` from `tungsten`, `tungsten-core` and `tungsten-render` from `example-02-bench` (its pixel test imports `tungsten::render::compare_png`) and `tungsten-core` from `example-04-shader-playground` (which reaches it as `tungsten::core`); `Cargo.lock` loses only those edges. No behavior change: both physics hashes and the reference image are unchanged. Symbol hashes change with the dependency set, so the step 29 suite judges code placement.
- **Dead `sprite.wgsl` mirror** (0.40 QA step 7): `crates/tungsten-render/src/sprite.wgsl` is gone. The renderer and the sprite pipeline compile in `assets/shaders/sprite.wgsl`, as `D-057` states, so the copy was only a byte-equal mirror that agents had to edit twice. `shader_coverage.rs` checks the stock mirror alone (69 paths, 30 mirror pairs) and the render `AGENTS.md` names the single file. No compiled shader changed: hash, digests and images are unchanged.
- **Dead render API and fields** (0.40 QA step 8): `PostStackRenderer::record_bloom_slot` and `final_target`, `BloomPipeline::record_pass`, `MaterialPipeline::write_uniforms` and its never-read `material_bind_group_layout`, `MaterialBuildError`, `Renderer::capture_armed`, `ScreenshotError::DeviceLost`, the `requested_present_mode_label` wrapper, `create_capture_target`'s unused format, the sprite pools' never-read texture views, the bloom pyramid's never-read texture handle (its mip views keep the texture alive) and `TextPipeline::new`'s sample-count and depth parameters, which its one caller fixed at 1 and none. These were public `tungsten-render` items with no caller in the workspace. Hash, digests and images are unchanged, and the `gpu`, `gpu-throughput` and `integrated` rows read 0 regressed against `qa-0.40-pre`.
- **Unused `CycleMode::None`** (0.40 QA step 13): the platformer's orbit-light cycle mode loses the variant no light used, its `#[allow(dead_code)]` and its match arm in `systems.rs`. No behavior change.

### Fixed

- **Release guide** (`docs/releases.md`): a new milestone branch is created with `git switch -c 0.NN --no-track origin/main` and published with `git push -u origin 0.NN`. Created the old way, 0.40 tracked `main`, and an editor sync pushed its work to `main`, which was then restored to the 0.39 release commit.
- **0.40 QA close-out** (0.40 QA steps 9 and 15–29): the dead core API is gone (`AnyColumn::len` and `type_id`, `Archetype::id` and `row_count`, `Shape::min_half_extent` and `LightKind::Point`'s never-read `falloff`; step 9). `tungsten::asset_loader` is a directory module (`atlas`, `reload`, `scene`) whose load, rebuild and hot-reload paths share one sibling decode and one page blit, with the same public paths (step 15). `tungsten-core`'s animation, tilemap and audio loaders return `AnimationError`, `TilemapError` and `AudioDecodeError` with the same messages, and the crate no longer depends on `anyhow` (step 16). `physics/step.rs` is split by stage into `physics/step/{sleep,pairs,solver,arrival,sweep,gather}.rs`, a pure move (step 17). `just ci` runs CI's six recipes locally and `just udeps` runs `cargo shear`; `.claude/settings.json` allows `just physics-release` (step 18). `just script-test` and CI lint the workflows with `actionlint` 1.7.12 (step 19). `docs/LLM_INDEX.md` routes the smoke runner, the pixel tests, the context checker and the Criterion benches (step 20). Fixed: a `Tween` queued through the `CommandBuffer` in the frame the old one completes is no longer deleted by the old one's removal; `CommandBuffer::call` queues a plain function at flush (B4, `D-099`, step 21); an unsupported `render.msaa` or `render.bloom_max_mips` in `tungsten.json` is reported as `ConfigError::InvalidValue` naming the file, not as an env override (B7, step 22); a scene tween whose `uniform_vec4_lane` lane is above 3 fails validation instead of driving lane 3 (B6, step 23); camera smoothing is a rate per 1/60 s, so the camera converges the same at any frame rate (B5, `D-100`, step 24); a sprite hot-reloaded to a smaller image keeps its UV and size together and draws at its new size instead of squeezed (B3, step 25); a shader or material edit that passes Naga but fails wgpu pipeline validation logs and keeps the live pipeline instead of panicking (B2, step 26); every batch of a material draws with a uniform buffer of its own, so two sprites with one material and different overrides no longer both draw the last one, and the `integrated` row now shows each walker's own flash (B1, `D-101`, step 27); a broadphase AABB whose max edge lies exactly on a cell boundary no longer claims the next cell far from the origin (B8, step 28). Both physics hashes, every row digest and `gpu-visual.png` are unchanged, and the final suite reads no owned metric `regressed` against the 0.40 baseline (step 29).

## [0.39.0] - 2026-10-02

Summary: agent tooling and documentation only, with no plan executed. No engine, example, asset or dependency change, no decision and no determinism or benchmark digest change. Two skills and a patch-series tool cover work while Git is human-only, the profiling workflow records the capture rules the recent perf passes learned, and a new discussion draft, `docs/plans/1.0-criteria-draft.md`, scopes what 1.0 means; it and `docs/plans/ui-text-suite-draft.md` stay drafts.

### Added

- **`tungsten-decision` skill** (`.claude/skills/tungsten-decision/SKILL.md`): adds, amends or supersedes a `DECISIONS.md` entry, with the next `D-NNN` ID, the entry shape, the `Superseded by` marker and the `docs/DECISION_INDEX.md` row the same change needs.
- **`tungsten-patch-handoff` skill and `scripts/patch-series.py`:** when Git mutations are human-only, `init`, `cut`, `verify` and `script` export HEAD, cut one patch and commit message per step, replay the series on a fresh export and compare it byte for byte, and write a `commit.sh` the human runs (`git apply --cached` and `git commit -F` per step). The tool is read-only toward the repository; `scripts/test-patch-series.py` runs under `just script-test`.
- **`just physics-release`:** runs `physics_determinism`, `physics_tunneling` and `physics_containment` in release with the perf runner's flags. Determinism and containment are ignored in debug, so `just check` does not cover them; `docs/LLM_INDEX.md` points physics changes at the recipe.
- **`docs/plans/1.0-criteria-draft.md`:** a rough draft of the 1.0 definition, eleven candidate workstreams with proposed tiers (among them the UI and text suite, a multi-core rendering pass, a frame loop with a game clock and timers, an API freeze, and logs, crash reports, settings and save slots), an order sketch and questions for the owner. No decision, code or dependency change.
- **Plan execution rules** in `docs/plans/README.md`: treat the plan as the map, do only the requested steps, quote done-when results, keep `status` current, and hand over per-step patches when Git is human-only.

### Changed

- **Profiling workflow** (`docs/perf/profiling-workflow.md`): nothing else may load the machine during a sitting, including other agent sessions, `cargo` builds and the agent's own commands; peak RSS compares only within one sitting (transparent huge pages moved one build 62 MiB). A new "Writing done-when checks" section asks for "not `regressed`" in τ's dead zone, a capture of every CPU row a shared change can move, a cost estimate on the judged rows, and frame-time judgment for relocated work. A/B-build rules cover confirming the rebuild after restoring files, predicting code placement with `nm`, de-inlining by a second caller and a helper's return type changing a hot loop. Criterion benches need a quiet machine and compare through saved baselines. The `tungsten-perf` skill points at these rules.
- **`docs/plans/ui-text-suite-draft.md`** revised again: glyphon's atlas growth and AccessKit's Linux thread checked against their sources, the measurement contract corrected, the first milestone split, and a section on adjacent systems the foundation must not preclude. It stays a draft.
- **`docs/agent-setup.md` and `docs/LLM_INDEX.md`** list the new skills, the recipe and the patch-series tests.

## [0.38.0] - 2026-10-02

Summary: example 01's ball extract is about three times faster (plan `docs/plans/archive/ball-pit-extract.md`). No engine, physics or render code changes, no decision and no determinism or benchmark digest change; it is an example-level rework of one extract loop, plus a revised discussion draft.

### Changed

- **Example 01 ball extract** (`extract_balls` in `examples/01_platformer/src/extract.rs`): two zipped `query2_opt2::<Ball, Position, _, _>` queries read all six columns with no per-ball component lookup; a last-batch memo and a 32-slot direct-mapped sprite cache replace a `HashMap` entry and a registry lookup per ball; balls outside the camera view are skipped; the flame sprite IDs are a `const` array instead of eight formatted strings a frame. Indicative timing, taken with a video encoder running and a clean rerun owed: about 0.10 µs per ball before and 0.03 µs after (1.2 ms to 0.35 ms at 12,000 balls in view). Two batches on one texture now come out in first-seen order instead of `HashMap` iteration order.
- **`rainbow_rgba`** takes one `powf` instead of three, since at full saturation and value only one channel ramps; the output is bit-identical.
- **`docs/plans/ui-text-suite-draft.md`** revised: constraints checked against the text, window and input code and the locked crate sources, a proposal summary, a text-engine seam sketch and a milestone ladder. It stays a draft: no decision, no code and no dependency change.

## [0.37.0] - 2026-10-02

Summary: a slow frame no longer collapses a dense awake pile (`D-094`, plan `docs/plans/archive/physics-dense-pile-collapse.md`). One physics step advances at most 1/30 s, so below 30 FPS physics runs slow instead of softening its contacts; example 01 caps its balls at 12,000. Nothing changes at a 1/60 s step: the determinism hash and all eight benchmark digests are unchanged, and no owned metric reads `regressed`.

### Added

- **`PhysicsConfig::max_step_dt` (`D-094`):** the longest simulated time one `physics_step` call advances, 1/30 s by default; `<= 0` is unbounded. Code that builds a `PhysicsConfig` as a full literal adds the field.

### Changed

- **Example 01** stops spawning at `BALL_CAP = 12_000` live balls, and a spawner at the cap drops its accumulated time. The cap is a presentation guard, not the fix.
- **Recorded limit** in `docs/known-issues.md`: a soft contact carries a bounded load (about 65 ball weights at gravity 3,600), so a deep pile is compressed at any frame rate. `contact_hertz` is the lever and needs a constant substep first.

### Fixed

- **Dense pile collapse on slow frames (`D-094`):** contact stiffness is capped at a quarter of the substep rate and the substep was the frame dt over 4, so a frame longer than 1/30 s softened every contact, the softer pile cost more, and the dt ended pinned at the 0.1 s cap of `D-088`. In example 01 a pit of 10,000+ balls went above 200 ms of physics a frame, lost balls through the floor and never recovered; three stalled frames were enough, in any container and with any body shape. The step now clamps the dt it advances. An 11,502-ball pile keeps every body through 15 s of 0.1 s frames (938 below the floor before), and with the frame dt fed from the step time the pit fills to 18,000 balls at a 25–31 ms step (448–536 ms before). Below 30 FPS simulated time falls behind frame time, and other systems still see the frame dt.

## [0.36.0] - 2026-10-02

Summary: M31, the last Phase 4 milestone (`D-093`, plan `docs/plans/archive/phase4-milestone-31-mesh-particles-transitions.md`): a particle config can draw an instanced triangle mesh instead of a sprite quad, and a state change can run behind a screen transition (fade, radial wipe, dissolve, pixelate). Accepted with it: `particles` `stage.unattributed` p50 reads `regressed` at the threshold after the mesh particle work (2.07 → 2.14 ms; `noisy` at 2.10 ms in the final suite), and `churn` `stage.flush` p50 reads `regressed` in the final suite (2.41 → 2.54 ms; `noisy` in two recaptures), which no M31 change touches.

### Added

- **Mesh particles (`D-093`):** the manifest section `particle_meshes` holds inline triangle meshes (`vertices`, `u16` `indices`), and `ParticleConfig.render` selects `{"kind": "quad"}` (default) or `{"kind": "mesh", "mesh": "<id>"}`. A mesh particle is still one entity and carries `MeshParticle` in place of `Sprite`. New in core: `ParticleMesh`, `ParticleMeshAssetId`, `ParticleMeshRegistry`, `ParticleRender`, `MeshParticle`, `ManifestError::InvalidParticleMesh`. New in render: `MeshParticlePipeline`, `MeshParticleBatch`, `MeshParticleInstance`, `Renderer::upload_particle_mesh` and `Renderer::update_mesh_particles`. New in the umbrella crate: `asset_loader::load_particle_meshes`, `extract_mesh_particles` and `spawn_mesh_particle_via`. A manifest reload registers new meshes and re-uploads changed ones under the same ID.
- **Screen transitions (`D-093`):** `Transition`, `TransitionEffect` (`Fade`, `WipeRadial`, `Dissolve`, `Pixelate`), `TransitionPhase` and `TransitionState`, with `StateStack::request_push_transition`, `request_pop_transition` and `request_replace_transition` (each returns `false` when a transition is already queued or active) and `is_transitioning`, `transition_state`, `transition_cover` and `transition_pass`. The command applies on the frame the cover completes; the pass draws as the last post-stack slot.
- **Fixtures and checks:** example 04 trails its first bouncer with mesh particles (`TUNGSTEN_MESH_TRAIL_FIXTURE=off` leaves the trail out); example 03 takes `TUNGSTEN_TRANSITION_FIXTURE={none|fade|wipe_radial|dissolve|pixelate}`. `just smoke` gains a five-row M31 section, and `just visual` also runs `mesh_trail_draws_instanced_triangles` and example 03's `each_transition_effect_changes_the_frame`.

### Changed

- **`ParticleConfig.sprite`** is optional for a mesh config and still required for a quad config. Code that builds a `ParticleConfig` literal adds `render: ParticleRender::Quad`.
- **Example 03** changes state through engine transitions (fade, pixelate, radial wipe, dissolve) and fades its text with the cover; its tweened overlay sprite and `TweenComplete`-driven replace are gone.
- **Recorded limits** in `docs/known-issues.md`: a transition does not cover screen-space text, mesh particles draw above every sprite, the radial wipe's center pixel at an odd window size, and pixelate never hides the frame.

## [0.35.0] - 2026-10-02

Summary: five correctness fixes (`D-088`–`D-092`, plan `docs/plans/archive/p2-correctness-pass.md`): the frame dt is capped, manifest roots validate and reload as one merged graph, a stock post effect can repeat with different parameters, stock post shaders hot-reload, and a physics arrival pass stops a pushed body at the gate behind it. The pass costs the two physics benchmark rows 5–9% of `physics_step`. Documentation cleanup and plan retirement.

### Changed

- **Documentation cleanup:** archive the GPU performance, platformer polish and debug/docs cleanup plans; shorten the current design status and shipped Phase 4 scope; correct architecture, reload support, font IDs, capture availability and stale references; route local skills to canonical guides.
- **`docs/known-issues.md`:** the live home of open findings, recorded limits, unchecked platforms and follow-ups, with rows in `docs/README.md` and `docs/LLM_INDEX.md`. The repository review of 2026-09-25 moves to `docs/plans/archive/repo-review-2026-09-25.md` without its four P2 rows.
- **`asset_loader::reload_manifest`** takes the list of manifest roots where it took one path (`D-089`). `just visual` also runs example 04's `post_regression` test.

### Fixed

- **Frame dt cap (`D-088`):** `DeltaTime.dt` is the elapsed time capped at 0.1 s. A stall no longer reaches the systems and the physics step as one long step: a 2 s step put 1 of 5 stacked boxes and 6 of 30 piled circles through the floor. Smoke runs keep 1/60 s.
- **Manifest roots (`D-089`):** a material in one manifest root can name a shader in another, in either root order. An edit to any root manifest reloads the merged graph, where a reload used to load one root over it and report the other roots' sprites as removed. The watcher is built when the app runs, so the order of `enable_hot_reload` and `set_manifest_roots` no longer matters.
- **Repeated stock post effects (`D-090`):** each post-stack slot owns its params buffer, so two passes of one effect draw with their own parameters. Both used to draw with the parameters written last.
- **Stock post shader reload (`D-091`):** a body edit to one of the 17 stock post shaders rebuilds that effect's pipeline. The edit used to be validated, cached and logged as reloaded while the frame did not change.
- **Pushed bodies (`D-092`):** a body the solver accelerates is checked against every neighbour its new velocity reaches, clamped and moved again within the substep. A resting body hit by a pusher of up to 1,000 times its mass no longer ends beyond the gate behind it (246 of 1,536 cases before, in three spawn orders), and a slow push no longer crosses a thin static wall (9 of 320). Simulation results change: the determinism hash and the `physics`, `physics-sparse` and `integrated` digests are new. Accepted with it: `physics_step` p50 reads `regressed` in `physics` (5.61 → 6.08 ms) and `physics-sparse` (3.45 → 3.63 ms), the pass's own work in two collision-heavy scenes; `integrated` reads `unchanged`. Two cases stay open in `docs/known-issues.md`.

## [0.34.0] - 2026-10-02

Summary: GPU and render-path performance pass (`D-085`–`D-087`, plan `docs/plans/archive/gpu-perf-pass.md`, with three experiments left unapproved). The text pipeline keeps a bounded layout cache and the post chain keeps its GPU objects between frames, the default extract and the tilemap extract write one pass into kept buffers, and the last full-screen stage renders into the swapchain. Measured on benchmark suite v2: `gpu-throughput` runs at 68.7 FPS where it ran at 38.6, `particles` at 158.5 against 89.7, `integrated` at 117.7 against 94.9 and the empty frame at 2,074 against 1,101; `gpu` peak RSS falls from 720.9 to 168.9 MiB and no frame of it exceeds 1.5 × the run's p50. `display.frame_rate_cap` and the perf runner's pacing overrides now take effect, and telemetry reports the frame `interval`. Pixels are unchanged. In the render crate, the post-chain recording functions take `&mut self` and `default_pass_order` takes a `PresentPath`.

### Added

- **`TUNGSTEN_DISPLAY_PRESENT_MODE` and `TUNGSTEN_DISPLAY_MAX_FRAME_LATENCY`:** environment overrides for `display.present_mode` and `display.max_frame_latency`. Unlike the `TUNGSTEN_RENDER_*` pair, they win over values the `display` section of `tungsten.json` sets.
- **`TUNGSTEN_CAPTURE_DIRECT=1`** and `Renderer::capture_frame_direct`: beside `TUNGSTEN_CAPTURE_FRAME`, capture what the direct present path draws (`D-087`) instead of the blit path's source. A test hook: `just visual` uses it to check that both paths draw the same image.
- **Frame-cap smoke row:** `just smoke` ends with 20 frames of `example-03-scene-state` at `display.frame_rate_cap = 20`, which must take at least 0.9 s.
- **`interval` in the `frame:` telemetry line and `FrameTimings::interval_ms`:** the time from the previous frame's start to this frame's start, `n/a` and `None` on the first frame. It is the frame's period, so FPS is 1000 / mean `interval`; `total` leaves out the telemetry logging, a frame cap's wait and the event loop's turnaround, 0.05–0.13 ms per frame in the benchmark rows. `just perf` reports it as a stage that no row owns and counts it in neither `unattributed`, the stacked stage bars nor capacity's limiting stage.
- **Spikes and the largest frame per run:** `capture.json` holds `runs[].spikes`, the measured frames whose `total` exceeds 1.5 × the run's p50, and the capture README lists it per run beside the run's p50 and largest `total`.

### Changed

- **Text layout cache (`D-085`):** the text pipeline looks a section up before it builds a buffer, keeps about three frames of layouts where it kept 360 frames' worth, recycles evicted buffers, and skips glyphon's prepare and the atlas trim on a frame whose sections and viewport equal the last one drawn. Pixels are unchanged.
- **Post-chain GPU objects (`D-085`):** bloom's uniform buffers and bind groups, the stock effects' source bind groups, SMAA's four bind groups and the present blit's one are built once per target allocation (`RenderTargetPool::generation`), not every frame. Camera, light, material, stock-effect and bloom uniform writes are skipped when the buffer already holds the bytes, and `stage_render` borrows the `PostStack`. `PostStackRenderer::record_pass` and `record_bloom_slot`, `BloomPipeline::record_pass`, the `SmaaPipeline::record_*` functions and `Renderer::update_lights` now take `&mut self`. Pixels are unchanged.
- **Measured** (`just perf compare`, the session baseline `gpu-pass-a0` against `gpu-pass-a3`, five runs a side): `gpu` `render_encode` p50 2.72 → 2.18 ms and p95 3.15 → 2.53 ms, `extract` p50 2.00 → 1.27 ms; no frame above 1.5 × p50 in 300 or 900 frames, where there were 2 and 8; largest frame 23.34 → 14.41 ms; peak RSS 720.9 → 168.9 MiB, and 904.7 → 168.7 MiB at 900 frames. `integrated` `total` p50 10.59 → 9.64 ms and p95 11.25 → 10.27 ms, 94.9 → 104.7 FPS, peak RSS 271.8 → 167.4 MiB. `gpu` creates no bind group and 5 buffers per frame, where it created 31 and 38. Its frame rate is unchanged (88 FPS): the row waits on the GPU.
- **Default extract and tilemap extract (`D-086`):** `extract_sprites_default` writes one instance and one 16-byte sort key per visible sprite in a single pass, sorts the keys only when the query is not already in painter order, and resolves asset IDs through a small cache. `extract_tilemaps` resolves a tileset entry once per map and call. Both reuse buffers the app keeps between frames (`App::new` inserts them as a world resource) and allocate per call without them. Batches, instances and pixels are unchanged, and so are both signatures; a tile layer that spans two atlas pages now yields its batches in first-seen order.
- **Measured** (`just perf compare`, `gpu-pass-a3` against the tree with `D-086`, five runs a side): `gpu-throughput` `extract` p50 24.27 → 13.06 ms and 38.6 → 68.7 FPS, with 1,610 → 2 page faults and 8.81 → 0.17 ms of kernel time per frame; `particles` `total` p50 11.05 → 6.17 ms, 89.7 → 158.5 FPS; `integrated` `total` p50 9.64 → 8.58 ms and p95 10.27 → 9.20 ms, 104.7 → 117.7 FPS; `gpu` `extract` p50 1.27 → 0.66 ms. Accepted with it: `particles` `animate_sprites` p95 reads `regressed` (0.38 → 0.40 ms; 0.37 → 0.40 ms with 15 runs a side), peak RSS rises by 2 to 5 MiB in `particles` and `integrated`, and both rows now run below their calibration bands.
- **Direct present path (`D-087`):** the last full-screen stage of a frame renders into the swapchain and the text overlay draws there: SMAA's neighborhood pass, else the last post pass or bloom's composite, else the scene pass or its MSAA resolve. The present blit and `PresentSource` are used only on a frame with a screenshot armed, whose source and pixels do not change. `default_pass_order` takes a `PresentPath`, GPU timings have no `present` pass on a normal frame, `render_span` ends with the text overlay, and the `gpu` benchmark row no longer owns `gpu.present`. Both paths draw byte-equal images.
- **Measured** (`gpu-pass-a2-landed` against the tree with `D-087`): the empty frame runs at 2,074 FPS against 1,101 (`render_span` p50 0.41 → 0.15 ms); `gpu` `total` p50 11.05 → 10.72 ms, 88.3 → 90.2 FPS; GPU time per frame 10.93 → 10.60 ms in `gpu`, 6.70 → 6.30 ms in `integrated` and 1.45 → 0.72 ms in `physics`; a `gpu` frame records 18 render passes against 19 and wgpu adds no clear pass. Accepted with it: the surface clear moves into the pass that first writes the swapchain, so `gpu` `smaa_neighborhood` p50 reads `regressed` (0.61 → 0.79 ms) while the three tail passes sum to 0.97 → 0.85 ms, and `particles` `unattributed` p95 reads `regressed` (2.47 → 2.71 ms) with the row's `total` unchanged.
- **`docs/perf/benchmarks.md`, `docs/perf/profiling-workflow.md`:** dated numbers for `gpu`, `gpu-throughput`, `particles` and `integrated`; the text-cache finding closed; the default extract's per-frame allocation and its rewrite; the surface clear; code placement on the `ecs` and `churn` rows and the per-run mode of `ecs`; the per-frame values of the scene pass in the GPU diagnostic run; the direct present path; where a GPU-bound frame waits.
- **Measured on 2026-10-02** (Session B of the render-path pass; its suite reads no `regressed` and no `improved` owned verdict against the session's baseline): by `interval` the rows run at 89.7 FPS (`gpu`), 67.5 (`gpu-throughput`), 157.8 (`particles`) and 117.8 (`integrated`), 0.5–1.5% below 1000 / mean `total`. Capacity at 60 Hz is scale 1.61 for `gpu`, 1.04 for `gpu-throughput` (417,708 sprites) and 1.76 for `integrated` (4,391 actors). The docs add the pacing matrix of that day (`immediate`, `mailbox` and `fifo` on `gpu`, `integrated` and `gpu` at `min`), the `ecs` rows that move with its per-run mode, and what the session left open: extract culling, the X11 compositor-bypass hint and the shape-run cache experiment were not run.

### Fixed

- **`display.frame_rate_cap` limits the frame rate.** Every frame requested its next redraw at frame end, which woke the event loop before `ControlFlow::WaitUntil` could wait, so a cap changed nothing. A capped frame now leaves the request to `App::about_to_wait`, which makes it once `frame_start + 1 / cap` has passed; uncapped frames are unchanged. Measured on `gpu` at `min`, 360 frames: 6.22 s at a cap of 60 and 2.81 s at 144, where both took 1.5–1.6 s before. The rate reached is 59.7 and 142.1 FPS, because each deadline counts from the frame's own start; on a quiet machine the `interval` field reads 16.70–16.72 ms at p50 and at most 17.30 ms at p99 at a cap of 60 (59.8 FPS). The HUD `fps` row still shows `1000 / CPU frame time` and overstates the rate under a cap.
- **`just perf run --present-mode` and `--max-frame-latency` apply.** They set `TUNGSTEN_RENDER_PRESENT_MODE` and `TUNGSTEN_RENDER_MAX_FRAME_LATENCY`, which lose to the `display.*` fields the checked-in `tungsten.json` sets, so every override capture ran `immediate / 1` and still read valid. The runner now sets the two `TUNGSTEN_DISPLAY_*` overrides, and a run whose `backend:` line does not confirm the requested mode and latency makes the capture invalid. `docs/perf/profiling-workflow.md` states the precedence.

## [0.33.0] - 2026-10-01

Summary: release procedure rework (`D-079`) and physics and ECS performance pass 2 (`D-080`–`D-084`, plan `docs/plans/archive/physics-ecs-perf-pass.md`). For releases, the agent runs every check and hands over five plain commands; merging the release pull request in GitHub publishes. The performance pass, measured on benchmark suite v2, cuts `physics_step` by 58% in both physics rows, `churn` `flush` by 68%, `integrated` `total` by 13% and `ecs` `follow` by 11%. Physics trajectories change (pair order), so the three physics-bearing benchmark digests did; no API is removed. No example or asset changes.

### Added

- **`World::flush_reusing(&mut CommandBuffer)` (`D-084`):** flushes a buffer and leaves it empty with its storage. `App` keeps one buffer and drains it every frame; `World::flush(CommandBuffer)` is unchanged for one-off buffers.
- `ecs_bench`: `command_buffer_reused_flush_1k_spawns`.

### Changed

- **Physics broadphase layout (`D-080`):** the safety-net sweep queries a statics-only grid once a frame's sweep queries reach the static count; compact worlds get a direct cell table, with the hashed table as the fall-back; cell coordinates use an inline floor; the pair query reads one flag byte per proxy. Results and candidate order are unchanged.
- **Physics pair repair (`D-081`):** a proxy that exhausts its travel budget has only its own pairs rebuilt, under a budget with the exact gravity term and a `2·linear_slop` margin. A full rebuild remains for frame start, a contact wake and more than a quarter of the awake bodies tripping. Pair order, and so solver order and trajectories, differ from 0.32; the step stays deterministic.
- **Physics sleep state (`D-082`):** per-body sleep entries sit in arrays parallel to the proxies and are rebuilt by entity key only when the body sequence changes.
- **ECS column storage (`D-083`):** an archetype's columns are a `Vec` in sorted type-key order, created with the archetype, and rows move between columns without boxing. `World::get` and `get_mut` find their column through a per-archetype slot table. Iteration order is unchanged.
- **Command buffer (`D-084`):** recording an insert or a removal no longer boxes a command (typed value queues, function-pointer removals), and a flush applies consecutive inserts on one entity as one archetype move. Flush results equal one-by-one application.
- **Measured** (`just perf compare`, the baseline `pre-physics-ecs-pass` against the final suite, five runs a side, p50 unless stated): `physics` `physics_step` 13.25 → 5.56 ms; `physics-sparse` `physics_step` 8.05 → 3.42 ms (p95 9.82 → 3.48); `churn` `flush` 8.48 → 2.69 ms, with 208 `malloc` calls per frame where there were 293,960; `ecs` `follow` 1.95 → 1.74 ms and `update` 10.74 → 10.46 ms (`noisy`, just under the 3% threshold); `integrated` `total` 12.12 → 10.55 ms. One owned metric reads `regressed`: `particles` `animate_sprites` p95, 0.35 → 0.38 ms, accepted in `D-084`.
- **`docs/perf/benchmarks.md`:** the three physics-bearing digests; dated numbers for the six rows the pass moved, with the `churn` mode table and the `integrated` `actors` sweep and `tile_collision` numbers measured again; the two accepted readings; new engine findings (contact wakes still rebuild the pair list, the cost of many archetypes, readings that follow the allocator's state through string sprite IDs); and a "Below the calibration band" note for `physics`, `physics-sparse` and `churn`, whose `total` p95 now sits under 8 ms with unchanged defaults. `DESIGN.md` §ECS follows `D-083` and `D-084`; `docs/perf/profiling-workflow.md`'s hotspot list no longer names boxed commands.
- **Release procedure (`docs/releases.md`, `D-079`):** one hand-off replaces the seven pasted blocks. The agent cuts, runs every check and ends with `git add`, `git commit`, `git tag`, `git push origin 0.NN vX.Y.Z` and `gh pr create`; the human pastes them and merges the pull request in GitHub. The local squash merge, refspec pushes and post-merge tag are gone; the tag names the tested milestone commit.
- **`.github/workflows/release.yml`:** runs when a pull request merges into `main`, on pushed prerelease tags and on `gh workflow run release.yml --ref TAG`. A merge publishes when `v<workspace version>` names its head commit and the merge commit has that tree; pushing a final tag publishes nothing by itself.
- **`just release-preflight`:** prints the remaining commands in every state. `--message` accepts the uncommitted cut as the release commit; it checks that `main` is contained in the branch, reports the pull request and its merged tree, matches pull-request and manual runs, and proposes `gh workflow run` when nothing started one. `--branch` defaults to the checked-out branch; `--base` and `--no-pr` are new.
- **`scripts/release.py version`** prints the workspace version of a consistent tree.
- `tungsten-release` and `tungsten-finalize` skills, `AGENTS.md`, `docs/agent-setup.md` and `docs/LLM_INDEX.md` follow the new procedure. `DECISIONS.md` adds `D-079`, superseding the tag-push-only trigger of `D-071` and the post-merge tag and local squash of `D-074`.

## [0.32.0] - 2026-09-30

Summary: benchmark suite v2 (`D-078`), plan `docs/plans/archive/benchmark-suite-redesign.md`. Six scalable, deterministic benchmarks in `example-02-bench` and a standard-library Python runner replace the four `STRESS_SCENE` scenes of `example-02-sprite-stress` and `scripts/perf-capture.sh`. No engine or library-crate behavior changes: two comments in `crates/tungsten/src/app.rs` name the new runner. Perf numbers from the old scenes aren't comparable with the new suite.

### Added

- **Benchmark suite (`examples/02_bench/`, `D-078`):** one binary runs `physics` (pachinko and sparse rows), `ecs`, `churn`, `gpu` (default and throughput rows), `particles` or `integrated`, selected by `TUNGSTEN_BENCH` and configured by `TUNGSTEN_BENCH_PRESET`, `TUNGSTEN_BENCH_SCALE` and `TUNGSTEN_BENCH_SET`; `TUNGSTEN_BENCH_DESCRIBE` prints the knob schema. Each benchmark owns one bottleneck, logs `bench-config:` and per-frame `bench:` counters, declares validity guards and is calibrated to a `total` p95 of 8–16 ms on the reference machine. Textures are generated in code; the only asset files are the `bench_heavy` shader and material.
- **Benchmark runner (`scripts/bench.py`, `scripts/bench_report.py`):** `just perf describe | run | suite | compare | capacity | baseline`. Captures record provenance, per-run statistics, peak RSS through `os.wait4`, RSS growth, guards and a determinism digest, and exit 3 when invalid. Compare judges owned metrics with per-run Welch intervals and practical thresholds (jitter takes p99's) and writes `compare.md`, a self-contained `compare.html` and `compare.json`, for single captures and whole suites. Capacity search reports each row's largest scale within a 60 Hz or 144 Hz budget and its limiting stage. `scripts/test-bench.py` (23 tests) runs in `just perf-test`.
- **`docs/perf/benchmarks.md`:** the ownership table; one section per benchmark with its workload, knobs, presets, counters, guards, owned metrics, calibrated defaults and version history; the engine findings the suite exposed; open proposals; a checklist for adding a benchmark.
- **Smoke and visual coverage:** a Benchmarks section in `scripts/smoke-examples.sh` runs every benchmark at `min` and `default`, plus physics `sparse-min` and `sparse` and gpu `throughput` (15 rows). `examples/02_bench/tests/visual_regression.rs` compares the `gpu` benchmark's `visual` preset with the new fixture `gpu-visual.png`.

### Changed

- **`docs/perf/profiling-workflow.md` rewritten** for the new suite: capture rules (including no remote-desktop encoder during captures), configuration, capture layout, telemetry lines, validity, compare verdicts, capacity search, memory, tracked rows, suites and the `--preset` rule, profiling, hotspots, frame pacing, backends and Criterion. The fixed stage guardrails are dropped; the capacity budgets remain.
- **Recipes:** `just perf` runs `scripts/bench.py`, `just visual` runs `-p example-02-bench`, and `just perf-test` runs `scripts/test-bench.py`.
- **Python 3.12 for the shared checks:** `just script-test` runs the runner's tests through `just perf-test`, so `README.md` and `docs/agent-setup.md` now ask for Python 3.12+ instead of 3.9+.
- **Smoke:** the M25 MSAA × depth-sort matrix runs on `gpu` at `min`, the render-features GPU-timing row folds into the `gpu` default row, and the generic example loop covers four examples.
- `AGENTS.md`, `README.md`, `DESIGN.md`, `docs/LLM_INDEX.md` and the `tungsten-perf` skill name only the new suite; `scripts/check-repo.py` checks `docs/perf/benchmarks.md`; `.cargo/config.toml`'s flags note names the new runner.
- `DECISIONS.md`: adds `D-078`, which holds the April 2026 Vulkan pacing matrix and narrows clause 6 of `D-044` (HUD overhead is measured on the new suite).

### Removed

- **`examples/02_sprite_stress/`** (package `example-02-sprite-stress`): the `baseline`, `ecs-high-load`, `physics-stress` and `render-features` scenes, `STRESS_SCENE` and `STRESS_COUNT`, its visual test and `baseline-sprite-stress.png`.
- **`scripts/perf-capture.sh`** and `scripts/test-perf-capture.sh`.

## [0.31.0] - 2026-09-29

Summary: platformer ball pit, spreading fire, a fireball spell and burning-ball intensity (plans `docs/plans/archive/platformer-ball-pit-effects.md`, `docs/plans/archive/platformer-spreading-fire.md`, `docs/plans/archive/platformer-fireball-spell.md`, `docs/plans/archive/platformer-fire-intensity.md`), plus the owner-directed art passes of `docs/plans/archive/platformer-polish-pass.md` (steps 6–12 and D7; its Rust refactor steps were deferred). The engine gains PCM WAV decoding (`D-077`); no library API or rendering output changes.

### Added

- **PCM WAV decoding (`D-077`):** symphonia's `pcm` codec feature joins the existing `wav` demuxer, so ordinary PCM WAV files decode at load time like OGG, MP3 and AAC. `crates/tungsten-core/tests/audio_decode.rs` now checks the stereo PCM fixture's rate, channels and exact 11,025-frame length instead of pinning its rejection.
- **Platformer ball pit:** middle mouse spawns half-size glass marbles at five times the orb rate with their own spin animation. Small-ball impacts above a 420 px/s closing speed burst rainbow particles under per-frame caps, and black-hole vortex births spiral inward; a regression test fills the level with 2,048 mixed balls. `S` alone stops audio, and `platformer_bindings` (`examples/01_platformer/src/setup.rs`) keeps the example-local bindings across shared `input.json` hot reloads.
- **Spreading fire (`examples/01_platformer/src/burning.rs`):** fire hazards ignite small balls, including swept crossings, and still destroy normal balls. Burning balls ignite touching small balls through symmetric current-frame contacts, and resting piles spread one contact hop per pass. A burn lasts 10 s without refresh and leaves a charcoal ball that never reignites; black holes douse burning balls inside their radius with a steam puff and a throttled sizzle. Each burning ball shows layered flickering flames, a glow and flying sparks from a rotating, bounded pool of 128 emitters that drains after burnout.
- **Fireball spell (`examples/01_platformer/src/fireball.rs`):** Mouse 4 (winit `Back`) casts a missile from the player toward the cursor. It falls under low gravity, bends around black holes and explodes on the first solid it touches, igniting nearby small balls.
- **Synthesized sound effects:** `examples/01_platformer/tools/sfx.py` writes deterministic 44.1 kHz mono 16-bit PCM WAVs for `ex10_fireball_cast_sfx`, `ex10_fireball_blast_sfx` and `ex10_extinguish_sfx`.
- **Soft-glow material:** example-local `examples/01_platformer/assets/shaders/soft_glow.wgsl`, with `ex10_soft_halo` and `ex10_soft_flame` materials, draws glows as analytic, dithered falloffs. Tests cover its Naga validation and the glow material batch.

### Changed

- **Platformer player redrawn:** a new hooded lantern-courier rig with clips idle ×8, walk ×12, jump ×3, fall ×4 and land ×3, plus a double-jump tuck clip (`ex10_player_double_jump`) that plays while rising after an aerial jump. The jump clip is retimed to 200 ms.
- **Fire hazard becomes a fireball:** the 10-frame `ex10_fireball` is drawn unlit, flips to face its travel and stretches with speed. `ex10_fire_trail` is retuned for it, and a new `ex10_fireball_drips` emitter hangs under each one (48 particles per fire across both). The `ball_burn*` configs are unchanged.
- **Scenery, terrain and backdrops redesigned:** new generator modules `actors.py`, `terrain.py`, `backdrops.py`, `scenery.py`, `effects.py` and the shared `pixels.py` under `examples/01_platformer/tools/`. Masonry is periodic and seam-safe, decks stay 23 px, supports form an X-braced trestle, and every backdrop strip wraps. New tiles `cliff_left`, `cliff_right` and `cliff_back_top`; legacy IDs remain as restyled copies. Terrain and big pieces get relief normals and most props are now lit. Gameplay constants, collision and layer occupancy are unchanged; only derived prop and torch-emitter y positions move.
- **Balls redesigned:** the left-click orb is an untinted bronze sphere with rolling teal runes, and the untinted fallback is white instead of magenta. The middle-click marble is neutral glass with a turning cat's-eye ribbon and is the only ball carrying `BallHue`.
- **Glow sprites:** `halo` and `flame_glow` use dithered gradients and the `linear` filter; `push_instance` no longer merges into material batches. The moon, `dust`, `droplet` and `mote` (now blinking fireflies) are redrawn.
- `examples/01_platformer/tools/README.md` documents the new modules, sound synthesis and visual limits; `docs/LLM_INDEX.md` adds `src/fireball.rs` and `src/burning.rs`. `DECISIONS.md`: adds `D-077`, extending `D-028`.

### Removed

- **Platformer grids:** the `fire_dance` animation and every file under `examples/01_platformer/tools/grids/` (six player key poses plus `gate`, `lantern`, `marker`, `orb` and `vines`); the art is now drawn in code. `polish_art.py` keeps only the hearts, burning-ball flames, black-hole and glow sprites.

## [0.30.0] - 2026-09-28

Summary: a profiling-driven performance pass on the three stress scenes (`ecs-high-load`, `physics-stress`, `sprite-stress`), plan `docs/plans/archive/stress-perf-optimization.md`. No shipped behavior, pacing default or public rendering output changes; the empty-stack frame stays byte-identical.

### Added

- **Persistent broadphase pair lists (`D-075`):** physics pair finding runs once per frame instead of once per substep, with per-proxy travel budgets that force a rebuild on velocity/gravity drift, a contact wake or a proxy-set change; the narrow phase, solver, wake test, island union and collision events stay per substep over that list. `crates/tungsten-core/src/physics/broadphase.rs` gains a single-cell fast path (`for_each_in`) that skips per-query dedupe bookkeeping when every staged entry still covers one cell.
- **Persistent warm-start impulses (`D-076`):** while a `D-075` pair list is reused, each substep's solved normal impulse carries by pair index in a parallel array instead of a full keyed-map rebuild; the map still synchronizes at frame boundaries and pair rebuilds so a pair absent for even one substep returns to zero.
- **GPU per-pass timing (`crates/tungsten-render/src/timing.rs`):** `GpuFrameTimings` now reports `render_gpu_ms` (first scene timestamp through the present blit) and `pass_gpu_ms` (per named render pass, including every bloom mip and post-stack slot) alongside the existing scene-only `frame_gpu_ms`, via a timestamp-query pool sized to the active post stack.
- **Render-attribution stress workload:** `examples/02_sprite_stress/src/render_features.rs` exercises mixed sprite batches, materials, lights, bloom, vignette, SMAA and text together against the root manifest's assets, giving GPU per-pass timing a baseline scene.
- **Perf-capture tooling gaps closed:** `scripts/perf-capture.sh` adds awake-frame-only filtering (frames where `physics:` reports `sleeping < dynamic`), per-system and per-GPU-pass timing tables, a `--repeat <n>` mode with a median `summary.md` across runs, an `ecs-high-load` density-preserving sweep (`--ecs-density preserve`), a `physics-stress` sleep toggle (`--physics-sleep off`), `--call-graph`/`--sample-frequency` flags, and dirty-tree provenance fingerprinting.

### Changed

- **ECS archetype maps use a pass-through `TypeId` hasher:** `columns`, `add_edges` and `remove_edges` are now `TypeIdMap` (Bevy's `NoOpHash` equivalent), since `TypeId` is already a high-quality hash and re-hashing it through SipHash on every lookup was wasted work.
- **`World` gains `query3_opt2` and `query3_mut_without`:** columnar three-required-plus-optional and exclude-one-type query shapes that resolve column presence once per archetype, avoiding per-entity `get`/lookup calls in hot systems.
- **Sprite instance upload writes directly into the mapped GPU buffer** (`SpritePipeline::upload`, via `queue.write_buffer_with`) per batch instead of flattening every batch into an intermediate `Vec` first; the flatten was 21% of samples at 100k sprites.
- **`crates/tungsten/src/app.rs` perf-line formatting** is split into named helpers (`format_perf_systems_line`, `format_perf_physics_line`) so per-system and physics summary lines share one formatter.
- `docs/perf/profiling-workflow.md` and `.claude/skills/tungsten-perf/SKILL.md` document the awake-phase filtering, density sweeps and tracked-row conventions above.
- `DECISIONS.md`: `D-062` and `D-066` are partially superseded by `D-075` (staging/drift-budget/prefilter and event-order clauses only; the flat hash and per-substep narrow phase/solve stand); `D-063` is partially superseded by `D-076` (per-substep impulse-map rebuild only; warm-start values and solver order stand). Adds `D-075`, `D-076`.

## [0.29.0] - 2026-09-26

Summary: the platformer art and gameplay revamp (plan `docs/plans/archive/platformer-art-revamp.md`) plus the internal release workflow ([plan](docs/plans/archive/release-workflow.md), `D-074`). The revamp is entirely example-local: no engine, renderer, shader-source or dependency change and no new decision. `docs/plans/phase4.md` carries the user-approved scoped exception that allows this authoring before M32.

### Added

- **64×64 platformer art:** `examples/01_platformer/assets/` is regenerated as a midnight moss-covered ruin — 156 registered sprites across 296 PNGs, with normal and emissive maps for every player, orb, terrain and prop frame. Every tile, character, prop and particle sprite is authored at 64×64; only the sky, moon and parallax strips exceed it. The local `ex10_black_hole_sfx` registration is retained.
- **Offline authoring generator:** `examples/01_platformer/tools/` holds the palette, hand-placed `*.grid` char grids, declarative `level.json` and an explicit 319-entry output inventory (`outputs.json`). `python3 examples/01_platformer/tools/generate.py` rewrites every owned output deterministically; `--check` regenerates into a temporary directory and byte-compares without touching tracked files. `python3 -B -m unittest discover -s examples/01_platformer/tools -p 'test_*.py'` (13 tests) covers malformed grids, palette membership, GID/reference validity, support anchors, opaque bounds versus collision and byte determinism. Authoring is offline only — it never runs at startup, during a Cargo build or during asset loading, and `just script-test` does not discover it (`examples/01_platformer/tools/README.md`).
- **Level:** `assets/tilemaps/level.tmj` is regenerated from `tools/level.json` at 128×48 tiles of 64 px — 8192×3072 world units, within `D-033`'s 128×128 budget — with `background`, `decorations`, `terrain`, `foreground` and `collision` layers, so foreground art stays ornamental while collision is independent. Generated `src/level_layout.rs` supplies dimensions, spawn, kill row, 41 route platforms, 10 named routes, 260 prop placements, 155 shallow slab colliders, 23 lantern anchors, 10 ambient emitters, 12 hazards and 3 moving platforms; game code still resolves assets through the registries and loads no new filesystem paths.
- **Player animation:** `PlayerPresentation` in `src/state.rs` selects `ex10_player_idle` (4 frames), `_walk` (12), `_jump` (2), `_fall` (2) and `_land` (3) after ground detection, holds facing and a landing lock, and resets `AnimationState` only when the clip changes. Facing flips in extract and leaves the collider alone; landing frames share the M30 `SpriteSquashStretch` envelope rather than squashing twice. Props animate through the existing `AnimationState` (`ex10_torch_flicker`, `ex10_waterfall_flow`, `ex10_vines_sway`, `ex10_fire_dance`).
- **`examples/01_platformer/src/gameplay.rs`:** a three-heart `Health` with immunity, knockback and safe respawn; six static spike traps and six moving flames using swept relative contact; one bounded explosion per destroyed ball; three prescribed moving platforms that carry supported riders and release accepted jumps through public `physics` APIs; per-prop `Glow` with native point lights; and vortex particle steering that reuses the engine's single particle loop. A `PlayerLantern` halo and light follow the per-frame lantern anchor, facing and squash; `L` toggles both.
- **Air jump:** one fresh-press aerial jump per landing or respawn (`Player::air_jump_used`), with an `ex10_double_jump` burst and a jump-clip restart. Holding jump cannot spend it and a third press is rejected.
- **Particles:** `ex10_landing_dust`, `ex10_jump_puff`, `ex10_double_jump`, `ex10_torch_embers`, `ex10_waterfall_spray`, `ex10_wind_motes`, `ex10_fire_trail` and `ex10_ball_explosion` join the retuned `ex10_black_hole`. Transient emitters are capped at 16 and despawned only after `ParticleEmitterState` reports drained, ambient emitters live for the level, and the example sets a 2048 global `ParticleBudget`.
- **HUD and post stack:** three camera-independent pixel hearts read `Health` directly and hold their screen size across zoom. The example inserts a stock `PostStack` of Bloom (threshold 0.72, intensity 0.22) and Vignette (strength 0.18) over the pixel art; no new shader source.
- **Release operations (`D-074`):** `docs/releases.md` and the shared `tungsten-release` skill cover preparation, exact-commit publication/verification, unique rehearsals and recovery by release state. `just release-preflight` checks committed files, live Git refs and GitHub state without changing them; temporary-repository tests cover Git failure and resume cases.

### Changed

- **Platformer scale:** `TILE` 32 → 64, `PLAYER_HALF` `(20, 28)`, move 560 units/s, jump impulse 1280, gravity 3600, a 15 px ball collision radius against a 32 px rendered diameter, and black-hole radius/force derived from `TILE`. Balls no longer damage the player. `src/state.rs` derives every grid constant from `level_layout`.
- **Platformer camera:** base zoom fits 18 tile rows instead of the whole map height, follow and clamping work on both axes, and the user zoom multiplier spans 35–300%. Parallax coverage is derived from camera bounds, parallax factors, minimum zoom, viewport aspect and shake overhang; two cloud strips drift independently behind the ridges, woodland and moon.
- **Platformer extraction (`src/extract.rs`):** an example-local extractor over public `TilemapData`/`TilemapRegistry` culls tiles and props to the viewport and draws parallax → background/decorations → terrain → world props, actors and effects → foreground → hearts → cursor, keeping deterministic atlas/filter runs inside each stage. It now also renders animated props, hazards, explosions and the vortex.
- **Platformer systems:** `RUNTIME_SYSTEM_ORDER` captures accepted jumps before physics, detects landings and hazard contacts after it, then selects animations, cleans up drained emitters and synchronises transforms before the squash and camera stages. Material lookup moved into the startup callback, after the manifests register `damage_flash`.
- **`docs/plans/phase4.md`:** records the approved scoped exception letting the platformer author assets and gameplay presentation before M32, and narrows the asset-preprocessing non-goal to engine pipelines so an example-local offline generator with checked-in outputs is permitted. The M32 → M33 ordering, M33 acceptance requirements and the `examples/05_showcase/` prohibition are unchanged.
- **Finalize handoff:** recognizes already-cut versions and hands off to the internal release guide; future milestone tags select the final merged commit. Release checks/cuts retain DESIGN version validation and leave README untouched.

### Removed

- **Superseded platformer art:** the 32×32 tiles and the `cloud_*`, `mountain_*`, `sky_1`/`sky_2`, `lantern_big_*` and `vines_big_*` strips are replaced by the generated roster. The previously unregistered `sprites/player.png` is now the registered 64×64 idle frame, so it no longer needs an asset-coverage exception.
- **README release content:** version/branch/milestone status and release procedure moved out of the project overview.
- `DECISIONS.md` adds `D-074`.

## [0.28.0] - 2026-09-26

Summary: M30 game feel — per-layer parallax scroll, trauma-enveloped camera shake and one-shot squash/stretch (plan `docs/plans/archive/phase4-milestone-30-parallax-shake-squash.md`, `D-073`), demoed in `examples/01_platformer/` and `examples/04_shader_playground/`. No render-crate change; a sprite without `ParallaxLayer` and a `CameraController::default()` reproduce the pre-M30 output exactly.

### Added

- **Parallax (`D-073`):** `ParallaxLayer { scroll_factor: Vec2 }` and `parallax_world_position` in `core/components.rs`. `tungsten/sprite_extract.rs` remaps a layer entity's instance position against `CameraState.position` at extract time (`1.0` = world-locked, `0.0` = screen-locked); `BatchKey`, `z_norm` and `crates/tungsten-render/` are untouched, and `Sprite.z_order` stays the only ordering authority.
- **Camera shake trauma (`D-073`):** `CameraController` gains `shake_trauma`, `shake_decay` and `shake_max_offset` plus `add_trauma` and `shake_offset()` in `core/camera.rs` — a trauma-squared envelope over the existing sine carrier, additive with `shake_amplitude` and inert at zero. `tungsten/camera.rs` calls `shake_offset()` in place of its inline sine block.
- **Squash/stretch (`D-073`):** `SpriteSquashStretch`, `SquashTrigger` and `SquashStretchState` in `core/components.rs`, driven by the symmetric `sin(easing(t) * π)` envelope over `Transform.scale`. Not a `Tween`: `TweenRepeat` has no out-and-back one-shot and `D-055`'s one slot per entity is spent on the M26 damage flash.
- **`tungsten/game_feel.rs`:** `shake_tick_system`, `squash_stretch_trigger_system` and `squash_stretch_tick_system`, exported from `tungsten/lib.rs` and registered by examples like `camera_update_system`. All three read the current event window, so an example registers them after the systems that send, the trigger before the tick, and `shake_tick_system` before `camera_update_system`. Structural work routes through `CommandBuffer` (`D-039`); a re-trigger restarts an in-flight envelope in place.
- **Events:** `ShakeEvent { trauma_add }` and `SquashEvent { entity, trigger }` in `core/ecs/event_queue.rs`, registered in `App::new` alongside `CollisionEvent`.
- **Tests:** parallax remap and squash-envelope cases in `core/tests/components.rs`, trauma cases in `core/tests/camera.rs`, the three systems in `tungsten/tests/game_feel.rs`, parallax extract cases in `tungsten/tests/sprite_extract.rs`, and a new `tungsten/tests/camera.rs` covering follow, dead zone, bounds and shake not accumulating into the base position.
- **Smoke:** an M30 section in `scripts/smoke-examples.sh` — the playground under `TUNGSTEN_GAME_FEEL_FIXTURE=on` (arms trauma and every squash at startup, so a three-frame run reaches both paths) and the platformer's parallax backdrop.

### Changed

- **Platformer:** three parallax backdrop layers (sky `0.05`, mid-hills `0.35`, near foliage `0.6`) spawn in `setup.rs` from existing sprite IDs and are emitted first by the custom extract; `level.tmj`'s fully-filled `background` tile layer is emptied so the sky layer supplies it. The player squashes on the rising edge of `grounded` (new `Player.was_grounded`) and a ball hit now sends `ShakeEvent` alongside the M26 damage flash, so shake and flash fire together. The player quad reads `Transform.scale` and stays bottom-centered on its physics AABB.
- **Shader playground:** three tinted `ex04_quad` parallax layers under a camera that lazily follows one bouncer, with trauma shake and squash on wall and pair impacts. No new assets in either example.
- `DECISIONS.md` adds `D-073`.

## [0.27.0] - 2026-09-25

Summary: the branch-`0.27` release — a physics scale and CCD pass (plan `docs/plans/archive/physics-scale-and-ccd.md`, `D-062`–`D-067`), the agent/tooling restructure and dependency refresh (plan `docs/plans/archive/agentic-restructure.md`, `D-068`–`D-070`), a repository review with targeted correctness fixes, and a tag-triggered release pipeline (`D-071`). No rendering or gameplay features; physics behavior and `PhysicsConfig` change (see Changed and Removed).

### Added

- **Physics island sleeping (`D-065`):** an island of touching dynamic bodies sleeps once every member stays below `PhysicsConfig::sleep_threshold` (20 px/s; `<= 0` disables) for `time_to_sleep` (0.5 s). It wakes on a fast contact, an external `Position`/`Velocity` write, member removal or `physics::wake(world, entity)`; `PhysicsBuffers::{wake, is_sleeping, sleeping_count}` expose the state. A sleeping island emits no `CollisionEvent`s.
- **Columnar optional-component queries (`D-066`):** `World::query2_opt2` / `query2_opt2_mut` (two required plus two optional components, column presence resolved per archetype) back the once-per-frame physics gather and writeback.
- **Physics regression suite:** `crates/tungsten-core/tests/physics_tunneling.rs`, `physics_containment.rs`, `physics_determinism.rs` (identical state hashes across runs) and `substep_probe.rs`; `physics_bench.rs` gains `dense_pile` (3k/10k/25k) and `pile_plus_bullet` scenarios; `scripts/perf-capture.sh --stress-count <n>` scales a scene's body count and reports `update` percentiles.
- **Repository QA:** `just repo-check` checks asset coverage, active-plan lifecycle, documentation links/decision references and agent configuration; `just quick` adds a shorter edit-loop check tier. Synthetic checker tests join `just script-test`, and CPU CI runs repository QA.
- **Shared commands:** `justfile` (`check`, `lint`, `test`, `bench-build`, `smoke`, `visual`, `perf`, `deps`, `ctx`, `script-test`, …) wrapping the raw cargo commands; `just check` runs format check, `clippy -D warnings` and all tests.
- **Dependency policy:** `deny.toml` for `cargo-deny` (advisories, licenses, bans, sources; no git sources). One reasoned advisory exception remains (RUSTSEC-2026-0192, unmaintained `ttf-parser` via cosmic-text).
- **CPU-only CI:** `.github/workflows/ci.yml` (PRs, manual dispatch, pushes to `main`/`0.*`), SHA-pinned actions, read-only token, informational only (`D-070`).
- **Checks and tests:** `tests/shader_coverage.rs` (Naga-validates all 67 WGSL files and the 31 mirror pairs), `tests/audio_decode.rs` with synthetic WAV/Ogg/MP3/AAC fixtures, surface-acquire transition tests, `scripts/test-smoke-examples.sh` (stubbed-cargo smoke regressions), `scripts/check-agent-context.py` (instruction budgets, links, skill symlinks).
- **Visual baseline:** `examples/02_sprite_stress/tests/fixtures/baseline-sprite-stress.png` with reference-machine provenance.
- **Release pipeline:** `.github/workflows/release.yml` builds the examples for Linux and Windows x86-64 on pushed `v*` tags and publishes one archive per platform (binaries plus the runtime files they read), `SHA256SUMS` and the tag's changelog section as a GitHub Release; build only; pre-release tags without their own section (such as `v0.0.0-test`) rehearse as GitHub pre-releases (`D-071`). `scripts/release.py` with `just release-check` (tag/version/changelog/status-line agreement, also in `just repo-check` and as the first CPU CI step) and `just release-cut X.Y.Z` (moves `[Unreleased]`, bumps the workspace version, status lines and `Cargo.lock`); tests join `just script-test`. The `tungsten-finalize` skill is the pre-finalize docs pass; README "Releases" documents tagging, verification and rehearsal tags.
- **Fastest build per CPU (`D-072`):** release archives carry `x86-64-v3` and portable builds of every example. Each example at the archive root is a copy of the new standard-library-only `tools/launcher`, which runs the fastest build the CPU supports with the archive folder as working directory, so examples start from anywhere (`TUNGSTEN_CPU_LEVEL` overrides). Measured on the reference machine: physics-bound frames about 5% faster, ECS-bound frames within 1%; `native` and fat LTO were no faster.
- **Agent setup:** scoped `crates/tungsten-render/AGENTS.md`, tracked `.claude/skills/` shared with Codex via `.agents/skills/` symlinks, `.claude/settings.json`, `.ignore`, `docs/agent-setup.md` (`D-068`).

### Changed

- **Physics broadphase (`D-062`):** the `HashMap` grid becomes a flat prefix-sum spatial hash, staged once per frame and restaged only when accumulated travel exceeds the half-cell margin; candidate pairs pass an AABB-overlap prefilter.
- **Physics solver (`D-063`):** the narrow phase runs once per substep into a contact buffer, and warm-started, clamped accumulated impulses with a soft-constraint bias replace per-iteration MTV projection. New `PhysicsConfig` fields: `contact_hertz`, `contact_damping_ratio`, `linear_slop`, `max_push_speed`, `restitution_threshold`. Bodies rest at about `linear_slop` penetration, and deep overlaps recover over several frames instead of in one push.
- **Physics CCD and substeps (`D-064`):** speculative signed-distance contacts are the primary CCD. A fixed `PhysicsConfig::substeps` (4) with `solver_iterations = 1` replaces the velocity-derived substep count. Collision events fire only on real penetration, so a first touch can be reported one substep later (one frame at a frame boundary).
- **Physics staging (`D-066`, `D-067`):** body state is gathered and written back once per frame instead of every substep. The step stays single-threaded: a color-parallel solver was built, measured and dropped.
- **Physics measurements (Ryzen 5 6600H):** 10k settled-pile jitter 114 → 11.5 px/s; `dense_pile/3000` under its 4 ms budget; 25k awake churn 124.9 ms, still above the ≤ ~16 ms goal.
- **Toolchain:** Rust pinned to 1.98.1 with `rust-version = "1.98.1"`; workspace on edition 2024 / resolver 3 (`D-069`); `rustfmt.toml` sets `style_edition = "2024"`.
- **Dependencies:** wgpu 30.0.1 with glyphon 0.12.0 from crates.io (was a git pin) and cosmic-text 0.19; symphonia 0.6.1; cpal 0.18.2; notify 8.2.0; glam 0.33.10; pollster 1.0.1; criterion 0.8.2; rtrb 0.3.5; compatible refresh of the rest.
- **Rendering:** surface acquire now reconfigures after suboptimal frames (once per window size), retries once on outdated surfaces at the window's current size, recreates lost surfaces and fails clearly if that doesn't recover; `SurfaceColorSpace::Auto` keeps SDR output. Pixels match the pre-upgrade baseline.
- **Audio:** decoding keeps a truncated file's decoded prefix but now reports real I/O errors; MP3/Ogg gapless trimming removes codec padding; cpal opens the default device at its native rate (48 kHz on the reference machine) and the mixer resamples.
- **Instructions:** `AGENTS.md` condensed (≈6 KB), `CLAUDE.md` imports it, `docs/LLM_INDEX.md` and `docs/DECISION_INDEX.md` condensed, plan conventions moved to `docs/plans/README.md`; both project skills corrected (shader hot reload, canonical perf scene).
- **Scripts:** smoke discovery fails on metadata errors or zero examples, and timeouts are reported as timeouts; perf captures record compiler and build flags (`TUNGSTEN_PERF_RUSTFLAGS`) and keep all profiler output in the capture directory.
- **Status docs:** `README.md` and `DESIGN.md` name workspace `0.27.0` on branch `0.27`; the `DESIGN.md` physics section and the README stack line describe the `D-062`–`D-067` pipeline instead of the per-substep uniform grid. `docs/plans/agentic-restructure.md` is done and archived; its platform checks and follow-ups moved to `docs/plans/archive/repo-review-2026-09-25.md`.
- `DECISIONS.md` adds `D-062`–`D-072`.

### Removed

- `PhysicsConfig::max_substeps`, replaced by the fixed `substeps` count (`D-064`).
- `.claudeignore` (not honored by Claude Code; replaced by `.ignore` and a Glob setting).

### Fixed

- Piles under pressure no longer push bodies through thin walls (a 3k pile in 80 px walls leaked 7 bodies in ~400 steps; now 0 over 2,400, `D-063`), and fast bodies no longer tunnel through circles, dynamic walls or each other up to 15,360 px/s (`D-064`).
- Input-map persistence uses exclusive temporary files without a global counter, preserves stale temporary files and cleans up failed writes.
- Animation playback survives shortened hot-reloaded clips; Tiled loading resolves sparse tile IDs and rejects invalid GIDs without underflow.
- Stopped/empty audio voices retire without another mixed callback; commands drain when no audio device is available.
- Sprite/atlas hot reload keeps the previous lit bundle when declared normal/emissive siblings fail validation.
- App initialization failures reach the process result; surface creation returns a typed error; renderer honors `WGPU_BACKEND`.
- Returning from pause to the scene demo menu removes the underlying gameplay state and entities.
- Removed three unread renderer fields and ten unused direct dependency declarations without upgrading packages; corrected documentation and archived completed audits.

## [0.26.0] - 2026-07-06

Summary: the branch-`0.26` release — M29 2D forward normal-mapped lighting plus a performance pass: engine-wide performance-overhead audit, mutable multi-component ECS queries (audit follow-up item #1), migration of every hot engine/example call site off the naive `query_entities` + per-entity `get`/`get_mut` pattern, and a new `physics-stress` canonical capture scene.

### Added

- M29 — 2D forward normal-mapped lighting. New `Light { kind, color, intensity }` component and closed `LightKind::{Point, Directional}` enum live in `tungsten-core::components`; `AmbientLight(Vec3)` is a world resource defaulting to `Vec3::ONE`. Render-side `LightingResources` owns a 544-byte `LightUbo` (cap 16) bound at group 2 of a sibling `LitSpritePipeline` reusing the sprite vertex/instance layout. Manifest-tracked `normal_map` and `emissive_mask` sibling fields on `sprites.<id>` pack into parallel atlas pages keyed by the existing albedo `TextureHandle`. `extract_sprites_default` flips `SpriteBatch.lit` on `SpriteAsset.lit_atlas.is_some()`; lit + material warns and lit wins. `extract_lights` culls by camera-AABB squared-distance, retains directionals first, caps at `LIGHT_CAP = 16`. Empty light list + no aux atlases keeps the captured frame byte-identical to the M28 baseline. New shader id triple (`lit_sprite`, `emissive_mask`, `rim_light`) extends the `D-053` body-edit hot-reload table; helpers are validated-only. The platformer example gains four `walk_*_n.png` and four `walk_*_e.png` sibling assets, an `orbit_lights_system`, and a `TUNGSTEN_LIGHTING_FIXTURE=on|off` env switch wired into the smoke matrix and showcase capture. See `D-061`. No new runtime dependency.
- **Mutable multi-component ECS queries (`tungsten_core::ecs`):** `World::query_mut<T>`, `World::query2_mut<A, B>`, and `World::query3_mut<A, B, C>` yield `(Entity, &mut …)` tuples via per-archetype split column borrows; all yielded refs are mutable (the migrated call sites need double-mut shapes), and distinct `TypeId`s are asserted per call — duplicate component types panic. Backed by new `Archetypes::archetypes_with_mut` / `_two_mut` / `_three_mut` iterators. Extends `D-036`; no new decision entry. Six new unit tests cover in-place mutation, superset archetypes, order-equivalence with the immutable queries, and the duplicate-type panics.
- **`query2_mut_10k` criterion bench (`crates/tungsten-core/benches/ecs_bench.rs`):** 3.44 µs median beside `query2_homogeneous_10k` at 6.69 µs — ~172× under the 591 µs `naive_query2_via_entities_10k` pattern the engine hot loops previously used.
- **`physics-stress` capture scene (`example-02-sprite-stress`):** 3,000 dynamic circle bodies with `Collider`s piling under gravity in a static box, driven through the engine-default extract — the first canonical scene exercising the narrow phase and solver (`ecs-high-load` spawns bodies without colliders). Registered in `scripts/perf-capture.sh` and `docs/perf/profiling-workflow.md`.
- **Performance-overhead audit report (`docs/plans/archive/perf-overhead-audit.md`):** 16 verified findings across rendering, physics, and engine logic with capture artifacts under `perf-runs/20260703T*`, plus a 12-item follow-up backlog ordered by measured impact. Item #1 (this release's ECS work) is marked implemented with measured deltas; items #2/#8 remeasure against the new baselines.

### Changed

- **Engine hot paths migrated to columnar iteration:** physics step (`compute_substeps`, `apply_gravity_and_integrate`, collider gather — `PhysicsBuffers` dropped its per-substep `collider_entities` / `dynamic_entities` scratch lists), `sync_position_to_transform`, `tween_tick_system` (single columnar `Tween` pass with buffered channel application), `particle_count_refresh_system` / `particle_tick_system`, and the four `ecs-high-load` example systems (steer/confine/orient/tint).
- **Measured impact (AMD Ryzen 5 6600H + Radeon 660M, Vulkan, `--release`):** ecs-high-load avg total 80.46 → 63.08 ms (update 76.8 → 62.04 ms); physics-stress avg total 4.88 → 4.41 ms (update 4.36–4.51 → 4.02 ms). sprite-stress and platformer p50 unchanged (0.88 / 1.07 ms). Captures: `perf-runs/20260706T*`.
- Behavioral note: `apply_gravity_and_integrate` now requires `Position` in addition to `Velocity` + dynamic `RigidBody`; a dynamic body carrying `Velocity` but no `Position` (none exist in-tree) no longer accumulates gravity. Recorded in the audit doc's item #1 implementation record.
- Workspace version corrected to `0.26.0`: branch `0.26` ships as `0.26.0`. The interim `0.27.0` bump from the M29 integration commit is rolled back, and the M29 notes previously cut as `[0.27.0]` are folded into this entry.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, and `docs/plans/phase4.md` status lines now agree on workspace `0.26.0`, branch `0.26`, with M25–M29 shipped.
- `docs/perf/profiling-workflow.md` registers `physics-stress` in the canonical capture rules and quick-start examples; `docs/LLM_INDEX.md` sprite-stress row lists the scene modules.

### Fixed

- **0.26 release-polish QA pass:** clippy debt from the M29 ship (which skipped the `-D warnings` gate) is cleared — `LitSpritePipeline::new` and the three M29 emissive-decode functions in `asset_loader.rs` carry documented `#[allow]`s matching the repo's stable-surface convention, two `Rebuild … atlas` log lines use inlined format args, and the platformer's `extract.rs` moves its test module below the items it tests plus documents the HSV-math single-char bindings.
- Release QA pass completed locally: `cargo fmt --all -- --check`, `cargo test --workspace` (599 passed), `cargo clippy --workspace --all-targets -- -D warnings`, `bash scripts/test-perf-capture.sh`, and `WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh` (4/4 examples, all fixture matrices) all passed. Full perf verification for the ECS work ran against the audit baselines: `cargo bench -p tungsten-core --bench ecs_bench`, `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300`, and `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh physics-stress 300`.

## [0.25.0] - 2026-04-25

### Added

- M28 bloom (`D-060`). New `PostPass::Bloom(BloomParams { threshold, knee, intensity, radius })` ships as the 18th `PostPass` variant on the existing reorderable `PostStack`. Each bloom slot runs a multi-subpass program against a new `Rgba16Float` `BloomPyramid` allocated on `SceneTarget`: bright-pass extract into mip 0, an `N-1` 13-tap Karis-weighted downsample chain, an `N-1` 9-tap tent additive upsample chain, and a replace-blend composite that writes `mix(src, src + bloom * intensity, radius)` into the slot's `dst`. Pyramid mip count is sized by `bloom_mip_count_for_size(width, height, render.bloom_max_mips)` with mip 0 starting at half resolution; `bloom_max_mips` is config-validated to `1..=8` (default 6) and overridable via the new `TUNGSTEN_RENDER_BLOOM_MAX_MIPS` env var. Bloom is the first `PostPass` recorded at encoder level — the renderer detects the variant before `PassRecorder::begin` and dispatches `BloomPipeline::record_pass`, which opens its own per-subpass `RenderPass`es. Four new manifest-tracked stage shaders (`bloom_threshold`, `bloom_downsample`, `bloom_upsample`, `bloom_composite`) live under `crates/tungsten-render/src/shaders/stock/` with byte-equal mirrors under `assets/shaders/stock/`; body-edit hot-reload routes through `Renderer::reload_shader` → `BloomPipeline::rebuild_stage_with_module` with `naga` validation and last-known-good fallback. `SceneColor` stays sRGB — only the pyramid is HDR. With `PostStack` empty the captured frame remains byte-identical to the M27 baseline. `example-04-shader-playground` gains a `KeyL` bloom toggle, `Y/H U/J I/K` live-tune bindings, an HUD row for the active bloom params, an emissive-quad sprite for the LDR demo fixture, and the new `TUNGSTEN_BLOOM_FIXTURE=on|off` and `TUNGSTEN_POST_STACK_FIXTURE=bloom_only` env pins. `scripts/smoke-examples.sh` gains a bloom row over the playground.

### Changed

- Workspace version bumped to `0.25.0`.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, and `docs/plans/phase4.md` now reflect branch `0.25` with M25, M26, M27, and M28 shipped.

### Fixed

- **M28 release-polish QA pass:** top-level status docs now consistently include M28 as shipped, `scripts/smoke-examples.sh` reports the M27/M28 fixture denominators correctly, and the 0.25 changelog entry is cut as `[0.25.0] - 2026-04-25`.
- Release QA pass completed locally: `cargo fmt --all -- --check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `bash scripts/test-perf-capture.sh`, `bash -n scripts/smoke-examples.sh scripts/perf-capture.sh scripts/test-perf-capture.sh`, `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --telemetry-only`, and `WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh` all passed.

## [0.24.0] - 2026-04-25

Summary: Phase 4 Milestone 27 - SMAA 1x presentation AA (runtime post-AA modes, renderer-owned SMAA tail passes, manifest-tracked stage shaders, internal lookup textures, and shader-playground controls). Phase 4 scope is tracked in [`docs/plans/phase4.md`](docs/plans/archive/phase4.md).

### Added

- M27 SMAA 1x presentation AA (`D-059`). New `RenderConfig.post_aa` (`Off / SmaaLow / SmaaMedium / SmaaHigh / SmaaUltra`, `#[non_exhaustive]`) and matching `TUNGSTEN_RENDER_POST_AA` env override; renderer-owned three-pass tail (edge → blend weights → neighborhood blend) splices between the M26 `PostStack` and the screen-space text overlay, writing into a new `PresentSource` target that the present blit + screenshot path source. Three new manifest-tracked stage shaders (`smaa_edge`, `smaa_blend_weights`, `smaa_neighborhood_blend`) live under `crates/tungsten-render/src/shaders/stock/` with byte-equal mirrors under `assets/shaders/stock/`; the `area` and `search` lookup textures ship as `include_bytes!` engine-internal content under `crates/tungsten-render/src/assets/smaa/` with MIT attribution. Preset knobs ride a 256-byte UBO so switching presets neither rebuilds nor recompiles a pipeline. `SceneColor` and the post ping/pong targets carry a non-sRGB twin in `view_formats` while SMAA is active so edge detection sees gamma-encoded values. Runtime changes go through new `tungsten::request_post_aa(world, mode)` and apply at a frame boundary — no relaunch (unlike `msaa`). `post_aa = Off` is byte-identical to the M26 frame across the full msaa × depth_sort × post-stack-length matrix. `example-04-shader-playground` gains Tab-cycle and 0/5/6/7/8 quick-set bindings plus a new HUD row showing the applied mode; `TUNGSTEN_POST_AA_FIXTURE` pins a preset for smoke runs.
- `scripts/smoke-examples.sh` appends a `TUNGSTEN_POST_AA_FIXTURE=smaa_high` row over `example-04-shader-playground` with `TUNGSTEN_POST_STACK_FIXTURE=empty`.

### Changed

- Workspace version bumped to `0.24.0`.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, and `docs/plans/phase4.md` now reflect branch `0.24` with M25, M26, and M27 shipped.

### Fixed

- **M27 release-polish QA pass:** SMAA rustdoc now avoids overindented list items, SMAA pass/LUT builders are marked `#[must_use]`, the intentional GPU-layout / bind-layout naming in `SmaaPresetUbo` and `SmaaLayouts` is locally documented for clippy, and shader-playground fixture parsing uses inlined format args. This keeps `cargo clippy --workspace --all-targets -- -D warnings` green without changing runtime behavior.
- **Checked-in input map startup fix:** canonical key serde and the winit bridge now cover the M27 shader-playground bindings (`Tab`, `Digit0`, `Digit5`-`Digit8`), and a core test parses the workspace `input.json` so future key-name drift fails in `cargo test` before example startup.
- Release QA pass completed locally: `cargo fmt --all --check`, `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `bash scripts/test-perf-capture.sh`, `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --telemetry-only`, and `WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh` all passed.

## [0.23.0] - 2026-04-24

Summary: Phase 4 Milestone 26 — materials + post-stack + tween→material bridge (manifest-tracked materials, a reorderable 17-effect post stack, entity-local uniform overrides shared with tween channels, and a new shader-playground example). Phase 4 scope is tracked in [`docs/plans/phase4.md`](docs/plans/archive/phase4.md).

### Added

- M26 materials + post-stack + tween→material bridge (`D-058`). New `materials` section in the manifest graph maps a stable material id to a WGSL shader id + 256-byte `MaterialUniformDefaults`; render-side `MaterialPipeline` reuses the built-in sprite layout and adds a per-material UBO at group 2. New `PostStack` world resource (default empty, byte-identical to the M25 baseline) carries a reorderable `Vec<PostPass>` — 17 stock effects (tonemap, vignette, lut, chromatic_aberration, color_adjust, tone_mono, crt, film_grain, dither, pixel_outline, fade, wipe_radial, dissolve, glitch, pixelate, fog, god_rays) ping-pong between `PostPing` / `PostPong` offscreen targets before the present blit. New `UniformOverrideBlock` component + `TweenChannel::UniformVec4Lane` / `UniformScalar` / `UniformInt` drive per-entity animation into the same 256-byte payload shared with the M32 MSDF outline/glow slot. Stock shaders live under `crates/tungsten-render/src/shaders/stock/` with MIT LYGIA-derived helpers; `assets/shaders/stock/` mirrors them for manifest-driven hot reload. New workspace `damage_flash` material + platformer ball-hit tween fires through the new `Sprite.material_id` path. New `example-04-shader-playground` crate exercises the 17-effect fixture under `TUNGSTEN_POST_STACK_FIXTURE`.
- `scripts/smoke-examples.sh` appends a `TUNGSTEN_POST_STACK_FIXTURE ∈ empty, all` matrix over `example-04-shader-playground`.

### Changed

- Workspace version bumped to `0.23.0`.
- `README.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/phase4.md` now reflect branch `0.23` with both M25 and M26 shipped; the detailed M26 plan moved to [`docs/plans/archive/phase4-milestone-26-materials-post-stack.md`](docs/plans/archive/phase4-milestone-26-materials-post-stack.md).
- `AGENTS.md` §Asset Rules lists the new `materials` manifest section and the vendored `assets/shaders/stock/` mirror rule.
- `DESIGN.md` §Status and §Hot Reload matrix: M26 row added; `shader` row widened to include material-pipeline rebuilds on shader reload.

### Fixed

- **M26 release-polish QA pass:** `MaterialUniformDefaults::to_override_block()` now builds its `UniformOverrideBlock` in one initializer, `PostStack::{as_slice, as_slice_mut}` are marked `#[must_use]`, and `UniformOverrideBlock` no longer exposes its reserved padding tail as a public field. This keeps `cargo clippy --workspace --all-targets -- -D warnings` green without weakening the lint surface.
- Release QA pass completed locally: `cargo fmt --all --check`, `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `bash scripts/test-perf-capture.sh`, `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh ecs-high-load 300 --telemetry-only`, and `WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh` all passed.

## [0.22.0] - 2026-04-24

Summary: Phase 4 Milestone 25 — render foundation (offscreen `SceneTarget` with optional depth + MSAA, named/ordered pass list with an engine-internal present blit, manifest-tracked WGSL with body-edit hot reload, and opt-in GPU depth-test sprite path). Phase 4 scope is tracked in [`docs/plans/phase4.md`](docs/plans/archive/phase4.md).

### Added

- M25 render foundation (`D-057`): offscreen `SceneTarget` (color + optional depth + optional MSAA) driven by a named, ordered pass list (`scene` → `present`). The present pass blits `SceneColor` into the swapchain via an engine-internal `shaders/present_blit.wgsl` with exact-texel `textureLoad` so the default `msaa=1`, `depth_sort=cpu_stable` config stays byte-identical to the 0.21 baseline.
- M25 config knobs in `RenderConfig`: `msaa` (1 | 2 | 4 | 8, default 1), `depth_enabled` (default `true`), `depth_sort` (`cpu_stable` default, `gpu_depth` opt-in) with matching `TUNGSTEN_RENDER_MSAA`, `TUNGSTEN_RENDER_DEPTH_ENABLED`, `TUNGSTEN_RENDER_DEPTH_SORT` env overrides.
- M25 WGSL hot reload: shaders move into `assets/shaders/` under a manifest `shaders` section with a core-side `ShaderRegistry` + render-side `ShaderModuleCache`. Body edits hot-reload through the existing umbrella `notify` watcher after `wgpu::naga` parse + validation; the previous `ShaderModule` + live pipeline stay intact on any validation or rebuild failure.
- M25 GPU depth-test sprite path: `SpriteInstance` gains a `z_norm` field derived from deterministic `(z_order, Entity::id)` painter order; under `depth_sort = "gpu_depth"` the sprite pipeline attaches `Depth32Float` with `depth_compare = LessEqual` so the depth buffer reproduces the same visible order as the CPU-stable path.
- `scripts/smoke-examples.sh` appends a `{msaa ∈ 1, 4} × {depth_sort ∈ cpu_stable, gpu_depth}` matrix over `example-02-sprite-stress` via the new env overrides.

### Changed

- Workspace version bumped to `0.22.0`.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, `docs/plans/phase4.md`, and `docs/plans/phase4-milestone-25-render-foundation.md` now reflect branch `0.22` as the active integration line.
- `AGENTS.md` §Asset Rules: shaders are now manifest-tracked with body-edit hot reload (`D-057`).
- `DESIGN.md` §Status + §Hot Reload matrix: `shader` row added, `SceneColor` format noted.
- `tungsten.json` `render` block documents the new `msaa` / `depth_enabled` / `depth_sort` defaults.
- `renderer.rs` is split: surface/present-mode helpers moved to `surface.rs`, frame timing types moved to `timing.rs`, and the main frame now loops over a `PassOrder` instead of a single inline `begin_render_pass`.
- `SpriteInstance` grew from 40 B to 48 B (+20%) to carry the new `z_norm: f32` and an explicit 4-byte `_pad` for 16-byte GPU alignment. The default-data path still writes `z_norm = 0.0` for callers that build instances by hand (`SpriteInstance::whole`, tilemap extract, custom example extracts).
- Screenshot path simplified: captures now read directly from `SceneColor` after the scene pass (single draw, no duplicate capture-only pass). Under `msaa > 1` the read picks up the resolved target. Baseline image-diff is still byte-stable for the default config.

### Fixed

- **Smoke-mode dt is now deterministic.** Under `TUNGSTEN_SMOKE_FRAMES`, `App::stage_delta_time` pins the per-frame `DeltaTime.dt` to `1/60 s` instead of reading wall-clock. Previously the visual-regression fixture run and a subsequent test re-run would integrate different `dt` values at frame N, so sprite positions (and therefore pixels) diverged across otherwise-identical runs. With the pin in place, smoke-mode captures are reproducible across build profiles and host load, which is what the `visual_regression` fixture requires. Outside smoke mode, dt continues to come from `Instant::now()` as before.
- **M25 QA pass:** four `GpuDepth` / MSAA bugs that would have reached 0.22 release without this sweep.
  - `depth_sort = gpu_depth` now forces the quad / debug-line / text pipelines to carry a matching read-only `DepthStencilState` (`Always` + no write). Previously they declared no depth state and wgpu rejected them the moment the pass attached `SceneDepth`.
  - `Renderer::new` now builds the sprite pipeline with the correct `depth_write` up front; the first frame under `gpu_depth` no longer boots a `depth: None` pipeline against a depth-attached pass.
  - `SceneColorMsaa` drops `COPY_SRC` + `TEXTURE_BINDING` from its usage flags — multisampled textures reject `COPY_SRC` in wgpu, and nothing reads from the MSAA color target directly (the present blit reads the resolved `SceneColor`).
  - `depth_sort = gpu_depth` + `depth_enabled = false` used to panic in the recorder (requested a depth target the pool never allocated). `Renderer::new` now logs and falls back to `cpu_stable` for that combination; `default_pass_order` takes `depth_enabled` so the depth attachment can never disagree with the pool.
- **Painter-order depth orientation:** `z_norm` now decreases along painter order (`(total-1-i)/total`) so `LessEqual` accepts later-drawn fragments as they overwrite earlier overlaps. The previous ascending formula silently culled every later-drawn sprite in overlapping stacks under `gpu_depth`.

## [0.21.0] - 2026-04-23

Summary: Phase 3 Milestone 24 — tween system (closed-enum easings, multi-channel tweens on one component, `TweenComplete` via `EventQueue`, scene-authored tweens, and a fade-on-state-transition demo in `03_scene_state`).

### Added

- **Tween primitives (`tungsten_core::tween`):** `Easing` (Linear/Quad/Cubic/Quart/Sine/Expo/Back/Bounce × In/Out/InOut; `Easing::apply(t)` pre-clamped `[0,1]` — Back/Bounce overshoot intentionally), `TweenChannel` (per-property track for `PositionX/Y`, `Rotation`, `ScaleX/Y`, `ColorR/G/B/A`), `TweenRepeat { Once, Loop, PingPong, Times(u32) }`, `TweenDirection`, `Tween { channels, easing, duration, elapsed, repeat, direction, completed_cycles, on_complete_tag, pending_remove }` with `Tween::new(duration, easing).with_channel().with_repeat().with_tag()` builders, `lerp_f32` / `lerp_u8` helpers, and `TweenComplete { entity, tag }`. Closed `enum` avoids a trait-object dependency per `D-054` / `D-015` rule 3.
- **Scene-authored tweens (`tungsten_core::assets::scene`):** `SceneEntry.tweens: Vec<SceneTween>` plus `SceneTween { duration, easing, repeat, tag, channels }`, `SceneTweenChannel` (tagged-union mirror of `TweenChannel`), and `SceneTweenRepeat` (`once | loop | ping_pong | { "times": n }`). `SceneData::load` runs `SceneTween::validate()` on every tween — non-finite / non-positive durations and empty channel lists are fatal via a new `SceneError::Validation` variant.
- **`tween_tick_system` (`tungsten::tweens`):** advances every `Tween` using `DeltaTime.dt`, writes interpolated `Transform` / `Sprite` fields in-place, and defers terminal completion through `EventQueue<TweenComplete>` + `CommandBuffer::remove_component::<Tween>`; `Once` / `Times(n)` emit exactly one `TweenComplete` and latch `pending_remove` so subsequent ticks cannot re-fire before the next frame-end flush; `Loop` rewinds silently, `PingPong` flips direction at each boundary.
- **Frame-order slot:** `App::render_frame_*` now runs `stage_tweens` between `stage_particles` and `stage_flush_commands`, so tween writes override particle writes on the same frame and `TweenComplete` enqueues before the event flush window rotates (D-039 / D-040).
- **`App::new` event registration:** `EventQueue<TweenComplete>` is pre-registered alongside `CollisionEvent`, `ParticleBurstEmitted`, and `ParticleSystemDrained`.
- **Scene spawn (`tungsten::asset_loader`):** `spawn_scene` inserts one `Tween` per entry; entries carrying more than one tween log `ERROR` and keep the first (D-055 archetypal one-component-per-type).
- **Example 03 fade transitions (`examples/03_scene_state`):** `scene.json` now ships `color_a 0 → 255 / cubic_out / 0.45s` fade-in tweens on five representative hub/ring sprites. `GameplayState::on_enter` spawns a full-viewport black `fade_overlay` that tweens `color_a 255 → 0`. Pressing `state_back` inserts a reverse `color_a → 255 / 0.35s / cubic_in` tween tagged `state_exit`; the new `handle_tween_complete_system` reads `EventQueue<TweenComplete>` and calls `StateStack::request_replace(MainMenuState)` only after the opaque frame arrives. A `PendingTransition` resource gates re-presses while the fade-out is in flight.
- **`tween_tick` bench (`crates/tungsten-core/benches/tween_tick.rs`):** 5 000 entities each carrying `Tween + Transform + Sprite` with two channels and `cubic_in_out` easing; one `criterion` iteration advances the inline equivalent of `tween_tick_system`. Baseline additive to the existing `particle_tick_5k` / `position_integration_50k` / `broadphase_rebuild_5k` / `action_map_dispatch` benches.
- **Integration tests (`crates/tungsten/src/tests/tweens.rs`):** `tween_once_completes_and_removes_component`, `tween_times_fires_once_after_n_cycles`, `tween_loop_never_completes`, `tween_pingpong_reverses_at_boundary`, `tween_position_and_color_together_at_u_half`, `tween_complete_carries_tag`, `tween_without_target_components_is_noop`, `scene_tween_spawns_component_through_command_buffer`. Core unit tests in `crates/tungsten-core/src/tests/tween.rs` cover every easing endpoint + known-sample values and `lerp_u8` clamp behavior. Scene tween parsing + validation covered in `crates/tungsten-core/src/tests/assets/scene.rs`.
- **Decision records:** `DECISIONS.md` adds `D-054` (closed-enum easings, no trait object, no dependency), `D-055` (single `Tween` component per entity with `Vec<TweenChannel>`), and `D-056` (`TweenComplete` routes through `EventQueue`, component removal through `CommandBuffer`). `docs/DECISION_INDEX.md` carries matching takeaways.

### Changed

- Workspace version bumped to `0.21.0`.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.21.0` / branch `0.21` release line, `M24` complete, and Phase 3 closeout.
- `docs/LLM_INDEX.md` gains a Tweens subsystem row and a "Change tween easing/channel behavior or scene-tween authoring" task row.
- `docs/plans/Phase3.md` marks `M24` as `complete` at `v0.21.0` / `2026-04-23`; the implementation plan is archived at `docs/plans/archive/phase3-milestone-24-plan.md`.
- `CLAUDE.md` Status line now reflects `0.21.0` on branch `0.21` with M24 shipped.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, and `./scripts/smoke-examples.sh` all passed.

## [0.20.0] - 2026-04-20

Summary: Phase 3 Milestone 23 — particle system (ECS-native emitters with Arc-snapshot hot reload, per-emitter/global caps, burst/continuous/pulse modes, lifecycle events, and a platformer black-hole demo).

### Added

- **In-tree PRNG (`tungsten_core::rng`):** `Pcg32` (PCG32 XSH-RR, seeded via `Pcg32::seeded(u64)`, `next_u32`, `next_f32`, `range_f32`) plus a `splitmix64(u64) -> u64` helper and a `WorldRngSeed` resource that mints per-emitter seeds through SplitMix64 so emitters are decoupled from spawn order. No new dependency — `rand` / `getrandom` were both rejected under the three-rule acceptance test (`D-049`). Unit tests cover statistical sanity, deterministic replay, and SplitMix64 distribution.
- **Particle asset + registry (`tungsten_core::assets::particle`):** `AssetId<ParticleConfig>`, `Range`, `BlendMode { Alpha, Premultiplied }`, `EmissionKind { Burst { count, once }, Continuous { rate_hz }, Pulse { count_per_pulse, interval_sec, total_pulses } }`, `InitialVelocity { Cone, Radial, Vector }`, `Curve<V: Copy + Lerp>` with a piecewise-linear sampler, and `ParticleConfig` carrying `sprite / max_alive / seed / blend / emission / lifetime / initial_velocity / gravity / drag_per_sec / angular_velocity / start_scale / scale_over_life / color_over_life / alpha_over_life / tint`. `ParticleConfig::load(path)` parses JSON and runs `validate()` (checks ranges are min ≤ max, lifetimes positive, emission parameters non-negative, curves sorted and non-empty). `ParticleConfigRegistry` holds `Arc<ParticleConfig>` per id and exposes `register / replace / get / id_for_name / name_for_id / id_for_path / path_for_id`. `ParticleBudget { global_cap }` and `ParticleActive { count }` are engine-owned resources.
- **Manifest `particles` section (`tungsten_core::assets::manifest`):** `RawManifest` / `ResolvedManifest` gain `particles: HashMap<String, ParticleEntry { path }>`; `ResolvedManifest::load` resolves every `particles.*.path` relative to the manifest directory and reports `MissingParticleFile` when the sibling does not exist; `merge` treats duplicate particle IDs as fatal through the existing `DuplicateId` error. Three new unit tests cover the resolve + merge paths.
- **Particle components (`tungsten_core::components`):** `ParticleEmitter { config: AssetId<ParticleConfig>, seed_override: Option<u64> }`, `ParticleEmitterState { config_snapshot: Option<Arc<ParticleConfig>>, rng: Pcg32, elapsed, continuous_accum, pulse_timer, pulses_fired, active_count, drained, first_tick_done, drain_reported }`, and `Particle { config: Arc<ParticleConfig>, emitter: Option<Entity>, age, lifetime, velocity, angular_velocity, start_scale, base_rgba }`. `ParticleEmitter::new(id)` is the default constructor; `ParticleEmitterState::default()` constructs without a snapshot (resolved on first tick).
- **Particle systems (`tungsten::particles`):** three frame-order-hardened systems — `particle_count_refresh_system` walks live particles and rewrites each emitter's `active_count` + drain latch; `particle_emit_system` resolves the `Arc` snapshot on first tick (from `ParticleConfigRegistry` + `WorldRngSeed`), plans one frame's emission through `plan_emission` (Burst uses a one-shot latch, Continuous uses a rate-hz accumulator, Pulse fires at most one pulse per tick from a `pulse_timer`), clips against per-emitter `max_alive` and global `ParticleBudget.global_cap`, and spawns each particle via `CommandBuffer` (`Particle + Transform + Sprite + Visibility`); `particle_tick_system` ages every particle, integrates `(velocity + gravity * dt) * exp(-drag_per_sec * dt)`, samples `scale_over_life` / `color_over_life` / `alpha_over_life`, applies CPU-side `Premultiplied` RGB premultiply when requested, and despawns age-outs through the command buffer. Events: `ParticleBurstEmitted { emitter, count }` fires on every Burst/Pulse discrete spawn; `ParticleSystemDrained { emitter }` fires exactly once when a drained emitter's `active_count` reaches zero (latched via `drain_reported`). A `spawn_particle_via(world, buf, cfg, origin, rng)` helper is exposed for external callers.
- **Asset loader + hot reload (`tungsten::asset_loader`):** new `load_particles(manifest, world)` parses every `ResolvedParticle`, validates each config's `sprite` against the live `AssetRegistry`, and registers into `ParticleConfigRegistry`. `reload_particle(id, path, world)` parses, validates, and calls `ParticleConfigRegistry::replace`; parse errors and unknown-sprite references warn and retain the previous config (last-known-good per `D-031`). `load_all` runs particle loading after sprite loading so cross-references are verifiable. `HotReloadWatcher` `.json` dispatch tries `AnimationRegistry` first, then `ParticleConfigRegistry`.
- **App wiring (`tungsten::app`):** `App::new` inserts `ParticleConfigRegistry`, `ParticleActive`, `ParticleBudget`, and `WorldRngSeed` resources plus the `ParticleBurstEmitted` / `ParticleSystemDrained` event queues; the frame loop runs `particle_count_refresh_system → particle_emit_system → particle_tick_system` immediately after user systems and before the `CommandBuffer` flush, so spawned particles are visible to the extract path in the same frame.
- **Integration tests (`crates/tungsten/tests/particles.rs`):** six headless tests — `burst_once_emits_exactly_count_then_drains`, `continuous_rate_matches_expected_count` (60 ticks @ 100 Hz → 99–101 particles), `pulse_emits_fixed_pulses_then_drains` (exactly 3 pulses), `per_emitter_max_alive_clips_emissions` (1000 requested, 16 `max_alive` → 16), `global_budget_cap_clips_across_emitters` (cap 10, two emitters → ≤ 10), `hot_reload_snapshot_preserves_live_particles` (`Arc::as_ptr` unchanged after `replace`).
- **Particle tick bench (`crates/tungsten/benches/particle_tick.rs`):** `particle_tick_5k` measures ~657 µs per frame for 5000 live particles with gravity, drag, `scale_over_life`, `color_over_life`, and `alpha_over_life` all active (Ryzen 7 + RADV, release profile).
- **Platformer black-hole emitter:** `examples/01_platformer/assets/particles/black_hole.json` ships a `continuous { rate_hz: 160 }` emitter with premultiplied blend, `0.4–1.1 s` lifetime, `40–140` radial speed, `drag_per_sec = 1.2`, `±6 rad/s` angular velocity, `0.6–1.2` start scale, `scale_over_life` `[0→0, 0.15→1, 1→0]`, purple-to-magenta `color_over_life`, and matching `alpha_over_life`. A new `ex10_spark` 8×8 radial-falloff sprite backs the emitter. `spawn_black_hole_system` attaches `ParticleEmitter + ParticleEmitterState + Transform` to each black hole entity and updates the `Transform` position on every drag frame.
- **Decision records + archived plan:** `DECISIONS.md` gains `D-049` (in-tree PCG32 + SplitMix64), `D-050` (Arc snapshot semantics on hot reload), and `D-051` (entity-per-particle, no pool); `docs/DECISION_INDEX.md` carries the new takeaways; `docs/LLM_INDEX.md` adds a Particles subsystem row and a "Tune or add particle effects" task row; the implementation plan is archived at `docs/plans/archive/phase3-milestone-23-particle-system.md`.

### Changed

- Workspace version bumped to `0.20.0`.
- `README.md`, `AGENTS.md`, `DESIGN.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.20.0` / M23 release line and the next-step `M24` planning state; `DESIGN.md` Non-Commitments drops the stale "Texture atlases / sprite sheet packing" bullet now covered by M22.

## [0.19.0] - 2026-04-20

Summary: Phase 3 Milestone 22 — sprite atlases (shelf-next-fit packer, per-filter pages, half-texel UV inset, renderer-minted texture handles, rebuild-on-growth hot reload), and release-line alignment.

### Added

- **CPU-side atlas packer (`tungsten_core::assets::atlas`):** new module `crates/tungsten-core/src/assets/atlas.rs` ships `UvRect { min, max }` (plus the `UvRect::FULL` constant), `PackInput { id, width, height }`, `PackedSprite { id, page, x, y, width, height }`, `AtlasPage { width, height }`, and `PackResult { pages, sprites }`. `pack_shelf(inputs, max_dim, padding)` sorts a stable copy by `(height desc, width desc, id asc)`, fills shelves inside the current page until either axis overflows `max_dim`, then opens a new power-of-two-sized page. Panics when a single sprite exceeds `max_dim - 2 * padding` on either axis. Unit tests cover empty input, single-sprite origin placement, shared-page packing, two-page overflow, oversize panic, and determinism.
- **Per-filter atlas registry (`tungsten::asset_loader`):** new `AtlasRegistry { nearest_pages, linear_pages, packed: HashMap<String, PackedSprite> }` resource partitions sprites by `FilterMode` and records each sprite's packed rect. `build_atlas_for_filter` packs one filter class, uploads every page through `Renderer::upload_texture`, and registers each sprite with a half-texel-inset `UvRect` so bilinear sampling cannot reach the transparent padding column. `load_sprites` logs `Packed N sprites → M atlas pages (X nearest + Y linear)` for every manifest load.
- **Renderer-minted texture handles (`tungsten_render::sprite`):** `SpritePipeline::allocate_texture_handle()` returns a monotonically increasing `TextureHandle`; `drop_texture(handle)` removes the `GpuTexture` pool entry on rebuild shrink; `upload_texture(handle, bytes, w, h, filter)` now takes the filter up front so the bind group bakes in the sampler and `SpritePipeline::draw` no longer switches samplers per batch. `write_subtexture(queue, handle, rgba, x, y, w, h)` supports in-place sub-region uploads. `Renderer::max_2d_texture_dimension()` exposes the backend's max page dimension clamped to 8192. A `SpriteInstance::whole()` constructor builds an instance with `uv_min = [0.0, 0.0]` / `uv_size = [1.0, 1.0]` for callers that want full-texture behaviour.
- **Per-instance UV slice on the GPU:** `SpriteInstance` gains `uv_min: [f32; 2]` at `@location(6)` and `uv_size: [f32; 2]` at `@location(7)`. `sprite.wgsl` computes `out.tex_coord = instance.inst_uv_min + vertex.uv * instance.inst_uv_size`, so one atlas texture serves many sprites within a single bind group.
- **Hot-reload rebuild-on-growth (`tungsten::asset_loader`):** `reload_sprite` takes the in-place fast path via `write_subtexture` when the new decode is `≤` the packed rect on both axes (leaving `SpriteAsset.uv` untouched); otherwise `rebuild_atlas_for_filter` re-reads every sprite in the affected filter class from disk, repacks, reuses old `TextureHandle`s 1:1, drops excess, and writes the new atlas bindings through `AssetRegistry::update_sprite_entry`. Decode errors anywhere in the rebuild partition abandon the rebuild and keep the previous atlas (last-known-good per `D-031`). Manifest additions run through the same rebuild path with placeholder (`atlas = TextureHandle(0)`, `uv = UvRect::FULL`) entries so the orphan case is bounded.
- **Atlas integration test:** `crates/tungsten/tests/atlas_integration.rs` asserts that two sprites sharing an atlas collapse to a single `SpriteBatch` with distinct `uv_min` slices, and that three sprites across two atlases produce two batches sized `2` and `1` through the default extract path.
- **Atlas pack bench baseline:** `atlas_pack_startup_200` ≈ `7.45 µs` on AMD Radeon 660M / RADV Vulkan — first recorded number on this machine; future runs guard the `≤20%` regression rule from `docs/plans/Phase3.md`.
- **Decision record + archived plan:** `DECISIONS.md` now includes `D-048` covering the six coupled M22 choices (shelf packer, per-filter pages, half-texel inset, rebuild-on-growth hot reload, renderer handle authority, manifest-addition path); the implementation plan is archived at `docs/plans/archive/phase3-milestone-22-sprite-atlases.md`; `docs/DECISION_INDEX.md` and `docs/LLM_INDEX.md` carry the new subsystem/task rows.

### Changed

- Workspace version bumped to `0.19.0`.
- **`AssetRegistry::register_sprite` signature (`tungsten_core::assets::registry`):** now takes `(id, filter, width, height, path, atlas: TextureHandle, uv: UvRect)` — the registry no longer mints handles, and every sprite carries its packed UV slice. `SpriteAsset` gains `atlas: TextureHandle` and `uv: UvRect` alongside the existing `filter / width / height / path`. `update_sprite_entry(id, atlas, uv, width, height)` replaces the M9-era `update_sprite_dimensions` path used by hot reload.
- **Batch key (`tungsten::sprite_extract`):** the default extract now groups by `(asset.atlas.0, asset.filter)` and emits one `SpriteInstance` per entity with `uv_min = asset.uv.min` and `uv_size = asset.uv.max - asset.uv.min`. Sprites that share an atlas page collapse into a single batch; `SpritePipeline::draw` warns and skips when the pool entry's filter disagrees with the batch's filter.
- **`SpriteInstance` size grew from 24 B to 40 B (+66%)** to carry the per-instance UV slice. `sprite_extract_batch_build_2k` measures pre-M22 ≈ `6.32 µs` vs. post-M22 ≈ `7.72 µs` (+22%). The bench pre-allocates 10 fixed batches and does not exercise batch collapse, so the synthetic regression is stride-dominated; the engineered-in wins (fewer bind-group switches, fewer live textures) land in the real-scene draw path.
- Image diff (Pillow per-pixel RGBA, tolerance `0`): `01_platformer`, `02_sprite_stress`, and `03_scene_state` are pixel-identical against the pre-M22 HEAD capture.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.19.0` / M22 release line and the next-step `M23` planning state.

## [0.18.0] - 2026-04-20

Summary: Phase 3 Milestone 21 — debug tooling (geometric overlays, text inspector, screenshot + image-diff), and release-line alignment.

### Added

- **Core debug primitives (`tungsten_core`):** `DebugDraw`, `DebugShape::{Aabb, Circle, Line}`, `DebugCommand`, and `DEFAULT_CIRCLE_SEGMENTS` ship as pure POD in `crates/tungsten-core/src/debug_draw.rs`. `Inspectable` (`crates/tungsten-core/src/inspect.rs`) is a trait with blanket impls for `Tag`, `Transform`, `Visibility`, `Position`, `Velocity`, and `Sprite`. `KeyCode::{F1, F2, F3}` are new variants and round-trip through the input bridge and `key_serde` tables.
- **Engine overlays (`tungsten`):** `PhysicsDebugOverlay` (`F1`), `SystemTimingOverlay` (`F2`, EWMA-smoothed per-system timings sourced from `FrameTimings`), and `InspectorState` (`F3`, LMB pick + registered `Inspectable` row renderers) ship as independent action-toggled resources; `App::register_inspectable::<T: Inspectable>(label)` wires new component types into the inspector.
- **Action-map defaults + `input.json` entries:** `engine_toggle_physics_debug` (`F1`), `engine_toggle_systems_overlay` (`F2`), `engine_toggle_inspector` (`F3`) merge into user input maps via `ActionMap::merged_with_defaults`.
- **Render seam (`tungsten_render`):** new `DebugLinePipeline` + `DebugLineInstance` draws oriented lines and circle polylines and borrows `QuadPipeline`'s camera bind group layout so only one `view_proj` uniform ships on the GPU. `Renderer::render_frame_full[_timed]` gain `debug_quads: &[QuadInstance]` and `debug_lines: &[DebugLineInstance]` parameters; AABB edges expand into four thin `QuadInstance`s drawn through the existing pipeline. `QuadPipeline::camera_bind_group_layout()` / `camera_bind_group()` are now public accessors.
- **Screenshot + visual-regression helpers (`tungsten_render`):** `Renderer::capture_frame(path)` renders into an offscreen `RENDER_ATTACHMENT | COPY_SRC` texture and encodes the readback via `image::save_buffer`; `image_diff::compare_png(lhs, rhs, tolerance)` returns a `DiffReport { width, height, max_delta, mean_delta, pixels_above_tolerance }`. Capture is armed via `TUNGSTEN_CAPTURE_FRAME=<n>` plus optional `TUNGSTEN_CAPTURE_PATH` / `TUNGSTEN_CAPTURE_RESOLUTION=<WxH>` and is off by default. An opt-in integration test (`examples/02_sprite_stress/tests/visual_regression.rs`) gated on `TUNGSTEN_VISUAL_REGRESSION=1` shells out to `example-02-sprite-stress` and diffs against the committed baseline.
- **GPU debug groups + explicit wgpu labels:** the encoder wraps each frame in `push_debug_group("tungsten_frame")`; the main pass opens named groups for `quads`, `sprites`, `debug_quads`, `debug_lines`, and `text`. Always-on, no feature gate; RenderDoc captures are self-describing.
- **Perf-capture scaffolding:** `examples/02_sprite_stress` parses `TUNGSTEN_OVERLAYS_ON=physics,systems,inspector` to flip overlay `.enabled` flags before `App::run`, so the overlays-on vs. overlays-off capture pair is driven purely from the command line. A perf-run skeleton lives at `perf-runs/M21-debug-tooling/README.md`.
- **Decision record + archived plan:** `DECISIONS.md` now includes `D-047`; the implementation plan is archived at `docs/plans/archive/phase3-milestone-21-debug-tooling.md`; `docs/DECISION_INDEX.md` and `docs/LLM_INDEX.md` carry the new subsystem/task rows.

### Changed

- Workspace version bumped to `0.18.0`.
- `App::new` now inserts `DebugDraw`, `PhysicsDebugOverlay`, `SystemTimingOverlay`, and `InspectorState` world resources; engine toggle systems (`__physics_debug_toggle`, `__systems_overlay_toggle`, `__inspector_toggle`, `__inspector_pick`) register at the head of the engine chain so they observe `just_pressed` before user systems. `physics_debug_emit_system` runs at the start of the extract stage before `DebugDraw::drain`, then commands are split into `Vec<QuadInstance>` (AABB edges) + `Vec<DebugLineInstance>` (lines / circle polylines) and passed through to `Renderer::render_frame_full[_timed]` alongside the existing quad / sprite / text channels.
- `example-01-platformer`'s header `Controls:` block documents the new `F1` / `F2` / `F3` overlays.
- `tungsten-render` gains `image = { workspace = true }` as a direct dependency (the workspace dep already existed for `asset_loader`); no new workspace dependency.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.18.0` / M21 release line and the next-step `M22` planning state.

## [0.17.0] - 2026-04-20

Summary: Phase 3 Milestone 20 — scene / state dispatcher, `scene.json` data-driven spawn path, and release-line alignment.

### Added

- **Scene / state system (`tungsten::state`):** `StateStack`, the `GameState` trait, `StateContext`, `StateId`, and a `SceneEntity { state_id }` marker now ship in the umbrella crate. A single engine-owned `state_dispatcher_system` drains deferred `request_push` / `request_pop` / `request_replace` requests each frame, fires the `on_pause` / `on_enter` / `on_exit` / `on_resume` matrix, auto-despawns scene-owned entities through `CommandBuffer` on exit, and mirrors the active state id into `HudActiveState` so the M18 `state` HUD row keeps rendering.
- **Scene data model (`tungsten_core::assets::scene`):** `SceneData`, `SceneEntry`, `SceneTransform`, `SceneSprite`, and `SceneError` define a minimal JSON schema that reuses the M15 `Transform` / `Sprite` / `Visibility` / `Tag` components. `SceneData::load` parses a `scene.json` file; `asset_loader::load_scene` and `asset_loader::spawn_scene` wrap the load + `CommandBuffer` spawn path so scenes land at the canonical frame boundary.
- **State-transition action defaults:** `ActionMap::default_map()` now ships `state_start` (`Enter`), `state_pause` (`KeyP`), and `state_back` (`Backspace`) so examples drive transitions without an edited `input.json`. `KeyCode::Backspace` and `KeyCode::KeyP` are new variants on the core-owned keyboard enum (and route through the input bridge + serde tables).
- **New example — `example-03-scene-state`:** end-to-end demo of the `MainMenu → Gameplay → Pause → Gameplay` flow. Gameplay entities come from `scene.json` via `spawn_scene` (25-entity constellation: pulsing hub + three counter-rotating orbital rings); Pause overlays Gameplay without tearing the scene down; the HUD `state` row mirrors the active state id.
- **Decision record + detailed plan:** `DECISIONS.md` now includes `D-046`; the implementation plan is archived at `docs/plans/archive/phase3-milestone-20-scene-state-system.md`; `docs/DECISION_INDEX.md` and `docs/LLM_INDEX.md` reflect the new subsystem.

### Changed

- Workspace version bumped to `0.17.0`.
- `App::new` now inserts `StateStack` and `HudActiveState` as world resources and registers `__state_dispatcher` immediately after `__display_input` so state transitions fire before user systems observe this frame's input.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, `docs/LLM_INDEX.md`, `docs/DECISION_INDEX.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.17.0` / M20 release line and the next-step `M21` planning state.
- Release QA pass completed locally: `cargo fmt --all --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `bash scripts/test-perf-capture.sh`, and `WGPU_BACKEND=vulkan bash scripts/smoke-examples.sh` all passed (4/4 examples).

## [0.16.0] - 2026-04-19

Summary: Phase 3 Milestone 19 — input mapping, mouse support, runtime rebind persistence, and release-line alignment.

### Added

- **Core action map (`tungsten_core::input`):** `ActionMap`, `Binding`, and `ActionMapError` now ship as the core-owned boolean input binding surface. Actions resolve through keys, mouse buttons, or discrete wheel directions and are re-exported from both `tungsten_core` and `tungsten`.
- **Workspace-root `input.json`:** default bindings now live in a checked-in action-map file with hot reload, missing-file fallback, startup-fatal invalid JSON handling, and a runtime persist path that writes atomically back to disk.
- **Mouse input surface:** `InputState` now exposes current cursor position, per-frame cursor delta, wheel line delta, and wheel pixel delta; extra mouse buttons serialize as `button4`, `button5`, etc.
- **Engine-owned actions:** HUD toggle, vsync toggle, fullscreen toggle, and exit now route through action names (`engine_toggle_hud`, `engine_toggle_vsync`, `engine_toggle_fullscreen`, `engine_exit`) instead of hardcoded key branches.
- **Action-map micro-bench:** `crates/tungsten-core/benches/action_map_bench.rs` now records per-call keyboard and mouse dispatch costs. Current local medians: `action_map_is_pressed_key` ~`51.051 ns`, `action_map_just_pressed_key` ~`34.912 ns`, `action_map_is_pressed_mouse_button` ~`32.267 ns`, `action_map_just_pressed_scroll` ~`35.365 ns`.

### Changed

- Workspace version bumped to `0.16.0`.
- `example-01-platformer` now consumes gameplay input exclusively through action lookups, demonstrates mouse-button bindings (`LMB` jump, `RMB` music toggle, `MMB` stop-all) plus scroll zoom, and renders live cursor / wheel telemetry in the on-screen text.
- `docs/plans/Phase3.md`, `AGENTS.md`, `CLAUDE.md`, `README.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, and `docs/DECISION_INDEX.md` now reflect the shipped M19 release line; the detailed plan moved to `docs/plans/archive/phase3-milestone-19-input-mapping.md`.
- Release QA pass completed locally: `cargo build --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo fmt --all --check`, `./scripts/smoke-examples.sh`, and `cargo bench -p tungsten-core --bench action_map_bench` all passed.

### Fixed

- **Reserved-key drift:** `F4`, `F9`, `F11`, and `Escape` now share the same action-map pipeline as gameplay bindings, removing the last hardcoded key checks from the shipped engine flow.
- **Mouse extra-button coverage:** the input bridge now preserves `winit` back/forward mouse buttons as rebindable extra-button IDs instead of collapsing them into an unusable fallback.
- **Action-map persistence coverage:** runtime rebinds can now round-trip back to `input.json` without discarding unrelated top-level fields when the existing file layout can be safely patched.

## [0.15.0] - 2026-04-18

Summary: Phase 3 Milestone 18 — runtime telemetry HUD, diagnostic counters, and release-line alignment.

### Added

- **Runtime telemetry HUD (`tungsten::debug_hud`):** `DebugHud`, `HudCorner`, `HudRow`, `HudActiveState`, `hud_toggle_system`, and built-in/custom row providers now ship in the umbrella crate. Built-in rows cover FPS/frame ms, camera state, display state, tagged player position/speed, live entity + sprite counts, and top-N slowest systems.
- **Diagnostic counters:** `tungsten::RenderCounts` mirrors per-frame entity and sprite counts into the `World`, while `tungsten_core::World::entity_count()` exposes the live ECS entity count in O(1).
- **HUD toggle + example wiring:** `KeyCode::F4` is now plumbed through the input bridge, `example-01-platformer` tags the player entity for HUD lookup, and the controls text documents the new developer HUD toggle.
- **Decision record + archived plan:** `DECISIONS.md` now includes `D-044`, the detailed M18 rollout plan now lives at `docs/plans/archive/phase3-milestone-18-runtime-telemetry-hud.md`, and the capture summary lives at `perf-runs/M18-hud/README.md`.

### Changed

- Workspace version bumped to `0.15.0`.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.15.0` / M18 release line and the next-step `M19` planning state.
- The shipped HUD defaults now favor readability in busy scenes: larger text, taller line spacing, and a throttled text refresh interval while the EWMA timing row keeps updating from frame telemetry.
- Release QA pass completed locally: `cargo fmt --all`, `cargo build --workspace`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `bash scripts/test-perf-capture.sh`, `WGPU_BACKEND=vulkan ./scripts/perf-capture.sh sprite-stress 300 --telemetry-only`, and `WGPU_BACKEND=vulkan ./scripts/smoke-examples.sh` all passed.

### Fixed

- **Perf-capture README quoting:** `scripts/perf-capture.sh` now escapes the literal `` `STRESS_SCENE` `` / `` `STRESS_COUNT` `` notes in its generated README so shell command substitution cannot corrupt the notes section.
- **Sprite-stress lint noise:** `example-02-sprite-stress` now uses `usize::div_ceil` for row count calculation and gates the `leader` field's dead-code allowance to non-test builds.

## [0.14.0] - 2026-04-17

Summary: Phase 3 Milestone 17 — display state/config, frame-boundary runtime display changes, and release-line alignment.

### Added

- **Display model (`tungsten_core::display`):** `DisplayState`, `DisplayConfig`, `DisplayMode`, `ScaleMode`, `Resolution`, and `DisplayValidationError` now ship as the core-owned display data/validation surface. The checked-in `tungsten.json` now includes a canonical `display` block while legacy `window.*` / `render.*` display inputs remain valid for M17 compatibility.
- **Single runtime display request path:** `tungsten::request_display_settings(&mut World, DisplayState)` validates requests up front, queues one pending change, and lets `App` apply fullscreen, resize, surface-pacing, and frame-cap deltas only at the top of `RedrawRequested`.
- **Display telemetry:** `tungsten::DisplayTelemetry` mirrors authoritative resolution, display mode, vsync intent, lower-case applied present-mode label, max-frame-latency hint, scale mode, and frame-rate cap back into the `World`.
- **Runtime display demo wiring:** `example-01-platformer` now exercises the runtime path directly: `F11` toggles borderless fullscreen and `F9` toggles `vsync` while re-running auto present-mode selection.
- **Decision record + archived plan:** `DECISIONS.md` now includes `D-043` for the single-file display config shape and frame-boundary apply rule, and the detailed M17 rollout plan now lives at `docs/plans/archive/phase3-milestone-17-display-state-config.md`.

### Changed

- Workspace version bumped to `0.14.0`.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, `docs/plans/Phase3.md`, and `docs/perf/profiling-workflow.md` now reflect the shipped `0.14.0` / M17 release line and the `display.*` config surface.
- `example-02-sprite-stress` and `example-03-component-sprites` now express startup sizing through `config.display.resolution` instead of post-load legacy `config.window.*` mutations that are shadowed by the checked-in `display` block.
- `scripts/perf-capture.sh` help text now describes pacing overrides without pointing at superseded pre-M17 config wording.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, and `./scripts/smoke-examples.sh` all passed.

### Fixed

- **Release metadata drift:** top-level docs, planning docs, and changelog entries now agree on branch `0.14`, workspace `0.14.0`, and M17 shipped state.
- **Example display override drift:** sprite-stress and component-sprites no longer rely on legacy startup window overrides that do not win over the resolved `display` block after `Config::load()`.
- **Config error masking:** `example-03-component-sprites` now propagates `Config::load` failures instead of silently falling back to defaults.

## [0.13.0] - 2026-04-17

Summary: Phase 3 Milestone 16 — shared camera module and authoritative camera flow.

### Added

- **Shared camera data model (`tungsten_core::camera`):** `CameraState { position, zoom, rotation }`, `CameraController`, `CameraMode`, and `CameraBounds` centralize camera ownership and follow behavior. The default camera still matches the pre-M10 top-left pixel-ortho matrix at `(0, 0)` / `zoom = 1.0`.
- **Shared camera update system:** `tungsten::camera_update_system` reads `CameraController`, `DeltaTime`, `WindowSize`, and a followed entity `Transform`, then writes the authoritative `CameraState` for the frame.
- **Controller features:** follow/free/scripted modes, dead-zone sizing, smoothing, bounds clamp, zoom multiplier, and deterministic shake fields (`shake_amplitude`, `shake_frequency_hz`, `shake_phase`).
- **Camera test coverage:** `crates/tungsten/tests/camera.rs` covers follow, bounds clamp, scripted zoom scaling, pre-M10 zero-rotation matrix parity, zoom-multiplier changes, and deterministic shake; `tungsten-core::camera` unit tests cover bounds math plus rotated visible-AABB over-coverage.

### Changed

- Workspace version bumped to `0.13.0`.
- `App::new` now inserts `CameraState` and `CameraController` resources by default alongside the existing runtime resources.
- `example-01-platformer` now configures player follow and map-bounds clamp through `CameraController`, recomputes base zoom from window height each frame, and runs `camera_update_system` after `sync_position_to_transform`.
- `extract_tilemaps` now culls through `CameraState::visible_world_aabb(...)`, so tile visibility follows the shared camera state and still over-covers safely when camera rotation is non-zero.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.13.0` / M16 release line.

### Fixed

- **Base-camera stability:** shared camera bookkeeping now avoids compounding `zoom_multiplier` or shake offsets when gameplay rewrites the base camera pose/zoom each frame before `camera_update_system` runs.

## [0.12.0] - 2026-04-16

Summary: Phase 3 Milestone 15 — canonical render components (`Transform`, `Sprite`, `Visibility`, `Tag`) and a default sprite-extract path that removes the need for per-example extract closures in the common case.

### Added

- **Render components (`tungsten_core::components`):** `Transform { position, rotation, scale }`, `Sprite { asset_id, color, z_order }`, `Visibility { visible }`, and `Tag { name }` ship as the baseline gameplay/render component types. Re-exported from `tungsten_core` for convenience.
- **One-way physics sync:** `tungsten_core::sync_position_to_transform` copies physics `Position.0` into `Transform.position` for every entity that carries both. Explicit, opt-in registration; there is no reverse sync (`D-033`).
- **Default sprite extract:** `tungsten::extract_sprites_default` iterates `Transform + Sprite + Visibility`, resolves each sprite against `AssetRegistry`, and builds per-`(texture, filter)` `SpriteBatch`es stably sorted by `z_order`. Installed automatically by `App::run` when no custom sprite extract is set. `Visibility` is required — no implicit fallback (`D-042`).
- **Per-instance rotation + tint on the GPU:** `SpriteInstance` now carries `rotation: f32` (radians, CCW, around the quad centre) and `color: [u8; 4]` (RGBA `Unorm8x4`). The WGSL pipeline rotates around centre and multiplies the sampled texel by the tint.
- **`KeyCode::KeyV`:** added for the new example's `Visibility` toggle demo.
- **Example `examples/03_component_sprites`:** renders rotating, pulsing, tint-cycling, and z-stacked sprites through the default extract path with no `set_extract_sprites` call. `V` toggles visibility on a tagged entity.
- **Bench `sprite_components_query3_2k`:** new ecs_bench entry that regression-tests `query3::<Transform, Sprite, Visibility>` over 2 000 matching entities spread across five archetypes.
- **DECISIONS.md D-042:** records the four coupled M15 choices — component ownership in `tungsten-core`, the one-way physics sync, the `SpriteInstance` layout change, and the `Visibility`-required default extract.

### Changed

- Workspace version bumped to `0.12.0`.
- `SpriteInstance` size grew from 16 bytes to 24 bytes; all in-tree call sites (`tilemap_extract`, `01_platformer`, `02_sprite_stress`, render bench) migrated in the same commit with no backwards-compat shim.
- `sprite.wgsl` now applies centre-origin rotation. When `rotation == 0.0`, `world_pos` reduces algebraically to the pre-M15 top-left-anchored expression so existing sprites render unchanged.
- `FilterMode` derives `Hash` so `(TextureHandle, FilterMode)` can key batch maps.
- `DESIGN.md`, `docs/LLM_INDEX.md`, and `docs/plans/Phase3.md` updated to reference the new component surface and default extract path.
- Release QA pass completed locally: `cargo fmt --all -- --check`, `cargo build --workspace`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`, `bash scripts/test-perf-capture.sh`, `./scripts/smoke-examples.sh`, `cargo bench -p tungsten-core --bench ecs_bench -- sprite_components_query3_2k`, and `cargo bench -p tungsten-render --bench render_bench -- sprite_extract_batch_build_2k` all passed. Current local bench medians: `sprite_components_query3_2k` ~`711 ns`, `sprite_extract_batch_build_2k` ~`5.79 us`.

### Fixed

- `SpritePipeline::draw` now advances its packed instance-buffer cursor even when a batch is skipped for a missing GPU texture, so later batches keep the correct instance slice instead of rendering misaligned sprite data.

## [0.11.0] - 2026-04-16

Summary: Phase 3 Milestone 14 — typed event queues and fixed-frame event flush.

### Added

- **Typed event buffering:** `tungsten_core::EventQueue<T>` adds a reusable two-window event resource with `send`, `iter`, `iter_current`, `flush`, `len`, `is_empty`, and `Default`.
- **App-level event registration:** `App::register_event::<T>()` inserts an `EventQueue<T>` resource and schedules its per-frame flush alongside the existing command-buffer lifecycle.
- **Event-queue benchmark:** `event_queue_flush_10_types` added to the `tungsten-core` ECS Criterion suite; current local result is ~2.44 us for 10 queue types with 100 events each.
- **DECISIONS.md D-040:** Records the two-window event design, frame-boundary flush order, startup-only registration contract, and initial benchmark result.

### Changed

- Workspace version bumped to `0.11.0`.
- `App` frame order is now explicit: run systems, flush command buffers, flush event queues, then hot reload, extract, and render.
- Physics collision signaling migrated from the bespoke `CollisionEvents` resource to `EventQueue<CollisionEvent>`.
- `example-01-platformer` now consumes collision contacts through `EventQueue<CollisionEvent>` for grounded detection and HUD contact counts.
- `README.md`, `DESIGN.md`, `CLAUDE.md`, `AGENTS.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.11.0` release line and Phase 3 M14 completion.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, `./scripts/smoke-examples.sh`, and `cargo bench -p tungsten-core --bench ecs_bench -- event_queue_flush_10_types` all passed.

### Fixed

- **Release metadata drift:** top-level status docs and workspace version metadata now agree on the active `0.11.0` release line instead of mixing `0.10.0` and M14-complete language.

## [0.10.0] - 2026-04-15

Summary: Phase 3 Milestone 13 — command buffers and fixed-frame structural mutation flush.

### Added

- **Deferred ECS mutation path:** `tungsten_core::CommandBuffer` and `PendingEntity` provide queued `spawn`, `despawn`, `insert`, `insert_pending`, and `remove_component` operations without requiring structural mutation during system iteration.
- **`World::flush`:** New two-pass flush API resolves pending spawns first, then replays queued mutations in registration order with dead-entity guards for late inserts/despawns.
- **Flush telemetry:** `tungsten::FrameTimings` now records `flush_ms`, and `App` logs flush timing in `TUNGSTEN_PERF_LOG` output.
- **M13 ECS coverage:** New unit/integration tests cover command buffer queueing, pending-entity resolution, command ordering, dead-entity guards, and empty-buffer no-op behavior.
- **Command-buffer benchmark:** `command_buffer_flush_1k_spawns` added to `tungsten-core` Criterion benches; current local result is ~252 us for 1k spawns plus 2k deferred inserts.
- **Frame-pacing config knobs:** `render.present_mode` and `render.max_frame_latency` are now typed `tungsten.json` fields backed by `PresentModeConfig`.
- **Perf-capture parser regression test:** `scripts/test-perf-capture.sh` exercises metadata parsing plus nearest-rank `p50`/`p95`/`p99` calculations against a synthetic telemetry log.
- **DECISIONS.md D-039:** Records the resource-based command-buffer delivery model, two-pass flush design, and initial benchmark numbers.

### Changed

- Workspace version bumped to `0.10.0`.
- `App` now inserts a fresh `CommandBuffer` resource on startup and drains/replaces it once per frame between system execution and hot reload/extract.
- `tungsten-render` now resolves present mode through explicit precedence rules: concrete `render.present_mode` overrides `window.vsync`, unsupported concrete modes fail fast, and `render.max_frame_latency = 0` is rejected at renderer init.
- `scripts/perf-capture.sh` now records renderer backend/adapter/present-mode metadata as separate README rows and reports post-warm-up `p50`/`p95`/`p99` for total and acquire timing.
- `docs/perf/profiling-workflow.md`, `README.md`, `DESIGN.md`, `CLAUDE.md`, `AGENTS.md`, and `docs/plans/Phase3.md` now reflect the shipped `0.10.0` release line instead of a pre-release state.
- Release QA pass completed locally: `cargo fmt --all`, `cargo test --workspace`, `./scripts/smoke-examples.sh`, `cargo clippy --workspace --all-targets`, `bash scripts/test-perf-capture.sh`, the new `command_buffer_flush_1k_spawns` bench, and steady-state ECS regression benches all passed.

### Fixed

- **Perf metadata wording:** release docs now describe `max_frame_latency` as the requested `wgpu` hint rather than a backend-confirmed effective queue depth.
- **Sprite-stress capture note:** example docs now describe the checked-in default auto no-vsync path without implying that the example hard-overrides `render.present_mode` from `tungsten.json`.

## [0.9.0] - 2026-04-15

Summary: Phase 3 Milestone 12 — performance baseline, telemetry, and profiling harness.

### Added

- **CPU frame telemetry:** `tungsten::FrameTimings` resource now records per-frame stage timings (`update`, `extract`, `render`, `audio`, `hot_reload`, `total`) plus a per-system timing breakdown. The render stage is also split into `render_acquire`, `render_encode`, and `render_submit_present` for finer profiling. `App::add_system_named()` allows stable system labels for diagnostics while preserving existing unnamed-system registration.
- **GPU timing diagnostics:** `tungsten_render::GpuFrameTimings` and `Renderer::render_frame_full_timed()` add an opt-in timestamp-query path for render-pass GPU timing. Backend, adapter, chosen present mode, and max-frame-latency metadata are exposed for downstream tooling and HUD work.
- **Benchmark suite expansion:** `tungsten-core` now ships `physics_bench` alongside the existing ECS benchmarks, and `tungsten-render` now has a Criterion-backed `render_bench` target for CPU-side render-data construction costs.
- **`example-02-sprite-stress`:** Canonical 2k-sprite stress scene for repeatable perf captures. Uses a startup-uploaded placeholder texture, named systems, and periodic telemetry logging.
- **Profiling workflow docs:** `docs/perf/profiling-workflow.md` documents canonical capture rules, backend overrides, manual profiling commands, RenderDoc workflow, and perf budgets.
- **Automated capture script:** `scripts/perf-capture.sh` builds a release binary with frame pointers, captures engine telemetry and GPU timing logs, and integrates optional `cargo flamegraph`, `perf stat`, and `perf record` runs into one timestamped output directory.
- **`perf-runs/.gitkeep`:** Placeholder directory for local machine-specific baseline captures.
- **DECISIONS.md D-037 / D-038:** Render-side Criterion rationale and the inline `Instant`-based telemetry decision are now recorded.

### Changed

- Workspace version bumped to `0.9.0`.
- `README.md`, `DESIGN.md`, `AGENTS.md`, `CLAUDE.md`, and `docs/LLM_INDEX.md` now reflect that Phase 3 M12 is complete and point to the new perf tooling/docs.
- `scripts/perf-capture.sh` bounds flamegraph capture with `TUNGSTEN_SMOKE_FRAMES`, matching the rest of the scripted capture flow.
- Engine defaults now ship with `vsync = false`, and the renderer prefers lower-latency no-vsync present modes plus a 1-frame latency hint when the backend supports them.
- Release QA pass completed locally: `cargo test --workspace`, `cargo clippy --workspace --all-targets`, all three benchmark targets, `./scripts/smoke-examples.sh`, and short release perf sanity runs all passed.

## [0.8.0-alpha] - 2026-04-15

Summary: Phase 2 integration — comprehensive platformer demo, example consolidation, and Phase 3 planning.

### Added

- **`example-01-platformer` (comprehensive demo):** Single example that exercises every Phase 2 engine feature in one scene: ECS, physics (AABB player + bouncing circles + tilemap collision), sprites, walk-cycle animation, audio (one-shot SFX, looping music, volume levels), HUD text, camera follow with zoom (= / −), keyboard input, and hot reload. Supersedes and retires the ten separate milestone examples.
- **`KeyCode::Equal` / `KeyCode::Minus`:** New key code variants to support zoom-in / zoom-out input.
- **`docs/plans/Phase3.md`:** Execution plan for M13–M21: command buffers, event queues, transform/render components, input mapping, scene/state system, sprite atlases, debug tooling, particle system, and tween system.

### Changed

- Workspace version bumped to `0.8.0-alpha`.
- Previous milestone examples (`01_window` through `10_platformer`) removed; their feature coverage is consolidated into `01_platformer`.
- `PHASE2.md` archived to `docs/plans/archive/phase2.md`.

### Fixed

- **First-frame dt spike:** `App` now stamps `last_frame` after the startup callback completes rather than before. Asset-load time no longer registers as game time, preventing fast-moving physics bodies from tunneling through thin geometry on the very first frame.
- **Walk animation frame timing:** `walk_2` frame duration corrected from 1500 ms to 150 ms (copy-paste typo in the original JSON).

## [0.7.0-alpha] - 2026-04-14

Summary: Phase 2 Milestone 12 — Archetypal ECS rewrite.

### Added

- **Archetypal storage engine:** Replaced naive `HashMap<TypeId, HashMap<EntityId, Box<dyn Any>>>` with a proper archetype table. Components of the same type within an archetype are stored in a contiguous `TypedVec<T>` column. Query iteration is now cache-friendly across homogeneous entity sets.
- **Archetype graph:** Lazy-cached add/remove edges between archetypes. First transition builds the edge; subsequent transitions follow the cached pointer in O(1).
- **Generational entity IDs:** Entity handles now carry a generation counter. Stale handles to recycled slots are detected and rejected.
- **Multi-component queries:** `query2` / `query2_entities` / `query3` / `query3_entities` iterate over all archetypes that contain the requested component set, yielding contiguous slices per archetype.
- **Criterion benchmark suite:** Benchmarks on ≥10 000 entities with 3+ component types. Results: ~6× improvement on single-type queries; ~200× on multi-component queries vs. the M2 baseline.
- **DECISIONS.md D-036:** Decision to proceed with the rewrite (cites D-030 "skip if naive suffices"), storage design rationale, and benchmark results.

### Changed

- Workspace version bumped to `0.7.0-alpha`.
- All 10 existing examples compile and smoke-test clean without API changes — the `World` public surface is unchanged.
- PHASE2.md: M12 marked complete.

### Fixed

- **Sound path canonicalization:** `ResolvedManifest::load` now canonicalizes resolved sound asset paths, consistent with sprites, animations, fonts, and tilemaps.
- **Window creation error handling:** `App::resumed` now logs and calls `event_loop.exit()` on window creation failure instead of panicking — consistent with the existing renderer initialization failure path.
- **ECS clippy polish:** `Archetype::move_components_to` uses `entry().or_insert_with()` (avoids double lookup); `split_two_mut` parameter narrowed from `&mut Vec<Archetype>` to `&mut [Archetype]`.
- **Stale doc comment in tilemap extract:** comment updated to reflect that M11 ships as `physics_step` reading collision layers directly.

## [0.6.0-alpha] - 2026-04-14

Summary: Phase 2 Milestone 11 — 2D Physics.

### Added

- **`tungsten-core::physics` module:** Hand-rolled 2D collision subsystem. Exports `Position`, `Velocity`, `Collider`, `RigidBody`, `Shape { Aabb, Circle }`, `BodyKind { Static, Dynamic }`, plus `PhysicsConfig` and `CollisionEvents` resources. No external physics crate — `rapier2d`/`box2d`/`parry2d` all rejected (see D-033).
- **Narrow-phase shape tests:** `aabb_vs_aabb`, `circle_vs_circle`, `aabb_vs_circle` in `physics::collision`. Each returns `Option<Contact { normal, penetration }>` with a consistent convention: `normal` points from `a` into `b`'s free space (the direction `a` should move to escape). MTV on the axis of minimum overlap for AABB, closest-point test for AABB/circle, distance check for circle/circle. No SAT — AABB axes are world-aligned and circles need no SAT; the generalization is documented as a learning note.
- **Uniform-grid broad-phase:** `SpatialGrid` (`HashMap<IVec2, Vec<ProxyId>>`) keyed on `floor(pos / cell_size)`. Cell size is tunable via `PhysicsConfig::broadphase_cell_size` (default 32.0 px). Rebuilt from scratch each physics substep — no incremental state.
- **`physics_step` system:** Registered by the user via `app.add_system(physics_step)`. Per substep: integrate (`position += velocity * dt`, `velocity += gravity * dt`), gather entity proxies + transient tilemap-tile proxies, broad-phase, narrow-phase with MTV resolution split along inverse-mass ratio, velocity impulse `j = -(1+e)·(v·n)/Σ(1/m)`, collision events pushed into `CollisionEvents`. Substep count = `ceil(max_dynamic_speed * dt / min_half_extent)` capped at `PhysicsConfig::max_substeps` (default 8) — guards against tunneling without swept CCD.
- **Tilemap collision layers:** The step walks every `TilemapInstance` and emits one static AABB per non-negative tile on any `LayerKind::Collision` layer, fresh each substep. Hot-reloaded collision layers take effect on the next frame with zero extra machinery. `CollisionEvent.b = None` marks tile contacts.
- **`PhysicsConfig` resource:** `broadphase_cell_size`, `max_substeps`, `gravity` (default `Vec2::ZERO` so top-down games cost nothing). Auto-inserted by `App::new`; games override before `app.run()`.
- **`CollisionEvents` resource:** Per-frame event stream populated each step. Game code reads `events` for ground detection, triggers, damage, etc. `CollisionEvent { a: Entity, b: Option<Entity>, normal, penetration }`.
- **`example-10-platformer`:** Side-scrolling platformer with a player AABB driven by A/D + Space, three bouncing circles at restitution 0.85, gravity override (`Vec2::new(0.0, 900.0)`), a 48×18 tilemap with ground/platforms/walls on a `LayerKind::Collision` layer, grounded detection via `CollisionEvents` scan (`normal.y < -0.5`), and a camera that follows the player horizontally clamped to level bounds. Exercises AABB↔AABB, circle↔circle, AABB↔circle, dynamic↔tilemap-static, event consumption by game code, and non-zero gravity in one scene.
- **DECISIONS.md D-033:** Hand-rolled physics, uniform spatial grid broad-phase, AABB+circle only, library-level `Position`/`Velocity` placement, transient tilemap colliders.

### Changed

- Workspace version bumped to `0.6.0-alpha`.
- `App::new` inserts `PhysicsConfig` and `CollisionEvents` resources alongside the existing resource set.
- `aabb_vs_circle` normal convention fixed to match `aabb_vs_aabb` and `circle_vs_circle` — normal now consistently points from `a` into `b`'s free space across all three helpers.
- PHASE2.md: M11 marked complete.

## [0.5.0-alpha] - 2026-04-13

Summary: Phase 2 Milestone 10 — Tilemaps.

### Added

- **Tilemap data types:** `TilemapData`, `TilemapLayer`, `LayerKind { Render, Collision }`, `TileIndex` (alias for `i32`), and `EMPTY_TILE = -1` sentinel in `tungsten-core`. Custom `.tmj` JSON format (tilemap JSON) with `tile_width`, `tile_height`, `width`, `height`, `tileset: Vec<String>`, and `layers: [{name, kind, tiles}]`. Flat row-major `tiles` array with `-1` as the empty-tile marker; non-empty indices look up into `tileset` (D-010 precedent).
- **`TilemapRegistry` resource:** String-ID → `TilemapData` lookup mirroring `AnimationRegistry`, with path-indexed hot-reload lookup (`insert_with_path`, `id_for_path`, `ids`).
- **`TilemapInstance` component:** Plain-data ECS component (`id: String`, `origin: Vec2`) placed on an entity to draw a tilemap at a world position. Multiple instances are supported.
- **`Camera2D` resource:** World-space `position` (top-left) and `zoom`, with a `view_projection(viewport_w, viewport_h) -> Mat4` method. The default (position zero, zoom 1.0) produces the exact same matrix the sprite pipeline built before M10, so examples 01–08 are pixel-identical.
- **Camera-aware pipelines:** `SpritePipeline::update_camera` and `QuadPipeline::update_camera` now take a view-projection `&Mat4` directly; the ortho is computed by the umbrella crate from the `Camera2D` resource each frame. Text is deliberately *not* transformed by the camera — HUD/UI remains screen-space (glyphon owns its own viewport).
- **Manifest tilemaps section:** `assets/manifest.json` gains a `tilemaps` section, with the same fatal missing-file and duplicate-ID checks as sprites/fonts/sounds/animations. `ManifestError::MissingTilemapFile` added.
- **`extract_tilemaps(&World) -> Vec<SpriteBatch>`:** Free function in the umbrella crate that walks every `TilemapInstance`, computes the visible world-AABB from `Camera2D` + `WindowSize`, clips to the tile grid (this is the culling), and batches tiles per texture handle per layer. Returned in layer order so draw order is preserved. Callers concatenate it with their own sprite extract inside `set_extract_sprites` — flat API, caller controls ordering (behind or in front of entity sprites).
- **Tilemap hot reload:** Editing a `.tmj` file re-parses it and replaces the entry in `TilemapRegistry` live. Tileset sprite IDs are revalidated on every reload; a bad reference logs an error and keeps the stale data rather than crashing. Manifest hot reload handles added/removed tilemap entries the same way it already handles sprites/animations/fonts.
- **`example-09-tilemap`:** 48×30 two-render-layer tilemap (ground + decorations) with a non-rendering `collision` layer (M11 seam, accepted by the loader but skipped by extract). WASD/arrows pan a `Camera2D` at 280 px/sec clamped to map bounds. HUD text stays screen-space while the world scrolls. Edit `assets/tilemaps/demo.tmj` live and changes apply within a frame.
- **DECISIONS.md D-032:** `.tmj` extension picked for hot-reload watcher dispatch, tilemaps reuse sprite pipeline, Camera2D default preserves pre-M10 behavior.

### Changed

- Workspace version bumped to `0.5.0-alpha`.
- `Renderer::render_frame_full` now takes `&Mat4` view-projection as its first parameter.
- `SpritePipeline::update_camera` / `QuadPipeline::update_camera` take `&Mat4` instead of `(width, height)`.
- `App::new` inserts `Camera2D` and `TilemapRegistry` resources alongside the existing asset/animation/font/sound registries.
- PHASE2.md: M10 marked complete.
- CLAUDE.md: status line updated to Phase 2 through M10 complete, branch `0.5`.

## [0.4.0-alpha] - 2026-04-13

Summary: Phase 2 Milestone 9 — Hot Reload.

### Added

- **Hot reload watcher:** `HotReloadWatcher` uses `notify` v6 (`RecommendedWatcher`) to watch the `assets/` directory on a background thread. Events cross to the main thread via `std::sync::mpsc` only — no `Arc<Mutex>`, no async (D-031).
- **50ms debounce:** Events are coalesced per path; a path is only dispatched to the reload handler after no new events have arrived for 50ms. Collapses editor double-writes into a single reload per save.
- **Sprite hot reload:** Editing a PNG re-uploads the decoded RGBA bitmap behind the same `TextureHandle`. If dimensions change the old `wgpu::Texture` is replaced in-place (deferred GPU destruction). No restart needed.
- **Animation hot reload:** Editing an animation JSON file reparses the data and replaces the entry in `AnimationRegistry` live. Running `AnimationState` components pick up the new frame timings on the next advance.
- **Font hot reload:** Editing a TTF/OTF removes the old `fontdb` face IDs, trims the glyph atlas, and re-registers the new bytes — text using that font updates within a few frames.
- **Manifest hot reload:** Adding entries to `assets/manifest.json` while running loads new sprites, animations, and fonts immediately. Removed entries log a warning and stay stale (no crash). Duplicate IDs log an error and are skipped.
- **`App::enable_hot_reload(assets_dir, manifest_path)`:** Opt-in per example. Has no effect if the watcher fails to start (the error is logged and the engine continues without hot reload).
- **`FontRegistry` resource:** New resource in `tungsten-core` tracking path→font ID for hot-reload reverse lookup. Inserted by `load_fonts`.
- **`AnimationRegistry` path index:** Added `insert_with_path`, `id_for_path`, `ids()` to `AnimationRegistry`.
- **`AssetRegistry` path index:** Added `path` field to `SpriteAsset`, `path_to_sprite_id` reverse map, `sprite_id_for_path`, `update_sprite_dimensions`.
- **`example-08-hot-reload`:** Demonstrates all three live asset types — a static sprite, a walk-cycle animation, and an instruction text label. Edit any of the watched files while the example is running; no restart needed.
- **DECISIONS.md D-031:** `notify` v6 rationale under D-015 rule 1.

### Changed

- Workspace version bumped to `0.4.0-alpha`.
- `load_fonts` now takes `world: &mut World` to insert the `FontRegistry` resource.
- `register_sprite` now takes a `path: PathBuf` parameter (stored for hot-reload reverse lookup).
- AGENTS.md, CLAUDE.md, DESIGN.md: status updated to M9 complete, M10 tilemaps next.
- PHASE2.md: M7/M8 condensed; M9 marked complete with all acceptance criteria checked.

## [0.3.0-alpha] - 2026-04-13

Summary: Phase 2 Milestone 8 — Audio.

### Added

- **Audio subsystem:** `cpal` output device init with a hand-rolled mixer running on a dedicated callback thread. Game code writes to `AudioCommands` resource; the audio thread drains it each callback. No async runtime (D-027, D-029).
- **Sound decoding:** `symphonia` decodes OGG/WAV/MP3/AAC files eagerly at startup into `SoundData` (f32 PCM). Linear interpolation resampling and mono→stereo upmix happen at decode time, so the mixer callback stays simple (D-028).
- **Sound manifest section:** `assets/manifest.json` extended with a `sounds` section (`looping`, `volume` fields). Sounds are loaded by string ID — consistent with the sprite/animation/font registry pattern.
- **Audio registry:** `SoundRegistry` resource maps string IDs → `AudioHandle(u32)` and stores manifest-declared default volume and looping per handle (`get_volume()`, `get_looping()`). `AudioHandle` is opaque and cheap to copy.
- **`AudioCommands` resource:** `play()`, `play_looping()`, `play_with()`, `stop()`, `stop_all()`, `set_master_volume()` — synchronous API from any system.
- **`AudioSystem` integration in `App`:** Initialized after the startup callback (so sounds are decoded first). Non-fatal if no audio device is available (logs a warning and continues).
- **`KeyCode` variants:** Added `KeyM`, `Digit1`, `Digit2`, `Digit3` to the engine key enum and input bridge.
- **`exit_on_escape` on `App`:** `set_exit_on_escape(false)` lets game code claim the Escape key for pause menus.
- **`assets/sounds/`:** `sfx_blip.ogg` (short one-shot blip) and `music_main.ogg` (30-second looping tone).
- **`example-07-audio`:** Demonstrates one-shot SFX (Space), looping music toggle (M), master volume levels (1/2/3), and stop-all (S), with live status text using M7 fonts.
- **Asset smoke test** (`crates/tungsten/tests/asset_smoke.rs`): headless integration test that loads the workspace manifest, decodes all animations and sounds, and runs as part of `cargo test --workspace` — catches codec/format bugs before example runtime.
- **DECISIONS.md D-027–D-030:** `cpal`, `symphonia`, hand-rolled mixer, and M12 conditional framing.

### Changed

- Workspace version bumped to `0.3.0-alpha`.
- AGENTS.md: structured AI session workflow (startup checklist, session types, principles checklist); font family directory exception documented.
- DESIGN.md: audio architecture section, resolved Phase 2 gating questions table.
- PHASE2.md: M8 complete, M12 conditional on ECS pain.
- CLAUDE.md: current status updated to M8 complete; font family exception documented.

### Fixed

- **OGG Vorbis playback:** Added `vorbis` feature to the `symphonia` workspace dependency. The `ogg` feature only enables the container demuxer; `vorbis` is the required codec. Without it, any OGG file panicked at runtime with "unsupported codec".
- **Manifest sound defaults ignored:** `SoundRegistry::register()` now accepts `volume` and `looping` and stores them per handle. `load_sounds()` passes the manifest-declared values. Previously the `volume` and `looping` fields in the manifest `sounds` section were parsed but silently dropped, so all sounds played at volume 1.0 regardless of their manifest entries.
- **`example-07-audio` volume mixing:** The example now issues `play_with(handle, manifest_volume, looping)` and relies on `set_master_volume` for global scaling, rather than incorrectly passing the master volume as the per-sound volume.

## [0.2.0-alpha.0] - 2026-04-12

Summary: Phase 2 Milestone 7 — Text rendering.

### Added

- **Text rendering pipeline:** GPU text rendering via `glyphon` (built on `cosmic-text` + `swash`), integrated alongside the existing quad and sprite pipelines in `tungsten-render` (D-026).
- **Font manifest section:** `assets/manifest.json` extended with a `fonts` section. Fonts are loaded by string ID, never by file path — consistent with the sprite/animation registry pattern.
- **Font loading:** TTF/OTF files decoded and registered at startup. Three font families staged in `assets/fonts/`: Inter (sans), Source Serif 4 (serif), JetBrains Mono (mono).
- **Text extraction API:** `ExtractTextFn` added to `App`; `TextSection` type in `tungsten-render` for specifying text content, position, font ID, size, and color. The renderer resolves font IDs at draw time via an internal atlas.
- **`example-06-text`:** Demonstrates multi-font text rendering, labels at fixed positions, and a live FPS overlay using the debug text path.
- **DECISIONS.md D-026:** Rationale for `glyphon`/`cosmic-text` under D-015 rule 2.

## [0.1.0-alpha] - 2026-04-12

Summary: Phase 1 complete (milestones M0 through M6).

### Added

- **Workspace scaffold:** Three-crate Cargo workspace (`tungsten-core`, `tungsten-render`, `tungsten`) with pinned dependencies and `rust-toolchain.toml`.
- **Hand-rolled ECS:** `World` with entity lifecycle, type-erased component storage, singleton resources, and typed queries (`query`, `query_entities`). Panic on programmer error, `Option` on runtime lookups (D-022).
- **wgpu renderer:** GPU initialization, surface management, window resizing, and a clear-color render pass. Shaders embedded via `include_str!` from `.wgsl` files (D-023).
- **Colored-quad pipeline:** Instanced rendering of axis-aligned colored rectangles with an orthographic camera.
- **Textured-sprite pipeline:** Instanced sprite rendering with per-sprite nearest/linear filter modes (D-011), a GPU texture pool keyed by opaque `TextureHandle`s (D-016), and alpha blending.
- **Data-driven config:** `tungsten.json` loaded at startup via `serde_json`, with sensible defaults when the file is missing (D-008).
- **Manifest-driven asset loading:** `assets/manifest.json` registers sprites and animations by string ID. Paths resolve relative to the manifest. Multiple manifests compose by extension with fatal duplicate-ID checks (D-017). Validation catches missing files and unresolved sprite references at load time (D-009).
- **Frame-based animation:** Custom JSON animation format with per-frame sprite IDs and durations (D-010). `AnimationState` component advances frames, supports looping and one-shot playback, and guards against zero-duration infinite loops.
- **Edge-triggered input:** Keyboard and mouse input with `is_pressed`, `just_pressed`, and `just_released` semantics. Engine-specific key/button enums decoupled from `winit` via an input bridge.
- **Frame timing:** `DeltaTime` resource updated each frame.
- **Five examples:** `01_window` (clear screen), `02_ecs` (stdout ECS demo), `03_dots` (bouncing quads with keyboard/mouse input), `04_sprites` (textured sprites from manifest), `05_animation` (looping walk cycle).
- **MIT license.**
