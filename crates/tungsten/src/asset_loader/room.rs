//! Rooms and tilesets (redo 7.12): the manifest's `tilesets` and `rooms`
//! loaded into the World's `RoomRegistry`, each room compiled and its map
//! registered under its ID, after the physics layer names (which a
//! collision layer names) and before the prefabs, so the cross-checks that
//! follow cover each room's tileset sprites and objects as a map's.

use std::path::Path;

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

#[cfg(test)]
#[path = "../tests/rooms.rs"]
pub(crate) mod tests;
