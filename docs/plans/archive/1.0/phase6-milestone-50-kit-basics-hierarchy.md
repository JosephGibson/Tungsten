# Phase 6 milestone 50: kit basics and transform hierarchy (W13a and W13b) — done

- **status:** done
- **goal:** A fourth crate, `tungsten-kit`, on `tungsten-core` alone and re-exported by the umbrella as `tungsten::kit` behind a default `kit` feature. It holds W13a's basics (an animated sprite the default extract draws, lifetimes, despawn outside a region or the view, a sound played on an event, `cursor_to_world`) and W13b's transform hierarchy (`Parent`, `Children`, a local offset, propagation in `post_update` after `physics_sync`, bodies on roots, recursive despawn through the command buffer). Each item has an opt-in plugin or function, rustdoc with an example and a harness test, and `AGENTS.md` gains the kit's "Where code goes" row.
- **non-goals:** W13c and W13d (controllers, health and damage, trigger zones, the path follower, the state machine) and the kit's spawner, which waits for W16b's prefabs; moving example 01 or any other example onto the kit (W12b); rotation or scale inheritance, reparenting and spawning children through the command buffer (no user; Q7); moving `CameraPlugin`, `GameFeelPlugin` or `TweensPlugin` into core (`D-133`'s deferral; Q2); a `Last` stage (`D-128`; Q3); scene components beyond `animation` (Q8); W6c's buses and voice limits for the sound item; fixing the inspector's rotated pick (A14, a follow-up).
- **files to touch:**
  - Step 1: `Cargo.toml`, `Cargo.lock`; `crates/tungsten-kit/Cargo.toml`, `crates/tungsten-kit/src/{lib,animation,cursor,despawn,hierarchy,lifetime,sound}.rs`, `crates/tungsten-kit/src/tests/{animation,cursor,despawn,hierarchy,lifetime,sound}.rs` (all new); `crates/tungsten-core/src/schedule.rs`, `crates/tungsten-core/src/camera.rs`, `crates/tungsten-core/src/tests/camera.rs`; `crates/tungsten/Cargo.toml`, `crates/tungsten/src/lib.rs`, `crates/tungsten/src/plugins.rs`; `scripts/public-api.sh`; `api/tungsten-core.txt`, `api/tungsten.txt`, `api/tungsten-kit.txt` (new); `templates/basic/AGENTS.md`
  - Step 2: `crates/tungsten/src/tests/kit.rs` (new), `crates/tungsten/src/lib.rs`; `scripts/check-repo.py`, `scripts/test-check-repo.py`
  - Step 3: nothing in the tree; evidence under `perf-runs/<YYYYMMDD>-m50-kit/` and a baseline copy under `target/m50-base/`, both outside Git
  - Step 4: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `AGENTS.md`, `docs/LLM_INDEX.md`, `DESIGN.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/known-issues.md`, `docs/plans/1.0/w13-kit.md`, `docs/plans/1.0/roadmap.json`
  - Step 5: `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `DESIGN.md` (the cut rewrites its workspace line: A25), `docs/plans/1.0/implementation-plan.md` (the register row), `docs/plans/1.0/README.md` (the four owned Now fields), this plan (moved to `docs/plans/archive/1.0/`)
- **ordered steps:** (1) The kit's API: the engine's system names in core, `CameraState::screen_to_world`, the crate with every item, the umbrella's `kit` feature, the snapshots and the template's rule; then the API critique. (2) A harness test per item and the resolved order, and the check selector for kit paths. (3) `integrated` and `particles` not regressed; smoke, visual and the physics release tests. (4) Records and routes: decision 155, `AGENTS.md`, the LLM index, `DESIGN.md`, the ledger, known issues, the workstream file and the roadmap summary. (5) Release 0.69.
- **done-when:** every step's evidence row quoted; workflow §5 closed (§8); the release checks pass after the cut, with no gate by change left that no step ran; the preflight prints the command block and the post-merge block.

Candidate W13a and W13b ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M50, release 0.69. Written 2026-10-14 at `5fd7322`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. The workstream has not graduated, so [criteria](../../1.0/criteria.md) §8.4 is its scoping text ([w13-kit.md](../../1.0/w13-kit.md) holds its placement).

## Context digest

W13 is the kit: generic 2D components and systems that the template, the examples and the acceptance game use instead of their own copies ([criteria](../../1.0/criteria.md) §8.4). Its first candidate takes the basics (W13a) and the transform hierarchy (W13b) as one level C candidate, after W3a's `Timer` (`D-134`), W16a's component registry (`D-153`) and W15a's `Schedule` and `Plugin` in core (`D-128`, `D-133`). The owner fixed the dependency direction on 2026-10-03: the kit depends on core and the umbrella re-exports it behind a default feature, so a game keeps one dependency, and the kit's API is inside the 1.0 promise through that re-export (`D-103`). The frame-loop gate set the tiers from the acceptance game's mechanics rows and kept physics bodies on root entities, the hierarchy propagating after `physics_sync` (`D-132`; [acceptance game](../../1.0/acceptance-game.md#proposed-tiers)).

Users: the game's bolts (lifetimes, despawn outside the arena), walk cycles (the animated sprite), hits, kills and pickups (a sound on an event) and aura ring (a child of the player); example 01's click-spawn (`cursor_to_world`, Must only if example 01 moves onto the kit by the freeze) and its emitter and light anchors (the hierarchy), which W12b migrates. The kit writes core components the default extract already reads, so it adds no render type and no extract contribution (`D-007`, `D-016`, `D-018`).

Two seams decide the design. A crate on core alone cannot name the umbrella's `post_update` systems, so the engine's system names move to core, re-exported unchanged (A2). The default extract draws a sprite from its unrotated top-left at `Transform.position` and turns it about its centre (A8), so the hierarchy propagates positions only and leaves rotation and scale to each entity (Q7). Later candidates build on the kit: W16b's spawner, W13c's controllers and W13d's helpers as further kit plugins, W12b's example migration and the freeze, which lists the kit's surface (W4b).

## 1. Audit

Read at `5fd7322` (the 0.68 release, tree clean). Where a finding has a consequence, §2 or §3 names it.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The workspace has three library crates and no kit; `D-006` names three crates and `D-107` three snapshot files | `Cargo.toml:9-21`, `DECISIONS.md:34`, `scripts/public-api.sh:21` |
| A2 | The engine's 17 system names are constants in the umbrella; only `PhysicsPlugin`'s three are core's. A crate on core alone cannot name `particle_count_refresh`, `tween_tick`, `squash_stretch_trigger` or `camera_update`. Their users reach them as `crate::plugins::X` or `tungsten::plugins::X`, which a re-export keeps: `crates/tungsten/src/app.rs:21`, `crates/tungsten/src/tests/plugins.rs`, `examples/01_platformer/src/setup.rs`, `examples/01_platformer/src/tests/ice.rs` | `crates/tungsten/src/plugins.rs:40-73`, `crates/tungsten-core/src/physics/plugin.rs:20-25` |
| A3 | `Stage` is a closed, exhaustive enum, and `PostUpdate`'s doc already lists "hierarchy, animation"; `Stage::ALL` is `[Stage; 5]`, so a `Last` stage (`D-128`'s consequence) would be a break | `crates/tungsten-core/src/schedule.rs:24-42`, `:46` |
| A4 | Frame order: `post_update`, then the command flush, the event flush and the extract. An entity a command spawns lands after `post_update`, so a child spawned through the buffer would draw one frame without propagation | `crates/tungsten/src/app.rs:1512`, `:1515`, `:1516`, `:1522` |
| A5 | `CommandBuffer::call(entity, fn(&mut World, Entity))` runs a plain function at flush in queue order, on live entities only; `despawn` is plain. A recursive despawn needs no ECS change | `crates/tungsten-core/src/ecs/command_buffer.rs:243-248`, `:251` |
| A6 | The state stack's scene cleanup queues plain despawns, so a child without its parent's `SceneEntity` outlives it by a flush | `crates/tungsten/src/state.rs:183-197` |
| A7 | `physics_sync` writes `Transform.position` for every entity with `Transform` and `Position`, interpolated when it has `PrevPosition`; `PhysicsPlugin` registers it in `PostUpdate` | `crates/tungsten-core/src/physics/plugin.rs:52`, `:71-98` |
| A8 | The default extract draws a sprite with its unrotated top-left at `Transform.position` and its size from the asset times `scale`, and the shader turns it about its centre, so turning a child's offset about the parent's `Transform.position` would not follow the parent's drawn shape | `crates/tungsten/src/sprite_extract.rs:445-452`, `:477-480`; `assets/shaders/sprite.wgsl:39-48` |
| A9 | Example 01's copies of kit items: `CurrentSprite` and `animation_system`, which clones `AnimationRegistry` every frame; `cursor_to_world`, which refuses a rotated camera; `black_hole_lifetime_system`; `despawn_out_of_bounds`; `EmitterAnchor` and `anchor_emitters`, ordered after `physics_sync` and before the particle emit. They stay until W12b | `examples/01_platformer/src/state.rs:211`, `systems.rs:223-247`, `:585-595`, `:970-1008`, `:1010-1054`; `gameplay.rs:125-128`, `:605-618` |
| A10 | `AnimationState::advance` returns a sprite only on a frame change; `current_sprite` reads the current frame | `crates/tungsten-core/src/assets/animation.rs:146-199` |
| A11 | `App` inserts no `AnimationRegistry`; the manifest loader builds it, so a harness world has none until a test seeds it | `crates/tungsten/src/asset_loader/mod.rs:94` |
| A12 | Audio is reachable from core: `AudioCommands::play_with`, `SoundRegistry::get_by_id` and `get_volume`; the harness keeps each frame's commands | `crates/tungsten-core/src/audio.rs:53-60`, `crates/tungsten-core/src/assets/audio.rs:196-207`, `crates/tungsten/src/testing.rs:213-216` |
| A13 | Outside the fixed steps a queue reads its previous window, then its current one, and rotates once a frame: a reader after its senders needs `iter_current`, one before them `iter`, unless it counts what it has handled | `crates/tungsten-core/src/ecs/event_queue.rs:76-137` |
| A14 | `view_projection` draws world point `w` at pixel `zoom · R(−rotation)(w − position)`; `visible_world_aabb` inverts it with `R(rotation)`; the inspector's `screen_to_world` turns by `R(−rotation)`. At position (100, 50), zoom 2 and rotation 0.5 rad, (130, 70) draws at (71.83, 6.34), which the inspector maps to (133.04, 35.56) (a scratch computation). Out of scope: a follow-up | `crates/tungsten-core/src/camera.rs:205-234`, `crates/tungsten/src/inspector.rs:265-277` |
| A15 | The cursor arrives in physical pixels, as `WindowSize` and `view_projection`'s viewport are | `crates/tungsten/src/app.rs:2174-2177`, `crates/tungsten-core/src/display.rs:99-104` |
| A16 | `Time::delta()` is the game dt outside the steps; `Timer::once`, `remaining` and `fraction` exist | `crates/tungsten-core/src/time.rs:211`, `:436`, `:509`, `:516` |
| A17 | A plugin registers a scene component with `World::register_component_with`; `D-153` expects the animated sprite to register its own | `crates/tungsten-core/src/component_registry.rs:386-396` |
| A18 | `post_update` readers and writers of an entity's `Transform`: the particle emit reads an emitter's, tweens write its position, rotation and scale, squash its scale, and the camera follows its target's. `DefaultPlugins` resolves `post_update` from `physics_sync` to `camera_update`, with the tween tick after the particle tick, so no single place runs after every position writer and before every position reader (critique 1) | `crates/tungsten/src/particles.rs:142`, `tweens.rs:125-150`, `game_feel.rs:80`, `:147`, `camera.rs:80`, `plugins.rs:178-186`, `tests/plugins.rs:7-14` |
| A19 | The bench builds from an explicit plugin set and keeps its own `animate_sprites`, an owned metric of `particles` (`D-133`); no row adds a kit plugin | `examples/02_bench/src/main.rs:78-87`, `examples/02_bench/src/particles.rs:491-516` |
| A20 | The check selector routes `crates/tungsten`, `-core` and `-render` only, so a kit path prints `UNCLASSIFIED` | `scripts/check-repo.py:507` |
| A21 | The umbrella snapshot prints a crate re-export as one `pub use` line and the plugins' names as 17 `pub const` and 3 `pub use` lines; an item from a private module prints only at its public paths | `api/tungsten.txt:23-25`, `:351-353`; `api/tungsten-core.txt:4667`, `:12153` |
| A22 | Budgets: `docs/LLM_INDEX.md` holds 12,286 of 12,288 B and `AGENTS.md` 5,186 of 6,144 B (`just ctx`); the index's platformer row spends 453 B on eleven paths under one folder | `docs/LLM_INDEX.md:77` |
| A23 | A bundle tuple's elements are components; `Bundle::with` joins two bundles | `crates/tungsten-core/src/ecs/bundle.rs:11-14`, `:38-43` |
| A24 | The roadmap catalog's `w13ab` summary names health and damage with invulnerability time and a hit flash, which are W13d's | `docs/plans/1.0/roadmap.json`, stop `w13ab` |
| A25 | `just release-cut` rewrites the workspace version in `DESIGN.md`'s status line, so the release step touches `DESIGN.md` (critique 3) | `scripts/release.py:57-58`, `:303-305`; `DESIGN.md:5` |
| A26 | The runner writes a suite under its own root's `perf-runs/`, so a baseline tree's captures live inside that tree (critique 4) | `scripts/bench.py:439-445`, `:971` |

## 2. Design

### 2.1 The crate and its seam

- **Crate.** `crates/tungsten-kit`, package `tungsten-kit`, with the workspace's version, edition, `rust-version`, licence and lints. Its dependencies are `tungsten-core`, `glam`, `serde` (derive) and `log`, all already in the graph, so no new third-party package, no `D-015` entry and no change to the notices; `serde_json` is a dev-dependency. It never depends on `tungsten-render` or the umbrella: every need goes through `Schedule`, `World` and core's components and resources (`D-007`, `D-016`).
- **Re-export.** The umbrella takes `tungsten-kit = { workspace = true, optional = true }` with `[features] default = ["kit"]` and `kit = ["dep:tungsten-kit"]`, and `#[cfg(feature = "kit")] pub use tungsten_kit as kit;`. A game reaches every item as `tungsten::kit::…` (`D-103`).
- **Admission rule.** An item joins the kit when it is generic and has a user (the template, a migrated example or the acceptance game), with rustdoc carrying an example and a harness test. An item with no user by the freeze moves to 1.x. Every item here has the game as its user except `cursor_to_world`, whose only user is example 01 (`D-132`).
- **Opt-in plugins.** One plugin per item, unit structs like the engine's, and a game adds the ones it uses (`App::add_plugin`). `DefaultPlugins`, its snapshot and every example's schedule stay as they are (Q5). The kit's constraints on engine names and across its own plugins use the if-present forms, as the engine's cross-plugin couplings do, so leaving a plugin out never fails startup (amends `D-128`'s clause that the kit's constraints are required).
- **Shape of the surface.** The modules are private and the crate root re-exports each item, so each has one public path. `#![deny(missing_docs)]` holds the crate to rustdoc. Value types keep private fields behind constructors, so no settings struct needs `#[non_exhaustive]` (`D-103`); `LocalOffset` is a public newtype component, like `Position`.
- **Names in core.** The umbrella's 17 system-name constants move verbatim, docs included, into an inline `pub mod names` in `crates/tungsten-core/src/schedule.rs`. `tungsten::plugins` re-exports each, so no caller changes (A2). `D-133`'s deferral is answered with names only: the camera, game-feel and tween plugins stay the umbrella's (Q1, Q2).
- **Harness tests in the umbrella.** The harness is the umbrella's, and a kit dev-dependency on the umbrella would be a cycle, so the kit's harness tests live in `crates/tungsten/src/tests/kit.rs` under `cfg(all(test, feature = "kit"))` (Q12). The kit's own unit tests and doctests drive a bare `World` and `Schedule` from core (`Schedule::run_stage` is public).

### 2.2 API sketch

Level C: step 1's API critique reads the snapshots against this sketch.

```rust
// tungsten_core::schedule (core)
pub mod names {
    pub const PHYSICS_DEBUG_TOGGLE: &str = "physics_debug_toggle";
    // … the umbrella's other sixteen, verbatim: SYSTEMS_OVERLAY_TOGGLE, INSPECTOR_TOGGLE,
    // INSPECTOR_PICK, HUD_TOGGLE, UI_DEBUG_TOGGLE, DISPLAY_INPUT, STATE_DISPATCHER, UI_ROUTE,
    // PARTICLE_COUNT_REFRESH, PARTICLE_EMIT, PARTICLE_TICK, TWEEN_TICK, SQUASH_STRETCH_TRIGGER,
    // SQUASH_STRETCH_TICK, SHAKE_TICK, CAMERA_UPDATE
}
// tungsten::plugins: `pub use tungsten_core::schedule::names::{…all seventeen…};`

// tungsten_core::camera (core)
impl CameraState {
    /// The world point drawn at `screen` (physical pixels from the window's
    /// top-left): the inverse of `view_projection`, rotation included.
    #[must_use]
    pub fn screen_to_world(&self, screen: Vec2) -> Vec2;
}

// tungsten_kit (re-exported as tungsten::kit)
pub const ANIMATION_TICK: &str = "animation_tick";
pub const LIFETIME_TICK: &str = "lifetime_tick";
pub const DESPAWN_OUTSIDE: &str = "despawn_outside";
pub const TRANSFORM_PROPAGATE: &str = "transform_propagate";           // after the sync
pub const TRANSFORM_PROPAGATE_LATE: &str = "transform_propagate_late"; // after the tweens

/// Advances every entity with an `AnimationState` and a `Sprite` and writes
/// the current frame into `Sprite.asset_id`; registers the `animation` scene component.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnimationPlugin;

/// Despawns its entity, with its children, once its time has passed on the game clock.
#[derive(Debug, Clone)]
pub struct Lifetime { /* timer: Timer */ }
impl Lifetime {
    /// # Panics: on a negative or non-finite `seconds`, as `Timer::once`.
    pub fn new(seconds: f32) -> Self;
    pub fn timer(&self) -> &Timer;
}
#[derive(Debug, Default, Clone, Copy)]
pub struct LifetimePlugin;

/// Despawns its entity, with its children, once its `Transform.position`
/// leaves a world-space box or the camera's view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DespawnOutside { /* Region { min, max } or View { margin } */ }
impl DespawnOutside {
    /// # Panics: on a non-finite coordinate or `min` above `max` on an axis.
    pub fn region(min: Vec2, max: Vec2) -> Self;
    /// # Panics: on a negative or non-finite `margin`.
    pub fn view(margin: f32) -> Self;
}
#[derive(Debug, Default, Clone, Copy)]
pub struct DespawnOutsidePlugin;

/// Plays a manifest sound for each event `E`, once per event.
pub struct SoundOnEvent<E> { /* pick */ }
impl<E: 'static> SoundOnEvent<E> {
    // `name` names its system, as `system(name, f)` does
    pub fn new(name: &'static str, sound: &'static str) -> Self;             // every event plays `sound`
    pub fn select(name: &'static str, pick: fn(&E) -> Option<&str>) -> Self; // per event; `None` plays nothing
}
impl<E: 'static> Plugin for SoundOnEvent<E> { /* … */ }

/// The world point under the cursor; `None` without a cursor position, an
/// `InputState` or a `CameraState`.
#[must_use]
pub fn cursor_to_world(world: &World) -> Option<Vec2>;

/// The entity this one hangs from; only `spawn_child` sets it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Parent(/* Entity */);
impl Parent { #[must_use] pub fn entity(self) -> Entity; }

/// An entity's children, in the order `spawn_child` added them; the kit keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Children(/* Vec<Entity> */);
impl Children {
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_;
    #[must_use] pub fn len(&self) -> usize;
    #[must_use] pub fn is_empty(&self) -> bool;
}

/// A child's position relative to its parent's `Transform.position`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LocalOffset(pub Vec2);

/// Spawns `bundle` as a child of `parent`, `offset` from it, and returns it.
/// # Panics: when `parent` is not alive or has no `Transform`, or when the
/// bundle holds a `RigidBody` (bodies live on root entities, `D-132`).
pub fn spawn_child<B: Bundle>(world: &mut World, parent: Entity, offset: Vec2, bundle: B) -> Entity;

/// Queues the despawn of `entity` and its descendants for the command flush.
pub fn despawn_recursive(commands: &mut CommandBuffer, entity: Entity);

#[derive(Debug, Default, Clone, Copy)]
pub struct HierarchyPlugin;
```

### 2.3 Schedule placement

Every kit system runs in `PostUpdate`; its constraints on other names are if-present:

| System | Plugin | After | Before |
| --- | --- | --- | --- |
| `transform_propagate` | `HierarchyPlugin` | `PHYSICS_SYNC` | `PARTICLE_COUNT_REFRESH`, `TWEEN_TICK`, `SQUASH_STRETCH_TRIGGER`, `CAMERA_UPDATE` |
| `transform_propagate_late` | `HierarchyPlugin` | `TRANSFORM_PROPAGATE` (required: the plugin's own chain), `TWEEN_TICK` | `CAMERA_UPDATE` |
| `animation_tick` | `AnimationPlugin` | — | — |
| `lifetime_tick` | `LifetimePlugin` | — | — |
| `despawn_outside` | `DespawnOutsidePlugin` | `PHYSICS_SYNC`, `TRANSFORM_PROPAGATE_LATE`, `CAMERA_UPDATE` | — |
| the name the game passes | `SoundOnEvent<E>` | — | — |

With `DefaultPlugins` and then `HierarchyPlugin`, `AnimationPlugin`, `LifetimePlugin`, `DespawnOutsidePlugin` and `SoundOnEvent<E>` added in that order, `post_update` resolves to the line step 2 pins:

```text
post_update: physics_sync, transform_propagate, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, transform_propagate_late, camera_update, animation_tick, lifetime_tick, despawn_outside, <the sound plugin's name>
```

The propagation runs twice, because the engine reads positions before it has finished writing them: the particle emit reads an emitter's position before the tween tick writes positions (A18), and the tween tick cannot move ahead of the particles without changing the engine's order. The first pass, right after the sync, puts children where their bodies are before the emit reads them. The second, after the tween tick and before the camera, puts them where a tweened parent ended, before the camera and the extract read them (Q16). The four `before` names on the first pass and the `before` on the second hold this order in any plugin set: the kit is added after `DefaultPlugins`, and ties go to registration order. A game's `post_update` system that moves a parent runs before `TRANSFORM_PROPAGATE_LATE`, and one that reads a child's position runs after it. The sound plugin's system takes the name the game passes, as every system's does (Q11).

### 2.4 The basics

- **Animated sprite** (`AnimationPlugin`). Each frame `animation_tick` advances every entity with an `AnimationState` and a `Sprite` by `Time::delta()` in milliseconds (the game clock: a pause holds it, a scale stretches it) against the `AnimationRegistry` resource. It then writes the current frame's sprite into `Sprite.asset_id` when the two differ, so the first frame shows from the entity's first frame and a clip the game swaps shows on the next. It takes the registry out of the World and puts it back, as the bench's copy does, never cloning it (A9). Without a registry it does nothing, and an unknown animation ID leaves the sprite alone, as `advance` does (A10, A11). The plugin registers the scene component `animation`: `{ "id": "<animation id>" }`, unknown keys failing, spawns `AnimationState::new(id)` (A17).
- **Lifetime** (`Lifetime`, `LifetimePlugin`). `Lifetime::new(seconds)` holds a `Timer::once`. Each frame `lifetime_tick` ticks every `Lifetime` by `Time::delta()`, and one that has finished queues `despawn_recursive`, so the entity and its children go at this frame's flush, before the extract. A pause holds every lifetime. The criteria's "lifetime and despawn-after timers" are this one component (Q6).
- **Despawn outside** (`DespawnOutside`, `DespawnOutsidePlugin`). A region despawns its entity when `Transform.position` leaves the closed box `min..=max`. A view despawns it when the position lies more than `margin` world units outside `CameraState::visible_world_aabb` at `WindowSize`; without either resource, view entities stay. `despawn_outside` runs after the late propagation pass and the camera, so it judges the frame's drawn point and view, and queues `despawn_recursive`. It tests a point; the margin stands for the entity's extent (Q14).
- **Sound on an event** (`SoundOnEvent<E>`). The plugin registers `EventQueue<E>` (idempotent) and its system under the name the game passes. The system plays each event once, whatever its order against the senders. It remembers how many events the current window held when it last ran; at the next frame it skips that many of the previous window and handles the rest, then the whole current window. So an event sent before it plays in its own frame, one sent after it plays in the next, and none plays twice (A13). It plays through `AudioCommands::play_with(handle, volume, false)` at the manifest volume (`SoundRegistry::get_volume`), never looping. An unknown ID logs one warning per ID and plays nothing, as does a world without `SoundRegistry` or `AudioCommands` (A12). Audio runs on real time (`D-129`), so events sent during a pause play. Two sound plugins need two names, as any two systems do, and each plays once per event; `select` picks a sound per event.
- **`cursor_to_world`.** It maps `InputState::cursor_position()`, in physical pixels (A15), through `CameraState::screen_to_world`, and returns `None` without a cursor position, an `InputState` or a `CameraState`. It does not check `pointer_in_window`, so it maps the last position after the pointer leaves. `screen_to_world` is `position + R(rotation)(screen / max(zoom, ε))`, the inverse that `visible_world_aabb` already uses, so a rotated camera inverts where example 01's copy refuses (A9, A14; Q9).

### 2.5 The hierarchy

- **Positions only.** A child's `Transform.position` is its parent's `Transform.position` plus its `LocalOffset`, down the chain. Rotation and scale stay each entity's own: tweens, squash and game systems write them as on any entity. Turning offsets would not follow the parent's drawn shape (A8), and the channels stay with their writers (A18). Inheritance is a backlog item with a pivot design (Q7).
- **Spawning.** `spawn_child` spawns `bundle.with((Parent, LocalOffset(offset)))` in one archetype move (A23). It sets the child's `Transform.position` to the parent's position plus the offset, keeping the bundle's rotation and scale, and inserts `Transform::from_position` when the bundle has none. It appends the child to the parent's `Children`, inserting `Children` on the first child: one archetype move of the parent, a body included, at the game's own call. The spawn is immediate, so this frame's propagation sees the child before the extract, and no `Last` stage is needed (A3, A4; Q3).
- **Propagation** (`HierarchyPlugin`; two passes, §2.3). Each pass walks from every root (an entity with `Children` and no `Parent`), parents before children, with an explicit stack. It writes each live child's position and never touches rotation or scale. A child without a `LocalOffset` takes a zero offset. A child is live when it is alive and its `Parent` still names the entity whose `Children` list it; the walk skips any other entry. Bodies may be parents: their children follow the interpolated point `physics_sync` writes (A7). The first pass, `transform_propagate`, also does the structural work: it prunes the entries the walk skipped from `Children`, so removing a child's `Parent` detaches it where it stands, despawns orphans and refuses bodies (below). The second pass, `transform_propagate_late`, only writes positions, so a parent a `post_update` system moved after the first pass (a tween, a game system) draws with its children.
- **Orphans.** The first pass despawns, with its subtree, an entity whose parent has died. A parent despawned directly earlier in the frame takes its children in the same frame; one despawned at a flush, through a plain `despawn` or the state stack's `SceneEntity` cleanup (A6), leaves its children for the next frame's first pass, so they draw one frame after it. `despawn_recursive`, or the parent's `SceneEntity` on the child, avoids that frame (Q13).
- **Bodies on roots** (`D-132`). `spawn_child` panics when the bundle holds a `RigidBody`, and the first pass panics on any entity that has both a `Parent` and a `RigidBody`, naming the entity (Q4).
- **Recursive despawn.** `despawn_recursive` queues `CommandBuffer::call(entity, …)` (A5). At flush it despawns the entity and every live descendant reached through `Children`, liveness as the walk defines it, so a child whose `Parent` a command removed earlier in the same flush survives (critique 2). It drops the entity from its parent's `Children`; a dead entity is a no-op. The kit's own despawners queue it, so a child of a timed-out or escaped entity goes with it.
- **The kit's components.** `Parent` and `Children` have no public constructor, and only the kit writes them. A game reads them and moves a child through its `LocalOffset`; a child's `Transform.position` is the propagation's output.

### 2.6 What stays as it is

`DefaultPlugins` and its snapshot (`crates/tungsten/src/tests/plugins.rs:7-14`), the template's and the examples' schedule snapshots, every example's source and the bench's rows (no row adds a kit plugin: A19), `api/tungsten-render.txt`, the extract and the core/render seam (the kit writes `Sprite.asset_id` and `Transform`, which the default extract already reads: `D-018`, `D-042`), and the template's code: only its `AGENTS.md` gains the kit (workflow §5 line 6).

## 3. Steps

### Step 1: The kit's API

- **Files:** `Cargo.toml`, `Cargo.lock`; `crates/tungsten-kit/Cargo.toml`, `crates/tungsten-kit/src/{lib,animation,cursor,despawn,hierarchy,lifetime,sound}.rs`, `crates/tungsten-kit/src/tests/{animation,cursor,despawn,hierarchy,lifetime,sound}.rs`; `crates/tungsten-core/src/schedule.rs`, `crates/tungsten-core/src/camera.rs`, `crates/tungsten-core/src/tests/camera.rs`; `crates/tungsten/Cargo.toml`, `crates/tungsten/src/lib.rs`, `crates/tungsten/src/plugins.rs`; `scripts/public-api.sh`; `api/tungsten-core.txt`, `api/tungsten.txt`, `api/tungsten-kit.txt`; `templates/basic/AGENTS.md`
- **Change:**
  1. Core: move the umbrella's 17 name constants, docs and all, into `pub mod names` in `schedule.rs` (§2.1). Add `CameraState::screen_to_world` with the test `screen_to_world_inverts_view_projection`: at rotation 0 and 0.5 rad and zoom 2, `view_projection` maps `screen_to_world(p)` back to `p` within 1e-3 px.
  2. Umbrella: `plugins.rs` re-exports the 17 from core and defines none; `Cargo.toml` gains the optional dependency and the `kit` and `default` features; `lib.rs` gains `#[cfg(feature = "kit")] pub use tungsten_kit as kit;`.
  3. Workspace: the member and `tungsten-kit = { path = "crates/tungsten-kit" }` under `[workspace.dependencies]`.
  4. The kit (§2.1–§2.5): `lib.rs` with a crate doc (dependency direction, admission rule, stability, the item list) and `#![deny(missing_docs)]`, private modules and the root re-exports. Each item's rustdoc carries an `# Examples` doctest on a bare `World` and `Schedule`, one per item: the animated sprite, `Lifetime`, `DespawnOutside`, `SoundOnEvent`, `cursor_to_world` and the hierarchy. Unit tests in `src/tests/<module>.rs`, mounted with `#[cfg(test)] #[path = "tests/<module>.rs"] mod tests;` as core's are, cover every rule of §2.4 and §2.5 on a hand-driven `World`, flushing the `CommandBuffer` with `World::flush_reusing`. Tuple queries only, no deprecated query form.
  5. Snapshots: `public-api.sh` lists `tungsten-kit` (crates and header comment); `just api`.
  6. Template: `templates/basic/AGENTS.md` gains one rule after the drawing rule: take generic gameplay from `tungsten::kit` before writing it (the six items in a sentence each, physics bodies on root entities), add each item's plugin in `game::register`, and run a `post_update` system that moves a parent before `tungsten::kit::TRANSFORM_PROPAGATE_LATE` and one that reads a child's position after it.
  7. Then the API critique as the skill's level C rule runs it, recorded in §9 as `C.<n>`; a finding that holds changes this step, and renamed items rename the patterns below.
- **Done-when:**
  - `cargo test -p tungsten-kit --locked` → passes; `cargo test -p tungsten-kit --locked --doc` → `test result: ok`, at least 6 passed
  - `cargo test -p tungsten-core --locked camera` → passes, `screen_to_world_inverts_view_projection` among them
  - `cargo test -p tungsten --locked plugins` → passes (`DEFAULT_SCHEDULE` unchanged)
  - `cargo check -p tungsten --no-default-features --all-targets --locked` → builds
  - `rg -c '^pub const ' crates/tungsten/src/plugins.rs` → no match (17 before); `rg -c 'pub const [A-Z_]+: &str' crates/tungsten-core/src/schedule.rs` → 17 (none before)
  - `just api` → writes four files; `just api-check` → `Public API snapshots current: 4 crates`; `git diff --quiet HEAD -- api/tungsten-render.txt` → exit 0
  - `rg -c '^pub const tungsten::plugins::' api/tungsten.txt` → no match (17 before); `rg -c '^pub use tungsten::plugins::[A-Z_]+$' api/tungsten.txt` → 20 (3 before); `rg -c '^pub use tungsten::kit$' api/tungsten.txt` → 1
  - `rg -c '^pub const tungsten_core::schedule::names::[A-Z_]+: &str$' api/tungsten-core.txt` → 17; `rg -c 'CameraState::screen_to_world' api/tungsten-core.txt` → 2 (one line per public path, as `visible_world_aabb` has)
  - `rg -c '^pub const tungsten_kit::[A-Z_]+: &str$' api/tungsten-kit.txt` → 5; `rg -c '^pub fn tungsten_kit::(cursor_to_world|despawn_recursive|spawn_child)\b' api/tungsten-kit.txt` → 3; `rg -c '^pub struct tungsten_kit::(AnimationPlugin|Children|DespawnOutside|DespawnOutsidePlugin|HierarchyPlugin|Lifetime|LifetimePlugin|LocalOffset|Parent|SoundOnEvent)\b' api/tungsten-kit.txt` → 10
  - `rg -c 'tungsten::kit' templates/basic/AGENTS.md` → 1 (none before)
  - `rg -q 'tungsten::kit|tungsten_kit' examples crates/tungsten-render` → exit 1; `git diff --quiet HEAD -- examples crates/tungsten/src/tests/plugins.rs` → exit 0
  - `just lint` → passes; `just deps` → passes; `shellcheck scripts/public-api.sh` → clean
- **Moves:** none

### Step 2: A harness test per item, the resolved order and the selector

- **Files:** `crates/tungsten/src/tests/kit.rs` (new), `crates/tungsten/src/lib.rs`; `scripts/check-repo.py`, `scripts/test-check-repo.py`
- **Change:** mount `#[cfg(all(test, feature = "kit"))] #[path = "tests/kit.rs"] mod kit_tests;` in `lib.rs`. On `Harness` over `App::new(Config::default())`, seeding resources through `world_mut`, add at least these tests:
  1. `kit_plugins_resolve_between_the_sync_and_the_readers`: §2.3's `post_update` line, with the sound plugin's name.
  2. `animated_sprite_shows_the_current_frame_and_holds_while_paused`: frame 0 on the first frame, the next after its duration, none while the clock is paused.
  3. `lifetime_despawns_the_entity_and_its_children_at_its_end`: alive before, gone with a child after, held while paused.
  4. `despawn_outside_takes_what_leaves_the_region_or_the_view`: both forms, the margin, a view entity kept without `WindowSize`.
  5. `sound_on_event_plays_each_event_once_from_any_stage`: an event sent in `update` plays that frame and one sent by a `post_update` system after the reader plays the next, each once, read through `Harness::audio`; a paused clock still plays.
  6. `cursor_to_world_maps_the_harness_cursor_through_the_camera`: after `set_cursor`, with a moved and zoomed camera.
  7. `children_follow_an_interpolated_body_at_their_offsets`: a moving dynamic body, a child and a grandchild, each frame's `Transform.position` equal to the parent's plus the offset.
  8. `children_follow_a_tweened_parent_in_the_frame_it_moves`: a root with a position tween and a child; after each frame the child sits at the root's tweened position plus its offset (critique 1).
  9. `despawn_recursive_takes_the_subtree_at_the_flush`: the parent, child and grandchild gone, a sibling root kept, and a child whose `Parent` a command removed earlier in the same buffer kept (critique 2).
  10. `children_of_a_despawned_parent_go_at_the_next_propagation`: a direct despawn in `update` takes them that frame, a queued `despawn` the next.
  11. `spawn_child_refuses_a_body`, `#[should_panic]`.
  12. `the_propagation_refuses_a_body_on_a_child`, `#[should_panic]`, for a `RigidBody` inserted on a child later.

  In `check-repo.py`'s selector, `crates/tungsten-kit/(src|tests|benches)/` recommends `cargo test -p tungsten-kit --locked`, and `cargo test -p tungsten --locked` for the harness tests, with a test case beside the renderer's in `test-check-repo.py`.
- **Done-when:**
  - `cargo test -p tungsten --locked kit_tests` → passes, at least 12 tests
  - `python3 -B scripts/test-check-repo.py` → OK
  - `python3 -B scripts/check-repo.py --select-checks crates/tungsten-kit/src/hierarchy.rs` → lists `cargo test -p tungsten-kit --locked` and `cargo test -p tungsten --locked`, and prints no `UNCLASSIFIED` line
  - `just check` → passes; `git diff --check` → clean
- **Moves:** none

### Step 3: Perf rows and invariant gates

- **Files:** nothing in the tree. Evidence in `perf-runs/<YYYYMMDD>-m50-kit/` (README, scripts, the base suites copied in, compare logs); the baseline tree in `target/m50-base/`, deleted once its suites are copied out (A26).
- **Change:** the card's rows, `integrated` and `particles`, under the [capture rules](../../../perf/profiling-workflow.md#comparison-rule-and-capture-rules) and workflow §6. No row runs kit code (A19), and no ECS storage, query or frame-loop code changes (the names are constants re-exported unchanged, and no row calls `screen_to_world`), so the card's two rows are the rows this code touches (workflow §5 line 2). The step checks that the umbrella's new dependency and the moved constants leave them alone.
  1. Baseline: `mkdir -p target/m50-base && git archive HEAD | tar -x -C target/m50-base` (the 0.68 tree). Build `example-02-bench` in both trees with the runner's flags (`RUSTFLAGS="-C force-frame-pointers=yes" cargo build --release -p example-02-bench --locked`), twice each, so the second prints `Finished`. Record the offsets mod 64 of `particle_count_refresh_system`, `particle_emit_system`, `particle_tick_system` and the bench's `animate_sprites` in both binaries (`nm -C --defined-only`) for the evidence.
  2. One blocking sitting, nothing else running and no write to the tree: base A, candidate A, candidate B, base B. A base suite is `WGPU_BACKEND=vulkan python3 -B scripts/bench.py suite --only integrated,particles --repeat 5` run from `target/m50-base`, a candidate suite `WGPU_BACKEND=vulkan just perf suite --only integrated,particles --repeat 5` from the repository root. Take each suite's folder from its log.
  3. Copy the two base suites out of `target/m50-base/perf-runs/` into `perf-runs/<YYYYMMDD>-m50-kit/base/` (`cp -a`) and confirm each copy with `diff -rq` (its exit code, since the shell hook rewrites diff output). Then `just perf compare` on base A with base B and candidate A with candidate B (the A/A pairs), and on base A with candidate A and base B with candidate B (the direct pairs), the base sides read from the copies. Delete `target/m50-base` only after the compares.
  4. Then `just smoke`, `just visual` and `just physics-release`, each as one blocking command after the sitting.
- **Done-when:**
  - `diff -rq <base suite> <its copy>` for each base suite → exit 0, before the base tree is deleted
  - Both direct pairs → no owned metric of `integrated` or `particles` `regressed`, and the first-run digests match on both rows
  - Both A/A pairs → no owned metric `regressed` or `improved`. A pair that trips only on `particles` `animate_sprites` p95 or `particle_count_refresh` p50 or p95, the recorded same-build drifts, reruns that row's two suites once, and the rerun decides
  - `just smoke` → passes
  - `just visual` → passes: `gpu-visual.png`, the post-stack, transition and UI overlay references match
  - `just physics-release` → passes: determinism `0x088ec07a73c1b168`, containment `0xaee272e01ffc4e4c` and tunneling unchanged
- **Moves:** none

### Step 4: Records and routes

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `AGENTS.md`, `docs/LLM_INDEX.md`, `DESIGN.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/known-issues.md`, `docs/plans/1.0/w13-kit.md`, `docs/plans/1.0/roadmap.json`
- **Change:** in this order, the decision first ([tungsten-decision](../../../../.claude/skills/tungsten-decision/SKILL.md)):
  1. Decision 155 (§5) with step 3's numbers, and a marker line on each entry it amends (`D-006`, `D-107`, `D-128`, `D-133`). Its index row goes at the end of "ECS / Runtime Flow", and each amended row gains "Amended by" with its note.
  2. `AGENTS.md`: the Project line lists `tungsten-kit` (generic gameplay components and systems over core, re-exported as `tungsten::kit`), and "Where code goes" gains a row: generic gameplay components and systems with a user, under the kit's admission rule → `tungsten-kit`, on core alone.
  3. `docs/LLM_INDEX.md`, inside 12,288 B: one Runtime row, "Kit: animated sprite, lifetimes, despawn outside a region or the view, a sound on an event, `cursor_to_world`, the transform hierarchy" with the decision, routed to `crates/tungsten-kit/src/` (one module per item) and `tungsten/tests/kit.rs`. To make room, the platformer row's eleven paths become the folder `` `examples/01_platformer/src/` `` and the module names in plain text (main, setup, systems, extract, state, gameplay, fireball, ice, burning, brick and death). Measured at `5fd7322`: 318 B saved and about 225 B added, for about 12,193 B.
  4. `DESIGN.md`: the workspace sentence names four crates, and a section "Kit and Transform Hierarchy — M50" follows "Component Registry — M48". It covers the crate and its seam, the admission rule, the names in core, the items' rules, the hierarchy's positions-only propagation and the order of §2.3.
  5. `w04-api-freeze.md`: a ledger row, "The kit (W13a and W13b), for the freeze", recording additions, not breaks: the kit's surface, `tungsten::kit`, `CameraState::screen_to_world` and the names defined in core and re-exported unchanged at `tungsten::plugins`.
  6. `known-issues.md`: under Follow-ups, the inspector's rotated pick (A14), whose fix is a call to `CameraState::screen_to_world`, homed in the Phase 6 QA pass. Under Recorded limits, the kit's: children of a parent despawned at a flush draw one frame after it; positions only, no rotation or scale inheritance; one sound per event at the manifest volume with no voice limit until W6c.
  7. `w13-kit.md`: a revision line (W13a and W13b landed in 0.69, M50, with the decision) and §Follow-ups' rows.
  8. `roadmap.json`: the `w13ab` summary, rewritten without W13d's health and damage (A24): "Animated sprites, lifetimes, despawn outside a region or the view, a sound on an event and the cursor in world space come from tungsten-kit instead of example code; children follow their parents."
- **Done-when:**
  - `rg -c '^## D-15[5] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-15[5]:\*\*' DECISIONS.md` → 4 (the class `[5]` keeps the ID out of the plan's text until the entry exists)
  - ``rg -c '^\| `D-15[5]` \|' docs/DECISION_INDEX.md`` → 1; ``rg -c 'Amended by `D-15[5]`' docs/DECISION_INDEX.md`` → 4
  - `cargo test -p tungsten-core --test decision_index --locked` → passes
  - `rg -c 'tungsten-kit' AGENTS.md` → 2 (none before); `rg -c 'tungsten-kit/src/' docs/LLM_INDEX.md` → 1 (none before)
  - `rg -c '^### Kit and Transform Hierarchy — M50$' DESIGN.md` → 1; `rg -c 'four crates' DESIGN.md` → 1 (none before)
  - `rg -c 'W13a and W13b' docs/plans/1.0/w04-api-freeze.md` → 1 (none before); `rg -c 'screen_to_world' docs/known-issues.md` → at least 1 (none before); `rg -c 'M50' docs/plans/1.0/w13-kit.md` → at least 1 (none before)
  - `jq -e '.stops[] | select(.id == "w13ab") | .summary | test("[Hh]ealth") | not' docs/plans/1.0/roadmap.json` → `true`
  - `just ctx` → OK; `just repo-check` → passes
- **Moves:** none

### Step 5: Release 0.69

- **Files:** `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `DESIGN.md` (the cut rewrites its workspace line, A25; its M50 section landed in step 4), the register row in `docs/plans/1.0/implementation-plan.md`, the README's four owned Now fields (`just release-cut` writes them), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 (§8 below) and this step's evidence row with `status: done`. Then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): archive the plan, run `just release-cut 0.69.0` once, then every release check once after it. No gate by change is left for the release: step 1 ran `just deps`, step 3 `just visual` and `just physics-release`. Then the preflight with `--message`.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.69.0 --repo JosephGibson/Tungsten --message 'Update 0.69: M50 kit basics and transform hierarchy'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next register row's plan prompt, for W16b: `Use the tungsten-milestone skill. Write the milestone plan for W16b: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and docs/plans/1.0/criteria.md §8.7 (W16); audit only the code it touches; name it docs/plans/1.0/phase6-milestone-51-prefabs-tiled-objects.md; critique it last as the skill's step 7 says; end with the run prompt. Don't commit: leave the changes in the tree for the release commit.`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Move all 17 of the umbrella's system names to core, or only the four `post_update` names the propagation orders against? | All 17, in `tungsten_core::schedule::names`, so the engine's vocabulary has one home; `tungsten::plugins` re-exports them, so no path a game uses changes |
| Q2 | `D-133` left moving `CameraPlugin`, `GameFeelPlugin` and `TweensPlugin` into core to W13's plan. Move them? | No: the kit needs their names, not their code |
| Q3 | `D-128` keeps a `Last` stage in reserve for a propagation that must see the frame's deferred spawns. Add it? | No: `spawn_child` is immediate and writes the first position (§2.5); a stage would break the exhaustive `Stage` (A3) |
| Q4 | Refuse a body on a child by panicking, or by logging and treating it as a root? | Panic in `spawn_child` and in the first propagation pass, naming the entity, as `D-022` and `spawn_scene` treat misuse |
| Q5 | One plugin per item, or one `KitPlugin`, or the kit in `DefaultPlugins`? | One per item, opt-in, as the engine's are; `DefaultPlugins` and every snapshot stay unchanged |
| Q6 | The criteria name "lifetime and despawn-after timers": one component or two? | One, `Lifetime`, a despawn-after timer whose `Timer` a game reads for fades |
| Q7 | Inherit rotation and scale (full transform composition), or positions only? | Positions only, in a `LocalOffset`: the default extract pivots a sprite at its centre, not at `Transform.position` (A8), so turned offsets would leave the parent's drawn shape, and tweens and squash keep rotation and scale. Inheritance is a backlog row that comes back with a pivot design and a user. The card's "local transform" is this offset |
| Q8 | Register scene components for `lifetime` and `despawn_outside` too? | No: only `animation`, which `D-153` expects. W16b's bolt prefabs add the others when a prefab needs them |
| Q9 | `cursor_to_world` under a rotated camera: invert or refuse? | Invert, through core's `CameraState::screen_to_world`; the inspector's own copy is a follow-up (A14) |
| Q10 | A per-sound volume or bus on `SoundOnEvent`? | Manifest volume only; W6c's buses and voice limits extend it through new constructors, since its fields are private |
| Q11 | The sound system's name: derived from the event type, or passed by the game? | Passed, in `SoundOnEvent::new(name, sound)` and `select(name, pick)`: `type_name` is not a unique identifier, since two versions of one crate can print the same path (round 2), and a chosen name reads in the systems overlay and the `systems:` line as any system's does |
| Q12 | The kit's harness tests: in the umbrella, or in the kit with a dev-dependency on the umbrella? | The umbrella, under `cfg(all(test, feature = "kit"))`; the other way is a dependency cycle |
| Q13 | Orphans: despawn them with their subtree, or detach them as roots? | Despawn, as Godot and Unity do with a freed parent's children; `despawn_recursive` and a child's `SceneEntity` avoid the extra frame |
| Q14 | `DespawnOutside`: test a point or the entity's extent, and `Transform.position` or `Position`? | `Transform.position` as a point, after the late propagation pass and the camera; the margin covers the extent |
| Q15 | A placement verdict in step 3: which control? | The regression policy's: a padded baseline at the candidate's offsets, captured once; §7 has the path when none can be built |
| Q16 | Propagate once, or twice around the tween tick (critique 1)? | Twice, as §2.3 places the passes. One pass after the tweens would leave a child emitter one frame behind its body, the lag example 01's `anchor_emitters` fixed at M41 (A9); one pass before them would leave a tweened parent's children behind it. The second pass only writes positions, so it costs one more walk of the parented entities |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

155: W13a and W13b landed (M50), next free at `5fd7322` and checked again in step 4. It records:

- `tungsten-kit` as the fourth crate, on core alone and re-exported as `tungsten::kit` behind the default `kit` feature, with no new third-party dependency; the admission rule; opt-in plugins per item; private modules and fields; harness tests in the umbrella.
- The engine's system names in core.
- The items' rules (§2.4) and the hierarchy's (§2.5): positions only, propagated twice around the tween tick, bodies on roots refused, orphans despawned at the next propagation, recursive despawn through the command buffer that spares detached children, no `Last` stage.
- `CameraState::screen_to_world`, and step 3's verdicts.

It amends `D-006` (the crate list), `D-107` (the snapshot files gain `api/tungsten-kit.txt`), `D-128` (the kit's constraints on engine names and across its plugins are if-present) and `D-133` (clause 2's constants are defined in core and re-exported at `tungsten::plugins`), and answers `D-133`'s plugin question and `D-128`'s `Last` stage. `D-103`, `D-132` and `D-153` stand.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash `0x088ec07a73c1b168`, the pinned containment hash `0xaee272e01ffc4e4c`, the row digests (step 3 compares the two rows it captures), `gpu-visual.png`, the post, transition and UI overlay regressions, `DefaultPlugins`' resolved schedule and its snapshot, the template's and the examples' schedule snapshots, and `api/tungsten-render.txt`.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with the steps that don't depend on it. Steps 2–4 depend on step 1, step 3 on step 2, and the release on every step.
- A change is needed in a file outside its step's list: an example, the bench or the template's code among them, since §2.6 expects none. Restore and skip the step with the file named.
- The step 1 API critique: a finding that holds changes step 1 and the patterns it renames; one that would change another step's scope becomes a follow-up. If Codex fails, record the critique as not run with its error line and go on (`D-150`).
- Step 3: an owned metric `regressed` in both direct pairs takes Q15's control once. If a padded baseline reproduces it, record placement and go on. If the padded baseline reads `unchanged`, or no single pad can match the candidate's offsets (as at M49), skip the step, and the release waits for the owner (workflow §3).
- Step 3: a suite the runner marks invalid (the encoder, a foreign build, an edited tree) is set aside and rerun once; a second invalid run skips the step.
- Step 4: `just ctx` over budget after the index trim: drop the three `scripts/test-*` paths from the release row (about 95 B). Still over: skip the row and record it, since a budget raise needs a decision.
- Decision 155 taken by another session: take the next free ID and update every reference. Another session's uncommitted files are never reverted; the release step lists them.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1, 2, 3, release | `just deps` and `just api-check` in 1, `just check` in 2, the card's rows with smoke, visual and the physics release tests in 3; the release step runs `just check`, `just smoke`, `just script-test`, `just template-check` and the rest of the release checks once, after the cut |
| 3 Invariants | 3 | Hashes, digests and references unchanged; nothing moves on purpose |
| 4 Decisions | 4 | 155 with its four marker lines and index rows, before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 4 | Snapshots in 1, rustdoc held by `deny(missing_docs)`; the ledger's "for the freeze" row in 4; no break |
| 6 Template and guide | 1 | `templates/basic/AGENTS.md` gains the kit's rule; the getting-started guide is n/a until W9b starts (Phase 7) |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 2, 4 | The selector in 2; the index row and `AGENTS.md` in 4 |
| 8 Design | 4 | `DESIGN.md`'s M50 section and crate count |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 4, release | Known issues, `w13-kit.md` and the roadmap summary in 4; the backlog gains nothing, since the card's scope ships whole; the gap log has no rows; the register row and the owned fields at the release (`python3 -B scripts/roadmap.py summary` supplies the fields) |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, 2026-10-14, on `5fd7322` with this plan uncommitted, §9 and the evidence log cut by the skill's `sed` filter. Footer: model gpt-6-astra, effort max, context 872000, output cap 4000, 436.2 s, tokens in 1,139,825 (cached 1,026,176), out 13,667 (reasoning 8,476), commands 45, output 389.2 KB (largest 43.4 KB), usage 5h 0% → 28%, week 24% → 29%. Four findings, all accepted. Finding 1 changed a done-when check (the kit's constants, the harness tests), so round 2 runs on the revised plan.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Position tweens move parents after their children have been propagated: `tween_tick` writes `Transform.position` after the propagation, and an after-tween constraint would cycle with before-particles, since tweens run after particles | `crates/tungsten/src/tweens.rs:125-136` (the position channels), `crates/tungsten/src/plugins.rs:178-186` (`TWEEN_TICK` after `PARTICLE_TICK`) | Accepted | A second, positions-only pass `transform_propagate_late` after `TWEEN_TICK` and before `CAMERA_UPDATE` (§2.2, §2.3, §2.5, Q16, A18); the kit's constants 4 → 5 in step 1's check; step 2's test 8 for a tweened parent; the template's rule names the late pass |
| 2 | [MEDIUM] Recursive despawn can delete a child already detached during the same flush: a queued `remove_component::<Parent>` followed by `despawn_recursive` walks a `Children` list only the propagation repairs | `crates/tungsten-core/src/ecs/command_buffer.rs:238-248` (removal and call run in queue order at flush); §2.5 as written checked liveness in the propagation only | Accepted | `despawn_recursive` takes only live children, the propagation's definition (§2.5); step 2's test 9 covers the command order |
| 3 | [MEDIUM] The required release cut violates step 5's file restriction: `release-cut` rewrites `DESIGN.md` | `scripts/release.py:57-58`, `:303-305`; `DESIGN.md:5` | Accepted | `DESIGN.md` in step 5's files and the header (A25) |
| 4 | [MEDIUM] Deleting the baseline tree also deletes its benchmark captures, which the runner writes under that tree's `perf-runs/` | `scripts/bench.py:439-445`, `:971` | Accepted | Step 3 copies the base suites into the evidence folder, checks the copies by `diff -rq` exit code and deletes the tree after the compares (A26) |

Round 2: the same skill and filter on the revised plan, with a `--focus-file` naming the sections round 1 changed (no rejections to carry). Footer: model gpt-6-astra, effort xhigh, context 872000, output cap 4000, 215.0 s, tokens in 585,130 (cached 514,304), out 5,657 (reasoning 3,540), commands 17, output 164.9 KB (largest 26.8 KB), usage 5h 28% → 44%, week 29% → 31%. It found round 1's four changes sound and raised one finding, accepted. No third round runs (the skill's step 7).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] `type_name::<E>()` cannot guarantee distinct sound-system names: Rust does not promise a unique `type_name`, event registration keys on `TypeId` and the schedule rejects a repeated name, so two event types could register and still fail startup | `crates/tungsten-core/src/ecs/world.rs:72-79` (`register_event` keys on `TypeId`), `crates/tungsten-core/src/schedule.rs:290-300` (`resolve` rejects a duplicate name); `std::any::type_name`'s documentation, which says the string must not be taken as a unique identifier | Accepted | `SoundOnEvent::new(name, sound)` and `select(name, pick)` take the system's name, as `system(name, f)` does (§2.2, §2.3, §2.4, Q11, step 2's test 1) |

Step 1's API critique (level C): the `critique` skill with `--raw --repo --force`, 2026-10-14, on the snapshot and template diff with the untracked `api/tungsten-kit.txt`, after `just api`. Footer: model gpt-6-astra, effort xhigh, context 872000, output cap 4000, 120.7 s, tokens in 318,991 (cached 260,224), out 3,143 (reasoning 1,472), commands 15, output 189.6 KB (largest 40.6 KB), usage 5h 44% → 54%, week 31% → 33%. No concerns: it checked the default `kit` feature, the plugins' registration, the camera conversion, the recursive despawn, the hierarchy's order and the sound reader's once-per-event rule against the source, read only, and found the template's rule consistent with them. No `C.<n>` rows; step 1 stands as written.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-14 | Done | `cargo test -p tungsten-kit --locked`: 65 passed, doctests 6 passed (one per item); `cargo test -p tungsten-core --locked camera`: 15 passed with `screen_to_world_inverts_view_projection`; `cargo test -p tungsten --locked plugins`: 11 passed, `DEFAULT_SCHEDULE` unchanged; `cargo check -p tungsten --no-default-features --all-targets --locked` builds. `^pub const ` in `plugins.rs`: no match (17 before); `pub const [A-Z_]+: &str` in `schedule.rs`: 17. `just api` wrote four files; `just api-check`: "Public API snapshots current: 4 crates"; `api/tungsten-render.txt` unchanged (exit 0). Umbrella snapshot: `pub const tungsten::plugins::` no match, `pub use tungsten::plugins::` 20, `pub use tungsten::kit` 1; core: 17 names, `CameraState::screen_to_world` 2; kit: 5 constants, 3 functions, 10 structs. Template `tungsten::kit`: 1 (none before). `tungsten::kit\|tungsten_kit` in `examples` and `crates/tungsten-render`: exit 1; `examples` and `tests/plugins.rs` unchanged: exit 0. `just lint` passes after one `manual_midpoint` rewrite in the camera test; `just deps`: "advisories ok, bans ok, licenses ok, sources ok"; `shellcheck scripts/public-api.sh` clean. API critique: no concerns (§9) | `crates/tungsten-kit/`, `api/tungsten-kit.txt`, `scratchpad/api-critique.md` (outside Git) |
| 2 | 2026-10-14 | Done | `cargo test -p tungsten --locked kit_tests`: 13 passed (the twelve named, plus `the_kit_writes_only_what_the_default_extract_reads`); mutations caught: the late pass dropped fails the tweened-parent test, a liveness check without `Parent` fails the recursive-despawn test (and three kit unit tests), the reader skipping the whole previous window fails the sound test. `python3 -B scripts/test-check-repo.py`: 48 tests OK; `--select-checks crates/tungsten-kit/src/hierarchy.rs` lists `cargo test -p tungsten-kit --locked` and `cargo test -p tungsten --locked` ("the kit's harness tests, which live in the umbrella"), no `UNCLASSIFIED` line. `just check` passes; `git diff --check` clean (exit 0), no trailing whitespace in the new files | `crates/tungsten/src/tests/kit.rs` |
| 3 | 2026-10-14 | Done: owner-accepted drift. The run skipped it (§7: the direct-pair check failed); the owner then accepted `direct-a`'s `particles` `particle_count_refresh` p50 reading (0.620 → 0.654) as the recorded same-build drift, 2026-10-14, and the release ran | Base `git archive 5fd7322` against the candidate, second builds fresh (base sha256 `74eb80ffe9d35d91`, candidate `5412106f5e0f7cc6`, no kit symbol in either); the four `particles` functions moved +112 bytes, 0 → 48, 32 → 16, 16 → 0 and 48 → 32 mod 64. Sitting 06:00:31–06:04:38Z with the encoder gone, four suites valid on the first attempt, tree hash unchanged; base copies `diff -rq` exit 0 and 0. First-run digests match on both rows of every compare. `direct-a`: `particles` `particle_count_refresh` p50 `regressed` 0.620 → 0.654 (Δ +0.034, [+0.002, +0.066], τ 0.020), every other owned metric not `regressed`; `direct-b`: 0 `regressed` (`particle_count_refresh` p50 0.652 → 0.624 `noisy`); `integrated` `total` p50 7.964 → 7.918 and 7.944 → 7.952, p95 8.432 → 8.354 and 8.462 → 8.468, `unchanged`. `base-aa`: 0 `regressed`, 0 `improved` (`particle_count_refresh` p50 0.620 → 0.652 `noisy`); `cand-aa`: `particle_count_refresh` p95 `improved` 0.850 → 0.754, a recorded drift, so the row's two suites reran once: 0 `regressed`, 0 `improved`. The direct-pair check has no rerun clause and §7's control covers a regression in both pairs only, so the run skipped the step and left the release to the owner. Owner ruling (2026-10-14): `direct-a`'s `particle_count_refresh` p50 (0.620 → 0.654) is the recorded same-build drift, as `base-aa`'s 0.620 → 0.652 on one build shows; no metric is accepted as a regression and no invariant moves. Gates after the sitting: `just smoke` passes (5 examples, the template, every fixture matrix); `just visual` passes (`gpu_visual_matches_fixture`, post 5, transition 1, UI overlay 4); `just physics-release` passes (containment 7, determinism 1, tunneling 7; the pinned hashes are asserted by unchanged sources) | `perf-runs/20261014-m50-kit/` (outside Git) |
| 4 | 2026-10-14 | Done | `D-155` free at the run and written first (tungsten-decision). `^## D-15[5] — ` in `DECISIONS.md`: 1; `^**Amended by D-15[5]:**`: 4 (`D-006`, `D-107` after `D-110`'s, `D-128`, `D-133`); the index row: 1; `Amended by \`D-15[5]\`` in the index: 4; `cargo test -p tungsten-core --test decision_index --locked`: 2 passed. `tungsten-kit` in `AGENTS.md`: 2 (0 before; 5,416 of 6,144 B); `tungsten-kit/src/` in `docs/LLM_INDEX.md`: 1 (0 before), the platformer row trimmed to its folder, 12,286 → 12,193 of 12,288 B. `### Kit and Transform Hierarchy — M50` in `DESIGN.md`: 1; `four crates`: 1 (0 before). `W13a and W13b` in the ledger: 1 (0 before); `screen_to_world` in known issues: 1 (0 before); `M50` in `w13-kit.md`: 4 (0 before). The `w13ab` summary's `jq` check: `true`. `just ctx`: "Agent context checks: OK", self-test OK; `just repo-check`: 0 errors (53 notes), release consistency OK, manifests and decision index 5 passed | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `AGENTS.md`, `docs/LLM_INDEX.md`, `DESIGN.md`, `docs/plans/1.0/{w04-api-freeze,w13-kit}.md`, `docs/plans/1.0/roadmap.json`, `docs/known-issues.md` |
| 5 | 2026-10-14 | Done (the close; the cut and its checks follow the archive) | Workflow §5 closed (§8): evidence quoted for steps 1–4, step 3 under the owner's ruling; gates by change all ran in steps 1–3, so the release adds none; invariants unchanged (step 3: digests matched in every pair, `just visual` and `just physics-release` passed, hashes asserted by unchanged sources); `D-155` and its four markers and index rows landed in step 4, its perf paragraph now carrying the owner's ruling; API additions only, the ledger's freeze row in step 4; template rule in step 1; routes and design in step 4; `DESIGN.md`'s status line gains the kit; the register row records the cut; one `CHANGELOG.md` entry; `status: done`. The plan moves to `docs/plans/archive/1.0/` before `just release-cut 0.69.0`, so the release checks and the preflight are quoted in the session's report, not here | `CHANGELOG.md`, `DESIGN.md`, `DECISIONS.md` (`D-155`'s last perf sentence), `docs/plans/1.0/implementation-plan.md`, this plan |

## Follow-ups

- The inspector's rotated pick (A14): step 4 records it in known issues, homed in the Phase 6 QA pass.
- The roadmap catalog's `w13ab` summary (A24): step 4 corrects it.
- Into `w13-kit.md`'s Follow-ups at step 4, each waiting for a user: reparenting and detaching, spawning children through the command buffer, rotation and scale inheritance with a pivot design (Q7), a `KitPlugins` set, scene components for `lifetime` and `despawn_outside` (Q8), and an exactly-once event reader in core if a second reader needs §2.4's cursor.
