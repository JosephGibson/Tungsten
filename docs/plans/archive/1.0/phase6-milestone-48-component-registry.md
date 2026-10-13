# Phase 6 milestone 48: component registry and components in scene files (W16a) — draft

- **status:** done
- **goal:** Scene entries name registered components. A `ComponentRegistry` in core, a `World` resource keyed by name and by `TypeId` over serde types, is filled by `app.register_component::<T>("name")`, by plugins through `World::register_component`, and with the engine's data-only components by the code that owns them. A scene file carries `"components": { "name": { … } }` beside today's fields; the load checks it, its errors name the file, the entry and the field, and the components spawn with the entry.
- **non-goals:** Prefab files, `spawn_prefab`, the `prefabs` manifest section, Tiled object classes and the tilemap builder (W16b); engine components whose value names an asset (`ParticleEmitter`, `TilemapInstance`, `AnimationState`; Q4); `tungsten check` (W14b); world snapshots, saves by reflection, nested prefabs and scene hot reload (`D-046`); a `Component` marker trait (Q6); the template's player and enemy from data (W16b's done-when); perf captures, since no step changes `core/ecs/`, a schedule stage or a per-frame path.
- **files to touch:** Step 1: `crates/tungsten-core/src/component_registry.rs` (new), `crates/tungsten-core/src/tests/component_registry.rs` (new), `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/assets/scene.rs`, `crates/tungsten-core/src/tests/assets/scene.rs`, `Cargo.toml` (`[workspace.dependencies]`), `crates/tungsten-core/Cargo.toml`, `crates/tungsten/Cargo.toml`, `Cargo.lock`, `licenses/clarifications.json` and `licenses/serde_path_to_error/` (only if `just notices` asks for a licence text, `D-122`), `crates/tungsten/src/app.rs`, `crates/tungsten/src/asset_loader/scene.rs`, `crates/tungsten/src/tests/asset_loader.rs`, `crates/tungsten/src/tests/tweens.rs`, `examples/03_scene_state/src/states.rs`, `api/tungsten-core.txt`, `api/tungsten.txt`. Step 2: `crates/tungsten-core/src/component_registry.rs` (module doc), `crates/tungsten-core/src/physics/plugin.rs`, `crates/tungsten-core/src/tests/physics/plugin.rs`, `crates/tungsten/src/asset_loader/scene.rs`, `crates/tungsten/src/asset_loader/mod.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/plugins.rs`, `crates/tungsten/src/tests/plugins.rs`, `crates/tungsten/src/tests/asset_loader.rs`. Step 3: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/w16-prefabs.md`, `docs/plans/1.0/w14-tooling.md`, `docs/plans/1.0/implementation-plan.md` (the W6a and W16b cards). Step 4: `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `DESIGN.md` (the status line the cut rewrites), `docs/plans/1.0/implementation-plan.md` (the register row), `docs/plans/1.0/README.md` (the four owned Now fields), this plan (moved to `docs/plans/archive/1.0/`).
- **ordered steps:** (1) The registry, `components` in scene entries and the public API; then the level C API critique. (2) The engine's data-only components, registered by their owners. (3) Records: decision 153, the break ledger, `DESIGN.md`, the index route and the follow-ups. (4) Release 0.67.
- **done-when:** Every step's evidence row quoted; workflow §5 closed (§8); the release checks, plus `just physics-release` and `just visual`, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W16a ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M48, release 0.67. Written 2026-10-13 at `c638b29` on `0.67`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. W16 has not graduated, so [criteria](../../1.0/criteria.md) §8.7 is its scoping source. The new decision is written here as the bare number 153 until its entry exists.

## Context digest

- W16 is owner-picked (criteria §8.7): data-driven entities with game components, without `World` serialization. Amendment 3 split it so that W16a, the registry and `components` in scene entries, lands before the kit, whose components then register as they land (W13a and W13b). W16b adds prefab files, `spawn_prefab`, Tiled object classes and the tilemap builder on this milestone's registry and spawn path.
- It stands on W15a's plugins (`D-128`, `D-133`): `Plugin::build` gets the `World`, which plugins already use for events (`World::register_event`) and inspector rows (the `InspectRegistry` resource). It also stands on W15b's bundles (`D-135`), whose tuples treat any `'static` type as a component and which left "a `Component` marker that would let bundle tuples nest bundles" to W16a; on `D-046`'s scene format, which it extends; and on `D-022`: a data error fails the load, and data that skipped the check is a programmer error.
- `D-103` puts the game-facing file formats under semver from 1.0, so the `components` object, the four engine shapes in §2.4 and the set of names the defaults register freeze with the Rust API (Q7).
- Later candidates expect: W6a extends `rigid_body`'s `kind` with its body kinds; W13a and W13b register kit components in their plugins; W16b's prefab files carry today's entry fields and `components`, checked and spawned through this path, and W16b adds the registration form that checks asset IDs against the World the load already takes, then registers `particle_emitter` and `tilemap` with it; W14b's `check` reuses the load's check and needs the game's names (follow-ups).
- One new crate, `serde_path_to_error`, names the field inside a component that fails (Q12).
- Level C (workflow §4): an Opus 5.5 max plan, the API sketch in §2, and a critique of step 1's `api/` diff before steps 2–4.

## 1. Audit

Read at `c638b29`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `SceneEntry` holds `transform`, `sprite`, `visible`, `tag` and `tweens`, and no scene struct has `deny_unknown_fields`: a misspelt key, `"componets"` among them, parses and is dropped | `crates/tungsten-core/src/assets/scene.rs:27-44`, `:204-220` |
| A2 | `SceneData::load(path)` reads, parses and then validates each tween, naming `entities[i].tweens[j]` in `SceneError::Validation { path, message }`; `SceneError` is not `#[non_exhaustive]` | `scene.rs:11-25`, `:234-255` |
| A3 | `spawn_scene` interns sprite names (`D-113`), then queues per entry `Transform`, `Sprite`, `Visibility`, `Tag`, `SceneEntity` and the first `Tween` through `insert_pending`; it `expect`s `AssetRegistry` (only for an entry with a sprite) and `CommandBuffer` | `crates/tungsten/src/asset_loader/scene.rs:20-80` |
| A4 | `asset_loader::load_scene(path)` wraps `SceneData::load` in `anyhow`; nothing in the workspace calls it | `asset_loader/scene.rs:14-18`; `rg -n 'load_scene\(' --type rust` |
| A5 | Example 03, the only scene user, loads `scene.json` in `GameplayState::on_enter` and spawns it; its 25 entries use only known keys at every level | `examples/03_scene_state/src/states.rs:111-118`; `jq` over `examples/03_scene_state/assets/scene.json` |
| A6 | Two tests build `SceneEntry` by struct literal, on Worlds with no registry (one on `World::new()`) | `crates/tungsten/src/tests/asset_loader.rs:208-232`; `crates/tungsten/src/tests/tweens.rs:171-201` |
| A7 | The core scene tests call `SceneData::load(&path)` three times and never remove their temp directories (the scene half of a known issue) | `crates/tungsten-core/src/tests/assets/scene.rs:156`, `:173`, `:195`, `:202-208`; `docs/known-issues.md:61` |
| A8 | Any `'static` type is a component (`World::insert<T: 'static>`); resources sit in a `TypeId` map; `register_event` is idempotent and returns whether the type was new | `crates/tungsten-core/src/ecs/world.rs:18-30`, `:72-81`, `:158` |
| A9 | A pending spawn's inserts land in one archetype move at the flush (`D-084`), and a type inserted twice keeps the later value | `crates/tungsten-core/src/ecs/command_buffer.rs:202-236`; `crates/tungsten-core/src/ecs/bundle.rs:22-31` |
| A10 | A bundle tuple's elements are components, never bundles: registering `RigidBodyBundle` as a plain component would insert the bundle struct itself as one component | `bundle.rs:11-14` |
| A11 | `Plugin::build(&self, &mut Schedule, &mut World)`: a plugin reaches the World, not the App | `crates/tungsten-core/src/schedule.rs:487-495` |
| A12 | `InspectRegistry` is the precedent: a core World resource that plugins and games fill. `App::with_plugins` inserts its resources (that registry at `:252`), then builds the plugins (`:296`); `register_inspectable` and `register_event` delegate to the World | `crates/tungsten-core/src/inspect.rs:18-67`; `crates/tungsten/src/app.rs:206-259`, `:296`, `:308-312`, `:386-388` |
| A13 | A core test builds `PhysicsPlugin` on a bare `World::new()`, so a registration that needs an inserted resource would panic there | `crates/tungsten-core/src/tests/physics/plugin.rs:89-100` |
| A14 | No engine component derives serde, and `glam` has no `serde` feature; the scene format writes vectors as `[f32; 2]` | `crates/tungsten-core/src/components.rs:14`, `:51`, `:79`, `:91`, `:236`, `:283`; `crates/tungsten-core/src/physics/components.rs:8-96`; `Cargo.toml:53`; `scene.rs:204-211` |
| A15 | `RigidBodyBundle` puts `Position` and `PrevPosition` at one point, `Velocity` when it has one, `Collider` and `RigidBody`; `with_mass` stores `1/mass`, `with_restitution` clamps to 0..=1; default gravity is zero | `physics/components.rs:98-130`, `:140-203`; `crates/tungsten-core/src/physics/mod.rs:83` |
| A16 | Who reads each data-only engine component: `Light` the app's light extract and `ParallaxLayer` the default sprite extract (neither is a plugin), `SpriteSquashStretch` `GameFeelPlugin`'s systems, the body components `PhysicsPlugin`'s | `app.rs:1278`; `crates/tungsten/src/sprite_extract.rs:387-447`; `crates/tungsten/src/plugins.rs:192-216`; `crates/tungsten-core/src/physics/plugin.rs:26-40` |
| A17 | Engine components whose value names an asset: `ParticleEmitter.config` is an `AssetId<ParticleConfig>` that `ParticleConfigRegistry::id_for_name` resolves; `TilemapInstance.id` and `AnimationState.animation_id` are names | `components.rs:141-145`; `crates/tungsten-core/src/assets/particle.rs:535`; `crates/tungsten-core/src/assets/tilemap.rs:332-336`; `crates/tungsten-core/src/assets/animation.rs:127-128` |
| A18 | The umbrella crate has no `serde` dependency; core has `serde` and `serde_json` | `crates/tungsten/Cargo.toml` `[dependencies]`; `crates/tungsten-core/Cargo.toml:10-11` |
| A19 | serde 1 and serde_json 1, probed on 2026-10-13 in a scratch crate under `target/` (deleted): a unit struct reads `null` only (`{}` → "invalid type: map, expected unit struct"); a field of the wrong type reports the expected type, not the field ("invalid type: string \"ten\", expected u32"); unknown fields pass silently without `deny_unknown_fields`, which also works per variant on an internally tagged enum; a JSON object with a repeated key keeps the last value, in a `BTreeMap` and in a `Value`; a `try_from` error comes back as serde's message | the probe's output, quoted |
| A20 | The template spawns its player in code and loads no scene; its crate depends on `anyhow` and `tungsten` only | `templates/basic/src/states.rs:77-92`; `templates/basic/Cargo.toml:8-10` |
| A21 | The LLM index is 12,240 bytes of its 12,288-byte budget | `scripts/check-agent-context.py:25`; `just ctx` |
| A22 | `just repo-check` requires every file under an `assets/` folder to be in a manifest, with example 03's `scene.json` an explicit exception, so a new tracked scene file needs a checker change | `scripts/check-repo.py:28-34` |
| A23 | The snapshots list `SceneEntry`'s fields and `SceneData::load` at three paths, each `World` method twice, each `App` method twice, `load_scene` and `spawn_scene` once | `api/tungsten-core.txt:1346-1371`, `:4806`, `:14792`; `api/tungsten.txt:40`, `:110`, `:128`, `:964` |
| A24 | Nothing in `02_bench` or `app.rs` reads scenes, and plugins build at startup: no perf row runs a path this milestone changes | `rg -l 'SceneData\|spawn_scene' examples/02_bench crates/tungsten/src/app.rs` → none |
| A25 | `D-128` deferred W16a's registry, and `D-135` deferred a `Component` marker to W16a | `DECISIONS.md:1579`, `:1665` |
| A26 | Asset loading builds a fresh `ParticleConfigRegistry` and inserts it over the old one, so a component naming a config resolves it from the live World at spawn, never from a copy taken at registration | `crates/tungsten/src/asset_loader/mod.rs:374`, `:394` |
| A27 | `RigidBody::with_mass` stores `1.0 / mass` for any mass above zero, so a subnormal mass (`1e-40`) gives an infinite `inv_mass`, which the solver uses as is | `physics/components.rs:120-123`; `crates/tungsten-core/src/physics/step/mod.rs:532-561` |
| A28 | `just release-cut` rewrites `CHANGELOG.md`, `Cargo.toml`, `DESIGN.md`'s status line, the README's owned fields and `Cargo.lock` | `justfile:131-134`; `scripts/release.py:57`, `:296-306` |
| A29 | `D-103` clause (2): from 1.0, 1.x only adds fields to scenes and prefabs; a default that claims a component name a game already registered would fail that game's startup in a minor release | `DECISIONS.md:1226` |
| A30 | `serde_path_to_error` 0.1.20 (2025-09-15, MIT OR Apache-2.0, `rust-version` 1.61) depends on `itoa`, `serde` and `serde_core`, all in `Cargo.lock` already, so it adds one package; it is not in the local registry cache | crates.io API; `Cargo.lock` |
| A31 | The broadphase saturates cell coordinates to `i32` and subtracts them unchecked, so a finite collider such as a radius of `1e12` overflows in debug (a wrapped span in release), and nothing caps the cells one body covers; cells are at least 1 px, so an edge within ±2^25 stays in range at every cell size; known issues already carry the overflow as a P3 for coordinates written from code | `crates/tungsten-core/src/physics/broadphase.rs:84-88`, `:137-139`, `:320-333`; `docs/known-issues.md:16` |
| A32 | No scene transform is validated: JSON `1e40` reads as an infinite `f32`, and a body there interpolates to NaN (`prev + (pos - prev) * alpha`) | `scene.rs:204-211`, `:245-252`; `crates/tungsten-core/src/physics/plugin.rs:73-76` |
| A33 | The core snapshot spells the World `tungsten_core::World` (116 uses of `&tungsten_core::World`), the umbrella's `tungsten_core::ecs::world::World` | `api/tungsten-core.txt:5394`; `api/tungsten.txt:128` |

## 2. Design

### 2.1 The registry (core)

A new module, `tungsten_core::component_registry`, with its four types re-exported at the crate root; games reach them as `tungsten::core::…`. No file under `core/ecs/` changes: the two `World` methods sit in an `impl World` block in the new module and use only `World`'s public resource API.

```rust
/// The component types that scene entries (and W16b's prefab files) name in
/// their `components` object: a `World` resource keyed by name and by
/// `TypeId`, one name per type and one type per name.
#[derive(Default)]
pub struct ComponentRegistry { /* name → registration, sorted; TypeId → name */ }

impl ComponentRegistry {
    pub fn new() -> Self;

    /// `components.<name>` deserializes to `T`, inserted as is. Returns whether
    /// the pair is new; the same name and type again is a no-op.
    /// # Panics
    /// When `name` is not `[a-z][a-z0-9_]*`, already names another type, or `T`
    /// already has another name; the message names both types (`D-022`).
    #[track_caller]
    pub fn register<T: DeserializeOwned + 'static>(&mut self, name: &str) -> bool;

    /// `components.<name>` deserializes to `D`, and `build` turns it and the
    /// entity's `SpawnContext` into the bundle that spawns: for a definition that
    /// becomes several components or reads the entity's transform or the World.
    /// Keyed by `D`'s `TypeId`; panics as `register` does.
    #[track_caller]
    pub fn register_with<D, B>(&mut self, name: &str, build: impl Fn(D, &SpawnContext<'_>) -> B + 'static) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle;

    pub fn contains(&self, name: &str) -> bool;
    pub fn name_of<T: 'static>(&self) -> Option<&str>;
    /// The registered names, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;

    /// Whether `value` deserializes as the type registered under `name`.
    pub fn check(&self, name: &str, value: &serde_json::Value) -> Result<(), ComponentError>;

    /// Deserializes `value` as `name`'s type and builds what it spawns, owned
    /// and ready to queue: the caller prepares with `&World`, then queues with
    /// the `CommandBuffer`, so neither resource leaves the World.
    pub fn prepare(&self, name: &str, value: &serde_json::Value, at: &SpawnContext<'_>)
        -> Result<PreparedComponent, ComponentError>;
}

/// One registered component built for one entity. `Debug` prints its name.
pub struct PreparedComponent { /* name, boxed bundle */ }

impl PreparedComponent {
    /// Queues the built bundle on `pending`, one `insert_bundle_pending`.
    pub fn insert_pending(self, buffer: &mut CommandBuffer, pending: PendingEntity);
}

/// What a registered component's `build` sees of the entity it spawns on and
/// of the World it spawns into. Its fields are private, so it can grow.
pub struct SpawnContext<'w> { /* world: &'w World, transform */ }

impl<'w> SpawnContext<'w> {
    pub fn new(world: &'w World, transform: Transform) -> Self;
    /// The entity's transform: a scene entry's own.
    pub fn transform(&self) -> &Transform;
    /// The World, read only: where W16b's components resolve the asset IDs
    /// their values name (A26). None of this milestone's four reads it.
    pub fn world(&self) -> &'w World;
}

/// A `components` value that does not resolve.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ComponentError {
    /// Prints "unknown component `<name>`".
    Unknown { name: String },
    /// `field` is the path inside the value where deserialization stopped, as
    /// `serde_path_to_error` gives it (`max`, `shape.radius`), empty at the
    /// value's top. Prints "component `<name>`, field `<field>`: <source>", or
    /// "component `<name>`: <source>" when `field` is empty.
    Invalid { name: String, field: String, source: serde_json::Error },
}

impl World {
    /// `ComponentRegistry::register` on this World's registry, which it inserts
    /// first when the World has none, so a plugin built on a bare World registers.
    #[track_caller]
    pub fn register_component<T: DeserializeOwned + 'static>(&mut self, name: &str) -> bool;

    #[track_caller]
    pub fn register_component_with<D, B>(&mut self, name: &str, build: impl Fn(D, &SpawnContext<'_>) -> B + 'static) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle;
}
```

- **Values.** A component's value deserializes by serde's own rules, through `serde_path_to_error` so that a failure names its field (Q12): a struct from an object, a unit struct (a marker) from `null` (Q2), and the type's own `#[serde(deny_unknown_fields)]` decides whether an unknown field fails. `register`'s rustdoc says so, with a doctest. Checks beyond the types belong in deserialization (`#[serde(try_from = …)]`), so `build` is infallible, `check` is a deserialization alone, and `check` and `prepare` agree on every value.
- **Three doors to one registry.** `App::register_component` for `main.rs`; `World::register_component` and `register_component_with` for plugins (Q15), the way `World::register_event` serves events; the registry's own methods for a registry built without a World (a test, or W14b's `check`). `Debug` prints the names.
- **Conflicts panic at the call** (Q8): `Plugin::build` has no error path, and a harness test that builds the app catches the panic as it catches a schedule error.
- **What W16b adds** (A26): `SpawnContext::world` already lets a `build` resolve an asset ID from the live World at spawn, and the load already takes the World (§2.2), so W16b's registration form that also checks the IDs a value names extends this API without changing a signature here.
- `serde_json::Value` in a public signature ties core's API to serde_json 1, as `SceneError::Parse` already does.

### 2.2 Scene entries (core)

```rust
#[serde(deny_unknown_fields)] // on SceneData, SceneEntry, SceneTransform, SceneSprite and SceneTween (Q3)
pub struct SceneEntry {
    // transform, sprite, visible, tag, tweens: as today
    /// Registered components by name: each value deserializes as the type its
    /// name was registered with and spawns after the fields above, in name
    /// order. A name given twice fails the parse.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty", deserialize_with = "…")]
    pub components: BTreeMap<String, serde_json::Value>,
}

impl SceneData {
    /// Reads and parses `path`, then checks every tween, and every entry's
    /// components against `world`'s `ComponentRegistry` (an empty one when
    /// the World has none, so any component is unknown).
    pub fn load(path: &Path, world: &World) -> Result<Self, SceneError>;
}
```

- **The load takes the World** (Q16), not the registry alone: this milestone reads only the `ComponentRegistry` from it, and W16b's ID checks read the asset registries from the same World without another signature change (A26).
- **Component values parse strictly.** The `components` map's reader builds each value as a `serde_json::Value` but fails on a key repeated at any depth, where a plain `Value` keeps the last one (A19): `{"health": {"max": "ten", "max": 10}}` fails as `Parse`, naming `max` with its line and column, as a derived struct read from text would.
- **Transforms are checked** (Q17, A32): every number in an entry's `transform` finite, and `position` within ±16,777,216 (2^24, where an `f32` still holds whole pixels); a failure is `Validation` naming `entities[<i>].transform`. A component's `SpawnContext` therefore never carries a non-finite transform.
- **Errors name the file, the entry and the field** (criteria §8.7, `D-022`) in today's `SceneError` variants, with none added. A parse error (an unknown key; a repeated component name, "duplicate component `health`") is `Parse`, whose serde_json message carries the line and column. A component that fails its check is `Validation` with the message `entities[<i>]: <ComponentError>`, printed as ``invalid scene '<path>': entities[3]: unknown component `helth` ``, ``… entities[0]: component `health`, field `max`: invalid type: string "ten", expected u32`` or ``… entities[0]: component `health`: missing field `max` ``. Tweens keep `entities[i].tweens[j]: …`. The first error stops the load, as today (Q10).
- `SceneTweenChannel`, an internally tagged enum, parses as today; strictness covers the five structs.

### 2.3 Spawn and the umbrella

```rust
impl App {
    /// `World::register_component` on the app's world: `components.<name>`
    /// in a scene entry spawns a `T`.
    pub fn register_component<T: DeserializeOwned + 'static>(&mut self, name: &str);
}

/// `SceneData::load(path, world)`, with the path in the error.
pub fn load_scene(world: &World, path: &Path) -> anyhow::Result<SceneData>;

/// As today, then each entry's components.
/// # Panics
/// As today; and on a component this World's registry rejects (data that
/// skipped `SceneData::load`'s check against it), before anything is queued.
pub fn spawn_scene(world: &mut World, data: &SceneData, state_id: StateId);
```

- `App::with_plugins` inserts an empty `ComponentRegistry` beside `default_inspect_registry()`, registers the default extracts' components (step 2), then builds the plugins.
- `spawn_scene` runs in three passes. (1) With `&World`, it prepares every component of every entry, each with a `SpawnContext` of the World and the entry's `Transform`, and panics on the first that fails, naming `entities[<i>]` and the error, before anything changes (Q1). It reads the registry only when some entry has components, so a World without one still spawns a scene that names none (A6). (2) It interns the sprite names, as today. (3) It queues each entry as today and, after the `Tween`, each prepared component in name order. The entity still lands in one archetype move (A9); a registered type that a field also inserts keeps the component's value. No resource leaves the World.
- The umbrella gains `serde = { workspace = true }` for `App::register_component`'s bound (A18), and core `serde_path_to_error` (A30), the one crate that joins the graph.
- Example 03 loads through `asset_loader::load_scene(ctx.world, …)`. Its scene file is unchanged (A5), so its transition fixture must not move.

### 2.4 The engine's components (step 2)

Each registers where the code that reads it lives (Q11), through `register_component_with` and a private definition type with `deny_unknown_fields`; every number must be finite. The JSON shapes are file format under `D-103`.

| Name | Registered by | Value | Spawns |
| --- | --- | --- | --- |
| `rigid_body` | `PhysicsPlugin` | `kind`: `"dynamic"` or `"static"`, required. `shape`: `{ "kind": "aabb", "half_extents": [x, y] }` or `{ "kind": "circle", "radius": r }`, required, sizes in (0, 16,384]. `offset`: `[x, y]` within ±16,384, default `[0, 0]` (Q17: with the transform's range, a body stays inside the broadphase's `i32` cells, A31). `mass` > 0 with `1 / mass` finite, so a subnormal mass such as `1e-40` fails (A27); default 1, dynamic only: a static body that names it fails. `restitution` in 0..=1, default 0. `velocity`: `[x, y]`, default `[0, 0]` on a dynamic body and none on a static one | `RigidBodyBundle` at the entry's `transform.position`: `Position` and `PrevPosition` there, `Collider`, `RigidBody` (`inv_mass` from `mass`), `Velocity`; the transform's scale does not size the collider |
| `light` | `App`, beside the light extract | `{ "kind": "point", "radius": r, "color": [r, g, b], "intensity": i }` or `{ "kind": "directional", "angle": a, "color": [r, g, b], "intensity": i }`; `radius` > 0, colour channels ≥ 0, `intensity` ≥ 0 and default 1 | `Light` |
| `parallax_layer` | `App`, beside the sprite extract | `{ "scroll_factor": [x, y] }` | `ParallaxLayer` |
| `squash_stretch` | `GameFeelPlugin` | `{ "on": …, "amount": [x, y], "duration": d, "easing": … }`: `on` one of `on_land`, `on_hit`, `on_pickup` and `manual`; `amount` > 0 on both axes; `duration` > 0; `easing` as tweens name it, default `linear` | `SpriteSquashStretch` |

- The core definition lives in `physics/plugin.rs`, the umbrella's three in `asset_loader/scene.rs` beside `spawn_scene`, so no new file needs an index route (A21). `component_registry`'s module doc lists the four names, owners and shapes.
- `App::with_plugins(PluginSet::new())` keeps `light` and `parallax_layer`, since its extracts still draw them; a set without `PhysicsPlugin` or `GameFeelPlugin` drops their names, as it drops their systems and events.

### 2.5 The seam and the template

- Everything here is core and umbrella CPU code: no render type, and the extract still reads `&World` (`D-007`, `D-018`). The registry is data the scene path reads; no system and no stage changes.
- The template does not change (Q5). It spawns no scene (A20), so nothing it shows changes; W16b, whose done-when moves its player and enemy onto prefabs, registers `Player` then, with the template's first `serde` dependency. The getting-started guide waits for W9b (workflow §5 line 6).

## 3. Steps

### Step 1: The registry, `components` in scene entries and the public API

- **Files:** `crates/tungsten-core/src/component_registry.rs` (new), `crates/tungsten-core/src/tests/component_registry.rs` (new), `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/assets/scene.rs`, `crates/tungsten-core/src/tests/assets/scene.rs`, `Cargo.toml`, `crates/tungsten-core/Cargo.toml`, `crates/tungsten/Cargo.toml`, `Cargo.lock`, `licenses/clarifications.json` and `licenses/serde_path_to_error/` (only if `just notices` asks for a licence text), `crates/tungsten/src/app.rs`, `crates/tungsten/src/asset_loader/scene.rs`, `crates/tungsten/src/tests/asset_loader.rs`, `crates/tungsten/src/tests/tweens.rs`, `examples/03_scene_state/src/states.rs`, `api/tungsten-core.txt`, `api/tungsten.txt`
- **Change:** in order: (a) `serde_path_to_error = "0.1"` in `[workspace.dependencies]` and core's `[dependencies]` (network for the first fetch; A30); (b) §2.1's module, its tests through `#[path = "tests/component_registry.rs"]`, the root re-exports; (c) §2.2 in `scene.rs` (strict keys, the strict component reader, the transform checks, the load against the World), and the core scene tests: the three `load` calls take `&World::new()`, and each test removes its temp directory at its end (Q13); (d) §2.3: the registry resource and `App::register_component`, `load_scene`, `spawn_scene`'s three passes, the umbrella's `serde` dependency, `components: Default::default()` in the two literals (A6), example 03's load; (e) rustdoc on every new public item, and a doctest on `World::register_component` with a game component and its JSON; (f) `just api`, and its diff read against §2.
- **Tests:** Core registry: one registration per name and type, the second call `false` and a no-op; each conflict, and a name outside `[a-z][a-z0-9_]*`, panics naming the types; `check` returns `Unknown`, and `Invalid` for a missing field (an empty `field`, serde's message naming it), for an unknown field under `deny_unknown_fields`, for a field of the wrong type (``field `max` ``) and for a wrong type one struct down (``field `inner.max` ``, or the dotted path `serde_path_to_error` prints); a unit struct reads `null` and refuses `{}`; `register_with` builds from the `SpawnContext`'s transform and can read a resource through `world()`; `prepare` builds without touching the buffer, and `insert_pending` queues one bundle; `World::register_component` inserts the registry when missing, and the World's and the registry's doors fill one registry. Core scene: `components` parse, default to empty and round-trip without the key when empty; a repeated component name, a key repeated inside a component's value (`{"max": "ten", "max": 10}`, where the valid second value would mask the first) and an unknown entry key fail as `Parse`; a transform position of `1e40` (read as infinite) and one past 2^24 fail as `Validation` naming `entities[0].transform`; `load` fails as `Validation` naming the path and ``entities[1]: unknown component `helth` ``, the path and ``entities[0]: component `health`, field `max`: invalid type: string "ten", expected u32``, and the path and ``entities[0]: component `health`: missing field `max` ``; a World without a registry finds every component unknown; a registered component passes. Umbrella: after one flush a spawned entry carries its components beside `Transform`, `Visibility` and `SceneEntity`; a registered type that a field also inserts keeps the component's value; an unregistered name panics naming `entities[0]` and leaves the `CommandBuffer` empty and no sprite name interned (`catch_unwind`); `load_scene` uses the World's registry; on the harness, `App::register_component` plus a scene file in a temp directory spawns the component, and the state's exit despawns the entity.
- **Done-when:**
  - `cargo test -p tungsten-core --locked component_registry` → `test result: ok`, with the registry tests above
  - `rm -rf "${TMPDIR:-/tmp}"/tungsten_scene_*; cargo test -p tungsten-core --locked assets::scene; fd -d 1 -t d '^tungsten_scene_' "${TMPDIR:-/tmp}" | wc -l` → `test result: ok`, then `0`
  - `cargo test -p tungsten --locked asset_loader` and `cargo test -p tungsten --locked tweens` → `test result: ok` each; `cargo test -p example-03-scene-state --locked` → `test result: ok`, its pixel comparison not run without `TUNGSTEN_VISUAL_REGRESSION=1` (the release runs `just visual`)
  - `cargo test -p tungsten-core --test manifests --locked` (layer 1) → `test result: ok`
  - `cargo check --workspace --all-targets --locked` → finishes with no error: every caller of the changed signatures is updated
  - `rg -c '^serde = \{ workspace = true \}' crates/tungsten/Cargo.toml` → `1`; `rg -c '^serde_path_to_error = \{ workspace = true \}' crates/tungsten-core/Cargo.toml` → `1`; `rg -c '^name = "serde_path_to_error"$' Cargo.lock` → `1`; `rg -c '^\[\[package\]\]' Cargo.lock` is one more than `git show HEAD:Cargo.lock | rg -c '^\[\[package\]\]'`; `just deps` → passes; `just notices` → passes
  - after `just api`, in `api/tungsten-core.txt`: `rg -c '^pub struct tungsten_core::(component_registry::)?ComponentRegistry$'` → `2`; the same pattern for `PreparedComponent` → `2`; `rg -c '^pub struct tungsten_core::(component_registry::)?SpawnContext<.[a-z_]+>$'` → `2`; `rg -c '^#\[non_exhaustive\] pub enum tungsten_core::(component_registry::)?ComponentError$'` → `2` (corrected in the run: the snapshot prints `#[non_exhaustive] ` before each of its 51 such enums, so the plan's `^pub enum …` read 0 on a correct snapshot); `rg -c '^pub tungsten_core::(assets::(scene::)?)?SceneEntry::components: '` → `3`; `rg -c '^pub fn tungsten_core::World::register_component(_with)?<'` → `4`; `rg -c '^pub fn tungsten_core::assets::scene::SceneData::load\(&std::path::Path, &tungsten_core::World\)'` → `3` (the core snapshot's spelling, A33). In `api/tungsten.txt`: `rg -c '^pub fn tungsten::app::App::register_component<'` → `2`; `rg -c '^pub fn tungsten::asset_loader::load_scene\(&tungsten_core::ecs::world::World, &std::path::Path\)'` → `1`
  - `rtk proxy git --no-pager diff --unified=0 -- api/ | rg -c '^-pub'` → `4`: the three `SceneData::load` lines and `load_scene`, so nothing else leaves the surface; `just api-check` → passes
- **Then:** the level C API critique from the skill's "Running steps", in the background. Check each finding, record it in §9 as `C.<n>`, and change step 1 where one holds; a change to the sketch updates §2 and these counts in the same edit, and the done-when above runs again before step 2.
- **Moves:** none.

### Step 2: The engine's data-only components

- **Files:** `crates/tungsten-core/src/component_registry.rs` (module doc), `crates/tungsten-core/src/physics/plugin.rs`, `crates/tungsten-core/src/tests/physics/plugin.rs`, `crates/tungsten/src/asset_loader/scene.rs`, `crates/tungsten/src/asset_loader/mod.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/plugins.rs`, `crates/tungsten/src/tests/plugins.rs`, `crates/tungsten/src/tests/asset_loader.rs`
- **Change:** §2.4. Before the step, also copy `api/tungsten-core.txt` and `api/tungsten.txt` to the scratchpad. `rigid_body`'s definition and its registration in `PhysicsPlugin::build`; the umbrella's three definitions in `asset_loader/scene.rs`, made reachable to `plugins.rs` through `asset_loader/mod.rs`; `light` and `parallax_layer` registered by `App::with_plugins` before the plugins build, `squash_stretch` by `GameFeelPlugin::build`; the four names, owners and shapes in `component_registry`'s module doc.
- **Tests:** Core: `PhysicsPlugin` registers `rigid_body` on a bare World; a dynamic body spawns at the transform's position with `PrevPosition` equal to it, zero `Velocity`, the shape and offset, `inv_mass` 0.5 for mass 2 and the given restitution; a static one has no `Velocity` unless one is given; a mass on a static body, a mass ≤ 0, a subnormal mass (`1e-40`, whose inverse overflows), a size ≤ 0, a radius of `1e12` and an offset past 16,384 (A31), a restitution outside 0..=1, an unknown field and an unknown body or shape kind each fail `check` with a message naming the field or value. Umbrella: `App::new`'s registry names read `light, parallax_layer, rigid_body, squash_stretch`, and `light, parallax_layer` without `PhysicsPlugin` and `GameFeelPlugin` and with the empty set; each umbrella shape spawns its component with the given values, and its bad values fail; on the harness, a scene file naming all four spawns them, and after one fixed step a dynamic body with a velocity has moved while a static body has not.
- **Done-when:**
  - `cargo test -p tungsten-core --locked physics::plugin` → `test result: ok`, `physics_plugin_alone_resolves_to_the_snapshot_the_step_and_the_sync` among the passes
  - `cargo test -p tungsten --locked plugins` and `cargo test -p tungsten --locked asset_loader` → `test result: ok` each
  - `cmp <(git show HEAD:crates/tungsten/src/tests/plugins.rs | sed -n '/^const DEFAULT_SCHEDULE/,/^";$/p') <(sed -n '/^const DEFAULT_SCHEDULE/,/^";$/p' crates/tungsten/src/tests/plugins.rs)` → exit 0: no system moved
  - `cargo check --workspace --all-targets --locked` → finishes with no error
  - `just api`, then `cmp` of each `api/` file against its copy from before the step → identical (the definitions are private); `just api-check` → passes
- **Moves:** none.

### Step 3: Records

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/w16-prefabs.md`, `docs/plans/1.0/w14-tooling.md`, `docs/plans/1.0/implementation-plan.md` (the W6a and W16b cards only)
- **Change:** in order:
  - (a) Recheck the next free ID (`rg -n '^## D-1' DECISIONS.md | tail -2`; `git status --short` for another session's entries). With the tungsten-decision skill, decision 153, "W16a landed (M48)": §2's choices with the open questions' outcomes and step 1's critique changes, the tests that pin it, the umbrella's `serde` edge, and "`serde_path_to_error` satisfies `D-015` rule 3" (a solved primitive, A30). It amends `D-046` choice 5 (a scene entry also carries registered components, the load takes the World, unknown keys fail) and `D-103` clause (2) (Q7: the default component names are fixed from 1.0), with a marker line on each and "Amended by" in both index rows; its own row goes in ECS / Runtime Flow after `D-149`. It settles the registry `D-128` deferred and the marker `D-135` deferred (none in 1.0), with no marker line on either, as `D-133` did for `D-128`.
  - (b) Three break-ledger rows, each from "W16a (M48 …)", landed with 153 in 0.67 (M48): `SceneEntry` gains `components`, so a struct literal needs the field (the workspace's two gained `components: Default::default()`); `SceneData::load` and `asset_loader::load_scene` take the `World`; scene files reject unknown keys, repeated component names, keys repeated inside a component's value and transforms that are not finite or lie past ±2^24, a behaviour change, not a compile break.
  - (c) `DESIGN.md`: a `### Component Registry — M48` section after `### UI Input Routing — M47` (the registry, its three doors, the load's check and its errors, the spawn order, the four engine names and owners, what W16b reuses); in the M20 section, "each `SceneEntry` maps to `Transform + Sprite? + Visibility + Tag?`" gains "and registered components (M48)".
  - (d) The LLM index's scene row becomes `| State stack, scenes and components, `SceneEntity` cleanup, transitions (`D-046`, `D-093`, <153>) | `tungsten/state.rs`, `tungsten/transition.rs`, `core/assets/scene.rs`, `core/component_registry.rs`, `tungsten/asset_loader/scene.rs`, `examples/03_scene_state/src/states.rs` |`, with 153's full ID in place of `<153>`: 42 bytes more, 12,282 of 12,288. No other row changes; if it does not fit, shorten that row's task words.
  - (e) Known issues: the temp-directory line keeps only the manifest helper (`core/tests/assets/manifest.rs`).
  - (f) `w16-prefabs.md`: the placement notes W16a's landing (153, 0.67); Follow-ups gains the registration form W16b adds, its two registrations and the shared prepare-then-queue spawn path (this plan's first follow-up).
  - (g) `w14-tooling.md`, follow-up 2: `SceneData::load` stops at the first error and checks against the World it is given; `check` reports every error and needs the game's registered names, which only the game's code holds, so W14b decides how it gets them.
  - (h) The W16b card's Touches cell gains "the registration form that checks the asset IDs a component names, then `particle_emitter` and `tilemap` on it (M48)"; the W6a card's gains "the `rigid_body` scene shape's new kinds (M48)".
- **Done-when:**
  - `rg -c '^## D-15[3] ' DECISIONS.md` → `1`; ``rg -c '^\| `D-15[3]` ' docs/DECISION_INDEX.md`` → `1`; `rg -c '^\*\*Amended by D-15[3]:\*\*' DECISIONS.md` → `2`; ``rg -c '^\| `D-(046|103)` .*Amended by `D-15[3]`' docs/DECISION_INDEX.md`` → `2`; `rg -A3 '^## D-15[3] ' DECISIONS.md | rg -c 'D-015\W? rule 3'` → `1` (the `[3]` keeps the ID unwritten here until its entry exists; another number if (a) moved it)
  - `rg -c '\| W16a \(M48' docs/plans/1.0/w04-api-freeze.md` → `3`
  - `rg -c '^### Component Registry — M48$' DESIGN.md` → `1`; `rg -c 'and registered components \(M48\)' DESIGN.md` → `1`
  - `rg -c 'core/component_registry.rs' docs/LLM_INDEX.md` → `1`; `just ctx` → `Agent context checks: OK`
  - `rg -c 'core/tests/assets/scene.rs' docs/known-issues.md` → `0`; `rg -c 'core/tests/assets/manifest.rs' docs/known-issues.md` → `1`
  - `rg -c 'M48' docs/plans/1.0/w16-prefabs.md` → at least `1`; `sed -n '/^## Follow-ups/,$p' docs/plans/1.0/w14-tooling.md | rg -c '^2\. '` → `1` (its Steps list already has a `2.`); `rg -c 'particle_emitter' docs/plans/1.0/implementation-plan.md` → `1`
  - `just repo-check` → passes
- **Moves:** none.

### Step 4: Release 0.67

- **Files:** `CHANGELOG.md`, `Cargo.toml`, `Cargo.lock`, `DESIGN.md` (the status line `just release-cut` rewrites, A28), the register row (`docs/plans/1.0/implementation-plan.md` §10), the README's four owned Now fields (`roadmap.py summary`, also rewritten by the cut), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 (§8) and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.67.0` once, every release check once after it with `just physics-release` (step 2 edits `core/physics/plugin.rs`) and `just visual` (example 03's scene spawns through the changed path), then the preflight with `--message`
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just physics-release` → passes, printing the pinned determinism hash `0x088ec07a73c1b168` and containment hash `0xaee272e01ffc4e4c`; `just visual` → every comparison passes; `just release-preflight 0.67.0 --repo JosephGibson/Tungsten --message 'Update 0.67: M48 component registry'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for the next register row, W6a ([tungsten-next's prompts](../../../../.claude/skills/tungsten-next/prompts.md), `plan` row: `docs/plans/1.0/criteria.md §8.1 (W6)`, `phase6-milestone-49-kinematic-sensors-queries.md`)

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | A component the World's registry rejects at spawn, data that skipped `SceneData::load`'s check against it: panic, or an error from `spawn_scene`? | Panic before queueing anything, naming `entities[<i>]` and the error, and keep `spawn_scene`'s signature (`D-022`). The load is where data fails; the registry only grows, so a scene loaded against the World's registry always spawns |
| Q2 | A marker, a unit struct, in JSON: serde's `null`, or `{}` coerced? | Serde's rule, `"player": null`, with no coercion; rustdoc says so, and a braced `struct Player {}` reads `{}` |
| Q3 | Unknown keys in a scene file: fail, or keep dropping them? | Fail: `deny_unknown_fields` on the five scene structs, since a misspelt `components` would otherwise drop every component silently (A1); a ledger row |
| Q4 | Which engine components register now? | The four data-only ones of §2.4. `ParticleEmitter`'s value has to become an `AssetId` and `TilemapInstance` names a map, so `particle_emitter` and `tilemap` go to W16b's card, with the registration form that checks those IDs against the World the load takes (Q16); `AnimationState` belongs to W13a's animated sprite |
| Q5 | Does the template change? | No (§2.5): it spawns no scene, and W16b moves its player and enemy onto prefabs |
| Q6 | A `Component` marker trait, which `D-135` left to W16a? | None in 1.0: the registration bound is `DeserializeOwned + 'static`, any `'static` type stays a component and bundles nest through `with`; a marker would break every component type for nothing the registry needs |
| Q7 | Names: what rule, one namespace, and how do later releases add names? | `[a-z][a-z0-9_]*` in one flat set with no prefix. From 1.0 the names that `App` and `DefaultPlugins` register are fixed: a minor release adds a component name only in a plugin a game adds itself, since a new default could take a name the game registered and fail its startup, which `D-103` clause (2) rules out (A29); a major release may add defaults. Until the freeze, each new default name is a ledger row. 153 amends `D-103` clause (2) to say so |
| Q8 | Registration conflicts: panic, or an error later? | Panic at the registering call (`D-022`), since `Plugin::build` returns nothing; `D-128`'s resolve errors stay with systems |
| Q9 | The card's "one entry with W16b" | 153 records the registry and scene `components`; W16b's prefab entry amends it with a marker line, since a settled entry is immutable |
| Q10 | Load errors: the first, or all of them? | The first, as tween validation does; W14b's `check`, which reports every error, gets a collecting form (step 3's follow-up) |
| Q11 | Where do the engine's components register? | Where the code that reads them lives: `PhysicsPlugin` and `GameFeelPlugin` theirs, `App::with_plugins` the default extracts' `light` and `parallax_layer` |
| Q12 | Name the field inside a component when its value is of the wrong type? | Yes, as criteria §8.7 asks: serde's own message names a missing or unknown field but only the expected type for a mismatch (A19), so core deserializes through `serde_path_to_error` (`D-015` rule 3, one crate whose dependencies are already in the graph, A30) and `ComponentError::Invalid` carries the path |
| Q13 | Fix the scene half of the temp-directory known issue? | Yes: step 1 edits each of those tests; the manifest helper's half stays in known issues |
| Q14 | `load_scene`'s new signature: `(world, path)` or `(path, registry)`? | `(world, path)`, beside `spawn_scene(world, …)`, so a state loads with what its world registered |
| Q15 | Plugins register through `World` methods, or through `world.resource_mut::<ComponentRegistry>()`? | `World` methods defined in the registry's module, which insert the registry when missing: they mirror `register_event`, work on a bare World (A13) and change no file under `core/ecs/` |
| Q16 | What does `SceneData::load` take, the registry or the World? | The World: its registry now, and the asset registries W16b's ID checks read, with no second change to the signature (A26) |
| Q17 | How far may a scene's numbers go? | Every number finite; transform positions within ±2^24 px; `rigid_body` sizes in (0, 16,384] and offsets within ±16,384, which bodies (actors and props; level geometry is tile collision) never approach and which keep a scene's bodies inside the broadphase's `i32` cells (A31). Widening a limit later is additive; the broadphase's own guard stays the known issue's P3 |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

153: W16a landed (M48): the `ComponentRegistry`, `components` in scene entries checked at load against the World with field paths, strict scene keys, the spawn order, the engine's data-only components by their owners; no `Component` marker; `serde_path_to_error` under `D-015` rule 3. Amends `D-046` (choice 5) and `D-103` (clause 2: the default component names are fixed from 1.0); settles what `D-128` and `D-135` deferred, with no marker line on either. Next free when written; step 3 checks it again.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash, the pinned containment hash, the row digests, `gpu-visual.png`, the post and transition regressions (example 03's scene is unchanged and spawns through the changed path), the UI overlay regression, `DefaultPlugins`' resolved schedule (`DEFAULT_SCHEDULE`) and the template's and examples' schedule snapshots.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Step 2 needs step 1; step 3 records what landed, a skip included.
- A step needs a file outside its list, a change under `crates/tungsten-core/src/ecs/`, a new system or stage, or a change to a per-frame path in `app.rs`: stop that step. The card has no perf row, and shared ECS or frame-loop code would owe every CPU row (workflow §5 line 2).
- A crate other than `serde_path_to_error` joins the dependency graph, or a step needs a feature on an existing crate (glam's `serde`, serde_json's `raw_value`): stop; that needs a `D-015` decision this plan does not make.
- An invariant in §6 moves: stop the step and restore it.
- Step 1's API critique raises a finding that holds but needs a file outside step 1's list: apply what fits step 1, and record the rest as an open question with its default.
- `git status --short` before a step shows another session's change in a file the step edits: stop that step (workflow §6.1).
- The next free decision ID is not 153 when step 3 runs: take the next free one and use it wherever this plan says 153.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1, 2, release | Step 1 runs `just deps` and `just notices` for `serde_path_to_error` and the umbrella's `serde` edge. No perf row: nothing a row runs changes (A24), and no step changes `core/ecs/`, a stage or a per-frame path. The release runs `just check`, `just smoke`, `just script-test` and the rest of the release checks once, after the cut, with `just physics-release` and `just visual` |
| 3 Invariants | 2, release | Step 2 pins `DEFAULT_SCHEDULE`; the release's `just physics-release` and `just visual` pin the hashes and the regressions |
| 4 Decisions | 3 | 153 with its index row, the `D-015` rule for `serde_path_to_error`, and the marker lines and row notes on `D-046` and `D-103`, before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 3 | Step 1: rustdoc on every new item, the doctest, `just api` and its counts; the snapshot diff lists the new surface. Step 3: the three ledger rows. The four JSON shapes, file format under `D-103`, are listed in `component_registry`'s module doc and `DESIGN.md` |
| 6 Template and guide | — | n/a: the template spawns no scene (Q5); the guide waits for W9b |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 3 | The scene row names `core/component_registry.rs` within the budget; no `AGENTS.md` rule |
| 8 Design | 3 | `### Component Registry — M48` and the M20 section's line |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 3, release | Known issues lose the scene half of the temp-directory line. No backlog row: nothing goes to 1.x, and the asset-naming components move to W16b's card (Q4). The gap log has no rows and no game repository exists yet. W16 is a skeleton, so `w16-prefabs.md` takes the placement note and follow-ups; the W6a and W16b cards and W14's follow-up 2 take theirs. The register row and the README's owned fields come at the release; `python3 -B scripts/roadmap.py summary` supplies the fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra`, 2026-10-13, on the plan as written at `c638b29` (uncommitted): `critique: model gpt-6-astra, effort max, context 872000, mode repo, 553.5 s, tokens in 1408391 (cached 1262080), out 13675 (reasoning 9275), usage 5h 1% → 35%, week 0% → 6%`. Five findings, all MEDIUM; all five held. They changed done-when checks in steps 1–4, so round 2 ran on the revised plan at xhigh.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] W16b cannot add its promised registrations through the proposed API alone: `check` is deserialization only, `build` sees only the transform, and `ParticleEmitter.config` needs the live `ParticleConfigRegistry`, which asset loading replaces | `crates/tungsten/src/asset_loader/mod.rs:374`, `:394` (A26); `crates/tungsten-core/src/assets/particle.rs:535` | Accepted | §2.1: `SpawnContext<'w>` carries the World (`world()`), and `prepare` plus `PreparedComponent` replace `insert_pending`, so a spawn prepares with `&World` and queues after with no resource taken out; §2.2: `SceneData::load` takes the World (Q16); §2.3's three passes; the W16b follow-up and card text name the registration form W16b adds |
| 2 | [MEDIUM] Q12 drops the explicit requirement to name the invalid field: a wrong-type field gets only "invalid type … expected u32" | `docs/plans/1.0/criteria.md:183`; the serde probe (A19) | Accepted | Q12 now yes: `serde_path_to_error` (`D-015` rule 3, one new package, A30) and `ComponentError::Invalid { field }`; step 1's files, change (a), tests (a wrong type at the top and one struct down) and done-when (`just notices`, the lockfile counts); §7's dependency stop allows only this crate |
| 3 | [MEDIUM] Q7 lets a later engine or kit release take a game's registered name and fail its startup, against `D-103` | `DECISIONS.md:1226` (A29) | Accepted | Q7: the default names (`App`'s and `DefaultPlugins`') are fixed from 1.0, and a minor adds names only in plugins a game adds itself; 153 amends `D-103` clause (2), with step 3's marker and row checks |
| 4 | [MEDIUM] A finite positive mass such as `1e-40` still gives an infinite `inv_mass`, which the solver uses | `crates/tungsten-core/src/physics/components.rs:120-123`; `crates/tungsten-core/src/physics/step/mod.rs:532-561` (A27) | Accepted | §2.4: `mass` > 0 with `1 / mass` finite; step 2's tests reject a subnormal mass |
| 5 | [MEDIUM] Step 4 leaves out `DESIGN.md`, which `just release-cut` rewrites, so the stop rule blocks the release or a rollback leaves its status line behind | `scripts/release.py:57`, `:296-306`; `justfile:131-134` (A28) | Accepted | Step 4 and the header's files list `DESIGN.md` |

Round 2: the same reviewer at xhigh, 2026-10-13, on the revised plan: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo, 435.4 s, tokens in 1622995 (cached 1484160), out 11748 (reasoning 7149), usage 5h 35% → 70%, week 6% → 11%`. Four findings, all MEDIUM; all four held and were applied with no third round, so none stays open.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Accepted collider sizes can overflow the physics grid: a finite radius of `1e12` saturates the cell coordinates and overflows `SpatialGrid::insert`'s subtraction in the first debug step | `crates/tungsten-core/src/physics/broadphase.rs:84-88`, `:137-139`, `:320-333` (A31) | Accepted | §2.4: sizes in (0, 16,384] and offsets within ±16,384, with the transform range of 2.2 (Q17); step 2 tests a `1e12` radius and a large offset. The broadphase's own guard stays the existing P3 (`docs/known-issues.md:16`), which code paths reach too |
| 2.2 | [MEDIUM] A rigid body's initial position bypasses the finiteness checks: `1e40` in a transform reads as infinity and interpolates to NaN | `crates/tungsten-core/src/assets/scene.rs:204-211`, `:245-252`; `crates/tungsten-core/src/physics/plugin.rs:73-76` (A32) | Accepted | §2.2: the load checks every transform (finite, positions within ±2^24) as `Validation` naming `entities[<i>].transform`; step 1's tests; the strictness ledger row |
| 2.3 | [MEDIUM] Parsing component values through `Value` keeps the last of a repeated key, so a valid repeat masks an invalid earlier value that a derived struct would reject | the serde probe (A19) | Accepted | §2.2: a strict reader for component values fails on a key repeated at any depth (`Parse`, with line and column); step 1's test for the masking case |
| 2.4 | [MEDIUM] Step 1's core check spells the World as the umbrella does, so it would read 0 on a correct snapshot | `api/tungsten-core.txt:5394`; `api/tungsten.txt:128` (A33) | Accepted | Step 1's `SceneData::load` count matches `&tungsten_core::World` |

API critique (level C, after step 1): the `critique` skill with `--raw --repo --force` on step 1's `api/` and `templates/` diff, 2026-10-13: `critique: model gpt-6-astra, effort xhigh, context 872000, mode repo, 117.3 s, tokens in 344703 (cached 266240), out 2736 (reasoning 1359), usage 5h 70% → 82%, week 11% → 13%`. Its verdict was "No concerns" (registration conflicts, type-to-name lookup, component validation, preparation before queueing and the migrated scene-loading callers checked; the template has no scene-loading call), so no `C.<n>` finding and no change to step 1.

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-13 | Done | `cargo test -p tungsten-core --locked component_registry` → `test result: ok. 14 passed` (doctests: `--doc component_registry` → `ok. 2 passed`); scene: `test result: ok. 20 passed`, then `0` temp dirs; `cargo test -p tungsten --locked asset_loader` → `ok. 23 passed`, `tweens` → `ok. 10 passed`; `cargo test -p example-03-scene-state --locked` → `ok. 2 passed` and `ok. 1 passed` (pixel comparison not run without `TUNGSTEN_VISUAL_REGRESSION=1`); `--test manifests` → `ok. 3 passed`; `cargo check --workspace --all-targets --locked` → exit 0; Cargo counts `1`, `1`, `1`, packages 405 against 404; `just deps` → `advisories ok, bans ok, licenses ok, sources ok`; `just notices` → exit 0 (206 and 179 crates, no licence text asked); API counts `2`, `2`, `2`, `2` (the enum check as written read 0, corrected above), `3`, `4`, `3`, `2`, `1`; `^-pub` → `4` (three `SceneData::load`, `load_scene`); `just api-check` → `Public API snapshots current: 3 crates`; `just lint` → exit 0. The two struct literals gained `components: std::collections::BTreeMap::new()`, since clippy's `default_trait_access` refuses `Default::default()`; `licenses/` unchanged. API critique: no concerns | `crates/tungsten-core/src/component_registry.rs`, `crates/tungsten-core/src/tests/component_registry.rs`, `crates/tungsten-core/src/assets/scene.rs`, `api/` |
| 2 | 2026-10-13 | Done | `cargo test -p tungsten-core --locked physics::plugin` → `test result: ok. 9 passed`, `physics_plugin_alone_resolves_to_the_snapshot_the_step_and_the_sync ... ok` among them; `cargo test -p tungsten --locked plugins` → `ok. 11 passed`, `asset_loader` → `ok. 26 passed`; `DEFAULT_SCHEDULE` `cmp` → exit 0; `cargo check --workspace --all-targets --locked` → exit 0; `just api`, then `cmp` of both snapshots against their pre-step copies → identical; `just api-check` → `Public API snapshots current: 3 crates`; `just lint` → exit 0 (after `RigidBodyDef` became `Copy` for `needless_pass_by_value`). Registry names: `light, parallax_layer, rigid_body, squash_stretch` under `App::new`, `light, parallax_layer` without physics and game feel and with the empty set | `crates/tungsten-core/src/physics/plugin.rs`, `crates/tungsten/src/asset_loader/scene.rs`, `crates/tungsten/src/tests/asset_loader.rs`, `crates/tungsten/src/tests/plugins.rs` |
| 3 | 2026-10-13 | Done | Next free ID was `D-153` (`rg -n '^## D-1' DECISIONS.md | tail -2` → `D-151`, `D-152`; no other session's entries). `^## D-15[3] ` → `1`; its index row → `1`; marker lines → `2`; `D-046` and `D-103` rows with "Amended by `D-153`" → `2`; `D-015 rule 3` within the heading's next three lines → `1`; ledger `W16a (M48` → `3`; `### Component Registry — M48` → `1`, the M20 line → `1`; `core/component_registry.rs` in the index → `1`, 12,282 of 12,288 bytes, `just ctx` → `Agent context checks: OK`; known issues: scene `0`, manifest `1`; `w16-prefabs.md` `M48` → `2`; `w14-tooling.md` follow-up `2.` → `1`; `particle_emitter` in the implementation plan → `1`; `just repo-check` → exit 0. The first ledger row names `components: std::collections::BTreeMap::new()`, what step 1 landed | `DECISIONS.md`, `docs/DECISION_INDEX.md`, `docs/plans/1.0/w04-api-freeze.md`, `DESIGN.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/w16-prefabs.md`, `docs/plans/1.0/w14-tooling.md`, `docs/plans/1.0/implementation-plan.md` |
| 4 | 2026-10-13 | Done (closed before the cut) | Workflow §5: (1) steps 1–3 quoted above; (2) the release checks, `just physics-release` and `just visual` run once after the cut, quoted in the session's report, since the plan is archived before it; (3) invariants pinned by step 2's `DEFAULT_SCHEDULE` `cmp` and by the release's physics and visual checks; (4) `D-153` and its row before the docs that cite it; (5) three ledger rows, rustdoc on every new item, `just api` reviewed at steps 1 and 2; (6) n/a (Q5); (7) the index row, `just ctx` OK; (8) `### Component Registry — M48`; (9) known issues, `w16-prefabs.md`, `w14-tooling.md`, the W6a and W16b cards, the register row (`Cut 2026-10-13 as v0.67.0`), the README's owned fields by the cut; (10) one `[Unreleased]` entry with its summary, `status: done`, this plan archived. `DESIGN.md`'s status line: Phase 6 in progress, scene components added | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/archive/1.0/phase6-milestone-48-component-registry.md` |

## Follow-ups

- W16b: add the registration form whose check also resolves the asset IDs a value names, against the World that `SceneData::load` and the prefab section's load take, and register `particle_emitter` (its config named by manifest ID, resolved through `SpawnContext::world` at spawn since asset loading replaces the registry, A26) and `tilemap` (`TilemapInstance`) on it; spawn prefab entries through this milestone's prepare-then-queue path (on W16b's card and in `w16-prefabs.md` from step 3).
- W6a: `rigid_body`'s `kind` gains the new body kinds and sensors (on W6a's card from step 3).
- W14b: `check` reports every error and needs the game's registered names (`w14-tooling.md` follow-up 2 from step 3).
- W13a: the kit's animated sprite registers its own component; `AnimationState` names its animation as a string.
