//! The coverage test: every scene name and data kind the kit registers
//! appears in at least two genres' fixtures, and no fixture names one the
//! registries lack. The fixtures are read through the engine's loader on a
//! recipe harness, so each one loads with every check first.

use std::collections::{BTreeMap, BTreeSet};

use tungsten_core::{ComponentRegistry, DataRegistry, PrefabRegistry, Schedule, World};

use super::{DTS, GENRES, recipe};
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
    assert_eq!(registered.len(), 32);
    assert_eq!(kinds.len(), 3);
}
