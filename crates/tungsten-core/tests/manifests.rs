//! Repo-wide manifest validation: every `manifest.json` in the workspace must
//! load without error. Catches broken relative paths, missing asset files,
//! malformed JSON, and duplicate-ID collisions before they reach runtime.
//!
//! No GPU or display required — this runs as part of `cargo test --workspace`.

use std::path::{Path, PathBuf};
use tungsten_core::assets::manifest::ResolvedManifest;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for tungsten-core is `<root>/crates/tungsten-core`.
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

/// Verifies that asset IDs are globally unique across all manifests in the
/// workspace (D-017, D-035). Each manifest is loaded individually first so
/// path resolution errors don't contaminate the merge step.
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
