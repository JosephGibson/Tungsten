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
//! `ui`, the names a screen file may use that the game registers, by kind
//! (redo 7.20): the bindings, row providers, mounts and actions its
//! `UiBindings` holds and the screens it registered by name; `screens`,
//! each `ui_screens` file's `id`, `file`, `keys` and the names it `uses` by
//! kind; `prefabs`, each loaded prefab resolved (redo 7.12), as `{name,
//! extends, prefab}`: its ID, the base it extends or `null`, and its fields
//! with a variant's base under them; and `errors`, each `{file, message}`
//! the load found.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tungsten_core::assets::ResolvedManifest;
use tungsten_core::script::ScriptHooks;
use tungsten_core::ui::{ScreenNames, UiBindings, UiScreenRegistry};
use tungsten_core::{ComponentRegistry, PrefabRegistry, World};

use crate::asset_loader::check::check_files;
use crate::screens::ScreenRegistry;

/// The description's format version; a field added or changed bumps it.
pub(crate) const DESCRIBE_VERSION: u32 = 3;

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
        "ui": ui(world),
        "screens": screens(world, &manifest, game_dir),
        "prefabs": prefabs(world),
        "errors": errors,
    })
}

/// Each loaded prefab by ID, resolved: its name, the base it extends and its
/// fields, `extends` given beside them.
fn prefabs(world: &World) -> Vec<Value> {
    let Some(registry) = world.get_resource::<PrefabRegistry>() else {
        return Vec::new();
    };
    registry
        .ids()
        .filter_map(|id| {
            let data = registry.get(id)?;
            let mut prefab = serde_json::to_value(data).ok()?;
            if let Value::Object(fields) = &mut prefab {
                fields.remove("extends");
            }
            Some(json!({ "name": id, "extends": data.extends, "prefab": prefab }))
        })
        .collect()
}

/// The names a screen file may use that the game registers, by kind.
fn ui(world: &World) -> Value {
    let names = world
        .get_resource::<UiBindings>()
        .map(UiBindings::names)
        .unwrap_or_default();
    let mut registered = world
        .get_resource::<ScreenRegistry>()
        .map(ScreenRegistry::ids)
        .unwrap_or_default();
    registered.sort_unstable();
    json!({
        "text": names.text,
        "value": names.value,
        "fill": names.fill,
        "visible": names.visible,
        "enabled": names.enabled,
        "rows": names.rows,
        "mounts": names.mounts,
        "actions": names.actions,
        "screens": registered,
    })
}

/// Each loaded screen file: its ID, its file relative to `game_dir`, its
/// keys and the names it uses by kind.
fn screens(world: &World, manifest: &ResolvedManifest, game_dir: &Path) -> Vec<Value> {
    let Some(registry) = world.get_resource::<UiScreenRegistry>() else {
        return Vec::new();
    };
    let relative = |path: &Path| {
        let canonical = game_dir
            .canonicalize()
            .unwrap_or_else(|_| game_dir.to_path_buf());
        let path = path
            .strip_prefix(&canonical)
            .or_else(|_| path.strip_prefix(game_dir))
            .unwrap_or(path);
        let parts: Vec<String> = path
            .components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect();
        parts.join("/")
    };
    registry
        .ids()
        .into_iter()
        .filter_map(|id| {
            let file = registry.get(id)?;
            let path = manifest
                .ui_screens
                .get(id)
                .map(|entry| relative(&entry.path));
            Some(json!({
                "id": id,
                "file": path,
                "keys": file.keys(),
                "uses": names_json(&file.uses()),
            }))
        })
        .collect()
}

/// `names` by kind, each sorted; `refused` left out.
fn names_json(names: &ScreenNames) -> Value {
    json!({
        "text": names.text,
        "value": names.value,
        "fill": names.fill,
        "visible": names.visible,
        "enabled": names.enabled,
        "rows": names.rows,
        "mounts": names.mounts,
        "actions": names.actions,
        "styles": names.styles,
        "sizes": names.sizes,
        "screens": names.screens,
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
        "ui_screens": ids(&manifest.ui_screens),
        "prefabs": ids(&manifest.prefabs),
        "data": data,
    })
}

#[cfg(test)]
#[path = "tests/describe.rs"]
mod tests;
