//! A room's level rules: spawns clear of the terrain as contacts meet it,
//! props on what supports them, and budgets.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;
use crate::assets::registry::TextureHandle;
use crate::assets::{FilterMode, Room, Tileset, UvRect};

/// A World with the physics layers `terrain` (1), `player` (2) and
/// `enemy` (3), the sprites `lamp` (8 × 16), `post` (8 × 16, anchored at
/// its bottom's middle), `chain` (4 × 12) and `torch` (6 × 8), and the
/// prefabs `prefabs` gives.
fn world(prefabs: &[(&str, Value)]) -> World {
    let mut world = World::new();
    world.insert_resource(PhysicsLayers::new([
        ("terrain".to_owned(), 1),
        ("player".to_owned(), 2),
        ("enemy".to_owned(), 3),
    ]));
    let mut sprites = AssetRegistry::new();
    for (name, w, h) in [
        ("lamp", 8, 16),
        ("post", 8, 16),
        ("chain", 4, 12),
        ("torch", 6, 8),
    ] {
        sprites.register_sprite(
            name.to_owned(),
            FilterMode::Nearest,
            w,
            h,
            PathBuf::from(format!("__test__/{name}.png")),
            TextureHandle(0),
            UvRect::FULL,
            None,
            None,
            None,
        );
    }
    sprites.set_sprite_frame("post", None, Some([4.0, 16.0]));
    world.insert_resource(sprites);
    let mut registry = PrefabRegistry::new();
    for (id, value) in prefabs {
        registry.insert(
            (*id).to_owned(),
            serde_json::from_value(value.clone()).unwrap(),
        );
    }
    world.insert_resource(registry);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset(
        "cave",
        Tileset::from_json(
            r#"{ "version": 1, "tiles": [
                { "sprite": "rock" },
                { "sprite": "plank", "one_way": true },
                { "sprite": "ramp_br", "slope": "45 br" }
            ] }"#,
        )
        .unwrap(),
    );
    world.insert_resource(rooms);
    world
}

/// An 8 × 6 room of 16 px cells: a floor along row 5 in `solid`, a layer
/// in `terrain` that meets `player`, and `extra` (shapes, cells, objects).
fn room(world: &mut World, extra: &Value) -> Vec<String> {
    let mut room = json!({
        "version": 1, "cell": [16, 16], "size": [8, 6], "tileset": "cave",
        "layers": [{ "name": "solid", "kind": "collision", "layers": ["terrain"], "mask": ["player"] }],
        "shapes": [{ "id": "floor", "rect": [0, 5, 8, 1], "tile": "rock", "layers": ["solid"] }]
    });
    for (key, value) in extra.as_object().unwrap() {
        match (key.as_str(), value) {
            ("shapes", Value::Array(more)) => {
                room["shapes"]
                    .as_array_mut()
                    .unwrap()
                    .extend(more.iter().cloned());
            }
            ("cells", cells) => room["layers"][0]["cells"] = cells.clone(),
            _ => room[key] = value.clone(),
        }
    }
    let room = Room::from_json(&room.to_string()).unwrap();
    world.insert_room("cave", room).unwrap();
    world
        .room_findings("cave", &["light"])
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn object(id: &str, prefab: &str, at: [f32; 2]) -> Value {
    json!({ "id": id, "prefab": prefab, "at": at })
}

fn body(kind: &str, shape: &Value, layers: &[&str], mask: &[&str]) -> Value {
    json!({ "components": { "rigid_body": {
        "kind": kind, "shape": shape, "layers": layers, "mask": mask } } })
}

#[test]
fn a_body_spawned_in_a_solid_cell_is_a_finding() {
    let ball = body(
        "dynamic",
        &json!({ "kind": "circle", "radius": 6 }),
        &["player"],
        &["terrain"],
    );
    let mut world = world(&[("ball", ball)]);
    // Its circle at (2.5, 4.75) cells, (40, 76) px, reaches 2 px into the
    // floor's top; at (2.5, 4) it clears it by 10.
    assert_eq!(
        room(
            &mut world,
            &json!({ "objects": [
            object("ball_1", "ball", [2.5, 4.75]),
            object("ball_2", "ball", [2.5, 4.0])
        ] })
        ),
        ["cave: ball_1: objects[0]: spawns inside the terrain: its collider overlaps cell (2, 5)"]
    );
    // An object's `set` reaches its body: a smaller circle clears.
    assert_eq!(
        room(
            &mut world,
            &json!({ "objects": [{
            "id": "ball_1", "prefab": "ball", "at": [2.5, 4.75],
            "set": { "rigid_body.shape.radius": 3 } }] })
        ),
        Vec::<String>::new()
    );
}

#[test]
fn an_asymmetric_mask_is_judged_as_contacts_are() {
    let circle = json!({ "kind": "circle", "radius": 6 });
    let mut world = world(&[
        (
            "both",
            body("dynamic", &circle.clone(), &["player"], &["terrain"]),
        ),
        // Its mask meets the terrain, but the terrain's mask does not meet it.
        (
            "enemy",
            body("dynamic", &circle.clone(), &["enemy"], &["terrain"]),
        ),
        // The terrain's mask meets it, but its mask does not meet the terrain.
        ("blind", body("dynamic", &circle, &["player"], &["enemy"])),
    ]);
    let found = room(
        &mut world,
        &json!({ "objects": [
        object("a", "both", [2.5, 4.75]),
        object("b", "enemy", [4.5, 4.75]),
        object("c", "blind", [6.5, 4.75])
    ] }),
    );
    assert_eq!(
        found,
        ["cave: a: objects[0]: spawns inside the terrain: its collider overlaps cell (2, 5)"],
        "both ways, as contacts need"
    );
}

#[test]
fn a_body_over_a_slope_is_judged_by_its_solid_part() {
    // A 45° ramp at (3, 4): solid below the diagonal from (48, 80) to
    // (64, 64). A 6 px box at (52, 68) sits over its empty part, one at
    // (58, 74) in its solid part.
    let crate_body = body(
        "kinematic",
        &json!({ "kind": "aabb", "half_extents": [3, 3] }),
        &["player"],
        &["terrain"],
    );
    let mut world = world(&[("crate", crate_body)]);
    let found = room(
        &mut world,
        &json!({
            "cells": { "3,4": "ramp_br" },
            "objects": [
                object("over", "crate", [3.25, 4.25]),
                object("in", "crate", [3.625, 4.625])
            ]
        }),
    );
    assert_eq!(
        found,
        ["cave: in: objects[1]: spawns inside the terrain: its collider overlaps cell (3, 4)"]
    );
}

#[test]
fn a_sensor_or_static_body_is_not_judged() {
    let circle = json!({ "kind": "circle", "radius": 6 });
    let mut world = world(&[
        (
            "zone",
            body("sensor", &circle.clone(), &["player"], &["terrain"]),
        ),
        ("pillar", body("static", &circle, &["player"], &["terrain"])),
    ]);
    assert_eq!(
        room(
            &mut world,
            &json!({ "objects": [
            object("zone_1", "zone", [2.0, 5.0]),
            object("pillar_1", "pillar", [4.0, 5.0])
        ] })
        ),
        Vec::<String>::new()
    );
}

fn prop(sprite: &str, support: &str) -> Value {
    json!({ "sprite": { "asset_id": sprite }, "support": support })
}

#[test]
fn a_ground_prop_over_a_gap_floats() {
    let mut world = world(&[("lamp", prop("lamp", "ground"))]);
    assert_eq!(
        room(
            &mut world,
            &json!({ "objects": [object("lamp_1", "lamp", [3.0, 3.0])] })
        ),
        ["cave: lamp_1: objects[0]: floats: 0 of 8 bottom columns have ground within 2 px"]
    );
    // Half over the floor's end: 4 of 8 columns, under 60 %.
    assert_eq!(
        room(
            &mut world,
            &json!({
                "shapes": [{ "id": "hole", "rect": [3, 5, 1, 1], "tile": null, "layers": ["solid"] }],
                "objects": [object("lamp_1", "lamp", [3.75, 4.0])]
            })
        ),
        ["cave: lamp_1: objects[0]: floats: 4 of 8 bottom columns have ground within 2 px"]
    );
    assert_eq!(
        room(
            &mut world,
            &json!({ "objects": [object("lamp_1", "lamp", [3.0, 3.9])] })
        ),
        Vec::<String>::new(),
        "1.6 px over the floor is within 2"
    );
}

#[test]
fn a_bottom_anchored_prop_on_a_floor_is_supported() {
    let mut world = world(&[("post", prop("post", "ground"))]);
    // The anchor, the bottom's middle, lands on the floor's top: drawn from
    // (48, 64) to (56, 80), not from its point down.
    assert_eq!(
        room(
            &mut world,
            &json!({ "objects": [object("post_1", "post", [3.25, 5.0])] })
        ),
        Vec::<String>::new()
    );
    // A one-way plank holds a ground prop up too.
    assert_eq!(
        room(
            &mut world,
            &json!({
                "cells": { "3,2": "plank" },
                "objects": [object("post_1", "post", [3.25, 2.0])]
            })
        ),
        Vec::<String>::new()
    );
}

#[test]
fn a_ground_prop_sunk_past_6_px_is_a_finding() {
    let mut world = world(&[("lamp", prop("lamp", "ground"))]);
    assert_eq!(
        room(
            &mut world,
            &json!({ "objects": [object("lamp_1", "lamp", [3.0, 4.5])] })
        ),
        [
            "cave: lamp_1: objects[0]: sunk: 8 of 8 bottom columns are more than 6 px into the ground"
        ]
    );
    assert_eq!(
        room(
            &mut world,
            &json!({ "objects": [object("lamp_1", "lamp", [3.0, 4.25])] })
        ),
        Vec::<String>::new(),
        "4 px in"
    );
}

#[test]
fn an_under_prop_hangs_within_2_px() {
    let mut world = world(&[("chain", prop("chain", "under"))]);
    let ceiling =
        json!([{ "id": "roof", "rect": [0, 0, 8, 1], "tile": "rock", "layers": ["solid"] }]);
    assert_eq!(
        room(
            &mut world,
            &json!({ "shapes": ceiling, "objects": [object("chain_1", "chain", [3.0, 1.0])] })
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        room(
            &mut world,
            &json!({ "shapes": ceiling, "objects": [object("chain_1", "chain", [3.0, 1.25])] })
        ),
        ["cave: chain_1: objects[0]: hangs free: 0 of 4 top columns have a ceiling within 2 px"]
    );
}

#[test]
fn a_wall_prop_touches_a_side_within_3_px() {
    let mut world = world(&[("torch", prop("torch", "wall"))]);
    let wall = json!([{ "id": "wall", "rect": [0, 0, 1, 5], "tile": "rock", "layers": ["solid"] }]);
    assert_eq!(
        room(
            &mut world,
            &json!({ "shapes": wall, "objects": [object("torch_1", "torch", [1.0, 2.0])] })
        ),
        Vec::<String>::new()
    );
    // Its right side touches a wall too.
    assert_eq!(
        room(
            &mut world,
            &json!({
                "shapes": [{ "id": "wall", "rect": [5, 0, 1, 5], "tile": "rock", "layers": ["solid"] }],
                "objects": [object("torch_1", "torch", [4.625, 2.0])]
            })
        ),
        Vec::<String>::new()
    );
    assert_eq!(
        room(
            &mut world,
            &json!({ "shapes": wall, "objects": [object("torch_1", "torch", [1.5, 2.0])] })
        ),
        ["cave: torch_1: objects[0]: stands free: 0 of 8 rows touch a wall within 3 px"]
    );
}

#[test]
fn lights_and_objects_past_the_budget_are_findings() {
    let lamp =
        json!({ "components": { "light": { "kind": "point", "radius": 32, "color": [1, 1, 1] } } });
    let mut world = world(&[("lamp", lamp), ("rock", json!({ "tag": "rock" }))]);
    let found = room(
        &mut world,
        &json!({
            "objects": [
                object("lamp_1", "lamp", [1.0, 4.0]),
                object("lamp_2", "lamp", [2.0, 4.0]),
                object("lamp_3", "lamp", [3.0, 4.0]),
                object("rock_1", "rock", [4.0, 4.0]),
                object("rock_2", "rock", [5.0, 4.0])
            ],
            "budget": { "lights": 2, "objects": 4 }
        }),
    );
    assert_eq!(
        found,
        [
            "cave: lamp_3: objects[2]: budget: 3 objects hold a light, past budget.lights 2",
            "cave: rock_2: objects[4]: budget: 5 objects, past budget.objects 4"
        ]
    );
    // The defaults: 16 lights.
    let lamps: Vec<Value> = (0..17)
        .map(|i| object(&format!("lamp_{i}"), "lamp", [0.25 + i as f32 * 0.25, 4.0]))
        .collect();
    let found = room(&mut world, &json!({ "objects": lamps }));
    assert_eq!(
        found,
        ["cave: lamp_16: objects[16]: budget: 17 objects hold a light, past budget.lights 16"]
    );
}
