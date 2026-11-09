//! Route reach on small rooms drawn as text: `#` rock, `-` a one-way
//! plank, `/` and `\` 45° floor slopes rising right and left, `S` and `E`
//! a route's two stops.

use serde_json::json;
use tungsten_core::assets::{PrefabData, Room, Tileset};

use super::*;

/// Hero numbers: 12 × 20 px box, 120 px/s run, jump 300 at gravity 900, a
/// 50 px rise and 80 px of air run on the level.
fn hero(controller: &Value, body: &Value) -> PrefabData {
    let mut controller_value = json!({ "run_speed": 120, "jump_speed": 300, "coyote_time": 0.1 });
    for (key, value) in controller.as_object().unwrap() {
        controller_value[key] = value.clone();
    }
    let mut body_value =
        json!({ "kind": "kinematic", "shape": { "kind": "aabb", "half_extents": [6, 10] } });
    for (key, value) in body.as_object().unwrap() {
        body_value[key] = value.clone();
    }
    serde_json::from_value(json!({
        "components": { "platformer_controller": controller_value, "rigid_body": body_value }
    }))
    .unwrap()
}

/// A World with gravity 900, the physics layers `terrain` (1) and `player`
/// (2), and the prefabs `hero`, `bug` (a top-down circle) and `rock`.
fn make_world(prefabs: &[(&str, PrefabData)]) -> World {
    let mut world = World::new();
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, 900.0),
        ..PhysicsConfig::default()
    });
    world.insert_resource(PhysicsLayers::new([
        ("terrain".to_owned(), 1),
        ("player".to_owned(), 2),
    ]));
    let mut registry = PrefabRegistry::new();
    registry.insert("hero".into(), hero(&json!({}), &json!({})));
    let bug: PrefabData = serde_json::from_value(json!({ "components": {
        "top_down_mover": { "speed": 100 },
        "rigid_body": { "kind": "dynamic", "shape": { "kind": "circle", "radius": 6 } } } }))
    .unwrap();
    registry.insert("bug".into(), bug);
    let rock: PrefabData = serde_json::from_value(json!({ "tag": "rock" })).unwrap();
    registry.insert("rock".into(), rock);
    for (id, data) in prefabs {
        registry.insert((*id).to_owned(), data.clone());
    }
    world.insert_resource(registry);
    let mut rooms = RoomRegistry::new();
    rooms.insert_tileset(
        "cave",
        Tileset::from_json(
            r#"{ "version": 1, "tiles": [
                { "sprite": "rock" },
                { "sprite": "plank", "one_way": true },
                { "sprite": "ramp_br", "slope": "45 br" },
                { "sprite": "ramp_bl", "slope": "45 bl" }
            ] }"#,
        )
        .unwrap(),
    );
    world.insert_resource(rooms);
    world
}

/// Room `picture` on `world`, its route from `S` to `E` by `by`, its
/// collision layer in `terrain` and meeting everything.
fn findings(world: &mut World, picture: &[&str], by: &str) -> Vec<String> {
    let rows = picture.len();
    let cols = picture[0].len();
    let mut cells = serde_json::Map::new();
    let (mut start, mut end) = ([0.0, 0.0], [0.0, 0.0]);
    for (row, line) in picture.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            let tile = match ch {
                '#' => "rock",
                '-' => "plank",
                '/' => "ramp_br",
                '\\' => "ramp_bl",
                'S' => {
                    start = [col as f32 + 0.5, row as f32 + 0.5];
                    continue;
                }
                'E' => {
                    end = [col as f32 + 0.5, row as f32 + 0.5];
                    continue;
                }
                _ => continue,
            };
            cells.insert(format!("{col},{row}"), json!(tile));
        }
    }
    let room = Room::from_json(
        &json!({
            "version": 1, "cell": [16, 16], "size": [cols, rows], "tileset": "cave",
            "layers": [{ "name": "solid", "kind": "collision", "layers": ["terrain"], "cells": cells }],
            "places": [{ "id": "main", "route": [start, end], "by": by }]
        })
        .to_string(),
    )
    .unwrap();
    world.insert_room("cave", room).unwrap();
    route_findings(world, "cave")
        .iter()
        .map(ToString::to_string)
        .collect()
}

const OUT: &str = "cave: main: places[0].route[1]: out of reach of route[0] for `hero`";

#[test]
fn a_gap_wider_than_the_jump_is_out_of_reach() {
    let mut world = make_world(&[]);
    let near = [
        "............",
        "............",
        "............",
        "...S....E...",
        "#####...####",
    ];
    assert_eq!(
        findings(&mut world, &near, "hero"),
        Vec::<String>::new(),
        "a 3-cell gap"
    );
    let far = [
        "..............",
        "..............",
        "..............",
        "..S........E..",
        "####.......###",
    ];
    assert_eq!(findings(&mut world, &far, "hero"), [OUT], "a 7-cell gap");
}

#[test]
fn a_ledge_above_the_jump_is_out_of_reach_until_an_air_jump() {
    let ledge = [
        "..........",
        "......E...",
        "......####",
        "..........",
        "..........",
        "..........",
        "..S.......",
        "##########",
    ];
    let mut world = make_world(&[]);
    assert_eq!(
        findings(&mut world, &ledge, "hero"),
        [OUT],
        "an 80 px ledge over a 50 px jump"
    );
    let mut world = make_world(&[("hero", hero(&json!({ "air_jumps": 1 }), &json!({})))]);
    assert_eq!(findings(&mut world, &ledge, "hero"), Vec::<String>::new());
}

#[test]
fn a_ceiling_over_the_arc_blocks_the_jump() {
    let open = [
        "............",
        "............",
        "............",
        "...S....E...",
        "#####...####",
    ];
    let low = [
        "............",
        "############",
        "............",
        "...S....E...",
        "#####...####",
    ];
    let mut world = make_world(&[]);
    assert_eq!(findings(&mut world, &open, "hero"), Vec::<String>::new());
    assert_eq!(
        findings(&mut world, &low, "hero"),
        [OUT],
        "a 32 px ceiling over a 50 px arc"
    );
}

#[test]
fn a_one_way_ledge_is_reached_from_below() {
    let planks = [
        "..........",
        "..........",
        "....E.....",
        "----------",
        "..........",
        "....S.....",
        "##########",
    ];
    let rock = [
        "..........",
        "..........",
        "....E.....",
        "##########",
        "..........",
        "....S.....",
        "##########",
    ];
    // 330 rises 57.75 px at 60 Hz, past the plank's 48.
    let high = hero(&json!({ "jump_speed": 330 }), &json!({}));
    let mut world = make_world(&[("hero", high)]);
    assert_eq!(findings(&mut world, &planks, "hero"), Vec::<String>::new());
    assert_eq!(
        findings(&mut world, &rock, "hero"),
        [OUT],
        "a rock ledge over the start is met from below"
    );
}

const RAMP: [&str; 8] = [
    "............",
    "............",
    "............",
    "..........E.",
    "......../###",
    "......./####",
    "...S../#####",
    "############",
];

#[test]
fn a_walk_up_a_floor_slope_reaches_its_top() {
    // Too high for the jump: 48 px over 50 px of rise needs the ramp's
    // faces, which the default floor rule takes at 45°.
    let mut world = make_world(&[("hero", hero(&json!({ "jump_speed": 200 }), &json!({})))]);
    assert_eq!(findings(&mut world, &RAMP, "hero"), Vec::<String>::new());
}

#[test]
fn a_slope_past_the_slope_limit_or_sliding_is_no_ground() {
    let steep = hero(&json!({ "jump_speed": 200, "slope_limit": 40 }), &json!({}));
    let mut world = make_world(&[("hero", steep)]);
    assert_eq!(
        findings(&mut world, &RAMP, "hero"),
        [OUT],
        "45° is past a 40° floor"
    );
    let sliding = hero(
        &json!({ "jump_speed": 200, "slope_limit": 60, "slide_limit": 30 }),
        &json!({}),
    );
    let mut world = make_world(&[("hero", sliding)]);
    assert_eq!(
        findings(&mut world, &RAMP, "hero"),
        [OUT],
        "a floor it slides down"
    );
}

#[test]
fn a_step_past_step_height_is_no_walk() {
    let step = [
        "........", "........", "........", "..S..E..", ".....###", "########",
    ];
    // A jump of a pixel's height: only a step climbs the 16 px cell.
    let short = hero(&json!({ "jump_speed": 20, "step_height": 8 }), &json!({}));
    let mut world = make_world(&[("hero", short)]);
    assert_eq!(findings(&mut world, &step, "hero"), [OUT]);
    let tall = hero(&json!({ "jump_speed": 20, "step_height": 16 }), &json!({}));
    let mut world = make_world(&[("hero", tall)]);
    assert_eq!(findings(&mut world, &step, "hero"), Vec::<String>::new());
}

#[test]
fn a_top_down_route_round_a_wall_is_reachable_and_a_sealed_one_is_not() {
    let round = [
        "##########",
        "#........#",
        "#.S.#..E.#",
        "#...#....#",
        "#...#....#",
        "##########",
    ];
    let sealed = [
        "##########",
        "#...#....#",
        "#.S.#..E.#",
        "#...#....#",
        "#...#....#",
        "##########",
    ];
    let mut world = make_world(&[]);
    assert_eq!(findings(&mut world, &round, "bug"), Vec::<String>::new());
    assert_eq!(
        findings(&mut world, &sealed, "bug"),
        ["cave: main: places[0].route[1]: out of reach of route[0] for `bug`"]
    );
}

#[test]
fn reach_follows_the_controllers_filter_where_it_differs_from_the_contact_masks() {
    let flat = ["........", "........", "..S..E..", "########"];
    // Contacts would miss the floor (the body meets `player` alone), but the
    // controller's casts see every layer: reachable.
    let ghost = hero(&json!({}), &json!({ "mask": ["player"] }));
    let mut world = make_world(&[("hero", ghost)]);
    assert_eq!(findings(&mut world, &flat, "hero"), Vec::<String>::new());
    // The body would meet the floor, but the casts skip `terrain`: nothing
    // to stand on.
    let skipper = hero(&json!({ "without": ["terrain"] }), &json!({}));
    let mut world = make_world(&[("hero", skipper)]);
    let found = findings(&mut world, &flat, "hero");
    assert_eq!(
        found,
        [
            "cave: main: places[0].route[0]: nothing to stand on within 2 cells under it",
            "cave: main: places[0].route[1]: nothing to stand on within 2 cells under it"
        ]
    );
}

#[test]
fn a_point_with_nothing_to_stand_on_is_a_finding() {
    let mid_air = [
        "..E.....", "........", "........", "........", "..S.....", "########",
    ];
    let mut world = make_world(&[]);
    assert_eq!(
        findings(&mut world, &mid_air, "hero"),
        ["cave: main: places[0].route[1]: nothing to stand on within 2 cells under it"]
    );
    // A `by` with no mover, and one naming no prefab.
    assert_eq!(
        findings(&mut world, &mid_air, "rock"),
        ["cave: main: places[0].by: prefab `rock` has no platformer_controller or top_down_mover"]
    );
    assert_eq!(
        findings(&mut world, &mid_air, "ghost"),
        ["cave: main: places[0].by: no prefab `ghost`"]
    );
}
