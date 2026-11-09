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

mod reloads {
    use serde_json::{Value, json};
    use tungsten_core::assets::{RoomCommand, RoomJournal, TileChanges};
    use tungsten_core::save::{SaveId, SaveState};
    use tungsten_core::{Entity, Transform};

    use super::*;
    use crate::asset_loader::reload_json_files;
    use crate::state::SceneEntity;

    /// The fixture's copy loaded and room `cave` spawned at the origin
    /// under state `game`.
    fn live_cave() -> (PathBuf, World) {
        let dir = fixture_copy();
        let manifest = ResolvedManifest::load(dir.join("manifest.json")).unwrap();
        let mut world = app_world();
        load(&manifest, &mut world);
        world
            .spawn_room("cave", [0.0, 0.0], &(SceneEntity { state_id: "game" },))
            .unwrap();
        (dir, world)
    }

    /// The JSON file at `path` changed by `change`.
    fn edit(path: &Path, change: impl FnOnce(&mut Value)) {
        let mut value: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        change(&mut value);
        fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    }

    fn room_file(world: &World, id: &str) -> PathBuf {
        world
            .resource::<RoomRegistry>()
            .path(id)
            .unwrap()
            .to_path_buf()
    }

    fn tileset_file(world: &World) -> PathBuf {
        world
            .resource::<RoomRegistry>()
            .tileset_path("cave")
            .unwrap()
            .to_path_buf()
    }

    /// The live entity named `name`.
    fn named(world: &World, name: &str) -> Option<Entity> {
        world
            .get_resource::<SaveState>()?
            .named_entity(SaveId::authored(name))
            .filter(|&entity| world.is_alive(entity))
    }

    fn undo_len(world: &World, room: &str) -> usize {
        world
            .get_resource::<RoomJournal>()
            .map_or(0, |journal| journal.undo_len(room))
    }

    fn nudge(world: &mut World, room: &str, batch: &str) {
        let batch = RoomCommand::parse_batch(batch).unwrap();
        world.apply_room_batch(room, &batch).unwrap();
    }

    #[test]
    fn reloading_a_room_file_patches_the_live_room() {
        let (_dir, mut world) = live_cave();
        let ball = named(&world, "cave.ball_1").unwrap();
        let crate_1 = named(&world, "cave.crate_1").unwrap();
        let path = room_file(&world, "cave");
        edit(&path, |room| {
            room["objects"][2]["at"] = json!([32.5, 12]);
            room["layers"][0]["cells"]["3,3"] = json!("plank");
        });
        let errors = reload_json_files(&[path], &mut world, false);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(!world.is_alive(ball), "the moved ball spawned again");
        let again = named(&world, "cave.ball_1").unwrap();
        assert_eq!(
            world.get::<Transform>(again).unwrap().position,
            Vec2::new(520.0, 192.0)
        );
        assert_eq!(
            world.get::<SceneEntity>(again).map(|scene| scene.state_id),
            Some("game"),
            "the instance's state, by the spawn inheritance"
        );
        assert_eq!(named(&world, "cave.crate_1"), Some(crate_1));
        let map = world.resource::<TilemapRegistry>().get("cave").unwrap();
        assert_eq!(map.tile_at(0, 3, 3), Some(4), "the plank, in place");
    }

    #[test]
    fn a_room_failing_its_reload_keeps_the_live_room() {
        let (_dir, mut world) = live_cave();
        let ball = named(&world, "cave.ball_1").unwrap();
        let document = world.resource::<RoomRegistry>().room("cave").cloned();
        let cursor = world.resource::<TilemapRegistry>().cursor("cave").unwrap();
        let path = room_file(&world, "cave");
        edit(&path, |room| {
            room["objects"][2]["at"] = json!([32.5, 12]);
            room["shapes"][0]["tile"] = json!("rokc");
        });
        let errors = reload_json_files(std::slice::from_ref(&path), &mut world, false);
        assert_eq!(
            errors,
            [
                "Hot reload room 'cave': room `cave`: shapes[0].tile: no tile `rokc` in the tileset; keeping the live room"
            ]
        );
        fs::write(&path, "{ not json").unwrap();
        let errors = reload_json_files(&[path], &mut world, false);
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].starts_with("Hot reload room 'cave': "),
            "{errors:?}"
        );
        assert_eq!(
            world.resource::<RoomRegistry>().room("cave").cloned(),
            document
        );
        assert_eq!(named(&world, "cave.ball_1"), Some(ball));
        assert!(matches!(
            world
                .resource::<TilemapRegistry>()
                .changes_since("cave", &cursor),
            Some(TileChanges::Unchanged)
        ));
    }

    #[test]
    fn a_tileset_edit_patches_every_room_on_it() {
        let (_dir, mut world) = live_cave();
        let cursors: Vec<_> = ["cave", "hall"]
            .map(|room| world.resource::<TilemapRegistry>().cursor(room).unwrap())
            .into();
        let path = tileset_file(&world);
        edit(&path, |tileset| {
            tileset["tiles"][4]["material"] = json!("stone");
        });
        let errors = reload_json_files(&[path], &mut world, false);
        assert!(errors.is_empty(), "{errors:?}");
        for (room, cursor) in ["cave", "hall"].iter().zip(&cursors) {
            let maps = world.resource::<TilemapRegistry>();
            assert!(
                matches!(
                    maps.changes_since(room, cursor),
                    Some(TileChanges::Replaced)
                ),
                "{room}'s map inserted whole"
            );
            let props = maps.get(room).unwrap().tile_props(4);
            assert_eq!(props.material.as_deref(), Some("stone"), "{room}");
        }
        let tileset = world.resource::<RoomRegistry>().tileset("cave").unwrap();
        assert_eq!(tileset.tiles[4].material.as_deref(), Some("stone"));
    }

    #[test]
    fn a_tileset_reload_naming_an_unknown_sprite_keeps_the_old_one() {
        let (_dir, mut world) = live_cave();
        let before = world.resource::<RoomRegistry>().tileset("cave").cloned();
        let cursor = world.resource::<TilemapRegistry>().cursor("cave").unwrap();
        let path = tileset_file(&world);
        edit(&path, |tileset| {
            tileset["tiles"][4]["material"] = json!("stone");
            tileset["tiles"]
                .as_array_mut()
                .unwrap()
                .push(json!({ "sprite": "rock_88" }));
        });
        let errors = reload_json_files(&[path], &mut world, false);
        assert_eq!(
            errors,
            [
                "Hot reload tileset 'cave': room `cave`: tileset `cave`: tiles[6].sprite: unknown sprite `rock_88`; keeping the previous tileset and its rooms"
            ]
        );
        assert_eq!(
            world.resource::<RoomRegistry>().tileset("cave").cloned(),
            before
        );
        assert!(matches!(
            world
                .resource::<TilemapRegistry>()
                .changes_since("cave", &cursor),
            Some(TileChanges::Unchanged)
        ));
    }

    #[test]
    fn a_reload_changing_the_document_or_the_tileset_clears_its_journal_and_an_equal_one_keeps_it()
    {
        let (_dir, mut world) = live_cave();
        let path = room_file(&world, "cave");
        let file = fs::read_to_string(&path).unwrap();
        let ball = r#"[{ "move": { "id": "ball_1", "by": [1, 0] } }]"#;
        nudge(&mut world, "cave", ball);
        assert_eq!(undo_len(&world, "cave"), 1);
        // The registry's document saved to the file, as an editor saves it.
        let saved = world
            .resource::<RoomRegistry>()
            .room("cave")
            .unwrap()
            .to_canonical();
        fs::write(&path, saved).unwrap();
        assert!(reload_json_files(std::slice::from_ref(&path), &mut world, false).is_empty());
        assert_eq!(undo_len(&world, "cave"), 1, "an equal document keeps it");
        // The file as it was: a changed document, the ball back.
        fs::write(&path, &file).unwrap();
        assert!(reload_json_files(&[path], &mut world, false).is_empty());
        assert_eq!(undo_len(&world, "cave"), 0, "a changed document clears it");
        let ball_at = |world: &World| {
            let ball = named(world, "cave.ball_1").unwrap();
            world.get::<Transform>(ball).unwrap().position
        };
        assert_eq!(ball_at(&world), Vec2::new(488.0, 192.0));

        nudge(&mut world, "cave", ball);
        nudge(
            &mut world,
            "hall",
            r#"[{ "move": { "id": "west_door", "by": [1, 0] } }]"#,
        );
        let tileset = tileset_file(&world);
        assert!(reload_json_files(std::slice::from_ref(&tileset), &mut world, false).is_empty());
        assert_eq!(
            (undo_len(&world, "cave"), undo_len(&world, "hall")),
            (1, 1),
            "an equal tileset keeps them"
        );
        edit(&tileset, |tileset| {
            tileset["tiles"][4]["material"] = json!("stone");
        });
        assert!(reload_json_files(&[tileset], &mut world, false).is_empty());
        assert_eq!(
            (undo_len(&world, "cave"), undo_len(&world, "hall")),
            (0, 0),
            "a changed tileset clears each room's on it"
        );
    }

    #[test]
    fn a_manifest_reload_patches_the_live_rooms() {
        let (dir, mut world) = live_cave();
        let ball = named(&world, "cave.ball_1").unwrap();
        edit(&room_file(&world, "cave"), |room| {
            room["objects"][2]["at"] = json!([32.5, 12]);
        });
        // A new room, and the tileset moved.
        fs::copy(dir.join("rooms/hall.json"), dir.join("rooms/den.json")).unwrap();
        fs::create_dir_all(dir.join("tiles")).unwrap();
        fs::rename(dir.join("tilesets/cave.json"), dir.join("tiles/cave.json")).unwrap();
        edit(&dir.join("manifest.json"), |manifest| {
            manifest["rooms"]["den"] = json!({ "path": "rooms/den.json" });
            manifest["tilesets"]["cave"] = json!({ "path": "tiles/cave.json" });
        });
        let edited = ResolvedManifest::load(dir.join("manifest.json")).unwrap();
        super::super::reload_rooms_from_manifest(&edited, &mut world);
        assert!(!world.is_alive(ball));
        let again = named(&world, "cave.ball_1").unwrap();
        assert_eq!(
            world.get::<Transform>(again).unwrap().position,
            Vec2::new(520.0, 192.0)
        );
        let rooms = world.resource::<RoomRegistry>();
        assert_eq!(rooms.ids().collect::<Vec<_>>(), ["cave", "den", "hall"]);
        assert_eq!(rooms.path("den"), Some(edited.rooms["den"].path.as_path()));
        assert_eq!(
            rooms.tileset_path("cave"),
            Some(edited.tilesets["cave"].path.as_path()),
            "the moved file recorded"
        );
        assert!(world.resource::<TilemapRegistry>().get("den").is_some());
        // The tilemap block leaves the rooms' maps to the room block.
        assert!(crate::asset_loader::reload::reload_new_tilemaps(&edited, &mut world).is_empty());
    }
}

mod generators {
    use serde_json::Value;
    use tungsten_core::assets::{Generate, RoomGenerator, RoomGenerators};

    use super::*;
    use crate::asset_loader::room::{
        CheckSeeds, check_generators, reload_rooms_from_manifest, stale_warnings,
    };

    /// Two copies of the fixture's `hall`, side by side.
    struct Pair;

    impl RoomGenerator for Pair {
        fn version(&self) -> u32 {
            1
        }

        fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String> {
            let hall = generation.template("hall")?;
            generation.add("a", hall.clone(), [0.0, 0.0])?;
            generation.add("b", hall, [640.0, 0.0])
        }
    }

    #[test]
    fn app_register_generator_reaches_the_world() {
        let mut app = App::new(Config::default()).unwrap();
        app.register_generator("pair", Pair);
        let world = app.world_mut();
        let ids: Vec<&str> = world.resource::<RoomGenerators>().ids().collect();
        assert_eq!(ids, ["pair"]);
        let manifest = ResolvedManifest::load(fixture().join("manifest.json")).unwrap();
        load(&manifest, world);
        let rooms = world.generate_rooms("pair", 1, "pair").unwrap();
        let ids: Vec<&str> = rooms.rooms().map(|(id, _)| id).collect();
        assert_eq!(ids, ["pair_a", "pair_b"]);
        let registry = world.resource::<RoomRegistry>();
        assert_eq!(registry.generation("pair_b"), Some(rooms.generation()));
        assert_eq!(registry.generation("hall"), None);
        assert!(world.resource::<TilemapRegistry>().get("pair_a").is_some());
    }

    #[test]
    fn a_manifest_reload_keeps_generated_rooms_without_a_warning() {
        let dir = fixture_copy();
        let path = dir.join("manifest.json");
        let manifest = ResolvedManifest::load(&path).unwrap();
        let mut world = app_world();
        load(&manifest, &mut world);
        world.register_generator("pair", Pair);
        world.generate_rooms("pair", 1, "pair").unwrap();
        let held = |world: &World| -> Vec<String> {
            world
                .resource::<RoomRegistry>()
                .ids()
                .map(str::to_owned)
                .collect()
        };
        assert_eq!(held(&world), ["cave", "hall", "pair_a", "pair_b"]);
        assert!(stale_warnings(world.resource::<RoomRegistry>(), &manifest).is_empty());
        // The manifest without `hall`: the authored room it no longer lists
        // warns, the generated ones do not.
        let mut edited: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        edited["rooms"].as_object_mut().unwrap().remove("hall");
        fs::write(&path, edited.to_string()).unwrap();
        let edited = ResolvedManifest::load(&path).unwrap();
        assert_eq!(
            stale_warnings(world.resource::<RoomRegistry>(), &edited),
            ["Manifest reload: room 'hall' removed — keeping stale"]
        );
        reload_rooms_from_manifest(&edited, &mut world);
        assert_eq!(held(&world), ["cave", "hall", "pair_a", "pair_b"]);
        let registry = world.resource::<RoomRegistry>();
        assert!(registry.generation("pair_a").is_some() && registry.path("pair_a").is_none());
        assert!(world.resource::<TilemapRegistry>().get("pair_b").is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    /// `hall` with a lamp, standing on its floor at odd seeds and three
    /// cells over it at even ones.
    struct Lamps;

    impl RoomGenerator for Lamps {
        fn version(&self) -> u32 {
            2
        }

        fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String> {
            let mut hall = generation.template("hall")?;
            let row = if generation.seed() % 2 == 0 { 6 } else { 9 };
            let lamp = format!(
                r#"[{{ "place": {{ "objects": {{ "id": "lamp_9", "prefab": "lamp", "at": [5, {row}] }} }} }}]"#
            );
            let batch = tungsten_core::assets::RoomCommand::parse_batch(&lamp).unwrap();
            generation.apply("a", &mut hall, &batch)?;
            generation.add("a", hall, [0.0, 0.0])
        }
    }

    /// `hall` alone, failing at seed 3.
    struct Broken;

    impl RoomGenerator for Broken {
        fn version(&self) -> u32 {
            1
        }

        fn generate(&self, generation: &mut Generate<'_>) -> Result<(), String> {
            if generation.seed() == 3 {
                return Err("no exit fits".to_owned());
            }
            let hall = generation.template("hall")?;
            generation.add("a", hall, [0.0, 0.0])
        }
    }

    /// The fixture checked into an app's World with `register`'s generators,
    /// then swept over `seeds`: the findings as `<file>: <message>`, and
    /// the notes.
    fn sweep(
        register: impl FnOnce(&mut App),
        seeds: CheckSeeds,
    ) -> (World, Vec<String>, Vec<String>) {
        let mut app = App::new(Config::default()).unwrap();
        register(&mut app);
        let dir = fixture();
        let errors = check_files(app.world_mut(), &[dir.join("manifest.json")], &dir);
        assert!(errors.is_empty(), "{errors:?}");
        let mut world = std::mem::take(app.world_mut());
        let (mut findings, mut notes) = (Vec::new(), Vec::new());
        check_generators(
            &mut world,
            seeds,
            |file, message| findings.push(format!("{file}: {message}")),
            |note| notes.push(note),
        );
        (world, findings, notes)
    }

    #[test]
    fn check_generators_sweeps_every_generator_over_its_seeds() {
        let (_, findings, notes) = sweep(
            |app| {
                app.register_generator("pair", Pair);
                app.register_generator("broken", Broken);
            },
            CheckSeeds { first: 5, count: 3 },
        );
        assert!(findings.is_empty(), "{findings:?}");
        assert_eq!(
            notes,
            [
                "generator `broken` (version 1): seeds 5+3, 3 rooms",
                "generator `pair` (version 1): seeds 5+3, 6 rooms"
            ],
            "sorted, each over every seed"
        );
        let (_, _, notes) = sweep(
            |app| app.register_generator("pair", Pair),
            CheckSeeds {
                first: 0,
                count: 750,
            },
        );
        assert_eq!(
            notes,
            ["generator `pair` (version 1): seeds 0+750, 1,500 rooms"]
        );
    }

    #[test]
    fn a_finding_at_many_seeds_is_one_line_naming_them() {
        let seeds = CheckSeeds {
            first: 0,
            count: 12,
        };
        let (_, findings, notes) = sweep(|app| app.register_generator("lamps", Lamps), seeds);
        assert_eq!(
            findings,
            [
                "generator lamps: seeds 0, 2, 4, 6, 8 and 1 more: lamps_a: lamp_9: objects[0]: floats: 0 of 8 bottom columns have ground within 2 px"
            ]
        );
        assert_eq!(
            notes,
            ["generator `lamps` (version 2): seeds 0+12, 12 rooms"]
        );
        let (_, findings, _) = sweep(
            |app| app.register_generator("lamps", Lamps),
            CheckSeeds { first: 4, count: 5 },
        );
        assert_eq!(
            findings,
            [
                "generator lamps: seeds 4, 6 and 8: lamps_a: lamp_9: objects[0]: floats: 0 of 8 bottom columns have ground within 2 px"
            ]
        );
    }

    #[test]
    fn a_generators_error_is_a_finding_at_its_seed() {
        let (_, findings, notes) = sweep(
            |app| app.register_generator("broken", Broken),
            CheckSeeds { first: 0, count: 5 },
        );
        assert_eq!(findings, ["generator broken: seed 3: no exit fits"]);
        assert_eq!(
            notes,
            ["generator `broken` (version 1): seeds 0+5, 4 rooms"]
        );
    }

    #[test]
    fn the_sweep_leaves_no_generated_room_registered() {
        let (world, _, _) = sweep(
            |app| {
                app.register_generator("pair", Pair);
                app.register_generator("lamps", Lamps);
            },
            CheckSeeds { first: 0, count: 4 },
        );
        let rooms: Vec<&str> = world.resource::<RoomRegistry>().ids().collect();
        assert_eq!(rooms, ["cave", "hall"]);
        let mut maps: Vec<&str> = world.resource::<TilemapRegistry>().ids().collect();
        maps.sort_unstable();
        assert_eq!(maps, ["cave", "hall"]);
        assert!(
            !world.has_resource::<tungsten_core::save::SaveState>(),
            "nothing spawned or named"
        );
    }
}
