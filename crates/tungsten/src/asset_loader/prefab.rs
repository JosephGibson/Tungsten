//! Prefab and data files (M51, M61): the manifest's `prefabs` and `data`
//! loaded in two phases into the World's `PrefabRegistry` and
//! `DataRegistry`, their hot reload, the manifest reload's block, and the
//! check of every map's objects against the loaded prefabs.

use std::path::Path;

use tungsten_core::World;
use tungsten_core::assets::{
    LoadedManifest, PrefabRegistry, ResolvedManifest, StagedLoad, StagedLoadError, TilemapRegistry,
};

/// The way out a failed reload's log names: two edits saved apart that need
/// each other load together from the manifest's block.
pub(crate) const MANIFEST_HINT: &str =
    "saving the manifest reloads every prefab and data file together";

/// Loads the manifest's data entries and prefabs in two phases
/// (`StagedLoad`): every data file by kind, then ID, and every prefab file
/// by ID, read and parsed; then all of them put in place over the World's
/// registries, and every entry loaded from a file checked against `world`,
/// so a prefab or a data entry may name any other. The checks read
/// `world`, so the particle configs and tilemaps the components name load
/// first ([`load_all`](super::load_all) runs this after them), and the
/// plugins that register data kinds build before it. The first failure is
/// an error naming the prefab or the entry, and the registries keep what
/// they held.
///
/// A game's tests, which load no manifest, call it on the harness's World
/// to seed the prefabs and data entries.
pub fn load_prefabs(manifest: &ResolvedManifest, world: &mut World) -> anyhow::Result<()> {
    let mut load = StagedLoad::new();
    let staged = stage(manifest, world, &mut load).map_err(|e| anyhow::anyhow!(e))?;
    load.commit(world)
        .map_err(|e| anyhow::anyhow!(failure(&e)))?;
    for (name, path) in staged {
        log::info!("Loaded {name} from '{}'", path.display());
    }
    Ok(())
}

/// Hot-reloads prefab `id` from `path` for later spawns, in one commit that
/// checks every prefab and data entry loaded from a file; a file that fails
/// its load or a check that fails keeps the previous prefab and logs why.
/// Live entities stay as they are.
pub fn reload_prefab(id: &str, path: &Path, world: &mut World) -> anyhow::Result<()> {
    let mut load = StagedLoad::new();
    if let Err(e) = load.add_prefab(id, path) {
        log::error!("Hot reload prefab '{id}': {e}; keeping the previous prefab");
        return Ok(());
    }
    if let Err(e) = load.commit(world) {
        log::error!(
            "Hot reload prefab '{id}': {}; keeping the previous prefab ({MANIFEST_HINT})",
            failure(&e)
        );
        return Ok(());
    }
    log::info!("Hot-reloaded prefab '{id}'");
    Ok(())
}

/// Hot-reloads data entry `id` of kind `kind` from `path` for later reads,
/// as [`reload_prefab`] does a prefab: a file that fails its load or a
/// check that fails keeps the previous entry and logs why. A reader that
/// copied the entry keeps its copy.
pub fn reload_data(kind: &str, id: &str, path: &Path, world: &mut World) -> anyhow::Result<()> {
    let mut load = StagedLoad::new();
    if let Err(e) = load.add_data(world, kind, id, path) {
        log::error!("Hot reload data `{kind}` entry `{id}`: {e}; keeping the previous entry");
        return Ok(());
    }
    if let Err(e) = load.commit(world) {
        log::error!(
            "Hot reload data `{kind}` entry `{id}`: {}; keeping the previous entry ({MANIFEST_HINT})",
            failure(&e)
        );
        return Ok(());
    }
    log::info!("Hot-reloaded data `{kind}` entry `{id}`");
    Ok(())
}

/// The manifest reload's block, after the new tilemaps and particle
/// configs: every prefab and data entry of `new_manifest` is staged, each
/// file read again, so new, moved and edited files load together, and
/// committed once; a failure logs and keeps every previous entry, and a
/// removed entry warns and stays. Then the objects of each map the reload
/// added (`new_maps`) are checked against the prefabs now loaded; a failure
/// logs the error, the map keeps its tiles, and `World::spawn_map_objects`
/// returns the same error.
pub(crate) fn reload_prefabs_and_data(
    new_manifest: &ResolvedManifest,
    world: &mut World,
    new_maps: &[String],
) {
    warn_removed(new_manifest, world);
    let before: Vec<String> = world
        .get_resource::<PrefabRegistry>()
        .map(|registry| registry.ids().map(ToString::to_string).collect())
        .unwrap_or_default();
    let mut load = StagedLoad::new();
    let committed = stage(new_manifest, world, &mut load)
        .and_then(|staged| load.commit(world).map(|()| staged).map_err(|e| failure(&e)));
    match committed {
        Ok(staged) => {
            let mut ids: Vec<&String> = new_manifest.prefabs.keys().collect();
            ids.sort();
            for id in ids.into_iter().filter(|id| !before.contains(id)) {
                log::info!("Manifest reload: loaded new prefab '{id}'");
            }
            log::info!(
                "Manifest reload: {} prefab and data files loaded",
                staged.len()
            );
        }
        Err(e) => {
            log::error!("Manifest reload: prefabs and data keep their previous entries: {e}");
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

/// Warns for each prefab the World holds and each data entry the loaded
/// manifest listed that `new_manifest` no longer lists; they stay.
fn warn_removed(new_manifest: &ResolvedManifest, world: &World) {
    if let Some(registry) = world.get_resource::<PrefabRegistry>() {
        for id in registry.ids() {
            if !new_manifest.prefabs.contains_key(id) {
                log::warn!("Manifest reload: prefab '{id}' removed — keeping stale");
            }
        }
    }
    if let Some(loaded) = world.get_resource::<LoadedManifest>() {
        for (kind, entries) in &loaded.as_resolved().data {
            for id in entries.keys() {
                let kept = new_manifest
                    .data
                    .get(kind)
                    .is_some_and(|entries| entries.contains_key(id));
                if !kept {
                    log::warn!(
                        "Manifest reload: data `{kind}` entry `{id}` removed — keeping stale"
                    );
                }
            }
        }
    }
}

/// Phase one over `manifest`: every data entry by kind, then ID, then every
/// prefab by ID. The staged entries' names and files, for the log, or the
/// first failure's message.
fn stage(
    manifest: &ResolvedManifest,
    world: &World,
    load: &mut StagedLoad,
) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    let mut staged = Vec::new();
    for (kind, entries) in &manifest.data {
        for (id, entry) in entries {
            load.add_data(world, kind, id.clone(), entry.path.clone())
                .map_err(|e| format!("Failed to load data `{kind}` entry `{id}`: {e}"))?;
            staged.push((format!("data `{kind}` entry `{id}`"), entry.path.clone()));
        }
    }
    let mut ids: Vec<&String> = manifest.prefabs.keys().collect();
    ids.sort();
    for id in ids {
        let entry = &manifest.prefabs[id];
        load.add_prefab(id.clone(), entry.path.clone())
            .map_err(|e| format!("Failed to load prefab '{id}': {e}"))?;
        staged.push((format!("prefab '{id}'"), entry.path.clone()));
    }
    Ok(staged)
}

/// A failed commit as the load's error reads: the prefab or the entry that
/// failed, then why.
pub(crate) fn failure(err: &StagedLoadError) -> String {
    match err {
        StagedLoadError::Prefab { id, source } => format!("Failed to load prefab '{id}': {source}"),
        StagedLoadError::Data { kind, id, source } => {
            format!("Failed to load data `{kind}` entry `{id}`: {source}")
        }
        other => format!("Failed to load prefabs and data: {other}"),
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

#[cfg(test)]
#[path = "../tests/prefabs.rs"]
mod tests;
