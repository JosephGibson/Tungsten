//! Live rooms patched (redo 7.12, §2.8 of its plan): a room file's reload,
//! its tileset's, a manifest reload and a script's `level` step take a room
//! from its compiled form before to the one after, on the main thread.
//!
//! A patch is prepared whole before it changes anything: the new document
//! compiled, its tileset's sprites checked against the registered sprites,
//! its objects against the prefabs ([`World::check_map_objects`]), and each
//! object it spawns prepared with its name checked. A failure leaves the
//! registry's room and tileset, the room's [`RoomJournal`] and the live
//! entities as they were. Then:
//!
//! 1. The registry takes the new document. Whether the live room changes is
//!    read from the compiled rooms, not the documents, so a tileset's edit
//!    reaches every room on it and an equal compiled room patches nothing.
//! 2. The map: with its size, cell, tileset (sprites and properties) and
//!    layers unchanged, each changed cell is written in place
//!    ([`TilemapRegistry::set_authored_tile`]), so a play's edits elsewhere
//!    stay; otherwise it is inserted whole, its history restarted. Either
//!    way its objects are replaced in the map and in its copy as inserted
//!    ([`TilemapRegistry::set_authored_objects`]), which the room's next
//!    spawn reads.
//! 3. The objects of the room's live instance, by ID: one gone despawns;
//!    one added spawns with what the World's spawn inheritance derives from
//!    the instance (`D-170`), then is named `<room>.<id>`; one whose prefab,
//!    `set` or `at` changed despawns and spawns again under its name, so its
//!    save baseline is its new spawn's. One play despawned stays gone, and
//!    the room's next spawn shows it.
//! 4. `post` and `ambient` are set again when they changed.
//!
//! A reload that changes a room's document or its tileset clears the room's
//! journal, whose inverses were written against them; one whose document
//! and tileset are equal keeps it. A `level` step journals its batch.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use glam::{Vec2, Vec3};

use super::spawn::RegisteredTileset;
use super::{
    CompiledRoom, ROOM_OBJECT_LAYER, Room, RoomCommand, RoomContext, RoomJournal, RoomRegistry,
    TILESET_VERSION, Tileset, hint,
};
use crate::assets::registry::AssetRegistry;
use crate::assets::tilemap::{MapObject, TileIndex, TilemapData};
use crate::assets::{
    PrefabError, PrefabRegistry, PreparedPrefab, TilemapInstance, TilemapRegistry,
};
use crate::ecs::{CommandBuffer, Entity, World};
use crate::lighting::AmbientLight;
use crate::physics::PhysicsLayers;
use crate::post::{PostPass, PostStack};
use crate::save::{SaveId, SaveState, name};

/// What a patch did to a room.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RoomPatch {
    /// The room's ID.
    pub room: String,
    /// How its map's cells changed.
    pub map: MapPatch,
    /// Whether its objects changed, live or not.
    pub objects: bool,
    /// The objects of its live instance despawned, gone from the room, by
    /// ID, sorted.
    pub despawned: Vec<String>,
    /// The objects spawned, new to the room, by ID, sorted.
    pub spawned: Vec<String>,
    /// The objects spawned again, their prefab, `set` or `at` changed, by
    /// ID, sorted.
    pub respawned: Vec<String>,
    /// Whether its places changed.
    pub places: bool,
    /// Whether its `post` or `ambient` changed.
    pub settings: bool,
}

impl RoomPatch {
    /// Whether it changed nothing: an equal compiled room's.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map == MapPatch::Unchanged
            && !self.objects
            && self.despawned.is_empty()
            && self.spawned.is_empty()
            && self.respawned.is_empty()
            && !self.places
            && !self.settings
    }
}

impl fmt::Display for RoomPatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        match self.map {
            MapPatch::Unchanged => {}
            MapPatch::Cells(cells) => parts.push(format!("{cells} cells written in place")),
            MapPatch::Inserted => parts.push("its map inserted whole".to_owned()),
        }
        for (verb, ids) in [
            ("spawned", &self.spawned),
            ("respawned", &self.respawned),
            ("despawned", &self.despawned),
        ] {
            if !ids.is_empty() {
                parts.push(format!("{verb} {}", ids.join(", ")));
            }
        }
        let live = self.spawned.len() + self.respawned.len() + self.despawned.len();
        if self.objects && live == 0 {
            parts.push("its objects".to_owned());
        }
        if self.places {
            parts.push("its places".to_owned());
        }
        if self.settings {
            parts.push("its post or ambient".to_owned());
        }
        if parts.is_empty() {
            write!(f, "room `{}` unchanged", self.room)
        } else {
            write!(f, "room `{}`: {}", self.room, parts.join("; "))
        }
    }
}

/// How a patch changed a room's map.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum MapPatch {
    /// Its cells are as they were.
    #[default]
    Unchanged,
    /// This many changed cells written in place.
    Cells(usize),
    /// Inserted whole, its history restarted: its size, cell, tileset or
    /// layers changed.
    Inserted,
}

/// Why a patch changed nothing, naming the room or tileset and the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomPatchError {
    message: String,
}

impl RoomPatchError {
    fn new(message: String) -> Self {
        Self { message }
    }

    fn room(id: &str, message: impl fmt::Display) -> Self {
        Self::new(format!("room `{id}`: {message}"))
    }

    /// What failed.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RoomPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RoomPatchError {}

/// One room's patch, prepared: everything that can fail has passed.
struct Prepared {
    id: String,
    document: Room,
    compiled: CompiledRoom,
    /// Whether it clears the room's journal.
    clear: bool,
    map: MapWrite,
    /// The live instance's changes, when the room has one.
    live: Option<Live>,
    report: RoomPatch,
}

/// The map's write: the changed cells (layer, column, row, tile), or the map
/// whole.
enum MapWrite {
    Cells(Vec<(usize, u32, u32, TileIndex)>),
    Whole,
}

/// A live instance's changes: what despawns, what spawns with its name,
/// and the settings set again.
struct Live {
    instance: Entity,
    despawn: Vec<Entity>,
    spawn: Vec<(PreparedPrefab, String)>,
    post: Option<Vec<PostPass>>,
    ambient: Option<Vec3>,
}

/// What a patch's spawn carries through its flush, so the patch finds the
/// entity the flush made; removed once found.
struct PatchSpawn;

impl World {
    /// Patches room `id` to `document`, as a reload of its file does: the
    /// document compiled against the tileset it names and the patch
    /// prepared whole (the module docs), then the registry, the map and the
    /// room's live instance changed. A changed document clears the room's
    /// [`RoomJournal`]; an equal one keeps it.
    ///
    /// # Errors
    ///
    /// A [`RoomPatchError`] when the World has no room `id`, the document
    /// names a tileset the registry lacks, does not compile, names a sprite
    /// the `AssetRegistry` lacks, or has an object that does not prepare or
    /// whose name a live entity holds. Nothing changes on an error.
    ///
    /// # Panics
    ///
    /// As [`prepare_prefab`](Self::prepare_prefab), for an object it spawns.
    pub fn patch_room(&mut self, id: &str, document: Room) -> Result<RoomPatch, RoomPatchError> {
        let (tileset, clear) = {
            let rooms = self.rooms_with(id)?;
            let clear = rooms.rooms[id].document != document;
            (tileset_of(rooms, id, &document)?, clear)
        };
        let prepared = self.prepare_patch(id, document, &tileset, clear)?;
        Ok(self.apply_patch(prepared))
    }

    /// Registers `tileset` as `id`, keeping its file, and patches every
    /// room on it, as a reload of its file does; `documents` gives rooms
    /// read again with it (a manifest reload's), each compiled against the
    /// tileset its new document names. A changed tileset clears the journal
    /// of each room on it, a changed document its room's. An equal tileset
    /// with no documents changes nothing.
    ///
    /// # Errors
    ///
    /// A [`RoomPatchError`] for a document of a room the World lacks, or
    /// the first room whose patch fails to prepare; then the registry keeps
    /// the tileset it had and every room on it stays as it was.
    ///
    /// # Panics
    ///
    /// As [`patch_room`](Self::patch_room).
    pub fn patch_tileset(
        &mut self,
        id: &str,
        tileset: Tileset,
        documents: Vec<(String, Room)>,
    ) -> Result<Vec<RoomPatch>, RoomPatchError> {
        let empty = RoomRegistry::default();
        let rooms = self.get_resource::<RoomRegistry>().unwrap_or(&empty);
        let changed = rooms.tileset(id) != Some(&tileset);
        if !changed && documents.is_empty() {
            return Ok(Vec::new());
        }
        let mut given: BTreeMap<String, Room> = BTreeMap::new();
        for (room, document) in documents {
            if !rooms.rooms.contains_key(&room) {
                return Err(RoomPatchError::new(format!(
                    "room `{room}`: no such room{}",
                    hint(&room, rooms.ids())
                )));
            }
            given.insert(room, document);
        }
        let mut jobs = Vec::new();
        for (room, entry) in &rooms.rooms {
            let on_it = entry.document.tileset.as_deref() == Some(id);
            let document = match given.remove(room) {
                Some(document) => document,
                None if on_it => entry.document.clone(),
                None => continue,
            };
            let names_it = document.tileset.as_deref() == Some(id);
            let compile_on = if names_it {
                tileset.clone()
            } else {
                tileset_of(rooms, room, &document)?
            };
            let clear = entry.document != document || (names_it && changed);
            jobs.push((room.clone(), document, compile_on, clear));
        }
        let mut prepared = Vec::with_capacity(jobs.len());
        for (room, document, compile_on, clear) in jobs {
            prepared.push(self.prepare_patch(&room, document, &compile_on, clear)?);
        }
        if !self.has_resource::<RoomRegistry>() {
            self.insert_resource(RoomRegistry::new());
        }
        let tilesets = &mut self.resource_mut::<RoomRegistry>().tilesets;
        let path = tilesets.get(id).and_then(|entry| entry.path.clone());
        tilesets.insert(id.to_owned(), RegisteredTileset { tileset, path });
        Ok(prepared
            .into_iter()
            .map(|prepared| self.apply_patch(prepared))
            .collect())
    }

    /// Applies `batch` to room `id`'s document ([`Room::apply`], against
    /// its tileset, the World's physics layer names and its prefab IDs),
    /// patches the room as [`patch_room`](Self::patch_room) does and
    /// journals the batch with its inverse in the World's [`RoomJournal`]:
    /// a script's `level` step. No file is written, so the room file's next
    /// reload wins.
    ///
    /// # Errors
    ///
    /// A [`RoomPatchError`] for a batch that does not apply, then as
    /// [`patch_room`](Self::patch_room); the room, its journal and the
    /// entities stay as they were.
    ///
    /// # Panics
    ///
    /// As [`patch_room`](Self::patch_room).
    pub fn apply_room_batch(
        &mut self,
        id: &str,
        batch: &[RoomCommand],
    ) -> Result<RoomPatch, RoomPatchError> {
        let mut document = self.rooms_with(id)?.rooms[id].document.clone();
        let inverse = self
            .context_parts(id, &document)?
            .with(id, |context| document.apply(batch, context))
            .map_err(|err| RoomPatchError::room(id, err))?;
        let tileset = tileset_of(self.rooms_with(id)?, id, &document)?;
        let prepared = self.prepare_patch(id, document, &tileset, false)?;
        let report = self.apply_patch(prepared);
        if !self.has_resource::<RoomJournal>() {
            self.insert_resource(RoomJournal::new());
        }
        self.resource_mut::<RoomJournal>()
            .record(id, batch.to_vec(), inverse);
        Ok(report)
    }

    /// Undoes room `id`'s newest batch in the World's [`RoomJournal`]: its
    /// inverse applied to the room's document and the room patched as
    /// [`apply_room_batch`](Self::apply_room_batch) patches it. `Ok(None)`
    /// when the room has nothing to undo.
    ///
    /// # Errors
    ///
    /// As [`apply_room_batch`](Self::apply_room_batch), for an inverse that
    /// no longer applies; the journal stays as it was.
    ///
    /// # Panics
    ///
    /// As [`patch_room`](Self::patch_room).
    pub fn undo_room_batch(&mut self, id: &str) -> Result<Option<RoomPatch>, RoomPatchError> {
        let Some(mut journal) = self.get_resource::<RoomJournal>().cloned() else {
            return Ok(None);
        };
        if journal.undo_len(id) == 0 {
            return Ok(None);
        }
        let mut document = self.rooms_with(id)?.rooms[id].document.clone();
        self.context_parts(id, &document)?
            .with(id, |context| journal.undo(id, &mut document, context))
            .map_err(|err| RoomPatchError::room(id, err))?;
        let tileset = tileset_of(self.rooms_with(id)?, id, &document)?;
        let prepared = self.prepare_patch(id, document, &tileset, false)?;
        let report = self.apply_patch(prepared);
        *self.resource_mut::<RoomJournal>() = journal;
        Ok(Some(report))
    }

    /// The World's `RoomRegistry` when it holds room `id`.
    fn rooms_with(&self, id: &str) -> Result<&RoomRegistry, RoomPatchError> {
        let Some(rooms) = self.get_resource::<RoomRegistry>() else {
            return Err(RoomPatchError::room(
                id,
                "no such room; the World has no rooms",
            ));
        };
        if rooms.rooms.contains_key(id) {
            Ok(rooms)
        } else {
            Err(RoomPatchError::room(
                id,
                format!("no such room{}", hint(id, rooms.ids())),
            ))
        }
    }

    /// What a batch on room `id`, now `document`, is checked against.
    fn context_parts(&self, id: &str, document: &Room) -> Result<ContextParts, RoomPatchError> {
        self.rooms_with(id)?;
        ContextParts::of(self, id, document)
    }

    /// Prepares room `id`'s patch to `document`, compiled on `tileset`:
    /// everything checked and every spawn built, nothing changed but the
    /// sprite names its spawns intern.
    fn prepare_patch(
        &mut self,
        id: &str,
        document: Room,
        tileset: &Tileset,
        clear: bool,
    ) -> Result<Prepared, RoomPatchError> {
        let fail = |message: String| RoomPatchError::room(id, message);
        let compiled = {
            let no_names = PhysicsLayers::default();
            let layers = self.get_resource::<PhysicsLayers>().unwrap_or(&no_names);
            document
                .compile(id, tileset, layers)
                .map_err(|err| fail(err.to_string()))?
        };
        if let Some(sprites) = self.get_resource::<AssetRegistry>() {
            for (index, sprite) in compiled.map.tileset.iter().enumerate() {
                if sprites.get_sprite(sprite).is_none() {
                    return Err(fail(format!(
                        "tileset `{}`: tiles[{index}].sprite: unknown sprite `{sprite}`",
                        document.tileset.as_deref().unwrap_or_default()
                    )));
                }
            }
        }
        self.check_map_objects(id, &compiled.map)
            .map_err(|err| fail(object_error(&document, &err)))?;

        let rooms = self.rooms_with(id)?;
        let before = &rooms.rooms[id].compiled;
        let mut report = RoomPatch {
            room: id.to_owned(),
            objects: before.map.object_layers != compiled.map.object_layers,
            places: before.places != compiled.places,
            settings: before.post != compiled.post || before.ambient != compiled.ambient,
            ..RoomPatch::default()
        };
        let map = map_write(&before.map, &compiled.map);
        let instance = rooms.rooms[id]
            .live
            .filter(|&instance| self.is_alive(instance));
        let mut despawn = Vec::new();
        let mut spawn_names = BTreeSet::new();
        if instance.is_some() {
            let state = self.get_resource::<SaveState>();
            let holder = |name: &str| {
                state
                    .and_then(|state| state.named_entity(SaveId::authored(name)))
                    .filter(|&entity| self.is_alive(entity))
            };
            let prefix = format!("{id}.");
            let local = |name: &str| name.strip_prefix(&prefix).unwrap_or(name).to_owned();
            let old = objects_of(&before.map);
            let new = objects_of(&compiled.map);
            for (&name, &object) in &old {
                match new.get(name) {
                    None => {
                        if let Some(entity) = holder(name) {
                            despawn.push(entity);
                            report.despawned.push(local(name));
                        }
                    }
                    Some(&next) if !same_object(object, next) => {
                        if let Some(entity) = holder(name) {
                            despawn.push(entity);
                            spawn_names.insert(name.to_owned());
                            report.respawned.push(local(name));
                        }
                    }
                    Some(_) => {}
                }
            }
            for &name in new.keys().filter(|name| !old.contains_key(*name)) {
                spawn_names.insert(name.to_owned());
                report.spawned.push(local(name));
            }
            for name in &spawn_names {
                let save_id = SaveId::authored(name);
                if let Some(other) = state.and_then(|state| state.name_of(save_id))
                    && other != name
                {
                    return Err(fail(format!(
                        "save name `{name}` takes the ID of `{other}`; rename one"
                    )));
                }
                if holder(name).is_some_and(|entity| !despawn.contains(&entity)) {
                    return Err(fail(format!("save name `{name}` is held by a live entity")));
                }
            }
        }
        let live = match instance {
            Some(instance) => {
                let origin = self
                    .get::<TilemapInstance>(instance)
                    .map_or(Vec2::ZERO, |placed| placed.origin);
                let post = (before.post != compiled.post)
                    .then(|| compiled.post.clone())
                    .flatten();
                let ambient = (before.ambient != compiled.ambient)
                    .then_some(compiled.ambient)
                    .flatten();
                let spawn = if spawn_names.is_empty() {
                    Vec::new()
                } else {
                    self.prepare_objects_of(id, &compiled.map, origin, |object| {
                        spawn_names.contains(&object.name)
                    })
                    .map_err(|err| fail(object_error(&document, &err)))?
                };
                Some(Live {
                    instance,
                    despawn,
                    spawn,
                    post,
                    ambient,
                })
            }
            None => None,
        };
        Ok(Prepared {
            id: id.to_owned(),
            document,
            compiled,
            clear,
            map,
            live,
            report,
        })
    }

    /// Applies a prepared patch: the map, the registry, the live instance
    /// and the journal, in that order.
    fn apply_patch(&mut self, prepared: Prepared) -> RoomPatch {
        let Prepared {
            id,
            document,
            compiled,
            clear,
            map,
            live,
            mut report,
        } = prepared;
        if let Some(maps) = self.get_resource_mut::<TilemapRegistry>() {
            match map {
                MapWrite::Whole => {
                    maps.insert(id.clone(), compiled.map.clone());
                    report.map = MapPatch::Inserted;
                }
                MapWrite::Cells(cells) => {
                    for &(layer, col, row, tile) in &cells {
                        maps.set_authored_tile(&id, layer, col, row, tile);
                    }
                    if !cells.is_empty() {
                        report.map = MapPatch::Cells(cells.len());
                    }
                    maps.set_authored_objects(&id, compiled.map.object_layers.clone());
                }
            }
        }
        if let Some(entry) = self.resource_mut::<RoomRegistry>().rooms.get_mut(&id) {
            entry.document = document;
            entry.compiled = compiled;
        }
        if let Some(live) = live {
            for entity in live.despawn {
                self.despawn(entity);
            }
            for (prefab, object) in live.spawn {
                let Some(entity) = spawn_from(self, live.instance, prefab) else {
                    continue;
                };
                if let Err(err) = name(self, entity, Some(&id), &object) {
                    log::error!("room `{id}`: {err}");
                }
            }
            if let Some(post) = live.post {
                self.insert_resource(PostStack(post));
            }
            if let Some(ambient) = live.ambient {
                self.insert_resource(AmbientLight(ambient));
            }
        }
        if clear && let Some(journal) = self.get_resource_mut::<RoomJournal>() {
            journal.clear(&id);
        }
        report
    }
}

/// What a batch's [`RoomContext`] borrows, owned.
pub(super) struct ContextParts {
    tileset: Tileset,
    layers: PhysicsLayers,
    prefabs: Vec<String>,
}

impl ContextParts {
    /// The parts a batch on `document`, room `id`'s, is checked against:
    /// the tileset it names from the World's `RoomRegistry`, the World's
    /// physics layer names and its prefab IDs. Room `id` need not be
    /// registered, as a generated room is not yet.
    pub(super) fn of(world: &World, id: &str, document: &Room) -> Result<Self, RoomPatchError> {
        let empty = RoomRegistry::default();
        let rooms = world.get_resource::<RoomRegistry>().unwrap_or(&empty);
        let tileset = tileset_of(rooms, id, document)?;
        let layers = world
            .get_resource::<PhysicsLayers>()
            .cloned()
            .unwrap_or_default();
        let prefabs = world
            .get_resource::<PrefabRegistry>()
            .map(|prefabs| prefabs.ids().map(str::to_owned).collect())
            .unwrap_or_default();
        Ok(Self {
            tileset,
            layers,
            prefabs,
        })
    }

    /// Runs `check` with the context of room `room`.
    pub(super) fn with<T>(&self, room: &str, check: impl FnOnce(&RoomContext<'_>) -> T) -> T {
        let prefabs: Vec<&str> = self.prefabs.iter().map(String::as_str).collect();
        check(&RoomContext {
            room,
            tileset: &self.tileset,
            layers: &self.layers,
            prefabs: &prefabs,
        })
    }
}

/// The tileset `document`, room `id`'s, names: the registry's, or none for a
/// room without one.
fn tileset_of(rooms: &RoomRegistry, id: &str, document: &Room) -> Result<Tileset, RoomPatchError> {
    match &document.tileset {
        None => Ok(Tileset {
            version: TILESET_VERSION,
            tiles: Vec::new(),
        }),
        Some(name) => rooms.tileset(name).cloned().ok_or_else(|| {
            RoomPatchError::room(
                id,
                format!(
                    "tileset: no tileset `{name}`{}",
                    hint(name, rooms.tileset_ids())
                ),
            )
        }),
    }
}

/// How `before`'s map becomes `after`'s, both compiled and not yet
/// auto-tiled: cell by cell when only cells differ, else whole.
fn map_write(before: &TilemapData, after: &TilemapData) -> MapWrite {
    let tiles = after.tileset.len();
    let layers = after.layers.len();
    let same_frame = (
        before.tile_width,
        before.tile_height,
        before.width,
        before.height,
    ) == (
        after.tile_width,
        after.tile_height,
        after.width,
        after.height,
    ) && before.tileset == after.tileset
        && (0..tiles).all(|tile| {
            let tile = tile as TileIndex;
            before.tile_props(tile) == after.tile_props(tile)
        })
        && before.layers.len() == layers
        && before
            .layers
            .iter()
            .zip(&after.layers)
            .enumerate()
            .all(|(index, (a, b))| {
                (a.name == b.name && a.kind == b.kind)
                    && before.layer_physics(index) == after.layer_physics(index)
                    && before.layer_draw(index) == after.layer_draw(index)
            });
    if !same_frame {
        return MapWrite::Whole;
    }
    let width = (after.width as usize).max(1);
    let mut cells = Vec::new();
    for (layer, (a, b)) in before.layers.iter().zip(&after.layers).enumerate() {
        for (index, (&old, &new)) in a.tiles.iter().zip(&b.tiles).enumerate() {
            if old != new {
                cells.push((layer, (index % width) as u32, (index / width) as u32, new));
            }
        }
    }
    MapWrite::Cells(cells)
}

/// A compiled map's room objects by name.
fn objects_of(map: &TilemapData) -> BTreeMap<&str, &MapObject> {
    map.object_layers
        .iter()
        .filter(|layer| layer.name == ROOM_OBJECT_LAYER)
        .flat_map(|layer| &layer.objects)
        .map(|object| (object.name.as_str(), object))
        .collect()
}

/// Whether an object spawns the same: its prefab, `set` and `at`.
fn same_object(a: &MapObject, b: &MapObject) -> bool {
    a.class == b.class && a.properties == b.properties && a.position == b.position
}

/// A map object's error named by the room's entry: `objects[2] (`ball_1`)`.
fn object_error(document: &Room, err: &PrefabError) -> String {
    if let PrefabError::Object {
        object, message, ..
    } = err
    {
        let index = (*object as usize).saturating_sub(1);
        if let Some(entry) = document.objects.get(index) {
            return format!("objects[{index}] (`{}`): {message}", entry.id);
        }
    }
    err.to_string()
}

/// Spawns `prefab` now from `source`, taking what the World's spawn
/// inheritance derives from it, and returns the entity; `None` when the
/// source is dead. The prefab spawns unsaved, so a saved source gives it
/// no save ID: the patch names it.
fn spawn_from(world: &mut World, source: Entity, prefab: PreparedPrefab) -> Option<Entity> {
    let mut buffer = CommandBuffer::new();
    buffer.spawn_prefab_from(source, prefab.unsaved().with((PatchSpawn,)));
    world.flush(buffer);
    let entity = world
        .query::<(Entity, &PatchSpawn)>()
        .map(|(entity, _)| entity)
        .next()?;
    world.remove_component::<PatchSpawn>(entity);
    Some(entity)
}

#[cfg(test)]
#[path = "../../tests/assets/room_patch.rs"]
mod tests;
