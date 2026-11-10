//! Live rooms patched (redo 7.12): the registries, the map and the live
//! instance, prepared whole before anything changes.

use glam::Vec2;
use serde_json::{Value, json};

use super::*;
use crate::assets::{PrefabData, SpawnedRoom, TileChanges};
use crate::components::{Tag, Transform};
use crate::physics::{PhysicsPlugin, Position};
use crate::save::{Saved, capture_world, restore_world};
use crate::schedule::{Plugin, Schedule};

/// What the room's spawn gives its instance, which a patch's spawn takes
/// through the spawn inheritance.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Marker(u32);

const TILESET: &str = r#"{ "version": 1, "tiles": [
    { "sprite": "rock" }, { "sprite": "moss" }, { "sprite": "plank", "one_way": true } ] }"#;
const MOSS: TileIndex = 1;
const ART: usize = 0;
const SOLID: usize = 1;

/// A World with physics' `rigid_body` and saved `position`, the prefabs
/// `crate` (a box), `ball` (a circle) and `lamp` (a tag), the `cave`
/// tileset, and `Marker` passed on by the spawn inheritance.
fn world() -> World {
    let mut world = World::new();
    PhysicsPlugin.build(&mut Schedule::new(), &mut world);
    world.inherit_from_source::<Marker>();
    let mut prefabs = PrefabRegistry::new();
    for (id, value) in [
        (
            "crate",
            json!({ "tag": "crate", "components": { "rigid_body": { "kind": "dynamic",
                "shape": { "kind": "aabb", "half_extents": [8, 8] } } } }),
        ),
        (
            "ball",
            json!({ "tag": "ball", "components": { "rigid_body": { "kind": "dynamic",
                "shape": { "kind": "circle", "radius": 6 } } } }),
        ),
        ("lamp", json!({ "tag": "lamp" })),
    ] {
        let data: PrefabData = serde_json::from_value(value).unwrap();
        prefabs.insert(id.into(), data);
    }
    world.insert_resource(prefabs);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset("cave", Tileset::from_json(TILESET).unwrap());
    world.insert_resource(rooms);
    world
}

/// A 10 × 6 room of 16 px cells: a rock floor along its bottom row in
/// `art` and `solid`, `crate_1` at (2, 4), `ball_1` at (6, 4).
fn cave() -> Value {
    json!({ "version": 1, "cell": [16, 16], "size": [10, 6], "tileset": "cave",
        "layers": [{ "name": "art" }, { "name": "solid", "kind": "collision" }],
        "shapes": [{ "id": "floor", "rect": [0, 5, 10, 1], "tile": "rock", "layers": ["art", "solid"] }],
        "objects": [{ "id": "crate_1", "prefab": "crate", "at": [2, 4] },
                    { "id": "ball_1", "prefab": "ball", "at": [6, 4] }],
        "places": [{ "id": "start", "point": [1, 4] }] })
}

fn room(document: &Value) -> Room {
    Room::from_json(&document.to_string()).unwrap()
}

/// Room `cave` inserted from `document` and spawned at (100, 50) with
/// `Marker(7)`.
fn spawn(world: &mut World, document: &Value) -> SpawnedRoom {
    world.insert_room("cave", room(document)).unwrap();
    world
        .spawn_room("cave", [100.0, 50.0], &(Marker(7),))
        .unwrap()
}

/// The live entity named `cave.<id>`.
fn object(world: &World, id: &str) -> Option<Entity> {
    world
        .get_resource::<SaveState>()?
        .named_entity(SaveId::authored(&format!("cave.{id}")))
        .filter(|&entity| world.is_alive(entity))
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Transform>(entity).unwrap().position
}

fn tag(world: &World, entity: Entity) -> &str {
    &world.get::<Tag>(entity).unwrap().name
}

fn maps(world: &World) -> &TilemapRegistry {
    world.resource::<TilemapRegistry>()
}

/// The instance and every object of a spawn despawned, as a game leaving
/// the area does.
fn leave(world: &mut World, spawned: &SpawnedRoom) {
    world.despawn(spawned.instance());
    for id in ["crate_1", "ball_1", "lamp_1"] {
        if let Some(entity) = object(world, id) {
            world.despawn(entity);
        }
    }
}

#[test]
fn a_moved_object_respawns_with_its_new_spot_as_its_baseline() {
    let mut world = world();
    let spawned = spawn(&mut world, &cave());
    let ball = spawned.object("ball_1").unwrap();
    let crate_1 = spawned.object("crate_1").unwrap();
    let mut moved = cave();
    moved["objects"][1]["at"] = json!([8, 4]);
    let patch = world.patch_room("cave", room(&moved)).unwrap();
    assert_eq!(patch.respawned, ["ball_1"]);
    assert!(patch.spawned.is_empty() && patch.despawned.is_empty());
    assert_eq!(patch.map, MapPatch::Unchanged);
    assert_eq!(
        patch.to_string(),
        "room `cave`: respawned ball_1",
        "the log's line"
    );
    assert!(!world.is_alive(ball), "the old spawn despawned");
    assert_eq!(
        object(&world, "crate_1"),
        Some(crate_1),
        "an unchanged one stays"
    );
    let again = object(&world, "ball_1").expect("spawned again under its name");
    assert_eq!(position(&world, again), Vec2::new(228.0, 114.0));
    assert_eq!(world.get::<Marker>(again), Some(&Marker(7)));
    assert_eq!(world.get::<Saved>(again).unwrap().area(), Some("cave"));
    // Its baseline is its new spawn: a capture writes no change for it.
    let changes = |world: &World| {
        let save = serde_json::to_value(capture_world(world).unwrap()).unwrap();
        save.pointer("/areas/cave/named/cave.ball_1/components")
            .cloned()
    };
    assert_eq!(changes(&world), None);
    world.get_mut::<Position>(again).unwrap().0.x = 300.0;
    assert!(changes(&world).is_some(), "a change from it is written");
}

#[test]
fn a_patch_move_then_a_save_restores_the_saved_spot() {
    // Cells of 1 px: `at` is in pixels.
    let at = |x: f32| {
        let mut document = cave();
        document["cell"] = json!([1, 1]);
        document["size"] = json!([32, 8]);
        document["shapes"] = json!([]);
        document["objects"] = json!([{ "id": "ball_1", "prefab": "ball", "at": [x, 4] }]);
        document["places"] = json!([]);
        document
    };
    let mut world = world();
    world.insert_room("cave", room(&at(10.0))).unwrap();
    world.spawn_room("cave", [0.0, 0.0], &(Marker(7),)).unwrap();
    world.patch_room("cave", room(&at(20.0))).unwrap();
    let ball = object(&world, "ball_1").unwrap();
    assert_eq!(world.get::<Position>(ball).unwrap().0.x, 20.0);
    // Play moves it back to 10, where the room had it before the patch.
    world.get_mut::<Position>(ball).unwrap().0.x = 10.0;
    let save = capture_world(&world).unwrap();

    let mut loaded = self::world();
    loaded.insert_room("cave", room(&at(20.0))).unwrap();
    loaded
        .spawn_room("cave", [0.0, 0.0], &(Marker(7),))
        .unwrap();
    let report = restore_world(&mut loaded, &save, &(Marker(7),)).unwrap();
    assert!(report.is_clean(), "{report:?}");
    let ball = object(&loaded, "ball_1").unwrap();
    assert_eq!(
        loaded.get::<Position>(ball).unwrap().0.x,
        10.0,
        "saved against the patch's spot, not the old one"
    );
}

#[test]
fn a_changed_prefab_respawns_under_the_same_name() {
    let mut world = world();
    let spawned = spawn(&mut world, &cave());
    let old = spawned.object("crate_1").unwrap();
    let mut changed = cave();
    changed["objects"][0]["prefab"] = json!("lamp");
    let patch = world.patch_room("cave", room(&changed)).unwrap();
    assert_eq!(patch.respawned, ["crate_1"]);
    assert!(!world.is_alive(old));
    let lamp = object(&world, "crate_1").unwrap();
    assert_eq!(tag(&world, lamp), "lamp");
    assert_eq!(position(&world, lamp), Vec2::new(132.0, 114.0));
    assert_eq!(world.get::<Marker>(lamp), Some(&Marker(7)));
}

#[test]
fn a_removed_object_despawns_and_an_added_one_spawns_named_with_the_instances_inheritance() {
    let mut world = world();
    let spawned = spawn(&mut world, &cave());
    let ball = spawned.object("ball_1").unwrap();
    let mut changed = cave();
    changed["objects"] = json!([
        { "id": "crate_1", "prefab": "crate", "at": [2, 4] },
        { "id": "lamp_1", "prefab": "lamp", "at": [4.5, 3] }
    ]);
    let patch = world.patch_room("cave", room(&changed)).unwrap();
    assert_eq!(patch.despawned, ["ball_1"]);
    assert_eq!(patch.spawned, ["lamp_1"]);
    assert!(patch.respawned.is_empty());
    assert!(!world.is_alive(ball));
    assert_eq!(object(&world, "ball_1"), None);
    let lamp = object(&world, "lamp_1").expect("named `cave.lamp_1`");
    assert_eq!(tag(&world, lamp), "lamp");
    assert_eq!(position(&world, lamp), Vec2::new(172.0, 98.0));
    assert_eq!(world.get::<Saved>(lamp).unwrap().area(), Some("cave"));
    assert_eq!(
        world.get::<Marker>(lamp),
        Some(&Marker(7)),
        "what the instance passes on"
    );
}

#[test]
fn changed_cells_write_in_place_and_a_resized_room_inserts_whole() {
    let mut world = world();
    spawn(&mut world, &cave());
    // A play's edit elsewhere.
    world
        .resource_mut::<TilemapRegistry>()
        .set_tile("cave", SOLID, 0, 0, MOSS);
    let cursor = maps(&world).cursor("cave").unwrap();
    let mut painted = cave();
    painted["layers"][0]["cells"] = json!({ "3,5": "moss", "4,2": "plank" });
    let patch = world.patch_room("cave", room(&painted)).unwrap();
    assert_eq!(patch.map, MapPatch::Cells(2));
    let cells: Vec<(usize, u32, u32)> = match maps(&world).changes_since("cave", &cursor) {
        Some(TileChanges::Cells(cells)) => cells.map(|c| (c.layer, c.col, c.row)).collect(),
        _ => panic!("the cells journaled"),
    };
    assert_eq!(cells, [(ART, 4, 2), (ART, 3, 5)], "in cell order");
    let map = maps(&world).get("cave").unwrap();
    assert_eq!(map.tile_at(ART, 3, 5), Some(MOSS));
    assert_eq!(
        map.tile_at(SOLID, 0, 0),
        Some(MOSS),
        "the play's edit stays"
    );
    assert_eq!(maps(&world).edited_cells("cave").count(), 1);
    // The map as inserted holds the patch, so a revert keeps it.
    world.resource_mut::<TilemapRegistry>().revert("cave");
    let map = maps(&world).get("cave").unwrap();
    assert_eq!(map.tile_at(ART, 3, 5), Some(MOSS));
    assert_eq!(
        map.tile_at(SOLID, 0, 0),
        Some(EMPTY),
        "the play's edit is not"
    );

    let cursor = maps(&world).cursor("cave").unwrap();
    let mut wider = painted.clone();
    wider["size"] = json!([12, 6]);
    let patch = world.patch_room("cave", room(&wider)).unwrap();
    assert_eq!(patch.map, MapPatch::Inserted);
    assert!(matches!(
        maps(&world).changes_since("cave", &cursor),
        Some(TileChanges::Replaced)
    ));
    assert_eq!(maps(&world).get("cave").unwrap().width, 12);
}

const EMPTY: TileIndex = crate::assets::tilemap::EMPTY_TILE;

#[test]
fn a_props_change_inserts_the_map_whole() {
    let mut world = world();
    spawn(&mut world, &cave());
    let cursor = maps(&world).cursor("cave").unwrap();
    let mut tileset = Tileset::from_json(TILESET).unwrap();
    tileset.tiles[MOSS as usize].one_way = true;
    let patches = world
        .patch_tileset("cave", tileset.clone(), Vec::new())
        .unwrap();
    assert_eq!(patches.len(), 1);
    assert_eq!(patches[0].room, "cave");
    assert_eq!(patches[0].map, MapPatch::Inserted);
    assert!(matches!(
        maps(&world).changes_since("cave", &cursor),
        Some(TileChanges::Replaced)
    ));
    assert!(maps(&world).get("cave").unwrap().tile_props(MOSS).one_way);
    assert_eq!(
        world.resource::<RoomRegistry>().tileset("cave"),
        Some(&tileset)
    );
    // The same tileset again changes nothing.
    assert_eq!(
        world.patch_tileset("cave", tileset, Vec::new()),
        Ok(Vec::new())
    );
}

#[test]
fn a_patched_room_spawns_again_as_patched() {
    let mut world = world();
    let spawned = spawn(&mut world, &cave());
    let mut changed = cave();
    changed["objects"][1]["at"] = json!([8, 4]);
    changed["layers"][0]["cells"] = json!({ "3,5": "moss" });
    world.patch_room("cave", room(&changed)).unwrap();
    leave(&mut world, &spawned);
    let again = world
        .spawn_room("cave", [100.0, 50.0], &(Marker(8),))
        .unwrap();
    let ball = again.object("ball_1").unwrap();
    assert_eq!(position(&world, ball), Vec2::new(228.0, 114.0));
    assert_eq!(object(&world, "ball_1"), Some(ball));
    assert_eq!(
        maps(&world).get("cave").unwrap().tile_at(ART, 3, 5),
        Some(MOSS)
    );
}

#[test]
fn a_patch_that_fails_to_prepare_changes_nothing() {
    let mut world = world();
    let spawned = spawn(&mut world, &cave());
    let ball = spawned.object("ball_1").unwrap();
    let nudge =
        RoomCommand::parse_batch(r#"[{ "move": { "id": "crate_1", "by": [1, 0] } }]"#).unwrap();
    world.apply_room_batch("cave", &nudge).unwrap();
    let crate_1 = object(&world, "crate_1").unwrap();
    let document = world
        .resource::<RoomRegistry>()
        .room("cave")
        .unwrap()
        .clone();
    let cursor = maps(&world).cursor("cave").unwrap();

    let bad = RoomCommand::parse_batch(
        r#"[{ "set": { "id": "ball_1", "path": "set.rigid_body.shape.radius", "value": -1 } },
            { "paint": { "layer": "art", "cells": [[3, 5]], "tile": "moss" } }]"#,
    )
    .unwrap();
    let err = world.apply_room_batch("cave", &bad).unwrap_err();
    assert!(
        err.message()
            .starts_with("room `cave`: objects[1] (`ball_1`): "),
        "{err}"
    );
    assert!(err.message().contains("shape.radius"), "{err}");
    assert_eq!(
        world.resource::<RoomRegistry>().room("cave"),
        Some(&document),
        "the room"
    );
    assert_eq!(
        world.resource::<RoomJournal>().undo_len("cave"),
        1,
        "its journal"
    );
    assert!(
        matches!(
            maps(&world).changes_since("cave", &cursor),
            Some(TileChanges::Unchanged)
        ),
        "its map"
    );
    assert_eq!(object(&world, "ball_1"), Some(ball), "the entities");
    assert_eq!(object(&world, "crate_1"), Some(crate_1));
    // The good batch undoes.
    let undone = world.undo_room_batch("cave").unwrap().unwrap();
    assert_eq!(undone.respawned, ["crate_1"]);
    assert_eq!(
        world.resource::<RoomRegistry>().room("cave"),
        Some(&room(&cave()))
    );
    assert_eq!(world.undo_room_batch("cave"), Ok(None));
}

#[test]
fn an_object_play_despawned_stays_gone() {
    let mut world = world();
    let spawned = spawn(&mut world, &cave());
    world.despawn(spawned.object("ball_1").unwrap());
    let mut moved = cave();
    moved["objects"][1]["at"] = json!([8, 4]);
    let patch = world.patch_room("cave", room(&moved)).unwrap();
    assert!(patch.respawned.is_empty(), "{patch}");
    assert!(patch.objects, "the room's objects changed");
    assert_eq!(object(&world, "ball_1"), None, "play removed it");
    // The room's next spawn shows it where the patch put it.
    leave(&mut world, &spawned);
    let again = world
        .spawn_room("cave", [100.0, 50.0], &(Marker(7),))
        .unwrap();
    assert_eq!(
        position(&world, again.object("ball_1").unwrap()),
        Vec2::new(228.0, 114.0)
    );
}

#[test]
fn an_equal_compiled_room_patches_nothing() {
    let mut world = world();
    let spawned = spawn(&mut world, &cave());
    let cursor = maps(&world).cursor("cave").unwrap();
    // An override the floor already paints: a new document, the same room.
    let mut same = cave();
    same["layers"][0]["cells"] = json!({ "0,5": "rock" });
    let patch = world.patch_room("cave", room(&same)).unwrap();
    assert!(patch.is_empty(), "{patch:?}");
    assert_eq!(patch.to_string(), "room `cave` unchanged");
    assert!(matches!(
        maps(&world).changes_since("cave", &cursor),
        Some(TileChanges::Unchanged)
    ));
    for id in ["crate_1", "ball_1"] {
        assert_eq!(object(&world, id), spawned.object(id), "{id}");
    }
    assert_eq!(
        world.resource::<RoomRegistry>().room("cave"),
        Some(&room(&same)),
        "the registry takes the document"
    );
}

#[test]
fn redo_after_undo_applies_the_batch_again_and_patches_the_live_object() {
    let mut world = world();
    let spawned = spawn(&mut world, &cave());
    let batch =
        RoomCommand::parse_batch(r#"[{ "move": { "id": "crate_1", "by": [1, 0] } }]"#).unwrap();
    assert_eq!(world.redo_room_batch("cave"), Ok(None), "no journal yet");
    world.apply_room_batch("cave", &batch).unwrap();
    let moved = world
        .resource::<RoomRegistry>()
        .room("cave")
        .unwrap()
        .clone();
    let at = |world: &World| position(world, object(world, "crate_1").unwrap());
    let after = at(&world);
    assert_eq!(world.redo_room_batch("cave"), Ok(None), "nothing undone");
    world.undo_room_batch("cave").unwrap().unwrap();
    assert_eq!(at(&world), Vec2::new(132.0, 114.0), "back at (2, 4)");
    let journal = world.resource::<RoomJournal>();
    assert_eq!((journal.undo_len("cave"), journal.redo_len("cave")), (0, 1));
    let redone = world.redo_room_batch("cave").unwrap().unwrap();
    assert_eq!(redone.respawned, ["crate_1"]);
    assert_eq!(at(&world), after);
    assert_eq!(world.resource::<RoomRegistry>().room("cave"), Some(&moved));
    let journal = world.resource::<RoomJournal>();
    assert_eq!(
        (journal.undo_len("cave"), journal.redo_len("cave")),
        (1, 0),
        "from the undone list to the done list"
    );
    assert_eq!(world.redo_room_batch("cave"), Ok(None));
    // Undone again, then a reload that changes the document clears the
    // journal: nothing to redo.
    world.undo_room_batch("cave").unwrap().unwrap();
    let mut edited = cave();
    edited["objects"][1]["at"] = json!([7, 4]);
    world.patch_room("cave", room(&edited)).unwrap();
    assert_eq!(world.resource::<RoomJournal>().redo_len("cave"), 0);
    assert_eq!(world.redo_room_batch("cave"), Ok(None));
    assert!(spawned.object("ball_1").is_some());
}

#[test]
fn an_objects_error_names_its_id() {
    let mut world = world();
    spawn(&mut world, &cave());
    let document = world
        .resource::<RoomRegistry>()
        .room("cave")
        .unwrap()
        .clone();
    let mut bad = cave();
    bad["objects"][1]["set"] = json!({ "rigid_body.shape.radius": -1 });
    let bad = room(&bad);
    let compiled = bad
        .compile(
            "cave",
            &Tileset::from_json(TILESET).unwrap(),
            &PhysicsLayers::default(),
        )
        .unwrap();
    let err = world.check_map_objects("cave", &compiled.map).unwrap_err();
    // The map's terms, then the room's.
    assert!(
        err.to_string()
            .starts_with("tilemap `cave`, layer `objects`, object 2: prefab `ball`"),
        "{err}"
    );
    let (index, object, message) = bad.object_of(&err).expect("a room object's error");
    assert_eq!((index, object.id.as_str()), (1, "ball_1"));
    assert!(message.starts_with("prefab `ball`"), "{message}");
    // An error about no object is no entry's.
    let unknown = PrefabError::UnknownMap { map: "x".into() };
    assert!(document.object_of(&unknown).is_none());
}

#[test]
fn a_refused_batch_gives_its_reason_without_the_path() {
    let mut world = world();
    spawn(&mut world, &cave());
    let far =
        RoomCommand::parse_batch(r#"[{ "move": { "id": "crate_1", "by": [0, 40] } }]"#).unwrap();
    let err = world.apply_room_batch("cave", &far).unwrap_err();
    assert_eq!(
        err.message(),
        "room `cave`: commands[0].move: objects[0].at: [2, 44] is past the room's 10 \u{d7} 6 cells"
    );
    assert_eq!(err.reason(), "[2, 44] is past the room's 10 \u{d7} 6 cells");
    // A patch that fails outside a batch: the reason is the message past
    // the room's name.
    let mut gone = cave();
    gone["tileset"] = json!("nope");
    let err = world.patch_room("cave", room(&gone)).unwrap_err();
    assert_eq!(err.message(), "room `cave`: tileset: no tileset `nope`");
    assert_eq!(err.reason(), "tileset: no tileset `nope`");
}
