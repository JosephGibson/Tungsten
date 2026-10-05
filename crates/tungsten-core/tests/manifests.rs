//! Repo-wide manifest validation; no GPU/display.

use std::path::{Path, PathBuf};
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
            .map(|e| e.path().join("assets").join("manifest.json"))
            .filter(|p| p.exists())
            .collect();
        example_paths.sort();
        out.extend(example_paths);
    }

    out
}

/// Each `templates/*/assets/manifest.json`. A game loads its template's
/// manifest as its only root, so each one loads and resolves on its own,
/// outside the examples' uniqueness merge (`D-123`).
fn collect_template_manifests(root: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(root.join("templates"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("assets").join("manifest.json"))
        .filter(|p| p.exists())
        .collect();
    out.sort();
    out
}

#[test]
fn each_template_manifest_loads_as_its_own_root() {
    let manifests = collect_template_manifests(&workspace_root());
    assert!(
        !manifests.is_empty(),
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
