//! Compiling a room against its tileset: shapes, slopes, lines, cell
//! overrides, objects, layer names and determinism.

use serde_json::{Value, json};

use super::super::tests::{FULL, FULL_TILESET};
use super::*;
use crate::assets::tilemap::TileProps;

/// The cave tileset: style `rock` with its brush and four slope tiles, a
/// crack, a plank and an arch.
fn tileset() -> Tileset {
    Tileset::from_json(
        r#"{ "version": 1, "tiles": [
            { "sprite": "rock", "style": "rock" },
            { "sprite": "rock_45_br", "style": "rock", "slope": "45 br" },
            { "sprite": "rock_45_tl", "style": "rock", "slope": "45 tl" },
            { "sprite": "rock_low_br", "style": "rock", "slope": "2:1 low br" },
            { "sprite": "rock_high_br", "style": "rock", "slope": "2:1 high br" },
            { "sprite": "rock_crack" },
            { "sprite": "plank", "one_way": true },
            { "sprite": "arch", "whole": true }
        ] }"#,
    )
    .unwrap()
}

const ROCK: TileIndex = 0;
const ROCK_45_BR: TileIndex = 1;
const ROCK_45_TL: TileIndex = 2;
const ROCK_LOW_BR: TileIndex = 3;
const ROCK_HIGH_BR: TileIndex = 4;
const ROCK_CRACK: TileIndex = 5;
const PLANK: TileIndex = 6;

fn layers() -> PhysicsLayers {
    PhysicsLayers::new([
        ("terrain".to_owned(), 1),
        ("player".to_owned(), 2),
        ("enemy".to_owned(), 3),
    ])
}

/// A `cols` × `rows` room of 16 px cells on the cave tileset, with the
/// render layer `art`, the collision layer `solid` and `shapes`.
fn room(cols: u32, rows: u32, shapes: &Value) -> Room {
    Room::from_json(
        &json!({
            "version": 1,
            "cell": [16, 16],
            "size": [cols, rows],
            "tileset": "cave",
            "layers": [{ "name": "art" }, { "name": "solid", "kind": "collision" }],
            "shapes": shapes,
        })
        .to_string(),
    )
    .unwrap()
}

fn compile(room: &Room) -> Result<CompiledRoom, RoomError> {
    room.compile("cave", &tileset(), &layers())
}

/// Every non-empty cell of `layer`, as (col, row, tile).
fn painted(map: &TilemapData, layer: usize) -> Vec<(u32, u32, TileIndex)> {
    let mut cells = Vec::new();
    for row in 0..map.height {
        for col in 0..map.width {
            let tile = map.tile_at(layer, col, row).unwrap();
            if tile >= 0 {
                cells.push((col, row, tile));
            }
        }
    }
    cells
}

#[test]
fn a_rect_paints_each_layer_it_names() {
    let room = room(
        4,
        4,
        &json!([
            { "id": "floor", "rect": [0, 2, 4, 1], "tile": "rock", "layers": ["art", "solid"] },
            { "id": "back", "rect": [1, 0, 1, 1], "tile": "plank", "layers": ["art"] }
        ]),
    );
    let map = compile(&room).unwrap().map;
    let floor: Vec<_> = (0..4).map(|col| (col, 2, ROCK)).collect();
    let mut art = vec![(1, 0, PLANK)];
    art.extend(&floor);
    assert_eq!(painted(&map, 0), art);
    assert_eq!(painted(&map, 1), floor, "the collision layer, rect alone");
    assert_eq!(map.layers[1].kind, LayerKind::Collision);
    assert_eq!(
        map.tileset[..2],
        ["rock", "rock_45_br"],
        "the tileset in file order"
    );
    assert!(map.tile_props(PLANK).one_way);
    // A rect past the room fails naming its shape and the cell.
    let past = room_with(&json!([
        { "id": "wide", "rect": [2, 0, 3, 1], "tile": "rock", "layers": ["art"] }
    ]));
    assert_eq!(
        compile(&past).unwrap_err().to_string(),
        "shapes[0].rect: cell (4, 0) is past the room's 4 \u{d7} 4 cells"
    );
}

fn room_with(shapes: &Value) -> Room {
    room(4, 4, shapes)
}

#[test]
fn a_later_shape_paints_over_an_earlier_and_null_erases() {
    let room = room_with(&json!([
        { "id": "floor", "rect": [0, 2, 4, 2], "tile": "rock", "layers": ["art"] },
        { "id": "boards", "rect": [1, 2, 2, 1], "tile": "plank", "layers": ["art"] },
        { "id": "hole", "rect": [3, 3, 1, 1], "tile": null, "layers": ["art"] }
    ]));
    let map = compile(&room).unwrap().map;
    assert_eq!(
        painted(&map, 0),
        [
            (0, 2, ROCK),
            (1, 2, PLANK),
            (2, 2, PLANK),
            (3, 2, ROCK),
            (0, 3, ROCK),
            (1, 3, ROCK),
            (2, 3, ROCK)
        ]
    );
}

#[test]
fn cell_overrides_paint_over_the_shapes() {
    let mut room = room_with(&json!([
        { "id": "floor", "rect": [0, 2, 4, 1], "tile": "rock", "layers": ["art", "solid"] }
    ]));
    room.layers[1].cells = [
        ("1,2".to_owned(), Some("rock_crack".to_owned())),
        ("2,2".to_owned(), None),
        ("0,0".to_owned(), Some("plank".to_owned())),
    ]
    .into_iter()
    .collect();
    let map = compile(&room).unwrap().map;
    assert_eq!(
        painted(&map, 1),
        [
            (0, 0, PLANK),
            (0, 2, ROCK),
            (1, 2, ROCK_CRACK),
            (3, 2, ROCK)
        ]
    );
    assert_eq!(painted(&map, 0).len(), 4, "the other layer keeps the shape");
    room.layers[1].cells = [("1,2".to_owned(), Some("gravel".to_owned()))].into();
    assert_eq!(
        compile(&room).unwrap_err().to_string(),
        "layers[1].cells.1,2: no tile `gravel` in the tileset; its tiles are `rock`, `rock_45_br`, `rock_45_tl`, `rock_low_br`, `rock_high_br`, `rock_crack`, `plank`, `arch`"
    );
}

#[test]
fn a_missing_tile_names_the_tilesets_tiles() {
    let message = |tile: &str, tileset: &Tileset| {
        let mut room: Room = serde_json::from_value(json!({
            "version": 1, "cell": [16, 16], "size": [4, 4], "tileset": "cave",
            "layers": [{ "name": "art" }],
            "shapes": [{ "id": "floor", "rect": [0, 3, 4, 1], "tile": "rock", "layers": ["art"] }]
        }))
        .unwrap();
        room.shapes[0].tile = Some(tile.to_owned());
        room.compile("cave", tileset, &layers())
            .unwrap_err()
            .to_string()
    };
    // A near name is the hint, a swap of two letters included.
    assert_eq!(
        message("rokc", &tileset()),
        "shapes[0].tile: no tile `rokc` in the tileset; did you mean `rock`?"
    );
    // With none near, a small tileset's tiles are listed.
    assert_eq!(
        message("grass", &tileset()),
        "shapes[0].tile: no tile `grass` in the tileset; its tiles are `rock`, `rock_45_br`, `rock_45_tl`, `rock_low_br`, `rock_high_br`, `rock_crack`, `plank`, `arch`"
    );
    // A large one's are not.
    let many: Vec<Value> = (0..13)
        .map(|n| json!({ "sprite": format!("stone_{n}") }))
        .collect();
    let many = Tileset::from_json(&json!({ "version": 1, "tiles": many }).to_string()).unwrap();
    assert_eq!(
        message("grass", &many),
        "shapes[0].tile: no tile `grass` in the tileset"
    );
    // An empty one says so.
    let empty = Tileset::from_json(r#"{ "version": 1 }"#).unwrap();
    assert_eq!(
        message("grass", &empty),
        "shapes[0].tile: no tile `grass` in the tileset, which has no tiles"
    );
}

#[test]
fn a_45_degree_edge_takes_the_styles_slope_tiles() {
    // A ramp rising to the right, solid below its edge.
    let ramp = room(
        6,
        6,
        &json!([
            { "id": "ramp", "polygon": [[1, 5], [5, 1], [5, 5]], "tile": "rock", "layers": ["solid"] }
        ]),
    );
    let cells = painted(&compile(&ramp).unwrap().map, 1);
    for (col, row) in [(1, 4), (2, 3), (3, 2), (4, 1)] {
        assert!(
            cells.contains(&(col, row, ROCK_45_BR)),
            "({col}, {row}): {cells:?}"
        );
    }
    for (col, row) in [(2, 4), (3, 4), (4, 4), (3, 3), (4, 3), (4, 2)] {
        assert!(
            cells.contains(&(col, row, ROCK)),
            "({col}, {row}) is the brush"
        );
    }
    assert_eq!(cells.len(), 10, "{cells:?}");
    // A ceiling falling to the right, solid above its edge.
    let ceiling = room(
        4,
        4,
        &json!([
            { "id": "roof", "polygon": [[0, 0], [4, 0], [0, 4]], "tile": "rock", "layers": ["solid"] }
        ]),
    );
    let cells = painted(&compile(&ceiling).unwrap().map, 1);
    for (col, row) in [(3, 0), (2, 1), (1, 2), (0, 3)] {
        assert!(
            cells.contains(&(col, row, ROCK_45_TL)),
            "({col}, {row}): {cells:?}"
        );
    }
    // A plain tile takes each crossed cell, its centre on the edge.
    let boards = room(
        6,
        6,
        &json!([
            { "id": "ramp", "polygon": [[1, 5], [5, 1], [5, 5]], "tile": "plank", "layers": ["art"] }
        ]),
    );
    let cells = painted(&compile(&boards).unwrap().map, 0);
    assert_eq!(cells.len(), 10);
    assert!(cells.iter().all(|&(_, _, tile)| tile == PLANK));
}

#[test]
fn a_2_to_1_edge_takes_low_and_high_cells() {
    let ramp = room(
        4,
        4,
        &json!([
            { "id": "ramp", "polygon": [[0, 4], [4, 2], [4, 4]], "tile": "rock", "layers": ["solid"] }
        ]),
    );
    let cells = painted(&compile(&ramp).unwrap().map, 1);
    assert_eq!(
        cells,
        [
            (2, 2, ROCK_LOW_BR),
            (3, 2, ROCK_HIGH_BR),
            (0, 3, ROCK_LOW_BR),
            (1, 3, ROCK_HIGH_BR),
            (2, 3, ROCK),
            (3, 3, ROCK)
        ]
    );
    // A plain tile: a low cell's centre lies above the edge, a high one's
    // below it, inside.
    let boards = room(
        4,
        4,
        &json!([
            { "id": "ramp", "polygon": [[0, 4], [4, 2], [4, 4]], "tile": "plank", "layers": ["art"] }
        ]),
    );
    let cells = painted(&compile(&boards).unwrap().map, 0);
    assert_eq!(
        cells,
        [(3, 2, PLANK), (1, 3, PLANK), (2, 3, PLANK), (3, 3, PLANK)]
    );
}

#[test]
fn an_edge_at_another_angle_fails_naming_its_shape() {
    let room = room_with(&json!([
        { "id": "floor", "rect": [0, 3, 4, 1], "tile": "rock", "layers": ["art"] },
        { "id": "wedge", "polygon": [[0, 0], [3, 1], [0, 1]], "tile": "rock", "layers": ["art"] }
    ]));
    assert_eq!(
        compile(&room).unwrap_err().to_string(),
        "shapes[1].polygon[0]: the edge from [0, 0] to [3, 1] is not horizontal, vertical, 45\u{b0} or 2:1"
    );
    let steep = room_with(&json!([
        { "id": "steep", "polygon": [[0, 0], [1, 2], [0, 2]], "tile": "rock", "layers": ["art"] }
    ]));
    assert_eq!(
        compile(&steep).unwrap_err().field,
        "shapes[0].polygon[0]",
        "1:2 is not 2:1"
    );
    let crossed = room_with(&json!([
        { "id": "bow", "polygon": [[0, 0], [2, 0], [0, 2], [2, 2]], "tile": "rock", "layers": ["art"] }
    ]));
    let err = compile(&crossed).unwrap_err();
    assert_eq!(err.field, "shapes[0].polygon");
    assert!(err.message.contains("a polygon is simple"), "{err}");
}

#[test]
fn a_cell_two_edges_cross_fails_naming_it() {
    let room = room_with(&json!([
        { "id": "sliver", "polygon": [[0, 0], [2, 1], [1, 1]], "tile": "rock", "layers": ["art"] }
    ]));
    assert_eq!(
        compile(&room).unwrap_err().to_string(),
        "shapes[0].polygon: cell (0, 0) is crossed by edges 0 and 2; a slope cell holds one face"
    );
}

#[test]
fn a_missing_slope_tile_fails_naming_it() {
    // A ramp rising to the left needs `45 bl`, which the tileset lacks.
    let room = room_with(&json!([
        { "id": "ramp", "polygon": [[0, 1], [3, 4], [0, 4]], "tile": "rock", "layers": ["solid"] }
    ]));
    assert_eq!(
        compile(&room).unwrap_err().to_string(),
        "shapes[0]: cell (0, 1) needs style `rock`'s `45 bl` tile, which the tileset lacks"
    );
}

#[test]
fn a_line_paints_the_cells_it_passes() {
    let room = room(
        6,
        5,
        &json!([
            { "id": "bridge", "line": [[1, 1], [4, 1], [4, 3]], "tile": "plank", "layers": ["art"] }
        ]),
    );
    assert_eq!(
        painted(&compile(&room).unwrap().map, 0),
        [
            (1, 1, PLANK),
            (2, 1, PLANK),
            (3, 1, PLANK),
            (4, 1, PLANK),
            (4, 2, PLANK),
            (4, 3, PLANK)
        ]
    );
    let bent = room_with(&json!([
        { "id": "bent", "line": [[0, 0], [2, 2]], "tile": "plank", "layers": ["art"] }
    ]));
    assert_eq!(
        compile(&bent).unwrap_err().to_string(),
        "shapes[0].line[1]: from [0, 0] to [2, 2] is neither horizontal nor vertical"
    );
}

#[test]
fn objects_compile_to_classed_objects_named_room_dot_id() {
    let room = Room::from_json(FULL).unwrap();
    let compiled = room
        .compile(
            "cave",
            &Tileset::from_json(FULL_TILESET).unwrap(),
            &layers(),
        )
        .unwrap();
    let layers = &compiled.map.object_layers;
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, ROOM_OBJECT_LAYER);
    let objects = &layers[0].objects;
    assert_eq!(objects.len(), 2);
    assert_eq!(
        (
            objects[0].id,
            objects[0].name.as_str(),
            objects[0].class.as_str(),
            objects[0].position
        ),
        (1, "cave.crate_1", "crate", Vec2::new(192.0, 304.0))
    );
    assert_eq!(objects[1].name, "cave.crawler_2");
    assert_eq!(objects[1].position, Vec2::new(360.0, 176.0), "at × cell");
    assert_eq!(objects[1].properties["health.max"], json!(30));
    // Places in pixels, a route's place stops resolved.
    assert_eq!(
        compiled.place("player_start").unwrap().shape,
        PlaceShape::Point(Vec2::new(32.0, 304.0))
    );
    assert_eq!(
        compiled.place("main").unwrap().shape,
        PlaceShape::Route(vec![
            Vec2::new(32.0, 304.0),
            Vec2::new(256.0, 240.0),
            Vec2::new(624.0, 304.0)
        ])
    );
    assert_eq!(
        compiled.place("exit").unwrap().to.as_deref(),
        Some("hall.west_door")
    );
    assert_eq!(compiled.lights, 16);
    assert_eq!(compiled.ambient, Some(Vec3::new(0.1, 0.1, 0.16)));
    assert_eq!(compiled.post.as_ref().map(Vec::len), Some(1));
    // A room ID that is not one fails.
    assert!(room.compile("Cave", &tileset(), &self::layers()).is_err());
}

#[test]
fn layer_names_resolve_to_bits_and_an_unknown_one_fails() {
    let mut room = room_with(&json!([]));
    room.layers[1].layers = Some(vec!["terrain".to_owned()]);
    room.layers[1].mask = Some(vec!["player".to_owned(), "enemy".to_owned()]);
    let map = compile(&room).unwrap().map;
    assert_eq!(
        map.layer_physics(1),
        LayerPhysics {
            layers: 1 << 1,
            mask: (1 << 2) | (1 << 3)
        }
    );
    // Without them, layer 0 meeting every layer.
    let bare = compile(&room_with(&json!([]))).unwrap().map;
    assert_eq!(bare.layer_physics(1), LayerPhysics::DEFAULT);
    room.layers[1].mask = Some(vec!["ghost".to_owned()]);
    assert_eq!(
        compile(&room).unwrap_err().to_string(),
        "layers[1].mask: unknown physics layer `ghost`"
    );
    // A render layer's draw.
    room.layers[1].mask = None;
    room.layers[0].z = Some(3);
    let map = compile(&room).unwrap().map;
    assert_eq!(map.layer_draw(0).z, Some(3));
}

#[test]
fn compiling_twice_gives_equal_maps() {
    let room = Room::from_json(FULL).unwrap();
    let tileset = Tileset::from_json(FULL_TILESET).unwrap();
    let first = room.compile("cave", &tileset, &layers()).unwrap();
    let second = room.compile("cave", &tileset, &layers()).unwrap();
    assert_eq!(first, second);
    assert!(first.map.tile_props(ROCK_45_BR_FULL).slope.is_some());
    // The tileset's props reach the map.
    assert_eq!(
        first.map.tile_props(3),
        &TileProps {
            material: Some("stone".to_owned()),
            health: Some(10.0),
            broken: 4,
            ..TileProps::default()
        }
    );
}

/// `rock_45_br` in the full tileset.
const ROCK_45_BR_FULL: TileIndex = 2;
