//! Rooms and tilesets: reading, checks, the canonical form and schemas.

use super::*;

/// A room setting every field the format has.
pub(crate) const FULL: &str = r#"{
  "version": 1,
  "cell": [16, 16],
  "size": [40, 24],
  "tileset": "cave",
  "layers": [
    { "name": "art", "z": 1, "unlit": true, "tint": [200, 220, 255, 255] },
    { "name": "solid", "kind": "collision", "layers": ["terrain"], "mask": ["player", "enemy"],
      "cells": { "7,19": "rock_crack", "8,19": null } }
  ],
  "shapes": [
    { "id": "floor", "rect": [0, 20, 40, 4], "tile": "rock", "layers": ["art", "solid"] },
    { "id": "ramp", "polygon": [[10, 20], [14, 16], [14, 20]], "tile": "rock", "layers": ["art", "solid"] },
    { "id": "bridge", "line": [[18, 12], [26, 12]], "tile": "plank", "layers": ["art", "solid"] },
    { "id": "pit", "rect": [30, 20, 3, 4], "tile": null, "layers": ["art", "solid"] }
  ],
  "objects": [
    { "id": "crate_1", "prefab": "crate", "at": [12, 19], "on": "floor" },
    { "id": "crawler_2", "prefab": "crawler", "at": [22.5, 11], "set": { "health.max": 30 } }
  ],
  "places": [
    { "id": "player_start", "point": [2, 19] },
    { "id": "arena", "rect": [20, 4, 16, 16], "to": "hall.arena" },
    { "id": "main", "route": ["player_start", [16, 15], "exit"], "by": "player" },
    { "id": "exit", "point": [39, 19], "to": "hall.west_door" }
  ],
  "post": [{ "kind": "bloom", "params": { "threshold": 0.8, "knee": 0.2, "intensity": 0.6, "radius": 1.0 } }],
  "ambient": [0.1, 0.1, 0.16],
  "budget": { "lights": 16, "objects": 1024 }
}"#;

/// A tileset setting every field the format has.
pub(crate) const FULL_TILESET: &str = r#"{
  "version": 1,
  "tiles": [
    { "sprite": "rock", "style": "rock" },
    { "sprite": "rock_1", "style": "rock", "mask": 1, "weight": 2 },
    { "sprite": "rock_45_br", "style": "rock", "slope": "45 br" },
    { "sprite": "rock_crack", "material": "stone", "health": 10, "broken": "rubble" },
    { "sprite": "rubble" },
    { "sprite": "plank", "one_way": true },
    { "sprite": "arch", "whole": true }
  ]
}"#;

fn error(json: &str) -> RoomError {
    Room::from_json(json).expect_err("the room fails")
}

#[test]
fn a_room_reads_its_version_first() {
    let err = error(r#"{ "version": 2, "cell": "nonsense", "colour": 1 }"#);
    assert_eq!(err.field, "version");
    assert_eq!(
        err.message,
        "version 2 is not supported; this engine reads version 1"
    );
    let err = error(r#"{ "cell": [16, 16], "size": [4, 4] }"#);
    assert_eq!(
        (err.field.as_str(), err.message.as_str()),
        ("version", "missing; this engine reads version 1")
    );
    let err = error(r#"{ "version": "1", "cell": [16, 16], "size": [4, 4] }"#);
    assert_eq!(err.field, "version");
    assert_eq!(error("[]").message, "a room file is a JSON object");
    assert!(error("{ not json").message.starts_with("not JSON"));
    let err = Tileset::from_json(r#"{ "version": 3, "tiles": 7 }"#).unwrap_err();
    assert_eq!(
        err.to_string(),
        "version: version 3 is not supported; this engine reads version 1"
    );
}

#[test]
fn an_unknown_field_fails_with_its_path() {
    let err = error(
        r#"{ "version": 1, "cell": [16, 16], "size": [4, 4],
             "objects": [{ "id": "a", "prefab": "p", "at": [0, 0] },
                         { "id": "b", "prefab": "p", "at": [1, 1], "colour": 1 }] }"#,
    );
    assert_eq!(err.field, "objects[1].colour");
    assert!(err.message.starts_with("unknown field `colour`"), "{err}");
    let err = error(r#"{ "version": 1, "cell": [16, 16], "size": [4, 4], "sizes": [1, 1] }"#);
    assert_eq!(err.field, "sizes");
    let err = error(r#"{ "version": 1, "cell": [16, 16], "size": [4, "x"] }"#);
    assert_eq!(err.field, "size[1]");
    // A check past the model names its path too.
    let err = error(
        r#"{ "version": 1, "cell": [16, 16], "size": [4, 4],
             "objects": [{ "id": "a", "prefab": "p", "at": [9, 0] }] }"#,
    );
    assert_eq!(
        err.to_string(),
        "objects[0].at: [9, 0] is past the room's 4 \u{d7} 4 cells"
    );
    let err = Tileset::from_json(r#"{ "version": 1, "tiles": [{ "sprite": "a", "shape": 1 }] }"#)
        .unwrap_err();
    assert_eq!(err.field, "tiles[0].shape");
}

#[test]
fn the_canonical_form_reads_back_as_the_same_room() {
    let room = Room::from_json(FULL).unwrap();
    let canonical = room.to_canonical();
    assert_eq!(Room::from_json(&canonical).unwrap(), room);
    assert!(canonical.ends_with("}\n"));
    // Arrays of numbers on one line, a polygon's points one a line, keys
    // sorted, whole numbers without a fraction.
    assert!(
        canonical.contains("\n  \"cell\": [16, 16],\n"),
        "{canonical}"
    );
    assert!(
        canonical.contains(
            "\"polygon\": [\n        [10, 20],\n        [14, 16],\n        [14, 20]\n      ],"
        ),
        "{canonical}"
    );
    assert!(canonical.contains("\"at\": [22.5, 11]"), "{canonical}");
    assert!(
        canonical.contains("\"tile\": null"),
        "a shape's tile is always given"
    );
    assert!(
        canonical.find("\"ambient\"").unwrap() < canonical.find("\"budget\"").unwrap(),
        "keys sorted"
    );
    // Defaults are left out.
    let bare = Room::from_json(
        r#"{ "version": 1, "cell": [8, 8], "size": [2, 2], "layers": [], "budget": {} }"#,
    )
    .unwrap();
    assert_eq!(
        bare.to_canonical(),
        "{\n  \"budget\": {},\n  \"cell\": [8, 8],\n  \"size\": [2, 2],\n  \"version\": 1\n}\n"
    );
    let tileset = Tileset::from_json(FULL_TILESET).unwrap();
    assert_eq!(
        Tileset::from_json(&tileset.to_canonical()).unwrap(),
        tileset
    );
}

#[test]
fn the_canonical_form_is_its_own_canonical_form() {
    let canonical = Room::from_json(FULL).unwrap().to_canonical();
    assert_ne!(canonical, FULL, "the file is not canonical as written");
    assert_eq!(
        Room::from_json(&canonical).unwrap().to_canonical(),
        canonical
    );
    let tileset = Tileset::from_json(FULL_TILESET).unwrap().to_canonical();
    assert_eq!(
        Tileset::from_json(&tileset).unwrap().to_canonical(),
        tileset
    );
}

#[test]
fn ids_are_unique_across_shapes_objects_and_places() {
    let room = |places: &str| {
        format!(
            r#"{{ "version": 1, "cell": [16, 16], "size": [8, 8], "tileset": "cave",
                 "layers": [{{ "name": "art" }}],
                 "shapes": [{{ "id": "floor", "rect": [0, 6, 8, 2], "tile": "rock", "layers": ["art"] }}],
                 "objects": [{{ "id": "crate_1", "prefab": "crate", "at": [1, 5] }}],
                 "places": {places} }}"#
        )
    };
    let err = error(&room(
        r#"[{ "id": "start", "point": [1, 1] }, { "id": "floor", "point": [2, 2] }]"#,
    ));
    assert_eq!(
        err.to_string(),
        "places[1].id: `floor` is also shapes[0]'s ID"
    );
    let err = error(&room(r#"[{ "id": "crate_1", "point": [1, 1] }]"#));
    assert_eq!(
        err.to_string(),
        "places[0].id: `crate_1` is also objects[0]'s ID"
    );
    let err = error(&room(r#"[{ "id": "Start", "point": [1, 1] }]"#));
    assert_eq!(err.field, "places[0].id");
    assert!(err.message.contains("[a-z][a-z0-9_]*"), "{err}");
    assert!(Room::from_json(&room(r#"[{ "id": "start", "point": [1, 1] }]"#)).is_ok());
}

#[test]
fn a_sprite_given_twice_in_a_tileset_fails_the_load() {
    let err = Tileset::from_json(
        r#"{ "version": 1, "tiles": [{ "sprite": "rock" }, { "sprite": "moss" }, { "sprite": "rock", "one_way": true }] }"#,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "tiles[2].sprite: sprite `rock` is also tiles[0]'s; a sprite names one tile"
    );
    // The tile properties' own rules, and one brush a style.
    let err = Tileset::from_json(
        r#"{ "version": 1, "tiles": [{ "sprite": "rock", "broken": "rubbl", "health": 2 }, { "sprite": "rubble" }] }"#,
    )
    .unwrap_err();
    assert_eq!(
        err.to_string(),
        "tiles[0].broken: no tile `rubbl`; did you mean `rubble`?"
    );
    let err = Tileset::from_json(
        r#"{ "version": 1, "tiles": [{ "sprite": "a", "style": "s" }, { "sprite": "b", "style": "s" }] }"#,
    )
    .unwrap_err();
    assert_eq!(err.field, "tiles[1].style");
    let err = Tileset::from_json(r#"{ "version": 1, "tiles": [{ "sprite": "a", "weight": 2 }] }"#)
        .unwrap_err();
    assert_eq!(err.to_string(), "tiles[0]: `weight` needs `mask`");
    let err =
        Tileset::from_json(r#"{ "version": 1, "tiles": [{ "sprite": "a", "slope": "46 br" }] }"#)
            .unwrap_err();
    assert_eq!(err.field, "tiles[0].slope");
}

/// Each key of `value`, an instance of `schema` within `root`, against the
/// schema's properties, recursively; and each property the schema names
/// against the instance, so neither has a field the other lacks.
fn same_fields(value: &Value, schema: &Value, root: &Value, path: &str, seen: &mut Vec<String>) {
    let schema = match schema.get("$ref").and_then(Value::as_str) {
        Some(reference) => {
            let name = reference.trim_start_matches("#/$defs/");
            &root["$defs"][name]
        }
        None => schema,
    };
    match value {
        Value::Object(fields) => {
            let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
                return;
            };
            for (key, field) in fields {
                let property = properties
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}.{key}: not in the schema"));
                seen.push(format!("{path}.{key}"));
                same_fields(field, property, root, &format!("{path}.{key}"), seen);
            }
            for key in properties.keys() {
                assert!(
                    fields.contains_key(key) || path.contains('[') || path.is_empty(),
                    "{path}.{key}: the schema names a field the model lacks"
                );
            }
        }
        Value::Array(items) => {
            if let Some(items_schema) = schema.get("items") {
                for (at, item) in items.iter().enumerate() {
                    same_fields(item, items_schema, root, &format!("{path}[{at}]"), seen);
                }
            }
        }
        _ => {}
    }
}

#[test]
fn the_schemas_name_every_field() {
    let room: Value = serde_json::from_str(&Room::from_json(FULL).unwrap().to_canonical()).unwrap();
    let schema = json_schema();
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    let mut seen = Vec::new();
    same_fields(&room, &schema, &schema, "", &mut seen);
    // Every property of the room and of each item's definition appears in
    // the full room, so the schema lists no field the model lacks.
    let top: Vec<&String> = schema["properties"].as_object().unwrap().keys().collect();
    for key in top {
        assert!(room.get(key).is_some(), "the full room sets `{key}`");
    }
    for (definition, list) in [
        ("layer", "layers"),
        ("shape", "shapes"),
        ("object", "objects"),
        ("place", "places"),
    ] {
        for key in schema["$defs"][definition]["properties"]
            .as_object()
            .unwrap()
            .keys()
        {
            let set = room[list]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item.get(key).is_some());
            assert!(set, "some {definition} in the full room sets `{key}`");
        }
    }
    let tileset: Value =
        serde_json::from_str(&Tileset::from_json(FULL_TILESET).unwrap().to_canonical()).unwrap();
    let schema = tileset::json_schema();
    same_fields(&tileset, &schema, &schema, "", &mut seen);
    for key in schema["$defs"]["tile"]["properties"]
        .as_object()
        .unwrap()
        .keys()
    {
        let set = tileset["tiles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tile| tile.get(key).is_some());
        assert!(set, "some tile in the full tileset sets `{key}`");
    }
    assert!(seen.iter().any(|path| path == ".places[2].by"), "{seen:?}");
}

mod spawning {
    use glam::{Vec2, Vec3};
    use serde::Deserialize;
    use serde_json::json;

    use super::*;
    use crate::assets::{PrefabData, PrefabRegistry, TilemapRegistry};
    use crate::ecs::{Entity, World};
    use crate::lighting::AmbientLight;
    use crate::post::{PostPass, PostStack};
    use crate::save::{AreaError, SaveId, SaveState, Saved};

    #[derive(Debug, Clone, PartialEq, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Health {
        max: u32,
    }

    /// What a spawn's `with` gives.
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Marker(u32);

    /// A World with the prefab `crate` and the cave tileset.
    fn world() -> World {
        let mut world = World::new();
        world.register_component::<Health>("health");
        let mut prefabs = PrefabRegistry::new();
        let crate_prefab: PrefabData = serde_json::from_value(
            json!({ "tag": "crate", "components": { "health": { "max": 3 } } }),
        )
        .unwrap();
        prefabs.insert("crate".into(), crate_prefab);
        world.insert_resource(prefabs);
        let mut rooms = RoomRegistry::new();
        rooms.insert_tileset("cave", Tileset::from_json(FULL_TILESET).unwrap());
        world.insert_resource(rooms);
        world
    }

    /// An 8 × 4 room on the cave tileset with two crates.
    fn crates(extra: &str) -> Room {
        Room::from_json(&format!(
            r#"{{ "version": 1, "cell": [16, 16], "size": [8, 4], "tileset": "cave",
                 "layers": [{{ "name": "solid", "kind": "collision" }}],
                 "shapes": [{{ "id": "floor", "rect": [0, 3, 8, 1], "tile": "rock", "layers": ["solid"] }}],
                 "objects": [{{ "id": "crate_1", "prefab": "crate", "at": [1, 2] }},
                             {{ "id": "crate_2", "prefab": "crate", "at": [5.5, 2] }}],
                 "places": [{{ "id": "start", "point": [0.5, 2] }}] {extra} }}"#
        ))
        .unwrap()
    }

    fn name_of(world: &World, entity: Entity) -> String {
        let saved = world.get::<Saved>(entity).expect("named");
        world
            .resource::<SaveState>()
            .name_of(saved.id())
            .unwrap()
            .to_owned()
    }

    #[test]
    fn spawn_room_names_each_object_room_dot_id_and_returns_them_by_id() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        assert!(world.resource::<TilemapRegistry>().get("cave").is_some());
        let spawned = world
            .spawn_room("cave", [100.0, 50.0], &(Marker(1),))
            .unwrap();
        let ids: Vec<&str> = spawned.objects().map(|(id, _)| id).collect();
        assert_eq!(ids, ["crate_1", "crate_2"]);
        let first = spawned.object("crate_1").unwrap();
        assert_eq!(name_of(&world, first), "cave.crate_1");
        assert_eq!(world.get::<Saved>(first).unwrap().area(), Some("cave"));
        assert_eq!(
            world
                .get::<crate::components::Transform>(first)
                .unwrap()
                .position,
            Vec2::new(116.0, 82.0),
            "the origin plus at × cell"
        );
        assert_eq!(
            world.get::<Health>(spawned.object("crate_2").unwrap()),
            Some(&Health { max: 3 })
        );
        assert_eq!(
            spawned.place("start").unwrap().shape,
            PlaceShape::Point(Vec2::new(108.0, 82.0)),
            "a place in world pixels"
        );
        assert_eq!(
            world
                .resource::<RoomRegistry>()
                .place("cave", "start")
                .unwrap()
                .shape,
            PlaceShape::Point(Vec2::new(8.0, 32.0)),
            "in the room's own"
        );
        assert_eq!(
            world.resource::<RoomRegistry>().live("cave"),
            Some(spawned.instance())
        );
        assert!(world.resource::<SaveState>().is_loaded("cave"));
    }

    #[test]
    fn two_rooms_with_the_same_ids_spawn_side_by_side() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        world.insert_room("den", crates("")).unwrap();
        let cave = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        let den = world
            .spawn_room("den", [200.0, 0.0], &(Marker(2),))
            .unwrap();
        let (a, b) = (
            cave.object("crate_1").unwrap(),
            den.object("crate_1").unwrap(),
        );
        assert_ne!(a, b);
        assert_eq!(name_of(&world, a), "cave.crate_1");
        assert_eq!(name_of(&world, b), "den.crate_1");
        assert_ne!(
            world.get::<Saved>(a).unwrap().id(),
            world.get::<Saved>(b).unwrap().id()
        );
        assert!(world.is_alive(a) && world.is_alive(b));
    }

    #[test]
    fn spawning_a_live_room_again_fails_and_spawns_nothing() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        let count = world.entity_count();
        let err = world
            .spawn_room("cave", [300.0, 0.0], &(Marker(1),))
            .unwrap_err();
        assert!(
            matches!(err, AreaError::Live { ref room } if room == "cave"),
            "{err}"
        );
        assert_eq!(world.entity_count(), count, "nothing spawned");
        assert!(matches!(
            world.spawn_room("cove", [0.0, 0.0], &(Marker(1),)),
            Err(AreaError::UnknownRoom { .. })
        ));
    }

    #[test]
    fn a_room_whose_objects_all_died_still_refuses_a_second_spawn() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        let spawned = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        for (_, entity) in spawned.objects() {
            world.despawn(entity);
        }
        assert!(matches!(
            world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)),
            Err(AreaError::Live { .. })
        ));
    }

    #[test]
    fn a_room_spawns_again_once_its_instance_is_gone() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        let first = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        world.despawn(first.instance());
        for (_, entity) in first.objects() {
            world.despawn(entity);
        }
        let second = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        assert_ne!(second.instance(), first.instance());
        let again = second.object("crate_1").unwrap();
        assert_eq!(
            name_of(&world, again),
            "cave.crate_1",
            "the name taken again"
        );
        assert_eq!(
            world
                .resource::<SaveState>()
                .named_entity(SaveId::authored("cave.crate_1")),
            Some(again)
        );
    }

    #[test]
    fn spawn_room_sets_the_rooms_post_stack_and_ambient_light() {
        let mut world = world();
        let lit =
            crates(r#", "post": [{ "kind": "pixelate", "params": 4 }], "ambient": [0.25, 0.5, 1]"#);
        world.insert_room("lit", lit).unwrap();
        world.insert_room("plain", crates("")).unwrap();
        world.insert_resource(AmbientLight(Vec3::ONE));
        world
            .spawn_room("plain", [0.0, 0.0], &(Marker(1),))
            .unwrap();
        assert!(
            !world.has_resource::<PostStack>(),
            "a room without them leaves them"
        );
        assert_eq!(world.resource::<AmbientLight>().0, Vec3::ONE);
        world.spawn_room("lit", [0.0, 0.0], &(Marker(1),)).unwrap();
        assert_eq!(world.resource::<PostStack>().0, [PostPass::Pixelate(4.0)]);
        assert_eq!(
            world.resource::<AmbientLight>().0,
            Vec3::new(0.25, 0.5, 1.0)
        );
    }

    #[test]
    fn spawn_room_gives_its_bundle_to_the_instance_and_each_object() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        world.insert_room("den", crates("")).unwrap();
        let before = world.counters().archetype_moves;
        let spawned = world.spawn_room("cave", [0.0, 0.0], &(Marker(7),)).unwrap();
        let with = world.counters().archetype_moves - before;
        assert_eq!(world.get::<Marker>(spawned.instance()), Some(&Marker(7)));
        for (id, entity) in spawned.objects() {
            assert_eq!(world.get::<Marker>(entity), Some(&Marker(7)), "{id}");
        }
        // The bundle rides each spawn's own move: a spawn with a bundle of
        // two moves as many times as one with a bundle of one.
        let before = world.counters().archetype_moves;
        world
            .spawn_room("den", [0.0, 0.0], &(Marker(7), Health { max: 9 }))
            .unwrap();
        assert_eq!(world.counters().archetype_moves - before, with);
    }

    /// Despawns `spawned`'s instance and each of its objects.
    fn despawn_all(world: &mut World, spawned: &SpawnedRoom) {
        world.despawn(spawned.instance());
        for (_, entity) in spawned.objects() {
            world.despawn(entity);
        }
    }

    #[test]
    fn remove_room_refuses_a_live_room() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        let spawned = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        for (_, entity) in spawned.objects() {
            world.despawn(entity);
        }
        let err = world.remove_room("cave").unwrap_err();
        assert!(
            matches!(err, AreaError::Live { ref room } if room == "cave"),
            "{err}"
        );
        assert_eq!(
            err.to_string(),
            "room `cave` is live; despawn its instance first"
        );
        assert!(world.resource::<RoomRegistry>().room("cave").is_some());
        assert!(world.resource::<TilemapRegistry>().get("cave").is_some());
        assert!(world.resource::<SaveState>().is_loaded("cave"));
        // An unknown room, and a tilemap's ID that no room registered.
        let map = world
            .resource::<TilemapRegistry>()
            .get("cave")
            .unwrap()
            .clone();
        world
            .resource_mut::<TilemapRegistry>()
            .insert("level".into(), map);
        for id in ["cove", "level"] {
            assert!(matches!(
                world.remove_room(id),
                Err(AreaError::UnknownRoom { ref room }) if room == id
            ));
        }
        assert!(world.resource::<TilemapRegistry>().get("level").is_some());
    }

    #[test]
    fn remove_room_refuses_while_an_entity_named_in_it_lives() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        let spawned = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        world.despawn(spawned.instance());
        let held = |world: &mut World| match world.remove_room("cave") {
            Err(AreaError::Held { room, name }) => {
                assert_eq!(room, "cave");
                name
            }
            other => panic!("expected Held, got {other:?}"),
        };
        assert_eq!(held(&mut world), "cave.crate_1", "the first by name");
        world.despawn(spawned.object("crate_1").unwrap());
        assert_eq!(held(&mut world), "cave.crate_2");
        let err = world.remove_room("cave").unwrap_err();
        assert_eq!(
            err.to_string(),
            "room `cave`'s area names `cave.crate_2`, which lives; despawn it before removing the room"
        );
        assert!(world.resource::<RoomRegistry>().room("cave").is_some());
        assert!(world.resource::<TilemapRegistry>().get("cave").is_some());
        // An entity named in another area holds nothing here.
        let other = world.spawn_with((Marker(2),));
        crate::save::name(&mut world, other, Some("den"), "den.lamp").unwrap();
        world.despawn(spawned.object("crate_2").unwrap());
        assert!(world.remove_room("cave").is_ok());
        assert_eq!(
            world
                .resource::<SaveState>()
                .named_entity(SaveId::authored("den.lamp")),
            Some(other)
        );
    }

    #[test]
    fn remove_room_drops_the_room_its_map_and_its_journal() {
        let mut world = world();
        let room = crates("");
        world
            .insert_room_with_path("cave", room.clone(), "rooms/cave.json".into())
            .unwrap();
        world.insert_room("den", crates("")).unwrap();
        let mut journal = RoomJournal::new();
        journal.record("cave", Vec::new(), Vec::new());
        journal.record("den", Vec::new(), Vec::new());
        world.insert_resource(journal);
        let spawned = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        despawn_all(&mut world, &spawned);
        assert_eq!(world.remove_room("cave").unwrap(), room, "its document");
        let rooms = world.resource::<RoomRegistry>();
        assert!(rooms.room("cave").is_none() && rooms.compiled("cave").is_none());
        assert!(rooms.path("cave").is_none() && rooms.live("cave").is_none());
        assert_eq!(rooms.ids().collect::<Vec<_>>(), ["den"]);
        assert!(rooms.tileset("cave").is_some(), "its tileset stays");
        assert!(world.resource::<TilemapRegistry>().get("cave").is_none());
        assert!(world.resource::<TilemapRegistry>().get("den").is_some());
        let journal = world.resource::<RoomJournal>();
        assert_eq!((journal.undo_len("cave"), journal.undo_len("den")), (0, 1));
        assert!(matches!(
            world.remove_room("cave"),
            Err(AreaError::UnknownRoom { .. })
        ));
    }

    #[test]
    fn remove_room_forgets_its_areas_names() {
        use crate::save::{capture_world, restore_world};
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        world.insert_room("den", crates("")).unwrap();
        let cave = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        world
            .spawn_room("den", [200.0, 0.0], &(Marker(1),))
            .unwrap();
        let save = capture_world(&world).unwrap();
        despawn_all(&mut world, &cave);
        let areas = |world: &World| -> Vec<String> {
            capture_world(world)
                .unwrap()
                .area_names()
                .map(str::to_owned)
                .collect()
        };
        assert_eq!(
            areas(&world),
            ["cave", "den"],
            "the dead crates read as gone"
        );
        world.remove_room("cave").unwrap();
        let state = world.resource::<SaveState>();
        assert!(!state.is_loaded("cave") && state.is_loaded("den"));
        for name in ["cave.crate_1", "cave.crate_2"] {
            assert_eq!(state.name_of(SaveId::authored(name)), None, "{name}");
        }
        assert!(state.name_of(SaveId::authored("den.crate_1")).is_some());
        assert_eq!(areas(&world), ["den"]);
        // A kept area goes too: a restore keeps `den`, which this World has
        // not spawned, and its removal forgets the record.
        let mut other = self::world();
        assert!(!other.has_resource::<SaveState>());
        other.insert_room("cave", crates("")).unwrap();
        other.insert_room("den", crates("")).unwrap();
        other.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        restore_world(&mut other, &save, &(Marker(1),)).unwrap();
        assert!(other.resource::<SaveState>().is_kept("den"));
        assert_eq!(areas(&other), ["cave", "den"]);
        other.remove_room("den").unwrap();
        assert!(!other.resource::<SaveState>().is_kept("den"));
        assert_eq!(areas(&other), ["cave"]);
    }

    #[test]
    fn a_removed_room_inserts_and_spawns_again() {
        let mut world = world();
        world.insert_room("cave", crates("")).unwrap();
        let first = world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)).unwrap();
        despawn_all(&mut world, &first);
        world.remove_room("cave").unwrap();
        assert!(matches!(
            world.spawn_room("cave", [0.0, 0.0], &(Marker(1),)),
            Err(AreaError::UnknownRoom { .. })
        ));
        world.insert_room("cave", crates("")).unwrap();
        assert!(world.resource::<TilemapRegistry>().get("cave").is_some());
        let second = world
            .spawn_room("cave", [64.0, 0.0], &(Marker(1),))
            .unwrap();
        let again = second.object("crate_1").unwrap();
        assert_eq!(name_of(&world, again), "cave.crate_1");
        assert_eq!(
            world.resource::<RoomRegistry>().live("cave"),
            Some(second.instance())
        );
        assert!(world.resource::<SaveState>().is_loaded("cave"));
    }
}
