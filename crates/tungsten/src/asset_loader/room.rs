//! Rooms and tilesets (redo 7.12): the manifest's `tilesets` and `rooms`
//! loaded into the World's `RoomRegistry`, each room compiled and its map
//! registered under its ID, after the physics layer names (which a
//! collision layer names) and before the prefabs, so the cross-checks that
//! follow cover each room's tileset sprites and objects as a map's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use tungsten_core::World;
use tungsten_core::assets::{ResolvedManifest, Room, RoomError, RoomRegistry, Tileset};

/// Whether `manifest` lists a room or a tileset: only then does a load
/// insert the `RoomRegistry`, so a game without rooms holds none.
pub(crate) fn has_rooms(manifest: &ResolvedManifest) -> bool {
    !manifest.rooms.is_empty() || !manifest.tilesets.is_empty()
}

/// Reads and parses the tileset file at `path`.
pub(crate) fn read_tileset(path: &Path) -> Result<Tileset, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    Tileset::from_json(&text).map_err(|e| e.to_string())
}

/// Reads and parses the room file at `path`.
pub(crate) fn read_room(path: &Path) -> Result<Room, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    Room::from_json(&text).map_err(|e| e.to_string())
}

/// A load plan's tileset unit: tileset `id` read and registered with its
/// file.
pub(crate) fn load_tileset_unit(
    manifest: &ResolvedManifest,
    id: &str,
    world: &mut World,
) -> anyhow::Result<()> {
    let path = &manifest.tilesets[id].path;
    let tileset = read_tileset(path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to load tileset '{id}' from '{}': {e}",
            path.display()
        )
    })?;
    log::info!(
        "Loaded tileset '{id}' ({} tiles) from '{}'",
        tileset.tiles.len(),
        path.display()
    );
    registry(world).insert_tileset_with_path(id, tileset, path.clone());
    Ok(())
}

/// A load plan's room unit: room `id` read, compiled against its tileset
/// and registered, its map under its ID.
pub(crate) fn load_room_unit(
    manifest: &ResolvedManifest,
    id: &str,
    world: &mut World,
) -> anyhow::Result<()> {
    let path = &manifest.rooms[id].path;
    let failed = |e: &dyn std::fmt::Display| {
        anyhow::anyhow!("Failed to load room '{id}' from '{}': {e}", path.display())
    };
    let room = read_room(path).map_err(|e| failed(&e))?;
    world
        .insert_room_with_path(id, room, path.clone())
        .map_err(|e: RoomError| failed(&e))?;
    log::info!("Loaded room '{id}' from '{}'", path.display());
    Ok(())
}

/// The World's `RoomRegistry`, inserted when it has none.
fn registry(world: &mut World) -> &mut RoomRegistry {
    if !world.has_resource::<RoomRegistry>() {
        world.insert_resource(RoomRegistry::new());
    }
    world.resource_mut::<RoomRegistry>()
}

/// `tungsten check`'s rooms: every tileset, then every room, sorted, each
/// failure sent to `error` with its file and its message, which names the
/// tileset or room and the field's path; inserts the registry when the
/// manifests list either.
pub(crate) fn check_rooms(
    manifest: &ResolvedManifest,
    world: &mut World,
    mut error: impl FnMut(&Path, String),
) {
    if !has_rooms(manifest) {
        return;
    }
    registry(world);
    let mut tilesets: Vec<&String> = manifest.tilesets.keys().collect();
    tilesets.sort();
    for id in tilesets {
        let path = &manifest.tilesets[id].path;
        match read_tileset(path) {
            Ok(tileset) => registry(world).insert_tileset_with_path(id, tileset, path.clone()),
            Err(e) => error(path, format!("tileset '{id}': {e}")),
        }
    }
    let mut rooms: Vec<&String> = manifest.rooms.keys().collect();
    rooms.sort();
    for id in rooms {
        let path = &manifest.rooms[id].path;
        let loaded = read_room(path).and_then(|room| {
            world
                .insert_room_with_path(id, room, path.clone())
                .map_err(|e| e.to_string())
        });
        if let Err(e) = loaded {
            error(path, format!("room '{id}': {e}"));
        }
    }
}

/// `tungsten check`'s level rules on every listed room, sorted: core's
/// (spawns, supports, links and budgets, the umbrella's `light` lighting an
/// object), then, with the kit, each route's reach; each finding sent to
/// `error` with its room's file.
pub(crate) fn check_room_rules(
    manifest: &ResolvedManifest,
    world: &World,
    mut error: impl FnMut(&Path, String),
) {
    let mut rooms: Vec<&String> = manifest.rooms.keys().collect();
    rooms.sort();
    for id in rooms {
        let path = &manifest.rooms[id].path;
        for finding in world.room_findings(id, &["light"]) {
            error(path, finding.to_string());
        }
        #[cfg(feature = "kit")]
        for finding in tungsten_kit::route_findings(world, id) {
            error(path, finding.to_string());
        }
    }
}

/// The room whose file is `path`, by the files the registry recorded.
pub(crate) fn room_for_path(world: &World, path: &Path) -> Option<String> {
    let rooms = world.get_resource::<RoomRegistry>()?;
    rooms
        .ids()
        .find(|id| rooms.path(id) == Some(path))
        .map(str::to_owned)
}

/// The tileset whose file is `path`, by the files the registry recorded.
pub(crate) fn tileset_for_path(world: &World, path: &Path) -> Option<String> {
    let rooms = world.get_resource::<RoomRegistry>()?;
    rooms
        .tileset_ids()
        .find(|id| rooms.tileset_path(id) == Some(path))
        .map(str::to_owned)
}

/// Room and tileset files read again (redo 7.12), a hot-reload batch's or
/// a manifest reload's (`what` begins each log line): each tileset patched
/// with the batch's rooms on it, then each other room alone
/// (`World::patch_tileset`, `World::patch_room`), so a tileset and a room
/// saved together that need each other load together. A file that fails to
/// read or to patch keeps the registry's entry and the live room, and its
/// line goes to `error`; a room left behind by its tileset's failure is
/// tried alone.
pub(crate) fn reload_room_files(
    tilesets: &[(String, PathBuf)],
    rooms: &[(String, PathBuf)],
    world: &mut World,
    what: &str,
    error: &mut impl FnMut(String),
) {
    let mut documents: BTreeMap<String, Room> = BTreeMap::new();
    for (id, path) in rooms {
        match read_room(path) {
            Ok(room) => {
                documents.insert(id.clone(), room);
            }
            Err(e) => error(format!("{what} room '{id}': {e}; keeping the live room")),
        }
    }
    for (id, path) in tilesets {
        let tileset = match read_tileset(path) {
            Ok(tileset) => tileset,
            Err(e) => {
                error(format!(
                    "{what} tileset '{id}': {e}; keeping the previous tileset"
                ));
                continue;
            }
        };
        let with: Vec<(String, Room)> = documents
            .iter()
            .filter(|(_, room)| room.tileset.as_deref() == Some(id.as_str()))
            .map(|(room, document)| (room.clone(), document.clone()))
            .collect();
        match world.patch_tileset(id, tileset, with.clone()) {
            Ok(patches) => {
                for (room, _) in &with {
                    documents.remove(room);
                }
                log::info!("{what}: tileset '{id}' read again");
                for patch in &patches {
                    log_patch(what, patch);
                }
            }
            Err(e) => error(format!(
                "{what} tileset '{id}': {e}; keeping the previous tileset and its rooms"
            )),
        }
    }
    for (id, document) in documents {
        match world.patch_room(&id, document) {
            Ok(patch) => log_patch(what, &patch),
            Err(e) => error(format!("{what} room '{id}': {e}; keeping the live room")),
        }
    }
}

/// A patch's log line: info when it changed the room, debug when not.
fn log_patch(what: &str, patch: &tungsten_core::assets::room::RoomPatch) {
    if patch.is_empty() {
        log::debug!("{what}: {patch}");
    } else {
        log::info!("{what}: {patch}");
    }
}

/// The manifest reload's warnings for what `held` registers and
/// `new_manifest` no longer lists: each tileset, and each room but a
/// generated one, which no manifest lists.
pub(crate) fn stale_warnings(held: &RoomRegistry, new_manifest: &ResolvedManifest) -> Vec<String> {
    let tilesets = held
        .tileset_ids()
        .filter(|id| !new_manifest.tilesets.contains_key(*id))
        .map(|id| format!("Manifest reload: tileset '{id}' removed — keeping stale"));
    let rooms = held
        .ids()
        .filter(|id| !new_manifest.rooms.contains_key(*id) && held.generation(id).is_none())
        .map(|id| format!("Manifest reload: room '{id}' removed — keeping stale"));
    tilesets.chain(rooms).collect()
}

/// The manifest reload's rooms and tilesets (redo 7.12), after its prefabs,
/// which their objects name: a new tileset registers; a new room inserts
/// with its map, its objects checked against the prefabs now loaded (a
/// failure logs while its tiles stay); every other listed tileset and room
/// is read again and patched as [`reload_room_files`] patches a batch, a
/// moved file's path recorded; one the manifest no longer lists warns and
/// stays, but for a generated room, which no manifest lists. A manifest
/// without rooms or tilesets, in a World that holds none, does nothing.
pub(crate) fn reload_rooms_from_manifest(new_manifest: &ResolvedManifest, world: &mut World) {
    if !has_rooms(new_manifest) && !world.has_resource::<RoomRegistry>() {
        return;
    }
    for line in stale_warnings(registry(world), new_manifest) {
        log::warn!("{line}");
    }
    let mut tilesets: Vec<(&String, &PathBuf)> = new_manifest
        .tilesets
        .iter()
        .map(|(id, entry)| (id, &entry.path))
        .collect();
    tilesets.sort();
    let mut kept_tilesets = Vec::new();
    for (id, path) in tilesets {
        let rooms = registry(world);
        if rooms.set_tileset_path(id, path.clone()) {
            kept_tilesets.push((id.clone(), path.clone()));
            continue;
        }
        match read_tileset(path) {
            Ok(tileset) => {
                rooms.insert_tileset_with_path(id, tileset, path.clone());
                log::info!("Manifest reload: loaded new tileset '{id}'");
            }
            Err(e) => log::error!("Manifest reload: new tileset '{id}': {e}"),
        }
    }
    let mut rooms: Vec<(&String, &PathBuf)> = new_manifest
        .rooms
        .iter()
        .map(|(id, entry)| (id, &entry.path))
        .collect();
    rooms.sort();
    let mut kept_rooms = Vec::new();
    for (id, path) in rooms {
        if registry(world).set_path(id, path.clone()) {
            kept_rooms.push((id.clone(), path.clone()));
            continue;
        }
        let inserted = read_room(path).and_then(|room| {
            world
                .insert_room_with_path(id, room, path.clone())
                .map_err(|e| e.to_string())
        });
        match inserted {
            Ok(()) => {
                log::info!("Manifest reload: loaded new room '{id}'");
                let checked = world
                    .get_resource::<tungsten_core::TilemapRegistry>()
                    .and_then(|maps| maps.get(id))
                    .map(|map| world.check_map_objects(id, map));
                if let Some(Err(e)) = checked {
                    log::error!("Manifest reload: new room '{id}' keeps its tiles, but {e}");
                }
            }
            Err(e) => log::error!("Manifest reload: new room '{id}': {e}"),
        }
    }
    reload_room_files(
        &kept_tilesets,
        &kept_rooms,
        world,
        "Manifest reload",
        &mut |line| log::error!("{line}"),
    );
}

#[cfg(test)]
#[path = "../tests/rooms.rs"]
pub(crate) mod tests;
