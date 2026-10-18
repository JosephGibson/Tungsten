use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, Plugin, Schedule, Stage};

use crate::damage::health::{Health, HealthPlugin, Hit, damage};
use crate::foundation::tags::TagsPlugin;

/// A World with health and the tags `damage.fire` and `damage.cold`
/// declared.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["damage.fire", "damage.cold"])
        .build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn steps(world: &mut World, schedule: &mut Schedule, dt: f32, count: usize) {
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(dt);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    }
}

#[test]
fn resist_multiplies_the_factors_of_a_tag_and_its_ancestors() {
    let resist = Resist::new()
        .with_factor("damage", 0.5)
        .with_factor("damage.fire", 0.5)
        .with_factor("damage.fire", 0.25);
    assert_eq!(
        resist.factor(&Tags::new(["damage.fire"])),
        0.125,
        "replaced, then times its ancestor"
    );
    assert_eq!(resist.factor(&Tags::new(["damage.fire.blue"])), 0.125);
    assert_eq!(resist.factor(&Tags::new(["damage.cold"])), 0.5);
    assert_eq!(resist.factor(&Tags::default()), 1.0);
    let immune = resist.with_factor("damage.cold", 0.0);
    assert_eq!(immune.factor(&Tags::new(["damage.cold"])), 0.0);
    assert_eq!(
        immune.product(&Tags::new(["damage.cold"])),
        None,
        "immunity, no product"
    );
}

#[test]
fn resist_is_data_and_refuses_what_its_builder_refuses() {
    let resist = Resist::new()
        .with_factor("damage.fire", 0.5)
        .with_factor("damage.cold", 0.0);
    let value = serde_json::to_value(&resist).unwrap();
    assert_eq!(value, json!({ "damage.cold": 0.0, "damage.fire": 0.5 }));
    assert_eq!(serde_json::from_value::<Resist>(value).unwrap(), resist);
    for bad in [
        json!({ "Damage": 1 }),
        json!({ "damage": -0.5 }),
        json!({ "damage": 1000.5 }),
        json!({ "damage": "half" }),
    ] {
        assert!(
            serde_json::from_value::<Resist>(bad.clone()).is_err(),
            "{bad}"
        );
    }
    assert!(serde_json::from_value::<Resist>(json!({ "damage": 1000 })).is_ok());
}

#[test]
fn the_scene_form_checks_each_tag_is_declared() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let check = |value| components.check_in(RESIST, &value, &world);
    assert!(
        check(json!({ "damage.fire": 0.5, "damage": 2 })).is_ok(),
        "an ancestor"
    );
    assert_eq!(
        check(json!({ "damage.acid": 0.5 }))
            .unwrap_err()
            .to_string(),
        "component `resist`, field `damage.acid`: unknown tag `damage.acid`"
    );
    assert!(check(json!({ "damage.fire": 2000 })).is_err());
}

#[test]
fn a_shield_absorbs_then_recharges_after_its_delay_at_any_frame_rate() {
    for dt in [1.0 / 60.0, 1.0 / 30.0] {
        let (mut world, mut schedule) = rig();
        let carrier =
            world.spawn_with((Health::new(5.0), Shield::new(2.0).with_recharge(1.0, 0.5)));
        assert_eq!(damage(&mut world, carrier, 1.5), Hit::Absorbed);
        assert_eq!(world.get::<Shield>(carrier).unwrap().current(), 0.5);
        assert_eq!(world.get::<Health>(carrier).unwrap().current(), 5.0);
        // The delay: nothing comes back for half a second.
        steps(&mut world, &mut schedule, dt, (0.5 / dt).round() as usize);
        let shield = world.get::<Shield>(carrier).unwrap().current();
        assert!((shield - 0.5).abs() < 1e-4, "dt {dt}: {shield}");
        // Then a fifth of a second of recharge.
        steps(&mut world, &mut schedule, dt, (0.2 / dt).round() as usize);
        let shield = world.get::<Shield>(carrier).unwrap().current();
        assert!((shield - 0.7).abs() < 1e-4, "dt {dt}: {shield}");
        // A hit restarts the delay.
        assert_eq!(damage(&mut world, carrier, 0.25), Hit::Absorbed);
        steps(&mut world, &mut schedule, dt, (0.2 / dt).round() as usize);
        let shield = world.get::<Shield>(carrier).unwrap().current();
        assert!((shield - 0.45).abs() < 1e-4, "dt {dt}: {shield}");
        // And it stops at its max.
        steps(&mut world, &mut schedule, dt, (5.0 / dt).round() as usize);
        assert_eq!(world.get::<Shield>(carrier).unwrap().current(), 2.0);
    }
}

#[test]
fn a_shield_without_a_recharge_stays_down_and_restore_caps_it() {
    let (mut world, mut schedule) = rig();
    let wall = world.spawn_with((Health::new(5.0), Shield::new(1.0)));
    assert_eq!(damage(&mut world, wall, 3.0), Hit::Hurt);
    assert_eq!(
        world.get::<Health>(wall).unwrap().current(),
        3.0,
        "1 shield, 2 health"
    );
    steps(&mut world, &mut schedule, 1.0 / 60.0, 120);
    assert_eq!(world.get::<Shield>(wall).unwrap().current(), 0.0);
    world.get_mut::<Shield>(wall).unwrap().restore(5.0);
    assert_eq!(world.get::<Shield>(wall).unwrap().current(), 1.0);
}

#[test]
fn shield_is_data_within_its_bounds() {
    let mut shield = Shield::new(3.0).with_recharge(1.0, 2.0);
    shield.take(1.0);
    let value = serde_json::to_value(&shield).unwrap();
    assert_eq!(
        value,
        json!({ "current": 2.0, "max": 3.0, "recharge": 1.0, "delay": 2.0, "since_hit": 0.0 })
    );
    assert_eq!(serde_json::from_value::<Shield>(value).unwrap(), shield);
    for bad in [
        json!({ "current": 4.0, "max": 3.0 }),
        json!({ "current": 1.0, "max": 0.0 }),
        json!({ "current": 1.0, "max": 3.0, "recharge": -1.0 }),
        json!({ "current": 1.0, "max": 3.0, "since_hit": -1.0 }),
        json!({ "current": 1.0, "max": 3.0, "charge": 1.0 }),
    ] {
        assert!(
            serde_json::from_value::<Shield>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_scene_form_of_a_shield_spawns_it_full() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in(
                SHIELD,
                &json!({ "max": 3, "recharge": 1, "delay": 2 }),
                &world
            )
            .is_ok()
    );
    for bad in [
        json!({ "max": 0 }),
        json!({ "max": 3, "delay": -1 }),
        json!({ "max": 3, "regen": 1 }),
    ] {
        assert!(components.check_in(SHIELD, &bad, &world).is_err(), "{bad}");
    }
}

#[test]
#[should_panic(expected = "the factor must be finite and in [0, 1000]")]
fn a_factor_past_1000_panics() {
    let _ = Resist::new().with_factor("damage", 1000.5);
}

#[test]
#[should_panic(expected = "is not a tag name")]
fn a_factor_on_a_bad_tag_panics() {
    let _ = Resist::new().with_factor("Damage Fire", 0.5);
}

#[test]
#[should_panic(expected = "max must be finite and > 0")]
fn a_zero_shield_panics() {
    let _ = Shield::new(0.0);
}

#[test]
#[should_panic(expected = "delay must be finite and >= 0")]
fn a_negative_delay_panics() {
    let _ = Shield::new(1.0).with_recharge(1.0, -1.0);
}

#[test]
#[should_panic(expected = "amount must be finite and >= 0")]
fn a_nan_restore_panics() {
    Shield::new(1.0).restore(f32::NAN);
}
