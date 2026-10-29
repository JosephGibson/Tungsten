//! Registered components: the types a scene entry's or a prefab file's
//! `components` object names (W16a, M48; W16b, M51).
//!
//! A [`ComponentRegistry`] is a `World` resource keyed by name and by
//! `TypeId`, one name per type and one type per name. Three doors fill it:
//! `App::register_component` in a game's `main.rs`,
//! [`World::register_component`], [`World::register_component_with`] and
//! [`World::register_component_with_assets`] in a plugin's `build`, and the
//! registry's own methods for one built without a World. A component's value
//! is JSON that deserializes by serde's own rules;
//! [`check`](ComponentRegistry::check) is that deserialization alone, and
//! [`prepare`](ComponentRegistry::prepare) also builds what spawns, so the
//! two agree on every value.
//!
//! A value may name assets by their manifest IDs. A component registered
//! with [`register_with_assets`](ComponentRegistry::register_with_assets)
//! checks those IDs against a World: [`check_in`](ComponentRegistry::check_in)
//! runs the check after deserializing, and `prepare` runs it against the
//! World it spawns into before it builds, so a `build` may `expect` the IDs
//! it is given. An ID the World lacks is [`ComponentError::MissingAsset`].
//!
//! A spawn prepares every component with `&World`, then queues each
//! [`PreparedComponent`] on the `CommandBuffer`, so no resource leaves the
//! World and a value that fails stops the spawn before anything is queued.
//!
//! # The engine's components
//!
//! Each registers where the code that reads it lives, so a plugin set
//! without its owner drops the name. The shapes are file format (`D-103`):
//! unknown fields fail, every number must be finite, and a default is
//! written as `= value`.
//!
//! | Name | Registered by | Value | Spawns |
//! | --- | --- | --- | --- |
//! | `rigid_body` | `PhysicsPlugin` | `kind`: `"dynamic"`, `"static"`, `"kinematic"` or `"sensor"`; `shape`: `{ "kind": "aabb", "half_extents": [x, y] }` or `{ "kind": "circle", "radius": r }`, sizes in (0, 16384]; `offset`: `[x, y]` within ±16384 `= [0, 0]`; `mass` > 0 with a finite inverse `= 1`, dynamic only; `restitution` in 0..=1 `= 0`, no effect on a sensor; `velocity`: `[x, y]`, `= [0, 0]` on a dynamic or kinematic body or a sensor and none on a static one; `layers`, the layers the body is in, and `mask`, the layers it meets: lists of layer numbers 0 to 31, `= [0]` and every layer (`D-178`) | `RigidBodyBundle` at the entry's position; the transform's scale does not size the collider |
//! | `light` | `App`, beside the light extract | `{ "kind": "point", "radius": r, "color": [r, g, b], "intensity": i }` or `{ "kind": "directional", "angle": a, "color": [r, g, b], "intensity": i }`; `radius` > 0, channels >= 0, `intensity` >= 0 `= 1` | `Light` |
//! | `parallax_layer` | `App`, beside the sprite extract | `{ "scroll_factor": [x, y] }` | `ParallaxLayer` |
//! | `squash_stretch` | `GameFeelPlugin` | `{ "on": t, "amount": [x, y], "duration": d, "easing": e }`: `on` one of `on_land`, `on_hit`, `on_pickup`, `manual`; `amount` > 0 on both axes; `duration` > 0; `easing` as tweens name it `= "linear"` | `SpriteSquashStretch` |
//! | `particle_emitter` | `ParticlesPlugin` | `{ "config": id, "seed": s }`: `config` a manifest `particles` ID, checked against the World; `seed` a `u64` `= none`, which takes the config's seed, then the World's | `ParticleEmitter` and a fresh `ParticleEmitterState` |
//! | `tilemap` | `App`, beside the tilemap extract | `{ "id": id }`: a manifest `tilemaps` ID, checked against the World | `TilemapInstance` with the entry's position as the map's top-left; the map's objects spawn through `World::spawn_map_objects` |

use std::any::{TypeId, type_name};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::rc::Rc;

use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::components::Transform;
use crate::ecs::{Bundle, CommandBuffer, Entity, PendingEntity, World};

/// A built bundle, waiting for its entity.
type Insert = Box<dyn FnOnce(&mut CommandBuffer, Target)>;

/// Deserializes a value, checks the asset IDs it names against the
/// context's World and builds its bundle.
type Prepare = Box<dyn Fn(&Value, &SpawnContext<'_>) -> Result<Insert, Fail>>;

/// Deserializes a value and, given a World, checks the asset IDs it names.
type Check = Box<dyn Fn(&Value, Option<&World>) -> Result<(), Fail>>;

/// A definition's asset check, shared by its `Check` and its `Prepare`.
type AssetCheck<D> = dyn Fn(&D, &World) -> Result<(), MissingAsset>;

/// How many entities hold a registered type.
type Count = fn(&World) -> usize;

/// Where a built bundle goes: a buffer's pending spawn, or a live entity
/// (a prefab spawned at once).
#[derive(Clone, Copy)]
enum Target {
    Pending(PendingEntity),
    Live(Entity),
}

/// One registered name.
struct Registration {
    type_id: TypeId,
    type_name: &'static str,
    check: Check,
    prepare: Prepare,
    /// For a type registered as itself; a definition's build may spawn
    /// any types.
    count: Option<Count>,
}

/// Why a value does not resolve: where it stopped deserializing and serde's
/// error, or an asset ID it names that the World lacks.
enum Fail {
    Invalid {
        field: String,
        source: serde_json::Error,
    },
    Missing(MissingAsset),
}

impl Fail {
    fn into_error(self, name: &str) -> ComponentError {
        match self {
            Fail::Invalid { field, source } => ComponentError::Invalid {
                name: name.to_owned(),
                field,
                source,
            },
            Fail::Missing(missing) => ComponentError::MissingAsset {
                name: name.to_owned(),
                field: missing.field,
                kind: missing.kind,
                id: missing.id,
            },
        }
    }
}

/// The component types that scene entries and prefab files name in their
/// `components` object: a `World` resource keyed by name and by `TypeId`,
/// one name per type and one type per name. `Debug` prints the names.
#[derive(Default)]
pub struct ComponentRegistry {
    /// Name → registration, sorted by name.
    entries: BTreeMap<String, Registration>,
    /// The registered type → its name.
    names_by_type: HashMap<TypeId, String>,
}

impl ComponentRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `components.<name>` deserializes to `T`, inserted as is. Returns whether
    /// the pair is new; the same name and type again is a no-op.
    ///
    /// The value follows serde's rules for `T`: a struct reads an object, a
    /// unit struct (a marker) reads `null` and refuses `{}`, and an unknown
    /// field fails only when `T` has `#[serde(deny_unknown_fields)]`. Checks
    /// beyond the types belong in deserialization (`#[serde(try_from = …)]`).
    ///
    /// ```
    /// use serde::Deserialize;
    /// use serde_json::json;
    /// use tungsten_core::ComponentRegistry;
    ///
    /// #[derive(Deserialize)]
    /// struct Player;
    ///
    /// #[derive(Deserialize)]
    /// #[serde(deny_unknown_fields)]
    /// struct Health {
    ///     max: u32,
    /// }
    ///
    /// let mut registry = ComponentRegistry::new();
    /// assert!(registry.register::<Player>("player"));
    /// assert!(registry.register::<Health>("health"));
    /// assert!(!registry.register::<Health>("health"));
    ///
    /// assert!(registry.check("player", &json!(null)).is_ok());
    /// assert!(registry.check("player", &json!({})).is_err());
    /// assert!(registry.check("health", &json!({ "max": 10 })).is_ok());
    /// assert!(registry.check("health", &json!({ "max": 10, "regen": 1 })).is_err());
    /// ```
    ///
    /// # Panics
    ///
    /// When `name` is not `[a-z][a-z0-9_]*`, already names another type, or
    /// `T` already has another name; the message names both (`D-022`).
    #[track_caller]
    pub fn register<T: DeserializeOwned + 'static>(&mut self, name: &str) -> bool {
        self.insert_registration(
            name,
            None,
            |value: T, _: &SpawnContext<'_>| (value,),
            Some(count_of::<T>),
        )
    }

    /// `components.<name>` deserializes to `D`, and `build` turns it and the
    /// entity's [`SpawnContext`] into the bundle that spawns: for a definition
    /// that becomes several components or reads the entity's transform or the
    /// World. Keyed by `D`'s `TypeId`; returns and panics as
    /// [`register`](Self::register) does.
    #[track_caller]
    pub fn register_with<D, B>(
        &mut self,
        name: &str,
        build: impl Fn(D, &SpawnContext<'_>) -> B + 'static,
    ) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle,
    {
        self.insert_registration(name, None, build, None)
    }

    /// [`register_with`](Self::register_with), plus `assets`: the check of
    /// the asset IDs a definition names against a World, for a component
    /// whose value names a manifest entry. [`check_in`](Self::check_in) and
    /// [`prepare`](Self::prepare) run it after deserializing, `prepare`
    /// against the World it spawns into, before `build`; so `build` may
    /// `expect` the IDs it is given. [`check`](Self::check) skips it.
    ///
    /// ```
    /// use serde::Deserialize;
    /// use serde_json::json;
    /// use tungsten_core::{ComponentError, ComponentRegistry, MissingAsset, SpawnContext, World};
    ///
    /// /// The sounds a game loaded, by ID.
    /// struct Sounds(Vec<String>);
    ///
    /// #[derive(Deserialize)]
    /// struct Chime {
    ///     sound: String,
    /// }
    ///
    /// let mut registry = ComponentRegistry::new();
    /// registry.register_with_assets(
    ///     "chime",
    ///     |chime: &Chime, world: &World| {
    ///         let sounds = world.get_resource::<Sounds>();
    ///         if sounds.is_some_and(|sounds| sounds.0.contains(&chime.sound)) {
    ///             Ok(())
    ///         } else {
    ///             Err(MissingAsset::new("sound", "sound", &chime.sound))
    ///         }
    ///     },
    ///     |chime: Chime, _: &SpawnContext<'_>| (chime,),
    /// );
    /// let mut world = World::new();
    /// world.insert_resource(Sounds(vec!["bell".to_string()]));
    ///
    /// assert!(registry.check_in("chime", &json!({ "sound": "bell" }), &world).is_ok());
    /// // `check` reads the shape alone.
    /// assert!(registry.check("chime", &json!({ "sound": "gong" })).is_ok());
    /// let err = registry
    ///     .check_in("chime", &json!({ "sound": "gong" }), &world)
    ///     .unwrap_err();
    /// assert!(matches!(err, ComponentError::MissingAsset { .. }));
    /// assert_eq!(
    ///     err.to_string(),
    ///     "component `chime`, field `sound`: unknown sound `gong`"
    /// );
    /// ```
    ///
    /// # Panics
    ///
    /// As [`register`](Self::register).
    #[track_caller]
    pub fn register_with_assets<D, B>(
        &mut self,
        name: &str,
        assets: impl Fn(&D, &World) -> Result<(), MissingAsset> + 'static,
        build: impl Fn(D, &SpawnContext<'_>) -> B + 'static,
    ) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle,
    {
        self.insert_registration(name, Some(Rc::new(assets)), build, None)
    }

    /// The registration behind every door: the name and type checks, then
    /// the check and the prepare built for `D`, with `assets` when given.
    #[track_caller]
    fn insert_registration<D, B>(
        &mut self,
        name: &str,
        assets: Option<Rc<AssetCheck<D>>>,
        build: impl Fn(D, &SpawnContext<'_>) -> B + 'static,
        count: Option<Count>,
    ) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle,
    {
        let type_id = TypeId::of::<D>();
        let type_name = type_name::<D>();
        assert!(
            valid_name(name),
            "component name `{name}` for `{type_name}` must match [a-z][a-z0-9_]*"
        );
        if let Some(existing) = self.entries.get(name) {
            assert!(
                existing.type_id == type_id,
                "component name `{name}` already names `{}`; `{type_name}` cannot take it",
                existing.type_name
            );
            return false;
        }
        if let Some(existing) = self.names_by_type.get(&type_id) {
            panic!(
                "`{type_name}` is already registered as component `{existing}`; it cannot also be `{name}`"
            );
        }
        let check_assets = assets.clone();
        let check: Check = Box::new(move |value, world| {
            let def = deserialize::<D>(value)?;
            if let (Some(assets), Some(world)) = (&check_assets, world) {
                assets(&def, world).map_err(Fail::Missing)?;
            }
            Ok(())
        });
        let prepare: Prepare = Box::new(move |value, at| {
            let def = deserialize::<D>(value)?;
            if let Some(assets) = &assets {
                assets(&def, at.world()).map_err(Fail::Missing)?;
            }
            let bundle = build(def, at);
            let insert: Insert = Box::new(move |buffer: &mut CommandBuffer, target| match target {
                Target::Pending(pending) => buffer.insert_bundle_pending(pending, bundle),
                Target::Live(entity) => buffer.insert_bundle(entity, bundle),
            });
            Ok(insert)
        });
        self.names_by_type.insert(type_id, name.to_owned());
        self.entries.insert(
            name.to_owned(),
            Registration {
                type_id,
                type_name,
                check,
                prepare,
                count,
            },
        );
        true
    }

    /// Whether `name` is registered.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(name)
    }

    /// The name `T` is registered under, as a component or as a
    /// [`register_with`](Self::register_with) definition.
    #[must_use]
    pub fn name_of<T: 'static>(&self) -> Option<&str> {
        self.names_by_type
            .get(&TypeId::of::<T>())
            .map(String::as_str)
    }

    /// The registered names, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// How many names are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// How many of `world`'s entities hold the component registered under
    /// `name`. `None` for an unknown name, and for one registered with a
    /// definition ([`register_with`](Self::register_with) and
    /// [`register_with_assets`](Self::register_with_assets)), whose build
    /// may spawn any types.
    #[must_use]
    pub fn count(&self, name: &str, world: &World) -> Option<usize> {
        self.entries.get(name)?.count.map(|count| count(world))
    }

    /// Whether `value` deserializes as the type registered under `name`. A
    /// tool without a World checks shapes with it; the asset IDs a value
    /// names wait for [`check_in`](Self::check_in).
    pub fn check(&self, name: &str, value: &Value) -> Result<(), ComponentError> {
        (self.entry(name)?.check)(value, None).map_err(|fail| fail.into_error(name))
    }

    /// [`check`](Self::check), then, for a name registered with
    /// [`register_with_assets`](Self::register_with_assets), the asset IDs
    /// the value names against `world`.
    pub fn check_in(&self, name: &str, value: &Value, world: &World) -> Result<(), ComponentError> {
        (self.entry(name)?.check)(value, Some(world)).map_err(|fail| fail.into_error(name))
    }

    /// Deserializes `value` as `name`'s type, checks the asset IDs it names
    /// against `at`'s World, and builds what it spawns, owned and ready to
    /// queue: the caller prepares with `&World`, then queues with the
    /// `CommandBuffer`, so neither resource leaves the World.
    pub fn prepare(
        &self,
        name: &str,
        value: &Value,
        at: &SpawnContext<'_>,
    ) -> Result<PreparedComponent, ComponentError> {
        let insert =
            (self.entry(name)?.prepare)(value, at).map_err(|fail| fail.into_error(name))?;
        Ok(PreparedComponent {
            name: name.to_owned(),
            insert,
        })
    }

    fn entry(&self, name: &str) -> Result<&Registration, ComponentError> {
        self.entries
            .get(name)
            .ok_or_else(|| ComponentError::Unknown {
                name: name.to_owned(),
            })
    }
}

impl fmt::Debug for ComponentRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentRegistry")
            .field("names", &self.names().collect::<Vec<_>>())
            .finish()
    }
}

/// One registered component built for one entity. `Debug` prints its name.
pub struct PreparedComponent {
    name: String,
    insert: Insert,
}

impl PreparedComponent {
    /// Queues the built bundle on `pending`, one `insert_bundle_pending`.
    pub fn insert_pending(self, buffer: &mut CommandBuffer, pending: PendingEntity) {
        (self.insert)(buffer, Target::Pending(pending));
    }

    /// Queues the built bundle on the live `entity`, one `insert_bundle`: a
    /// prefab spawned at once records its run on a fresh entity.
    pub(crate) fn insert_live(self, buffer: &mut CommandBuffer, entity: Entity) {
        (self.insert)(buffer, Target::Live(entity));
    }
}

impl fmt::Debug for PreparedComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedComponent")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// What a registered component's `build` sees of the entity it spawns on and
/// of the World it spawns into. Its fields are private, so it can grow.
pub struct SpawnContext<'w> {
    world: &'w World,
    transform: Transform,
}

impl<'w> SpawnContext<'w> {
    /// The context of an entity at `transform`, spawning into `world`.
    #[must_use]
    pub fn new(world: &'w World, transform: Transform) -> Self {
        Self { world, transform }
    }

    /// The entity's transform: a scene entry's own.
    #[must_use]
    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    /// The World, read only: where a component resolves the asset IDs its
    /// value names. For one registered with
    /// [`ComponentRegistry::register_with_assets`], its check has passed
    /// against this World before `build` runs.
    #[must_use]
    pub fn world(&self) -> &'w World {
        self.world
    }
}

/// An asset ID that a component's value names and the World does not hold:
/// what the check of [`ComponentRegistry::register_with_assets`] returns,
/// and what [`ComponentError::MissingAsset`] reports with the component's
/// name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MissingAsset {
    /// The value's field that names the asset, as
    /// [`ComponentError::Invalid`] names fields (`config`, `layers.0`).
    pub field: String,
    /// The asset kind as messages print it: `"particle config"`, `"tilemap"`.
    pub kind: &'static str,
    /// The ID the value gave.
    pub id: String,
}

impl MissingAsset {
    /// The ID `id` of kind `kind`, named by the value's `field`.
    #[must_use]
    pub fn new(field: impl Into<String>, kind: &'static str, id: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            kind,
            id: id.into(),
        }
    }
}

/// A `components` value that does not resolve.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ComponentError {
    /// No component is registered under `name`. Prints
    /// "unknown component `<name>`".
    #[error("unknown component `{name}`")]
    Unknown {
        /// The name the value gave.
        name: String,
    },
    /// The value does not deserialize as `name`'s type. Prints
    /// "component `<name>`, field `<field>`: <source>", or
    /// "component `<name>`: <source>" when `field` is empty.
    #[error("component `{name}`{}: {source}", field_note(.field))]
    Invalid {
        /// The component's name.
        name: String,
        /// The path inside the value where deserialization stopped, as
        /// `serde_path_to_error` gives it (`max`, `shape.radius`), empty at
        /// the value's top.
        field: String,
        /// Serde's error.
        source: serde_json::Error,
    },
    /// The value names an asset the World does not hold, by the check of
    /// [`ComponentRegistry::register_with_assets`]. Prints
    /// "component `<name>`, field `<field>`: unknown \<kind\> `<id>`", or
    /// without the field when `field` is empty.
    #[error("component `{name}`{}: unknown {kind} `{id}`", field_note(.field))]
    MissingAsset {
        /// The component's name.
        name: String,
        /// The value's field that names the asset.
        field: String,
        /// The asset kind: `"particle config"`, `"tilemap"`.
        kind: &'static str,
        /// The ID the value gave.
        id: String,
    },
}

fn field_note(field: &str) -> String {
    if field.is_empty() {
        String::new()
    } else {
        format!(", field `{field}`")
    }
}

impl World {
    /// [`ComponentRegistry::register`] on this World's registry, which it
    /// inserts first when the World has none, so a plugin built on a bare
    /// World registers.
    ///
    /// ```
    /// use serde::Deserialize;
    /// use serde_json::json;
    /// use tungsten_core::{ComponentRegistry, World};
    ///
    /// /// A game component; a scene entry gives it as
    /// /// `"components": { "health": { "max": 10 } }`.
    /// #[derive(Deserialize)]
    /// struct Health {
    ///     max: u32,
    /// }
    ///
    /// let mut world = World::new();
    /// assert!(world.register_component::<Health>("health"));
    /// let registry = world.resource::<ComponentRegistry>();
    /// assert!(registry.check("health", &json!({ "max": 10 })).is_ok());
    /// let err = registry.check("health", &json!({ "max": "ten" })).unwrap_err();
    /// assert_eq!(
    ///     err.to_string(),
    ///     "component `health`, field `max`: invalid type: string \"ten\", expected u32"
    /// );
    /// ```
    ///
    /// # Panics
    ///
    /// As [`ComponentRegistry::register`].
    #[track_caller]
    pub fn register_component<T: DeserializeOwned + 'static>(&mut self, name: &str) -> bool {
        registry_mut(self).register::<T>(name)
    }

    /// [`ComponentRegistry::register_with`] on this World's registry, which
    /// it inserts first when the World has none.
    ///
    /// # Panics
    ///
    /// As [`ComponentRegistry::register`].
    #[track_caller]
    pub fn register_component_with<D, B>(
        &mut self,
        name: &str,
        build: impl Fn(D, &SpawnContext<'_>) -> B + 'static,
    ) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle,
    {
        registry_mut(self).register_with(name, build)
    }

    /// [`ComponentRegistry::register_with_assets`] on this World's registry,
    /// which it inserts first when the World has none.
    ///
    /// # Panics
    ///
    /// As [`ComponentRegistry::register`].
    #[track_caller]
    pub fn register_component_with_assets<D, B>(
        &mut self,
        name: &str,
        assets: impl Fn(&D, &World) -> Result<(), MissingAsset> + 'static,
        build: impl Fn(D, &SpawnContext<'_>) -> B + 'static,
    ) -> bool
    where
        D: DeserializeOwned + 'static,
        B: Bundle,
    {
        registry_mut(self).register_with_assets(name, assets, build)
    }
}

fn registry_mut(world: &mut World) -> &mut ComponentRegistry {
    if !world.has_resource::<ComponentRegistry>() {
        world.insert_resource(ComponentRegistry::new());
    }
    world.resource_mut::<ComponentRegistry>()
}

fn count_of<T: 'static>(world: &World) -> usize {
    world.query::<&T>().count()
}

/// `[a-z][a-z0-9_]*`.
fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn deserialize<D: DeserializeOwned>(value: &Value) -> Result<D, Fail> {
    serde_path_to_error::deserialize(value).map_err(|err| {
        let field = if err.path().iter().next().is_none() {
            String::new()
        } else {
            err.path().to_string()
        };
        Fail::Invalid {
            field,
            source: err.into_inner(),
        }
    })
}

#[cfg(test)]
#[path = "tests/component_registry.rs"]
mod tests;
