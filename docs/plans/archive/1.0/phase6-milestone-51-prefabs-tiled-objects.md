# Phase 6 milestone 51: prefabs and Tiled objects (W16b) — draft

- **status:** done
- **goal:** Prefab files, listed in a `prefabs` manifest section and spawned by ID at once or through the command buffer; Tiled object layers whose object classes name prefabs, the objects' properties over the prefabs' components; the registration form that checks the asset IDs a component names, with `particle_emitter` and `tilemap` on it; a public `TilemapData` builder with tile setters; the kit's spawner. The template's player and example 01's ambient emitters spawn from prefabs.
- **non-goals:** Nested prefabs, scene entries that instance prefabs and patching live entities on hot reload (backlog); World snapshots or saves by reflection (W11c owns save slots); a rotation or scale in prefab files, rotated or templated Tiled objects, and overrides of a prefab's sprite, tag or tween; a `spawner` scene component; moving the kit's `animation` component onto the asset-checked form; `tungsten check` (W14b); the controllers and gameplay helpers (W13c, W13d); tile layers ordered among sprites (backlog); an enemy in the template (Q13).
- **files to touch:** Step 1: `crates/tungsten-core/src/component_registry.rs`, `crates/tungsten-core/src/assets/{prefab.rs (new), manifest.rs, scene.rs, tilemap.rs, mod.rs}`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/{component_registry.rs, assets/prefab.rs (new), assets/manifest.rs, assets/scene.rs, assets/tilemap.rs, physics/query.rs, physics/step.rs}`, `crates/tungsten-core/tests/{manifests.rs, physics_containment.rs}`, `crates/tungsten-kit/src/{spawner.rs (new), lib.rs, tests/spawner.rs (new)}`, `crates/tungsten/src/tests/{extract.rs, tilemap_extract.rs}`, `examples/01_platformer/src/tests/{main.rs, presentation.rs}`, `examples/02_bench/src/integrated/assets.rs`, `examples/02_bench/src/gpu/scene.rs`, `api/tungsten-core.txt`, `api/tungsten-kit.txt`. Step 2: `crates/tungsten/src/asset_loader/{prefab.rs (new), mod.rs, reload.rs, scene.rs}`, `crates/tungsten/src/{plugins.rs, app.rs}`, `crates/tungsten/src/tests/{prefabs.rs (new), plugins.rs, kit.rs, asset_loader.rs}`, `api/tungsten.txt`. Step 3: `templates/basic/{Cargo.toml, AGENTS.md, assets/manifest.json, assets/prefabs/player.json (new), src/components.rs, src/game.rs, src/states.rs, tests/game.rs}`, `docs/getting-started.md`, `Cargo.lock`. Step 4: `examples/01_platformer/{Cargo.toml, tools/level.py, tools/generate.py, tools/README.md, tools/outputs.json, assets/manifest.json, assets/prefabs/ambient_emitter.json (new, generated), assets/tilemaps/level.tmj, src/level_layout.rs, src/setup.rs, src/state.rs, src/tests/main.rs, src/tests/presentation.rs, src/tests/death.rs}`, `Cargo.lock`. Step 5: none in the tree (a base copy and captures under the ignored `target/` and `perf-runs/`). Step 6: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/assets.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/{w04-api-freeze.md, w16-prefabs.md, w13-kit.md, backlog-1.x.md, roadmap.json}`. Step 7: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, the register row in `docs/plans/1.0/implementation-plan.md`, the four owned Now fields in `docs/plans/1.0/README.md`, this plan (moved to `docs/plans/archive/1.0/`).
- **ordered steps:** (1) Core and kit API: the asset-checked registration form, prefab files and spawning, the manifest section, Tiled objects, the tilemap builder, the kit's spawner, then the level C API critique. (2) Umbrella wiring: loading, hot reload and the engine's two components. (3) The template's player from a prefab, and the guide. (4) Example 01's ambient emitters from a Tiled object layer. (5) Invariants: the bench rows' digests and the pixel tests. (6) Records. (7) Release 0.71.
- **done-when:** every step's evidence row quoted; workflow §5 closed; the release checks, plus the gates by change no step ran, pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W16b ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M51, release 0.71. Written 2026-10-14 at `b6aefe4` on branch `0.71`. Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs. W16 has not graduated: [criteria](../../1.0/criteria.md) §8.7 is the scoping source and [w16](../../1.0/w16-prefabs.md) holds placement and M48's follow-up.

## Context digest

W16a (M48, 0.67, `D-153`) landed the component registry: scene entries name registered components, checked at load against the World with field paths, spawned prepare-then-queue. `D-153` left W16b the registration form that checks the asset IDs a value names, `particle_emitter` and `tilemap` on it, and a marker line for prefab files; `SpawnContext::world` and the load's World argument exist for it. The frame-loop gate gave W16b the public `TilemapData` builder (`D-132`): the [acceptance game](../../1.0/acceptance-game.md) builds its arena in code each run, spawns bolts, gems, enemies and props from prefabs, and places its spawners from its generator; Tiled object classes have example 01 as their only user. The kit (M50, `D-155`) takes the spawner "under the same rules": an opt-in plugin, a `PostUpdate` system under a public name, if-present constraints, rustdoc with an example, a harness test in the umbrella.

Level C (workflow §4, `D-150`, `D-151`): an Opus 5.5 max plan, the API sketch in §2, and a critique of step 1's API diff before the rest runs. Later candidates build on this: W14b's `check` reads prefabs, W13c and W13d register kit components, W12b migrates example 01's other placements. The template changed how a game spawns its entities, so it and the guide follow in step 3 (workflow §5.6).

## 1. Audit

Read at `b6aefe4`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | No `prefabs` section. `RawManifest` does not deny unknown keys, so a `prefabs` key parses and is dropped today. Each section resolves and canonicalizes its paths in `load_unvalidated`, and `merge_entries` refuses a duplicate ID per section. No `RawManifest` or `ResolvedManifest` literal and no exhaustive `ManifestError` match exists outside core | `crates/tungsten-core/src/assets/manifest.rs:9-51`, `:55-84`, `:252-269`, `:368-574`, `:648-732` |
| A2 | `ComponentRegistry::check` is a World-free function pointer; `prepare` builds with a `SpawnContext` that holds `&World`; `ComponentError` is `#[non_exhaustive]`; `SpawnContext::world`'s rustdoc names W16b's asset IDs; a `PreparedComponent` queues on a pending entity only | `crates/tungsten-core/src/component_registry.rs:46-55`, `:221-240`, `:259-270`, `:300-305`, `:308-333` |
| A3 | `SceneData::load` checks each component with `check`; `default_visible`, `strict_components` and `MAX_POSITION`, the ±2^24 bound a scene transform keeps, are private to the scene module. A JSON `1e40` reads as `f32` infinity and fails that bound; `rigid_body` copies the spawn position into `Position` | `crates/tungsten-core/src/assets/scene.rs:19-21`, `:248-250`, `:260-277`, `:311-315`, `:324-352`; `src/tests/assets/scene.rs:300-306`; `src/physics/plugin.rs:229-236` |
| A4 | `spawn_scene` prepares every component with `&World`, then queues the fields, then the components in name order, all in one archetype move; `SceneEntity` is the umbrella's. The umbrella's scene components register in `register_extract_components`, which `App::with_plugins` calls (and so `App::new`), and `register_squash_stretch` (`GameFeelPlugin`) | `crates/tungsten/src/asset_loader/scene.rs:39-120`, `:130-145`; `crates/tungsten/src/app.rs:254-257` |
| A5 | The Tiled loader skips object layers and reads no objects, layer offsets, object classes or tileset `objectalignment` (`TiledTileset` holds `firstgid` and `tiles` only). Tiled writes an object's class as `type` (1.8 and earlier, 1.10 and later) or `class` (1.9 only); `rotation` is degrees clockwise; a tile object's (`gid`) `y` is its bottom ([Tiled JSON map format](https://doc.mapeditor.org/en/stable/reference/json-map-format/), read 2026-10-14) | `crates/tungsten-core/src/assets/tilemap.rs:51-56`, `:65-74`, `:179-182` |
| A6 | `TilemapData` has public fields, no constructor and a `Deserialize` derive no caller uses; `validate` is private; `TilemapRegistry::insert` does not validate. `glam` has no serde feature, so a `Vec2` field cannot derive `Deserialize` | `tilemap.rs:106-116`, `:239-276`, `:312-314`; `Cargo.toml:55` |
| A7 | Seven `TilemapData` literals outside core's sources, which `rg -n '[^>] TilemapData \{' examples crates/tungsten crates/tungsten-core/tests` lists exactly (`[^>]` skips `-> TilemapData {` signatures): the release-only containment test, two umbrella tests, two example 01 tests and the bench's two (rows `integrated`, `gpu`, `gpu-throughput`). Ten more inside core | `crates/tungsten-core/tests/physics_containment.rs:754`; `crates/tungsten/src/tests/extract.rs:55`, `tilemap_extract.rs:56`; `examples/01_platformer/src/tests/main.rs:131`, `presentation.rs:303`; `examples/02_bench/src/integrated/assets.rs:160`, `gpu/scene.rs:294`; core: `src/assets/tilemap.rs:225`, `src/tests/assets/tilemap.rs:5`, `src/tests/physics/query.rs:38`, `src/tests/physics/step.rs:80, 456, 587, 627, 741, 2407, 2773, 3490` |
| A8 | `load_all` loads tilemaps, then particles, then checks tileset sprites and particle render references, and needs a `Renderer`. `App::with_plugins`, which `App::new` calls, inserts every asset registry empty, no prefab one. `resumed` loads the manifests after the plugins have built and before the startup hook | `crates/tungsten/src/asset_loader/mod.rs:186-205`, `:400-468`; `app.rs:218-227`, `:2093-2109` |
| A9 | Hot reload routes `.json` by the theme, animation and particle registries' path lookups and `.tmj` to `reload_tilemap`, which checks tileset sprites and keeps the last good map; `reload_manifest` adds new tilemaps, particles and themes. DESIGN's matrix has no prefab row, and `D-053` wants the matrix in the same change | `app.rs:622-689`; `crates/tungsten/src/asset_loader/reload.rs:191-222`, `:460-500`, `:552-584`, `:644-645`; `DESIGN.md:223-241` |
| A10 | `ParticlesPlugin` registers no scene component. `ParticleEmitter` holds a `ParticleConfig` `AssetId`, which a name resolves through `ParticleConfigRegistry::id_for_name`. The names test lists the four engine components | `crates/tungsten/src/plugins.rs:121-144`; `crates/tungsten-core/src/components.rs:140-163`; `crates/tungsten-core/src/assets/particle.rs:535-537`; `crates/tungsten/src/tests/plugins.rs:191-208` |
| A11 | The kit has no spawner. Its items: an opt-in plugin, a `PostUpdate` system under a public name, warn-once state in the system's closure; the harness tests' `kit_app` and their resolved `post_update` line | `crates/tungsten-kit/src/lib.rs:44-57`; `lifetime.rs:8-100`; `sound.rs:115-180`; `crates/tungsten/src/tests/kit.rs:21-44` |
| A12 | The template spawns its player by hand with `spawn_with` and needs no `glam` (it sets `transform.position.x` and `.y`); `Player` has no `Deserialize`; no prefab and no `serde` dependency; no enemy. The harness loads no manifest. The guide quotes `spawn_player` and the template's `Cargo.toml` | `templates/basic/src/states.rs:77-93`, `src/components.rs:3-5`, `assets/manifest.json`, `Cargo.toml:8-13`; `crates/tungsten/src/testing.rs:18-20`; `docs/getting-started.md:24-33`, `:108-118` |
| A13 | Example 01 spawns its ten ambient emitters from a generated table in `spawn_level_presentation`, which `populate_world` calls at startup and on a restart. Tests reach it: the ambient test counts `AmbientEmitter` against `EMITTERS`, the death tests run `populate_world` over test registries, and `load_presentation_assets` seeds them. No `serde` dependency | `examples/01_platformer/src/setup.rs:525-554`, `:633-646`, `:721-742`; `src/level_layout.rs:1775-1830`; `src/tests/presentation.rs:196-223`; `src/tests/death.rs:16-47`; `src/tests/main.rs:178-245` |
| A14 | Example 01's whole `assets/` tree, its manifest included, is generator output, and `--check` fails on any file under `assets/` outside the owned list. `level.py` writes tile layers only, and `validate_level` reads `data` from every layer; the generator's tests edit `layers[0]` and `layers[-1]`; the authoring guide says no object layers are used. The system Python has no Pillow: `just level-check` passed at `b6aefe4` with Pillow 12.3.0 in a scratch venv (47.8 s, 15 tests) | `examples/01_platformer/tools/generate.py:216-265`, `:296-299`; `tools/level.py:43-45`, `:55-57`; `tools/test_generate.py:43`, `:51`; `tools/README.md:149` |
| A15 | The LLM index is 12,202 of its 12,288 bytes (`just ctx`). `docs/assets.md` has no Tilemap, Particle or Prefab row. DESIGN says object layers are skipped, and its M48 section describes W16b's work ahead of it | `docs/assets.md:7-18`; `DESIGN.md:247`, `:452` |
| A16 | `integrated`'s first-run digest in the 0.70 sitting is `334b519e18b543d7`, as `docs/perf/benchmarks.md` records it; no recent capture holds the `gpu` rows' digests | `perf-runs/20261014T074646Z-suite/integrated/capture.json` (machine-local) |
| A17 | `D-153`'s open limits name this work; the backlog defers nested prefabs and live patching; `roadmap.json`'s `w13cd` summary still lists "spawners", which the W16b card owns | `DECISIONS.md:1913`; `docs/plans/1.0/backlog-1.x.md:19`; `docs/plans/1.0/roadmap.json` (`w13cd`) |

## 2. Design

### 2.1 The seam

Everything that reads or spawns prefabs is core, with no render type (`D-007`, `D-016`): prefab data and spawning in `tungsten_core::assets::prefab`, map objects in `assets::tilemap`, the asset-checked form in `component_registry`. Spawning is `World` and `CommandBuffer` methods, so the kit's spawner stays on core alone (`D-155`). The umbrella loads files, registers its two components and routes hot reload. Extract is untouched and still reads `&World` (`D-018`); no system, stage or schedule changes.

### 2.2 The asset-checked registration form

```rust
// tungsten_core::component_registry; MissingAsset also at the crate root
/// An asset ID that a component's value names and the World does not hold.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MissingAsset {
    /// The value's field that names it, as `ComponentError::Invalid` names fields.
    pub field: String,
    /// The asset kind as messages print it: "particle config", "tilemap".
    pub kind: &'static str,
    /// The ID the value gave.
    pub id: String,
}

impl MissingAsset {
    #[must_use]
    pub fn new(field: impl Into<String>, kind: &'static str, id: impl Into<String>) -> Self;
}

impl ComponentRegistry {
    /// `register_with`, plus `assets`: the check of the asset IDs a definition
    /// names against a World. `check_in` and `prepare` run it after deserializing.
    #[track_caller]
    pub fn register_with_assets<D, B>(
        &mut self,
        name: &str,
        assets: impl Fn(&D, &World) -> Result<(), MissingAsset> + 'static,
        build: impl Fn(D, &SpawnContext<'_>) -> B + 'static,
    ) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle;

    /// `check`, then, for a name registered with assets, its IDs against `world`.
    pub fn check_in(&self, name: &str, value: &Value, world: &World) -> Result<(), ComponentError>;
}

impl World {
    /// `register_with_assets` on this World's registry, inserted first when it has none.
    #[track_caller]
    pub fn register_component_with_assets<D, B>(
        &mut self,
        name: &str,
        assets: impl Fn(&D, &World) -> Result<(), MissingAsset> + 'static,
        build: impl Fn(D, &SpawnContext<'_>) -> B + 'static,
    ) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle;
}

#[non_exhaustive]
pub enum ComponentError {
    Unknown { name: String },
    Invalid { name: String, field: String, source: serde_json::Error },
    /// "component `<name>`, field `<field>`: unknown <kind> `<id>`"
    MissingAsset { name: String, field: String, kind: &'static str, id: String },
}
```

`check` stays deserialization alone, so a tool without a World still checks shapes; `check_in` adds the asset check. `prepare` runs the asset check against `SpawnContext::world()` before `build` and returns `MissingAsset` on a failure, so `build` stays infallible and may `expect` the IDs it was given; that also catches an asset that went between the load and the spawn. `SceneData::load` and `PrefabData::load` call `check_in`. Name and type conflicts panic as `register_with`'s do (`D-153`). `PreparedComponent` gains a crate-private insert on a live entity for §2.4's immediate spawn; its public API is unchanged.

### 2.3 The manifest section and the prefab file

```rust
// tungsten_core::assets::manifest, re-exported as the other entry types are
pub struct RawManifest { /* … */ pub prefabs: HashMap<String, PrefabEntry> } // `unique_ids`, as every section
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct PrefabEntry { pub path: String }
pub struct ResolvedManifest { /* … */ pub prefabs: HashMap<String, ResolvedPrefab> }
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ResolvedPrefab { pub path: PathBuf }
pub enum ManifestError { /* … */ MissingPrefabFile { id: String, path: String } } // "prefab '{id}' references missing file: {path}"
```

`load_unvalidated` resolves and canonicalizes each prefab path as it does a tilemap's, and `merge_entries` refuses a duplicate prefab ID (`D-017`). `docs/assets.md` gains the row (`D-106`): `assets/prefabs/`, section `prefabs`.

```rust
// tungsten_core::assets::prefab; PrefabData, PrefabRegistry, PreparedPrefab and
// PrefabError also at tungsten_core::assets and the crate root
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PrefabData {
    #[serde(default)]
    pub sprite: Option<SceneSprite>,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tweens: Vec<SceneTween>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty", deserialize_with = "strict_components")]
    pub components: BTreeMap<String, Value>,
}

impl PrefabData {
    /// Reads and parses `path`, then checks its tweens and each component with
    /// `ComponentRegistry::check_in` against `world` (an empty registry when the
    /// World has none). The first error stops the load.
    pub fn load(path: &Path, world: &World) -> Result<Self, PrefabError>;
}
```

A prefab file is a scene entry's fields without `transform` (Q1), parsed as strictly as scenes (`D-153`): `default_visible`, `strict_components` and the position bound `MAX_POSITION` become crate-visible in the scene module, and the prefab and map code share them.

```json
{
  "sprite": { "asset_id": "slime", "z_order": 2 },
  "tag": "enemy",
  "components": {
    "enemy": null,
    "rigid_body": { "kind": "dynamic", "shape": { "kind": "circle", "radius": 6 } }
  }
}
```

The load's errors, each a case of step 1's malformed-prefab test: a file that does not read is `Io`; text that is not JSON, an unknown key at the prefab's top level, in `sprite` or in a tween, a component named twice and a key repeated inside a value are `Parse`; more than one tween (a new format need not repeat `D-055`'s first-wins), a tween that fails `SceneTween::validate`, an unknown component, a value that does not deserialize (the field path named) and an asset ID the World lacks are `Validation`, whose message names `tweens`, `tweens[0]` or the `ComponentError`. Every message names the file. Two rules stay as scenes have them: a tween channel accepts unknown keys (`SceneTweenChannel` denies none), and a component's value follows its type's serde rules (`D-153`), so an unknown field fails, as `Validation`, only a type with `deny_unknown_fields` such as `rigid_body`'s; the test holds a case of each.

### 2.4 The registry, spawning and the command-buffer form

```rust
#[derive(Debug, Default)]
pub struct PrefabRegistry { /* id → data, path → id */ }

impl PrefabRegistry {
    #[must_use] pub fn new() -> Self;
    pub fn insert(&mut self, id: String, data: PrefabData); // replaces an entry of the same ID
    pub fn insert_with_path(&mut self, id: String, data: PrefabData, path: PathBuf);
    #[must_use] pub fn get(&self, id: &str) -> Option<&PrefabData>;
    pub fn ids(&self) -> impl Iterator<Item = &str>; // sorted
    #[must_use] pub fn id_for_path(&self, path: &Path) -> Option<&str>;
    #[must_use] pub fn len(&self) -> usize;
    #[must_use] pub fn is_empty(&self) -> bool;
}

/// One prefab spawn, built: its fields and every component prepared against the
/// World, nothing spawned yet. `Debug` prints the ID.
pub struct PreparedPrefab { /* … */ }

impl PreparedPrefab {
    #[must_use] pub fn id(&self) -> &str;
    /// Adds `bundle` after the prefab's own components, in the same archetype move.
    #[must_use] pub fn with<B: Bundle>(self, bundle: B) -> Self;
    /// Spawns it now, in one archetype move.
    pub fn spawn(self, world: &mut World) -> Entity;
}

impl World {
    pub fn prepare_prefab(&mut self, id: &str, position: impl Into<Vec2>) -> Result<PreparedPrefab, PrefabError>;
    pub fn spawn_prefab(&mut self, id: &str, position: impl Into<Vec2>) -> Result<Entity, PrefabError>;
}

impl CommandBuffer {
    /// Queues `prefab`: one spawn and its inserts, applied at the flush as one archetype move.
    pub fn spawn_prefab(&mut self, prefab: PreparedPrefab) -> PendingEntity;
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PrefabError {
    Io { path: String, source: std::io::Error },      // "failed to read prefab '{path}': {source}"
    Parse { path: String, source: serde_json::Error }, // "invalid prefab '{path}': {source}"
    Validation { path: String, message: String },      // "invalid prefab '{path}': {message}"
    Unknown { id: String },                            // "unknown prefab `{id}`"
    Component { id: String, source: ComponentError },  // "prefab `{id}`: {source}"
    Position { id: String, position: Vec2 },           // "prefab `{id}`: position {position} must be finite and within ±16777216"
    UnknownMap { map: String },                        // "unknown tilemap `{map}`"
    Object { map: String, layer: String, object: u32, message: String },
    // "tilemap `{map}`, layer `{layer}`, object {object}: {message}"
}
```

The entity gets, in order: `Transform::from_position(position)`; the sprite as `Sprite { asset_id, color, z_order, material_id: None }`, its name interned; `Visibility { visible }`; `Tag` when given; the tween as a `Tween`; the components in name order; then each `with` bundle in call order. That is a scene entry's order (`D-153`): a component, and after it a `with` bundle, wins over an earlier field of its type. `spawn` records the run on a fresh entity and flushes it, one archetype move; `CommandBuffer::spawn_prefab` records it on a pending entity for the frame's flush (`D-039`, `D-084`). Positions take `impl Into<Vec2>`, so a game without a `glam` dependency passes `[f32; 2]` (A12).

`prepare_prefab` returns `Unknown` when the World has no `PrefabRegistry` or the ID is not in it; `Position` when the position is not finite or lies past ±2^24 on either axis, the scene transform's bound (`D-153`), since a `rigid_body` copies it into `Position`; and `Component` for the first component whose `prepare` fails against `SpawnContext::new(world, transform)`. It interns the sprite only after every component has prepared, so an error leaves the World as it was. It takes `&mut World` because interning does (Q5), and panics when the prefab names a sprite and the World has no `AssetRegistry`, as `spawn_scene` does (`D-022`). A spawn by ID returns a `Result` because the ID comes from code or data and an asset can go after the load; `spawn_scene` keeps its panic on data its load checked (Q3). No `SceneEntity` is added: that marker is the umbrella's, so a state-owned spawn passes it through `with` (Q6).

### 2.5 Tiled objects

```rust
// tungsten_core::assets::tilemap; ObjectLayer and MapObject also at
// tungsten_core::assets and the crate root
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct TilemapData {
    pub tile_width: u32,
    pub tile_height: u32,
    pub width: u32,
    pub height: u32,
    pub tileset: Vec<String>,
    pub layers: Vec<TilemapLayer>,
    /// The object layers in file order; `Deserialize` skips it (A6).
    #[serde(skip)]
    pub object_layers: Vec<ObjectLayer>,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ObjectLayer {
    pub name: String,
    pub objects: Vec<MapObject>,
}

/// One Tiled object. `class` names the prefab it spawns; empty, it is a marker
/// the game reads by name.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MapObject {
    pub id: u32,
    pub name: String,
    pub class: String,
    /// Top-left in map pixels, the layer's offset added.
    pub position: Vec2,
    pub size: Vec2,
    /// Custom properties by name, each Tiled `value` as JSON.
    pub properties: BTreeMap<String, Value>,
}
```

`TilemapData::load` rules; each failure is a `TilemapError::Invalid` naming the layer and the object's ID, so `TilemapError` gains no variant:

- An `objectgroup` layer loads, in file order among object layers; its `offsetx` and `offsety` (default 0) add to its objects' positions. Image layers stay skipped and group layers still fail (0.70).
- An object needs `id`, `x` and `y`; `name`, `width` and `height` default to empty and 0. Its class is `type`, or `class` from a Tiled 1.9 file; both present, non-empty and different fails.
- Every number an object or its layer gives (`x`, `y`, `width`, `height`, `offsetx`, `offsety`) must be finite and each size at least 0, and the resulting position must lie within ±2^24 on both axes, the scene transform's bound, shared from the scene module. `prepare_map_objects` applies the same bound to `origin + position`, as an `Object` error.
- `position` is `(x, y)` plus the offset. A tile object (`gid` present) is placed by its tileset's `objectalignment`, which the loader now reads from the map's tileset (it reads one): `unspecified` (Tiled's default, bottom-left on an orthogonal map) and `bottomleft` give `y` at the object's bottom, so `position` takes `y - height`, the top-left Tiled draws; `topleft` gives the top-left as it is; any other alignment fails the load when a tile object uses it (Q10).
- `properties` keep each Tiled property's `value`: string, int, float, bool, color and file as scalars, object as its ID, class as an object of the members Tiled wrote. A name given twice fails.
- An object with a `template` fails, since its fields live in a `.tx` file the loader does not read; an object with a class and a `rotation` other than 0 fails, since the engine turns a sprite about its centre and Tiled turns an object about its origin (Q9).
- Tiled's `visible` flags are editor display and filter nothing.

```rust
impl World {
    /// Checks every object of `data` with a class against this World as
    /// `prepare_map_objects` would build it: its prefab exists, its properties
    /// apply, and each resulting component passes `check_in`. `map` names the map in errors.
    pub fn check_map_objects(&self, map: &str, data: &TilemapData) -> Result<(), PrefabError>;
    /// Prepares every object with a class of the `TilemapRegistry`'s map `map`, in
    /// layer and file order: its prefab at `origin + position`, its properties over
    /// the prefab's components. The first failure returns, the World as it was.
    pub fn prepare_map_objects(&mut self, map: &str, origin: impl Into<Vec2>) -> Result<Vec<PreparedPrefab>, PrefabError>;
    /// `prepare_map_objects`, then spawns each now; the entities in the same order.
    pub fn spawn_map_objects(&mut self, map: &str, origin: impl Into<Vec2>) -> Result<Vec<Entity>, PrefabError>;
}
```

`UnknownMap` when the map is not registered; `Object` for an unknown prefab, a property that does not apply and a component that fails, the message naming the prefab, the property, or the component and its field. A command-buffer form needs nothing more: each `PreparedPrefab` goes to `CommandBuffer::spawn_prefab`.

Properties over components (Q8), applied in property-name order, so a field property applies after its component's class property:

- A name is a dotted path. The first segment names a component, added as `{}` when the prefab lacks it; the check then judges the result.
- Each later segment steps into an object's member, created as `{}` while segments remain, or, when all digits, into an existing array element. Stepping into anything else fails, naming the property and the path so far.
- The value replaces what the path names, except an object (a class property), which merges member by member, recursively. No property removes a member, so a class property that switches a tagged value's variant (`rigid_body.shape` from a circle to a box) keeps the old variant's members and fails the check, which names the leftover field: another variant is another prefab (Q8). A plain member named `kind`, such as `rigid_body`'s body kind beside its `kind`-tagged `shape`, overrides as any other member does.
- A one-segment name needs a class value; a scalar fails with "name a field: `<component>.<field>`".
- Properties reach `components` only; the prefab's sprite, visibility, tag and tween stay as its file gives them.

### 2.6 The tilemap builder

```rust
impl TilemapData {
    /// An empty map of `width` × `height` cells of `tile_width` × `tile_height`
    /// pixels: no tileset entries, layers or objects.
    /// # Panics: a dimension of 0 (`D-022`).
    #[must_use] #[track_caller]
    pub fn new(tile_width: u32, tile_height: u32, width: u32, height: u32) -> Self;
    /// Appends `sprite`, a sprite ID, to the tileset; returns its tile index.
    /// # Panics: past `TileIndex::MAX` entries.
    #[track_caller]
    pub fn add_tile(&mut self, sprite: impl Into<String>) -> TileIndex;
    /// Appends a layer with every cell `EMPTY_TILE`; returns its index.
    pub fn add_layer(&mut self, name: impl Into<String>, kind: LayerKind) -> usize;
    /// Sets one cell; a negative tile clears it (stored as `EMPTY_TILE`).
    /// # Panics: `layer`, `col` or `row` out of range, or `tile` at or past the tileset's length.
    #[track_caller]
    pub fn set_tile(&mut self, layer: usize, col: u32, row: u32, tile: TileIndex);
    /// Sets every cell of `layer`. # Panics: as `set_tile`.
    #[track_caller]
    pub fn fill(&mut self, layer: usize, tile: TileIndex);
}
```

The acceptance game's arena:

```rust
let mut arena = TilemapData::new(16, 16, 64, 64);
let floor = arena.add_tile("arena_floor");
let wall = arena.add_tile("arena_wall");
let ground = arena.add_layer("ground", LayerKind::Render);
let walls = arena.add_layer("walls", LayerKind::Collision);
arena.fill(ground, floor);
for col in 0..64 {
    arena.set_tile(walls, col, 0, wall);
    arena.set_tile(walls, col, 63, wall);
}
world.resource_mut::<TilemapRegistry>().insert("arena".to_string(), arena);
```

`#[non_exhaustive]` ends construction by literal outside core, so a field added later is not a break; the fields stay public for reading and bulk writes, and `TilemapLayer` is unchanged (Q11). The seven literals of A7 become `TilemapData::new(..)` and assignments of the `tileset` and `layers` they set today, so each map stays the same value (Q18); core's ten add `object_layers: Vec::new()` or take the builder.

### 2.7 The engine's two components

| Name | Registered by | Value | Spawns | Asset check |
| --- | --- | --- | --- | --- |
| `particle_emitter` | `ParticlesPlugin` | `{ "config": "<particles ID>", "seed": <u64> }`, `seed` optional, unknown keys failing | `ParticleEmitter` with the config's `AssetId` and `seed_override`, and `ParticleEmitterState::default()` | `config` names a `ParticleConfigRegistry` entry ("particle config") |
| `tilemap` | `App`, beside the default extracts (`register_extract_components`) | `{ "id": "<tilemaps ID>" }`, unknown keys failing | `TilemapInstance::new(id, position)`, the entity's position as the map's top-left | `id` names a `TilemapRegistry` entry ("tilemap") |

Each registers where its reader lives (`D-153`); a `tilemap` component spawns the tiles only, and a game spawns the map's objects with `spawn_map_objects`. Both join the registry module's table and are default names fixed from 1.0 (`D-103` as `D-153` amended it). A game that registered either name now panics at its registration (a ledger row).

### 2.8 Loading and hot reload

```rust
// tungsten::asset_loader
/// Loads the manifest's prefabs in sorted ID order, each with `PrefabData::load`
/// against `world`, into a new `PrefabRegistry` with their paths. The first
/// failure is an error naming the ID.
pub fn load_prefabs(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()>;
/// Hot-reloads prefab `id` from `path` for later spawns; a file that fails its
/// load keeps the previous prefab and logs why. Live entities stay as they are.
pub fn reload_prefab(id: &str, path: &Path, world: &mut World) -> anyhow::Result<()>;
```

- `App::with_plugins` inserts an empty `PrefabRegistry` beside the other registries.
- `load_all` runs `load_prefabs` after `load_particles`, since a prefab's IDs name particle configs and tilemaps, then `check_map_objects` for every tilemap in sorted ID order. A failure fails the load, as an unknown tileset sprite does (Q19).
- `reload_tilemap` checks the objects after the tileset and keeps the last good map on a failure.
- `reload_manifest` keeps the startup order. New tilemaps parse, pass the tileset check and insert as today; then a prefab block loads each new prefab against the World, the new maps included, so a prefab added with the map its `tilemap` names loads (a failure logs and skips the prefab; a removed one warns and stays); then the objects of each map the reload added are checked against the prefabs now loaded, and a failure logs the error: the map keeps its tiles, and `spawn_map_objects` returns the same error.
- `process_hot_reload`'s `.json` branch looks up `PrefabRegistry::id_for_path` after the theme, animation and particle lookups and calls `reload_prefab`.
- DESIGN's matrix gains a Prefab row, and its Tilemap row the object check (`D-053`).

### 2.9 The kit's spawner

```rust
// tungsten_kit
/// [`SpawnerPlugin`]'s system, in `PostUpdate`.
pub const SPAWNER_TICK: &str = "spawner_tick";

/// Spawns a prefab at its entity's position once per interval of game time: an
/// enemy wave, a stream of pickups. A pause holds it; the time scale stretches it.
#[derive(Debug, Clone)]
pub struct Spawner { /* prefab ID, Timer::repeating, remaining */ }

impl Spawner {
    /// Spawns `prefab` every `interval` seconds, the first after one interval, without end.
    /// # Panics: `interval` not finite and above 0, as `Timer::repeating`.
    #[must_use] pub fn new(prefab: impl Into<String>, interval: f32) -> Self;
    /// Stops after `count` spawns; 0 spawns nothing.
    #[must_use] pub fn with_count(self, count: u32) -> Self;
    #[must_use] pub fn prefab(&self) -> &str;
    /// The spawns left; `None` without end.
    #[must_use] pub fn remaining(&self) -> Option<u32>;
    /// The countdown to the next spawn.
    #[must_use] pub fn timer(&self) -> &Timer;
}

/// Registers [`SPAWNER_TICK`] in `PostUpdate`, after the physics sync and the late
/// propagation pass when they are registered.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpawnerPlugin;
```

Each frame the system ticks every `Spawner` with a `Transform` by `Time::delta()` and, for each period the tick finished (`Timer::tick`'s count, capped by `remaining`), prepares the prefab at the spawner's `Transform.position` and queues it with `CommandBuffer::spawn_prefab`, so the frame's flush spawns it before the extract. A prefab that does not prepare logs one error per prefab ID (state in the system's closure, as `SoundOnEvent`'s warnings) and spawns nothing; without a `CommandBuffer` nothing spawns. Spawned entities carry no state marker (Q16). The kit's root exports it, and its module doc lists it; its doctest and unit tests run on a bare World, its harness test in the umbrella (`D-155`).

### 2.10 The template and the guide

- `templates/basic/assets/prefabs/player.json`: `{ "sprite": { "asset_id": "player" }, "components": { "player": null } }`, listed as `prefabs.player`.
- `Player` derives `serde::Deserialize` (Q14) and `GamePlugin::build` registers it as `player`. `spawn_player` becomes `world.prepare_prefab(PLAYER_PREFAB, PLAYER_START)`, `.with((SceneEntity { state_id: GAMEPLAY },))`, `.spawn(world)`, with an `expect` that names the prefab: the startup load checks it, so a failure is a programmer error (`D-022`). `PLAYER_PREFAB` replaces `PLAYER_SPRITE`.
- `tests/game.rs`'s `harness()` loads the manifest's prefabs through `tungsten::asset_loader::load_prefabs` on `world_mut()`, since the harness loads no manifest; `data_files_load` checks the prefab entry; `state_start_starts_gameplay_with_the_player` also checks the player's sprite and `SceneEntity`. The schedule snapshot does not change.
- `AGENTS.md` gains the rule: an entity kind the game places more than once, or that a level file places, is a prefab under `assets/prefabs/` in the manifest's `prefabs`, its components registered in the plugin's `build`, spawned with `prepare_prefab(..)?.spawn(world)` or, from a system, `CommandBuffer::spawn_prefab`, a state's `SceneEntity` passed through `with`.
- The guide's `Cargo.toml` block gains `serde`; "How the game is laid out" names `assets/prefabs/`; "Queries and spawns" quotes the new `spawn_player` and adds a paragraph on prefab files, the command-buffer form and Tiled object classes.

### 2.11 Example 01's object kind

- `tools/level.py` emits an `objectgroup` layer `emitters` before `collision`, one point object per placement in `source['emitters']` (after `prepare` places the flame emitters), `type` `ex10_ambient_emitter`, at `x * tile`, `y * tile`, with a string property `particle_emitter.config` and an int property `particle_emitter.seed`. `validate_level` checks the tile layers' data and skips object layers; `rust_layout` emits `EmitterPlacement` and `EMITTERS` under `#[cfg(test)]` as the test's table. The layer sits before `collision`, so the generator's tests keep `layers[0]` and `layers[-1]` (A14).
- `tools/generate.py` writes `assets/prefabs/ambient_emitter.json`, `{ "components": { "ambient_emitter": null, "particle_emitter": { "config": "ex10_torch_embers" } } }`, as an owned output, and the manifest's `prefabs` section with `ex10_ambient_emitter`, which the uniqueness and prefix checks cover. `tools/README.md` describes the layer in place of "No Tiled object layers".
- `AmbientEmitter` derives `Deserialize`; `configure_app` registers it as `ambient_emitter` through a `register_components(world)` the tests call too. `populate_world` calls a new `spawn_level_objects`, `spawn_map_objects("ex10_level", Vec2::ZERO)`, right after `spawn_level_presentation`, whose emitter loop goes.
- `load_presentation_assets` also registers the components, the level map from `level.tmj` and the manifest's prefabs (`load_prefabs`); the ambient test spawns the level objects and checks each emitter against `EMITTERS`: its position within 1e-3 px of `tile_position * TILE`, its config and its seed. The emitters are the entities they were, plus `Visibility { visible: true }`, which a prefab spawn gives every entity.

### 2.12 Performance

No frame runs changed code: the bench rows build their tilemaps at setup, and the per-frame readers of `TilemapData` (the physics gather, the tile extract) read the same fields. So no timing capture runs (Q18); step 5 checks that the three rows whose setup changed build the same work, by their first-run digests against the base tree with `--allow-background`, which checks digests only and tolerates the encoder. The pixel tests cover `gpu-visual.png`, built from `gpu/scene.rs`.

## 3. Steps

### Step 1: Core and kit API

- **Files:** step 1's in the header.
- **Change:** In order. (a) The asset-checked form, `MissingAsset`, `ComponentError::MissingAsset` and `check_in` (§2.2), and `SceneData::load` on `check_in`. (b) The manifest section (§2.3). (c) `assets::prefab`: `PrefabData`, `PrefabRegistry`, `PreparedPrefab`, `PrefabError`, the `World` and `CommandBuffer` methods (§2.3, §2.4). (d) Object layers, `ObjectLayer`, `MapObject`, the overrides, `check_map_objects`, `prepare_map_objects` and `spawn_map_objects` (§2.5), and the builder with `#[non_exhaustive]` (§2.6), migrating every literal of A7 in this step. (e) `Spawner`, `SpawnerPlugin` and `SPAWNER_TICK` (§2.9). (f) Layer 1: `crates/tungsten-core/tests/manifests.rs` gains a test that every `prefabs` entry of every example and template manifest parses as `PrefabData`. (g) Rustdoc on every new public item, with a doctest for `spawn_prefab`, the builder and the spawner. (h) `just api`. Core's tests: `tests/assets/prefab.rs` holds the table-driven `prefab_file_errors`, one case per kind of §2.3's list, each asserting the variant and that the message names the file and the field; spawn order and one archetype move, live and queued; `with`; `Unknown`, a vanished asset and a position past the bound, given or through a map's origin, as errors; map objects with overrides, a `rigid_body` class property `{ "kind": "kinematic" }` that keeps the prefab's shape and a circle-to-box `rigid_body.shape` that fails the check naming `radius`, their error per rule of §2.5, and all-or-nothing preparation. `tests/assets/tilemap.rs` covers the object rules, non-finite and out-of-range numbers and each tile-object alignment among them, and the builder, with its panics as `#[should_panic]` tests.
- **Done-when:**
  - `cargo check --workspace --all-targets --locked` → exit 0.
  - `just check` → exit 0.
  - `just physics-release` → exit 0, with 0 failed and 0 ignored in each of its three test binaries (the determinism hash `0x088e_c07a_73c1_b168` and the containment hash `0xaee2_72e0_1ffc_4e4c` are asserted inside the tests).
  - `rg -n '[^>] TilemapData \{' examples crates/tungsten crates/tungsten-core/tests` → no match, exit 1 (seven before).
  - `rg -c '#\[test\]' <file>` → `crates/tungsten-core/src/tests/assets/prefab.rs` at least 8; `crates/tungsten-core/src/tests/component_registry.rs` at least 17 (14 before); `crates/tungsten-core/src/tests/assets/scene.rs` at least 21 (20); `crates/tungsten-core/src/tests/assets/manifest.rs` at least 48 (46); `crates/tungsten-core/src/tests/assets/tilemap.rs` at least 16 (12); `crates/tungsten-core/tests/manifests.rs` 4 (3); `crates/tungsten-kit/src/tests/spawner.rs` at least 3.
  - `rg -c 'fn prefab_file_errors' crates/tungsten-core/src/tests/assets/prefab.rs` → 1, and the evidence row lists its cases.
  - `just api-check` → exit 0 after `just api`; `git diff --quiet HEAD -- api/tungsten.txt api/tungsten-render.txt` → exit 0.
  - `rg -c 'fn tungsten_core::World::(prepare_prefab|spawn_prefab|prepare_map_objects|spawn_map_objects|check_map_objects|register_component_with_assets)\b' api/tungsten-core.txt` → at least 6; `rg -c 'fn tungsten_core::assets::tilemap::TilemapData::(new|add_tile|add_layer|set_tile|fill)\(' api/tungsten-core.txt` → at least 5; `rg -c '^#\[non_exhaustive\] pub struct tungsten_core::assets::tilemap::TilemapData$' api/tungsten-core.txt` → at least 1; `rg -c 'fn tungsten_core::CommandBuffer::spawn_prefab\(' api/tungsten-core.txt` → at least 1; `rg -c 'ComponentRegistry::(register_with_assets|check_in)\b' api/tungsten-core.txt` → at least 2.
  - `rg -c '^pub struct tungsten_kit::(Spawner|SpawnerPlugin)$|^pub const tungsten_kit::SPAWNER_TICK: &str$' api/tungsten-kit.txt` → 3.
- **Moves:** none.
- **Then, the level C API critique:** in the background, `critique --raw --repo --force --out <scratchpad>/api-critique.md --from 'git --no-pager diff HEAD -- api/ templates/; git ls-files -o --exclude-standard -z -- api templates | xargs -0 -r -n1 git --no-pager diff --no-index /dev/null'` (this step confirms `--force`). Each finding is checked against the repo and recorded in §9 as `C.<n>`; one that holds changes step 1's files, after which step 1's done-when runs again; one that needs another file becomes an open question at its default, no change.

### Step 2: Umbrella wiring

- **Files:** step 2's in the header.
- **Change:** `asset_loader::prefab` with `load_prefabs` and `reload_prefab`, re-exported from `asset_loader` (§2.8); `load_all`'s order and map-object check; `reload_tilemap`'s object check and `reload_manifest`'s prefab block and waiting maps; the `.json` routing and the empty `PrefabRegistry` in `app.rs`; `particle_emitter` (registered from `ParticlesPlugin::build` through a crate-private function in `asset_loader/scene.rs`, as `register_squash_stretch` is) and `tilemap` (in `register_extract_components`), with their rows in the registry module's table (§2.7); `just api`. Tests in the new `tests/prefabs.rs`: `load_prefabs` loads and checks; a malformed file fails naming its ID, file and field; a prefab naming a missing particle config fails its load; `reload_prefab` replaces and a bad edit keeps the previous; a manifest reload that adds, in one edit, a map, a prefab whose `tilemap` names it and a map whose object's class names a prefab added with it loads all of them, and a new map whose objects fail logs the error; `reload_tilemap` keeps the last good map on a bad object; a scene naming an unknown `particle_emitter` config fails `load_scene`; through the harness, a `particle_emitter` prefab emits and a `tilemap` prefab's tiles draw (`Harness::draw`). `tests/kit.rs`: the spawner through the harness (intervals, count, a pause holding it, the spawned entity's position), `kit_app` with `SpawnerPlugin` and its resolved line. `tests/plugins.rs`: the names per plugin set.
- **Done-when:**
  - `just check` → exit 0.
  - `rg -c '#\[test\]' crates/tungsten/src/tests/prefabs.rs` → at least 8; `rg -c '#\[test\]' crates/tungsten/src/tests/kit.rs` → at least 14 (13 before).
  - `rg -c '\["light", "parallax_layer", "particle_emitter", "rigid_body", "squash_stretch", "tilemap"\]' crates/tungsten/src/tests/plugins.rs` → 1.
  - `rg -c '^post_update: physics_sync, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, camera_update$' crates/tungsten/src/tests/plugins.rs` → 1 (1 before: `DefaultPlugins`' schedule is unchanged).
  - `just api-check` → exit 0 after `just api`; `rg -c '^pub fn tungsten::asset_loader::(load_prefabs|reload_prefab)\(' api/tungsten.txt` → 2; `git diff --quiet HEAD -- api/tungsten-render.txt` → exit 0.
  - `just smoke` → exit 0.
- **Moves:** none.

### Step 3: The template's player from a prefab, and the guide

- **Files:** step 3's in the header.
- **Change:** §2.10.
- **Done-when:**
  - `cargo test -p tungsten-template-basic --locked` → exit 0.
  - `cargo test -p tungsten-core --test manifests --locked` → exit 0.
  - `just check` → exit 0; `just template-check` → exit 0; `just repo-check` → exit 0.
  - `rg -c 'spawn_with' templates/basic/src/states.rs` → 0 (1 before); `rg -c 'prepare_prefab' templates/basic/src/states.rs` → at least 1.
  - `rg -c '^post_update: physics_sync, particle_count_refresh, particle_emit, particle_tick, tween_tick, squash_stretch_trigger, squash_stretch_tick, shake_tick, camera_update$' templates/basic/tests/game.rs` → 1 (1 before).
  - `rg -c 'prepare_prefab|spawn_prefab' docs/getting-started.md` → at least 2 (0 before); `rg -c '^serde = ' docs/getting-started.md` → 1 (0 before); `rg -c 'prefab' templates/basic/AGENTS.md` → at least 1 (0 before).
- **Moves:** none.

### Step 4: Example 01's ambient emitters from a Tiled object layer

- **Files:** step 4's in the header.
- **Change:** §2.11, through the generator: a venv in the session's scratchpad first, `python3 -m venv "$SCRATCH/level-venv"` and `"$SCRATCH/level-venv/bin/pip" install -r examples/01_platformer/tools/requirements.txt` (network), then `PATH="$SCRATCH/level-venv/bin:$PATH" python3 -B examples/01_platformer/tools/generate.py` writes the outputs. Only the tests §2.11 names change (example tests stay light).
- **Done-when:**
  - `PATH="$SCRATCH/level-venv/bin:$PATH" just level-check` → exit 0: "OK: <N> owned outputs match", then the unittests OK.
  - `cargo test -p example-01-platformer --locked` → exit 0; `cargo test -p tungsten-core --test manifests --locked` → exit 0.
  - `rtk proxy jq -c '[.layers[] | select(.type == "objectgroup") | .objects | length]' examples/01_platformer/assets/tilemaps/level.tmj` → `[10]`.
  - `rg -c 'EMITTERS' examples/01_platformer/src/setup.rs` → 0 (2 before); `rg -c 'spawn_map_objects' examples/01_platformer/src/setup.rs` → 1.
  - `rg -c 'No Tiled object layers' examples/01_platformer/tools/README.md` → 0 (1 before).
  - `just check` → exit 0; `just smoke` → exit 0.
- **Moves:** none: the ten emitters keep their positions, configs and seeds.

### Step 5: Invariants: the bench rows' digests and the pixel tests

- **Files:** none in the tree.
- **Change:** `df -h .` first: at least 10 GiB free, after `just clean-target --apply` if needed. The base tree: `mkdir -p target/m51-base && git archive HEAD | tar -x -C target/m51-base`. In the base tree and in the working tree, one run each: `python3 -B scripts/bench.py run integrated --repeat 1 --allow-background`, `python3 -B scripts/bench.py run gpu --repeat 1 --allow-background` and `python3 -B scripts/bench.py run gpu --preset throughput --repeat 1 --allow-background`, with no tree edit while they run (`D-095`). The base tree's captures land under its own `perf-runs/`: copy them to `perf-runs/20261014-m51-digests/base/` (a `cp -a` checked by `diff -rq`'s exit code) and the candidate's to `.../cand/`, then delete `target/m51-base` with `shutil.rmtree`. Then `just visual`.
- **Done-when:**
  - Each row's `jq -r '.runs[0].digest' <capture>/capture.json` is equal between the trees, and `integrated`'s is `334b519e18b543d7`; each capture valid (`jq -r '.valid'` → `true`).
  - `just visual` → exit 0 with four `test result: ok` lines, one per target: `visual_regression`, `post_regression`, `transition_regression`, `ui_regression` (its one `load_shaders 'fade'` validation ERROR line is the test's own).
  - `git status --short` lists no path the step created.
- **Moves:** none.

### Step 6: Records

- **Files:** step 6's in the header.
- **Change:** First the decision (§5) with the tungsten-decision skill, its index row and the marker lines on `D-153` and `D-032`, then the docs that cite it. `DESIGN.md`: a "Prefabs and Tiled Objects — M51" section after the kit's; the Tilemaps — M10 paragraph's object layers; the reload matrix's Prefab row and the Tilemap row's object check (§2.8); the M48 section's W16b sentence in the past tense, pointing at the new section. `docs/assets.md`: Prefab and Tilemap rows (§2.3, §2.5). `docs/LLM_INDEX.md`: a row for prefabs, Tiled objects and the builder (`core/assets/prefab.rs`, `core/assets/tilemap.rs`, `tungsten/asset_loader/prefab.rs`) and the spawner in the kit row, within 12 KiB by trimming wording in this file only. The [break ledger](../../1.0/w04-api-freeze.md#break-ledger): five rows, each `From` cell starting `W16b (M51`: the manifest types; `TilemapData` `#[non_exhaustive]` with `object_layers`; Tiled files that loaded and now fail (a templated object, a classed rotated one, a `type` and `class` conflict, a repeated property); the two new default component names; the additions for the freeze. `w16-prefabs.md`: W16b landed, its follow-up replaced by this plan's. `w13-kit.md`: the spawner landed, with its follow-ups (a `spawner` scene component, state ownership of spawned entities). `backlog-1.x.md`: rows for rotated and templated Tiled objects, prefab transforms with overrides of a prefab's sprite, tag and tween, and scene entries that instance prefabs. `docs/known-issues.md`: a spawner's entities carry no state marker; the assets table lacks a Particle row. `roadmap.json`: the `w13cd` summary drops "plus spawners and", textually (Q20). The acceptance game's gap log is empty: nothing to mark.
- **Done-when:**
  - `rg -c '^## D-15[7] — ' DECISIONS.md` → 1; `rg -c '^\| `D-15[7]` ' docs/DECISION_INDEX.md` → 1; `rg -c '^\*\*Amended by D-15[7]:\*\*' DECISIONS.md` → 2; `rg -c 'Amended by `D-15[7]`' docs/DECISION_INDEX.md` → 2.
  - `rg -c 'Prefabs and Tiled Objects — M51' DESIGN.md` → 1; `rg -c '^\| Prefab \(' DESIGN.md` → 1; `rg -c 'object and image layers are skipped' DESIGN.md` → 0 (1 before).
  - `rg -c '^\| (Prefab|Tilemap) \|' docs/assets.md` → 2 (0 before); `rg -c 'core/assets/prefab.rs' docs/LLM_INDEX.md` → at least 1 (0 before).
  - `sed -n '/^## Break ledger/,/^## Context digest/p' docs/plans/1.0/w04-api-freeze.md | rg -c '^\| .* \| W16b \(M51'` → 5.
  - `rg -c 'M51' docs/plans/1.0/w16-prefabs.md` → at least 1; `rg -c 'Spawner' docs/plans/1.0/w13-kit.md docs/known-issues.md` → at least 1 each (0 before); `rg -c 'plus spawners and' docs/plans/1.0/roadmap.json` → 0 (1 before).
  - `just ctx` → exit 0; `just repo-check` → exit 0.
- **Moves:** none.

### Step 7: Release 0.71

- **Files:** `CHANGELOG.md`, `DESIGN.md` (its Workspace line; step 6 wrote the section), `Cargo.toml`, `Cargo.lock`, the register row, the README's four owned Now fields (`roadmap.py summary`), this plan (moved to `docs/plans/archive/`).
- **Change:** close workflow §5 and this step's evidence row with `status: done`; then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.71.0` once, every release check once after it, then the preflight with `--message`. Steps 1, 4 and 5 ran `just physics-release`, `just level-check`, the digests and `just visual`; the release adds none unless a later step changed what they cover.
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just release-preflight 0.71.0 --repo JosephGibson/Tungsten --message 'Update 0.71: M51 prefabs and Tiled objects'` → passes and prints the command block and the post-merge block.
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for the next register row, W13c and W13d: `Use the tungsten-milestone skill. Write the milestone plan for W13c and W13d: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and docs/plans/1.0/criteria.md §8.4 (W13); audit only the code it touches; name it docs/plans/1.0/phase6-milestone-52-controllers-helpers.md; critique it last as the skill's step 7 says; end with the run prompt. Don't commit: leave the changes in the tree for the release commit.`

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | What a prefab file holds | A scene entry's fields without `transform`: `sprite`, `visible`, `tag`, `tweens` (at most one) and `components`, unknown keys failing (§2.3) |
| Q2 | Where a prefab spawns | At the caller's position as `Transform.position` (a sprite's top-left, a body's `Position`), rotation 0, scale 1; a rotation or scale in prefab files waits for a user (backlog) |
| Q3 | A spawn by ID that fails | A `Result`: the ID comes from code or data and an asset can go after the load; `spawn_scene` keeps its panic on data its load checked |
| Q4 | The command-buffer form | `World::prepare_prefab`, then `CommandBuffer::spawn_prefab(PreparedPrefab)`: a system's buffer is a resource of the same World, so a form taking `&World` and `&mut CommandBuffer` together could not be called from a system |
| Q5 | `prepare_prefab`'s receiver | `&mut World`, since interning the sprite takes `&mut AssetRegistry`; components prepare first, so an error leaves the World as it was |
| Q6 | Extra components on a spawn | `PreparedPrefab::with(bundle)`, in the same archetype move: a state's `SceneEntity`, a game's additions |
| Q7 | Object classes | An empty class is a marker kept as data for the game to read; any other must name a prefab, checked at startup |
| Q8 | What properties reach | `components` only, by dotted path, class properties merging member by member, digits indexing existing arrays, in name order; no property removes a member, so a switch of a tagged variant fails the check at the leftover field, and another variant is another prefab (§2.5) |
| Q9 | Rotated and templated objects | A classed object with a rotation, any object with a template, a `type` and `class` conflict and a repeated property fail the load |
| Q10 | A tile object's position | Its top-left: under the tileset's `unspecified` or `bottomleft` alignment Tiled's bottom-left moved up by its height, under `topleft` as given, any other alignment failing the load; layer offsets add; Tiled's `visible` filters nothing |
| Q11 | `TilemapData`'s shape | `#[non_exhaustive]` with the builder of §2.6; the fields stay public; `TilemapLayer` unchanged |
| Q12 | Where map objects live | In `TilemapData::object_layers`, loaded and hot-reloaded with the map |
| Q13 | The criteria sketch's template enemy | Not added: the template has no enemy, and an inert one would be gameplay it does not have; its player comes from a prefab, and the acceptance game's enemies are its own (follow-up) |
| Q14 | The template's new dependency | `serde = { version = "1", features = ["derive"] }`, already in the lock; the guide's `Cargo.toml` block follows |
| Q15 | Example 01's object kind | The ten ambient emitters: one prefab, `ex10_ambient_emitter`, with the objects' `particle_emitter.config` and `.seed`, written by the generator; `EMITTERS` stays as the test's table |
| Q16 | The spawner's rules | The first spawn after one interval, one per finished period, at the spawner's `Transform.position`, through the command buffer; no `spawner` scene component (the game's generator places spawners in code); spawned entities carry no state marker |
| Q17 | The kit's `animation` component | Stays on the plain form: an unknown clip leaves the sprite as it is (`D-155`) |
| Q18 | Perf evidence | No timing capture: no frame runs changed code; first-run digests against the base tree, and `just visual` (§2.12) |
| Q19 | A map object that fails its check | Fails the startup load, as an unknown tileset sprite does. On hot reload an edited map keeps its last good version, and a map new to the manifest loads its tiles with the error logged, which `spawn_map_objects` then returns |
| Q20 | `roadmap.json`'s `w13cd` summary | Step 6 drops "plus spawners and", since the W16b card owns the spawner |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

157: W16b landed (M51): prefab files in a `prefabs` manifest section, spawned by ID now (`World::spawn_prefab`) or at the flush (`CommandBuffer::spawn_prefab`); Tiled object layers whose object classes name prefabs, with dotted property overrides; the asset-checked registration form with `particle_emitter` and `tilemap` on it; the `TilemapData` builder and `#[non_exhaustive]`; the kit's spawner. Amends `D-153` (prefab files carry the same components; `check_in` and the asset form) and `D-032` (object layers load). `D-132` is fulfilled, `D-103` covers the two default names and the formats as `D-153` amended it, `D-155`'s rules take the spawner, and `D-053`'s matrix gains its row. The next free ID when written; checked again when step 6 runs.

## 6. Invariants

- **May move:** none.
- **Must not move:** the determinism hash `0x088e_c07a_73c1_b168`, the containment hash `0xaee2_72e0_1ffc_4e4c`, the row digests (`integrated` `334b519e18b543d7`; `gpu` and `gpu-throughput` as the base tree reads them), `gpu-visual.png`, the post, transition and UI overlay regressions, `DefaultPlugins`' and the template's resolved schedules, and example 01's ten ambient emitters (positions, configs, seeds).

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it. Steps 2 to 6 depend on step 1; steps 3 and 4 on step 2.
- Step 1's API critique: a finding that holds changes step 1's files only, and step 1's done-when runs again; a finding that needs a file outside step 1 becomes an open question at its default.
- Pillow cannot be installed in the venv: step 4 is skipped, steps 5 and 6 run, and the run ends before the release, since the card's example 01 check is unmet.
- A row's first-run digest differs between the trees, or `just visual` fails: the run ends before the release, both readings recorded (an invariant moved).
- A file outside a step's list needs a change: restore the step and record it, except `Cargo.lock` where a listed `Cargo.toml` changes it.
- Less than 10 GiB free after `just clean-target --apply` before step 5: step 5 is skipped and the run ends before the release.
- The next decision ID is taken by another session: take the next free one and record it. Another session's changes in the tree are never reverted; the report lists them (`D-105`).

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1, 2, 3, 4, 5, release | `just physics-release` (1), `just smoke` (2, 4), `just template-check` (3), `just level-check` (4), digests and `just visual` (5); the release step runs `just check`, `just smoke`, `just script-test` and the rest of the release checks once, after the cut |
| 3 Invariants | 1, 2, 3, 5 | Hashes (1), schedules (2, 3), digests and pixels (5) |
| 4 Decisions | 6 | 157, before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 2, 6 | Core and kit snapshots (1), the umbrella's (2), five ledger rows (6) |
| 6 Template and guide | 3 | |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 6 | 86 bytes free before the step (A15) |
| 8 Design | 6 | The M51 section, the tilemap paragraph, the reload matrix; the release step's cut rewrites the Workspace line |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 6, release | The gap log is empty; `python3 -B scripts/roadmap.py summary` supplies the owned fields |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra`, 2026-10-14, reading the tree at `b6aefe4` with this plan uncommitted; the artifact was the plan without §9 and the evidence log (`--from` with the skill's `sed` cut), 60,436 characters, so `--force` (step 7). Footer: effort max, context 872,000, output cap 4,000, 429.7 s, tokens in 956,020 (849,408 cached), out 13,132 (7,753 reasoning), 36 commands, output 348.3 KB (largest 30.5 KB), usage 5h 0% → 26%, week 33% → 37%. Finding 5 changed a done-when check, so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [HIGH] Manifest reload checks prefabs before their new tilemap dependencies exist | §2.8 as written held new maps back while new prefabs loaded; startup inserts the maps (`crates/tungsten/src/asset_loader/mod.rs:203`) before anything reads them (`:412`) | Accepted | §2.8: the reload inserts new maps, then loads new prefabs against them, then checks the new maps' objects and logs a failure; Q19; step 2's test adds a map, a prefab naming it and a map naming that prefab in one edit |
| 2 | [MEDIUM] Tiled positions bypass the scene loader's numeric safeguards | `crates/tungsten-core/src/assets/scene.rs:19-21`, `:260-277`; `src/tests/assets/scene.rs:300-306` (`1e40` reads as infinity); `src/physics/plugin.rs:229-236` copies the position into `Position` | Accepted | §2.5: finite numbers, sizes at least 0 and positions within ±2^24, at load and for `origin + position`; §2.4: `PrefabError::Position`; §2.3 shares `MAX_POSITION`; A3; step 1's tests |
| 3 | [MEDIUM] Recursive overrides retain fields that invalidate a changed component variant | `crates/tungsten-core/src/physics/plugin.rs:117-122`: `SceneShape` is tagged on `kind` with `deny_unknown_fields` | Accepted | §2.5: an object value whose `kind` differs from the target's replaced it; replaced in round 2 (2.1) by a defined failure at the leftover field |
| 4 | [MEDIUM] The promised unknown-key error contract contradicts the reused deserializers | `scene.rs:82-84`: `SceneTweenChannel` denies no unknown key; `crates/tungsten-core/src/component_registry.rs:95-98`: a value follows its type's serde rules | Accepted | §2.3: `Parse` for an unknown key at the top level, in `sprite` or in a tween; tween channels and component values keep their rules, with a case of each in the test |
| 5 | [MEDIUM] The pixel-test gate expects the wrong number of results | `justfile:44-49`: `just visual` runs four targets | Accepted | Step 5's done-when: four `test result: ok` lines, each target named |

Round 2: the same reviewer and tree, 2026-10-14, at xhigh on the revised plan without §9 and the evidence log, 62,970 characters, so `--force`, with a `--focus-file` naming round 1's changes (none rejected). Footer: effort xhigh, context 872,000, output cap 4,000, 299.2 s, tokens in 807,554 (719,488 cached), out 7,560 (4,920 reasoning), 19 commands, output 208.9 KB (largest 23 KB), usage 5h 26% → 46%, week 37% → 40%. Round 2 changed no done-when check, step order or step; there is no third round (step 7).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] The `kind` replacement rule also deletes unrelated component fields | `crates/tungsten-core/src/physics/plugin.rs:124-136`: `RigidBodyFields` holds a plain `kind`, the body kind, beside the `kind`-tagged `shape`, so a class property `{ "kind": "kinematic" }` would have replaced the whole body | Accepted | §2.5 drops the replacement rule: objects only merge, no property removes a member, and a variant switch fails the check naming the leftover field (another variant is another prefab); Q8; step 1 tests a body-kind change that keeps the shape and a shape switch that fails |
| 2.2 | [MEDIUM] Tile-object positioning assumes an alignment the loader never checks | `crates/tungsten-core/src/assets/tilemap.rs:51-56`: `TiledTileset` reads no `objectalignment`; Tiled's tilesets set it | Accepted | §2.5: the loader reads the tileset's `objectalignment`; `unspecified` and `bottomleft` subtract the height, `topleft` keeps `y`, any other fails the load when a tile object uses it; Q10; A5; step 1's tests |

Step 1's API critique (level C, workflow §4): the `critique` skill, `--raw --repo --force` (the step confirms `--force`), `gpt-6-astra` at xhigh, 2026-10-14, on `git diff HEAD -- api/ templates/` after step 1 (no untracked file under `api/` or `templates/`). Footer: context 872,000, output cap 4,000, 221.4 s, tokens in 878,831 (786,432 cached), out 6,673 (3,011 reasoning), 36 commands, output 235.5 KB (largest 20.7 KB), usage 5h 46% → 66%, week 40% → 43%. The raw critique stayed in the session scratchpad (`api-critique.md`).

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| C.1 | [HIGH] Manifest-declared prefabs never reach the spawning APIs | `crates/tungsten/src/asset_loader/mod.rs:400-468`: `load_all` loads no prefab at step 1; §2.8 | Holds for step 1 alone; step 2 delivers it | No change to step 1: step 2's `load_prefabs` runs in `load_all`, and its tests spawn from a loaded manifest; step 3 spawns the template's player from its manifest through the harness |
| C.2 | [MEDIUM] The public deserialize-and-insert path bypasses tween validation | `PrefabRegistry::insert` takes data as given and `Built::new` took `tweens.first()`; `crates/tungsten-core/src/tween.rs:389-393`: `Tween::new` debug-asserts a duration > 0 | Accepted | `prepare_prefab` runs the load's checks of the prefab's own fields (at most one tween, each tween's checks) before its components, as the new `PrefabError::Invalid { id, message }`; `check_map_objects` and `prepare_map_objects` run them per object; test `a_prefab_inserted_in_code_is_checked_at_its_spawn`; step 1's done-when ran again |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-14 | Pass (after C.2) | `cargo check --workspace --all-targets --locked` exit 0; `just check` exit 0; `just physics-release` exit 0: containment 7 passed, 0 failed, 0 ignored, determinism 1/0/0, tunneling 7/0/0; the literal `rg` exit 1, no match; `#[test]` counts: core `prefab.rs` 15, `component_registry.rs` 18, `scene.rs` 21, `manifest.rs` 49, `tilemap.rs` 20, `tests/manifests.rs` 4, kit `spawner.rs` 8; `fn prefab_file_errors` 1, its 15 cases: a file that does not read (`Io`); not JSON, an unknown key at the top, in `sprite`, in a tween, a sprite without `asset_id`, a component named twice, a key repeated inside a value (`Parse`); two tweens, a tween failing its checks, an unknown component, a wrong-typed value, an unknown field under `deny_unknown_fields`, a `rigid_body` field its type denies, an asset ID the World lacks (`Validation`); `just api-check` exit 0 after `just api`; `api/tungsten.txt` and `api/tungsten-render.txt` unchanged, exit 0; snapshot counts: `World` methods 12, builder 15, `#[non_exhaustive]` `TilemapData` 1, `CommandBuffer::spawn_prefab` 2, `register_with_assets`/`check_in` 4, kit 3. Beyond the sketch: `ResolvedPrefab::new` (a `#[non_exhaustive]` type built outside core, as `ResolvedFontFamily::new`); an empty property path segment fails; the registry table's `particle_emitter` and `tilemap` rows, in `component_registry.rs`, written here. One archetype move: `World`'s archetypes are private to `ecs`, so the queued test checks the buffer's shape (one spawn, seven inserts on one target) and the end state; the live form records the same run on a fresh entity. A spawner period whose prefab fails still counts | `api/tungsten-core.txt`, `api/tungsten-kit.txt` |
| 2 | 2026-10-14 | Pass; one check's literal form reads 0 (deviation) | `just check` exit 0; `#[test]` counts `tests/prefabs.rs` 10, `tests/kit.rs` 14; the six-name `rg -c` reads 0, since rustfmt lays the six-element array out one name per line (default `array_width` 60), and the same pattern with `\s*` between the names, `rg -U -c`, reads 1: kept, not restored (the precedent of M32's index count); the `post_update` line 1; `just api-check` exit 0 after `just api`; `load_prefabs`/`reload_prefab` in `api/tungsten.txt` 2; `api/tungsten-render.txt` unchanged, exit 0; `just smoke` exit 0 (5 examples, the template, the 4-row matrix, post-stack 2/2, post-AA, bloom, lighting, game feel, mesh and transitions 5/5, benchmarks 15/15, frame cap). The reload test drives `reload_new_tilemaps` and `reload_new_prefabs`, the two blocks factored out of `reload_manifest` in its order, since the whole needs a renderer; no test captures logs, so the logged failures are asserted by their state (the map keeps its tiles, `spawn_map_objects` returns the error) | `crates/tungsten/src/asset_loader/prefab.rs`, `api/tungsten.txt` |
| 3 | 2026-10-14 | Pass | `cargo test -p tungsten-template-basic --locked` exit 0, 7 passed; `cargo test -p tungsten-core --test manifests --locked` exit 0, 4 passed; `just check` exit 0; `just template-check` exit 0 (the copy builds, tests and smoke-runs, its startup load taking the prefab, 28.6 s); `just repo-check` exit 0; `spawn_with` in `states.rs` 0, `prepare_prefab` 1; the template's `post_update` line 1; the guide's `prepare_prefab\|spawn_prefab` lines 2, `^serde = ` 1; `prefab` in the template's `AGENTS.md` 1. `Cargo.lock` gains the template's `serde` edge only | `templates/basic/`, `docs/getting-started.md` |
| 4 | 2026-10-14 | Pass | Pillow 12.3.0 in a scratchpad venv; `generate.py` wrote 582 outputs, removed 0, changing only `manifest.json`, `level.tmj`, `level_layout.rs`, `outputs.json` and the new `prefabs/ambient_emitter.json`; `PATH=… just level-check` exit 0: "OK: 582 owned outputs match; art, level, references and budgets validated", 15 unittests OK (35.5 s); `cargo test -p example-01-platformer --locked` exit 0, 106 passed (`death.rs` needed no change); `cargo test -p tungsten-core --test manifests --locked` exit 0, 4 passed; `jq` object counts `[10]`; `EMITTERS` in `setup.rs` 0, `spawn_map_objects` 1; "No Tiled object layers" 0; `just check` exit 0; `just smoke` exit 0, its example 01 log loading `ex10_ambient_emitter` with no error. `load_presentation_assets` builds `ParticlesPlugin` over a throwaway schedule (a no-op on an app's World) for the engine's `particle_emitter` on bare test Worlds, and inserts the level map only when the test seeded none. `Cargo.lock` gains the example's `serde` edge | `examples/01_platformer/` |
| 5 | 2026-10-14 | Pass | `df -h .` 145 GiB free; base tree `git archive HEAD` (`b6aefe4`, 1,286 files) in `target/m51-base`; one run each, `--repeat 1 --allow-background` (the owner's `nxcodec.bin` up, allowed for digests): every capture valid, exit 0; first-run digests base → candidate: `integrated` `334b519e18b543d7` → `334b519e18b543d7`, `gpu` `f45f26ebdf706840` → `f45f26ebdf706840`, `gpu-throughput` `121f8ef1503d8697` → `121f8ef1503d8697`; each copy checked by `diff -rq`, exit 0; `target/m51-base` removed with `shutil.rmtree`; `git status --porcelain` 62 lines before and after the sitting. `just visual` exit 0, four `test result: ok` lines: `visual_regression` 4 passed, `post_regression` 5, `transition_regression` 1, `ui_regression` 4, with the one `load_shaders 'fade'` ERROR line the test provokes | `perf-runs/20261014-m51-digests/{base,cand}/` (machine-local) |
| 6 | 2026-10-14 | Pass | `D-157` the next free ID when written (`D-156` the last); `^## D-15[7] — ` 1, its index row 1, `**Amended by D-15[7]:**` 2 (on `D-032` and `D-153`), the index's "Amended by" notes 2; DESIGN: the M51 section 1, the Prefab row 1, "object and image layers are skipped" 0; `docs/assets.md` Tilemap and Prefab rows 2; `core/assets/prefab.rs` in the LLM index 1, the index at 12,280 of 12,288 bytes after trimming six phrases there; ledger rows from `W16b (M51` 5; `M51` in `w16-prefabs.md` 5; `Spawner` in `w13-kit.md` 3 and in `known-issues.md` 1; "plus spawners and" 0 (now "plus a per-entity state machine"); `just ctx` exit 0; `just repo-check` exit 0. The acceptance game's gap log is empty: nothing to mark | `DECISIONS.md`, `DESIGN.md`, `docs/` |
| 7 | 2026-10-14 | Pass (closure; the cut, the release checks and the preflight follow in the session's report, since the plan is archived before the cut) | Workflow §5: (1) rows 1–6, with step 2's one literal check recorded; (2) gates by change: `just physics-release` (1), `just smoke` (2, 4), `just template-check` (3), `just level-check` (4), the digests and `just visual` (5), and the release checks once after the cut; (3) invariants unchanged: both hashes (1), `DefaultPlugins`' and the template's schedules (2, 3), the three digests and the pixel tests (5), example 01's ten emitters (4); (4) `D-157` written before the docs that cite it; (5) five ledger rows, rustdoc on every new public item, `api/` regenerated and reviewed; (6) the template and the guide (3); (7) the LLM index's prefab route, `just ctx`; (8) DESIGN's M51 section, with the status line's capability; (9) known issues, the backlog, the W13 and W16 files and the register row (0.71, cut), and the README's owned fields at the cut; (10) one `[Unreleased]` entry, `status: done`, the plan archived | `CHANGELOG.md`, `docs/plans/1.0/implementation-plan.md` |

## Follow-ups

1. **The assets table lacks a Particle row** (A15): step 6 adds the Tilemap and Prefab rows and records the Particle row in [known issues](../../../known-issues.md); home: the Phase 6 QA pass.
2. **Spawned entities and states** (Q16): a spawner's entities carry no `SceneEntity`, so a state's exit leaves them; home: W13c and W13d, which add more spawning helpers, or the acceptance game's first gap.
3. **The kit's `animation` component on the asset form** (Q17): home: W12b, when example 01 moves its animations onto the kit.
4. **The generator's Pillow**: `just level-check` needs Pillow 12.3.0, which the reference machine's Python lacks; a run sets up a scratch venv (step 4); home: the authoring guide's next edit.
