use super::*;
use serde_json::json;
use tungsten_core::{Plugin, Schedule};

use crate::foundation::stats::StatsPlugin;
use crate::foundation::tags::TagsPlugin;

fn parse(value: serde_json::Value) -> Result<Condition, String> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn condition(value: serde_json::Value) -> Condition {
    let text = value.to_string();
    parse(value).unwrap_or_else(|e| panic!("{text}: {e}"))
}

#[test]
fn each_form_reads_its_target() {
    let mut world = World::new();
    let keeper = world.spawn_with((
        Flags::new().with("lamp.promised"),
        Counters::new().with("trust", 3),
        Stats::new().with("vigour", 25.0),
        Tags::new(["team.npc"]),
    ));
    let warden = world.spawn_with((
        Flags::new().with("lamp.asked"),
        Counters::new().with("coins", 9),
        Tags::new(["team.player"]),
    ));
    world.insert_resource(Flags::new().with("night"));
    world.insert_resource(Counters::new().with("lamps", 2));
    let at = |value: serde_json::Value| condition(value).holds(&world, Some(keeper), Some(warden));

    assert!(at(json!({ "flag": "lamp.promised" })));
    assert!(
        !at(json!({ "flag": "lamp.asked" })),
        "the subject's, not the other's"
    );
    assert!(at(
        json!({ "flag": { "name": "lamp.asked", "target": "other" } })
    ));
    assert!(at(json!({ "flag": { "name": "night", "target": "game" } })));
    assert!(!at(
        json!({ "flag": { "name": "lamp.promised", "target": "game" } })
    ));

    assert!(at(
        json!({ "counter": { "name": "trust", "min": 3, "max": 9 } })
    ));
    assert!(!at(json!({ "counter": { "name": "trust", "min": 4 } })));
    assert!(
        at(json!({ "counter": { "name": "trust", "max": 3 } })),
        "inclusive"
    );
    assert!(
        at(json!({ "counter": { "name": "absent", "max": 0 } })),
        "none counts 0"
    );
    assert!(at(
        json!({ "counter": { "name": "coins", "min": 9, "target": "other" } })
    ));
    assert!(at(
        json!({ "counter": { "name": "lamps", "min": 2, "max": 2, "target": "game" } })
    ));

    assert!(at(json!({ "stat": { "name": "vigour", "min": 20 } })));
    assert!(!at(json!({ "stat": { "name": "vigour", "max": 20 } })));
    assert!(
        at(json!({ "stat": { "name": "vigour" } })),
        "no bound: it has the stat"
    );
    assert!(
        !at(json!({ "stat": { "name": "vigour", "target": "other" } })),
        "the other has none"
    );

    assert!(at(json!({ "tags": { "all": ["team.npc"] } })));
    assert!(at(
        json!({ "tags": { "any": ["team.player"], "target": "other" } })
    ));
    assert!(!at(
        json!({ "tags": { "none": ["team.player"], "target": "other" } })
    ));

    assert!(at(
        json!({ "all": [{ "flag": "lamp.promised" }, { "counter": { "name": "trust", "min": 1 } }] })
    ));
    assert!(!at(
        json!({ "all": [{ "flag": "lamp.promised" }, { "flag": "nets_torn" }] })
    ));
    assert!(at(
        json!({ "any": [{ "flag": "nets_torn" }, { "flag": "lamp.promised" }] })
    ));
    assert!(!at(json!({ "any": [{ "flag": "nets_torn" }] })));
    assert!(at(json!({ "not": { "flag": "nets_torn" } })));

    let built = Condition::all([
        Condition::flag("lamp.asked").for_other(),
        Condition::counter("lamps", Some(1), None).for_game(),
        Condition::not(Condition::stat("vigour", None, Some(10.0))),
        Condition::tags(TagFilter::new().all(["team.player"])).for_other(),
    ]);
    assert!(built.holds(&world, Some(keeper), Some(warden)));
    let data = serde_json::to_value(&built).unwrap();
    assert_eq!(
        data["all"][0],
        json!({ "flag": { "name": "lamp.asked", "target": "other" } })
    );
    assert_eq!(condition(data), built, "round trip");
    assert_eq!(
        serde_json::to_value(Condition::flag("night")).unwrap(),
        json!({ "flag": "night" })
    );
}

#[test]
fn an_absent_target_holds_no_form() {
    let mut world = World::new();
    let keeper = world.spawn_with((Flags::new().with("lit"), Stats::new().with("vigour", 5.0)));
    let gone = world.spawn_with((Flags::new().with("lit"), Tags::new(["team.npc"])));
    world.despawn(gone);
    for value in [
        json!({ "flag": { "name": "lit", "target": "other" } }),
        json!({ "counter": { "name": "trust", "max": 0, "target": "other" } }),
        json!({ "stat": { "name": "vigour", "target": "other" } }),
        json!({ "tags": { "none": ["team.npc"], "target": "other" } }),
        json!({ "flag": { "name": "lit", "target": "game" } }),
        json!({ "counter": { "name": "trust", "max": 0, "target": "game" } }),
    ] {
        let form = condition(value.clone());
        assert!(!form.holds(&world, Some(keeper), None), "{value}: no other");
        assert!(
            !form.holds(&world, Some(keeper), Some(gone)),
            "{value}: a dead other"
        );
        assert!(
            Condition::not(form).holds(&world, Some(keeper), Some(gone)),
            "{value}: its `not` holds"
        );
    }
    let mine = condition(json!({ "flag": "lit" }));
    assert!(mine.holds(&world, Some(keeper), None));
    assert!(!mine.holds(&world, None, Some(keeper)), "no subject");
    assert!(!mine.holds(&world, Some(gone), None), "a dead subject");
}

#[test]
fn the_load_refuses_an_undeclared_stat_a_game_stat_and_a_deep_nest() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    StatsPlugin::new()
        .with_stat("vigour", 0.0, 100.0)
        .build(&mut schedule, &mut world);
    TagsPlugin::new()
        .with_tags(["team.player"])
        .build(&mut schedule, &mut world);

    let declared = condition(json!({ "all": [
        { "stat": { "name": "vigour", "min": 1 } },
        { "stat": { "name": "max_health", "min": 1 } },
        { "tags": { "all": ["team.player"] } },
        { "flag": "anything.goes" },
    ] }));
    assert!(declared.check(&world, "when").is_ok());
    let error =
        condition(json!({ "not": { "any": [{ "flag": "x" }, { "stat": { "name": "grit" } }] } }))
            .check(&world, "when")
            .unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind, error.id.as_str()),
        ("when.not.any.1.stat.name", "stat", "grit")
    );
    let error = condition(json!({ "tags": { "any": ["team.ghost"] } }))
        .check(&world, "when")
        .unwrap_err();
    assert_eq!(
        (error.field.as_str(), error.kind),
        ("when.tags.any.0", "tag")
    );

    for (value, why) in [
        (
            json!({ "stat": { "name": "vigour", "target": "game" } }),
            "a game stat",
        ),
        (
            json!({ "tags": { "all": ["team.player"], "target": "game" } }),
            "game tags",
        ),
        (
            json!({ "stat": { "name": "vigour", "min": 5, "max": 1 } }),
            "min past max",
        ),
        (
            json!({ "counter": { "name": "trust" } }),
            "a counter with no bound",
        ),
        (
            json!({ "counter": { "name": "Trust", "min": 1 } }),
            "a bad name",
        ),
        (
            json!({ "flag": "lamp", "not": { "flag": "x" } }),
            "two forms",
        ),
        (json!({}), "no form"),
        (json!({ "all": [] }), "an empty list"),
        (json!({ "when": { "flag": "x" } }), "an unknown form"),
    ] {
        assert!(parse(value.clone()).is_err(), "{why}: {value} parsed");
    }
    let mut nest = json!({ "flag": "deep" });
    for _ in 1..MAX_DEPTH {
        nest = json!({ "not": nest });
    }
    assert!(parse(nest.clone()).is_ok(), "8 deep parses");
    let error = parse(json!({ "all": [nest] })).unwrap_err();
    assert!(error.contains("at most 8 deep"), "{error}");
    let long: Vec<_> = (0..=MAX_LIST).map(|_| json!({ "flag": "x" })).collect();
    let error = parse(json!({ "any": long })).unwrap_err();
    assert!(error.contains("at most 32"), "{error}");
}

#[test]
fn the_item_form_counts_the_bag_and_the_worn() {
    use crate::{Inventory, Item, ItemBase, ItemPlace, KitPlugins, move_item};
    let mut world = World::new();
    let mut schedule = Schedule::new();
    KitPlugins::set().build(&mut schedule, &mut world);
    for (id, base) in [
        ("net", json!({ "name": "Net", "icon": "net", "stack": 9 })),
        (
            "ring",
            json!({ "name": "Ring", "icon": "ring", "slot": "ring" }),
        ),
    ] {
        let base: ItemBase = serde_json::from_value(base).unwrap();
        world
            .resource_mut::<tungsten_core::DataRegistry>()
            .insert(id, base);
    }
    let fisher = world.spawn_with((Inventory::new(4)
        .with_slot("hand", "ring")
        .with_item(Item::new("net").with_count(3))
        .with_item(Item::new("ring")),));
    move_item(
        &mut world,
        fisher,
        ItemPlace::Cell(1),
        ItemPlace::worn("hand"),
    )
    .unwrap();
    let keeper = world.spawn_with((Inventory::new(2).with_item(Item::new("net")),));
    let at = |value: serde_json::Value| condition(value).holds(&world, Some(fisher), Some(keeper));
    assert!(at(json!({ "item": { "id": "net", "min": 3 } })));
    assert!(!at(json!({ "item": { "id": "net", "min": 4 } })));
    assert!(at(json!({ "item": { "id": "net", "min": 1, "max": 3 } })));
    assert!(
        at(json!({ "item": { "id": "ring" } })),
        "the worn count, at least 1 with no bound"
    );
    assert!(!at(json!({ "item": { "id": "ring", "max": 0 } })));
    assert!(
        at(json!({ "item": { "id": "net", "max": 1, "target": "other" } })),
        "the other's bag"
    );
    assert!(!at(json!({ "item": { "id": "lure" } })));
    assert!(
        !condition(json!({ "item": { "id": "net" } })).holds(&world, None, None),
        "no subject holds no form"
    );
    assert_eq!(
        condition(json!({ "item": { "id": "net", "min": 3 } })),
        Condition::item("net", Some(3), None)
    );
    for (value, why) in [
        (
            json!({ "item": { "id": "net", "target": "game" } }),
            "a bag is an entity's",
        ),
        (
            json!({ "item": { "id": "net", "min": 4, "max": 3 } }),
            "`min` 4 exceeds `max` 3",
        ),
    ] {
        let error = parse(value.clone()).unwrap_err();
        assert!(error.contains(why), "{value}: {error}");
    }
    let missing = condition(json!({ "all": [{ "item": { "id": "lure" } }] }))
        .check(&world, "when")
        .unwrap_err();
    assert_eq!(missing.field, "when.all.0.item.id");
    assert!(condition(json!({ "item": { "id": "net", "target": "other" } })).reads_other());
}
