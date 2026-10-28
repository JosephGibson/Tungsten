//! Repo-wide manifest validation; no GPU/display.

use std::path::{Path, PathBuf};
use tungsten_core::PrefabData;
use tungsten_core::assets::manifest::ResolvedManifest;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root should be two levels above crates/tungsten-core")
        .to_path_buf()
}

fn collect_manifests(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();

    let root_manifest = root.join("assets").join("manifest.json");
    if root_manifest.exists() {
        out.push(root_manifest);
    }

    let examples_dir = root.join("examples");
    if let Ok(entries) = std::fs::read_dir(&examples_dir) {
        let mut example_paths: Vec<PathBuf> = entries
            .flatten()
            .filter(|e| !e.path().join("tungsten.json").exists())
            .map(|e| e.path().join("assets").join("manifest.json"))
            .filter(|p| p.exists())
            .collect();
        example_paths.sort();
        out.extend(example_paths);
    }

    out
}

/// Each `templates/*/assets/manifest.json`, and each example's whose folder
/// holds `tungsten.json`: a game run from its own folder loads its manifest
/// as its only root, so each one loads and resolves on its own, outside the
/// examples' uniqueness merge (`D-123`).
fn collect_own_root_manifests(root: &Path) -> Vec<PathBuf> {
    let folder_games = ["templates", "examples"].into_iter().flat_map(|parent| {
        std::fs::read_dir(root.join(parent))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(move |path| parent == "templates" || path.join("tungsten.json").exists())
    });
    let mut out: Vec<PathBuf> = folder_games
        .map(|path| path.join("assets").join("manifest.json"))
        .filter(|p| p.exists())
        .collect();
    out.sort();
    out
}

#[test]
fn each_template_manifest_loads_as_its_own_root() {
    let manifests = collect_own_root_manifests(&workspace_root());
    assert!(
        manifests
            .iter()
            .any(|path| path.starts_with(workspace_root().join("templates"))),
        "no template manifest under templates/ — test is broken"
    );
    for manifest in &manifests {
        if let Err(e) = ResolvedManifest::load_and_merge_many(&[manifest]) {
            panic!("{}: {e:?}", manifest.display());
        }
    }
}

#[test]
fn all_manifests_load() {
    let root = workspace_root();
    let manifests = collect_manifests(&root);
    assert!(
        !manifests.is_empty(),
        "no manifests discovered under {} — test is broken",
        root.display()
    );

    let mut failures = Vec::new();
    for manifest in &manifests {
        match ResolvedManifest::load(manifest) {
            Ok(_) => {}
            Err(e) => failures.push(format!("{}: {e:?}", manifest.display())),
        }
    }

    assert!(
        failures.is_empty(),
        "{} manifest(s) failed to load:\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
}

/// D-017/D-035: asset IDs globally unique across loaded manifests.
#[test]
fn all_manifest_ids_are_globally_unique() {
    let root = workspace_root();
    let manifests = collect_manifests(&root);
    assert!(
        !manifests.is_empty(),
        "no manifests discovered under {} — test is broken",
        root.display()
    );

    let mut merged = ResolvedManifest::default();
    for manifest_path in &manifests {
        let loaded = ResolvedManifest::load(manifest_path).unwrap_or_else(|e| {
            panic!(
                "manifest failed to load (run all_manifests_load for details): {}: {e:?}",
                manifest_path.display()
            )
        });
        if let Err(e) = merged.merge(loaded) {
            panic!(
                "duplicate asset ID detected across manifests — {} introduced a collision: {e:?}",
                manifest_path.display()
            );
        }
    }
}

/// Every `prefabs` entry of every example and template manifest parses as a
/// prefab file: its keys, its sprite and tween, and its components' shape
/// as JSON. The components' types and asset IDs need the game's World,
/// which the game's own load checks.
#[test]
fn every_prefab_file_parses() {
    let root = workspace_root();
    let mut manifests = collect_manifests(&root);
    manifests.extend(collect_own_root_manifests(&root));
    let mut failures = Vec::new();
    for manifest in &manifests {
        let loaded = ResolvedManifest::load(manifest)
            .unwrap_or_else(|e| panic!("{}: {e:?}", manifest.display()));
        let mut prefabs: Vec<_> = loaded.prefabs.iter().collect();
        prefabs.sort_by(|a, b| a.0.cmp(b.0));
        for (id, prefab) in prefabs {
            let parsed = std::fs::read_to_string(&prefab.path)
                .map_err(|e| e.to_string())
                .and_then(|text| {
                    serde_json::from_str::<PrefabData>(&text).map_err(|e| e.to_string())
                });
            if let Err(e) = parsed {
                failures.push(format!(
                    "{}: prefab '{id}' ({}): {e}",
                    manifest.display(),
                    prefab.path.display()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} prefab file(s) failed to parse:\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
}

/// Every `data` entry's file of every example and template manifest parses
/// as JSON (M61). Its kind's type needs the game's plugins, which the
/// game's own load checks.
#[test]
fn every_data_file_parses() {
    let root = workspace_root();
    let mut manifests = collect_manifests(&root);
    manifests.extend(collect_own_root_manifests(&root));
    let mut failures = Vec::new();
    for manifest in &manifests {
        let loaded = ResolvedManifest::load(manifest)
            .unwrap_or_else(|e| panic!("{}: {e:?}", manifest.display()));
        for (kind, entries) in &loaded.data {
            for (id, entry) in entries {
                let parsed = std::fs::read_to_string(&entry.path)
                    .map_err(|e| e.to_string())
                    .and_then(|text| {
                        serde_json::from_str::<serde_json::Value>(&text).map_err(|e| e.to_string())
                    });
                if let Err(e) = parsed {
                    failures.push(format!(
                        "{}: data `{kind}` entry '{id}' ({}): {e}",
                        manifest.display(),
                        entry.path.display()
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} data file(s) failed to parse:\n  - {}",
        failures.len(),
        failures.join("\n  - ")
    );
}
