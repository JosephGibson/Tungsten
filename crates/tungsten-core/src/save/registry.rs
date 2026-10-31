//! The saved components and resources: the types a world save holds, each
//! under a name, with its version, its migration steps and the references
//! it hangs on entities.
//!
//! A [`SaveRegistry`] is a `World` resource with two maps, components and
//! resources, each one name per type and one type per name. A game
//! registers in one line, `App::register_saved::<T>(name)`; a plugin in its
//! `build` through [`World::register_saved`] and
//! [`World::register_saved_with`], whose [`SaveOptions`] add a version,
//! migration steps, a saved form and a [`RefCodec`].

use std::any::{TypeId, type_name};
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::rc::Rc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::id::SaveId;
use crate::ecs::{Entity, World};

/// One migration step: rewrites a value saved at version `from` into
/// version `from + 1`'s shape. A named entity's step sees a patch (the
/// fields its run changed), a run's spawn's the whole value, so a step
/// rewrites the fields it is given and assumes none is present.
pub type MigrationStep = fn(Value) -> Result<Value, String>;

/// A value's references in a save: each key the value hangs references on,
/// with the save IDs under it, in the value's order.
pub type Refs = BTreeMap<String, Vec<SaveId>>;

impl Entity {
    /// An entity no World holds: what a saved form gives a reference before
    /// the restore's second pass sets it.
    pub const PLACEHOLDER: Entity = Entity {
        index: u32::MAX,
        generation: u32::MAX,
    };
}

/// How a value's entity references save and load: by the save IDs of the
/// entities they name, each under the key it hangs on in the value (an
/// effect's serial, a hit's activation), so a reference into an area not
/// loaded waits under its key until the area loads, and drops once the
/// value no longer has the key.
pub struct RefCodec<T> {
    /// Writes each reference under its key through [`RefWriter::put`].
    pub save: fn(&T, &mut RefWriter<'_>),
    /// Sets the references [`RefReader`] resolved. A restore calls it once
    /// on the value its saved form gave; an area's entry may call it again
    /// with the references that resolved then, which it adds.
    pub load: fn(&mut T, &RefReader<'_>),
    /// The keys the value holds now.
    pub keys: fn(&T) -> Vec<String>,
}

impl<T> Clone for RefCodec<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for RefCodec<T> {}

/// What a [`RefCodec`]'s `save` writes into.
pub struct RefWriter<'a> {
    ids: &'a dyn Fn(Entity) -> Option<SaveId>,
    refs: Refs,
    dropped: usize,
}

impl<'a> RefWriter<'a> {
    pub(crate) fn new(ids: &'a dyn Fn(Entity) -> Option<SaveId>) -> Self {
        Self {
            ids,
            refs: Refs::new(),
            dropped: 0,
        }
    }

    /// Writes `entity` under `key` by its save ID; an entity without one is
    /// dropped and counted.
    pub fn put(&mut self, key: &str, entity: Entity) {
        match (self.ids)(entity) {
            Some(id) => self.refs.entry(key.to_owned()).or_default().push(id),
            None => self.dropped += 1,
        }
    }

    pub(crate) fn finish(self) -> (Refs, usize) {
        (self.refs, self.dropped)
    }
}

/// What a [`RefCodec`]'s `load` reads: each key's references that resolved
/// to live entities, in their saved order.
pub struct RefReader<'a> {
    refs: &'a BTreeMap<String, Vec<Entity>>,
}

impl<'a> RefReader<'a> {
    pub(crate) fn new(refs: &'a BTreeMap<String, Vec<Entity>>) -> Self {
        Self { refs }
    }

    /// The entities under `key`; empty for a key with none.
    #[must_use]
    pub fn get(&self, key: &str) -> &[Entity] {
        self.refs.get(key).map_or(&[], Vec::as_slice)
    }

    /// The first entity under `key`.
    #[must_use]
    pub fn first(&self, key: &str) -> Option<Entity> {
        self.get(key).first().copied()
    }

    /// Each key with its entities, by key.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &[Entity])> {
        self.refs
            .iter()
            .map(|(key, entities)| (key.as_str(), entities.as_slice()))
    }
}

/// A registration's version, migration steps, saved form and references.
pub struct SaveOptions<T> {
    version: u32,
    steps: BTreeMap<u32, MigrationStep>,
    refs: Option<RefCodec<T>>,
    to: fn(&T) -> serde_json::Result<Value>,
    from: fn(Value) -> serde_json::Result<T>,
}

impl<T: Serialize + DeserializeOwned + 'static> SaveOptions<T> {
    /// Version 1, no migration, no references, `T`'s own serde form.
    #[must_use]
    pub fn new() -> Self {
        Self::with_form(to_json::<T>, serde_json::from_value::<T>)
    }
}

impl<T: Serialize + DeserializeOwned + 'static> Default for SaveOptions<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> SaveOptions<T> {
    /// Version 1, saved through the form `F`: a serde type `T` converts to
    /// and from, for a type with no serde form of its own or one whose
    /// save leaves out what its file format holds.
    #[must_use]
    pub fn form<F>() -> Self
    where
        F: Serialize + DeserializeOwned + for<'a> From<&'a T>,
        T: From<F>,
    {
        Self::with_form(to_form::<T, F>, from_form::<T, F>)
    }

    fn with_form(
        to: fn(&T) -> serde_json::Result<Value>,
        from: fn(Value) -> serde_json::Result<T>,
    ) -> Self {
        Self {
            version: 1,
            steps: BTreeMap::new(),
            refs: None,
            to,
            from,
        }
    }

    /// The version a save writes; 1 by default.
    ///
    /// # Panics
    ///
    /// When `version` is 0.
    #[must_use]
    #[track_caller]
    pub fn version(mut self, version: u32) -> Self {
        assert!(version > 0, "a saved version starts at 1");
        self.version = version;
        self
    }

    /// The step from version `from` to `from + 1`.
    #[must_use]
    pub fn step(mut self, from: u32, step: MigrationStep) -> Self {
        self.steps.insert(from, step);
        self
    }

    /// The value's entity references.
    #[must_use]
    pub fn refs(mut self, codec: RefCodec<T>) -> Self {
        self.refs = Some(codec);
        self
    }
}

fn to_json<T: Serialize>(value: &T) -> serde_json::Result<Value> {
    serde_json::to_value(value)
}

fn to_form<T, F: Serialize + for<'a> From<&'a T>>(value: &T) -> serde_json::Result<Value> {
    serde_json::to_value(F::from(value))
}

fn from_form<T: From<F>, F: DeserializeOwned>(value: Value) -> serde_json::Result<T> {
    serde_json::from_value::<F>(value).map(T::from)
}

/// Where a registered value lives: on an entity, or as the resource.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Holder {
    Entity(Entity),
    Resource,
}

/// A registration with its type erased.
pub(crate) trait Erased {
    fn has(&self, world: &World, at: Holder) -> bool;
    fn to_value(&self, world: &World, at: Holder) -> Option<serde_json::Result<Value>>;
    fn check(&self, value: Value) -> serde_json::Result<()>;
    fn put(&self, world: &mut World, at: Holder, value: Value) -> serde_json::Result<()>;
    fn remove(&self, world: &mut World, at: Holder);
    fn has_refs(&self) -> bool;
    fn save_refs(
        &self,
        world: &World,
        at: Holder,
        ids: &dyn Fn(Entity) -> Option<SaveId>,
    ) -> Option<(Refs, usize)>;
    fn load_refs(&self, world: &mut World, at: Holder, refs: &BTreeMap<String, Vec<Entity>>);
    // Kept areas' pending references (redo 7.9's step 3) read it.
    #[allow(dead_code)]
    fn keys(&self, world: &World, at: Holder) -> Option<Vec<String>>;
}

struct Typed<T> {
    to: fn(&T) -> serde_json::Result<Value>,
    from: fn(Value) -> serde_json::Result<T>,
    refs: Option<RefCodec<T>>,
    _marker: PhantomData<T>,
}

fn get<T: 'static>(world: &World, at: Holder) -> Option<&T> {
    match at {
        Holder::Entity(entity) => world.get::<T>(entity),
        Holder::Resource => world.get_resource::<T>(),
    }
}

fn get_mut<T: 'static>(world: &mut World, at: Holder) -> Option<&mut T> {
    match at {
        Holder::Entity(entity) => world.get_mut::<T>(entity),
        Holder::Resource => world.get_resource_mut::<T>(),
    }
}

impl<T: 'static> Erased for Typed<T> {
    fn has(&self, world: &World, at: Holder) -> bool {
        get::<T>(world, at).is_some()
    }

    fn to_value(&self, world: &World, at: Holder) -> Option<serde_json::Result<Value>> {
        get::<T>(world, at).map(self.to)
    }

    fn check(&self, value: Value) -> serde_json::Result<()> {
        (self.from)(value).map(drop)
    }

    fn put(&self, world: &mut World, at: Holder, value: Value) -> serde_json::Result<()> {
        let value = (self.from)(value)?;
        match at {
            Holder::Entity(entity) => world.insert(entity, value),
            Holder::Resource => world.insert_resource(value),
        }
        Ok(())
    }

    fn remove(&self, world: &mut World, at: Holder) {
        match at {
            Holder::Entity(entity) => {
                world.remove_component::<T>(entity);
            }
            Holder::Resource => {
                world.remove_resource::<T>();
            }
        }
    }

    fn has_refs(&self) -> bool {
        self.refs.is_some()
    }

    fn save_refs(
        &self,
        world: &World,
        at: Holder,
        ids: &dyn Fn(Entity) -> Option<SaveId>,
    ) -> Option<(Refs, usize)> {
        let codec = self.refs?;
        let value = get::<T>(world, at)?;
        let mut writer = RefWriter::new(ids);
        (codec.save)(value, &mut writer);
        Some(writer.finish())
    }

    fn load_refs(&self, world: &mut World, at: Holder, refs: &BTreeMap<String, Vec<Entity>>) {
        let Some(codec) = self.refs else {
            return;
        };
        if let Some(value) = get_mut::<T>(world, at) {
            (codec.load)(value, &RefReader::new(refs));
        }
    }

    fn keys(&self, world: &World, at: Holder) -> Option<Vec<String>> {
        let codec = self.refs?;
        get::<T>(world, at).map(codec.keys)
    }
}

/// One registered type.
#[derive(Clone)]
pub(crate) struct Entry {
    pub(crate) type_name: &'static str,
    pub(crate) version: u32,
    pub(crate) steps: BTreeMap<u32, MigrationStep>,
    pub(crate) saved: Rc<dyn Erased>,
}

impl Entry {
    /// `value`, saved at version `from`, a step at a time to this entry's
    /// version.
    pub(crate) fn migrate(&self, from: u32, mut value: Value) -> Result<Value, String> {
        for version in from..self.version {
            let step = self
                .steps
                .get(&version)
                .ok_or_else(|| format!("no migration step from version {version}"))?;
            value = step(value).map_err(|err| format!("version {version}: {err}"))?;
        }
        Ok(value)
    }
}

/// One of the registry's two maps: entries by name, names by type.
#[derive(Clone, Default)]
pub(crate) struct Kind {
    pub(crate) entries: BTreeMap<String, Entry>,
    names: HashMap<TypeId, String>,
}

impl Kind {
    #[track_caller]
    fn register<T: 'static>(&mut self, kind: &str, name: &str, options: SaveOptions<T>) -> bool {
        let type_id = TypeId::of::<T>();
        let type_name = type_name::<T>();
        assert!(
            valid_name(name),
            "saved {kind} name `{name}` for `{type_name}` must match [a-z][a-z0-9_]*"
        );
        if let Some(existing) = self.names.get(&type_id) {
            assert!(
                existing == name,
                "`{type_name}` is already saved as {kind} `{existing}`; it cannot also be `{name}`"
            );
            return false;
        }
        if let Some(existing) = self.entries.get(name) {
            panic!(
                "saved {kind} name `{name}` already names `{}`; `{type_name}` cannot take it",
                existing.type_name
            );
        }
        if let Some(&from) = options.steps.keys().find(|&&from| from >= options.version) {
            panic!(
                "saved {kind} `{name}` has a step from version {from}, at or past its version {}",
                options.version
            );
        }
        self.names.insert(type_id, name.to_owned());
        self.entries.insert(
            name.to_owned(),
            Entry {
                type_name,
                version: options.version,
                steps: options.steps,
                saved: Rc::new(Typed {
                    to: options.to,
                    from: options.from,
                    refs: options.refs,
                    _marker: PhantomData,
                }),
            },
        );
        true
    }
}

/// The saved components and resources, a `World` resource
/// [`World::register_saved`] inserts at its first registration.
#[derive(Clone, Default)]
pub struct SaveRegistry {
    pub(crate) components: Kind,
    pub(crate) resources: Kind,
}

impl SaveRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Every saved component's name, sorted.
    pub fn component_names(&self) -> impl Iterator<Item = &str> {
        self.components.entries.keys().map(String::as_str)
    }

    /// Every saved resource's name, sorted.
    pub fn resource_names(&self) -> impl Iterator<Item = &str> {
        self.resources.entries.keys().map(String::as_str)
    }

    /// The name `T` is saved under as a component.
    #[must_use]
    pub fn component_name<T: 'static>(&self) -> Option<&str> {
        self.components
            .names
            .get(&TypeId::of::<T>())
            .map(String::as_str)
    }

    /// The name `T` is saved under as a resource.
    #[must_use]
    pub fn resource_name<T: 'static>(&self) -> Option<&str> {
        self.resources
            .names
            .get(&TypeId::of::<T>())
            .map(String::as_str)
    }

    /// Saved component `name`'s version.
    #[must_use]
    pub fn component_version(&self, name: &str) -> Option<u32> {
        self.components.entries.get(name).map(|entry| entry.version)
    }

    /// Saved resource `name`'s version.
    #[must_use]
    pub fn resource_version(&self, name: &str) -> Option<u32> {
        self.resources.entries.get(name).map(|entry| entry.version)
    }

    /// Whether saved component `name` has a reference codec.
    #[must_use]
    pub fn component_has_refs(&self, name: &str) -> bool {
        self.components
            .entries
            .get(name)
            .is_some_and(|entry| entry.saved.has_refs())
    }

    /// Whether saved resource `name` has a reference codec.
    #[must_use]
    pub fn resource_has_refs(&self, name: &str) -> bool {
        self.resources
            .entries
            .get(name)
            .is_some_and(|entry| entry.saved.has_refs())
    }

    /// Saves component `T` under `name`, with `options`; returns whether it
    /// was new.
    ///
    /// # Panics
    ///
    /// When `name` is not `[a-z][a-z0-9_]*`, already names another type, or
    /// `T` already has another name, or a step starts at or past the
    /// version (`D-022`).
    #[track_caller]
    pub fn register<T: 'static>(&mut self, name: &str, options: SaveOptions<T>) -> bool {
        self.components.register("component", name, options)
    }

    /// Saves resource `T` under `name`, with `options`; returns and panics
    /// as [`register`](Self::register).
    #[track_caller]
    pub fn register_resource<T: 'static>(&mut self, name: &str, options: SaveOptions<T>) -> bool {
        self.resources.register("resource", name, options)
    }
}

impl World {
    /// Saves component `T` under `name`: version 1, no migration, no
    /// references. Idempotent per pair; returns whether it was new.
    ///
    /// # Panics
    ///
    /// As [`SaveRegistry::register`].
    #[track_caller]
    pub fn register_saved<T: Serialize + DeserializeOwned + 'static>(
        &mut self,
        name: &str,
    ) -> bool {
        self.register_saved_with(name, SaveOptions::<T>::new())
    }

    /// Saves component `T` under `name` with `options`: a version, its
    /// migration steps, a saved form and a reference codec.
    ///
    /// # Panics
    ///
    /// As [`SaveRegistry::register`].
    #[track_caller]
    pub fn register_saved_with<T: 'static>(&mut self, name: &str, options: SaveOptions<T>) -> bool {
        registry_mut(self).register(name, options)
    }

    /// Saves resource `T` under `name`, whole: version 1, no migration, no
    /// references.
    ///
    /// # Panics
    ///
    /// As [`SaveRegistry::register`].
    #[track_caller]
    pub fn register_saved_resource<T: Serialize + DeserializeOwned + 'static>(
        &mut self,
        name: &str,
    ) -> bool {
        self.register_saved_resource_with(name, SaveOptions::<T>::new())
    }

    /// Saves resource `T` under `name` with `options`.
    ///
    /// # Panics
    ///
    /// As [`SaveRegistry::register`].
    #[track_caller]
    pub fn register_saved_resource_with<T: 'static>(
        &mut self,
        name: &str,
        options: SaveOptions<T>,
    ) -> bool {
        registry_mut(self).register_resource(name, options)
    }
}

fn registry_mut(world: &mut World) -> &mut SaveRegistry {
    if !world.has_resource::<SaveRegistry>() {
        world.insert_resource(SaveRegistry::new());
    }
    world.resource_mut::<SaveRegistry>()
}

/// `[a-z][a-z0-9_]*`, the component registry's rule.
fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[cfg(test)]
#[path = "../tests/save/registry.rs"]
mod tests;
