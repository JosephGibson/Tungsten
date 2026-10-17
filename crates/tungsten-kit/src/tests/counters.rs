use super::*;
use serde_json::json;
use tungsten_core::ComponentRegistry;

#[test]
fn counters_add_set_and_read_zero_when_never_set() {
    let mut counters = Counters::new().with("coins", 2);
    assert_eq!(counters.add("coins", 3), 5);
    assert_eq!(counters.add("door.keys", -1), -1);
    counters.set("coins", 0);
    assert_eq!(counters.get("coins"), 0);
    assert_eq!(counters.get("never"), 0);
    assert_eq!(counters.add("score", i64::MAX), i64::MAX);
    assert_eq!(counters.add("score", 1), i64::MAX, "saturating");
    let names: Vec<&str> = counters.iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["coins", "door.keys", "score"]);
}

#[test]
fn counters_round_trip_as_an_object_and_refuse_bad_names() {
    let counters = Counters::new().with("coins", 3).with("kills", 1);
    let data = serde_json::to_value(&counters).unwrap();
    assert_eq!(data, json!({ "coins": 3, "kills": 1 }));
    assert_eq!(serde_json::from_value::<Counters>(data).unwrap(), counters);
    assert!(serde_json::from_value::<Counters>(json!({ "Coins": 1 })).is_err());
    assert!(serde_json::from_value::<Counters>(json!({ "coins": 1.5 })).is_err());
}

#[test]
#[should_panic(expected = "is not a counter or flag name")]
fn a_bad_counter_name_panics() {
    Counters::new().add("two words", 1);
}

#[test]
fn flags_set_clear_and_say_whether_they_changed() {
    let mut flags = Flags::new().with("door.locked").with("door.locked");
    assert_eq!(flags.iter().collect::<Vec<_>>(), ["door.locked"]);
    assert!(flags.set("has_key", true));
    assert!(!flags.set("has_key", true));
    assert!(flags.set("door.locked", false));
    assert!(!flags.set("door.locked", false));
    assert!(flags.has("has_key") && !flags.has("door.locked"));
}

#[test]
fn flags_round_trip_as_a_sorted_list_and_refuse_bad_names() {
    let flags: Flags = serde_json::from_value(json!(["b", "a", "b"])).unwrap();
    assert_eq!(serde_json::to_value(&flags).unwrap(), json!(["a", "b"]));
    assert!(serde_json::from_value::<Flags>(json!(["a b"])).is_err());
}

#[test]
#[should_panic(expected = "is not a counter or flag name")]
fn a_bad_flag_name_panics() {
    Flags::new().set("A", true);
}

#[test]
fn the_plugin_inserts_the_games_own_and_keeps_existing_ones() {
    let mut world = World::new();
    world.insert_resource(Counters::new().with("score", 7));
    let mut schedule = Schedule::new();
    CountersPlugin.build(&mut schedule, &mut world);
    assert_eq!(world.resource::<Counters>().get("score"), 7);
    assert!(world.resource::<Flags>().iter().next().is_none());
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in(COUNTERS, &json!({ "coins": 0 }), &world)
            .is_ok()
    );
    assert!(
        components
            .check_in(FLAGS, &json!(["door.locked"]), &world)
            .is_ok()
    );
    assert!(
        components
            .check_in(FLAGS, &json!(["Door"]), &world)
            .is_err()
    );
}
