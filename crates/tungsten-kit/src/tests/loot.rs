use super::*;
use serde_json::json;
use tungsten_core::{DataRegistry, PrefabData, PrefabRegistry};

#[test]
fn a_weighted_draw_follows_its_weights_under_a_seed() {
    let table = WeightedTable::new()
        .with_entry("gem", 6.0)
        .with_entry("heart", 3.0)
        .with_nothing(1.0);
    let mut rng = KitRng::seeded(42);
    let mut counts = [0_u32; 3];
    for _ in 0..10_000 {
        match table.draw(&mut rng) {
            Some("gem") => counts[0] += 1,
            Some("heart") => counts[1] += 1,
            None => counts[2] += 1,
            Some(other) => panic!("{other}"),
        }
    }
    assert_eq!(rng.draws(), 10_000, "one number a draw");
    for (count, expected) in counts.into_iter().zip([6000, 3000, 1000]) {
        assert!(count.abs_diff(expected) < 200, "{counts:?}");
    }
    let mut again = KitRng::seeded(42);
    let first: Vec<Option<&str>> = (0..5).map(|_| table.draw(&mut again)).collect();
    let mut once_more = KitRng::seeded(42);
    let second: Vec<Option<&str>> = (0..5).map(|_| table.draw(&mut once_more)).collect();
    assert_eq!(first, second, "one seed, one sequence");
}

#[test]
fn an_empty_table_draws_nothing_and_takes_no_number() {
    let mut rng = KitRng::seeded(1);
    assert_eq!(WeightedTable::new().draw(&mut rng), None);
    assert_eq!(WeightedTable::default().rolls(), 1);
    assert_eq!(rng.draws(), 0);
}

#[test]
fn the_data_form_checks_rolls_weights_and_fields() {
    let table: WeightedTable = serde_json::from_value(
        json!({ "rolls": 3, "entries": [{ "prefab": "gem", "weight": 3 }, { "weight": 1 }] }),
    )
    .unwrap();
    assert_eq!(
        table,
        WeightedTable::new()
            .with_entry("gem", 3.0)
            .with_nothing(1.0)
            .with_rolls(3)
    );
    assert_eq!(
        serde_json::to_value(&table).unwrap(),
        json!({ "rolls": 3, "entries": [{ "prefab": "gem", "weight": 3.0 }, { "weight": 1.0 }] })
    );
    assert_eq!(
        serde_json::from_value::<WeightedTable>(json!({ "entries": [] }))
            .unwrap()
            .rolls(),
        1
    );
    for bad in [
        json!({ "rolls": 0, "entries": [] }),
        json!({ "rolls": 65, "entries": [] }),
        json!({ "entries": [{ "prefab": "gem", "weight": 0 }] }),
        json!({ "entries": [{ "prefab": "gem" }] }),
        json!({ "entries": [{ "weight": 1, "chance": 2 }] }),
        json!({ "rolls": 1 }),
    ] {
        assert!(
            serde_json::from_value::<WeightedTable>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_kind_checks_each_prefab_by_its_field() {
    let mut world = World::new();
    register_loot_tables(&mut world);
    let mut prefabs = PrefabRegistry::new();
    let gem: PrefabData = serde_json::from_value(json!({ "tag": "gem" })).unwrap();
    prefabs.insert("gem".to_string(), gem);
    world.insert_resource(prefabs);
    let good = WeightedTable::new()
        .with_entry("gem", 1.0)
        .with_nothing(1.0);
    assert!(good.check(&world).is_ok());
    let missing = WeightedTable::new()
        .with_nothing(1.0)
        .with_entry("ruby", 1.0);
    let error = missing.check(&world).unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.id.as_str()),
        ("entries.1.prefab", "ruby")
    );
    assert!(
        world
            .resource::<DataRegistry>()
            .kinds()
            .any(|kind| kind == LOOT_TABLES)
    );
}

#[test]
#[should_panic(expected = "rolls must be in 1..=64")]
fn no_rolls_panics() {
    let _ = WeightedTable::new().with_rolls(0);
}

#[test]
#[should_panic(expected = "weight must be finite and > 0")]
fn a_zero_weight_panics() {
    let _ = WeightedTable::new().with_entry("gem", 0.0);
}

#[test]
#[should_panic(expected = "weight must be finite and > 0")]
fn an_infinite_nothing_panics() {
    let _ = WeightedTable::new().with_nothing(f32::INFINITY);
}
