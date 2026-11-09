//! Rooms and tilesets from manifests: the load plan's units, the check and
//! the fixture under `tests/fixtures/rooms/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use glam::Vec2;
use tungsten_core::assets::{LayerPhysics, PlaceShape, RoomRegistry, TilemapRegistry};
use tungsten_core::{Config, LayerKind};

use super::*;
use crate::App;
use crate::asset_loader::check::check_files;
use crate::asset_loader::{HeadlessTarget, LoadPlan, LoadUnit};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// What a spawn's `with` gives.
#[derive(Debug, Clone, Copy)]
struct Marker;

/// The fixture: the `cave` tileset, rooms `cave` and `hall`, and the
/// prefabs `crate`, `lamp`, `ball` and `walker`.
pub(crate) fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rooms")
}

/// A copy of the fixture in a temp folder of its own, canonical.
pub(crate) fn fixture_copy() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_rooms_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    copy(&fixture(), &dir);
    dir.canonicalize().unwrap()
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// An app's World, the engine's components registered.
fn app_world() -> World {
    let mut app = App::new(Config::default()).unwrap();
    std::mem::take(app.world_mut())
}

/// `manifest` loaded through every unit of its plan, headless.
fn load(manifest: &ResolvedManifest, world: &mut World) -> Vec<LoadUnit> {
    let mut plan = LoadPlan::new(manifest.clone(), world);
    let units = plan.units().to_vec();
    let mut target = HeadlessTarget::default();
    while plan.run_next_into(world, &mut target).unwrap() {}
    units
}

#[test]
fn the_fixtures_rooms_load_after_the_physics_names() {
    let manifest = ResolvedManifest::load(fixture().join("manifest.json")).unwrap();
    let mut world = app_world();
    let units = load(&manifest, &mut world);
    let at = |unit: &LoadUnit| units.iter().position(|u| u == unit).unwrap();
    let names = at(&LoadUnit::PhysicsNames);
    assert_eq!(
        units[names + 1..names + 4],
        [
            LoadUnit::Tileset("cave".into()),
            LoadUnit::Room("cave".into()),
            LoadUnit::Room("hall".into())
        ],
        "after the physics names"
    );
    assert_eq!(at(&LoadUnit::Prefabs), names + 4, "before the prefabs");

    let rooms = world.resource::<RoomRegistry>();
    assert_eq!(rooms.ids().collect::<Vec<_>>(), ["cave", "hall"]);
    assert_eq!(rooms.tileset_ids().collect::<Vec<_>>(), ["cave"]);
    assert_eq!(
        rooms.place("cave", "exit").unwrap().to.as_deref(),
        Some("hall.west_door")
    );
    let map = world.resource::<TilemapRegistry>().get("cave").unwrap();
    assert_eq!(map.layers[1].kind, LayerKind::Collision);
    // `terrain` is layer 1, `player` 2 and `enemy` 3.
    assert_eq!(
        map.layer_physics(1),
        LayerPhysics {
            layers: 1 << 1,
            mask: (1 << 1) | (1 << 2) | (1 << 3)
        }
    );
    assert!(map.tile_props(5).whole, "the arch is drawn whole");
    let spawned = world.spawn_room("cave", [0.0, 0.0], &(Marker,)).unwrap();
    assert_eq!(spawned.objects().count(), 4);
    assert_eq!(
        spawned.place("player_start").unwrap().shape,
        PlaceShape::Point(Vec2::new(32.0, 304.0))
    );
}

#[test]
fn a_manifest_without_rooms_inserts_no_room_registry() {
    let dir = fixture_copy();
    let path = dir.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    let object = manifest.as_object_mut().unwrap();
    object.remove("rooms");
    object.remove("tilesets");
    fs::write(&path, manifest.to_string()).unwrap();
    let manifest = ResolvedManifest::load(&path).unwrap();
    let mut world = app_world();
    let units = load(&manifest, &mut world);
    assert!(
        !units
            .iter()
            .any(|unit| matches!(unit, LoadUnit::Tileset(_) | LoadUnit::Room(_)))
    );
    assert!(!world.has_resource::<RoomRegistry>());
    let _ = fs::remove_dir_all(&dir);
}

/// Every error `check_files` finds in the fixture copy `dir`, as
/// `<file>: <message>`.
fn check(dir: &Path) -> Vec<String> {
    let mut app = App::new(Config::default()).unwrap();
    check_files(app.world_mut(), &[dir.join("manifest.json")], dir)
        .into_iter()
        .map(|error| format!("{}: {}", error.file.display(), error.message))
        .collect()
}

#[test]
fn a_rooms_check_errors_name_its_file_room_id_and_field() {
    let dir = fixture_copy();
    let cave = dir.join("rooms/cave.json");
    let text = fs::read_to_string(&cave).unwrap();
    fs::write(
        &cave,
        text.replace(
            r#""tile": "rock", "layers": ["art", "solid"] },
    { "id": "ramp""#,
            r#""tile": "rock", "layers": ["art", "sold"] },
    { "id": "ramp""#,
        ),
    )
    .unwrap();
    let hall = dir.join("rooms/hall.json");
    let text = fs::read_to_string(&hall).unwrap();
    fs::write(
        &hall,
        text.replace(r#""point": [1, 9]"#, r#""point": [1, 99]"#),
    )
    .unwrap();
    let tileset = dir.join("tilesets/cave.json");
    let text = fs::read_to_string(&tileset).unwrap();
    fs::write(
        &tileset,
        text.replace(r#""material": "wood""#, r#""material": """#),
    )
    .unwrap();
    assert_eq!(
        check(&dir),
        [
            "tilesets/cave.json: tileset 'cave': tiles[4]: `material` must not be empty",
            "rooms/cave.json: room 'cave': shapes[0].layers[1]: no layer `sold`; did you mean `solid`?",
            "rooms/hall.json: room 'hall': places[0].point: [1, 99] is past the room's 20 \u{d7} 12 cells",
        ]
    );
    // With the room's own error fixed, the tileset it names is missing.
    fs::write(
        &cave,
        fs::read_to_string(fixture().join("rooms/cave.json")).unwrap(),
    )
    .unwrap();
    let errors = check(&dir);
    assert!(
        errors.contains(&"rooms/cave.json: room 'cave': tileset: no tileset `cave`".to_owned()),
        "{errors:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn room_and_tileset_files_are_not_checked_as_scenes() {
    let dir = fixture_copy();
    assert_eq!(check(&dir), Vec::<String>::new());
    // A room's object naming no prefab is the room's error, not a scene's.
    let cave = dir.join("rooms/cave.json");
    let text = fs::read_to_string(&cave).unwrap();
    fs::write(
        &cave,
        text.replace(r#""prefab": "lamp""#, r#""prefab": "lantern""#),
    )
    .unwrap();
    let errors = check(&dir);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].starts_with("rooms/cave.json: room 'cave': tilemap `cave`, layer `objects`, object 2: unknown prefab `lantern`"),
        "{errors:?}"
    );
    let _ = fs::remove_dir_all(&dir);
}

/// A game folder holding the rooms fixture under `rooms/` and the rules
/// fixture, one room a finding kind, under `room_rules/`.
fn rules_copy() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_room_rules_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    copy(&fixture(), &dir.join("rooms"));
    copy(&fixture().join("../room_rules"), &dir.join("room_rules"));
    dir.canonicalize().unwrap()
}

/// Every error `check_files` finds over `roots`, the kit's components
/// registered, as `<file>: <message>`.
fn check_with_kit(dir: &Path, roots: &[&str]) -> Vec<String> {
    let mut app = App::new(Config::default()).unwrap();
    #[cfg(feature = "kit")]
    app.add_plugins(tungsten_kit::KitPlugins::set());
    let roots: Vec<PathBuf> = roots.iter().map(|root| dir.join(root)).collect();
    check_files(app.world_mut(), &roots, dir)
        .into_iter()
        .map(|error| format!("{}: {}", error.file.display(), error.message))
        .collect()
}

#[test]
fn tungsten_check_reports_each_rules_finding_by_room_id_and_path() {
    let dir = rules_copy();
    let found = check_with_kit(&dir, &["rooms/manifest.json", "room_rules/manifest.json"]);
    let mut expected = vec![
        "room_rules/lamp_crowd.json: lamp_crowd: lamp_2: objects[1]: budget: 2 objects hold a light, past budget.lights 1",
        "room_rules/link_lost.json: link_lost: door: places[0].to: links to `hall.nowhere`, which no room holds",
        "room_rules/prop_float.json: prop_float: lamp_1: objects[0]: floats: 0 of 8 bottom columns have ground within 2 px",
        "room_rules/spawn_stuck.json: spawn_stuck: crate_1: objects[0]: spawns inside the terrain: its collider overlaps cell (2, 5)",
    ];
    if cfg!(feature = "kit") {
        expected.insert(
            3,
            "room_rules/route_gap.json: route_gap: main: places[1].route[1]: out of reach of route[0] for `runner`",
        );
    }
    assert_eq!(found, expected);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn the_fixtures_clean_rooms_have_no_findings() {
    let dir = rules_copy();
    assert_eq!(
        check_with_kit(&dir, &["rooms/manifest.json"]),
        Vec::<String>::new()
    );
    // With the rules rooms beside them, the clean rooms still find nothing.
    let found = check_with_kit(&dir, &["rooms/manifest.json", "room_rules/manifest.json"]);
    assert!(
        found.iter().all(|line| line.starts_with("room_rules/")),
        "{found:?}"
    );
    // Each rule of the clean rooms runs on its prop, crate and links.
    let manifest = ResolvedManifest::load(dir.join("rooms/manifest.json")).unwrap();
    let mut world = app_world();
    load(&manifest, &mut world);
    for room in ["cave", "hall"] {
        assert_eq!(world.room_findings(room, &["light"]), []);
    }
    let _ = fs::remove_dir_all(&dir);
}
