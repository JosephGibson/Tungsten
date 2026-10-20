//! Data entries (W13f, M61): shared definitions by kind and ID, listed in
//! the manifest's `data` section and named from prefabs and from other
//! entries through [`DataRef`], by ID or written in place.
//!
//! A plugin registers each kind it reads in its `build`, with
//! [`World::register_data_kind`] or
//! [`World::register_data_kind_with_assets`]: the kind's name and the type
//! its files deserialize to, one type per kind and one kind per type. A
//! data file holds one entry, read as strictly as a prefab file: a key
//! repeated at any depth fails, and the type's own serde rules decide the
//! rest (`#[serde(deny_unknown_fields)]` refuses an unknown key).
//!
//! ```json
//! "data": {
//!   "effects": { "burning": { "path": "data/effects/burning.json" } }
//! }
//! ```
//!
//! Prefabs and data entries load together in two phases ([`StagedLoad`]):
//! every file is read and parsed, everything is put in place, then every
//! entry the registries hold from a file is checked against the World. An
//! entry may so name any other, whatever the order, and a commit that fails
//! puts back what it replaced.

#![deny(missing_docs)]

use std::any::{Any, TypeId, type_name};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::de::value::{MapAccessDeserializer, SeqAccessDeserializer};
use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use thiserror::Error;

use crate::assets::prefab::{PrefabData, PrefabError, PrefabRegistry};
use crate::assets::scene::StrictValue;
use crate::component_registry::MissingAsset;
use crate::ecs::World;

/// Deserializes a data file's value as its kind's type; on failure, the
/// field where it stopped and serde's error.
type Parse = Box<dyn Fn(Value) -> Result<Box<dyn Any>, (String, serde_json::Error)>>;

/// A kind's asset check, over a definition of its type.
type Check = Box<dyn Fn(&dyn Any, &World) -> Result<(), MissingAsset>>;

/// One registered kind and its entries.
struct Kind {
    type_id: TypeId,
    type_name: &'static str,
    parse: Parse,
    check: Option<Check>,
    entries: BTreeMap<String, Entry>,
}

/// One entry: its definition, and the file it was loaded from, if any.
struct Entry {
    value: Box<dyn Any>,
    path: Option<PathBuf>,
}

/// The data kinds plugins registered and their entries by ID, a `World`
/// resource: what [`DataRef::resolve`] reads. The umbrella's loader fills
/// it from the manifest's `data` section; `Debug` prints the kinds and
/// their IDs.
///
/// ```
/// use serde::Deserialize;
/// use tungsten_core::{DataRegistry, World};
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Effect {
///     duration: f32,
/// }
///
/// let mut world = World::new();
/// assert!(world.register_data_kind::<Effect>("effects"));
/// let data = world.resource_mut::<DataRegistry>();
/// data.insert("burning", Effect { duration: 2.0 });
/// assert_eq!(data.get::<Effect>("burning"), Some(&Effect { duration: 2.0 }));
/// assert_eq!(data.kind_of::<Effect>(), Some("effects"));
/// assert_eq!(data.ids("effects").collect::<Vec<_>>(), ["burning"]);
/// ```
#[derive(Default)]
pub struct DataRegistry {
    /// Kind name → kind, sorted by name.
    kinds: BTreeMap<&'static str, Kind>,
    /// The registered type → its kind.
    kinds_by_type: HashMap<TypeId, &'static str>,
    /// Each entry loaded from a file: the file → the kind and ID.
    routes: HashMap<PathBuf, (&'static str, String)>,
}

impl DataRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers kind `kind`, whose files deserialize to `T`. Returns whether
    /// the pair is new; the same kind and type again is a no-op that keeps
    /// the first registration.
    ///
    /// # Panics
    ///
    /// When `kind` is not `[a-z][a-z0-9_]*`, already holds another type, or
    /// `T` already has another kind; the message names both (`D-022`).
    #[track_caller]
    pub fn register<T: DeserializeOwned + 'static>(&mut self, kind: &'static str) -> bool {
        self.insert_kind::<T>(kind, None)
    }

    /// [`register`](Self::register), plus `assets`: the check of the IDs a
    /// definition names, against a World. A load's commit runs it on every
    /// entry of the kind loaded from a file, and [`DataRef::check`] on a
    /// definition written in place; an ID the World lacks is a
    /// [`MissingAsset`].
    ///
    /// # Panics
    ///
    /// As [`register`](Self::register).
    #[track_caller]
    pub fn register_with_assets<T: DeserializeOwned + 'static>(
        &mut self,
        kind: &'static str,
        assets: impl Fn(&T, &World) -> Result<(), MissingAsset> + 'static,
    ) -> bool {
        let check: Check = Box::new(move |value, world| match value.downcast_ref::<T>() {
            Some(value) => assets(value, world),
            None => Ok(()),
        });
        self.insert_kind::<T>(kind, Some(check))
    }

    /// The registration behind both doors: the name and type checks, then
    /// the kind's parse for `T`.
    #[track_caller]
    fn insert_kind<T: DeserializeOwned + 'static>(
        &mut self,
        kind: &'static str,
        check: Option<Check>,
    ) -> bool {
        let type_id = TypeId::of::<T>();
        let type_name = type_name::<T>();
        assert!(
            valid_name(kind),
            "data kind `{kind}` for `{type_name}` must match [a-z][a-z0-9_]*"
        );
        if let Some(existing) = self.kinds.get(kind) {
            assert!(
                existing.type_id == type_id,
                "data kind `{kind}` already holds `{}`; `{type_name}` cannot take it",
                existing.type_name
            );
            return false;
        }
        if let Some(existing) = self.kinds_by_type.get(&type_id) {
            panic!(
                "`{type_name}` is already registered as data kind `{existing}`; it cannot also be `{kind}`"
            );
        }
        let parse: Parse = Box::new(|value| {
            serde_path_to_error::deserialize::<_, T>(value)
                .map(|definition| Box::new(definition) as Box<dyn Any>)
                .map_err(|err| {
                    let field = if err.path().iter().next().is_none() {
                        String::new()
                    } else {
                        err.path().to_string()
                    };
                    (field, err.into_inner())
                })
        });
        self.kinds_by_type.insert(type_id, kind);
        self.kinds.insert(
            kind,
            Kind {
                type_id,
                type_name,
                parse,
                check,
                entries: BTreeMap::new(),
            },
        );
        true
    }

    /// Whether kind `kind` is registered.
    #[must_use]
    pub fn contains_kind(&self, kind: &str) -> bool {
        self.kinds.contains_key(kind)
    }

    /// The registered kinds, sorted.
    pub fn kinds(&self) -> impl Iterator<Item = &'static str> {
        self.kinds.keys().copied()
    }

    /// The kind `T` is registered under.
    #[must_use]
    pub fn kind_of<T: 'static>(&self) -> Option<&'static str> {
        self.kinds_by_type.get(&TypeId::of::<T>()).copied()
    }

    /// The entry `id` of `T`'s kind, as it is now: a later load or reload
    /// replaces it.
    #[must_use]
    pub fn get<T: 'static>(&self, id: &str) -> Option<&T> {
        self.kind_for::<T>()?
            .1
            .entries
            .get(id)?
            .value
            .downcast_ref()
    }

    /// The IDs of kind `kind`, sorted; none for a kind not registered.
    pub fn ids(&self, kind: &str) -> impl Iterator<Item = &str> {
        self.kinds
            .get(kind)
            .into_iter()
            .flat_map(|kind| kind.entries.keys().map(String::as_str))
    }

    /// The kind and ID of the entry loaded from `path`, which hot reload
    /// reads. Each file backs one entry: an entry that a later commit gives
    /// the file takes it.
    #[must_use]
    pub fn entry_for_path(&self, path: &Path) -> Option<(&'static str, &str)> {
        self.routes.get(path).map(|(kind, id)| (*kind, id.as_str()))
    }

    /// Adds `value` as entry `id` of `T`'s kind, replacing an entry of the
    /// same ID and dropping the file it was loaded from. The value is taken
    /// as given: no load's commit checks it, and hot reload routes no file
    /// to it.
    ///
    /// # Panics
    ///
    /// When `T` is not a registered kind's type.
    #[track_caller]
    pub fn insert<T: 'static>(&mut self, id: impl Into<String>, value: T) {
        let id = id.into();
        let Some(kind) = self.kind_of::<T>() else {
            panic!(
                "`{}` is not a registered data kind's type; register it before inserting `{id}`",
                type_name::<T>()
            );
        };
        self.take_entry(kind, &id);
        self.put_entry(kind, id, Box::new(value), None);
    }

    /// How many entries are held, over every kind.
    #[must_use]
    pub fn len(&self) -> usize {
        self.kinds.values().map(|kind| kind.entries.len()).sum()
    }

    /// Whether no entry is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.kinds.values().all(|kind| kind.entries.is_empty())
    }

    fn kind_for<T: 'static>(&self) -> Option<(&'static str, &Kind)> {
        let name = self.kind_of::<T>()?;
        self.kinds.get(name).map(|kind| (name, kind))
    }

    /// Removes entry `id` of `kind` with its file's route.
    fn take_entry(&mut self, kind: &'static str, id: &str) -> Option<Entry> {
        let entry = self.kinds.get_mut(kind)?.entries.remove(id)?;
        if let Some(path) = &entry.path {
            self.routes.remove(path);
        }
        Some(entry)
    }

    /// Puts `value` as entry `id` of `kind`, routing `path` to it; the
    /// caller has freed the path.
    fn put_entry(
        &mut self,
        kind: &'static str,
        id: String,
        value: Box<dyn Any>,
        path: Option<PathBuf>,
    ) {
        let Some(slot) = self.kinds.get_mut(kind) else {
            return;
        };
        if let Some(path) = &path {
            self.routes.insert(path.clone(), (kind, id.clone()));
        }
        slot.entries.insert(id, Entry { value, path });
    }

    /// The entry `path` routes to loses the file, keeping its definition.
    fn release_path(&mut self, path: &Path) -> Option<(&'static str, String)> {
        let (kind, id) = self.routes.remove(path)?;
        if let Some(entry) = self
            .kinds
            .get_mut(kind)
            .and_then(|slot| slot.entries.get_mut(&id))
        {
            entry.path = None;
        }
        Some((kind, id))
    }

    /// Gives entry `id` of `kind` the file `path` back.
    fn give_path(&mut self, kind: &'static str, id: &str, path: PathBuf) {
        if let Some(entry) = self
            .kinds
            .get_mut(kind)
            .and_then(|slot| slot.entries.get_mut(id))
        {
            self.routes.insert(path.clone(), (kind, id.to_owned()));
            entry.path = Some(path);
        }
    }
}

impl fmt::Debug for DataRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kinds: BTreeMap<&str, Vec<&str>> = self
            .kinds
            .iter()
            .map(|(name, kind)| (*name, kind.entries.keys().map(String::as_str).collect()))
            .collect();
        f.debug_struct("DataRegistry")
            .field("kinds", &kinds)
            .finish_non_exhaustive()
    }
}

/// A field's reference to a data entry of `T`'s kind: an ID, or the
/// definition written in place. A component, a data definition or a save
/// holds one.
///
/// In a file, a JSON string is an ID and an object or an array is the
/// definition, read as `T` with its own field paths in errors; any other
/// value fails. A `T` written as a string has no inline form. Serializing
/// writes the same forms, and an inline definition that does not serialize
/// as an object or an array fails, since it would read back as an ID.
///
/// ```
/// use serde::Deserialize;
/// use tungsten_core::{DataRef, DataRegistry};
///
/// #[derive(Debug, Deserialize, PartialEq)]
/// struct Effect {
///     duration: f32,
/// }
///
/// #[derive(Deserialize)]
/// struct Projectile {
///     on_hit: DataRef<Effect>,
/// }
///
/// let mut data = DataRegistry::new();
/// data.register::<Effect>("effects");
/// data.insert("burning", Effect { duration: 2.0 });
///
/// let by_id: Projectile = serde_json::from_str(r#"{ "on_hit": "burning" }"#).unwrap();
/// assert_eq!(by_id.on_hit.id(), Some("burning"));
/// assert_eq!(by_id.on_hit.resolve(&data), Some(&Effect { duration: 2.0 }));
/// let inline: Projectile = serde_json::from_str(r#"{ "on_hit": { "duration": 0.5 } }"#).unwrap();
/// assert_eq!(inline.on_hit.resolve(&data), Some(&Effect { duration: 0.5 }));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataRef<T> {
    inner: Ref<T>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Ref<T> {
    Id(String),
    Inline(Box<T>),
}

impl<T> DataRef<T> {
    /// A reference to the entry `id`.
    #[must_use]
    pub fn by_id(id: impl Into<String>) -> Self {
        Self {
            inner: Ref::Id(id.into()),
        }
    }

    /// A definition written in place.
    #[must_use]
    pub fn inline(definition: T) -> Self {
        Self {
            inner: Ref::Inline(Box::new(definition)),
        }
    }

    /// The ID, when it names an entry.
    #[must_use]
    pub fn id(&self) -> Option<&str> {
        match &self.inner {
            Ref::Id(id) => Some(id),
            Ref::Inline(_) => None,
        }
    }

    /// The definition, when it is written in place.
    #[must_use]
    pub fn as_inline(&self) -> Option<&T> {
        match &self.inner {
            Ref::Id(_) => None,
            Ref::Inline(definition) => Some(&**definition),
        }
    }
}

impl<T: 'static> DataRef<T> {
    /// The definition: the one written in place, or the entry `data` holds
    /// under the ID now, so a reload shows on the next read. `None` for an
    /// ID `data` lacks.
    #[must_use]
    pub fn resolve<'a>(&'a self, data: &'a DataRegistry) -> Option<&'a T> {
        match &self.inner {
            Ref::Id(id) => data.get(id),
            Ref::Inline(definition) => Some(&**definition),
        }
    }

    /// The asset check of a field holding this reference, against `world`'s
    /// [`DataRegistry`]: an ID the registry lacks is a [`MissingAsset`] of
    /// `T`'s kind; a definition in place runs its kind's check, each field
    /// it reports prefixed with `field.`; a `T` no kind holds fails as an
    /// unknown `data kind` named by its type.
    pub fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        let Some((kind, slot)) = world
            .get_resource::<DataRegistry>()
            .and_then(DataRegistry::kind_for::<T>)
        else {
            return Err(MissingAsset::new(field, "data kind", type_name::<T>()));
        };
        match &self.inner {
            Ref::Id(id) if slot.entries.contains_key(id) => Ok(()),
            Ref::Id(id) => Err(MissingAsset::new(field, kind, id.clone())),
            Ref::Inline(definition) => match &slot.check {
                Some(check) => check(&**definition, world).map_err(|mut missing| {
                    missing.field = join_field(field, &missing.field);
                    missing
                }),
                None => Ok(()),
            },
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for DataRef<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RefVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de>> Visitor<'de> for RefVisitor<T> {
            type Value = DataRef<T>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a data entry's ID or its definition")
            }

            fn visit_str<E: de::Error>(self, id: &str) -> Result<Self::Value, E> {
                Ok(DataRef::by_id(id))
            }

            fn visit_string<E: de::Error>(self, id: String) -> Result<Self::Value, E> {
                Ok(DataRef::by_id(id))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(MapAccessDeserializer::new(map)).map(DataRef::inline)
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
                T::deserialize(SeqAccessDeserializer::new(seq)).map(DataRef::inline)
            }
        }

        deserializer.deserialize_any(RefVisitor(PhantomData))
    }
}

impl<T: Serialize> Serialize for DataRef<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error as _;
        match &self.inner {
            Ref::Id(id) => serializer.serialize_str(id),
            Ref::Inline(definition) => {
                let value = serde_json::to_value(definition).map_err(S::Error::custom)?;
                if value.is_object() || value.is_array() {
                    value.serialize(serializer)
                } else {
                    Err(S::Error::custom(
                        "an inline data definition must serialize as a JSON object or array, \
                         or it reads back as an ID",
                    ))
                }
            }
        }
    }
}

/// Prefabs and data entries staged for one two-phase load. Phase one,
/// [`add_prefab`](Self::add_prefab) and [`add_data`](Self::add_data), reads
/// and parses each file. Phase two, [`commit`](Self::commit), puts every
/// staged entry in place, then checks every entry the registries hold from a
/// file against the World, keeping all or none. Live entities are never
/// touched. `Debug` prints the staged IDs.
///
/// ```
/// use serde::Deserialize;
/// use tungsten_core::{DataRegistry, PrefabRegistry, StagedLoad, World};
///
/// #[derive(Deserialize)]
/// struct Effect {
///     trail: String,
/// }
///
/// let dir = std::env::temp_dir().join(format!("tungsten_staged_doc_{}", std::process::id()));
/// std::fs::create_dir_all(&dir).unwrap();
/// // The effect names the prefab `spark`, which loads in the same commit.
/// std::fs::write(dir.join("burning.json"), r#"{ "trail": "spark" }"#).unwrap();
/// std::fs::write(dir.join("spark.json"), r#"{ "tag": "spark" }"#).unwrap();
///
/// let mut world = World::new();
/// world.register_data_kind_with_assets::<Effect>("effects", |effect, world| {
///     world.check_prefab_id("trail", &effect.trail)
/// });
/// let mut load = StagedLoad::new();
/// load.add_data(&world, "effects", "burning", dir.join("burning.json")).unwrap();
/// load.add_prefab("spark", dir.join("spark.json")).unwrap();
/// load.commit(&mut world).unwrap();
/// assert!(world.resource::<PrefabRegistry>().get("spark").is_some());
///
/// // An edit naming a prefab that no file gives fails, and the commit keeps
/// // the previous entry.
/// std::fs::write(dir.join("burning.json"), r#"{ "trail": "sparks" }"#).unwrap();
/// let mut load = StagedLoad::new();
/// load.add_data(&world, "effects", "burning", dir.join("burning.json")).unwrap();
/// let err = load.commit(&mut world).unwrap_err();
/// assert!(err.to_string().ends_with("unknown prefab `sparks`; did you mean `spark`?"));
/// let data = world.resource::<DataRegistry>();
/// assert_eq!(data.get::<Effect>("burning").unwrap().trail, "spark");
/// std::fs::remove_dir_all(&dir).unwrap();
/// ```
#[derive(Default)]
pub struct StagedLoad {
    prefabs: BTreeMap<String, (PrefabData, PathBuf)>,
    data: BTreeMap<(&'static str, String), StagedData>,
}

/// A staged data entry: its definition, its file, and the type it was read
/// as.
struct StagedData {
    value: Box<dyn Any>,
    path: PathBuf,
    type_name: &'static str,
}

impl StagedLoad {
    /// Nothing staged.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads and parses the prefab file `path` as prefab `id`, as
    /// [`PrefabData::load`] does, leaving the World checks to the commit. The
    /// file is kept by its canonical path when it resolves, as the manifest
    /// keeps it, so hot reload's canonical paths route to it. A second add of
    /// `id` replaces the first.
    ///
    /// # Errors
    ///
    /// [`PrefabError::Io`] when the file does not read;
    /// [`PrefabError::Parse`] when it is not a prefab's JSON.
    pub fn add_prefab(
        &mut self,
        id: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Result<(), PrefabError> {
        let path = path.into();
        let data = PrefabData::parse(&path)?;
        let path = path.canonicalize().unwrap_or(path);
        self.prefabs.insert(id.into(), (data, path));
        Ok(())
    }

    /// Reads the data file `path` as entry `id` of kind `kind`, refusing a
    /// key repeated at any depth, and deserializes it as the type registered
    /// for the kind in `world`'s [`DataRegistry`]. The IDs it names wait for
    /// the commit, which goes to a World registering the kind for the same
    /// type. The file is kept by its canonical path when it resolves, as
    /// [`add_prefab`](Self::add_prefab) keeps one. A second add of the same
    /// kind and ID replaces the first.
    ///
    /// # Errors
    ///
    /// [`DataError::UnknownKind`] when no plugin registered `kind`;
    /// [`DataError::Io`] when the file does not read; [`DataError::Parse`]
    /// when it is not JSON or repeats a key; [`DataError::Invalid`] when it
    /// does not deserialize as the kind's type.
    pub fn add_data(
        &mut self,
        world: &World,
        kind: &str,
        id: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Result<(), DataError> {
        let path = path.into();
        let file = || path.display().to_string();
        let empty = DataRegistry::new();
        let registry = world.get_resource::<DataRegistry>().unwrap_or(&empty);
        let Some((&name, slot)) = registry.kinds.get_key_value(kind) else {
            return Err(DataError::UnknownKind {
                kind: kind.to_owned(),
                path: file(),
                registered: registry.kinds().map(str::to_owned).collect(),
                suggestion: nearest(kind, registry.kinds()).map(str::to_owned),
            });
        };
        let contents = std::fs::read_to_string(&path).map_err(|source| DataError::Io {
            path: file(),
            source,
        })?;
        let StrictValue(value) =
            serde_json::from_str(&contents).map_err(|source| DataError::Parse {
                path: file(),
                source,
            })?;
        let value = (slot.parse)(value).map_err(|(field, source)| DataError::Invalid {
            path: file(),
            field,
            source,
        })?;
        let path = path.canonicalize().unwrap_or(path);
        self.data.insert(
            (name, id.into()),
            StagedData {
                value,
                path,
                type_name: slot.type_name,
            },
        );
        Ok(())
    }

    /// How many prefabs and data entries are staged.
    #[must_use]
    pub fn len(&self) -> usize {
        self.prefabs.len() + self.data.len()
    }

    /// Whether nothing is staged.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.prefabs.is_empty() && self.data.is_empty()
    }

    /// Puts every staged entry in place of the World's entry of that ID,
    /// inserting the [`DataRegistry`] and [`PrefabRegistry`] the World lacks;
    /// each staged entry takes its file from whichever entry held it, which
    /// keeps its definition without a file. Then it checks every entry the
    /// two registries hold from a file, staged or not, with the staging in
    /// place: data entries by kind, then ID, through their kind's check,
    /// then prefabs by ID through the checks [`PrefabData::load`] runs.
    /// Entries without a file, inserted in code or left by a lost file, are
    /// taken as given.
    ///
    /// # Errors
    ///
    /// Before anything changes, [`StagedLoadError::Data`] with
    /// [`DataError::UnknownKind`] or [`DataError::MismatchedKind`] for a
    /// staged entry whose kind the World does not register, or registers for
    /// another type than the one it was read as. Then the first entry that
    /// fails its check, as [`StagedLoadError::Data`] with
    /// [`DataError::MissingAsset`] or [`StagedLoadError::Prefab`] with
    /// [`PrefabError::Validation`]; every staged entry's previous state comes
    /// back first: a replaced entry with its file, an added one removed, and
    /// a file's former holder with its file.
    pub fn commit(self, world: &mut World) -> Result<(), StagedLoadError> {
        if let Some((_, error)) = self.kind_failures(world).next() {
            return Err(error);
        }
        let undo = self.place(world);
        let checked = check_file_backed(world, &mut Err);
        if checked.is_err() {
            for step in undo.into_iter().rev() {
                step.revert(world);
            }
        }
        checked
    }

    /// Puts every staged entry in place as [`Self::commit`] does, then checks
    /// every entry the registries hold from a file and returns every failure,
    /// in the order `commit` checks them, rather than the first. A staged
    /// entry whose kind `world` registers for another type is a failure and
    /// stays out. Nothing is taken back, so the World is one a check throws
    /// away, as `tungsten check`'s is (M70).
    ///
    /// ```
    /// use serde::Deserialize;
    /// use tungsten_core::{StagedLoad, World};
    ///
    /// #[derive(Deserialize)]
    /// struct Effect {
    ///     trail: String,
    /// }
    ///
    /// let dir = std::env::temp_dir().join(format!("tungsten_staged_check_doc_{}", std::process::id()));
    /// std::fs::create_dir_all(&dir).unwrap();
    /// std::fs::write(dir.join("burning.json"), r#"{ "trail": "sparks" }"#).unwrap();
    /// std::fs::write(dir.join("frozen.json"), r#"{ "trail": "ice" }"#).unwrap();
    ///
    /// let mut world = World::new();
    /// world.register_data_kind_with_assets::<Effect>("effects", |effect, world| {
    ///     world.check_prefab_id("trail", &effect.trail)
    /// });
    /// let mut load = StagedLoad::new();
    /// load.add_data(&world, "effects", "burning", dir.join("burning.json")).unwrap();
    /// load.add_data(&world, "effects", "frozen", dir.join("frozen.json")).unwrap();
    /// // Both entries name a prefab no file gives: two failures, not one.
    /// assert_eq!(load.commit_for_check(&mut world).len(), 2);
    /// std::fs::remove_dir_all(&dir).unwrap();
    /// ```
    #[must_use]
    pub fn commit_for_check(mut self, world: &mut World) -> Vec<StagedLoadError> {
        let mut failures = Vec::new();
        let refused: Vec<_> = self.kind_failures(world).collect();
        for (key, error) in refused {
            self.data.remove(&key);
            failures.push(error);
        }
        self.place(world);
        let _ = check_file_backed(world, &mut |error| {
            failures.push(error);
            Ok(())
        });
        failures
    }

    /// Puts every staged entry in place of the World's entry of that ID,
    /// inserting the registries the World lacks; returns how to take each
    /// change back.
    fn place(self, world: &mut World) -> Vec<Undo> {
        if !world.has_resource::<DataRegistry>() {
            world.insert_resource(DataRegistry::new());
        }
        if !world.has_resource::<PrefabRegistry>() {
            world.insert_resource(PrefabRegistry::new());
        }
        let mut undo = Vec::new();
        for ((kind, id), StagedData { value, path, .. }) in self.data {
            take_file(world, &path, &mut undo);
            let data = world.resource_mut::<DataRegistry>();
            let previous = data.take_entry(kind, &id);
            data.put_entry(kind, id.clone(), value, Some(path));
            undo.push(Undo::Data { kind, id, previous });
        }
        for (id, (prefab, path)) in self.prefabs {
            take_file(world, &path, &mut undo);
            let prefabs = world.resource_mut::<PrefabRegistry>();
            let previous = prefabs.take_entry(&id);
            prefabs.put_entry(id.clone(), prefab, Some(path));
            undo.push(Undo::Prefab { id, previous });
        }
        undo
    }

    /// Each staged data entry whose kind `world` does not register for the
    /// type the entry was read as, with its error, by kind, then ID.
    fn kind_failures<'a>(
        &'a self,
        world: &'a World,
    ) -> impl Iterator<Item = ((&'static str, String), StagedLoadError)> + 'a {
        let registry = world.get_resource::<DataRegistry>();
        self.data.iter().filter_map(move |((kind, id), staged)| {
            let slot = registry.and_then(|registry| registry.kinds.get(kind));
            let source = match slot {
                Some(slot) if slot.type_id == (*staged.value).type_id() => return None,
                Some(slot) => DataError::MismatchedKind {
                    kind: (*kind).to_owned(),
                    path: staged.path.display().to_string(),
                    staged: staged.type_name,
                    registered: slot.type_name,
                },
                None => {
                    let kinds = || registry.into_iter().flat_map(DataRegistry::kinds);
                    DataError::UnknownKind {
                        kind: (*kind).to_owned(),
                        path: staged.path.display().to_string(),
                        registered: kinds().map(str::to_owned).collect(),
                        suggestion: nearest(kind, kinds()).map(str::to_owned),
                    }
                }
            };
            let error = StagedLoadError::Data {
                kind: (*kind).to_owned(),
                id: id.clone(),
                source: Box::new(source),
            };
            Some(((*kind, id.clone()), error))
        })
    }
}

impl fmt::Debug for StagedLoad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data: Vec<String> = self
            .data
            .keys()
            .map(|(kind, id)| format!("{kind}.{id}"))
            .collect();
        f.debug_struct("StagedLoad")
            .field("prefabs", &self.prefabs.keys().collect::<Vec<_>>())
            .field("data", &data)
            .finish()
    }
}

/// One change a commit made, and how to take it back.
enum Undo {
    /// Entry `id` of `kind` was replaced by a staged one, or added.
    Data {
        kind: &'static str,
        id: String,
        previous: Option<Entry>,
    },
    /// Prefab `id` was replaced by a staged one, or added.
    Prefab {
        id: String,
        previous: Option<(PrefabData, Option<PathBuf>)>,
    },
    /// Entry `id` of `kind` lost `path` to a staged entry.
    DataFile {
        kind: &'static str,
        id: String,
        path: PathBuf,
    },
    /// Prefab `id` lost `path` to a staged entry.
    PrefabFile { id: String, path: PathBuf },
}

impl Undo {
    fn revert(self, world: &mut World) {
        match self {
            Undo::Data { kind, id, previous } => {
                let data = world.resource_mut::<DataRegistry>();
                data.take_entry(kind, &id);
                if let Some(Entry { value, path }) = previous {
                    data.put_entry(kind, id, value, path);
                }
            }
            Undo::Prefab { id, previous } => {
                let prefabs = world.resource_mut::<PrefabRegistry>();
                prefabs.take_entry(&id);
                if let Some((prefab, path)) = previous {
                    prefabs.put_entry(id, prefab, path);
                }
            }
            Undo::DataFile { kind, id, path } => {
                world
                    .resource_mut::<DataRegistry>()
                    .give_path(kind, &id, path);
            }
            Undo::PrefabFile { id, path } => {
                world.resource_mut::<PrefabRegistry>().give_path(&id, path);
            }
        }
    }
}

/// Frees `path` in both registries for a staged entry, recording each loss.
fn take_file(world: &mut World, path: &Path, undo: &mut Vec<Undo>) {
    if let Some(id) = world.resource_mut::<PrefabRegistry>().release_path(path) {
        undo.push(Undo::PrefabFile {
            id,
            path: path.to_path_buf(),
        });
    }
    if let Some((kind, id)) = world.resource_mut::<DataRegistry>().release_path(path) {
        undo.push(Undo::DataFile {
            kind,
            id,
            path: path.to_path_buf(),
        });
    }
}

/// Phase two's check: every file-backed data entry by kind, then ID, then
/// every file-backed prefab by ID, each failure sent to `report`, which
/// `commit` stops at (`Err`) and `commit_for_check` keeps (`Ok`).
fn check_file_backed(
    world: &World,
    report: &mut dyn FnMut(StagedLoadError) -> Result<(), StagedLoadError>,
) -> Result<(), StagedLoadError> {
    let data = world.resource::<DataRegistry>();
    for (&kind, slot) in &data.kinds {
        let Some(check) = &slot.check else {
            continue;
        };
        for (id, entry) in &slot.entries {
            let Some(path) = &entry.path else {
                continue;
            };
            if let Err(missing) = check(&*entry.value, world) {
                report(StagedLoadError::Data {
                    kind: kind.to_owned(),
                    id: id.clone(),
                    source: Box::new(DataError::MissingAsset {
                        path: path.display().to_string(),
                        suggestion: suggestion(world, missing.kind, &missing.id),
                        field: missing.field,
                        kind: missing.kind,
                        id: missing.id,
                    }),
                })?;
            }
        }
    }
    for (id, prefab, path) in world.resource::<PrefabRegistry>().file_backed() {
        if let Err(message) = prefab.validate(world) {
            report(StagedLoadError::Prefab {
                id: id.to_owned(),
                source: Box::new(PrefabError::Validation {
                    path: path.display().to_string(),
                    message,
                }),
            })?;
        }
    }
    Ok(())
}

/// A data file that does not load or check.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DataError {
    /// No plugin registered the entry's kind.
    #[error(
        "data file '{path}': unknown data kind `{kind}` (registered: {}){}",
        list(.registered),
        did_you_mean(.suggestion.as_deref())
    )]
    UnknownKind {
        /// The kind the manifest gave.
        kind: String,
        /// The data file.
        path: String,
        /// The registered kinds, sorted.
        registered: Vec<String>,
        /// The registered kind nearest `kind`, when one is near.
        suggestion: Option<String>,
    },
    /// The World a commit goes to registers the entry's kind for another
    /// type than the one the file was read as: staged against one World,
    /// committed to another.
    #[error(
        "data file '{path}': data kind `{kind}` holds `{registered}` in this World, not `{staged}`, the type the file was read as"
    )]
    MismatchedKind {
        /// The entry's kind.
        kind: String,
        /// The data file.
        path: String,
        /// The type the file was read as.
        staged: &'static str,
        /// The type the World registers for the kind.
        registered: &'static str,
    },
    /// The file does not read.
    #[error("failed to read data file '{path}': {source}")]
    Io {
        /// The data file.
        path: String,
        /// The read's error.
        source: std::io::Error,
    },
    /// The file is not JSON, or repeats a key at some depth.
    #[error("invalid data file '{path}': {source}")]
    Parse {
        /// The data file.
        path: String,
        /// Serde's error, with its line and column.
        source: serde_json::Error,
    },
    /// The file does not deserialize as its kind's type.
    #[error("invalid data file '{path}'{}: {source}", field_note(.field))]
    Invalid {
        /// The data file.
        path: String,
        /// The path inside the definition where deserialization stopped, as
        /// `serde_path_to_error` gives it, empty at the top.
        field: String,
        /// Serde's error.
        source: serde_json::Error,
    },
    /// The definition names an asset or an entry the World does not hold,
    /// by its kind's check.
    #[error(
        "invalid data file '{path}'{}: unknown {kind} `{id}`{}",
        field_note(.field),
        did_you_mean(.suggestion.as_deref())
    )]
    MissingAsset {
        /// The data file.
        path: String,
        /// The definition's field that names it.
        field: String,
        /// What it names: `"prefab"`, a data kind, `"tilemap"`.
        kind: &'static str,
        /// The ID it gave.
        id: String,
        /// The prefab or data ID nearest `id`, when one is near.
        suggestion: Option<String>,
    },
}

/// A staged load whose commit failed: the entry that failed first, with
/// every staged entry back as it was.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StagedLoadError {
    /// A prefab failed its checks against the World.
    #[error("prefab `{id}`: {source}")]
    Prefab {
        /// The prefab's ID.
        id: String,
        /// Its error, naming the file.
        source: Box<PrefabError>,
    },
    /// A data entry failed its kind's check against the World.
    #[error("data `{kind}` entry `{id}`: {source}")]
    Data {
        /// The entry's kind.
        kind: String,
        /// The entry's ID.
        id: String,
        /// Its error, naming the file.
        source: Box<DataError>,
    },
}

impl World {
    /// [`DataRegistry::register`] on this World's registry, which it inserts
    /// first when the World has none, so a plugin built on a bare World
    /// registers.
    ///
    /// # Panics
    ///
    /// As [`DataRegistry::register`].
    #[track_caller]
    pub fn register_data_kind<T: DeserializeOwned + 'static>(
        &mut self,
        kind: &'static str,
    ) -> bool {
        data_mut(self).register::<T>(kind)
    }

    /// [`DataRegistry::register_with_assets`] on this World's registry,
    /// which it inserts first when the World has none.
    ///
    /// # Panics
    ///
    /// As [`DataRegistry::register`].
    #[track_caller]
    pub fn register_data_kind_with_assets<T: DeserializeOwned + 'static>(
        &mut self,
        kind: &'static str,
        assets: impl Fn(&T, &World) -> Result<(), MissingAsset> + 'static,
    ) -> bool {
        data_mut(self).register_with_assets(kind, assets)
    }

    /// The asset check of a field that names a prefab: a [`MissingAsset`] of
    /// kind `"prefab"` when this World's [`PrefabRegistry`] lacks `id`, or the
    /// World has none. A component's or a data kind's check calls it; during
    /// a load's commit the staged prefabs are in place.
    pub fn check_prefab_id(&self, field: &str, id: &str) -> Result<(), MissingAsset> {
        let found = self
            .get_resource::<PrefabRegistry>()
            .is_some_and(|prefabs| prefabs.get(id).is_some());
        if found {
            Ok(())
        } else {
            Err(MissingAsset::new(field, "prefab", id))
        }
    }
}

fn data_mut(world: &mut World) -> &mut DataRegistry {
    if !world.has_resource::<DataRegistry>() {
        world.insert_resource(DataRegistry::new());
    }
    world.resource_mut::<DataRegistry>()
}

/// The ID nearest a missing `id` of `kind`: a prefab's for `"prefab"`, else
/// a registered data kind's; `None` for any other kind or when none is near.
pub(crate) fn suggestion(world: &World, kind: &str, id: &str) -> Option<String> {
    if kind == "prefab" {
        let prefabs = world.get_resource::<PrefabRegistry>()?;
        return nearest(id, prefabs.ids()).map(str::to_owned);
    }
    let data = world.get_resource::<DataRegistry>()?;
    nearest(id, data.ids(kind)).map(str::to_owned)
}

/// "; did you mean `<name>`?", or nothing.
pub(crate) fn did_you_mean(suggestion: Option<&str>) -> String {
    suggestion.map_or_else(String::new, |name| format!("; did you mean `{name}`?"))
}

/// The candidate nearest `target` by Levenshtein distance over chars, at
/// most max(1, length / 3) edits away, ties to the first; `candidates` come
/// sorted, so the first is the smallest.
pub(crate) fn nearest<'a>(
    target: &str,
    candidates: impl Iterator<Item = &'a str>,
) -> Option<&'a str> {
    let target: Vec<char> = target.chars().collect();
    let bound = (target.len() / 3).max(1);
    let mut best: Option<(usize, &str)> = None;
    for candidate in candidates {
        let distance = levenshtein(&target, candidate);
        if distance == 0 || distance > bound {
            continue;
        }
        if best.is_none_or(|(least, _)| distance < least) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, candidate)| candidate)
}

fn levenshtein(a: &[char], b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != cb);
            current[j + 1] = substitution.min(previous[j + 1] + 1).min(current[j] + 1);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}

/// `[a-z][a-z0-9_]*`.
fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_lowercase())
        && bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// `outer.inner`, or whichever is not empty.
fn join_field(outer: &str, inner: &str) -> String {
    match (outer.is_empty(), inner.is_empty()) {
        (true, _) => inner.to_owned(),
        (false, true) => outer.to_owned(),
        (false, false) => format!("{outer}.{inner}"),
    }
}

fn field_note(field: &str) -> String {
    if field.is_empty() {
        String::new()
    } else {
        format!(", field `{field}`")
    }
}

fn list(names: &[String]) -> String {
    if names.is_empty() {
        "none".to_owned()
    } else {
        names
            .iter()
            .map(|name| format!("`{name}`"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
#[path = "../tests/assets/data.rs"]
mod tests;
