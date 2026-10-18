//! The coverage test: every scene name and data kind the kit registers
//! appears in at least two genres' fixtures, and no fixture names one the
//! registries lack. The fixtures are read through the engine's loader on a
//! recipe harness, so each one loads with every check first. The
//! genericity check: no kit source names a damage type the fixtures
//! declare.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use tungsten_core::{ComponentRegistry, DataRegistry, PrefabRegistry, Schedule, World};

use super::{DTS, GENRES, fixture_dir, recipe};
use crate::kit::KitPlugins;

/// The scene names a fixture may use that the kit does not register.
const ENGINE_NAMES: [&str; 1] = ["rigid_body"];

/// The scene names a genre's prefabs use, and the data kinds its manifest
/// lists, from the registries the fixture loads into.
fn genre_names(genre: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let harness = recipe(genre, KitPlugins::set(), DTS[0]);
    let world = harness.world();
    let prefabs = world.resource::<PrefabRegistry>();
    let names: BTreeSet<String> = prefabs
        .ids()
        .flat_map(|id| prefabs.get(id).unwrap().components.keys().cloned())
        .collect();
    let data = world.resource::<DataRegistry>();
    let kinds: BTreeSet<String> = data
        .kinds()
        .filter(|kind| data.ids(kind).next().is_some())
        .map(str::to_owned)
        .collect();
    (names, kinds)
}

#[test]
fn every_registered_scene_name_and_kind_appears_in_two_genres() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    KitPlugins::set().build(&mut schedule, &mut world);
    let registered: BTreeSet<String> = world
        .resource::<ComponentRegistry>()
        .names()
        .map(str::to_owned)
        .collect();
    let kinds: BTreeSet<String> = world
        .resource::<DataRegistry>()
        .kinds()
        .map(str::to_owned)
        .collect();

    let mut name_genres: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut kind_genres: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for genre in GENRES {
        let (names, used_kinds) = genre_names(genre);
        for name in names {
            assert!(
                registered.contains(&name) || ENGINE_NAMES.contains(&name.as_str()),
                "{genre} names `{name}`, which nothing registers"
            );
            name_genres.entry(name).or_default().push(genre);
        }
        for kind in used_kinds {
            assert!(kinds.contains(&kind), "{genre} lists kind `{kind}`");
            kind_genres.entry(kind).or_default().push(genre);
        }
    }
    let mut short = Vec::new();
    for name in &registered {
        let genres = name_genres.get(name).map_or(0, Vec::len);
        if genres < 2 {
            short.push(format!("`{name}` in {genres} genre(s)"));
        }
    }
    for kind in &kinds {
        let genres = kind_genres.get(kind).map_or(0, Vec::len);
        if genres < 2 {
            short.push(format!("kind `{kind}` in {genres} genre(s)"));
        }
    }
    assert!(short.is_empty(), "under two genres: {}", short.join(", "));
    assert_eq!(registered.len(), 45);
    assert_eq!(kinds.len(), 9);
}

/// The damage types every fixture's `damage_types` entries declare.
fn fixture_damage_types() -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for genre in GENRES {
        let dir = fixture_dir(genre).join("data/damage_types");
        let Ok(files) = std::fs::read_dir(&dir) else {
            continue;
        };
        for file in files {
            let text = std::fs::read_to_string(file.unwrap().path()).unwrap();
            names.extend(top_level_keys(&text));
        }
    }
    names
}

/// The keys of the JSON object `text`: each string at its top depth that a
/// `:` follows. A `damage_types` entry is an object keyed by type, and the
/// umbrella reads it with no JSON crate.
fn top_level_keys(text: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut depth = 0_u32;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '[' => depth += 1,
            '}' | ']' => depth = depth.saturating_sub(1),
            '"' => {
                let mut string = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => string.extend(chars.next()),
                        '"' => break,
                        other => string.push(other),
                    }
                }
                while chars.next_if(|c| c.is_whitespace()).is_some() {}
                if depth == 1 && chars.peek() == Some(&':') {
                    keys.push(string);
                }
            }
            _ => {}
        }
    }
    keys
}

/// The `(line, name)` of each of `names` that `source` names as a whole
/// token, not inside a dotted name or a longer word, on a line that is no
/// doc comment.
fn named_types(source: &str, names: &BTreeSet<String>) -> Vec<(usize, String)> {
    let inside = |byte: u8| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'.'
    };
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("///") || code.starts_with("//!") {
            continue;
        }
        for name in names {
            let bytes = line.as_bytes();
            let hit = line.match_indices(name.as_str()).any(|(at, _)| {
                let before = at.checked_sub(1).map(|b| bytes[b]);
                let after = bytes.get(at + name.len()).copied();
                !before.is_some_and(inside) && !after.is_some_and(inside)
            });
            if hit {
                found.push((i + 1, name.clone()));
            }
        }
    }
    found
}

/// Every `.rs` file under `dir`, `tests` folders left out.
fn kit_sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "tests") {
                kit_sources(&path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_kit_source_names_a_fixture_damage_type() {
    let names = fixture_damage_types();
    assert_eq!(names.len(), 19, "the fantasy and sci-fi sets: {names:?}");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tungsten-kit/src");
    let mut files = Vec::new();
    kit_sources(&src, &mut files);
    files.sort();
    assert!(files.len() > 50, "the kit's sources: {}", files.len());
    let mut named = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).unwrap();
        for (line, name) in named_types(&source, &names) {
            named.push(format!("{}:{line} `{name}`", file.display()));
        }
    }
    assert!(
        named.is_empty(),
        "the kit names a game's damage type: {named:?}"
    );
}

#[test]
fn the_damage_type_scan_finds_a_planted_name() {
    let names: BTreeSet<String> = ["physical", "physical.slash"].map(str::to_owned).into();
    let planted = "fn f() {\n    let t = \"physical.slash\";\n}\n";
    assert_eq!(
        named_types(planted, &names),
        [(2, "physical.slash".to_string())]
    );
    let documented = "/// let t = \"physical.slash\";\n//! physical pixels\n";
    assert!(
        named_types(documented, &names).is_empty(),
        "doc comments are left out"
    );
    let longer = "let t = \"physicality\"; let u = \"meta.physical\";\n";
    assert!(named_types(longer, &names).is_empty(), "whole tokens only");
}
