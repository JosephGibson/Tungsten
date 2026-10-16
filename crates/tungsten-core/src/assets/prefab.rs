//! Prefab files (W16b, M51): an entity kind in a file of its own, listed in
//! the manifest's `prefabs` section and spawned by ID, at once
//! ([`World::spawn_prefab`]) or at the frame's flush
//! ([`CommandBuffer::spawn_prefab`]); and the spawn of a Tiled map's
//! objects, whose classes name prefabs ([`World::spawn_map_objects`]).
//!
//! A prefab file holds a scene entry's fields without `transform`, parsed as
//! strictly as a scene (`D-153`): unknown keys fail, and so do a component
//! named twice and a key repeated inside a value.
//!
//! ```json
//! {
//!   "sprite": { "asset_id": "slime", "z_order": 2 },
//!   "tag": "enemy",
//!   "components": {
//!     "enemy": null,
//!     "rigid_body": { "kind": "dynamic", "shape": { "kind": "circle", "radius": 6 } }
//!   }
//! }
//! ```
//!
//! A spawn places the prefab at the caller's position as
//! `Transform.position`, with rotation 0 and scale 1, and gives the entity,
//! in order: the `Transform`, the `Sprite`, `Visibility`, the `Tag`, the
//! `Tween`, the components in name order, then each [`PreparedPrefab::with`]
//! bundle, in one archetype move. As in a scene entry, a later value wins
//! over an earlier one of its type. Every component is prepared against the
//! World before anything changes, so a spawn that fails leaves the World as
//! it was.
//!
//! A spawn queued from a source ([`CommandBuffer::spawn_prefab_from`]), such
//! as a spawner's, then takes what the World's spawn inheritance gives from
//! that source as the flush finds it ([`World::inherit_from_source`]), a
//! state's marker say, each type the spawn lacks, in one more move; a source
//! an earlier command despawned drops the spawn.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

use glam::Vec2;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::assets::data;
use crate::assets::registry::AssetRegistry;
use crate::assets::scene::{
    MAX_POSITION, SceneSprite, SceneTween, default_visible, strict_components,
    within_position_bound,
};
use crate::assets::tilemap::{MapObject, TilemapData, TilemapRegistry};
use crate::component_registry::{
    ComponentError, ComponentRegistry, PreparedComponent, SpawnContext,
};
use crate::components::{Sprite, Tag, Transform, Visibility};
use crate::ecs::{Bundle, CommandBuffer, Entity, PendingEntity, World};
use crate::tween::Tween;

/// One prefab file: a scene entry's fields without `transform`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct PrefabData {
    /// The sprite, by its manifest ID. The spawn interns the name, so a
    /// sprite that a later manifest reload registers draws from then on.
    #[serde(default)]
    pub sprite: Option<SceneSprite>,
    /// `Visibility.visible`, `= true`.
    #[serde(default = "default_visible")]
    pub visible: bool,
    /// The entity's `Tag`.
    #[serde(default)]
    pub tag: Option<String>,
    /// At most one tween, which spawns as a `Tween`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tweens: Vec<SceneTween>,
    /// Registered components by name, as a scene entry's: each value
    /// deserializes as the type its name was registered with and spawns
    /// after the fields above, in name order.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "strict_components"
    )]
    pub components: BTreeMap<String, Value>,
}

impl PrefabData {
    /// Reads and parses `path`, then checks its tweens and each component
    /// with `ComponentRegistry::check_in` against `world` (an empty registry
    /// when the World has none, so any component is unknown). The first
    /// error stops the load.
    ///
    /// A file that does not read is [`PrefabError::Io`]. Text that is not
    /// JSON, an unknown key at the top level, in `sprite` or in a tween, a
    /// component named twice and a key repeated inside a value are
    /// [`PrefabError::Parse`]. More than one tween, a tween that fails its
    /// checks, an unknown component, a value that does not deserialize and
    /// an asset ID that `world` lacks are [`PrefabError::Validation`], whose
    /// message names `tweens`, `tweens[0]` or the [`ComponentError`]; a
    /// missing prefab or data entry's ID ends with the nearest one, as
    /// "; did you mean `<id>`?", when one is near.
    ///
    /// A prefab that names other prefabs or data entries loads with them in
    /// one [`StagedLoad`](crate::assets::data::StagedLoad), which checks
    /// after every file is in place.
    pub fn load(path: &Path, world: &World) -> Result<Self, PrefabError> {
        let data = Self::parse(path)?;
        data.validate(world)
            .map_err(|message| PrefabError::Validation {
                path: path.display().to_string(),
                message,
            })?;
        Ok(data)
    }

    /// Phase one of a load: reads and parses `path`, with no World check.
    pub(crate) fn parse(path: &Path) -> Result<Self, PrefabError> {
        let file = || path.display().to_string();
        let contents = std::fs::read_to_string(path).map_err(|source| PrefabError::Io {
            path: file(),
            source,
        })?;
        serde_json::from_str(&contents).map_err(|source| PrefabError::Parse {
            path: file(),
            source,
        })
    }

    /// Phase two of a load: the fields' checks, then each component's
    /// `check_in` against `world`, a missing prefab or data ID with the
    /// nearest one appended.
    pub(crate) fn validate(&self, world: &World) -> Result<(), String> {
        self.validate_fields()?;
        let empty = ComponentRegistry::new();
        let registry = world.get_resource::<ComponentRegistry>().unwrap_or(&empty);
        for (name, value) in &self.components {
            registry.check_in(name, value, world).map_err(|err| {
                let hint = match &err {
                    ComponentError::MissingAsset { kind, id, .. } => {
                        data::suggestion(world, kind, id)
                    }
                    _ => None,
                };
                format!("{err}{}", data::did_you_mean(hint.as_deref()))
            })?;
        }
        Ok(())
    }

    /// The checks of the prefab's own fields: at most one tween, and each
    /// tween's own. [`load`](Self::load) runs them, and every spawn runs them
    /// again, since data deserialized in code and inserted into the
    /// [`PrefabRegistry`] skips the load.
    fn validate_fields(&self) -> Result<(), String> {
        if self.tweens.len() > 1 {
            return Err(format!(
                "tweens: a prefab holds at most one tween (got {})",
                self.tweens.len()
            ));
        }
        for (i, tween) in self.tweens.iter().enumerate() {
            tween
                .validate()
                .map_err(|msg| format!("tweens[{i}]: {msg}"))?;
        }
        Ok(())
    }
}

/// The loaded prefabs by ID, a `World` resource: what
/// [`World::prepare_prefab`] reads. The umbrella's loader fills it from the
/// manifest's `prefabs` section. Each file backs one prefab.
#[derive(Debug, Default)]
pub struct PrefabRegistry {
    prefabs: BTreeMap<String, PrefabData>,
    /// Each prefab loaded from a file: the file, by ID.
    paths: BTreeMap<String, PathBuf>,
    /// The reverse of `paths`: the prefab each file routes to.
    path_to_id: HashMap<PathBuf, String>,
}

impl PrefabRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `data` as prefab `id`, replacing a prefab of the same ID and
    /// dropping the file it was loaded from. The data is taken as given:
    /// each spawn checks it, no load's commit does, and hot reload routes no
    /// file to it.
    pub fn insert(&mut self, id: String, data: PrefabData) {
        self.drop_path(&id);
        self.prefabs.insert(id, data);
    }

    /// [`insert`](Self::insert), keeping `path` for
    /// [`id_for_path`](Self::id_for_path), which hot reload reads. The ID's
    /// old file no longer routes to it, and a prefab that held `path` loses
    /// it.
    pub fn insert_with_path(&mut self, id: String, data: PrefabData, path: PathBuf) {
        self.drop_path(&id);
        self.release_path(&path);
        self.put_entry(id, data, Some(path));
    }

    /// The prefab `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&PrefabData> {
        self.prefabs.get(id)
    }

    /// The prefab IDs, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.prefabs.keys().map(String::as_str)
    }

    /// The ID of the prefab loaded from `path`.
    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(String::as_str)
    }

    /// How many prefabs are loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.prefabs.len()
    }

    /// Whether no prefab is loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prefabs.is_empty()
    }

    /// Drops the route of the file prefab `id` was loaded from.
    fn drop_path(&mut self, id: &str) {
        if let Some(path) = self.paths.remove(id) {
            self.path_to_id.remove(&path);
        }
    }

    /// The prefab `path` routes to loses the file, keeping its data.
    pub(crate) fn release_path(&mut self, path: &Path) -> Option<String> {
        let id = self.path_to_id.remove(path)?;
        self.paths.remove(&id);
        Some(id)
    }

    /// Gives prefab `id` the file `path` back.
    pub(crate) fn give_path(&mut self, id: &str, path: PathBuf) {
        if self.prefabs.contains_key(id) {
            self.path_to_id.insert(path.clone(), id.to_owned());
            self.paths.insert(id.to_owned(), path);
        }
    }

    /// Removes prefab `id` with its file's route.
    pub(crate) fn take_entry(&mut self, id: &str) -> Option<(PrefabData, Option<PathBuf>)> {
        let data = self.prefabs.remove(id)?;
        let path = self.paths.remove(id);
        if let Some(path) = &path {
            self.path_to_id.remove(path);
        }
        Some((data, path))
    }

    /// Puts `data` as prefab `id`, routing `path` to it; the caller has
    /// freed the path and the ID's old one.
    pub(crate) fn put_entry(&mut self, id: String, data: PrefabData, path: Option<PathBuf>) {
        if let Some(path) = path {
            self.path_to_id.insert(path.clone(), id.clone());
            self.paths.insert(id.clone(), path);
        }
        self.prefabs.insert(id, data);
    }

    /// Every prefab loaded from a file, by ID, with its file.
    pub(crate) fn file_backed(&self) -> impl Iterator<Item = (&str, &PrefabData, &Path)> {
        self.paths.iter().filter_map(|(id, path)| {
            self.prefabs
                .get(id)
                .map(|data| (id.as_str(), data, path.as_path()))
        })
    }
}

/// A bundle a [`PreparedPrefab::with`] call added, waiting for its entity.
type Extra = Box<dyn FnOnce(&mut CommandBuffer, At)>;

/// Where a prepared prefab's run goes: a buffer's pending spawn, or a
/// fresh live entity.
#[derive(Clone, Copy)]
enum At {
    Pending(PendingEntity),
    Live(Entity),
}

impl At {
    fn insert<T: 'static>(self, buffer: &mut CommandBuffer, value: T) {
        match self {
            At::Pending(pending) => buffer.insert_pending(pending, value),
            At::Live(entity) => buffer.insert(entity, value),
        }
    }
}

/// One prefab spawn, built: its fields and every component prepared against
/// the World, nothing spawned yet. Spawn it now with
/// [`spawn`](Self::spawn), or queue it with [`CommandBuffer::spawn_prefab`].
/// `Debug` prints the ID.
pub struct PreparedPrefab {
    id: String,
    transform: Transform,
    sprite: Option<Sprite>,
    visibility: Visibility,
    tag: Option<Tag>,
    tween: Option<Tween>,
    components: Vec<PreparedComponent>,
    extras: Vec<Extra>,
}

impl PreparedPrefab {
    /// The prefab's ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Adds `bundle` after the prefab's own components, in the same
    /// archetype move: a state's `SceneEntity`, a game's own additions. A
    /// type the prefab already gives takes the bundle's value.
    #[must_use]
    pub fn with<B: Bundle>(mut self, bundle: B) -> Self {
        self.extras.push(Box::new(move |buffer, at| match at {
            At::Pending(pending) => buffer.insert_bundle_pending(pending, bundle),
            At::Live(entity) => buffer.insert_bundle(entity, bundle),
        }));
        self
    }

    /// Spawns it now, in one archetype move, and returns the entity.
    pub fn spawn(self, world: &mut World) -> Entity {
        let entity = world.spawn();
        let mut buffer = CommandBuffer::new();
        self.record(&mut buffer, At::Live(entity));
        world.flush(buffer);
        entity
    }

    /// Records the run: every value on one target, back to back, so the
    /// flush moves the entity once (`D-084`).
    fn record(self, buffer: &mut CommandBuffer, at: At) {
        let Self {
            transform,
            sprite,
            visibility,
            tag,
            tween,
            components,
            extras,
            ..
        } = self;
        at.insert(buffer, transform);
        if let Some(sprite) = sprite {
            at.insert(buffer, sprite);
        }
        at.insert(buffer, visibility);
        if let Some(tag) = tag {
            at.insert(buffer, tag);
        }
        if let Some(tween) = tween {
            at.insert(buffer, tween);
        }
        for component in components {
            match at {
                At::Pending(pending) => component.insert_pending(buffer, pending),
                At::Live(entity) => component.insert_live(buffer, entity),
            }
        }
        for extra in extras {
            extra(buffer, at);
        }
    }
}

impl fmt::Debug for PreparedPrefab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedPrefab")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// A prefab spawn built against the World, its sprite name not yet
/// interned: everything that can fail has passed.
struct Built {
    id: String,
    transform: Transform,
    sprite: Option<SceneSprite>,
    visible: bool,
    tag: Option<String>,
    tween: Option<Tween>,
    components: Vec<PreparedComponent>,
}

impl Built {
    /// Prepares `components`, the prefab's or a map object's overrides of
    /// them, at `position`, reading `world` only.
    fn new(
        world: &World,
        id: &str,
        data: &PrefabData,
        components: &BTreeMap<String, Value>,
        position: Vec2,
    ) -> Result<Self, ComponentError> {
        let empty = ComponentRegistry::new();
        let registry = world.get_resource::<ComponentRegistry>().unwrap_or(&empty);
        let transform = Transform::from_position(position);
        let at = SpawnContext::new(world, transform);
        let components = components
            .iter()
            .map(|(name, value)| registry.prepare(name, value, &at))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            id: id.to_owned(),
            transform,
            sprite: data.sprite.clone(),
            visible: data.visible,
            tag: data.tag.clone(),
            tween: data.tweens.first().map(SceneTween::into_tween),
            components,
        })
    }

    /// Interns the sprite's name: the one step that changes the World.
    fn intern(self, world: &mut World) -> PreparedPrefab {
        let sprite = self.sprite.map(|sprite| {
            let registry = world
                .get_resource_mut::<AssetRegistry>()
                .unwrap_or_else(|| {
                    panic!(
                        "prefab `{}` names sprite `{}` and the World has no AssetRegistry",
                        self.id, sprite.asset_id
                    )
                });
            Sprite {
                asset_id: registry.intern_sprite(&sprite.asset_id),
                color: sprite.color,
                z_order: sprite.z_order,
                material_id: None,
            }
        });
        PreparedPrefab {
            id: self.id,
            transform: self.transform,
            sprite,
            visibility: Visibility {
                visible: self.visible,
            },
            tag: self.tag.map(Tag::new),
            tween: self.tween,
            components: self.components,
            extras: Vec::new(),
        }
    }
}

impl World {
    /// Builds a spawn of prefab `id` at `position`, its top-left as
    /// `Transform.position`: its fields, and each component prepared against
    /// this World with a `SpawnContext` at that transform. Nothing spawns
    /// until [`PreparedPrefab::spawn`] or [`CommandBuffer::spawn_prefab`].
    /// The sprite's name is interned only after every component has
    /// prepared, so an error leaves the World as it was.
    ///
    /// # Errors
    ///
    /// [`PrefabError::Unknown`] when the World has no [`PrefabRegistry`] or
    /// no prefab `id`; [`PrefabError::Position`] when `position` is not
    /// finite or lies past ±2^24 on either axis, a scene transform's bound;
    /// [`PrefabError::Invalid`] when the prefab's own fields fail the checks
    /// a load runs (data inserted in code); [`PrefabError::Component`] for
    /// the first component that does not prepare, such as one naming an
    /// asset the World no longer holds.
    ///
    /// # Panics
    ///
    /// When the prefab names a sprite and the World has no `AssetRegistry`
    /// (`D-022`).
    pub fn prepare_prefab(
        &mut self,
        id: &str,
        position: impl Into<Vec2>,
    ) -> Result<PreparedPrefab, PrefabError> {
        let position = position.into();
        let built = {
            let data = self
                .get_resource::<PrefabRegistry>()
                .and_then(|prefabs| prefabs.get(id))
                .ok_or_else(|| PrefabError::Unknown { id: id.to_owned() })?;
            if !within_position_bound(position.x, position.y) {
                return Err(PrefabError::Position {
                    id: id.to_owned(),
                    position,
                });
            }
            data.validate_fields()
                .map_err(|message| PrefabError::Invalid {
                    id: id.to_owned(),
                    message,
                })?;
            Built::new(self, id, data, &data.components, position).map_err(|source| {
                PrefabError::Component {
                    id: id.to_owned(),
                    source,
                }
            })?
        };
        Ok(built.intern(self))
    }

    /// Spawns prefab `id` at `position` now, in one archetype move:
    /// [`prepare_prefab`](Self::prepare_prefab), then
    /// [`PreparedPrefab::spawn`]. A system spawns through its
    /// `CommandBuffer` instead ([`CommandBuffer::spawn_prefab`]).
    ///
    /// ```
    /// use serde::Deserialize;
    /// use serde_json::json;
    /// use tungsten_core::{PrefabData, PrefabRegistry, Transform, World};
    ///
    /// #[derive(Debug, Deserialize, PartialEq)]
    /// struct Gem {
    ///     value: u32,
    /// }
    ///
    /// let mut world = World::new();
    /// world.register_component::<Gem>("gem");
    /// // A prefab file's contents; a game's loader reads them from the
    /// // manifest's `prefabs` section.
    /// let gem: PrefabData =
    ///     serde_json::from_value(json!({ "components": { "gem": { "value": 5 } } })).unwrap();
    /// let mut prefabs = PrefabRegistry::new();
    /// prefabs.insert("gem".to_string(), gem);
    /// world.insert_resource(prefabs);
    ///
    /// let entity = world.spawn_prefab("gem", [40.0, 16.0]).unwrap();
    /// assert_eq!(world.get::<Gem>(entity), Some(&Gem { value: 5 }));
    /// assert_eq!(world.get::<Transform>(entity).unwrap().position.x, 40.0);
    /// let err = world.spawn_prefab("ruby", [0.0, 0.0]).unwrap_err();
    /// assert_eq!(err.to_string(), "unknown prefab `ruby`");
    /// ```
    ///
    /// # Errors
    ///
    /// As [`prepare_prefab`](Self::prepare_prefab).
    ///
    /// # Panics
    ///
    /// As [`prepare_prefab`](Self::prepare_prefab).
    pub fn spawn_prefab(
        &mut self,
        id: &str,
        position: impl Into<Vec2>,
    ) -> Result<Entity, PrefabError> {
        Ok(self.prepare_prefab(id, position)?.spawn(self))
    }

    /// Checks every object of `data` that has a class against this World,
    /// as [`prepare_map_objects`](Self::prepare_map_objects) would build
    /// it: its prefab exists, its properties apply, and each resulting
    /// component passes `ComponentRegistry::check_in`. `map` names the map
    /// in errors. A load runs it once the prefabs have loaded.
    ///
    /// The prefab's own fields are checked as a spawn checks them.
    ///
    /// # Errors
    ///
    /// [`PrefabError::Object`] for the first object that fails.
    pub fn check_map_objects(&self, map: &str, data: &TilemapData) -> Result<(), PrefabError> {
        let empty = ComponentRegistry::new();
        let registry = self.get_resource::<ComponentRegistry>().unwrap_or(&empty);
        for_each_classed_object(self, map, data, |object, prefab, components| {
            prefab
                .validate_fields()
                .map_err(|message| format!("prefab `{}`: {message}", object.class))?;
            for (name, value) in components {
                registry
                    .check_in(name, value, self)
                    .map_err(|err| format!("prefab `{}`: {err}", object.class))?;
            }
            Ok(())
        })
    }

    /// Prepares every object that has a class in the `TilemapRegistry`'s map
    /// `map`, in layer and file order: its prefab at `origin + position`,
    /// with its properties over the prefab's components. An object without
    /// a class is a marker the game reads from
    /// [`TilemapData::object_layers`]. The sprites' names are interned only
    /// after every object has prepared, so an error leaves the World as it
    /// was. Each `PreparedPrefab` may go to [`CommandBuffer::spawn_prefab`].
    ///
    /// Properties reach `components` only, applied in name order. A name is
    /// a dotted path: its first segment names a component, added as `{}`
    /// when the prefab lacks it, and each later segment steps into an
    /// object's member (created as `{}` while segments remain) or, when all
    /// digits, an existing array element. The value replaces what the path
    /// names, except an object (a Tiled class property), which merges member
    /// by member; no property removes a member, so a class property that
    /// switches a tagged value's variant fails the check at the leftover
    /// field. A one-segment name needs a class value. The prefab's sprite,
    /// visibility, tag and tween stay as its file gives them.
    ///
    /// # Errors
    ///
    /// [`PrefabError::UnknownMap`] when the map is not registered;
    /// [`PrefabError::Object`] for the first object whose prefab is
    /// unknown or fails the checks of its own fields, whose property does
    /// not apply, whose position lies past ±2^24, or whose component fails.
    ///
    /// # Panics
    ///
    /// As [`prepare_prefab`](Self::prepare_prefab).
    pub fn prepare_map_objects(
        &mut self,
        map: &str,
        origin: impl Into<Vec2>,
    ) -> Result<Vec<PreparedPrefab>, PrefabError> {
        let origin = origin.into();
        let built = {
            let data = self
                .get_resource::<TilemapRegistry>()
                .and_then(|maps| maps.get(map))
                .ok_or_else(|| PrefabError::UnknownMap {
                    map: map.to_owned(),
                })?;
            let mut built = Vec::new();
            for_each_classed_object(self, map, data, |object, prefab, components| {
                prefab
                    .validate_fields()
                    .map_err(|message| format!("prefab `{}`: {message}", object.class))?;
                let position = origin + object.position;
                if !within_position_bound(position.x, position.y) {
                    return Err(format!(
                        "prefab `{}`: position {position} must be finite and within ±{MAX_POSITION}",
                        object.class
                    ));
                }
                let next = Built::new(self, &object.class, prefab, components, position)
                    .map_err(|err| format!("prefab `{}`: {err}", object.class))?;
                built.push(next);
                Ok(())
            })?;
            built
        };
        Ok(built.into_iter().map(|next| next.intern(self)).collect())
    }

    /// [`prepare_map_objects`](Self::prepare_map_objects), then spawns each
    /// now; the entities in the same order.
    ///
    /// # Errors
    ///
    /// As [`prepare_map_objects`](Self::prepare_map_objects), with nothing
    /// spawned.
    ///
    /// # Panics
    ///
    /// As [`prepare_prefab`](Self::prepare_prefab).
    pub fn spawn_map_objects(
        &mut self,
        map: &str,
        origin: impl Into<Vec2>,
    ) -> Result<Vec<Entity>, PrefabError> {
        let prepared = self.prepare_map_objects(map, origin)?;
        Ok(prepared
            .into_iter()
            .map(|prefab| prefab.spawn(self))
            .collect())
    }
}

impl CommandBuffer {
    /// Queues `prefab`: one spawn and its inserts, back to back, which the
    /// flush applies as one archetype move (`D-084`). The form a system
    /// takes: [`World::prepare_prefab`], then this on the World's buffer.
    ///
    /// ```
    /// use serde_json::json;
    /// use tungsten_core::{CommandBuffer, PrefabData, PrefabRegistry, Tag, World};
    ///
    /// let mut world = World::new();
    /// let bolt: PrefabData = serde_json::from_value(json!({ "tag": "bolt" })).unwrap();
    /// let mut prefabs = PrefabRegistry::new();
    /// prefabs.insert("bolt".to_string(), bolt);
    /// world.insert_resource(prefabs);
    ///
    /// let prepared = world.prepare_prefab("bolt", [8.0, 8.0]).unwrap();
    /// let mut commands = CommandBuffer::new();
    /// commands.spawn_prefab(prepared);
    /// world.flush(commands);
    /// let tags: Vec<_> = world.query::<&Tag>().map(|tag| tag.name.as_str()).collect();
    /// assert_eq!(tags, ["bolt"]);
    /// ```
    pub fn spawn_prefab(&mut self, prefab: PreparedPrefab) -> PendingEntity {
        let pending = self.spawn();
        prefab.record(self, At::Pending(pending));
        pending
    }

    /// Queues `prefab` as [`spawn_prefab`](Self::spawn_prefab) does, spawned
    /// from `source`: at the flush, after its own fields and `with` bundles,
    /// the entity takes what the World's spawn inheritance
    /// ([`World::inherit_from_source`], [`World::derive_from_source`]) gives
    /// from `source` as the flush finds it, in one more archetype move when
    /// anything is given. A source an earlier command despawned drops the
    /// spawn: nothing of it remains.
    ///
    /// ```
    /// use serde_json::json;
    /// use tungsten_core::{CommandBuffer, PrefabData, PrefabRegistry, World};
    ///
    /// #[derive(Clone, Debug, PartialEq)]
    /// struct Team(u8);
    ///
    /// let mut world = World::new();
    /// let bolt: PrefabData = serde_json::from_value(json!({ "tag": "bolt" })).unwrap();
    /// let mut prefabs = PrefabRegistry::new();
    /// prefabs.insert("bolt".to_string(), bolt);
    /// world.insert_resource(prefabs);
    /// world.inherit_from_source::<Team>();
    /// let turret = world.spawn_with((Team(2),));
    ///
    /// let prepared = world.prepare_prefab("bolt", [8.0, 8.0]).unwrap();
    /// let mut commands = CommandBuffer::new();
    /// commands.spawn_prefab_from(turret, prepared);
    /// world.flush(commands);
    /// let teams: Vec<_> = world.query::<&Team>().collect();
    /// assert_eq!(teams, [&Team(2), &Team(2)], "the turret's and the bolt's");
    /// ```
    pub fn spawn_prefab_from(&mut self, source: Entity, prefab: PreparedPrefab) -> PendingEntity {
        let pending = self.spawn_prefab(prefab);
        self.derive_pending(source, pending);
        pending
    }
}

/// Visits every object of `data` with a class, in layer and file order:
/// the object, its prefab, and the prefab's components with the object's
/// properties applied. The first error, the visitor's included, returns as
/// an [`PrefabError::Object`].
fn for_each_classed_object(
    world: &World,
    map: &str,
    data: &TilemapData,
    mut visit: impl FnMut(&MapObject, &PrefabData, &BTreeMap<String, Value>) -> Result<(), String>,
) -> Result<(), PrefabError> {
    let prefabs = world.get_resource::<PrefabRegistry>();
    for layer in &data.object_layers {
        for object in layer
            .objects
            .iter()
            .filter(|object| !object.class.is_empty())
        {
            let resolve = || {
                let prefab = prefabs
                    .and_then(|prefabs| prefabs.get(&object.class))
                    .ok_or_else(|| format!("unknown prefab `{}`", object.class))?;
                let components = apply_properties(&prefab.components, &object.properties)?;
                Ok((prefab, components))
            };
            resolve()
                .and_then(|(prefab, components)| visit(object, prefab, &components))
                .map_err(|message| PrefabError::Object {
                    map: map.to_owned(),
                    layer: layer.name.clone(),
                    object: object.id,
                    message,
                })?;
        }
    }
    Ok(())
}

/// The prefab's `components` with the object's `properties` applied, in
/// property-name order, so a field property applies after its
/// component's class property.
fn apply_properties(
    components: &BTreeMap<String, Value>,
    properties: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    let mut components = components.clone();
    for (name, value) in properties {
        apply_property(&mut components, name, value)
            .map_err(|why| format!("property `{name}`: {why}"))?;
    }
    Ok(components)
}

fn apply_property(
    components: &mut BTreeMap<String, Value>,
    name: &str,
    value: &Value,
) -> Result<(), String> {
    let segments: Vec<&str> = name.split('.').collect();
    if segments.iter().any(|segment| segment.is_empty()) {
        return Err("a path segment is empty".to_owned());
    }
    let (component, fields) = segments
        .split_first()
        .expect("split yields at least one segment");
    if fields.is_empty() && !value.is_object() {
        return Err(format!(
            "a value that is not a class must name a field: `{component}.<field>`"
        ));
    }
    let mut current = components
        .entry((*component).to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    let mut path = (*component).to_owned();
    for (i, segment) in fields.iter().enumerate() {
        let last = i + 1 == fields.len();
        current = match current {
            Value::Object(members) => {
                if last {
                    match members.get_mut(*segment) {
                        Some(target) => merge(target, value),
                        None => {
                            members.insert((*segment).to_owned(), value.clone());
                        }
                    }
                    return Ok(());
                }
                members
                    .entry((*segment).to_owned())
                    .or_insert_with(|| Value::Object(Map::new()))
            }
            Value::Array(items) => {
                if !segment.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(format!(
                        "`{path}` is an array, which `{segment}` cannot step into"
                    ));
                }
                let len = items.len();
                let item = segment
                    .parse::<usize>()
                    .ok()
                    .and_then(|index| items.get_mut(index))
                    .ok_or_else(|| format!("`{path}` has no element {segment} (it has {len})"))?;
                if last {
                    merge(item, value);
                    return Ok(());
                }
                item
            }
            other => {
                return Err(format!(
                    "`{path}` is {}, which `{segment}` cannot step into",
                    describe(other)
                ));
            }
        };
        path.push('.');
        path.push_str(segment);
    }
    // One segment: a class value over the whole component.
    merge(current, value);
    Ok(())
}

/// `value` over `target`: an object merges member by member, recursively,
/// so no member goes; anything else replaces what is there.
fn merge(target: &mut Value, value: &Value) {
    match (target, value) {
        (Value::Object(target), Value::Object(members)) => {
            for (key, member) in members {
                match target.get_mut(key) {
                    Some(existing) => merge(existing, member),
                    None => {
                        target.insert(key.clone(), member.clone());
                    }
                }
            }
        }
        (target, value) => *target = value.clone(),
    }
}

/// A JSON value's kind, as an error names what a path could not step into.
fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a bool",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// A prefab that does not load, resolve or spawn.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PrefabError {
    /// The file does not read.
    #[error("failed to read prefab '{path}': {source}")]
    Io {
        /// The prefab file.
        path: String,
        /// The read's error.
        source: std::io::Error,
    },
    /// The file is not a prefab's JSON: a syntax error, an unknown key, a
    /// component named twice or a key repeated inside a value.
    #[error("invalid prefab '{path}': {source}")]
    Parse {
        /// The prefab file.
        path: String,
        /// Serde's error, with its line and column.
        source: serde_json::Error,
    },
    /// The file parses and fails a check: its tweens, or a component
    /// against the World.
    #[error("invalid prefab '{path}': {message}")]
    Validation {
        /// The prefab file.
        path: String,
        /// What failed: `tweens`, `tweens[0]` or the component's error.
        message: String,
    },
    /// No prefab of this ID is loaded.
    #[error("unknown prefab `{id}`")]
    Unknown {
        /// The ID asked for.
        id: String,
    },
    /// A component of the prefab does not prepare against the World.
    #[error("prefab `{id}`: {source}")]
    Component {
        /// The prefab's ID.
        id: String,
        /// The component's error.
        source: ComponentError,
    },
    /// The prefab's own fields fail the checks a load runs: more than one
    /// tween, or a tween that fails its own; data deserialized in code and
    /// inserted into the registry skips the load.
    #[error("prefab `{id}`: {message}")]
    Invalid {
        /// The prefab's ID.
        id: String,
        /// What failed: `tweens` or `tweens[0]`.
        message: String,
    },
    /// The spawn position is not finite or lies past ±2^24 on an axis.
    #[error(
        "prefab `{id}`: position {position} must be finite and within ±{}",
        MAX_POSITION
    )]
    Position {
        /// The prefab's ID.
        id: String,
        /// The position asked for.
        position: Vec2,
    },
    /// No tilemap of this ID is registered.
    #[error("unknown tilemap `{map}`")]
    UnknownMap {
        /// The map's ID.
        map: String,
    },
    /// An object of a map does not resolve: its prefab, a property, its
    /// position or a component.
    #[error("tilemap `{map}`, layer `{layer}`, object {object}: {message}")]
    Object {
        /// The map's ID.
        map: String,
        /// The object layer's name.
        layer: String,
        /// The object's Tiled ID.
        object: u32,
        /// What failed, naming the prefab, the property, or the component
        /// and its field.
        message: String,
    },
}

#[cfg(test)]
#[path = "../tests/assets/prefab.rs"]
mod tests;
