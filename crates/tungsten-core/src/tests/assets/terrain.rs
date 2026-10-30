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
