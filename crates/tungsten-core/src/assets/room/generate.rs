//! Room generators (redo 7.12, P7-16): code that builds rooms in memory from
//! a seed and the registered rooms it reads as templates, registered by ID
//! ([`World::register_generator`]) and run by [`World::generate_rooms`].
//!
//! A generator copies a template ([`Generate::template`]), edits it by
//! commands ([`Generate::apply`]) or its `pub` fields, and adds it
//! ([`Generate::add`]); each room it adds registers as
//! `<prefix>_<local>`, with its objects named `<room>.<id>` as any room's,
//! and carries the [`Generation`] that made it: the generator, its version,
//! the seed, the prefix and a hash of each template read. The same
//! generator, version, templates, seed and prefix give the same room IDs,
//! documents and object names, so the same save IDs.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use super::patch::ContextParts;
use super::spawn::RoomRegistry;
use super::{
    CompiledRoom, Room, RoomCommand, RoomError, TILESET_VERSION, Tileset, check_name, hint,
};
use crate::assets::TilemapRegistry;
use crate::ecs::World;
use crate::physics::PhysicsLayers;
use crate::rng::Pcg32;
use crate::save::SaveId;

/// Code that builds rooms from a seed: a game registers one by ID
/// ([`World::register_generator`]), and [`World::generate_rooms`] runs it.
///
/// A migration ([`migrate`](Self::migrate)) rewrites values and drops
/// records; it neither renames objects nor moves terrain, since saved
/// references name objects by their save IDs and saved resources (the kit's
/// tile statuses) name map cells. A version whose rooms move cells or object
/// IDs refuses its old saves instead.
pub trait RoomGenerator: 'static {
    /// The version the same seed gives the same rooms at; bump it when it
    /// would not.
    fn version(&self) -> u32;

    /// Whether it can still generate at `version`; by default only its own.
    fn reproduces(&self, version: u32) -> bool {
        version == self.version()
    }

    /// Adds the rooms `generation`'s seed gives at
    /// [`generation.version()`](Generate::version).
    ///
    /// # Errors
    ///
    /// Why it could not, which fails the generation
    /// ([`GenerateError::Failed`]).
    fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String>;

    /// Rewrites a saved area made under `from` for its room's generation
    /// `now` (`None`: an authored room, or none); false refuses the restore.
    /// By default it refuses.
    fn migrate(&self, from: &Generation, now: Option<&Generation>, area: &mut Value) -> bool {
        let _ = (from, now, area);
        false
    }
}

/// The generators a World holds by ID, a resource
/// [`World::register_generator`] inserts.
#[derive(Default)]
pub struct RoomGenerators {
    generators: BTreeMap<String, Box<dyn RoomGenerator>>,
}

impl RoomGenerators {
    /// No generators.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `generator` as `id`.
    ///
    /// # Panics
    ///
    /// When `id` is not `[a-z][a-z0-9_]*` or is already a generator's.
    #[track_caller]
    pub fn register(&mut self, id: &str, generator: impl RoomGenerator) {
        assert!(
            check_name(id).is_ok(),
            "register_generator: `{id}` must match [a-z][a-z0-9_]*"
        );
        assert!(
            !self.generators.contains_key(id),
            "register_generator: `{id}` is already a room generator"
        );
        self.generators.insert(id.to_owned(), Box::new(generator));
    }

    /// The generator `id`.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&dyn RoomGenerator> {
        self.generators.get(id).map(|generator| &**generator)
    }

    /// The generator IDs, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.generators.keys().map(String::as_str)
    }

    /// Whether it holds no generator.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.generators.is_empty()
    }
}

impl fmt::Debug for RoomGenerators {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(
                self.generators
                    .iter()
                    .map(|(id, generator)| (id, generator.version())),
            )
            .finish()
    }
}

/// What made a generated room: its generator and version, the seed, the
/// prefix, and each template the generation read by ID with the 64-bit
/// FNV-1a of its canonical form as 16 hex digits. A save of a generated
/// area names it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Generation {
    /// The generator's ID.
    pub generator: String,
    /// The version it generated at.
    pub version: u32,
    /// The seed.
    pub seed: u64,
    /// The prefix of its rooms' IDs.
    pub prefix: String,
    /// Each template read, by ID, to its hash.
    pub templates: BTreeMap<String, String>,
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "`{}` version {} at seed {} under prefix `{}`",
            self.generator, self.version, self.seed, self.prefix
        )?;
        for (template, hash) in &self.templates {
            write!(f, ", template `{template}` {hash}")?;
        }
        Ok(())
    }
}

/// The rooms one generation registered: its record, and each room's ID and
/// origin in the order the generator added them.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedRooms {
    generation: Generation,
    rooms: Vec<(String, Vec2)>,
}

impl GeneratedRooms {
    /// What made them.
    #[must_use]
    pub fn generation(&self) -> &Generation {
        &self.generation
    }

    /// Each room's ID and the origin the generator gave it, where the game
    /// spawns it, in the order added.
    pub fn rooms(&self) -> impl Iterator<Item = (&str, Vec2)> {
        self.rooms.iter().map(|(id, origin)| (id.as_str(), *origin))
    }
}

/// Why [`World::generate_rooms`] registered nothing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum GenerateError {
    /// No generator of this ID is registered.
    #[error("no room generator `{generator}`{hint}")]
    UnknownGenerator {
        /// The ID asked for.
        generator: String,
        /// "; did you mean ..." or nothing.
        hint: String,
    },
    /// The prefix is not an ID.
    #[error("generation prefix `{prefix}` is not `[a-z][a-z0-9_]*`")]
    Prefix {
        /// The prefix given.
        prefix: String,
    },
    /// The generator's own error, or a room it added wrongly.
    #[error("generator `{generator}` at seed {seed}: {message}")]
    Failed {
        /// The generator.
        generator: String,
        /// The seed.
        seed: u64,
        /// Why.
        message: String,
    },
    /// A template it asked for is not a registered room, or is a generated
    /// one.
    #[error("generator `{generator}` at seed {seed}: template `{template}`: {message}")]
    Template {
        /// The generator.
        generator: String,
        /// The seed.
        seed: u64,
        /// The template asked for.
        template: String,
        /// Why.
        message: String,
    },
    /// The generation added no room, so nothing would carry its record.
    #[error("generator `{generator}` at seed {seed} added no room")]
    Empty {
        /// The generator.
        generator: String,
        /// The seed.
        seed: u64,
    },
    /// A room's ID is a registered room's or map's: a run's rooms are
    /// removed before its prefix generates again.
    #[error("generated room `{room}`: a room or a map holds the ID; remove it first")]
    Taken {
        /// The room's ID.
        room: String,
    },
    /// A room did not compile.
    #[error("generated room `{room}`: {source}")]
    Room {
        /// The room's ID.
        room: String,
        /// Why.
        source: RoomError,
    },
    /// Two of the generation's object names take one save ID.
    #[error("save name `{name}` takes the ID of `{other}` in the same generation; rename one")]
    Clash {
        /// The later name.
        name: String,
        /// The name that has its ID.
        other: String,
    },
}

/// A generation in progress, which a [`RoomGenerator`] adds its rooms to:
/// the seed, the version and prefix it runs at, the seeded random stream,
/// the templates and a room's context. It borrows the World.
pub struct Generate<'w> {
    world: &'w World,
    seed: u64,
    version: u32,
    prefix: String,
    rng: Pcg32,
    templates: BTreeMap<String, String>,
    contexts: BTreeMap<Option<String>, ContextParts>,
    rooms: Vec<(String, Room, Vec2)>,
    locals: BTreeSet<String>,
    /// The first template refused or room added wrongly, which fails the
    /// generation whatever the generator returns.
    failure: Option<Failure>,
}

enum Failure {
    Template { template: String, message: String },
    Add(String),
}

impl Generate<'_> {
    /// The seed.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The version to generate at.
    #[must_use]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// The prefix of the rooms' IDs.
    #[must_use]
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// The generation's random stream, `Pcg32::seeded(seed)`.
    pub fn rng(&mut self) -> &mut Pcg32 {
        &mut self.rng
    }

    /// A copy of the registered room `id`'s document, recorded in the
    /// generation with its hash.
    ///
    /// # Errors
    ///
    /// For a room the World lacks or a generated one, which also fails the
    /// generation ([`GenerateError::Template`]).
    pub fn template(&mut self, id: &str) -> Result<Room, String> {
        let empty = RoomRegistry::default();
        let rooms = self.world.get_resource::<RoomRegistry>().unwrap_or(&empty);
        let found = match rooms.rooms.get(id) {
            None => Err(format!("no room `{id}`{}", hint(id, rooms.ids()))),
            Some(entry) if entry.generation.is_some() => Err(format!(
                "`{id}` is a generated room; a template is a file's or `insert_room`'s"
            )),
            Some(entry) => Ok(entry.document.clone()),
        };
        match found {
            Ok(document) => {
                self.templates.insert(id.to_owned(), hash(&document));
                Ok(document)
            }
            Err(message) => {
                if self.failure.is_none() {
                    self.failure = Some(Failure::Template {
                        template: id.to_owned(),
                        message: message.clone(),
                    });
                }
                Err(format!("template `{id}`: {message}"))
            }
        }
    }

    /// Room `local`'s ID in the World, `<prefix>_<local>`: what a link's
    /// `to` names.
    #[must_use]
    pub fn id(&self, local: &str) -> String {
        format!("{}_{local}", self.prefix)
    }

    /// Applies `batch` to `room`, to be added as `local`, as
    /// [`Room::apply`] does, against the tileset it names, the World's
    /// physics layer names and its prefab IDs; `room` is unchanged on an
    /// error.
    ///
    /// # Errors
    ///
    /// The batch's error, naming the room; the generator decides whether it
    /// fails the generation.
    pub fn apply(
        &mut self,
        local: &str,
        room: &mut Room,
        batch: &[RoomCommand],
    ) -> Result<(), String> {
        let id = self.id(local);
        let key = room.tileset.clone();
        if !self.contexts.contains_key(&key) {
            let parts = ContextParts::of(self.world, &id, room).map_err(|err| err.to_string())?;
            self.contexts.insert(key.clone(), parts);
        }
        self.contexts[&key]
            .with(&id, |context| room.apply(batch, context))
            .map(drop)
            .map_err(|err| format!("room `{id}`: {err}"))
    }

    /// Adds `room` as `local` (an ID, unique in the generation), to be
    /// spawned at `origin` in pixels.
    ///
    /// # Errors
    ///
    /// For a `local` that is not `[a-z][a-z0-9_]*` or was added already,
    /// which also fails the generation ([`GenerateError::Failed`]).
    pub fn add(&mut self, local: &str, room: Room, origin: impl Into<Vec2>) -> Result<(), String> {
        let refused = match check_name(local) {
            Err(why) => Some(format!("room {why}")),
            Ok(()) if self.locals.contains(local) => {
                Some(format!("room `{local}` was added already"))
            }
            Ok(()) => None,
        };
        if let Some(message) = refused {
            if self.failure.is_none() {
                self.failure = Some(Failure::Add(message.clone()));
            }
            return Err(message);
        }
        self.locals.insert(local.to_owned());
        self.rooms.push((local.to_owned(), room, origin.into()));
        Ok(())
    }
}

/// `document`'s hash as a generation records it: the 64-bit FNV-1a of its
/// canonical form, as 16 hex digits.
fn hash(document: &Room) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in document.to_canonical().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Template `id`'s hash now, as a generation that read it records it:
/// `None` for a room the registry lacks or a generated one.
fn template_hash(rooms: &RoomRegistry, id: &str) -> Option<String> {
    let entry = rooms.rooms.get(id)?;
    entry.generation.is_none().then(|| hash(&entry.document))
}

/// A generation prepared whole: every room compiled and checked, nothing
/// registered.
pub(crate) struct PreparedGeneration {
    generation: Generation,
    rooms: Vec<(String, Room, CompiledRoom, Vec2)>,
}

impl PreparedGeneration {
    /// Its rooms' IDs, in the order added.
    pub(crate) fn ids(&self) -> impl Iterator<Item = &str> {
        self.rooms.iter().map(|(id, ..)| id.as_str())
    }
}

impl World {
    /// Registers `generator` as `id` in the World's [`RoomGenerators`],
    /// inserted when it has none.
    ///
    /// # Panics
    ///
    /// When `id` is not `[a-z][a-z0-9_]*` or is already a generator's.
    #[track_caller]
    pub fn register_generator(&mut self, id: &str, generator: impl RoomGenerator) {
        if !self.has_resource::<RoomGenerators>() {
            self.insert_resource(RoomGenerators::new());
        }
        self.resource_mut::<RoomGenerators>()
            .register(id, generator);
    }

    /// Runs generator `generator` at its version on `seed` and registers
    /// each room it adds as `<prefix>_<local>`, with its [`Generation`], as
    /// [`insert_room`](Self::insert_room) registers a room. The generation
    /// is prepared whole first: every room compiled against the tileset it
    /// names and the World's physics layer names, its ID free in the
    /// `RoomRegistry` and the `TilemapRegistry`, and every object name across
    /// the generation taking its own save ID. Nothing spawns.
    ///
    /// # Errors
    ///
    /// A [`GenerateError`]; nothing changes on an error.
    pub fn generate_rooms(
        &mut self,
        generator: &str,
        seed: u64,
        prefix: &str,
    ) -> Result<GeneratedRooms, GenerateError> {
        let version = self
            .get_resource::<RoomGenerators>()
            .and_then(|generators| generators.get(generator))
            .map(RoomGenerator::version)
            .ok_or_else(|| self.unknown_generator(generator))?;
        let prepared = self.prepare_generation(generator, version, seed, prefix)?;
        Ok(self.register_generation(prepared))
    }

    fn unknown_generator(&self, generator: &str) -> GenerateError {
        let empty = RoomGenerators::new();
        let generators = self.get_resource::<RoomGenerators>().unwrap_or(&empty);
        GenerateError::UnknownGenerator {
            generator: generator.to_owned(),
            hint: hint(generator, generators.ids()),
        }
    }

    /// A saved generation prepared again, as a restore of its save needs
    /// it (`save::regenerate`): `None` when the World holds a room under an
    /// equal record; else at the saved version when the generator still
    /// reproduces it and every saved template's hash matches the template
    /// now, at its current version otherwise.
    pub(crate) fn prepare_regeneration(
        &self,
        saved: &Generation,
    ) -> Result<Option<PreparedGeneration>, GenerateError> {
        let empty = RoomRegistry::default();
        let rooms = self.get_resource::<RoomRegistry>().unwrap_or(&empty);
        if rooms
            .rooms
            .values()
            .any(|entry| entry.generation.as_ref() == Some(saved))
        {
            return Ok(None);
        }
        let generator = self
            .get_resource::<RoomGenerators>()
            .and_then(|generators| generators.get(&saved.generator))
            .ok_or_else(|| self.unknown_generator(&saved.generator))?;
        let same = generator.reproduces(saved.version)
            && saved
                .templates
                .iter()
                .all(|(id, hash)| template_hash(rooms, id).as_deref() == Some(hash.as_str()));
        let version = if same {
            saved.version
        } else {
            generator.version()
        };
        self.prepare_generation(&saved.generator, version, saved.seed, &saved.prefix)
            .map(Some)
    }

    /// Generation `generator` at `version` on `seed` under `prefix`,
    /// prepared whole: run, compiled and checked, nothing registered.
    pub(crate) fn prepare_generation(
        &self,
        generator: &str,
        version: u32,
        seed: u64,
        prefix: &str,
    ) -> Result<PreparedGeneration, GenerateError> {
        if check_name(prefix).is_err() {
            return Err(GenerateError::Prefix {
                prefix: prefix.to_owned(),
            });
        }
        let found = self
            .get_resource::<RoomGenerators>()
            .and_then(|generators| generators.get(generator))
            .ok_or_else(|| self.unknown_generator(generator))?;
        let mut generation = Generate {
            world: self,
            seed,
            version,
            prefix: prefix.to_owned(),
            rng: Pcg32::seeded(seed),
            templates: BTreeMap::new(),
            contexts: BTreeMap::new(),
            rooms: Vec::new(),
            locals: BTreeSet::new(),
            failure: None,
        };
        let ran = found.generate(&mut generation);
        let failed = |message: String| GenerateError::Failed {
            generator: generator.to_owned(),
            seed,
            message,
        };
        match (generation.failure.take(), ran) {
            (Some(Failure::Template { template, message }), _) => {
                return Err(GenerateError::Template {
                    generator: generator.to_owned(),
                    seed,
                    template,
                    message,
                });
            }
            (_, Err(message)) | (Some(Failure::Add(message)), Ok(())) => {
                return Err(failed(message));
            }
            (None, Ok(())) => {}
        }
        if generation.rooms.is_empty() {
            return Err(GenerateError::Empty {
                generator: generator.to_owned(),
                seed,
            });
        }
        let empty = RoomRegistry::default();
        let registry = self.get_resource::<RoomRegistry>().unwrap_or(&empty);
        let maps = self.get_resource::<TilemapRegistry>();
        let no_names = PhysicsLayers::default();
        let layers = self.get_resource::<PhysicsLayers>().unwrap_or(&no_names);
        let blank = Tileset {
            version: TILESET_VERSION,
            tiles: Vec::new(),
        };
        let mut names: BTreeMap<SaveId, String> = BTreeMap::new();
        let mut rooms = Vec::with_capacity(generation.rooms.len());
        for (local, document, origin) in std::mem::take(&mut generation.rooms) {
            let id = generation.id(&local);
            if registry.rooms.contains_key(&id) || maps.is_some_and(|maps| maps.get(&id).is_some())
            {
                return Err(GenerateError::Taken { room: id });
            }
            let room_error = |source: RoomError| GenerateError::Room {
                room: id.clone(),
                source,
            };
            let tileset = match &document.tileset {
                None => &blank,
                Some(name) => registry.tileset(name).ok_or_else(|| {
                    room_error(RoomError::new(
                        "tileset",
                        format!("no tileset `{name}`{}", hint(name, registry.tileset_ids())),
                    ))
                })?,
            };
            let compiled = document.compile(&id, tileset, layers).map_err(room_error)?;
            for object in compiled
                .map
                .object_layers
                .iter()
                .flat_map(|layer| &layer.objects)
            {
                if let Some(other) =
                    names.insert(SaveId::authored(&object.name), object.name.clone())
                    && other != object.name
                {
                    return Err(GenerateError::Clash {
                        name: object.name.clone(),
                        other,
                    });
                }
            }
            rooms.push((id, document, compiled, origin));
        }
        Ok(PreparedGeneration {
            generation: Generation {
                generator: generator.to_owned(),
                version,
                seed,
                prefix: prefix.to_owned(),
                templates: generation.templates,
            },
            rooms,
        })
    }

    /// Registers each of `prepared`'s rooms with its generation.
    pub(crate) fn register_generation(&mut self, prepared: PreparedGeneration) -> GeneratedRooms {
        let mut rooms = Vec::with_capacity(prepared.rooms.len());
        for (id, document, compiled, origin) in prepared.rooms {
            rooms.push((id.clone(), origin));
            self.register_room(
                id,
                document,
                compiled,
                None,
                Some(prepared.generation.clone()),
            );
        }
        GeneratedRooms {
            generation: prepared.generation,
            rooms,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/assets/room_generate.rs"]
mod tests;
