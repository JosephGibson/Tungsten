# Phase 6 milestone 62: kit foundations (W13g) — done

- **status:** done
- **goal:** F3 of the [kit catalogue](../../1.0/w13-kit.md): the kit reorganised into domain subfolders behind unchanged root paths, with `prelude` and `KitPlugins`; seven phase anchors every kit system joins; `Tags` and `TagFilter`, `Source`, `Zone`, `ActionInput` with `Intent`, and `CameraTarget`; a scene component for each of the eight data-shaped items; inspector rows, `KitDebugPlugin` and field descriptions snapshotted under `just api-check`; the recipe harness with four genre fixtures, the coverage test and the kit determinism hash; the template's player as a prefab of kit components with no game Rust for movement, health or the camera.
- **non-goals:** Anything the catalogue gives A, M, C or E (stats, cues, the trace, motion items, the kit grid, the damage pipeline, effects, abilities); nearest-name suggestions in load errors (`MissingAsset` has no field for one, M61's follow-up stays); the inspector showing field descriptions and `tungsten check` printing them (W14b); a JSON Schema export (Q8); cursor or nearest-target aim (M's `Aim`); a hotkey or console command for `KitDebugPlugin`; a measured criterion run with a linear bound (W13i's sitting, Q6); reworking the examples (W12b); any core, render or umbrella runtime change beyond one harness helper in `testing.rs`.
- **files to touch:** Step 1: `crates/tungsten-kit/Cargo.toml`, `crates/tungsten-kit/src/lib.rs`, the 14 item modules moved under `crates/tungsten-kit/src/{foundation,intent,motion,lifecycle,overlap,damage,present,transform}/` with their `mod.rs` files, the new `foundation/phases.rs`, `foundation/plugins.rs`, `foundation/tags.rs`, `foundation/source.rs`, `overlap/zone.rs`, `intent/action_input.rs`, `present/camera.rs`, their unit tests under `crates/tungsten-kit/src/tests/`, `crates/tungsten/src/tests/kit.rs`, `crates/tungsten/src/testing.rs`, `templates/basic/src/game.rs`, `templates/basic/src/states.rs`, `templates/basic/assets/prefabs/player.json`, `templates/basic/assets/manifest.json`, `templates/basic/assets/data/tags.json`, `templates/basic/assets/shaders/damage_flash.wgsl`, `templates/basic/tests/game.rs`, `templates/basic/AGENTS.md`, `templates/basic/README.md`, `Cargo.lock`, `api/tungsten-kit.txt`, `api/tungsten.txt`. Step 2: `crates/tungsten-kit/src/foundation/{describe,debug,inspect}.rs`, every item module's rows and description, `crates/tungsten-kit/examples/components.rs`, `scripts/public-api.sh`, `api/tungsten-kit-components.txt`. Step 3: `crates/tungsten/src/lib.rs` (one `mod` line), `crates/tungsten/src/tests/kit_recipes/` (`mod.rs`, `platformer.rs`, `top_down.rs`, `shmup.rs`, `grid.rs`, `coverage.rs`, `determinism.rs`), `crates/tungsten/tests/fixtures/kit/{platformer,top_down,shmup,grid}/`, `crates/tungsten-kit/benches/kit.rs`, `Cargo.lock`. Step 4: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `AGENTS.md` (the kit row's citation), `docs/LLM_INDEX.md`, `docs/getting-started.md`, `docs/known-issues.md`, `docs/perf/profiling-workflow.md` (the criterion list), `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w13-kit.md`. Step 5: `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields, this plan.
- **ordered steps:** (1) The kit's public surface and the template: subfolders, phases, `KitPlugins`, `prelude`, tags, `Source`, `Zone`, `ActionInput`, `CameraTarget`, the eight scene components, the bodiless top-down mover, `Harness::load_manifest`, the template's player prefab; `just api`, then the step 1 API critique. (2) Seeing the kit: inspector rows, `KitDebugPlugin`, field descriptions, their test and the components snapshot in `scripts/public-api.sh`. (3) Recipes: the four genre fixtures, the recipe tests, the moving-platform recipe in reverse plugin order, the coverage test, the kit determinism hash, the criterion scaffold. (4) Records: decision 172, `DESIGN.md`, the index row, the guide, known issues, the ledger row, the workstream file. (5) Release 0.82.
- **done-when:** every step's evidence row quoted; workflow §5 closed; the release checks, plus `just template-check` and `just smoke` if no step ran them, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W13g ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M62, release 0.82. Written 2026-10-16 at `abbcdb6`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

F1 (M60, `D-170`) gave the kit exactly-once event readers and spawn inheritance; F2 (M61, `D-171`) gave it the manifest's `data` section, `DataRef<T>` and a two-phase load that checks every ID. F3 is the last foundation milestone before the catalogue's items (A, M, C, E): everything they need to be data-first, phased, visible and tested lands here, so each later plan adds items and nothing else. The catalogue's contract (w13-kit §2) is the specification: generic, cross-genre, data first, phased, cheap, deterministic, visible, named by convention, documented and tested, saveable, data is API, composed not optioned. Its admission rule changes here from "has a user" to "recipes in two genres" (§4 there), its phases replace `D-158`'s pairwise chain (§2 rule 7), and the field descriptions close P7, the silent DTO break. What A expects from F3: `Tags` and `TagFilter` for cue rules, `Source` for credit, `Intent` for abilities, `KitDescriptions` to add its kinds to, the recipe harness and fixtures to add recipes to, and the determinism hash to keep. The template's growth rule (§8 there) is written into `templates/basic/AGENTS.md` here.

Decisions cited: `D-155` (the kit's crate, admission rule and stage rule), `D-158` (the fixed-step chain), `D-107` (API snapshots), `D-153` and `D-157` (scene components and prefabs), `D-170` and `D-171` (cursors, inheritance, data kinds), `D-150` (no approval stops; the step 1 critique), `D-106` (the index budget), `D-037` (criterion), `D-015` (dependencies).

## 1. Audit

Read at `abbcdb6`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | The kit is 14 flat private modules re-exported from the root; the root docs state the stage rule (`PostUpdate`, except body drivers and windows in `FixedUpdate` before `physics_step`) that the phases replace. 3,470 lines in `src/`, 4,440 with tests. | `crates/tungsten-kit/src/lib.rs:21-36, :68-104` |
| A2 | Within a stage the resolver runs Kahn's algorithm taking the earliest-registered ready system, so ties keep registration order; if-present constraints are looked up at `resolve`, not at `add`, so an anchor's constraint on a neighbour registered later still binds. `Schedule::contains` answers by name. | `crates/tungsten-core/src/schedule.rs:380-388, :420-434, :327` |
| A3 | `PluginSet` has `with`, `without`, `contains`, `names` and `build`; `App::add_plugins(PluginSet)` builds one; `DefaultPlugins::set()` is the pattern `KitPlugins::set()` copies. | `schedule.rs:554-614`; `crates/tungsten/src/app.rs:403-409`; `crates/tungsten/src/plugins.rs:246-262` |
| A4 | Today's chain is pairwise if-present edges: `health_tick` before the state machine, paths and controllers; the state machine before paths and controllers; `path_follow` before the controllers; all before `physics_step`; the trigger zone after it. The only edge that reads another item's output in the same step is `path_follow → controllers` (a rider reads its platform's velocity). `PostUpdate` items: `animation_tick` and `lifetime_tick` unconstrained, `despawn_outside` after the camera, `spawner_tick` after the sync and the late propagation, `hit_flash_tick` after `tween_tick`, the sound plugin unconstrained, the two propagation passes against the engine's names. | `health.rs:400-412`; `state_machine.rs:244-253`; `path.rs:232-237`; `platformer.rs:517-523`; `top_down.rs:154-158`; `trigger.rs:99-103`; `despawn.rs:113-117`; `spawner.rs:137-140`; `animation.rs:75`; `lifetime.rs:78`; `hierarchy.rs:195-213`; `sound.rs:127-128` (all under `crates/tungsten-kit/src/`) |
| A5 | `animation` is the one scene component: `register_component_with(name, \|def, ctx\| bundle)` with a private `#[serde(deny_unknown_fields)]` DTO. `register_component_with_assets` adds a check `Fn(&D, &World) -> Result<(), MissingAsset>`; `SpawnContext` gives `transform()` and `world()`. `MissingAsset` is `#[non_exhaustive]` with `field`, `kind` and `id` and no hint field, so a nearest-name suggestion needs a core change (out of scope). | `crates/tungsten-kit/src/animation.rs:72-84`; `crates/tungsten-core/src/component_registry.rs:430-455, :464-472, :579-609` |
| A6 | Data kinds: `World::register_data_kind::<T>(kind)`, `register_data_kind_with_assets`, `check_prefab_id(field, id)`; `DataRegistry::ids(kind)`, `get::<T>(id)`, `kinds()`. A kind's type may be private to the kit. | `crates/tungsten-core/src/assets/data.rs:195-223, :994-1024` |
| A7 | Spawn inheritance: `World::inherit_from_source::<T>()` clones; `derive_from_source::<T>(fn(&World, Entity) -> Option<T>)` derives, idempotent per type. `Source` is a derive: the source's own `Source` when it has one, else the source itself. | `crates/tungsten-core/src/ecs/world.rs:164-187` |
| A8 | `InspectRegistry::register::<T>(label)` and `register_with(label, fn)`; the App inserts `default_inspect_registry()` and an empty `ComponentRegistry` before the plugins build, so a kit plugin finds the registry in an App and inserts one in a bare World. | `crates/tungsten-core/src/inspect.rs:21-56`; `crates/tungsten/src/app.rs:323-326`; `crates/tungsten/src/inspector.rs:422-429` |
| A9 | `CameraMode::Follow(Entity)` on the `CameraController` resource; the umbrella's camera reads the followed entity's `Transform`, so a bodiless mover's `Position` reaches it through `physics_sync`. | `crates/tungsten-core/src/camera.rs:40-44, :82`; `crates/tungsten/src/camera.rs:77-88`; `app.rs:293` |
| A10 | `ActionMap::is_pressed`, `just_pressed`, `just_released` and `value` take an `&InputState`; `RoutedInput` derefs to one, so `ActionInput` reads the UI's leftovers when the resource exists (`D-149`). | `crates/tungsten-core/src/input/action_map.rs:521-561`; `crates/tungsten-core/src/input.rs:1292-1300` |
| A11 | The `rigid_body` scene component takes `kind: sensor` with an `aabb` or `circle` shape, so a zone prefab needs no new core shape; `SensorEntered { sensor, other: Option<Entity> }`, `None` for a tile. A sensor sees bodies only, so a bodiless mover never enters a zone (a recorded limit until C's grid). | `crates/tungsten-core/src/physics/plugin.rs:107-146`; `crates/tungsten-core/src/physics/events.rs:42-47` |
| A12 | `TopDownMover { speed, direction, filter, started }` with `set_direction`; it needs a kinematic body and skips any other entity with one warning. `PlatformerController` keeps `run`, `jump_held` and `pressed`, set by `set_run`, `press_jump` and `set_jump_held`. `Intent` feeds both when present. | `crates/tungsten-kit/src/top_down.rs:17-26, :85-148`; `platformer.rs:140-165, :290-310` |
| A13 | The DTOs' sources: `Health { current, max, invulnerability, window_left }` (`new`, `with_invulnerability`); `HitFlash::new(color, seconds)`; `PathFollower::new(points, speed)` with `looping`, `ping_pong`, `with_easing` over core's `Easing`, which derives serde in snake case; `Spawner::new(prefab, interval)` with `with_count`; `Lifetime::new(seconds)`; `DespawnOutside::region(min, max)` or `view(margin)`; `PlatformerController::new(run_speed, jump_speed)` with `with_gravity`, `with_coyote_time`, `with_jump_buffer`, `with_jump_cut`. `Health` and `Timer` derive no serde. | `health.rs:66-110, :297-330`; `path.rs:17-20, :72-150`; `spawner.rs:70-100`; `lifetime.rs:43-57`; `despawn.rs:48-105`; `platformer.rs:178-283`; `crates/tungsten-core/src/tween.rs:13-16`; `crates/tungsten-core/src/time.rs:403` |
| A14 | `Harness::new(App)` loads no manifest; the template's tests hand-load prefabs with `asset_loader::load_prefabs`; `load_all_merged` needs the renderer, while animations, sounds, tilemaps, particle configs, UI themes, prefabs and data have headless loaders. | `crates/tungsten/src/testing.rs:75-93`; `templates/basic/tests/game.rs:34-42`; `crates/tungsten/src/asset_loader/mod.rs:61-232, :396-496`; `asset_loader/prefab.rs:31` |
| A15 | The kit's harness tests live in the umbrella's `src/tests/kit.rs` (1,514 lines), wired from `lib.rs`; `testing` is `cfg(any(test, feature = "testing"))`, so recipes live under `src/tests/` too. `DefaultPlugins`' resolved text is pinned in `src/tests/plugins.rs`, the "schedule unchanged" check. | `crates/tungsten/src/lib.rs:29-30, :84-86`; `crates/tungsten/src/tests/kit.rs:90-110`; `crates/tungsten/src/tests/plugins.rs:9-20` |
| A16 | The physics determinism test hashes state with FNV-1a, runs the scenario twice and pins `EXPECTED`; the kit hash copies the shape. | `crates/tungsten-core/tests/physics_determinism.rs:29-43, :71-85` |
| A17 | `scripts/public-api.sh` loops four crates, writes `api/<crate>.txt` and, with `--check`, compares each with `cmp` and prints a diff head; the components snapshot joins that loop's tail. | `scripts/public-api.sh:21-23, :49-70`; `api/` |
| A18 | The template moves the player in `update` by `Transform` and `PLAYER_SPEED` under a `Player` marker; pause writes the autosave and leaves the game clock running; the prefab has `player: null` only; the manifest has no `data`, `shaders` or `materials`; the root manifest's `damage_flash` is a 68-line shader and a material. The tests pin the resolved schedule and expect `PLAYER_SPEED * 30 / 60` exactly. | `templates/basic/src/game.rs:17, :65-88`; `states.rs:76-99, :141-147`; `assets/prefabs/player.json`; `assets/manifest.json`; `assets/manifest.json:87-89, :172-180` (root); `tests/game.rs:76-83, :130-145` |
| A19 | The index's kit row names the items one by one; the file is 12,283 bytes of the 12,288 budget (`D-106`). | `docs/LLM_INDEX.md:14` |
| A20 | The admission rule's citation in the root rules; the template's kit paragraph lists M50's items only. | `AGENTS.md:31`; `templates/basic/AGENTS.md:14` |
| A21 | The guide cites `player_movement` as its system, query and clock example. | `docs/getting-started.md:118-124, :139-143, :180` |
| A22 | The kit depends on core, glam, serde and log, with serde_json for tests; no benches. Core uses criterion 0.8 (`D-037`) and serde_json at runtime. | `crates/tungsten-kit/Cargo.toml`; `crates/tungsten-core/Cargo.toml:21` |
| A23 | The last decision is `D-171`. | `DECISIONS.md:2252` |
| A24 | The workstream file's open follow-ups that this milestone takes: `KitPlugins`, scene components for `lifetime` and `despawn_outside` (M50), scene components for the controllers and helpers (M52), the index budget (M61). | `docs/plans/1.0/w13-kit.md:470-491` |

## 2. Design

The API sketch. Level C: the step 1 diff of `api/` and `templates/` is critiqued against it before steps 2 to 5 run. Names follow w13-kit §2 rule 12. Every value type keeps private fields behind constructors (`D-155`); a type that is data derives `Serialize` and `Deserialize` (rule 14).

### 2.1 Layout

Private domain modules; the root re-exports every item at the same path as today, so `api/tungsten-kit.txt` changes by additions only.

```text
crates/tungsten-kit/src/
  lib.rs            the catalogue by domain; `mod` lines; the root `pub use` lines, unchanged plus the new items; `pub mod prelude`
  foundation/       phases.rs, plugins.rs (KitPlugins), tags.rs, source.rs, describe.rs (step 2), debug.rs (step 2), inspect.rs (step 2)
  intent/           action_input.rs, state_machine.rs, cursor.rs
  motion/           platformer.rs, top_down.rs, path.rs, slide.rs
  lifecycle/        lifetime.rs, despawn.rs, spawner.rs
  overlap/          trigger.rs, zone.rs
  damage/           health.rs
  present/          animation.rs, camera.rs, sound.rs
  transform/        hierarchy.rs
  tests/            unchanged files; each module's `#[path]` line gains `../`
```

`prelude` re-exports the plugins, components, events, `Phase`, `KitPlugins`, `damage`, `cursor_to_world`, `spawn_child` and `despawn_recursive`, not the system-name constants. A, M, C and E add modules under these folders (`react/` for cues joins `present/sound.rs`'s domain when A lands; C's grid goes under `overlap/`).

### 2.2 Phases (`foundation/phases.rs`)

```rust
pub const KIT_TICK: &str = "kit_tick";       // FixedUpdate, after physics_prev_snapshot (if present)
pub const KIT_INTENT: &str = "kit_intent";
pub const KIT_MOVE: &str = "kit_move";       // before physics_step (if present)
pub const KIT_OVERLAP: &str = "kit_overlap"; // after physics_step (if present)
pub const KIT_RESOLVE: &str = "kit_resolve";
pub const KIT_REACT: &str = "kit_react";
pub const KIT_PRESENT: &str = "kit_present"; // PostUpdate, after physics_sync, transform_propagate_late and tween_tick (each if present), before camera_update (if present)

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Phase { Tick, Intent, Move, Overlap, Resolve, React, Present }
impl Phase {
    pub const ALL: [Phase; 7];
    pub fn name(self) -> &'static str;
    pub fn stage(self) -> Stage;
    /// `desc` ordered inside this phase: after its anchor and before the next anchor of its stage; a `Move` member is also before `physics_step` and a `Present` member before `camera_update`, each when registered, since the next anchor alone would leave the engine's earlier-registered system ahead of the member (A2).
    pub fn join(self, desc: SystemDesc) -> SystemDesc;
}
/// Adds every anchor `Schedule::contains` does not find, each a no-op system with the constraints above and ordered after every earlier anchor of its stage; every kit plugin calls it first, and a game's feature plugin that joins a phase calls it too.
pub fn ensure_phases(schedule: &mut Schedule);
```

Every kit system joins one phase: `schedule.add(phase.stage(), phase.join(system(NAME, run)))`. Inside a phase, registration order (A2), so the one within-phase edge stays explicit: `path_follow` keeps `before_if_present(PLATFORMER_CONTROLLER)` and `before_if_present(TOP_DOWN_MOVER)`, and the controllers `after_if_present(PATH_FOLLOW)`. The other pairwise edges of A4 go, since the anchors order them. `kit_overlap` sits after `physics_step`, so `join` gives a `Move` member its own `before_if_present(PHYSICS_STEP)`: the resolver takes the earliest-registered ready system (A2), and `physics_step`, registered by the engine first, would otherwise run ahead of a mover whose only bound is the next anchor.

| System | Phase | Note |
| --- | --- | --- |
| `health_tick`, `lifetime_tick` | `kit_tick` | the lifetime moves from `PostUpdate` to the fixed step (Q1) |
| `action_input`, a state machine's system | `kit_intent` | |
| `path_follow`, `platformer_controller`, `top_down_mover` | `kit_move` | |
| `despawn_outside`, `zone_tick`, a trigger zone's system | `kit_overlap` | `despawn_outside` reads `Position` when present, else `Transform.position`, and the last frame's camera view |
| `spawner_tick` | `kit_react` | reads `Position` when present, else `Transform.position`; a child spawner (`spawn_child` gives a `Transform` only, `hierarchy.rs:102-106`) spawns at its last propagated point, one frame behind a moving parent: a recorded limit (step 4), pinned by a harness test (step 1) |
| `animation_tick`, `hit_flash_tick`, a sound plugin's system, `camera_target`, `kit_debug_draw` | `kit_present` | |
| `transform_propagate`, `transform_propagate_late` | none | the hierarchy's own places against the engine's names stand (`D-155`) |

With `DefaultPlugins` then `KitPlugins::set()`, the stages resolve to (the DESIGN section records the final text):

```text
fixed_update: physics_prev_snapshot, kit_tick, lifetime_tick, health_tick, kit_intent, action_input, kit_move, path_follow, platformer_controller, top_down_mover, physics_step, kit_overlap, despawn_outside, zone_tick, kit_resolve, kit_react, spawner_tick
post_update: physics_sync, transform_propagate, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, transform_propagate_late, kit_present, animation_tick, hit_flash_tick, camera_target, kit_debug_draw, camera_update
```

### 2.3 `KitPlugins` (`foundation/plugins.rs`)

```rust
pub struct KitPlugins;
impl KitPlugins {
    /// Every kit plugin that takes no game type, in the kit's order: TagsPlugin, SourcePlugin, HierarchyPlugin, AnimationPlugin, LifetimePlugin, DespawnOutsidePlugin, SpawnerPlugin, HealthPlugin, PathFollowerPlugin, PlatformerControllerPlugin, TopDownMoverPlugin, ZonePlugin, ActionInputPlugin, CameraTargetPlugin, KitDebugPlugin.
    pub fn set() -> PluginSet;
}
```

`app.add_plugins(KitPlugins::set())`, or `.without::<KitDebugPlugin>()`. `SoundOnEvent<E>`, `TriggerZone<Z, T>` and `StateMachinePlugin<S>` take a game type, so a game adds them itself.

### 2.4 Tags (`foundation/tags.rs`)

```rust
/// Scene name `tags`: `"tags": ["team.player", "flying"]`. Names are `[a-z][a-z0-9_]*` joined by dots; sorted, deduplicated.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tags { names: Vec<String> }
impl Tags {
    pub fn new(names: impl IntoIterator<Item = impl Into<String>>) -> Self;   // panics on an invalid name
    pub fn has(&self, tag: &str) -> bool;    // exact, or a dotted ancestor: `has("team")` holds for `team.enemy`
    pub fn iter(&self) -> impl Iterator<Item = &str>;
    pub fn insert(&mut self, tag: impl Into<String>) -> bool;
    pub fn remove(&mut self, tag: &str) -> bool;
    pub fn is_empty(&self) -> bool;
}
/// Data form `{ "all": [..], "any": [..], "none": [..] }`, each optional; an empty filter matches everything, and an entity without `Tags` matches a filter with no `all` and no `any`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagFilter { all: Vec<String>, any: Vec<String>, none: Vec<String> }
impl TagFilter {
    pub fn new() -> Self;
    pub fn all(self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self;
    pub fn any(self, ..) -> Self;
    pub fn none(self, ..) -> Self;
    pub fn matches(&self, tags: Option<&Tags>) -> bool;
    pub fn is_everything(&self) -> bool;
}
/// Declares tags at build; the `tags` data kind declares the rest.
pub struct TagsPlugin { declared: Vec<String> }
impl TagsPlugin { pub fn new() -> Self; pub fn with_tags(self, names: impl IntoIterator<Item = impl Into<String>>) -> Self; }
```

`TagsPlugin::build`: `ensure_phases`; a `TagDeclarations` resource with the build-time names; `register_data_kind::<TagList>("tags")`, a `tags` entry being a JSON array of names; `register_component_with_assets("tags", check, build)`, where the check returns `MissingAsset::new(index, "tag", name)` for a name that neither the declarations nor any `tags` entry in the `DataRegistry` declares (a declared name declares its dotted ancestors), and the build gives `(Tags,)`. A private `tags::check_declared(world, field, name)` serves `zone`'s filter and later milestones' filters. Storage is sorted strings (Q3).

### 2.5 `Source` (`foundation/source.rs`)

```rust
/// The origin of the entity's spawn chain, derived at the flush from its source (`D-170`): the source's own `Source`, else the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Source(Entity);
impl Source { pub fn new(entity: Entity) -> Self; pub fn entity(self) -> Entity; }
pub struct SourcePlugin;   // build: ensure_phases; world.derive_from_source::<Source>(..); inspector row
```

No scene form: a spawn derives it, and a game sets one by hand with `new`.

### 2.6 `Zone` (`overlap/zone.rs`)

```rust
/// Scene name `zone`, on a sensor body: `"zone": { "filter": { "all": ["team.player"] } }`; `filter` defaults to everything.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Zone { filter: TagFilter }
impl Zone { pub fn new(filter: TagFilter) -> Self; pub fn filter(&self) -> &TagFilter; }
/// An entity whose `Tags` pass the zone's filter began to overlap it this step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneEntered { pub zone: Entity, pub entity: Entity, pub position: Vec2 }
pub const ZONE_TICK: &str = "zone_tick";
pub struct ZonePlugin;   // kit_overlap: reads SensorEntered through an EventCursor; `other: Some(e)` with matching Tags sends ZoneEntered with e's Position; a tile never matches
```

The zone's load check runs `check_declared` over the filter's names.

### 2.7 `ActionInput` and `Intent` (`intent/action_input.rs`)

```rust
/// Scene name `action_input`: `{ "move": ["move_left", "move_right"] or four actions, "aim": [left, right, up, down] (optional), "slots": { "jump": "jump" } (optional) }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionInput { move_axes: Vec<String>, aim_axes: Option<[String; 4]>, slots: Vec<(String, String)> }
impl ActionInput {
    pub fn new(left: impl Into<String>, right: impl Into<String>) -> Self;
    pub fn with_vertical(self, up, down) -> Self;
    pub fn with_aim(self, axes: [impl Into<String>; 4]) -> Self;
    pub fn with_slot(self, slot: impl Into<String>, action: impl Into<String>) -> Self;
}
/// What the entity means to do this step: written by `action_input`, or by an AI item (M), and read by the movers when present.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Intent { movement: Vec2, aim: Option<Vec2>, slots: Vec<Slot> }   // Slot { name, held, pressed }
impl Intent {
    pub fn movement(&self) -> Vec2;  pub fn set_movement(&mut self, movement: Vec2);
    pub fn aim(&self) -> Option<Vec2>;  pub fn set_aim(&mut self, aim: Option<Vec2>);
    pub fn held(&self, slot: &str) -> bool;  pub fn pressed(&self, slot: &str) -> bool;
    pub fn set_slot(&mut self, slot: impl Into<String>, held: bool, pressed: bool);
}
pub const ACTION_INPUT: &str = "action_input";
pub struct ActionInputPlugin;
```

`action_input` in `kit_intent`: for every `(&ActionInput, &mut Intent)`, movement is `(value(right) − value(left), value(down) − value(up))` clamped to length 1 (`y` is 0 with two axes), aim likewise or `None`, each slot's `held` is `is_pressed` and `pressed` is `just_pressed`, read from `RoutedInput` when the resource exists and from `InputState` otherwise, through `ActionMap`; without the map nothing is written. The scene build gives `(ActionInput, Intent::default())`. The movers: `TopDownMover` takes `intent.movement()` as its direction when the entity has an `Intent`, else its own `direction`; `PlatformerController` takes `movement().x` as the run, `held("jump")` as the held jump and `pressed("jump")` as a press, else its own fields. `set_direction`, `set_run`, `press_jump` and `set_jump_held` stand for a game without `Intent`. The load check names every `move`, `aim` and slot action against the `ActionMap` resource's declared actions (`ActionMap::iter`), failing with `MissingAsset::new(field, "action", name)` (`move.1`, `slots.jump`): an unknown action reads as unbound (`action_map.rs:498-500`), so a typo would load and move nothing; a World without the map passes the check.

### 2.8 `CameraTarget` (`present/camera.rs`)

```rust
/// Scene name `camera_target`: `"camera_target": null`. While an entity carries it, the camera follows the lowest-indexed live one; remove it, or despawn the entity, and the camera goes `Free`, unless the game has set another mode meanwhile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CameraTarget;
pub const CAMERA_TARGET: &str = "camera_target";
pub struct CameraTargetPlugin;   // kit_present, before camera_update: sets CameraController.mode = Follow(target) when it differs and remembers the target; no live entity with the marker and a mode still equal to that Follow → Free; a mode the game set itself is left alone; no controller → nothing
```

### 2.9 Scene components for the eight items

Each item's plugin registers its scene name in `build`, as `animation` does (A5), with a private `<Item>Def` DTO under `deny_unknown_fields`. Fields with no default are required.

| Scene name | DTO fields and defaults | Bundle |
| --- | --- | --- |
| `lifetime` | `seconds: f32 >= 0` | `(Lifetime,)` |
| `despawn_outside` | exactly one of `region: [[min_x, min_y], [max_x, max_y]]` or `view: f32 >= 0` (the margin) | `(DespawnOutside,)` |
| `spawner` | `prefab: String` (checked with `check_prefab_id`), `interval: f32 > 0`, `count: Option<u32>` | `(Spawner,)` |
| `health` | `max: f32 > 0`, `invulnerability: f32 >= 0 = 0` | `(Health,)` |
| `hit_flash` | `color: [f32; 4] = [1, 1, 1, 1]`, `seconds: f32 > 0 = 0.1` | `(HitFlash,)` |
| `path_follower` | `points: [[f32; 2]; 2+]` as offsets from the spawn position, `speed: f32 > 0`, `mode: "stop" \| "loop" \| "ping_pong" = "stop"`, `easing: Easing = "linear"` (core's names) | `(PathFollower,)` |
| `top_down_mover` | `speed: f32 > 0` | `(TopDownMover, Position(spawn), PrevPosition(spawn), Velocity::ZERO)`; a `rigid_body` on the same prefab builds first and the equal values stand |
| `platformer_controller` | `run_speed: f32 > 0`, `jump_speed: f32 > 0`, `gravity: Option<f32>`, `coyote_time: f32 >= 0 = 0.1`, `jump_buffer: f32 >= 0 = 0.1`, `jump_cut: f32 in [0, 1] = 0.5` | `(PlatformerController,)`; the body comes from `rigid_body` |
| `tags`, `zone`, `action_input`, `camera_target` | §2.4, §2.6, §2.7, §2.8 | |

The bodiless top-down mover (Q14 of the catalogue): an entity with `TopDownMover` and `Position` but no `RigidBody` moves its `Position` by `direction × speed × dt` with no cast; a kinematic body keeps the cast-and-slide path; any other body is skipped with one warning as today. `Health` gains serde derives (rule 14); `Lifetime` and `Spawner` wait on `Timer` (a follow-up).

### 2.10 Seeing the kit (step 2)

- **Rows.** `foundation/inspect.rs` holds `fn register_rows<T: Inspectable + 'static>(world, label)`, inserting the `InspectRegistry` when absent, and `Inspectable` impls for `Tags`, `Source`, `Zone`, `ActionInput`, `Intent`, `CameraTarget`, `Lifetime`, `DespawnOutside`, `Spawner`, `Health`, `HitFlash`, `PathFollower`, `TopDownMover`, `PlatformerController`, `Parent`, `Children` and `LocalOffset`; `AnimationPlugin` registers `AnimationState` rows with `register_with` (the animation's ID and frame); a state machine registers `elapsed` under its name.
- **`KitDebugPlugin`** (`foundation/debug.rs`): a `KitDebug` resource (`enabled()`, `set_enabled`), off by default, and `kit_debug_draw` in `kit_present`, which draws through core's `DebugDraw` when enabled and the resource exists: a path follower's polyline and current target, a `DespawnOutside` region, a zone's bounds, a spawner's mark, a camera target's cross. A hotkey or console command is a follow-up.
- **Descriptions** (`foundation/describe.rs`): `ComponentDescription::new(name, summary)` with `.field(FieldDescription::new(name, FieldType, doc).default("0.1"))`, `.event(..)`, `.genre(..)`, `.sample("{ \"max\": 5 }")` and `.roundtrip(fn(serde_json::Value) -> Result<serde_json::Value, String>)` (deserialize the DTO, serialize it back); `FieldType` is a closed enum (`Bool`, `F32 { min, max }`, `U32 { min, max }`, `Text`, `Vec2`, `Color`, `List(Box<FieldType>)`, `Enum(&'static [&'static str])`, `Prefab`, `Tag`, `Filter`, `Object(&'static str)`); a `KitDescriptions` World resource keeps them sorted by name, and `render()` prints one line per component and per field, with a `kind <name>` entry per data kind. Each plugin describes what it registers. The DTOs gain `Serialize`. `serde_json` joins the kit's runtime dependencies (Q12). The test (`describe::tests`): for each description, the sample round-trips and every defaulted field equals its described default; each described enum variant deserializes; each numeric field takes its described extremes and refuses what lies outside a bound; a scene name with no description, or a description with no registration, fails.
- **The snapshot.** `crates/tungsten-kit/examples/components.rs` builds `KitPlugins::set()` into a `World` and `Schedule` and prints `render()`; `scripts/public-api.sh` runs `cargo run -q --locked -p tungsten-kit --example components` after its crate loop into `$out/tungsten-kit-components.txt`, under the same `cmp` and diff-head rule (A17), so `just api-check` compares both files (`D-107` amended).

### 2.11 Recipes (step 3)

- **`Harness::load_manifest(&mut self, path) -> anyhow::Result<()>`** in `crates/tungsten/src/testing.rs`: loads the manifest's headless sections in `load_all_merged`'s order (animations, sounds, tilemaps, particle configs, UI themes, then prefabs and data), allocates each `materials` entry's metadata (ID, source manifest, shader ID, uniform defaults) in the `MaterialRegistry` as `load_materials` does without the upload, so a hit flash finds `damage_flash` headless (`health.rs:356-368`), and skips what needs the renderer (sprites, fonts, shaders, particle meshes); a prefab's sprite name is interned at spawn. The template's tests use it in place of hand-loading prefabs.
- **Fixtures** under `crates/tungsten/tests/fixtures/kit/<genre>/`: `manifest.json` (`prefabs`, `data.tags`, `animations` where used; no sprite files), `prefabs/*.json`, `data/tags.json`. The coverage table:

  | Scene name or kind | platformer | top_down | shmup | grid |
  | --- | --- | --- | --- | --- |
  | `tags`, `action_input`, `health`, `camera_target`, kind `tags` | hero | hero | ship | rogue |
  | `top_down_mover` | | hero (kinematic) | ship (bodiless) | rogue (bodiless) |
  | `platformer_controller` | hero | | walker (a ground enemy on a kinematic body under the move actions) | |
  | `path_follower` | lift | drone | raider | |
  | `lifetime` | spark | bolt | bullet | potion |
  | `despawn_outside` | | bolt (region) | bullet (view) | |
  | `spawner` | | nest → slime | wave → raider | hole → rat |
  | `zone` | door (`all: [team.player]`) | trap (`all: [team.player]`) | | |
  | `hit_flash` | | slime | ship | |
  | `animation` | hero | slime | | |

  Static bodies (ground, walls) come from `rigid_body`. A fixture whose prefabs flash lists `damage_flash` under `materials` and `shaders`, with a copy of the root's shader file under its `shaders/`, which `load_manifest` allocates without compiling.
- **Tests** under `crates/tungsten/src/tests/kit_recipes/`, one file per genre, each building `App::new(Config::default())` plus `KitPlugins::set()`, loading its fixture through `load_manifest` and running its script at dt 1/60 and again at 1/30 (two steps a frame): the platformer's hero runs and jumps over the ground and lands grounded, the door zone sends one `ZoneEntered` for the hero, the spark expires; the lift carries the hero with the plugins registered in reverse order (`PlatformerControllerPlugin` before `PathFollowerPlugin`, the moving-platform recipe); the top-down hero slides along a wall, `damage` lowers its health, sets its flash and sends `Damaged`, the nest's slimes carry `Source(nest)`, the trap fires for the hero and not for a slime, the bolt leaves its region, the camera follows the hero (`Follow(hero)` and `CameraState` moves); the shmup's ship moves by `Position` and draws by `Transform`, bullets expire and leave the view, the wave's raiders follow their path, the walker runs on its floor; the grid's rogue moves by `Position`, the hole spawns rats, the potion expires, the camera follows.
- **Coverage** (`coverage.rs`): builds `KitPlugins::set()` into a World, lists the `ComponentRegistry`'s names and the `DataRegistry`'s kinds, scans every fixture's prefab `components` keys and manifest `data` kinds, and fails when a registered scene name or kind appears in fewer than two genres, or a fixture names one the registries lack (`animation` and `rigid_body` count as registered).
- **Determinism** (`determinism.rs`): each genre runs a fixed input script for 300 frames at 1/60 s; the FNV-1a hash over every entity's index, `Position` or `Transform.position` bits, `Health` and `Tags`, in entity order, combined over the four genres; run twice in-process, equal, and pinned as `KIT_RECIPES_HASH` from the first run, recorded in the evidence log and in decision 172. The App flushes commands once per frame after `PostUpdate` (`app.rs:1605-1609`), so a despawn queued in a frame's first step is still alive in its second and a spawn queued there is absent until the next frame: the hash is a one-step-per-frame claim, as the physics hash is, not a claim over frame groupings; the recipes' behavioural asserts hold at both dts, and a flush per step is a core question under Follow-ups.
- **Criterion** (`crates/tungsten-kit/benches/kit.rs`, `[[bench]] name = "kit", harness = false`, criterion 0.8 as core's): `lifetime_tick`, `health_tick`, `action_input`, `top_down_mover_bodiless` and `tag_filter` at 1,000 and 10,000 entities; built and run once in test mode here; the measured run is W13i's (Q6).

### 2.12 The template (step 1)

- `assets/prefabs/player.json`: `player: null` stays (the game's own marker, which its tests and save use), plus `tags: ["team.player"]`, `action_input: { "move": ["move_left", "move_right", "move_up", "move_down"] }`, `top_down_mover: { "speed": 240 }`, `health: { "max": 5, "invulnerability": 0.5 }`, `hit_flash: { "color": [1, 1, 1, 1], "seconds": 0.12 }`, `camera_target: null`.
- `assets/data/tags.json` (`["team.player"]`) under the manifest's `data.tags.game`; `shaders.damage_flash` and `materials.damage_flash` as the root manifest has them, the shader copied to `assets/shaders/damage_flash.wgsl` (Q8).
- `src/game.rs`: `player_movement` and `PLAYER_SPEED` go; `register` adds `app.add_plugins(KitPlugins::set())` before `GamePlugin`; the docs say what the kit does for the player. `src/states.rs`: pause pauses the game clock on enter, after the autosave, and resumes it on exit (Q7). `src/components.rs` stands.
- `tests/game.rs`: `harness_with` uses `load_manifest`; the `SCHEDULE` snapshot takes the new text; the movement tests assert the distance within one step of `240 × 30 / 60` and the half-scale ratio within one step; `data_files_load` checks the tags entry and the material; new tests: `damage` lowers the health, starts the flash and sends `Damaged`; `CameraController.mode` is `Follow(player)` and the camera's position moves after the player does.
- `AGENTS.md`: the kit bullet names `KitPlugins::set()`, prefab components by scene name, `tags`, `ActionInput`, `CameraTarget`, `Phase::join` for a game system in a phase, and the engine's genre fixtures as the cookbook; the Layout section gains the growth rule (w13-kit §8's text). `README.md` says the player is a prefab of kit components.

## 3. Steps

### Step 1: The kit's public surface and the template

- **Files:** `crates/tungsten-kit/Cargo.toml` (`serde_json` runtime dependency, `[[bench]]` waits for step 3), `crates/tungsten-kit/src/lib.rs`, the moved modules and their `mod.rs` files, `foundation/phases.rs`, `foundation/plugins.rs`, `foundation/tags.rs`, `foundation/source.rs`, `overlap/zone.rs`, `intent/action_input.rs`, `present/camera.rs`, unit tests under `crates/tungsten-kit/src/tests/` (new files `phases.rs`, `tags.rs`, `source.rs`, `zone.rs`, `action_input.rs`, `camera.rs`), `crates/tungsten/src/tests/kit.rs`, `crates/tungsten/src/testing.rs`, the template files of §2.12, `Cargo.lock` (when the dependency move changes it), `api/tungsten-kit.txt`, `api/tungsten.txt` (`Harness::load_manifest`)
- **Change:** in this order: (1) `git mv` the modules into §2.1's folders, fix the `mod` and `#[path]` lines, build; (2) `phases.rs` and `ensure_phases`, each plugin calling it and joining its phase per §2.2, the lifecycle and presentation items moved to their phases, `despawn_outside` and `spawner_tick` reading `Position` when present; the kit's unit tests that ran `Stage::PostUpdate` for the moved items run `Stage::FixedUpdate`; `kit.rs` updated, with a test that a child spawner on a moving parent spawns at the parent's last propagated point; (3) `KitPlugins` and `prelude`; (4) `Tags`, `TagFilter`, `TagsPlugin` and the `tags` kind; (5) `Source` and `SourcePlugin`; (6) `Zone`, `ZoneEntered`, `ZonePlugin`; (7) `ActionInput`, `Intent`, `ActionInputPlugin` with the load check against the action map, the movers reading `Intent`, the bodiless top-down mover; (8) `CameraTarget` and `CameraTargetPlugin`, removal returning the camera to `Free`; (9) the eight scene components of §2.9 and `Health`'s serde; (10) `Harness::load_manifest`; (11) the template; (12) rustdoc with a doctest per new item, the root docs rewritten as the catalogue by domain, then `just api` and the diff reviewed. Then the step 1 API critique as the skill's "Running steps" says, its `--from` reading `api/` and `templates/`; a finding that holds changes this step before step 2 runs.
- **Done-when:** `cargo test -p tungsten-kit --locked` → passes; `cargo test -p tungsten --locked kit_tests` → passes; `cargo test -p tungsten --locked plugins` → passes with `git diff --stat crates/tungsten/src/tests/plugins.rs` empty (`DefaultPlugins`' schedule unchanged); `cd templates/basic && cargo test --locked` → passes, the schedule snapshot test quoting the new resolved text; `just api` → `Wrote api/…`, `git diff --stat api/` names `api/tungsten-kit.txt` and `api/tungsten.txt` (one added `load_manifest` line), and `git diff api/ | rg -c '^-pub'` → `0` (additions only); `cargo test -p tungsten-kit --locked phases` → with the engine's names registered before the kit's and after them, every `Move` member resolves before `physics_step` and every `Present` member before `camera_update`; `cargo test -p tungsten-kit --locked action_input` → a prefab naming `move_rigth` fails the load naming `move.1`; `cargo test -p tungsten-kit --locked camera` → removing the marker returns the mode to `Free` and a game-set `Scripted` stays; `cargo doc -p tungsten-kit --no-deps --locked` → no warnings; the API critique's findings recorded in §9 as `C.<n>`
- **Moves:** the template's resolved-schedule snapshot (`tests/game.rs`); the lifecycle items' stage (Q1); nothing pinned

### Step 2: Seeing the kit

- **Files:** `crates/tungsten-kit/src/foundation/{describe,debug,inspect}.rs`, each item module (rows and a description in its plugin's `build`), `crates/tungsten-kit/src/lib.rs` (re-exports: `KitDebug`, `KitDebugPlugin`, `KIT_DEBUG_DRAW`, `ComponentDescription`, `FieldDescription`, `FieldType`, `KitDescriptions`), `crates/tungsten-kit/src/tests/{describe,debug,inspect}.rs`, `crates/tungsten-kit/examples/components.rs`, `scripts/public-api.sh`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt`
- **Change:** §2.10 in its order: rows, the debug plugin, the descriptions and their test, the example, the script.
- **Done-when:** `cargo test -p tungsten-kit --locked describe` → passes (the defaults-and-variants test over every registered scene name and kind); `cargo test -p tungsten-kit --locked inspect` → a World built from `KitPlugins::set()` has one row set per described component; `cargo test -p tungsten-kit --locked debug` → an enabled `KitDebug` draws a path follower's polyline (`DebugDraw::len() > 0`), a disabled one nothing; `scripts/public-api.sh` → writes five files, `api/tungsten-kit-components.txt` among them, one `component <name>` entry per scene name of §2.9, §2.4, §2.6, §2.7 and §2.8 plus `animation`, and `kind tags`; `just api-check` → `Public API snapshots current` with no stale file; `just script-test` → passes (ShellCheck over the script)
- **Moves:** none

### Step 3: Recipes, coverage, determinism and the bench

- **Files:** `crates/tungsten/src/lib.rs` (`#[cfg(all(test, feature = "kit"))] #[path = "tests/kit_recipes/mod.rs"] mod kit_recipes;`), `crates/tungsten/src/tests/kit_recipes/{mod,platformer,top_down,shmup,grid,coverage,determinism}.rs`, `crates/tungsten/tests/fixtures/kit/{platformer,top_down,shmup,grid}/` (manifests, prefabs, `data/tags.json`, animation files where the table has one), `crates/tungsten-kit/Cargo.toml` (criterion dev-dependency, `[[bench]]`), `Cargo.lock`, `crates/tungsten-kit/benches/kit.rs`
- **Change:** §2.11: the fixtures to the coverage table, the recipe helper (`fn recipe(genre, plugins: PluginSet) -> Harness`), the genre tests, the reverse-order moving-platform recipe, the coverage test, the determinism test with its hash pinned from the first run, the bench.
- **Done-when:** `cargo test -p tungsten --locked kit_recipes` → every recipe passes at both dts, `coverage` passes with no name under two genres, `determinism` prints the hash twice equal and the pinned value matches on a second run; `just bench-build` → the kit bench compiles; `cargo bench -p tungsten-kit --bench kit -- --test` → every row runs once; `just repo-check` → no unlisted-asset error (the fixtures sit outside the asset roots it scans)
- **Moves:** none; the kit hash is pinned here, its value in the evidence log and decision 172

### Step 4: Records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `AGENTS.md`, `docs/LLM_INDEX.md`, `docs/getting-started.md`, `docs/known-issues.md`, `docs/perf/profiling-workflow.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w13-kit.md`
- **Change:** (1) decision 172 with tungsten-decision, its index row at the end of the kit's section, the `Amended by` markers and index notes on `D-155` (the admission rule and the stage rule), `D-158` (the chain onto the anchors) and `D-107` (the components snapshot), and the `serde_json` note under `D-015`'s rule; (2) `DESIGN.md`: "### Kit Foundations — M62" after the M61 section, with the phases and the resolved stage text of §2.2 as the run produced it, the data-first layers, tags and `Source`, `Intent`, the descriptions snapshot, the recipe harness and the template; (3) `AGENTS.md:31`: "with recipes in two genres, under the kit's admission rule (decision 172)", no longer than today; (4) the index's kit row rewritten no longer than today's (A19): "Kit by domain: phases, tags, scene components, recipes, fixtures, descriptions (`D-155`, `D-158`, the new decision)" with `crates/tungsten-kit/src/`, `tungsten/tests/kit.rs`, `tungsten/tests/kit_recipes/` and `tungsten/tests/fixtures/kit/`; (5) the guide: the layout bullets, the system, query and clock examples that cited `player_movement` (A21) rewritten around `toggle_mute` and a kit prefab, and a "Composing with the kit" section after "Queries and spawns" (`KitPlugins::set()`, the player prefab's components, `tags` data, `ActionInput` and slots, `CameraTarget`, `Health` and `damage`, `Phase::join`, the fixtures as the cookbook, the growth rule); (6) known issues: the controllers' row gains the bodiless mover (no casts, so walls do not stop it) and the zone's body-only sight (A11); the kit row gains the child spawner's one-frame lag behind a moving parent and the per-frame command flush between a frame's steps, and drops the `KitPlugins` and scene-component waits; (7) the profiling workflow's criterion list gains `kit` in `tungsten-kit`; (8) a "for the freeze" ledger row listing the new surface as additions; (9) the workstream file: a revision line, the M50, M52 and M61 follow-ups this milestone lands marked landed, and §5.1's `KitPlugins` note.
- **Done-when:** `just ctx` → passes with `wc -c docs/LLM_INDEX.md` ≤ 12288 and `wc -c AGENTS.md` ≤ 6144; `just repo-check` → passes (decision 172 cited only once it exists; every link resolves); `rg -n 'M62' docs/DECISION_INDEX.md` → one new row and three amended rows; `python3 -B scripts/check-repo.py` → `OK`
- **Moves:** none

### Step 5: Release 0.82

- **Files:** `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.82.0` once, every release check once after it with `just template-check` and `just smoke` (no step runs them), then the preflight with `--message`
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.82.0 --repo JosephGibson/Tungsten --message 'Update 0.82: M62 kit foundations'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the next prompt: the plan prompt for W13h (the catalogue's A)

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | Do the lifecycle items leave `PostUpdate`? `lifetime_tick` to `kit_tick`, `spawner_tick` to `kit_react` and `despawn_outside` to `kit_overlap` put every kit timer and spawn on the fixed step, which the determinism hash needs; a lifetime then ends on a step and a despawn-outside-view judges the last frame's camera. | Move them; `animation_tick`, `hit_flash_tick` and the sound plugin stay per frame in `kit_present`. The alternative, the three staying in `PostUpdate`, leaves the hash frame-rate-bound. The command flush stays per frame, so the hash is a one-step-per-frame claim (§2.11). |
| Q2 | Public domain modules (`tungsten::kit::motion::TopDownMover`) or private ones behind root re-exports? | Private: each item keeps one definition path, `api/tungsten-kit.txt` changes by additions, and the `prelude`'s `pub use` lines are the only second path. |
| Q3 | Tag storage: sorted strings or interned IDs? | Strings; `Tags` and `TagFilter` keep private fields, so an intern table can replace them later behind the same API, if A's cue rules show a cost. |
| Q4 | Does `ActionInput` aim? | Four optional aim axes only; cursor and nearest-target aim are M's `Aim`. |
| Q5 | What produces the components snapshot? | The kit's `components` example, run by `scripts/public-api.sh`; a test writing to an environment path was the alternative. |
| Q6 | Does this milestone take a measured criterion sitting with a linear bound? | No: the bench builds and runs in test mode; W13i's card owns the first measured rows and sets the bound. |
| Q7 | Does the template's pause pause the game clock? | Yes: the kit moves on the fixed step, so a pause that left the clock running would move the player; `Time::pause` on enter and `resume` on exit is what a game does. |
| Q8 | Does the template carry `hit_flash` with the `damage_flash` shader and material? | Yes, copied from the root manifest; without the material every hit warns. |
| Q9 | The platformer controller's second genre? | The shmup fixture's ground walker on a kinematic body under the move actions. |
| Q10 | Is `Source` the chain's origin or the immediate spawner? | The origin; an immediate-spawner component waits for a user. |
| Q11 | A bodiless entity never enters a zone (A11). | A recorded limit in known issues until C's grid gives zones a bodiless form. |
| Q12 | `serde_json` as a kit runtime dependency for the descriptions' round-trip hook? | Yes: a workspace runtime dependency of core already, so no new third-party crate (`D-015`); noted in decision 172. |
| Q13 | Zone events for an entity leaving a zone? | No: `SensorEntered` has no exit (`D-154`); the trigger zone's limit stands. |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

172: W13g landed (M62): the kit foundations: domain subfolders behind one public path per item, `prelude` and `KitPlugins`; seven phase anchors every kit system joins, the lifecycle items on the fixed step; `Tags` and `TagFilter` declared in the `tags` kind or at plugin build and checked at load; `Source` derived through spawn inheritance; `Zone`, `ActionInput` with `Intent`, `CameraTarget`; a scene component per item; inspector rows, `KitDebugPlugin` and field descriptions snapshotted as `api/tungsten-kit-components.txt` under `just api-check`; recipes in two genres as the admission rule; the kit determinism hash pinned. Amends `D-155` (the admission rule and the stage rule), `D-158` (the chain maps onto the anchors) and `D-107` (the second snapshot file). One decision, as `D-170` and `D-171` were. Next free ID when written; checked again when the step runs.

## 6. Invariants

- **May move:** the template's resolved-schedule snapshot (`templates/basic/tests/game.rs`, step 1); the kit harness tests' timing for the moved items (step 1); the kit determinism hash, pinned for the first time in step 3, its value in the evidence log and decision 172.
- **Must not move:** the determinism hash `0x088e_c07a_73c1_b168`, the pinned containment hash `0xaee2_72e0_1ffc_4e4c`, the row digests, `gpu-visual.png`, the post and transition regressions; `DefaultPlugins`' resolved schedule (`src/tests/plugins.rs` unchanged); the kit on core alone with core naming no kit type; no new third-party dependency; two background threads; opt-in plugins (`DefaultPlugins` holds none); no render type in the kit; prefab and scene files keep their fields (`D-153`, `D-157`); `api/tungsten-kit.txt` loses no line; `api/tungsten-core.txt`, `api/tungsten-render.txt` and `api/tungsten.txt` unchanged but for `Harness::load_manifest`.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it.
- An item needs a physics or core change beyond `Harness::load_manifest`: mark it waiting in the plan and leave it out of the coverage table.
- The API critique finds a shape tied to one genre: redesign it in step 1 or drop it, before step 2.
- A registered scene name cannot be placed in two genres' fixtures without an artificial prefab: record it as a coverage exception in the test with its reason and a follow-up, rather than invent a prefab.
- The index or the root rules cannot take their rewritten lines within budget: stop at step 4's line, record it, and leave the budget change to a decision.
- A file outside §files to touch needs a change: stop and record which.
- `just api` shows a removed line in `api/tungsten-kit.txt`: a break this plan did not plan; stop, record it, and add the ledger row before going on.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1–4, release | `just checks` recommends: `cargo test -p tungsten-kit`, `-p tungsten`, `-p tungsten-core --test manifests`, `-p tungsten-template-basic`, `just ctx`, `just repo-check`, `just api-check`, `just script-test`, `just check`, `git diff --check`; the release step runs `just check`, `just smoke`, `just script-test`, `just template-check` and the rest of the release checks once, after the cut. No CPU row: the kit touches no shared ECS or frame-loop code, and `DefaultPlugins` is unchanged |
| 3 Invariants | 1, 3 | §6 |
| 4 Decisions | 4 | decision 172 |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 2, 4 | additions only; rustdoc on every new item; the "for the freeze" ledger row; the second snapshot file |
| 6 Template and guide | 1, 4 | the player prefab, the growth rule, "Composing with the kit" |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 4 | the kit row rewritten within budget |
| 8 Design | 4 | "Kit Foundations — M62" |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 4, release | known issues and the workstream file in step 4; no backlog row (nothing cut); the gap log has no rows; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the user-level `critique` skill with `--repo` at `--effort max` (level C), `gpt-6-astra`, 2026-10-16, reading `abbcdb6` with this plan uncommitted: context 872,000, output cap 4,000, 606.7 s, 831,056 tokens in (729,472 cached), 18,775 out, 42 commands, 315.9 KB output, 5-hour window 51% → 78%, week 50% → 54%. Seven findings, each checked at `abbcdb6`, all accepted. Round 2 was owed, since round 1 changed done-when checks, and did not run: `critique: codex exec failed (exit 1, usage limit reached)` after 28 commands and 295.6 KB of output, the 5-hour window at 78% → 100% (it reopens at 16:18 on 2026-10-16). There is no third round; no finding is open, so no open question follows from it.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] The Move anchor does not order movers before physics: `join` places a Move member between `kit_move` and `kit_overlap`, and the resolver takes the earliest-registered ready system, so `physics_step` runs before the movers. | `schedule.rs:419-420`; §2.2 as written | accepted | §2.2: `join` gives a `Move` member `before_if_present(PHYSICS_STEP)`; step 1 gains the `phases` test under both registration orders |
| 2 | [MEDIUM] Moving spawners into `FixedUpdate` breaks their hierarchy positioning: `spawn_child` gives no `Position`, so a child spawner reads its previous frame's `Transform`. | `hierarchy.rs:102-106`; `spawner.rs:122-126` | accepted | §2.2 records the one-frame lag as a limit; step 1's `kit.rs` pins it; step 4's known-issues row |
| 3 | [MEDIUM] Fixed-step timers still depend on frame batching: the flush runs once per frame after `PostUpdate`, so two steps in one frame differ from two frames of one step, which a 1/60 hash cannot show. | `crates/tungsten/src/app.rs:1605-1609` | accepted | §2.11 limits the hash to one step per frame, the recipes run at 1/60 and 1/30, Q1 says so; a per-step flush is a follow-up |
| 4 | [MEDIUM] Removing `CameraTarget` leaves the camera following: only a dead target reset the mode to `Free`. | `crates/tungsten/src/camera.rs:77-88`; §2.8 as written | accepted | §2.8: the plugin remembers its target and resets a mode still equal to its `Follow`; step 1's `camera` test |
| 5 | [MEDIUM] `ActionInput` accepts misspelled action IDs silently: an unknown action reads as unbound and a value of zero. | `action_map.rs:498-500, :561`; `w13-kit.md:90` | accepted | §2.7: a load check against `ActionMap::iter`'s names; step 1's `action_input` test |
| 6 | [MEDIUM] The template's flash test lacks the material: `load_manifest` skipped materials and `start_flash` marks the flash missing without one. | `health.rs:356-368`; `asset_loader/mod.rs:274-300` | accepted | §2.11: `load_manifest` allocates material metadata as `load_materials` does without the upload; the fixtures list the material with a copied shader |
| 7 | [MEDIUM] The step gates exclude files their changes must update: `api/tungsten.txt` changes with `Harness::load_manifest` (the script enables `testing`), and criterion changes `Cargo.lock`, which `--locked` builds need. | `scripts/public-api.sh:49-53`; `Cargo.lock:3033-3041`; `justfile:38` | accepted | steps 1 and 3 and the header list `Cargo.lock` and `api/tungsten.txt`; step 1's diff check names both snapshot files |

Step 1 API critique (level C): the `critique` skill with `--repo --force` at xhigh, `gpt-6-astra`, 2026-10-16, over `git diff HEAD -- api/ templates/` plus the untracked template files: 207.6 s, 1,200,770 tokens in (1,097,472 cached), 7,396 out, 21 commands, 227 KB output, 5-hour window 0% → 25%, week 58% → 62%. Four findings, each checked against the tree, all accepted and applied in step 1 before step 2 ran.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| C.1 | [MEDIUM] Pausing while moving saves a position behind where the player stops: `Time::pause` takes effect from the next frame, the pushing frame still runs the steps it owed, and `write_autosave` read the drawn `Transform`, which trails `Position` by a step. | `time.rs:274`, `time.rs:160-183`; `app.rs:1176-1180`; `templates/basic/src/states.rs` | accepted | the template saves the body's `Position` on the pause's first frame that runs no step (`Time::fixed_steps_this_frame() == 0`); `continue_from_the_autosave` pauses with the movement still held and compares `Position` and `Transform` after the restart |
| C.2 | [MEDIUM] `Intent::deserialize` bypassed the direction clamp and the sorted slot insertion, so `{"movement":[10,0]}` drove a bodiless mover at ten times its speed and unsorted slots were missed by the binary search. | `action_input.rs` (the derive) | accepted | `Intent` deserializes through `IntentFields`: directions clamped, slots sorted, a slot named twice refused; unit test `an_intent_read_from_data_is_clamped_and_its_slots_sorted` |
| C.3 | [MEDIUM] `Health::deserialize` accepted the four private fields unchecked: `max: -1` with `current: 5` made a living entity that `heal(0)` set to −1. | `health.rs` (the derive), `Health::heal` | accepted | `Health` deserializes through `HealthFields`: `max` finite above 0, `current` within `0..=max`, both timers finite and at least 0, unknown fields refused; unit test `health_is_data_within_its_bounds` |
| C.4 | [MEDIUM] The follow camera with the controller's zero dead zone and snap smoothing cancels the player's displacement on screen, so in the template's empty world the movement disappears. | `camera.rs:108-127` (core), `camera.rs:10-32` (umbrella) | accepted | the template's plugin sets a 640 × 360 dead zone (`CAMERA_DEAD_ZONE`), so the player roams the middle of the view before the camera follows; `the_camera_follows_the_player` asserts the camera holds at 120 px and follows at 360 px; the README says why |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 2 | 2026-10-16 | pass | `cargo test -p tungsten-kit --locked describe`, `inspect` and `debug` pass inside the crate run (`212 passed`, doctests `26 passed`): every registered scene name and the `tags` kind described and nothing else, 13 components, the 13 samples round-trip to their described defaults, the path follower's 3 modes and 22 easings deserialize, the 15 numeric fields take their extremes and refuse beyond; a World from `KitPlugins::set()` has rows under every described component's label and under `Intent`, `Source`, `Parent`, `Children` and `LocalOffset`; an enabled `KitDebug` draws a looping path follower as 5 commands, a disabled one 0; `scripts/public-api.sh` → `Wrote api/tungsten-core.txt … api/tungsten-kit.txt`, `Wrote api/tungsten-kit-components.txt` (69 lines: 13 `component` entries, `tags`, `zone`, `action_input`, `camera_target`, `lifetime`, `despawn_outside`, `spawner`, `health`, `hit_flash`, `path_follower`, `top_down_mover`, `platformer_controller` and `animation`, plus `kind tags`); `just api-check` → `Public API snapshots current: 4 crates and the kit's components`; `just script-test` → `OK` (39, 7 and 22 tests, ShellCheck clean); `api/tungsten-kit.txt` 1,460 lines added since `HEAD`, 0 removed; clippy clean on the kit (with its example) and the umbrella; `cargo doc` no warnings. Deviations from §2.10: the `Inspectable` impls live with their types (private fields), `foundation/inspect.rs` holds the registration helpers; `FieldType` is `#[non_exhaustive]` with a `Bound` enum for numeric limits, so A's kinds are additions; a description or row registered twice by the same plugin is kept once; `KitDebugPlugin` joins `KitPlugins::set()`, so the umbrella's and the template's schedule snapshots gained `kit_debug_draw` (step 1's files, edited here). | `crates/tungsten-kit/src/foundation/{describe,debug,inspect}.rs`, `crates/tungsten-kit/src/tests/{describe,inspect,debug}.rs`, `crates/tungsten-kit/examples/components.rs`, `scripts/public-api.sh`, `api/tungsten-kit.txt`, `api/tungsten-kit-components.txt` |
| 3 | 2026-10-16 | pass | `cargo test -p tungsten --locked kit_recipes` → `22 passed` (every recipe at dt 1/60 and 1/30: the platformer's hero runs, jumps 50+ px with the jump held 0.3 s and lands grounded, the door sends one `ZoneEntered` for the hero, the spark expires, the lift carries the hero with `PlatformerControllerPlugin` registered before `PathFollowerPlugin` and the resolved text still puts `path_follow` first; the top-down hero slides along the wall, `damage` lowers the slime's health, starts its flash and sends `Damaged`, the nest's two slimes carry `Source(nest)`, the trap fires once for the hero and not for a slime, the bolt leaves its region, the drone visits all four corners, the camera follows; the shmup's ship moves by `Position` and draws by `Transform`, one bullet leaves the view and one expires, the wave's three raiders follow their path, the walker runs grounded on its floor; the grid's rogue moves by `Position`, the hole spawns two rats with `Source(hole)`, the potion expires, the camera follows); `coverage` passes: 13 registered scene names and the `tags` kind each in two or more of the four fixtures, no fixture names an unregistered one; `determinism` prints `kit recipes hash: 0x2b648d776dff9170` twice equal and the pin `KIT_RECIPES_HASH = 0x2b64_8d77_6dff_9170` matches on the rerun; `just bench-build` → every bench builds (`Executable benches/kit.rs` among them); `cargo bench -p tungsten-kit --bench kit -- --test` → `Success` for all 10 rows (5 systems × 1,000 and 10,000); `just repo-check` → `OK` (the fixtures sit outside the scanned asset roots); `Cargo.lock` gains the kit's criterion line. The coverage test reads the fixtures through the engine's loader on a recipe harness rather than parsing JSON, since the umbrella has no `serde_json` dependency. | `crates/tungsten/src/tests/kit_recipes/`, `crates/tungsten/tests/fixtures/kit/{platformer,top_down,shmup,grid}/` (34 files), `crates/tungsten/src/lib.rs`, `crates/tungsten-kit/benches/kit.rs`, `crates/tungsten-kit/Cargo.toml`, `Cargo.lock` |
| 4 | 2026-10-16 | pass | `just ctx` → `self-test: OK`, `wc -c docs/LLM_INDEX.md` → `12283` (≤ 12288), `wc -c AGENTS.md` → `5402` (≤ 6144); `just repo-check` → `Repository QA: 0 error(s), 68 note(s)`, `5 passed`; `rg -n 'M62' docs/DECISION_INDEX.md` → 4 rows: the new `D-172` row and the amended `D-107`, `D-155` and `D-158` rows; `python3 -B scripts/check-repo.py` → `0 error(s)`. Decision 172 written with its three marker lines and the `serde_json` note under `D-015`'s rule 2 in its Why; `DESIGN.md` gains "Kit Foundations — M62" with the resolved stage text as the run produced it and the status line the kit's new capability; `AGENTS.md`'s kit row cites `D-172` (shorter than before); the index's kit row names the domains, `tungsten/tests/kit_recipes/` and `crates/tungsten/tests/fixtures/kit/`, dropping `D-158` (which `D-172` amends) to fit the budget; the guide's layout, system, query, prefab, clock and pause text no longer cite `player_movement`, and "Composing with the kit" follows "Queries and spawns"; known issues gain the bodiless mover, the zone's body-only sight, the child spawner's lag and the per-frame flush; the profiling workflow's criterion list gains `kit`; the W4 ledger gains the "for the freeze" row; w13-kit gains its revision line, the landed M50, M52 and M61 follow-ups and §5.1's note. | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `AGENTS.md`, `docs/LLM_INDEX.md`, `docs/getting-started.md`, `docs/known-issues.md`, `docs/perf/profiling-workflow.md`, `docs/plans/1.0/w04-api-freeze.md`, `docs/plans/1.0/w13-kit.md` |
| 5 | 2026-10-16 | pass | Workflow §5 closed: invariants held (no core, render or physics code changed beyond `Harness::load_manifest`, so the physics determinism and containment hashes, the row digests, `gpu-visual.png` and the post and transition regressions stand; `DefaultPlugins`' schedule pinned unchanged by `crates/tungsten/src/tests/plugins.rs`; `api/tungsten-kit.txt` loses no line; `api/tungsten-core.txt` and `api/tungsten-render.txt` unchanged, `api/tungsten.txt` +1); `DESIGN.md` updated in step 4; the register row filled and the README's owned fields rewritten by `just release-cut 0.82.0`; one `CHANGELOG.md` entry; `status: done`. The plan is archived before the cut, so the cut, the release checks (`just check`, `just ctx`, `just repo-check`, `just api-check`, `just script-test`, `just smoke`, `just notices`, `just template-check`, `git diff --check`) and the preflight report in the session's final message. | `CHANGELOG.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`, `Cargo.toml`, `Cargo.lock`, `DESIGN.md`, this plan |
| 1 | 2026-10-16 | pass | `cargo test -p tungsten-kit --locked`: `199 passed`, doctests `24 passed`; `cargo test -p tungsten --locked kit_tests`: `29 passed`; `cargo test -p tungsten --locked plugins`: `13 passed`, `git diff --stat crates/tungsten/src/tests/plugins.rs` empty; `cd templates/basic && cargo test --locked`: `13 passed` (`the_schedule_matches_the_snapshot` pins `fixed_update: physics_prev_snapshot, kit_tick, lifetime_tick, health_tick, kit_intent, action_input, kit_move, path_follow, platformer_controller, top_down_mover, physics_step, kit_overlap, despawn_outside, zone_tick, kit_resolve, kit_react, spawner_tick` and `post_update: …, transform_propagate_late, kit_present, animation_tick, hit_flash_tick, camera_target, camera_update`); `just api` → `Wrote api/tungsten-core.txt … api/tungsten-kit.txt`; `git diff --stat api/` → `api/tungsten-kit.txt \| 1206 +`, `api/tungsten.txt \| 1 +` (`Harness::load_manifest`); removed lines 0 in both, counted with Python (`rg -c '^-pub'` through the shell hook is unreliable, so the count replaces it); `phases`, `action_input` and `camera` tests pass inside the crate run (both registration orders, `move.1` named for `move_rigth`, `Free` after the marker goes and a game-set `Scripted` kept); `cargo doc -p tungsten-kit --no-deps --locked`: no warnings; clippy `-D warnings` clean on the kit, the umbrella (`testing`) and the template; the API critique's C.1–C.4 applied (§9). Deviations from §2: `ZoneEntered` is `#[non_exhaustive]` with `new`, as the kit's other events; `CameraTarget` takes over a `Free` mode or its own earlier `Follow` only; a scene `tags` list or `zone` filter is checked in its written order, so an error's index is the file's; `TopDownMover::direction()` reads the intent's movement at the last step; the harness tests for the lifecycle items run at dt 1/30 (two steps a frame, `Time`'s default bound), since 0.05 s frames owe three; `kit_debug_draw` joins the schedule snapshots in step 2. | `crates/tungsten-kit/src/{foundation,intent,motion,lifecycle,overlap,damage,present,transform}/`, `crates/tungsten-kit/src/tests/{phases,tags,source,zone,action_input,camera}.rs`, `crates/tungsten/src/testing.rs`, `crates/tungsten/src/tests/kit.rs`, `templates/basic/`, `api/tungsten-kit.txt`, `api/tungsten.txt`; the critique at the session scratchpad's `api-critique.md` |

## Follow-ups

- `Lifetime` and `Spawner` derive no serde until core's `Timer` does (rule 14): a core row for the Phase 6 QA pass or W13h, whose `Cooldown` wants the same.
- A hotkey or log-console command for `KitDebug`: the debug views (umbrella) when W13h's trace view lands.
- The inspector showing a component's field description: W14b's `describe`, or the Phase 6 QA pass.
- Nearest-name suggestions in `MissingAsset`: unchanged from M61's follow-up.
- An immediate-spawner component beside `Source`, when a reaction needs the spawner rather than the chain's origin.
- A command flush per fixed step, so a despawn or spawn queued in one step is seen by the next step of the same frame (critique finding 3): a core question for W13h's plan or the Phase 6 QA pass; until then the kit hash and recipes are one-step-per-frame claims.
