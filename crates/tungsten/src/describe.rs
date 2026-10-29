//! `tungsten describe`'s engine side (redo 7.2). With `TUNGSTEN_DESCRIBE`
//! naming a file, `App::new` treats the run as a check run (no user folder,
//! warnings and errors only in the log) and `App::run` opens no window: it
//! resolves the schedule, loads every file the manifest roots name as
//! `tungsten check` does, past every error, and writes what the game's World
//! holds as JSON. The variable is the command line's, not a promised
//! interface.
//!
//! The JSON, at [`DESCRIBE_VERSION`]: `components`, each registered name
//! with the kit's description where the kit gives one (its `summary`,
//! `fields`, `events`, `genres`, `sample`); the kit's `shapes` and data
//! `kinds`; `script`, the hooks' `actions` and `probes`, each `{name,
//! doc}`; `assets`, the merged manifests' IDs by section, `data` by kind;
//! and `errors`, each `{file, message}` the load found.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tungsten_core::assets::ResolvedManifest;
use tungsten_core::script::ScriptHooks;
use tungsten_core::{ComponentRegistry, World};

use crate::asset_loader::check::check_files;

/// The description's format version; a field added or changed bumps it.
pub(crate) const DESCRIBE_VERSION: u32 = 1;

/// Loads the files `roots` name into `world`, as a check does, and
/// describes what it then holds; `game_dir` bounds every path.
pub(crate) fn describe(world: &mut World, roots: &[PathBuf], game_dir: &Path) -> Value {
    let errors: Vec<Value> = check_files(world, roots, game_dir)
        .into_iter()
        .map(|error| json!({ "file": error.file, "message": error.message }))
        .collect();
    let (manifest, _) = ResolvedManifest::load_for_check(roots, game_dir);
    let kit = kit_json(world);
    let described = |name: &str| {
        kit["components"]
            .as_array()
            .and_then(|all| all.iter().find(|d| d["name"] == name))
            .cloned()
    };
    let components: Vec<Value> = world
        .get_resource::<ComponentRegistry>()
        .map(|registry| {
            registry
                .names()
                .map(|name| described(name).unwrap_or_else(|| json!({ "name": name })))
                .collect()
        })
        .unwrap_or_default();
    let hooks = |list: Vec<(&str, &str)>| -> Vec<Value> {
        list.into_iter()
            .map(|(name, doc)| json!({ "name": name, "doc": doc }))
            .collect()
    };
    let (actions, probes) = world
        .get_resource::<ScriptHooks>()
        .map(|h| (hooks(h.actions().collect()), hooks(h.probes().collect())))
        .unwrap_or_default();
    json!({
        "version": DESCRIBE_VERSION,
        "components": components,
        "shapes": kit["shapes"].clone(),
        "kinds": kit["kinds"].clone(),
        "script": { "actions": actions, "probes": probes },
        "assets": assets(&manifest),
        "errors": errors,
    })
}

/// The kit's descriptions, or empty lists without them.
fn kit_json(world: &World) -> Value {
    #[cfg(feature = "kit")]
    if let Some(descriptions) = world.get_resource::<tungsten_kit::KitDescriptions>() {
        return descriptions.to_json();
    }
    let _ = world;
    json!({ "components": [], "shapes": [], "kinds": [] })
}

/// The manifest's IDs by section, each sorted.
fn assets(manifest: &ResolvedManifest) -> Value {
    fn ids<V>(map: &HashMap<String, V>) -> Vec<&str> {
        let mut ids: Vec<&str> = map.keys().map(String::as_str).collect();
        ids.sort_unstable();
        ids
    }
    let data: BTreeMap<&str, Vec<&str>> = manifest
        .data
        .iter()
        .map(|(kind, entries)| (kind.as_str(), entries.keys().map(String::as_str).collect()))
        .collect();
    json!({
        "sprites": ids(&manifest.sprites),
        "animations": ids(&manifest.animations),
        "fonts": ids(&manifest.fonts),
        "font_families": ids(&manifest.font_families),
        "sounds": ids(&manifest.sounds),
        "tilemaps": ids(&manifest.tilemaps),
        "particles": ids(&manifest.particles),
        "particle_meshes": ids(&manifest.particle_meshes),
        "shaders": ids(&manifest.shaders),
        "materials": ids(&manifest.materials),
        "ui_themes": ids(&manifest.ui_themes),
        "prefabs": ids(&manifest.prefabs),
        "data": data,
    })
}

#[cfg(test)]
#[path = "tests/describe.rs"]
mod tests;
