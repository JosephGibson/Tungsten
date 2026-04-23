use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::{accept_path, canonical_or_clone, DEBOUNCE_MS};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_hotreload_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn touch(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::File::create(path).unwrap();
}

#[test]
fn debounce_constant_is_50ms() {
    assert_eq!(DEBOUNCE_MS, 50);
}

#[test]
fn accept_extra_file_matches_exact_path() {
    let dir = tempdir();
    let input_json = dir.join("input.json");
    touch(&input_json);

    let mut extras = HashSet::new();
    extras.insert(canonical_or_clone(&input_json));
    let roots: Vec<PathBuf> = Vec::new();

    assert!(accept_path(&input_json, &roots, &extras));
}

#[test]
fn accept_recursive_root_matches_nested_file() {
    let dir = tempdir();
    let assets = dir.join("assets");
    fs::create_dir_all(&assets).unwrap();
    let nested = assets.join("sprites").join("hero.png");
    touch(&nested);

    let roots = vec![canonical_or_clone(&assets)];
    let extras = HashSet::new();

    assert!(
        accept_path(&nested, &roots, &extras),
        "files under a recursive root must be accepted"
    );
}

#[test]
fn reject_sibling_file_in_extra_file_parent() {
    // Extra-file watch is exact, not parent-wide.
    let dir = tempdir();
    let input_json = dir.join("input.json");
    let tungsten_json = dir.join("tungsten.json");
    touch(&input_json);
    touch(&tungsten_json);

    let mut extras = HashSet::new();
    extras.insert(canonical_or_clone(&input_json));
    let roots: Vec<PathBuf> = Vec::new();

    assert!(
        !accept_path(&tungsten_json, &roots, &extras),
        "sibling of an extra-file entry must be filtered out"
    );
}

#[test]
fn reject_parent_directory_of_recursive_root() {
    // Recursive root match is directional.
    let dir = tempdir();
    let assets = dir.join("assets");
    fs::create_dir_all(&assets).unwrap();
    let outside = dir.join("other.txt");
    touch(&outside);

    let roots = vec![canonical_or_clone(&assets)];
    let extras = HashSet::new();

    assert!(
        !accept_path(&outside, &roots, &extras),
        "a sibling of the watched root must not match"
    );
}
