//! Prefab files (M51): the manifest's `prefabs` loaded into a
//! `PrefabRegistry`, their hot reload, the manifest reload's prefab block,
//! and the check of every map's objects against the loaded prefabs.

use std::path::Path;

use tungsten_core::World;
use tungsten_core::assets::{PrefabData, PrefabRegistry, ResolvedManifest, TilemapRegistry};

/// Loads the manifest's prefabs in sorted ID order, each with
/// `PrefabData::load` against `world`, into a new `PrefabRegistry` with
/// their paths, which hot reload reads. A prefab's components are checked
/// against `world`, so the particle configs and tilemaps they name load
/// first; [`load_all`](super::load_all) runs this after them. The first
/// failure is an error naming the ID.
///
/// A game's tests, which load no manifest, call it on the harness's World
/// to seed the prefabs.
pub fn load_prefabs(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut registry = PrefabRegistry::new();
    let mut ids: Vec<&String> = manifest.prefabs.keys().collect();
    ids.sort();
    for id in ids {
        let entry = &manifest.prefabs[id];
        let data = PrefabData::load(&entry.path, world)
            .map_err(|e| anyhow::anyhow!("Failed to load prefab '{id}': {e}"))?;
        log::info!("Loaded prefab '{id}' from '{}'", entry.path.display());
        registry.insert_with_path(id.clone(), data, entry.path.clone());
    }
    world.insert_resource(registry);
    Ok(())
}

/// Hot-reloads prefab `id` from `path` for later spawns; a file that fails
/// its load keeps the previous prefab and logs why. Live entities stay as
/// they are.
pub fn reload_prefab(id: &str, path: &Path, world: &mut World) -> anyhow::Result<()> {
    let data = match PrefabData::load(path, world) {
        Ok(data) => data,
        Err(e) => {
            log::error!("Hot reload prefab '{id}': {e}; keeping the previous prefab");
            return Ok(());
        }
    };
    prefabs_mut(world).insert_with_path(id.to_owned(), data, path.to_path_buf());
    log::info!("Hot-reloaded prefab '{id}'");
    Ok(())
}

/// The manifest reload's prefab block, after the new tilemaps and particle
/// configs: each new prefab loads against the World, the new maps included
/// (a failure logs and skips it), and a removed one warns and stays. Then
/// the objects of each map the reload added (`new_maps`) are checked against
/// the prefabs now loaded; a failure logs the error, the map keeps its
/// tiles, and `World::spawn_map_objects` returns the same error.
pub(crate) fn reload_new_prefabs(
    new_manifest: &ResolvedManifest,
    world: &mut World,
    new_maps: &[String],
) {
    let existing: Vec<String> = world
        .get_resource::<PrefabRegistry>()
        .map(|registry| registry.ids().map(ToString::to_string).collect())
        .unwrap_or_default();
    for id in &existing {
        if !new_manifest.prefabs.contains_key(id.as_str()) {
            log::warn!("Manifest reload: prefab '{id}' removed — keeping stale");
        }
    }
    let mut ids: Vec<&String> = new_manifest.prefabs.keys().collect();
    ids.sort();
    for id in ids {
        if existing.iter().any(|e| e == id) {
            continue;
        }
        let entry = &new_manifest.prefabs[id];
        match PrefabData::load(&entry.path, world) {
            Ok(data) => {
                prefabs_mut(world).insert_with_path(id.clone(), data, entry.path.clone());
                log::info!("Manifest reload: loaded new prefab '{id}'");
            }
            Err(e) => log::error!("Manifest reload: new prefab '{id}': {e}"),
        }
    }
    let mut maps: Vec<&String> = new_maps.iter().collect();
    maps.sort();
    for map in maps {
        let checked = world
            .get_resource::<TilemapRegistry>()
            .and_then(|maps| maps.get(map))
            .map(|data| world.check_map_objects(map, data));
        if let Some(Err(e)) = checked {
            log::error!("Manifest reload: new tilemap '{map}' keeps its tiles, but {e}");
        }
    }
}

/// Every map's objects against the loaded prefabs, in sorted map ID order:
/// [`load_all`](super::load_all)'s last check, so a map whose object names
/// an unknown prefab fails the load as an unknown tileset sprite does.
pub(crate) fn check_all_map_objects(world: &World) -> anyhow::Result<()> {
    let Some(maps) = world.get_resource::<TilemapRegistry>() else {
        return Ok(());
    };
    let mut ids: Vec<&str> = maps.ids().collect();
    ids.sort_unstable();
    for id in ids {
        let data = maps.get(id).expect("a listed tilemap ID");
        world
            .check_map_objects(id, data)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    Ok(())
}

/// The World's `PrefabRegistry`, inserted first when it has none.
fn prefabs_mut(world: &mut World) -> &mut PrefabRegistry {
    if !world.has_resource::<PrefabRegistry>() {
        world.insert_resource(PrefabRegistry::new());
    }
    world.resource_mut::<PrefabRegistry>()
}

#[cfg(test)]
#[path = "../tests/prefabs.rs"]
mod tests;
