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
