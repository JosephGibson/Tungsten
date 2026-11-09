//! Rooms in the World (redo 7.12): the [`RoomRegistry`] of tilesets, room
//! documents and their compiled forms; [`World::insert_room`], which
//! compiles a room and registers its map; [`World::spawn_room`], which
//! spawns it as an area with its objects named `<room>.<id>`;
//! [`World::remove_room`], which takes it out with its map and its area.
//!
//! A room is live at most once: the registry records the instance
//! `spawn_room` spawned, and a second spawn of the room while that entity
//! lives is refused with [`AreaError::Live`], whether or not its objects
//! still hold their names (a dead entity's name is free). Saves work as for
//! any area (`D-230`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use glam::Vec2;

use super::{CompiledPlace, CompiledRoom, PlaceShape, Room, RoomError, RoomJournal, Tileset, hint};
use crate::assets::{TilemapInstance, TilemapRegistry};
use crate::ecs::{Bundle, Entity, World};
use crate::lighting::AmbientLight;
use crate::physics::PhysicsLayers;
use crate::post::PostStack;
use crate::save::{AreaError, SaveState, name};

/// The rooms and tilesets a World holds, a resource: each room's document,
/// its compiled form and file, and the instance its last spawn made; each
/// tileset with its file. The asset loader inserts it only for a manifest
/// that lists a room or a tileset, and [`World::insert_room`] when a game
/// builds rooms in code.
#[derive(Debug, Default)]
pub struct RoomRegistry {
    pub(super) rooms: BTreeMap<String, Registered>,
    pub(super) tilesets: BTreeMap<String, RegisteredTileset>,
}

#[derive(Debug)]
pub(super) struct Registered {
    pub(super) document: Room,
    pub(super) compiled: CompiledRoom,
    pub(super) path: Option<PathBuf>,
    pub(super) live: Option<Entity>,
}

#[derive(Debug)]
pub(super) struct RegisteredTileset {
    pub(super) tileset: Tileset,
    pub(super) path: Option<PathBuf>,
}

impl RoomRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `tileset` as `id`, replacing a tileset of that ID; rooms
    /// already compiled on the old one keep their maps until they are
    /// inserted again.
    pub fn insert_tileset(&mut self, id: impl Into<String>, tileset: Tileset) {
        self.tilesets.insert(
            id.into(),
            RegisteredTileset {
                tileset,
                path: None,
            },
        );
    }

    /// [`insert_tileset`](Self::insert_tileset), keeping the file it was
    /// loaded from.
    pub fn insert_tileset_with_path(
        &mut self,
        id: impl Into<String>,
        tileset: Tileset,
        path: PathBuf,
    ) {
        self.tilesets.insert(
            id.into(),
            RegisteredTileset {
                tileset,
                path: Some(path),
            },
        );
    }

    /// The tileset `id`.
    #[must_use]
    pub fn tileset(&self, id: &str) -> Option<&Tileset> {
        self.tilesets.get(id).map(|entry| &entry.tileset)
    }

    /// The file tileset `id` was loaded from.
    #[must_use]
    pub fn tileset_path(&self, id: &str) -> Option<&Path> {
        self.tilesets
            .get(id)
            .and_then(|entry| entry.path.as_deref())
    }

    /// The tileset IDs, sorted.
    pub fn tileset_ids(&self) -> impl Iterator<Item = &str> {
        self.tilesets.keys().map(String::as_str)
    }

    /// Records `path` as the file tileset `id` loads from, as a manifest
    /// reload that moved it does. False for a tileset the registry lacks.
    pub fn set_tileset_path(&mut self, id: &str, path: PathBuf) -> bool {
        self.tilesets
            .get_mut(id)
            .map(|entry| entry.path = Some(path))
            .is_some()
    }

    /// The room IDs, sorted.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.rooms.keys().map(String::as_str)
    }

    /// Room `id`'s document, as its file gave it.
    #[must_use]
    pub fn room(&self, id: &str) -> Option<&Room> {
        self.rooms.get(id).map(|entry| &entry.document)
    }

    /// Room `id` compiled: its map before the auto-tiler, its places and
    /// its settings.
    #[must_use]
    pub fn compiled(&self, id: &str) -> Option<&CompiledRoom> {
        self.rooms.get(id).map(|entry| &entry.compiled)
    }

    /// The file room `id` was loaded from.
    #[must_use]
    pub fn path(&self, id: &str) -> Option<&Path> {
        self.rooms.get(id).and_then(|entry| entry.path.as_deref())
    }

    /// Records `path` as the file room `id` loads from, as a manifest
    /// reload that moved it does. False for a room the registry lacks.
    pub fn set_path(&mut self, id: &str, path: PathBuf) -> bool {
        self.rooms
            .get_mut(id)
            .map(|entry| entry.path = Some(path))
            .is_some()
    }

    /// Place `id` of room `room`, in the room's own pixels.
    #[must_use]
    pub fn place(&self, room: &str, id: &str) -> Option<&CompiledPlace> {
        self.compiled(room).and_then(|compiled| compiled.place(id))
    }

    /// The instance room `id`'s last spawn made, live or not.
    #[must_use]
    pub fn live(&self, id: &str) -> Option<Entity> {
        self.rooms.get(id).and_then(|entry| entry.live)
    }
}

/// A room [`World::spawn_room`] spawned: its instance, its objects by ID
/// and its places in world pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnedRoom {
    instance: Entity,
    origin: Vec2,
    objects: BTreeMap<String, Entity>,
    places: BTreeMap<String, CompiledPlace>,
}

impl SpawnedRoom {
    /// The room's map instance, the entity its liveness is read from.
    #[must_use]
    pub fn instance(&self) -> Entity {
        self.instance
    }

    /// Where the room's top-left was spawned.
    #[must_use]
    pub fn origin(&self) -> Vec2 {
        self.origin
    }

    /// The object `id` spawned as, named `<room>.<id>`.
    #[must_use]
    pub fn object(&self, id: &str) -> Option<Entity> {
        self.objects.get(id).copied()
    }

    /// Each object by its room ID, sorted.
    pub fn objects(&self) -> impl Iterator<Item = (&str, Entity)> {
        self.objects
            .iter()
            .map(|(id, &entity)| (id.as_str(), entity))
    }

    /// Place `id` in world pixels: the room's own, moved by its origin.
    #[must_use]
    pub fn place(&self, id: &str) -> Option<&CompiledPlace> {
        self.places.get(id)
    }
}

/// `place` moved by `origin`.
fn moved(place: &CompiledPlace, origin: Vec2) -> CompiledPlace {
    let shape = match &place.shape {
        PlaceShape::Point(point) => PlaceShape::Point(*point + origin),
        PlaceShape::Rect { min, size } => PlaceShape::Rect {
            min: *min + origin,
            size: *size,
        },
        PlaceShape::Route(stops) => PlaceShape::Route(stops.iter().map(|&p| p + origin).collect()),
    };
    CompiledPlace {
        shape,
        to: place.to.clone(),
        by: place.by.clone(),
    }
}

impl World {
    /// Compiles `room` as `id` against the tileset it names, which the
    /// [`RoomRegistry`] holds ([`RoomRegistry::insert_tileset`] registers one
    /// in code), and the World's physics layer names, then registers it: its
    /// map in the `TilemapRegistry` under `id` (auto-tiled there) and the
    /// room in the `RoomRegistry`, each inserted when the World has none.
    /// A room inserted again keeps the record of its live instance.
    ///
    /// # Errors
    ///
    /// A [`RoomError`] when the tileset is not registered, the room does not
    /// compile ([`Room::compile`]), or `id` is a tilemap's that no room
    /// registered. Nothing changes on an error.
    pub fn insert_room(&mut self, id: impl Into<String>, room: Room) -> Result<(), RoomError> {
        self.insert_room_from(id.into(), room, None)
    }

    /// [`insert_room`](Self::insert_room), keeping the file it was loaded
    /// from.
    ///
    /// # Errors
    ///
    /// As [`insert_room`](Self::insert_room).
    pub fn insert_room_with_path(
        &mut self,
        id: impl Into<String>,
        room: Room,
        path: PathBuf,
    ) -> Result<(), RoomError> {
        self.insert_room_from(id.into(), room, Some(path))
    }

    fn insert_room_from(
        &mut self,
        id: String,
        room: Room,
        path: Option<PathBuf>,
    ) -> Result<(), RoomError> {
        let empty = RoomRegistry::default();
        let registry = self.get_resource::<RoomRegistry>().unwrap_or(&empty);
        let blank = Tileset {
            version: super::TILESET_VERSION,
            tiles: Vec::new(),
        };
        let tileset = match &room.tileset {
            None => &blank,
            Some(name) => registry.tileset(name).ok_or_else(|| {
                RoomError::new(
                    "tileset",
                    format!("no tileset `{name}`{}", hint(name, registry.tileset_ids())),
                )
            })?,
        };
        let no_names = PhysicsLayers::default();
        let layers = self.get_resource::<PhysicsLayers>().unwrap_or(&no_names);
        let compiled = room.compile(&id, tileset, layers)?;
        let a_map = self
            .get_resource::<TilemapRegistry>()
            .is_some_and(|maps| maps.get(&id).is_some());
        if a_map && !registry.rooms.contains_key(&id) {
            return Err(RoomError::new(
                "",
                format!("`{id}` is a tilemap's ID; a room's ID is its map's"),
            ));
        }
        if !self.has_resource::<TilemapRegistry>() {
            self.insert_resource(TilemapRegistry::new());
        }
        self.resource_mut::<TilemapRegistry>()
            .insert(id.clone(), compiled.map.clone());
        if !self.has_resource::<RoomRegistry>() {
            self.insert_resource(RoomRegistry::new());
        }
        let rooms = &mut self.resource_mut::<RoomRegistry>().rooms;
        let live = rooms.get(&id).and_then(|entry| entry.live);
        rooms.insert(
            id,
            Registered {
                document: room,
                compiled,
                path,
                live,
            },
        );
        Ok(())
    }

    /// Spawns room `id` at `origin`, as [`spawn_area`](Self::spawn_area)
    /// spawns a map: its map put back as inserted, its instance, then each
    /// object from its prefab, named `<room>.<id>` in area `<room>`; `with`
    /// goes to the instance and to each object as it is prepared, in its one
    /// archetype move. Sets the World's `PostStack` and `AmbientLight` when
    /// the room gives them, leaving them otherwise, and records the instance
    /// as the room's live one.
    ///
    /// # Errors
    ///
    /// [`AreaError::UnknownRoom`] for a room the [`RoomRegistry`] lacks;
    /// [`AreaError::Live`] while the instance of the room's last spawn
    /// lives; then as `spawn_area`. Nothing changes on an error.
    ///
    /// # Panics
    ///
    /// As [`prepare_prefab`](Self::prepare_prefab).
    pub fn spawn_room<B: Bundle + Clone>(
        &mut self,
        id: &str,
        origin: impl Into<Vec2>,
        with: &B,
    ) -> Result<SpawnedRoom, AreaError> {
        let origin = origin.into();
        let unknown = || AreaError::UnknownRoom {
            room: id.to_owned(),
        };
        let entry = self
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.rooms.get(id))
            .ok_or_else(unknown)?;
        if entry.live.is_some_and(|instance| self.is_alive(instance)) {
            return Err(AreaError::Live {
                room: id.to_owned(),
            });
        }
        let places: BTreeMap<String, CompiledPlace> = entry
            .compiled
            .places
            .iter()
            .map(|(place, compiled)| (place.clone(), moved(compiled, origin)))
            .collect();
        let post = entry.compiled.post.clone();
        let ambient = entry.compiled.ambient;
        let prepared = self.prepare_map_objects_named(id, origin)?;
        self.check_area_names(id, &prepared)?;
        self.load_area_map(id);
        let instance = self.spawn_with((TilemapInstance::new(id, origin),).with(with.clone()));
        let prefix = format!("{id}.");
        let mut objects = BTreeMap::new();
        for (prefab, object) in prepared {
            let entity = prefab.with(with.clone()).spawn(self);
            name(self, entity, Some(id), &object).map_err(|source| AreaError::Name {
                map: id.to_owned(),
                source,
            })?;
            let local = object.strip_prefix(&prefix).unwrap_or(&object).to_owned();
            objects.insert(local, entity);
        }
        if let Some(entry) = self.resource_mut::<RoomRegistry>().rooms.get_mut(id) {
            entry.live = Some(instance);
        }
        if let Some(post) = post {
            self.insert_resource(PostStack(post));
        }
        if let Some(ambient) = ambient {
            self.insert_resource(AmbientLight(ambient));
        }
        Ok(SpawnedRoom {
            instance,
            origin,
            objects,
            places,
        })
    }

    /// Removes room `id` and returns its document: its entry in the
    /// [`RoomRegistry`] (document, compiled form, file and live record), its
    /// map from the `TilemapRegistry`, its [`RoomJournal`] history, and its
    /// area from the World's save state: loaded, kept, and every name given
    /// in it. Nothing despawns; the game despawns the room's entities first.
    /// References waiting on a kept record it forgets stay waiting, as those
    /// into an area never entered do. A manifest room removed is no room
    /// until a manifest reload loads it again.
    ///
    /// # Errors
    ///
    /// [`AreaError::UnknownRoom`] for a room the registry lacks;
    /// [`AreaError::Live`] while the instance of its last spawn lives;
    /// [`AreaError::Held`] while an entity named in its area lives, since a
    /// named entity saves as changes over the spawn its room makes. Nothing
    /// changes on an error.
    pub fn remove_room(&mut self, id: &str) -> Result<Room, AreaError> {
        let entry = self
            .get_resource::<RoomRegistry>()
            .and_then(|rooms| rooms.rooms.get(id))
            .ok_or_else(|| AreaError::UnknownRoom {
                room: id.to_owned(),
            })?;
        if entry.live.is_some_and(|instance| self.is_alive(instance)) {
            return Err(AreaError::Live {
                room: id.to_owned(),
            });
        }
        let held = self.get_resource::<SaveState>().and_then(|state| {
            state
                .named
                .values()
                .filter(|named| named.area.as_deref() == Some(id) && self.is_alive(named.entity))
                .map(|named| named.name.as_str())
                .min()
        });
        if let Some(name) = held {
            return Err(AreaError::Held {
                room: id.to_owned(),
                name: name.to_owned(),
            });
        }
        let entry = self
            .resource_mut::<RoomRegistry>()
            .rooms
            .remove(id)
            .expect("the room was found");
        if let Some(maps) = self.get_resource_mut::<TilemapRegistry>() {
            maps.remove(id);
        }
        if let Some(journal) = self.get_resource_mut::<RoomJournal>() {
            journal.clear(id);
        }
        if let Some(state) = self.get_resource_mut::<SaveState>() {
            state.loaded.remove(id);
            state.kept.remove(id);
            state
                .named
                .retain(|_, named| named.area.as_deref() != Some(id));
        }
        Ok(entry.document)
    }
}
