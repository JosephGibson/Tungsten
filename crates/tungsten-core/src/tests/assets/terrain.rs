use super::*;
use crate::assets::tilemap::{LayerPhysics, TileProps};

/// A 4 × 3 map of 16 px cells: tiles `rock` (health 10, breaking into
/// `rubble`), `rubble` (health 4) and `plain`; a render layer `ground` of
/// `plain` and a collision layer `solid` with `rock` on its bottom row.
fn quarry() -> TilemapData {
    let mut map = TilemapData::new(16, 16, 4, 3);
    let rock = map.add_tile("rock");
    let rubble = map.add_tile("rubble");
    let plain = map.add_tile("plain");
    map.set_tile_props(
        rock,
        TileProps {
            health: Some(10.0),
            broken: rubble,
            ..TileProps::default()
        },
    );
    map.set_tile_props(
        rubble,
        TileProps {
            health: Some(4.0),
            ..TileProps::default()
        },
    );
    let ground = map.add_layer("ground", LayerKind::Render);
    let solid = map.add_layer("solid", LayerKind::Collision);
    map.fill(ground, plain);
    for col in 0..4 {
        map.set_tile(solid, col, 2, rock);
    }
    map
}

const ROCK: TileIndex = 0;
const RUBBLE: TileIndex = 1;
const PLAIN: TileIndex = 2;

fn registry() -> TilemapRegistry {
    let mut registry = TilemapRegistry::new();
    registry.insert("quarry".into(), quarry());
    registry
}

fn cell(layer: usize, col: u32, row: u32) -> CellChange {
    CellChange { layer, col, row }
}

/// What a `changes_since` answer reads, its cells collected.
#[derive(Debug, PartialEq)]
enum Read {
    Missing,
    Unchanged,
    Cells(Vec<CellChange>),
    Replaced,
}

fn read(changes: Option<TileChanges<'_>>) -> Read {
    match changes {
        None => Read::Missing,
        Some(TileChanges::Unchanged) => Read::Unchanged,
        Some(TileChanges::Cells(cells)) => Read::Cells(cells.collect()),
        Some(TileChanges::Replaced) => Read::Replaced,
    }
}

/// The cells `changes` lists, or `None` when it lists none.
fn listed(changes: Option<TileChanges<'_>>) -> Option<Vec<CellChange>> {
    match read(changes) {
        Read::Cells(cells) => Some(cells),
        _ => None,
    }
}

fn is_unchanged(changes: Option<TileChanges<'_>>) -> bool {
    read(changes) == Read::Unchanged
}

fn is_replaced(changes: Option<TileChanges<'_>>) -> bool {
    read(changes) == Read::Replaced
}

#[test]
fn set_tile_journals_each_written_cell() {
    let mut registry = registry();
    let start = registry.cursor("quarry").unwrap();
    assert!(is_unchanged(registry.changes_since("quarry", &start)));
    assert!(registry.set_tile("quarry", 1, 2, 1, ROCK));
    assert!(registry.set_tile("quarry", 0, 0, 0, RUBBLE));
    // The same tile again is no change, and no entry.
    assert!(registry.set_tile("quarry", 0, 0, 0, RUBBLE));
    assert!(registry.set_tile("quarry", 1, 2, 1, -7));
    let map = registry.get("quarry").unwrap();
    assert_eq!(map.tile_at(0, 0, 0), Some(RUBBLE));
    assert_eq!(
        map.tile_at(1, 2, 1),
        Some(EMPTY_TILE),
        "a negative tile clears"
    );
    assert_eq!(
        listed(registry.changes_since("quarry", &start)),
        Some(vec![cell(1, 2, 1), cell(0, 0, 0), cell(1, 2, 1)]),
        "oldest first, a cell repeated"
    );
    let now = registry.cursor("quarry").unwrap();
    assert!(is_unchanged(registry.changes_since("quarry", &now)));
    assert!(registry.set_tile("quarry", 1, 3, 0, PLAIN));
    assert_eq!(
        listed(registry.changes_since("quarry", &now)),
        Some(vec![cell(1, 3, 0)])
    );
    // Written back to what the cursor saw, the map is unchanged.
    assert!(registry.set_tile("quarry", 1, 3, 0, EMPTY_TILE));
    assert!(is_unchanged(registry.changes_since("quarry", &now)));
    assert_eq!(
        listed(registry.changes_since("quarry", &start)),
        Some(vec![
            cell(1, 2, 1),
            cell(0, 0, 0),
            cell(1, 2, 1),
            cell(1, 3, 0),
            cell(1, 3, 0)
        ])
    );
    // A map the registry lacks: nothing written, no cursor, no answer.
    assert!(!registry.set_tile("pit", 0, 0, 0, ROCK));
    assert!(registry.cursor("pit").is_none());
    assert!(registry.changes_since("pit", &start).is_none());
    // An insert starts the map's history again.
    let mut moved = quarry();
    moved.set_tile(1, 0, 0, ROCK);
    registry.insert("quarry".into(), moved);
    assert!(is_replaced(registry.changes_since("quarry", &start)));
    assert!(is_replaced(registry.changes_since("quarry", &now)));
}

#[test]
#[should_panic(expected = "TilemapData::set_tile: cell (4, 0) is outside the 4x3 map")]
fn set_tile_panics_outside_the_map() {
    registry().set_tile("quarry", 1, 4, 0, ROCK);
}

#[test]
#[should_panic(expected = "TilemapRegistry::damage_tile: layer 2 is past the map's 2 layers")]
fn damage_tile_panics_past_the_layers() {
    registry().damage_tile("quarry", 2, 0, 0, 1.0);
}

#[test]
fn damage_breaks_a_cell_into_its_broken_tile() {
    let mut registry = registry();
    let start = registry.cursor("quarry").unwrap();
    assert_eq!(registry.health("quarry", 1, 1, 2), Some(10.0));
    assert_eq!(
        registry.damage_tile("quarry", 1, 1, 2, 3.0),
        TileDamage::Damaged { health: 7.0 }
    );
    assert_eq!(registry.health("quarry", 1, 1, 2), Some(7.0));
    for nothing in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert_eq!(
            registry.damage_tile("quarry", 1, 1, 2, nothing),
            TileDamage::Damaged { health: 7.0 },
            "{nothing}"
        );
    }
    assert!(
        is_unchanged(registry.changes_since("quarry", &start)),
        "damage alone writes no cell"
    );
    assert_eq!(
        registry.damage_tile("quarry", 1, 1, 2, 7.0),
        TileDamage::Broken { tile: RUBBLE }
    );
    assert_eq!(
        registry.get("quarry").unwrap().tile_at(1, 1, 2),
        Some(RUBBLE)
    );
    assert_eq!(
        registry.health("quarry", 1, 1, 2),
        Some(4.0),
        "the broken tile starts whole"
    );
    assert_eq!(
        listed(registry.changes_since("quarry", &start)),
        Some(vec![cell(1, 1, 2)])
    );
    assert_eq!(
        registry.damage_tile("quarry", 1, 1, 2, 50.0),
        TileDamage::Broken { tile: EMPTY_TILE }
    );
    assert_eq!(
        registry.get("quarry").unwrap().tile_at(1, 1, 2),
        Some(EMPTY_TILE)
    );
    assert_eq!(registry.health("quarry", 1, 1, 2), None);
    assert_eq!(
        registry.damage_tile("quarry", 1, 1, 2, 1.0),
        TileDamage::Unbreakable,
        "an empty cell"
    );
    assert_eq!(
        registry.damage_tile("quarry", 0, 1, 1, 1.0),
        TileDamage::Unbreakable,
        "a tile without health"
    );
    assert_eq!(registry.health("quarry", 0, 1, 1), None);
    assert_eq!(
        registry.damage_tile("pit", 0, 0, 0, 1.0),
        TileDamage::Unbreakable
    );
    assert_eq!(registry.health("pit", 0, 0, 0), None);
    assert_eq!(registry.health("quarry", 1, 9, 9), None, "outside the map");
    // A write clears a cell's damage.
    assert_eq!(
        registry.damage_tile("quarry", 1, 2, 2, 2.0),
        TileDamage::Damaged { health: 8.0 }
    );
    assert!(registry.set_tile("quarry", 1, 2, 2, ROCK));
    assert_eq!(registry.health("quarry", 1, 2, 2), Some(10.0));
}

/// The 4 × 3 quarry, with `change` applied to it, as another registry holds
/// it alone.
fn other_registry(change: impl FnOnce(&mut TilemapData)) -> TilemapRegistry {
    let mut map = quarry();
    change(&mut map);
    let mut registry = TilemapRegistry::new();
    registry.insert("quarry".into(), map);
    registry
}

#[test]
fn a_cursor_from_another_history_reads_replaced() {
    let registry = registry();
    let cursor = registry.cursor("quarry").unwrap();
    let masked = other_registry(|map| {
        map.set_layer_physics(
            1,
            LayerPhysics {
                layers: 1,
                mask: 1 << 3,
            },
        );
    });
    let kind = other_registry(|map| map.layers[1].kind = LayerKind::Render);
    let property = other_registry(|map| {
        map.set_tile_props(
            PLAIN,
            TileProps {
                material: Some("sand".into()),
                ..TileProps::default()
            },
        );
    });
    let named = other_registry(|map| map.layers[0].name = "floor".into());
    for (what, other) in [
        ("a layer mask", &masked),
        ("a layer kind", &kind),
        ("a tile property", &property),
        ("a layer name", &named),
    ] {
        assert_eq!(
            other.get("quarry").unwrap().layers[1].tiles,
            registry.get("quarry").unwrap().layers[1].tiles,
            "{what}: the same cells"
        );
        assert!(
            is_replaced(other.changes_since("quarry", &cursor)),
            "{what}: another map"
        );
    }
    let same = other_registry(|_| {});
    assert!(is_unchanged(same.changes_since("quarry", &cursor)));
    // Another registry's history that went on from the same map lists what
    // it wrote since.
    let mut edited = other_registry(|_| {});
    edited.set_tile("quarry", 0, 0, 0, ROCK);
    assert_eq!(
        listed(edited.changes_since("quarry", &cursor)),
        Some(vec![cell(0, 0, 0)])
    );
    edited.set_tile("quarry", 0, 0, 0, PLAIN);
    assert!(
        is_unchanged(edited.changes_since("quarry", &cursor)),
        "written back to the same map"
    );
    // The same content inserted again is the same map.
    let mut again = registry.clone();
    again.insert("quarry".into(), quarry());
    assert!(is_unchanged(again.changes_since("quarry", &cursor)));
}

#[test]
fn a_journal_past_its_bound_reads_replaced() {
    assert_eq!(JOURNAL_BOUND, 4_096);
    let mut map = TilemapData::new(16, 16, 100, 100);
    let solid = map.add_tile("solid");
    map.add_layer("solid", LayerKind::Collision);
    let mut registry = TilemapRegistry::new();
    registry.insert("big".into(), map);
    let at = |index: u32| (index % 100, index / 100);
    let start = registry.cursor("big").unwrap();
    let (col, row) = at(0);
    registry.set_tile("big", 0, col, row, solid);
    let first = registry.cursor("big").unwrap();
    let (col, row) = at(1);
    registry.set_tile("big", 0, col, row, solid);
    let second = registry.cursor("big").unwrap();
    for index in 2..=JOURNAL_BOUND as u32 {
        let (col, row) = at(index);
        registry.set_tile("big", 0, col, row, solid);
    }
    // 4,097 entries written: the journal keeps the last 4,096.
    assert!(is_replaced(registry.changes_since("big", &start)));
    let cells = listed(registry.changes_since("big", &first)).unwrap();
    assert_eq!(cells.len(), JOURNAL_BOUND);
    assert_eq!((cells[0].col, cells[0].row), at(1));
    assert_eq!(
        listed(registry.changes_since("big", &second))
            .unwrap()
            .len(),
        JOURNAL_BOUND - 1
    );
    let (col, row) = at(JOURNAL_BOUND as u32 + 1);
    registry.set_tile("big", 0, col, row, solid);
    assert!(is_replaced(registry.changes_since("big", &first)));
    assert_eq!(
        listed(registry.changes_since("big", &second))
            .unwrap()
            .len(),
        JOURNAL_BOUND
    );
}

#[test]
fn edited_and_damaged_cells_list_what_changed() {
    let mut registry = registry();
    registry.set_tile("quarry", 1, 3, 0, RUBBLE);
    registry.set_tile("quarry", 0, 1, 2, ROCK);
    registry.set_tile("quarry", 0, 2, 1, RUBBLE);
    registry.set_tile("quarry", 1, 0, 0, ROCK);
    // The last tile written stays, an empty one included.
    registry.set_tile("quarry", 0, 2, 1, EMPTY_TILE);
    registry.damage_tile("quarry", 1, 0, 2, 25.0);
    registry.damage_tile("quarry", 1, 1, 2, 3.0);
    registry.damage_tile("quarry", 1, 3, 2, 1.5);
    registry.damage_tile("quarry", 1, 0, 0, 4.0);
    let edits: Vec<_> = registry.edited_cells("quarry").collect();
    assert_eq!(
        edits,
        [
            (cell(0, 2, 1), EMPTY_TILE),
            (cell(0, 1, 2), ROCK),
            (cell(1, 0, 0), ROCK),
            (cell(1, 3, 0), RUBBLE),
            (cell(1, 0, 2), RUBBLE),
        ],
        "(layer, row, col) order; the break is a write"
    );
    let damaged: Vec<_> = registry.damaged_cells("quarry").collect();
    assert_eq!(
        damaged,
        [
            (cell(1, 0, 0), 6.0),
            (cell(1, 1, 2), 7.0),
            (cell(1, 3, 2), 8.5)
        ]
    );
    // Replayed on the map as inserted, they rebuild its terrain.
    let mut replay = TilemapRegistry::new();
    replay.insert("quarry".into(), quarry());
    for (at, tile) in &edits {
        replay.set_tile("quarry", at.layer, at.col, at.row, *tile);
    }
    for (at, health) in &damaged {
        let full = replay.health("quarry", at.layer, at.col, at.row).unwrap();
        replay.damage_tile("quarry", at.layer, at.col, at.row, full - health);
    }
    let (built, replayed) = (
        registry.get("quarry").unwrap(),
        replay.get("quarry").unwrap(),
    );
    for layer in 0..2 {
        assert_eq!(built.layers[layer].tiles, replayed.layers[layer].tiles);
    }
    assert_eq!(replay.damaged_cells("quarry").collect::<Vec<_>>(), damaged);
    let cursor = registry.cursor("quarry").unwrap();
    assert!(is_unchanged(replay.changes_since("quarry", &cursor)));
    // An insert forgets them; a map the registry lacks has none.
    registry.insert("quarry".into(), quarry());
    assert_eq!(registry.edited_cells("quarry").count(), 0);
    assert_eq!(registry.damaged_cells("quarry").count(), 0);
    assert_eq!(registry.edited_cells("pit").count(), 0);
}

/// The tiles of style `cave`: its brush, variants for masks 0, 1, 4, 9 and
/// 3, then `plain` (unstyled) and `sand` (a variant of another style).
const CAVE: [(&str, Option<u8>); 6] = [
    ("cave_brush", None),
    ("cave_inner", Some(0)),
    ("cave_top", Some(1)),
    ("cave_bottom", Some(4)),
    ("cave_top_left", Some(9)),
    ("cave_top_right", Some(3)),
];
const BRUSH: TileIndex = 0;
const INNER: TileIndex = 1;
const TOP: TileIndex = 2;
const BOTTOM: TileIndex = 3;
const TOP_LEFT: TileIndex = 4;
const CAVE_PLAIN: TileIndex = 6;
const SAND: TileIndex = 7;

/// A 5 × 4 map of the cave tiles: in layer `block`, brushes over columns 1
/// to 3 of rows 1 and 2, `plain` at (0, 1), `sand` at (2, 0) and an
/// authored `cave_inner` at (4, 1); in layer `full`, brushes everywhere.
/// Brushes unresolved.
fn cave() -> TilemapData {
    let mut map = TilemapData::new(16, 16, 5, 4);
    for (sprite, mask) in CAVE {
        let tile = map.add_tile(sprite);
        map.set_tile_props(
            tile,
            TileProps {
                style: Some("cave".into()),
                mask,
                ..TileProps::default()
            },
        );
    }
    let plain = map.add_tile("plain");
    let sand = map.add_tile("sand");
    map.set_tile_props(
        sand,
        TileProps {
            style: Some("sand".into()),
            mask: Some(0),
            ..TileProps::default()
        },
    );
    let block = map.add_layer("block", LayerKind::Render);
    for (col, row) in [(1, 1), (2, 1), (3, 1), (1, 2), (2, 2), (3, 2)] {
        map.set_tile(block, col, row, BRUSH);
    }
    map.set_tile(block, 0, 1, plain);
    map.set_tile(block, 2, 0, sand);
    map.set_tile(block, 4, 1, INNER);
    let full = map.add_layer("full", LayerKind::Collision);
    map.fill(full, BRUSH);
    map
}

/// The resolved block layer, row by row, -1 empty.
const BLOCK: [[TileIndex; 5]; 4] = [
    [-1, -1, SAND, -1, -1],
    [CAVE_PLAIN, TOP_LEFT, TOP, TOP, INNER],
    [-1, BRUSH, BOTTOM, BRUSH, -1],
    [-1, -1, -1, -1, -1],
];

fn rows(map: &TilemapData, layer: usize) -> Vec<Vec<TileIndex>> {
    map.layers[layer]
        .tiles
        .chunks(map.width as usize)
        .map(<[TileIndex]>::to_vec)
        .collect()
}

#[test]
fn brushes_resolve_to_their_masks_variant_at_load() {
    let built = {
        let mut map = cave();
        map.auto_tile();
        map
    };
    assert_eq!(rows(&built, 0), BLOCK.map(Vec::from));
    assert!(
        built.layers[1].tiles.iter().all(|&tile| tile == INNER),
        "the map's edges are closed, its top too"
    );
    for (col, row) in [(1, 1), (2, 1), (3, 1), (1, 2), (2, 2), (3, 2)] {
        assert!(built.is_auto(0, col, row), "({col}, {row})");
    }
    for (col, row) in [(0, 1), (2, 0), (4, 1), (0, 0)] {
        assert!(
            !built.is_auto(0, col, row),
            "({col}, {row}) is no auto cell"
        );
    }
    assert!(!built.is_auto(0, 9, 9), "outside the map");
    // The same map from a Tiled file resolves alike at load, and an insert
    // resolves a map built in code.
    let property =
        |name: &str, value: serde_json::Value| serde_json::json!({ "name": name, "value": value });
    let mut tiles: Vec<serde_json::Value> = CAVE
        .iter()
        .enumerate()
        .map(|(id, (sprite, mask))| {
            let mut properties = vec![
                property("sprite_id", (*sprite).into()),
                property("style", "cave".into()),
            ];
            if let Some(mask) = mask {
                properties.push(property("mask", (*mask).into()));
            }
            serde_json::json!({ "id": id, "properties": properties })
        })
        .collect();
    tiles.push(
        serde_json::json!({ "id": 6, "properties": [property("sprite_id", "plain".into())] }),
    );
    tiles.push(serde_json::json!({ "id": 7, "properties": [
        property("sprite_id", "sand".into()), property("style", "sand".into()), property("mask", 0.into())
    ] }));
    let raw = cave();
    let gids = |layer: usize| -> Vec<u32> {
        raw.layers[layer]
            .tiles
            .iter()
            .map(|&tile| if tile < 0 { 0 } else { tile as u32 + 1 })
            .collect()
    };
    let json = serde_json::json!({
        "tilewidth": 16, "tileheight": 16, "width": 5, "height": 4,
        "tilesets": [{ "firstgid": 1, "tiles": tiles }],
        "layers": [
            { "type": "tilelayer", "name": "block", "data": gids(0) },
            { "type": "tilelayer", "name": "full", "data": gids(1),
              "properties": [{ "name": "kind", "value": "collision" }] }
        ]
    });
    let loaded = load_json(&json).unwrap();
    assert_eq!(rows(&loaded, 0), BLOCK.map(Vec::from));
    assert_eq!(loaded.layers[1].tiles, built.layers[1].tiles);
    assert!(loaded.is_auto(0, 1, 1) && !loaded.is_auto(0, 4, 1));
    let mut registry = TilemapRegistry::new();
    registry.insert("cave".into(), cave());
    assert_eq!(rows(registry.get("cave").unwrap(), 0), BLOCK.map(Vec::from));
}

/// Writes `map` to a `.tmj` of its own and loads it.
fn load_json(map: &serde_json::Value) -> Result<TilemapData, super::super::tilemap::TilemapError> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path =
        std::env::temp_dir().join(format!("tungsten-terrain-{}-{n}.tmj", std::process::id()));
    std::fs::write(&path, map.to_string()).unwrap();
    let result = TilemapData::load(&path);
    std::fs::remove_file(&path).unwrap();
    result
}

#[test]
fn variants_draw_by_weight_seeded_by_place() {
    // A 40 × 40 field of brushes, every cell's sides closed: mask 0's three
    // variants drawn by their weights 1, 2 and 7.
    let field = || {
        let mut map = TilemapData::new(16, 16, 40, 40);
        let brush = map.add_tile("moss");
        map.set_tile_props(
            brush,
            TileProps {
                style: Some("moss".into()),
                ..TileProps::default()
            },
        );
        for (sprite, weight) in [("moss_a", 1), ("moss_b", 2), ("moss_c", 7)] {
            let tile = map.add_tile(sprite);
            map.set_tile_props(
                tile,
                TileProps {
                    style: Some("moss".into()),
                    mask: Some(0),
                    weight,
                    ..TileProps::default()
                },
            );
        }
        let layer = map.add_layer("moss", LayerKind::Render);
        map.fill(layer, brush);
        map
    };
    let mut map = field();
    map.auto_tile();
    let count = |tile: TileIndex| map.layers[0].tiles.iter().filter(|&&t| t == tile).count();
    let counts = [count(1), count(2), count(3)];
    assert_eq!(counts.iter().sum::<usize>(), 1_600, "{counts:?}");
    for (got, share) in counts.iter().zip([0.1_f64, 0.2, 0.7]) {
        let want = 1_600.0 * share;
        let spread = 4.0 * (1_600.0 * share * (1.0 - share)).sqrt();
        assert!(
            (*got as f64 - want).abs() < spread,
            "{counts:?} against weights 1, 2, 7"
        );
    }
    let mut again = field();
    again.auto_tile();
    assert_eq!(
        again.layers[0].tiles, map.layers[0].tiles,
        "seeded by place"
    );
    // A cleared cell opens its neighbours' sides, which have no variant and
    // draw the brush; every other cell keeps its draw.
    let mut registry = TilemapRegistry::new();
    registry.insert("field".into(), field());
    registry.set_tile("field", 0, 20, 20, EMPTY_TILE);
    let edited = &registry.get("field").unwrap().layers[0].tiles;
    for (index, (&before, &after)) in map.layers[0].tiles.iter().zip(edited).enumerate() {
        let (col, row) = (index % 40, index / 40);
        let beside = col.abs_diff(20) + row.abs_diff(20) == 1;
        match (col, row) {
            (20, 20) => assert_eq!(after, EMPTY_TILE),
            _ if beside => assert_eq!(after, 0, "({col}, {row}) draws the brush"),
            _ => assert_eq!(after, before, "({col}, {row}) keeps its variant"),
        }
    }
}

#[test]
fn an_edit_retiles_only_its_auto_neighbours() {
    let mut registry = TilemapRegistry::new();
    registry.insert("cave".into(), cave());
    let before = registry.cursor("cave").unwrap();
    assert!(registry.set_tile("cave", 0, 2, 1, EMPTY_TILE));
    // Up is `sand`, an override; right, down and left are auto cells whose
    // masks gained an open side.
    assert_eq!(
        listed(registry.changes_since("cave", &before)),
        Some(vec![
            cell(0, 2, 1),
            cell(0, 3, 1),
            cell(0, 2, 2),
            cell(0, 1, 1)
        ])
    );
    let map = registry.get("cave").unwrap();
    assert_eq!(
        rows(map, 0)[1..3],
        [
            vec![CAVE_PLAIN, BRUSH, EMPTY_TILE, TOP_LEFT, INNER],
            vec![-1, BRUSH, BRUSH, BRUSH, -1],
        ],
        "masks 11 and 5 have no variant; (3, 1) is now 9"
    );
    // A write beside the override and an auto cell whose mask holds journals
    // the written cell alone.
    let now = registry.cursor("cave").unwrap();
    assert!(registry.set_tile("cave", 0, 4, 2, CAVE_PLAIN));
    assert_eq!(
        listed(registry.changes_since("cave", &now)),
        Some(vec![cell(0, 4, 2)])
    );
    assert_eq!(registry.get("cave").unwrap().tile_at(0, 4, 1), Some(INNER));
    // An override written over an auto cell is never re-picked.
    assert!(registry.set_tile("cave", 0, 1, 2, TOP));
    assert!(!registry.get("cave").unwrap().is_auto(0, 1, 2));
    registry.set_tile("cave", 0, 1, 3, BRUSH);
    assert_eq!(registry.get("cave").unwrap().tile_at(0, 1, 2), Some(TOP));
}

#[test]
fn a_mask_without_a_variant_draws_the_brush() {
    // A lone brush has every side open, mask 15, which no variant has.
    let lone = |variant: bool| {
        let mut map = TilemapData::new(16, 16, 3, 3);
        let brush = map.add_tile("stone");
        map.set_tile_props(
            brush,
            TileProps {
                style: Some("stone".into()),
                ..TileProps::default()
            },
        );
        if variant {
            let tile = map.add_tile("stone_lone");
            map.set_tile_props(
                tile,
                TileProps {
                    style: Some("stone".into()),
                    mask: Some(15),
                    ..TileProps::default()
                },
            );
        }
        let layer = map.add_layer("stone", LayerKind::Collision);
        map.set_tile(layer, 1, 1, brush);
        map.auto_tile();
        map
    };
    let map = lone(false);
    assert_eq!(map.tile_at(0, 1, 1), Some(0));
    assert!(map.is_auto(0, 1, 1));
    assert_eq!(lone(true).tile_at(0, 1, 1), Some(1));
    // The brush resolves again once a neighbour closes a side.
    let mut registry = TilemapRegistry::new();
    registry.insert("lone".into(), lone(false));
    registry.set_tile("lone", 0, 1, 0, 0);
    let map = registry.get("lone").unwrap();
    assert_eq!(
        (map.tile_at(0, 1, 1), map.tile_at(0, 1, 0)),
        (Some(0), Some(0))
    );
}

#[test]
fn two_brushes_for_one_style_fail_the_load() {
    let tile = |id: u32, sprite: &str, style: &str| {
        serde_json::json!({ "id": id, "properties": [
            { "name": "sprite_id", "value": sprite },
            { "name": "style", "value": style }
        ] })
    };
    let map = |tiles: serde_json::Value| {
        serde_json::json!({
            "tilewidth": 16, "tileheight": 16, "width": 1, "height": 1,
            "tilesets": [{ "firstgid": 1, "tiles": tiles }],
            "layers": [{ "type": "tilelayer", "name": "ground", "data": [1] }]
        })
    };
    let err = load_json(&map(serde_json::json!([
        tile(0, "cave", "cave"),
        tile(2, "sand", "sand"),
        tile(3, "cave_too", "cave")
    ])))
    .unwrap_err();
    assert!(
        err.to_string()
            .ends_with("tile id=3: style `cave` already has a brush, tile id=0"),
        "{err}"
    );
    assert!(
        load_json(&map(serde_json::json!([
            tile(0, "cave", "cave"),
            tile(1, "sand", "sand")
        ])))
        .is_ok()
    );
}

#[test]
#[should_panic(
    expected = "TilemapData::set_tile_props: tile 1: style `cave` already has a brush, tile 0"
)]
fn set_tile_props_panics_on_a_second_brush() {
    let mut map = TilemapData::new(16, 16, 1, 1);
    for sprite in ["cave", "cave_too"] {
        let tile = map.add_tile(sprite);
        map.set_tile_props(
            tile,
            TileProps {
                style: Some("cave".into()),
                ..TileProps::default()
            },
        );
    }
}

#[test]
fn a_replay_of_the_edits_rebuilds_auto_cells() {
    let mut painted = TilemapRegistry::new();
    painted.insert("cave".into(), cave());
    // A brush beside the block's corner and an explicit variant under it.
    painted.set_tile("cave", 0, 4, 2, BRUSH);
    painted.set_tile("cave", 0, 1, 3, TOP);
    let edits: Vec<_> = painted.edited_cells("cave").collect();
    assert_eq!(
        edits,
        [(cell(0, 4, 2), BRUSH), (cell(0, 1, 3), TOP)],
        "a brush stays a brush; the re-picked neighbours are not listed"
    );
    let mut replay = TilemapRegistry::new();
    replay.insert("cave".into(), cave());
    for (at, tile) in &edits {
        replay.set_tile("cave", at.layer, at.col, at.row, *tile);
    }
    let same = |a: &TilemapRegistry, b: &TilemapRegistry| {
        let (a, b) = (a.get("cave").unwrap(), b.get("cave").unwrap());
        assert_eq!(rows(a, 0), rows(b, 0));
        for row in 0..4 {
            for col in 0..5 {
                assert_eq!(
                    a.is_auto(0, col, row),
                    b.is_auto(0, col, row),
                    "({col}, {row})"
                );
            }
        }
    };
    same(&painted, &replay);
    let map = painted.get("cave").unwrap();
    assert!(map.is_auto(0, 4, 2) && !map.is_auto(0, 1, 3));
    assert_ne!(
        map.tile_at(0, 3, 2),
        Some(BRUSH),
        "(3, 2)'s right side closed"
    );
    // A neighbouring edit after the replay re-tiles both copies alike.
    for registry in [&mut painted, &mut replay] {
        registry.set_tile("cave", 0, 0, 2, BRUSH);
    }
    same(&painted, &replay);
    let cursor = painted.cursor("cave").unwrap();
    assert!(is_unchanged(replay.changes_since("cave", &cursor)));
}
