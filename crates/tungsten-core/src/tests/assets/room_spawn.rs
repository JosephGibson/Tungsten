//! Rooms put away (the level editor's pass): `World::despawn_room` despawns
//! a spawned room and keeps it registered, and never takes a record a
//! restore kept.

use serde_json::json;

use super::*;
use crate::assets::{PrefabData, PrefabRegistry};
use crate::save::{SaveId, capture_world, restore_world};

/// What a room's spawn gives its entities.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Marker;

/// A World with the prefab `crate`, a tag, and rooms `cave` and `den`, each
/// 4 by 4 cells with `crate_1` and `crate_2`.
fn world() -> World {
    let mut world = World::new();
    let mut prefabs = PrefabRegistry::new();
    let data: PrefabData = serde_json::from_value(json!({ "tag": "crate" })).unwrap();
    prefabs.insert("crate".into(), data);
    world.insert_resource(prefabs);
    for id in ["cave", "den"] {
        let room = json!({ "version": 1, "cell": [16, 16], "size": [4, 4],
            "objects": [{ "id": "crate_1", "prefab": "crate", "at": [1, 1] },
                        { "id": "crate_2", "prefab": "crate", "at": [2, 1] }] });
        world
            .insert_room(id, Room::from_json(&room.to_string()).unwrap())
            .unwrap();
    }
    world
}

/// The live entity named `name`.
fn named(world: &World, name: &str) -> Option<Entity> {
    world
        .get_resource::<SaveState>()?
        .named_entity(SaveId::authored(name))
        .filter(|&entity| world.is_alive(entity))
}

/// The areas a capture of `world` holds.
fn areas(world: &World) -> Vec<String> {
    capture_world(world)
        .unwrap()
        .area_names()
        .map(str::to_owned)
        .collect()
}

#[test]
fn despawn_room_puts_a_room_away_and_keeps_it_registered() {
    let mut world = world();
    let cave = world.spawn_room("cave", [0.0, 0.0], &(Marker,)).unwrap();
    world.spawn_room("den", [100.0, 0.0], &(Marker,)).unwrap();
    // One of its crates is gone already: the other and the instance go.
    world.despawn(cave.object("crate_2").unwrap());
    assert_eq!(world.despawn_room("cave").unwrap(), 2);
    assert!(!world.is_alive(cave.instance()));
    assert!(!world.is_alive(cave.object("crate_1").unwrap()));
    assert_eq!(named(&world, "cave.crate_1"), None);
    // The room is still a room, not live, out of the loaded areas, its
    // names free; the other room is untouched.
    let rooms = world.resource::<RoomRegistry>();
    assert!(rooms.room("cave").is_some());
    assert_eq!(rooms.live("cave"), None);
    assert!(world.resource::<TilemapRegistry>().get("cave").is_some());
    let state = world.resource::<SaveState>();
    assert!(!state.is_loaded("cave") && state.is_loaded("den"));
    assert_eq!(state.name_of(SaveId::authored("cave.crate_1")), None);
    assert!(named(&world, "den.crate_1").is_some());
    assert_eq!(areas(&world), ["den"]);
    // Away already, it despawns nothing; a room the registry lacks is an
    // error.
    assert_eq!(world.despawn_room("cave").unwrap(), 0);
    assert!(matches!(
        world.despawn_room("attic"),
        Err(AreaError::UnknownRoom { .. })
    ));
}

#[test]
fn despawn_room_leaves_a_kept_record() {
    let mut source = world();
    source.spawn_room("cave", [0.0, 0.0], &(Marker,)).unwrap();
    let den = source.spawn_room("den", [100.0, 0.0], &(Marker,)).unwrap();
    // Progress in `den`: one crate destroyed.
    source.despawn(den.object("crate_2").unwrap());
    let save = capture_world(&source).unwrap();

    // A World that has spawned `cave` alone keeps `den`'s record.
    let mut world = world();
    world.spawn_room("cave", [0.0, 0.0], &(Marker,)).unwrap();
    restore_world(&mut world, &save, &(Marker,)).unwrap();
    assert!(world.resource::<SaveState>().is_kept("den"));
    // Spawned without entering, the area is loaded beside its kept record;
    // put away, it is kept as before, and a capture still carries it.
    world.spawn_room("den", [100.0, 0.0], &(Marker,)).unwrap();
    assert!(world.resource::<SaveState>().is_loaded("den"));
    assert_eq!(world.despawn_room("den").unwrap(), 3);
    let state = world.resource::<SaveState>();
    assert!(state.is_kept("den"), "the player's progress there");
    assert!(!state.is_loaded("den"));
    assert_eq!(areas(&world), ["cave", "den"]);
    let again = serde_json::to_value(capture_world(&world).unwrap()).unwrap();
    let saved = serde_json::to_value(&save).unwrap();
    assert_eq!(
        again["areas"]["den"], saved["areas"]["den"],
        "the record as the save had it"
    );
    assert!(saved["areas"]["den"].is_object());
}

#[test]
fn a_room_spawns_again_after_despawn_room() {
    let mut world = world();
    world.spawn_room("cave", [0.0, 0.0], &(Marker,)).unwrap();
    assert!(matches!(
        world.spawn_room("cave", [0.0, 0.0], &(Marker,)),
        Err(AreaError::Live { .. })
    ));
    world.despawn_room("cave").unwrap();
    let again = world.spawn_room("cave", [50.0, 0.0], &(Marker,)).unwrap();
    assert!(world.is_alive(again.instance()));
    assert_eq!(again.objects().count(), 2);
    assert_eq!(
        named(&world, "cave.crate_1"),
        again.object("crate_1"),
        "named again"
    );
    assert!(world.resource::<SaveState>().is_loaded("cave"));
    assert_eq!(world.get::<Marker>(again.instance()), Some(&Marker));
}
