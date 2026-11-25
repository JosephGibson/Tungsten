//! A table's leaves, its layout's text and its file form.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::json;

use super::*;
use crate::ecs::World;
use crate::physics::{PhysicsPlugin, Position, Spin, Velocity};
use crate::save::SaveRegistry;
use crate::save::registry::Holder;
use crate::schedule::{Plugin, Schedule};

/// A seeded stream.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        crate::rng::splitmix64(self.0)
    }

    fn float(&mut self) -> f32 {
        (self.next() % 20_001) as f32 / 100.0 - 100.0
    }

    fn small(&mut self) -> u32 {
        (self.next() % 1_000) as u32
    }
}

#[derive(Serialize)]
struct Plain {
    count: u32,
    reach: f32,
    name: String,
}

#[derive(Serialize)]
struct Pair(f32, f32);

#[derive(Serialize)]
struct Meters(f64);

#[derive(Serialize)]
struct Unit;

#[derive(Serialize)]
struct Empty {}

#[derive(Serialize)]
enum Kind {
    Idle,
    Speed(f32),
    At(f32, f32),
    Named { x: i32, y: Option<i32> },
}

#[derive(Serialize)]
struct Skips {
    kept: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    maybe: Option<u8>,
    #[serde(skip)]
    never: u8,
}

#[derive(Serialize)]
struct Inner {
    x: i32,
    y: i32,
}

#[derive(Serialize)]
struct Flattened {
    id: u32,
    #[serde(flatten)]
    inner: Inner,
}

/// Bytes through `serialize_bytes`, which a slice of `u8` does not take.
struct Bytes(Vec<u8>);

impl Serialize for Bytes {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

/// Every serde shape under one struct.
#[derive(Serialize)]
struct Everything {
    plain: Plain,
    tuple: (u8, String, bool),
    array: [f32; 3],
    pair: Pair,
    newtype: Meters,
    some_struct: Option<Plain>,
    no_struct: Option<Plain>,
    some_tuple: Option<(u8, i64)>,
    some_array: Option<[u8; 2]>,
    some_newtype: Option<Meters>,
    kinds: (Kind, Kind, Kind, Kind),
    list: Vec<u32>,
    lists: Vec<Vec<Pair>>,
    map: BTreeMap<String, f32>,
    skips_one: Skips,
    skips_none: Skips,
    flattened: Flattened,
    unit: (),
    unit_struct: Unit,
    empty: Empty,
    none_at_all: [u8; 0],
    bytes: Bytes,
    not_finite: (f32, f32, f64),
    wide: (u64, i64, char, bool, u128),
}

fn plain(rng: &mut Seeded) -> Plain {
    Plain {
        count: rng.small(),
        reach: rng.float(),
        name: format!("name {}", rng.small()),
    }
}

fn everything(rng: &mut Seeded) -> Everything {
    Everything {
        plain: plain(rng),
        tuple: (rng.small() as u8, format!("t{}", rng.small()), true),
        array: [rng.float(), rng.float(), rng.float()],
        pair: Pair(rng.float(), rng.float()),
        newtype: Meters(f64::from(rng.float()) / 3.0),
        some_struct: Some(plain(rng)),
        no_struct: None,
        some_tuple: Some((7, -i64::from(rng.small()))),
        some_array: Some([1, rng.small() as u8]),
        some_newtype: Some(Meters(0.1)),
        kinds: (
            Kind::Idle,
            Kind::Speed(rng.float()),
            Kind::At(rng.float(), 2.5),
            Kind::Named {
                x: -3,
                y: rng.small().is_multiple_of(2).then_some(9),
            },
        ),
        list: (0..rng.small() % 5).collect(),
        lists: vec![vec![Pair(1.0, rng.float())], Vec::new()],
        map: (0..3)
            .map(|at| (format!("key {at}"), rng.float()))
            .collect(),
        skips_one: Skips {
            kept: 1,
            maybe: Some(rng.small() as u8),
            never: 9,
        },
        skips_none: Skips {
            kept: 2,
            maybe: None,
            never: 9,
        },
        flattened: Flattened {
            id: rng.small(),
            inner: Inner { x: 4, y: -4 },
        },
        unit: (),
        unit_struct: Unit,
        empty: Empty {},
        none_at_all: [],
        bytes: Bytes(vec![0, 255, rng.small() as u8]),
        not_finite: (f32::NAN, f32::INFINITY, f64::NEG_INFINITY),
        wide: (u64::MAX, i64::MIN, 'ß', false, u128::from(u64::MAX)),
    }
}

/// `value` through [`Flat`]: its leaves and its layout, checked against
/// what `serde_json::to_value` gives. The value built back from the layout
/// and the leaves is that value; the layout's text reads back as the
/// layout; and a second pass held to the layout fits and gives the leaves
/// again. `None` for a value whose layout has no text.
#[track_caller]
fn through_flat<T: Serialize>(value: &T, context: &str) -> Option<(Shape, Vec<Value>)> {
    let (mut leaves, mut layout) = (Vec::new(), Vec::new());
    let mut cells = Cells {
        leaves: &mut leaves,
        layout: &mut layout,
        cursor: None,
    };
    value.serialize(Flat(&mut cells)).unwrap();
    let shape = Shape::from_tokens(&layout)?;
    assert_eq!(Shape::parse(&shape.text()), Ok(shape.clone()), "{context}");
    assert_eq!(shape.leaves(), leaves.len(), "{context}: {}", shape.text());
    let mut read = leaves.iter();
    assert_eq!(
        shape.build(&mut read),
        serde_json::to_value(value).unwrap(),
        "{context}: {}",
        shape.text()
    );
    assert!(read.next().is_none(), "{context}: a leaf left over");

    let mut again = Vec::new();
    let mut held = Cells {
        leaves: &mut again,
        layout: &mut layout,
        cursor: Some(0),
    };
    value.serialize(Flat(&mut held)).unwrap();
    assert!(held.fits(), "{context}: the second pass");
    assert_eq!(again, leaves, "{context}: the second pass's leaves");
    Some((shape, leaves))
}

#[test]
fn leaves_build_back_the_value_to_value_gives() {
    let mut rng = Seeded(1);
    for round in 0..200 {
        let context = format!("round {round}");
        // Each shape as a component's whole form, then all under one struct.
        through_flat(&plain(&mut rng), &context).unwrap();
        through_flat(&(rng.small(), rng.float(), "text"), &context).unwrap();
        through_flat(&[rng.float(), rng.float()], &context).unwrap();
        through_flat(&Pair(rng.float(), rng.float()), &context).unwrap();
        through_flat(&Meters(f64::from(rng.float())), &context).unwrap();
        through_flat(&Some(plain(&mut rng)), &context).unwrap();
        through_flat(&None::<Plain>, &context).unwrap();
        through_flat(&Some((1_u8, rng.float())), &context).unwrap();
        through_flat(&Some([rng.small(), 2]), &context).unwrap();
        through_flat(&Some(Meters(1.5)), &context).unwrap();
        through_flat(&Kind::Idle, &context).unwrap();
        through_flat(&Kind::Speed(rng.float()), &context).unwrap();
        through_flat(&Kind::At(rng.float(), rng.float()), &context).unwrap();
        through_flat(&Kind::Named { x: 1, y: None }, &context).unwrap();
        through_flat(&vec![rng.small(), rng.small()], &context).unwrap();
        through_flat(&BTreeMap::from([("a", rng.float())]), &context).unwrap();
        through_flat(&(), &context).unwrap();
        through_flat(&Unit, &context).unwrap();
        through_flat(&Empty {}, &context).unwrap();
        through_flat(&Bytes(vec![rng.small() as u8; 3]), &context).unwrap();
        through_flat(&f32::NAN, &context).unwrap();
        through_flat(&(f64::INFINITY, rng.float()), &context).unwrap();
        through_flat(&rng.small(), &context).unwrap();
        through_flat(&format!("{}", rng.small()), &context).unwrap();
        through_flat(&everything(&mut rng), &context).unwrap();
    }

    // What opens and what stays one leaf.
    let (shape, leaves) = through_flat(&everything(&mut rng), "the layout").unwrap();
    assert_eq!(
        shape.text(),
        "{plain:{count:_,reach:_,name:_},tuple:[_,_,_],array:[_,_,_],pair:[_,_],newtype:_,\
         some_struct:_,no_struct:_,some_tuple:_,some_array:_,some_newtype:_,kinds:[_,_,_,_],\
         list:_,lists:_,map:_,skips_one:{kept:_,maybe:_},skips_none:{kept:_},flattened:_,\
         unit:_,unit_struct:_,empty:{},none_at_all:[],bytes:_,not_finite:[_,_,_],\
         wide:[_,_,_,_,_]}"
    );
    assert_eq!(leaves.len(), 39);
    // A skipped field changes the layout, so such a row fits no table that
    // holds the field.
    let with = Skips {
        kept: 1,
        maybe: Some(2),
        never: 0,
    };
    let (mut leaves, mut layout) = (Vec::new(), Vec::new());
    let mut cells = Cells {
        leaves: &mut leaves,
        layout: &mut layout,
        cursor: None,
    };
    with.serialize(Flat(&mut cells)).unwrap();
    let without = Skips {
        kept: 1,
        maybe: None,
        never: 0,
    };
    let mut held = Cells {
        leaves: &mut leaves,
        layout: &mut layout,
        cursor: Some(0),
    };
    assert!(without.serialize(Flat(&mut held)).is_err());
    assert_eq!(
        with.never + without.never,
        0,
        "a field serde skips reaches no layout"
    );

    // A field name the text cannot hold gives no layout, so no table.
    #[derive(Serialize)]
    struct Renamed {
        #[serde(rename = "odd-name")]
        odd: u8,
    }
    assert!(through_flat(&Renamed { odd: 1 }, "a renamed field").is_none());
}

#[test]
fn every_saved_form_core_registers_builds_back() {
    // The physics plugin's three: a body's position, velocity and spin.
    let mut world = World::new();
    PhysicsPlugin.build(&mut Schedule::new(), &mut world);
    let mut rng = Seeded(2);
    let mut checked = BTreeMap::new();
    for _ in 0..50 {
        let entity = world.spawn_with((
            Position(glam::Vec2::new(rng.float(), rng.float())),
            Velocity(glam::Vec2::new(rng.float(), f32::NAN)),
            Spin {
                angle: rng.float(),
                angular_velocity: rng.float(),
                prev_angle: rng.float(),
            },
        ));
        let registry = world.resource::<SaveRegistry>();
        for (name, entry) in &registry.components.entries {
            let at = Holder::Entity(entity);
            let Some(expected) = entry.saved.to_value(&world, at) else {
                continue;
            };
            let (mut leaves, mut layout) = (Vec::new(), Vec::new());
            let mut cells = Cells {
                leaves: &mut leaves,
                layout: &mut layout,
                cursor: None,
            };
            entry
                .saved
                .to_leaves(&world, at, &mut cells)
                .unwrap()
                .unwrap();
            let shape = Shape::from_tokens(&layout).unwrap();
            assert_eq!(shape.build(&mut leaves.iter()), expected.unwrap(), "{name}");
            checked.insert(name.clone(), shape.text());
        }
    }
    assert_eq!(
        checked,
        BTreeMap::from([
            ("position".to_owned(), "[_,_]".to_owned()),
            (
                "spin".to_owned(),
                "{angle:_,angular_velocity:_,prev_angle:_}".to_owned()
            ),
            ("velocity".to_owned(), "[_,_]".to_owned()),
        ]),
        "every component the registry holds, each with its layout"
    );
}

#[test]
fn a_layouts_text_reads_back_or_is_refused() {
    for text in [
        "_",
        "[]",
        "{}",
        "[_]",
        "[_,_]",
        "{a:_}",
        "{position:[_,_],rotation:_,scale:[_,_]}",
        "[[_,[]],{x:{y:{}}},_]",
        "{A_1:_,b2:[_]}",
    ] {
        let shape = Shape::parse(text).unwrap();
        assert_eq!(shape.text(), text);
    }
    assert_eq!(Shape::parse("{a:[_,_],b:_}").unwrap().leaves(), 3);
    for text in [
        "",
        "x",
        "__",
        "_ ",
        " _",
        "[",
        "[_",
        "[_,]",
        "[,_]",
        "[_ _]",
        "[_,,_]",
        "{",
        "{a}",
        "{a:}",
        "{a:_",
        "{a:_,}",
        "{:_}",
        "{a-b:_}",
        "{a:_ b:_}",
        "]",
        "}",
        "{a:_}}",
        "[_]]",
        "{\"a\":_}",
    ] {
        assert!(Shape::parse(text).is_err(), "`{text}` read as a layout");
    }
}

/// A table of `rows` rows over a position and a visibility.
fn orbs(ids: &[u64]) -> SpawnTable {
    let columns = vec![
        ("position".to_owned(), Shape::parse("[_,_]").unwrap()),
        (
            "visibility".to_owned(),
            Shape::parse("{visible:_}").unwrap(),
        ),
    ];
    SpawnTable {
        prefab: "orb".into(),
        columns,
        refs: BTreeMap::new(),
        ids: ids.iter().map(|&id| SaveId(id)).collect(),
        cells: ids
            .iter()
            .flat_map(|&id| [json!(id as f64 * 1.5), json!(-2.0), json!(id % 2 == 0)])
            .collect(),
        width: 3,
    }
}

#[test]
fn a_table_writes_its_header_once_and_a_row_a_spawn() {
    let table = orbs(&[7, 8]);
    let written = serde_json::to_value(&table).unwrap();
    assert_eq!(
        written,
        json!({
            "prefab": "orb",
            "columns": { "position": "[_,_]", "visibility": "{visible:_}" },
            "rows": [[7, 10.5, -2.0, false], [8, 12.0, -2.0, true]],
        })
    );
    assert_eq!(
        serde_json::from_value::<SpawnTable>(written.clone()).unwrap(),
        table
    );
    // The record a row stands for.
    assert_eq!(
        serde_json::to_value(table.record(1)).unwrap(),
        json!({ "id": 8, "prefab": "orb", "components": {
            "position": [12.0, -2.0], "visibility": { "visible": true } } })
    );
    // Its rows' references, written once.
    let mut shared = table.clone();
    shared.refs = BTreeMap::from([(
        "chase".to_owned(),
        BTreeMap::from([("target".to_owned(), vec![SaveId(3)])]),
    )]);
    let written = serde_json::to_value(&shared).unwrap();
    assert_eq!(written["refs"], json!({ "chase": { "target": [3] } }));
    assert_eq!(
        serde_json::to_value(shared.record(0)).unwrap()["refs"],
        written["refs"]
    );
    assert_eq!(
        serde_json::from_value::<SpawnTable>(written).unwrap(),
        shared
    );

    // Rows a file holds out of order come back by ID.
    let shuffled = json!({
        "prefab": "orb",
        "columns": { "position": "[_,_]", "visibility": "{visible:_}" },
        "rows": [[8, 12.0, -2.0, true], [7, 10.5, -2.0, false]],
    });
    assert_eq!(
        serde_json::from_value::<SpawnTable>(shuffled).unwrap(),
        table
    );

    // What does not read: a row of another width, a layout that is none, a
    // missing field, an ID that is no number.
    for (broken, reason) in [
        (
            json!({ "prefab": "orb", "columns": { "position": "[_,_]" }, "rows": [[7, 1.0]] }),
            "table row 0 of `orb` holds 2 cells; its columns take an ID and 2",
        ),
        (
            json!({ "prefab": "orb", "columns": { "position": "[_,_" }, "rows": [] }),
            "table column `position`: `[_,_` is no layout",
        ),
        (
            json!({ "columns": {}, "rows": [] }),
            "missing field `prefab`",
        ),
        (
            json!({ "prefab": "orb", "rows": [] }),
            "missing field `columns`",
        ),
        (
            json!({ "prefab": "orb", "columns": {}, "rows": [["seven"]] }),
            "table row 0 of `orb`: its ID",
        ),
    ] {
        let err = serde_json::from_value::<SpawnTable>(broken).unwrap_err();
        assert!(err.to_string().contains(reason), "{err}");
    }
}

#[test]
fn an_areas_spawns_hold_records_and_tables() {
    let listed = json!([
        { "id": 30, "prefab": "bolt", "components": { "position": [1.0, 2.0] } },
        { "prefab": "orb",
          "columns": { "position": "[_,_]", "visibility": "{visible:_}" },
          "rows": [[40, 60.0, -2.0, true], [20, 30.0, -2.0, true]] },
        { "id": 10, "prefab": "bolt", "components": {}, "refs": { "chase": { "target": [3] } } },
    ]);
    let entries: Vec<SpawnEntry> = serde_json::from_value(listed.clone()).unwrap();
    assert!(matches!(&entries[0], SpawnEntry::Record(record) if record.id == SaveId(30)));
    assert!(matches!(&entries[1], SpawnEntry::Table(table) if **table == orbs(&[20, 40])));
    // An element is a record's size: the table is boxed.
    assert_eq!(
        std::mem::size_of::<SpawnEntry>(),
        std::mem::size_of::<SpawnRecord>()
    );
    // As records: by ID across the table's rows and the records beside it.
    let ids: Vec<u64> = records_in_order(&entries)
        .iter()
        .map(|record| record.id.0)
        .collect();
    assert_eq!(ids, [10, 20, 30, 40]);
    let expanded = serde_json::to_value(expanded(&entries)).unwrap();
    assert_eq!(expanded.as_array().unwrap().len(), 4);
    assert_eq!(
        expanded[1],
        json!({ "id": 20, "prefab": "orb", "components": {
            "position": [30.0, -2.0], "visibility": { "visible": true } } })
    );
    // Records alone keep their file's order.
    let plain: Vec<SpawnEntry> =
        serde_json::from_value(json!([listed[0].clone(), listed[2].clone()])).unwrap();
    let ids: Vec<u64> = records_in_order(&plain)
        .iter()
        .map(|record| record.id.0)
        .collect();
    assert_eq!(ids, [30, 10]);
    assert_eq!(serde_json::to_value(&plain).unwrap()[1], listed[2]);
    // A record's errors are a record's.
    let err = serde_json::from_value::<SpawnEntry>(json!({ "id": 1, "components": {} }));
    assert!(
        err.unwrap_err()
            .to_string()
            .contains("missing field `prefab`")
    );
    let err = serde_json::from_value::<SpawnEntry>(json!({ "prefab": "bolt", "components": {} }));
    assert!(err.unwrap_err().to_string().contains("missing field `id`"));
}

#[test]
fn expand_tables_refuses_what_is_no_world_save() {
    for mut no_save in [
        json!(3),
        json!([]),
        json!({}),
        json!({ "level": 2 }),
        json!({ "global": {}, "areas": [] }),
        json!({ "global": { "spawns": 4 }, "areas": {} }),
        json!({ "global": {}, "areas": { "cave": { "spawns": [{ "rows": [] }] } } }),
    ] {
        let before = no_save.clone();
        let err = expand_tables(&mut no_save).unwrap_err();
        assert!(!err.is_empty(), "{before}");
    }
    // A save with no table is left as it is, its records in their order.
    let mut plain = json!({ "global": { "spawns": [
        { "id": 9, "prefab": "bolt", "components": {} },
        { "id": 2, "prefab": "bolt", "components": {} } ] }, "areas": {} });
    let before = plain.clone();
    expand_tables(&mut plain).unwrap();
    assert_eq!(plain, before);
}
