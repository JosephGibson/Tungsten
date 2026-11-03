use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, PrefabData, PrefabRegistry, Stage, Time};

use crate::{TagFilter, TagSet};

/// A World with the stats plugin built, declaring `armor` in `[0, 10]`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    StatsPlugin::new()
        .with_stat("armor", 0.0, 10.0)
        .build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

#[test]
fn modifiers_apply_in_the_catalogues_order_and_the_last_override_wins() {
    let mut stats = Stats::new().with("power", 10.0);
    stats.add_modifier("power", "a", Modifier::Mul(2.0));
    stats.add_modifier("power", "b", Modifier::Pct(0.5));
    stats.add_modifier("power", "c", Modifier::Add(2.0));
    stats.add_modifier("power", "d", Modifier::Pct(0.5));
    assert_eq!(stats.get("power"), Some(48.0), "(10 + 2) × (1 + 1) × 2");
    stats.add_modifier("power", "e", Modifier::Override(5.0));
    stats.add_modifier("power", "f", Modifier::Override(7.0));
    assert_eq!(stats.get("power"), Some(7.0));
    assert_eq!(stats.remove_modifiers("f"), 1);
    assert_eq!(stats.get("power"), Some(5.0));
    assert_eq!(stats.remove_modifiers("e"), 1);
    assert_eq!(stats.remove_modifiers("nobody"), 0);
    assert_eq!(stats.get("power"), Some(48.0));
    assert_eq!(stats.base("power"), Some(10.0));
}

#[test]
fn overflowing_sums_and_products_stay_finite_and_clamped() {
    let mut stats = Stats::new().with("mass", 3e38);
    stats.add_modifier("mass", "double", Modifier::Mul(2.0));
    assert_eq!(stats.get("mass"), Some(f32::MAX), "6e38 clamps to f32::MAX");
    for i in 0..9 {
        stats.add_modifier("mass", &format!("huge{i}"), Modifier::Mul(1e38));
    }
    assert_eq!(
        stats.get("mass"),
        Some(f32::MAX),
        "an f64 infinity clamps too"
    );
    stats.add_modifier("mass", "zero", Modifier::Mul(0.0));
    assert_eq!(
        stats.get("mass"),
        Some(0.0),
        "a zero factor makes the product 0"
    );
    stats.remove_modifiers("zero");
    stats.set_base("mass", -3e38);
    assert_eq!(stats.get("mass"), Some(-f32::MAX));
    stats.add_modifier("mass", "flip", Modifier::Pct(-1.0));
    assert_eq!(stats.get("mass"), Some(0.0), "a zero percentage factor");
    assert!(stats.iter().all(|(_, value)| value.is_finite()));
}

#[test]
fn the_well_known_stats_clamp_to_their_floors() {
    let mut stats = Stats::new()
        .with(Stats::MOVE_SPEED, 100.0)
        .with(Stats::MAX_HEALTH, 5.0);
    stats.add_modifier(Stats::MOVE_SPEED, "glue", Modifier::Add(-500.0));
    stats.add_modifier(Stats::MAX_HEALTH, "curse", Modifier::Mul(0.0));
    assert_eq!(stats.get(Stats::MOVE_SPEED), Some(0.0));
    assert_eq!(stats.get(Stats::MAX_HEALTH), Some(1.0), "no modifier kills");
    assert_eq!(Stats::WELL_KNOWN.len(), 10);
    let names: Vec<&str> = stats.iter().map(|(name, _)| name).collect();
    assert_eq!(names, ["max_health", "move_speed"]);
}

#[test]
fn the_crit_and_damage_taken_stats_take_their_bounds() {
    let mut stats = Stats::new()
        .with(Stats::CRIT_CHANCE, 0.5)
        .with(Stats::CRIT_MULTIPLIER, 2.0)
        .with(Stats::DAMAGE_TAKEN, 1.0);
    stats.add_modifier(Stats::CRIT_CHANCE, "luck", Modifier::Add(2.0));
    stats.add_modifier(Stats::CRIT_MULTIPLIER, "dull", Modifier::Mul(0.1));
    stats.add_modifier(Stats::DAMAGE_TAKEN, "stone", Modifier::Add(-5.0));
    assert_eq!(stats.get(Stats::CRIT_CHANCE), Some(1.0));
    assert_eq!(stats.get(Stats::CRIT_MULTIPLIER), Some(1.0));
    assert_eq!(stats.get(Stats::DAMAGE_TAKEN), Some(0.0));
    let saved = serde_json::to_value(&stats).unwrap();
    assert_eq!(saved["crit_chance"]["max"], json!(1.0));
    assert_eq!(saved["crit_multiplier"]["min"], json!(1.0));
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in("stats", &json!({ "crit_chance": 1.5 }), &world)
            .is_err()
    );
    assert!(
        components
            .check_in("stats", &json!({ "crit_multiplier": 0.5 }), &world)
            .is_err()
    );
    assert!(
        components
            .check_in(
                "stats",
                &json!({ "crit_chance": 0.05, "crit_multiplier": 2, "damage_taken": 1 }),
                &world
            )
            .is_ok()
    );
}

#[test]
fn an_older_save_loads_unchanged_and_kept_lines_round_trip() {
    // A save from before lines: every value as it was, the same bytes back.
    let older = json!({
        "armor": { "base": 2.0, "min": 0.0, "max": 10.0 },
        "move_speed": { "base": 80.0, "min": 0.0, "max": null, "modifiers": [["haste", { "pct": 0.25 }], ["difficulty", { "mul": 1.5 }]] }
    });
    let stats: Stats = serde_json::from_value(older.clone()).unwrap();
    assert_eq!(stats.get(Stats::MOVE_SPEED), Some(150.0));
    assert_eq!(stats.get("armor"), Some(2.0));
    assert_eq!(stats.lines().count(), 0);
    assert_eq!(serde_json::to_value(&stats).unwrap(), older);

    // A scoped line on a stat it has, and a line on a stat it lacks.
    let mut stats = stats;
    let blasts = TagFilter::new().all(["hit.blast"]);
    stats.add_line(
        Stats::MOVE_SPEED,
        "base",
        Line::new(Modifier::Mul(0.5)).with(blasts),
    );
    stats.add_line("cost", "passive:frugal", Line::new(Modifier::Pct(-0.25)));
    let saved = serde_json::to_value(&stats).unwrap();
    assert_eq!(
        saved["move_speed"]["lines"],
        json!([["base", { "modifier": { "mul": 0.5 }, "with": { "all": ["hit.blast"] } }]])
    );
    assert_eq!(
        saved["cost"],
        json!({ "min": null, "max": null, "lines": [["passive:frugal", { "modifier": { "pct": -0.25 } }]] })
    );
    let back: Stats = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(back, stats);
    assert_eq!(serde_json::to_value(&back).unwrap(), saved);
    assert_eq!(back.get("cost"), None, "a stat it lacks");
    for bad in [
        json!({ "cost": { "min": null, "max": null } }),
        json!({ "cost": { "min": 0, "lines": [["a", { "modifier": { "add": 1 } }]] } }),
        json!({ "cost": { "modifiers": [["a", { "add": 1 }]], "lines": [["a", { "modifier": { "add": 1 } }]] } }),
    ] {
        assert!(
            serde_json::from_value::<Stats>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_line_on_a_stat_it_lacks_becomes_a_modifier_when_the_stat_comes() {
    let mut stats = Stats::new();
    stats.add_line("luck", "charm", Line::new(Modifier::Add(1.0)));
    let sevens = TagFilter::new().all(["dice.seven"]);
    stats.add_line(
        "luck",
        "lucky_seven",
        Line::new(Modifier::Mul(2.0)).with(sevens),
    );
    assert_eq!(stats.get("luck"), None);
    stats.set_base("luck", 2.0);
    assert_eq!(
        stats.get("luck"),
        Some(3.0),
        "the unscoped line is a modifier now"
    );
    let lines: Vec<&str> = stats.lines().map(|(_, source, _)| source).collect();
    assert_eq!(lines, ["lucky_seven"], "the scoped one stays kept");
    let seven = [TagSet::Name("dice.seven")];
    assert_eq!(stats.read("luck", &Read::new(&seven)), Some(6.0));
    assert_eq!(stats.remove_modifiers("lucky_seven"), 1);
    assert_eq!(stats.remove_modifiers("charm"), 1);
    assert_eq!(stats.lines().count(), 0);
    assert_eq!(stats.get("luck"), Some(2.0));
}

#[test]
fn the_scene_form_takes_a_base_or_a_base_with_lines() {
    let (mut world, mut schedule) = rig();
    let components = world.resource::<ComponentRegistry>();
    let form = json!({
        "armor": 2,
        "damage_taken": { "base": 1, "lines": [{ "modifier": { "mul": 0.5 }, "with": { "all": ["hit.blast"] } }] }
    });
    let error = components.check_in("stats", &form, &world).unwrap_err();
    assert!(
        error.to_string().contains("hit.blast"),
        "the tag is checked: {error}"
    );
    crate::foundation::tags::TagsPlugin::new()
        .with_tags(["hit.blast"])
        .build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    components.check_in("stats", &form, &world).unwrap();
    for bad in [
        json!({ "damage_taken": { "base": 1, "lines": [{ "modifier": { "override": 0 }, "with": { "all": ["hit.blast"] } }] } }),
        json!({ "damage_taken": { "lines": [] } }),
        json!({ "damage_taken": { "base": 1, "spare": 0 } }),
        json!({ "damage_taken": { "base": -1 } }),
    ] {
        assert!(components.check_in("stats", &bad, &world).is_err(), "{bad}");
    }
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert(
        "knight".to_string(),
        serde_json::from_value::<PrefabData>(json!({ "components": { "stats": form } })).unwrap(),
    );
    world.insert_resource(prefabs);
    let knight = world.spawn_prefab("knight", [0.0, 0.0]).unwrap();
    let stats = world.get::<Stats>(knight).unwrap();
    assert_eq!(stats.get(Stats::DAMAGE_TAKEN), Some(1.0));
    let blast = [TagSet::Name("hit.blast")];
    assert_eq!(
        stats.read(Stats::DAMAGE_TAKEN, &Read::new(&blast)),
        Some(0.5)
    );
    let lines: Vec<(&str, &str)> = stats
        .lines()
        .map(|(stat, source, _)| (stat, source))
        .collect();
    assert_eq!(lines, [("damage_taken", "base")]);
}

#[test]
fn a_modifier_on_a_stat_it_lacks_changes_nothing() {
    let mut stats = Stats::new();
    assert!(!stats.add_modifier("luck", "charm", Modifier::Add(1.0)));
    assert_eq!(stats.get("luck"), None);
}

#[test]
fn the_save_form_round_trips_bases_bounds_and_modifiers() {
    let mut stats = Stats::new()
        .with("armor", 2.0)
        .with_bounds("armor", 0.0, 10.0)
        .with(Stats::MOVE_SPEED, 80.0);
    stats.add_modifier(Stats::MOVE_SPEED, "haste", Modifier::Pct(0.25));
    let saved = serde_json::to_value(&stats).unwrap();
    assert_eq!(
        saved,
        json!({
            "armor": { "base": 2.0, "min": 0.0, "max": 10.0 },
            "move_speed": { "base": 80.0, "min": 0.0, "max": null, "modifiers": [["haste", { "pct": 0.25 }]] }
        })
    );
    let back: Stats = serde_json::from_value(saved).unwrap();
    assert_eq!(back, stats);
    assert_eq!(back.get(Stats::MOVE_SPEED), Some(100.0));
}

#[test]
fn the_save_form_refuses_what_the_constructors_refuse() {
    for bad in [
        json!({ "Armor": { "base": 1 } }),
        json!({ "armor": { "base": 1, "min": 5, "max": 1 } }),
        json!({ "armor": { "base": 1, "spare": 0 } }),
    ] {
        assert!(
            serde_json::from_value::<Stats>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
#[should_panic(expected = "is not a stat name")]
fn a_bad_stat_name_panics() {
    let _ = Stats::new().with("Move Speed", 1.0);
}

#[test]
#[should_panic(expected = "must be finite")]
fn a_non_finite_base_panics() {
    let _ = Stats::new().with("armor", f32::NAN);
}

#[test]
#[should_panic(expected = "no stat `armor`")]
fn bounds_on_a_missing_stat_panic() {
    let _ = Stats::new().with_bounds("armor", 0.0, 1.0);
}

#[test]
#[should_panic(expected = "the modifier must be finite")]
fn a_non_finite_modifier_panics() {
    let mut stats = Stats::new().with("armor", 1.0);
    stats.add_modifier("armor", "x", Modifier::Mul(f32::INFINITY));
}

#[test]
#[should_panic(expected = "is a well-known stat")]
fn declaring_a_well_known_stat_panics() {
    let _ = StatsPlugin::new().with_stat(Stats::MOVE_SPEED, 0.0, 1.0);
}

#[test]
fn the_load_checks_each_name_and_base_against_its_bounds() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let check = |value| components.check_in(STATS, &value, &world);
    assert!(check(json!({ "move_speed": 160, "armor": 10 })).is_ok());
    let unknown = check(json!({ "luck": 1 })).unwrap_err().to_string();
    assert_eq!(
        unknown,
        "component `stats`, field `luck`: unknown stat `luck`"
    );
    let outside = check(json!({ "armor": 11 })).unwrap_err().to_string();
    assert!(outside.contains("armor = 11"), "{outside}");
    assert!(check(json!({ "move_speed": -1 })).is_err());
    assert!(check(json!({ "armor": "two" })).is_err());
}

#[test]
fn a_stats_data_entry_declares_a_game_stat() {
    let (mut world, _) = rig();
    let entry: StatBoundsList =
        serde_json::from_value(json!({ "luck": { "min": -1, "max": 1 }, "fame": {} })).unwrap();
    world.resource_mut::<DataRegistry>().insert("game", entry);
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in(STATS, &json!({ "luck": 1, "fame": 1e6 }), &world)
            .is_ok()
    );
    assert!(
        components
            .check_in(STATS, &json!({ "luck": 2 }), &world)
            .is_err()
    );
    for bad in [
        json!({ "move_speed": {} }),
        json!({ "luck": { "min": 2, "max": 1 } }),
        json!({ "luck": { "low": 0 } }),
    ] {
        assert!(
            serde_json::from_value::<StatBoundsList>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn a_spawned_game_stat_takes_its_declared_bounds() {
    let (mut world, _) = rig();
    let knight: PrefabData = serde_json::from_value(
        json!({ "components": { "stats": { "armor": 4, "move_speed": 90 } } }),
    )
    .unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("knight".to_string(), knight);
    world.insert_resource(prefabs);
    let entity = world.spawn_prefab("knight", [0.0, 0.0]).unwrap();
    let stats = world.get_mut::<Stats>(entity).unwrap();
    assert_eq!(stats.get(Stats::MOVE_SPEED), Some(90.0));
    stats.add_modifier("armor", "stone", Modifier::Add(100.0));
    assert_eq!(stats.get("armor"), Some(10.0), "the declared clamp");
}

#[test]
fn stats_apply_writes_the_movers_health_and_cooldowns() {
    let (mut world, mut schedule) = rig();
    let runner = world.spawn_with((
        TopDownMover::new(100.0),
        PlatformerController::new(200.0, 600.0),
        Health::new(5.0),
        Cooldown::new(1.0),
        Stats::new()
            .with(Stats::MOVE_SPEED, 0.0)
            .with(Stats::MAX_HEALTH, 8.0)
            .with(Stats::COOLDOWN_RATE, 2.0),
    ));
    let plain = world.spawn_with((TopDownMover::new(100.0), Health::new(5.0)));
    step(&mut world, &mut schedule);
    let health = world.get::<Health>(runner).unwrap();
    assert_eq!((health.current(), health.max()), (8.0, 8.0));
    assert_eq!(world.get::<Cooldown>(runner).unwrap().rate(), 2.0);
    assert_eq!(world.get::<Health>(plain).unwrap().max(), 5.0);

    // A max halved keeps the fraction; the dead stay dead.
    crate::damage(&mut world, runner, 2.0);
    world
        .get_mut::<Stats>(runner)
        .unwrap()
        .set_base(Stats::MAX_HEALTH, 4.0);
    step(&mut world, &mut schedule);
    let health = world.get::<Health>(runner).unwrap();
    assert_eq!((health.current(), health.max()), (3.0, 4.0));
}

#[test]
fn a_damage_stat_sets_a_hitboxs_and_a_contact_damages_damage() {
    let (mut world, mut schedule) = rig();
    let shape = crate::HitShape::circle(4.0);
    let brute = world.spawn_with((
        Hitbox::new(shape, 1.0),
        ContactDamage::new(1.0),
        Stats::new().with(Stats::DAMAGE, 3.0),
    ));
    let plain = world.spawn_with((Hitbox::new(shape, 1.0),));
    step(&mut world, &mut schedule);
    assert_eq!(world.get::<Hitbox>(brute).unwrap().damage(), 3.0);
    assert_eq!(world.get::<ContactDamage>(brute).unwrap().damage(), 3.0);
    assert_eq!(
        world.get::<Hitbox>(plain).unwrap().damage(),
        1.0,
        "its own field"
    );
    world.get_mut::<Stats>(brute).unwrap().add_modifier(
        Stats::DAMAGE,
        "curse",
        Modifier::Add(-10.0),
    );
    step(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Hitbox>(brute).unwrap().damage(),
        0.0,
        "the well-known floor"
    );
}

#[test]
fn a_move_speed_stat_sets_a_steerings_top_speed_and_zero_stops_it() {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    StatsPlugin::new().build(&mut schedule, &mut world);
    crate::SteeringPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let chaser = world.spawn_with((
        Steering::seek(crate::Target::point(glam::Vec2::new(500.0, 0.0)), 90.0),
        Stats::new().with(Stats::MOVE_SPEED, 40.0),
        tungsten_core::physics::Position(glam::Vec2::ZERO),
    ));
    step(&mut world, &mut schedule);
    let velocity = world.get::<Steering>(chaser).unwrap().velocity();
    assert_eq!(
        velocity,
        glam::Vec2::new(40.0, 0.0),
        "its top speed from the stat"
    );
    world
        .get_mut::<Stats>(chaser)
        .unwrap()
        .set_base(Stats::MOVE_SPEED, 0.0);
    step(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Steering>(chaser).unwrap().velocity(),
        glam::Vec2::ZERO
    );
}
