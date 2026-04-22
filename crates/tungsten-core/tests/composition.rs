//! Headless composition test for the D-052 asset-composition contract. Builds
//! two manifests under a temp dir, exercises `ResolvedManifest::load_and_merge_many`,
//! and confirms duplicate IDs halt the composition with `ManifestError::DuplicateId`.
//!
//! Runs under `cargo test --workspace` with no GPU or display.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use tungsten_core::assets::{ManifestError, ResolvedManifest};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_composition_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(dir: &Path, name: &str, bytes: &[u8]) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = fs::File::create(path).unwrap();
    f.write_all(bytes).unwrap();
}

fn write_manifest(dir: &Path, subdir: &str, contents: &str) -> PathBuf {
    let root = dir.join(subdir);
    fs::create_dir_all(&root).unwrap();
    let path = root.join("manifest.json");
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    path
}

/// Every asset class composes correctly when the two manifests are disjoint.
/// Verifies that `load_and_merge_many` produces one `ResolvedManifest` holding
/// the union of sprites, animations, fonts, sounds, tilemaps, and particles.
#[test]
fn merge_compose_two_manifests_disjoint_ids_all_types() {
    let dir = tempdir();

    // Root manifest: sprites, animations, fonts, sounds. Uses placeholder
    // binary content because the loader only checks file existence (JSON files
    // for animations/tilemaps/particles are not parsed by `ResolvedManifest`).
    write_file(&dir, "root/hero.png", b"\x89PNG stub");
    write_file(&dir, "root/walk.json", b"{}");
    write_file(&dir, "root/sans.ttf", b"ttf stub");
    write_file(&dir, "root/blip.ogg", b"ogg stub");
    let root_manifest = write_manifest(
        &dir,
        "root",
        r#"{
            "sprites": {
                "hero": {"path": "hero.png", "filter": "nearest"}
            },
            "animations": {
                "walk": {"path": "walk.json"}
            },
            "fonts": {
                "sans": {"path": "sans.ttf"}
            },
            "sounds": {
                "sfx_blip": {"path": "blip.ogg"}
            }
        }"#,
    );

    // Local manifest: tilemaps, particles, and one extra sprite.
    write_file(&dir, "local/tiles/grass.png", b"\x89PNG stub");
    write_file(&dir, "local/map.tmj", b"{}");
    write_file(&dir, "local/spark.json", b"{}");
    let local_manifest = write_manifest(
        &dir,
        "local",
        r#"{
            "sprites": {
                "grass": {"path": "tiles/grass.png", "filter": "nearest"}
            },
            "tilemaps": {
                "demo": {"path": "map.tmj"}
            },
            "particles": {
                "spark": {"path": "spark.json"}
            }
        }"#,
    );

    let merged = ResolvedManifest::load_and_merge_many(&[root_manifest, local_manifest])
        .expect("merged load should succeed when all IDs are disjoint");

    assert_eq!(merged.sprites.len(), 2, "sprites compose");
    assert!(merged.sprites.contains_key("hero"));
    assert!(merged.sprites.contains_key("grass"));
    assert!(merged.animations.contains_key("walk"));
    assert!(merged.fonts.contains_key("sans"));
    assert!(merged.sounds.contains_key("sfx_blip"));
    assert!(merged.tilemaps.contains_key("demo"));
    assert!(merged.particles.contains_key("spark"));
}

/// Duplicate IDs across manifests halt composition with `DuplicateId`
/// (D-017). Confirms the merge-first contract does not silently overwrite —
/// the previous loader shape where `world.insert_resource` replaced registries
/// wholesale is the thing this test guards against.
#[test]
fn merge_duplicate_id_across_manifests_is_fatal() {
    let dir = tempdir();

    write_file(&dir, "a/hero.png", b"stub");
    let a = write_manifest(&dir, "a", r#"{"sprites": {"hero": {"path": "hero.png"}}}"#);

    write_file(&dir, "b/hero.png", b"stub");
    let b = write_manifest(&dir, "b", r#"{"sprites": {"hero": {"path": "hero.png"}}}"#);

    let err = ResolvedManifest::load_and_merge_many(&[a, b])
        .expect_err("duplicate IDs across manifests must be fatal");

    assert!(
        matches!(&err, ManifestError::DuplicateId { id } if id == "hero"),
        "expected DuplicateId for 'hero', got: {err:?}"
    );
}

/// Empty roots list is valid and produces a default-empty manifest — matches
/// how `App::set_manifest_roots(vec![])` leaves composition to the user's
/// `on_startup` hook.
#[test]
fn merge_empty_roots_produces_empty_manifest() {
    let empty: &[PathBuf] = &[];
    let merged = ResolvedManifest::load_and_merge_many(empty).unwrap();
    assert!(merged.sprites.is_empty());
    assert!(merged.animations.is_empty());
    assert!(merged.fonts.is_empty());
    assert!(merged.sounds.is_empty());
    assert!(merged.tilemaps.is_empty());
    assert!(merged.particles.is_empty());
}
