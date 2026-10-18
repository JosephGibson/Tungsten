use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, Plugin, Schedule, Stage};

use crate::damage::health::{Health, HealthPlugin, Hit, damage};
use crate::foundation::stats::Modifier;
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
        immune.product_with(&Tags::new(["damage.cold"]), 0.0),
        None,
        "immunity, no product"
    );
}

#[test]
fn a_base_zero_stays_immune_under_every_modifier_and_penetration() {
    let mut resist = Resist::new().with_factor("toxic", 0.0);
    for (source, modifier) in [
        ("boost", Modifier::Add(5.0)),
        ("pct", Modifier::Pct(2.0)),
        ("mul", Modifier::Mul(10.0)),
        ("set", Modifier::Override(1.0)),
    ] {
        resist.add_modifier("toxic", source, modifier);
        let toxic = Tags::new(["toxic"]);
        assert_eq!(resist.factor(&toxic), 0.0, "after {source}");
        assert_eq!(resist.product_with(&toxic, 1.0), None, "pierced by none");
    }
    assert_eq!(resist.base("toxic"), Some(0.0));
    // A modifier that makes a factor 0 drops a part too, before any
    // product, and penetration does not lift it.
    let mut warded = Resist::new().with_factor("fire", 0.5);
    warded.add_modifier("fire", "ward", Modifier::Mul(0.0));
    assert_eq!(warded.product_with(&Tags::new(["fire"]), 1.0), None);
    // Points raise a factor in (0, 1), capped at 1, and leave 1 and above.
    let resist = Resist::new()
        .with_factor("fire", 0.5)
        .with_factor("ice", 1.0)
        .with_factor("acid", 2.0);
    assert_eq!(resist.product_with(&Tags::new(["fire"]), 0.25), Some(0.75));
    assert_eq!(resist.product_with(&Tags::new(["fire"]), 0.75), Some(1.0));
    assert_eq!(resist.product_with(&Tags::new(["ice"]), 0.5), Some(1.0));
    assert_eq!(resist.product_with(&Tags::new(["acid"]), 0.5), Some(2.0));
}

#[test]
fn modifiers_combine_as_stats_do_and_clamp_to_the_factor_bounds() {
    let mut resist = Resist::new().with_factor("fire", 0.5);
    resist.add_modifier("fire", "curse", Modifier::Add(0.5));
    resist.add_modifier("fire", "hex", Modifier::Pct(0.5));
    resist.add_modifier("fire", "doom", Modifier::Mul(2.0));
    let fire = Tags::new(["fire"]);
    assert_eq!(resist.factor(&fire), 3.0, "(0.5 + 0.5) × 1.5 × 2");
    let mut stats = crate::Stats::new().with("fire", 0.5);
    for (source, modifier) in [
        ("curse", Modifier::Add(0.5)),
        ("hex", Modifier::Pct(0.5)),
        ("doom", Modifier::Mul(2.0)),
    ] {
        stats.add_modifier("fire", source, modifier);
    }
    assert_eq!(
        stats.get("fire"),
        Some(resist.factor(&fire)),
        "the stat's formula"
    );
    resist.add_modifier("fire", "stone", Modifier::Override(0.25));
    resist.add_modifier("fire", "glass", Modifier::Override(0.75));
    assert_eq!(resist.factor(&fire), 0.75, "the last override wins");
    assert_eq!(resist.remove_modifiers("glass"), 1);
    assert_eq!(resist.factor(&fire), 0.25);
    resist.add_modifier("fire", "huge", Modifier::Mul(1e9));
    resist.remove_modifiers("stone");
    assert_eq!(resist.factor(&fire), 1000.0, "clamped to 1000");
    resist.add_modifier("fire", "sink", Modifier::Add(-1e9));
    assert_eq!(resist.factor(&fire), 0.0, "clamped to 0");
    // A modifier on a tag with no base works on 1 and goes with its last
    // modifier.
    resist.add_modifier("ice", "chill", Modifier::Mul(0.5));
    assert_eq!(resist.factor(&Tags::new(["ice"])), 0.5);
    assert_eq!(resist.base("ice"), None);
    assert_eq!(resist.remove_modifiers("chill"), 1);
    assert_eq!(resist.factor(&Tags::new(["ice"])), 1.0, "gone with it");
    assert_eq!(resist.remove_modifiers("nobody"), 0);
    let rows = resist.inspect_rows();
    assert!(
        rows[0].1.starts_with("fire × 0 (base 0.5; curse Add(0.5)"),
        "{rows:?}"
    );
    assert!(
        std::panic::catch_unwind(|| {
            Resist::new().add_modifier("fire", "nan", Modifier::Add(f32::NAN));
        })
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| {
            Resist::new().add_modifier("Fire", "bad", Modifier::Add(1.0));
        })
        .is_err()
    );
}

#[test]
fn an_085_save_reads_and_modifiers_round_trip() {
    let old: Resist = serde_json::from_value(json!({ "damage.fire": 0.5, "damage": 2 })).unwrap();
    assert_eq!(
        old,
        Resist::new()
            .with_factor("damage.fire", 0.5)
            .with_factor("damage", 2.0)
    );
    assert_eq!(
        serde_json::to_value(&old).unwrap(),
        json!({ "damage": 2.0, "damage.fire": 0.5 }),
        "written as 0.85 wrote it"
    );
    let mut cursed = old.clone();
    cursed.add_modifier("damage.fire", "curse", Modifier::Mul(2.0));
    cursed.add_modifier("damage.cold", "chill", Modifier::Add(-0.5));
    let saved = serde_json::to_value(&cursed).unwrap();
    assert_eq!(
        saved,
        json!({
            "damage": 2.0,
            "damage.cold": { "modifiers": [["chill", { "add": -0.5 }]] },
            "damage.fire": { "base": 0.5, "modifiers": [["curse", { "mul": 2.0 }]] }
        })
    );
    let back: Resist = serde_json::from_value(saved).unwrap();
    assert_eq!(back, cursed);
    assert_eq!(back.factor(&Tags::new(["damage.fire"])), 2.0);
    let base_only: Resist = serde_json::from_value(json!({ "damage": { "base": 0.5 } })).unwrap();
    assert_eq!(base_only, Resist::new().with_factor("damage", 0.5));
    for bad in [
        json!({ "damage": {} }),
        json!({ "damage": { "base": 2000 } }),
        json!({ "damage": { "base": 0.5, "modifiers": [["x", { "mul": 1e39 }]] } }),
        json!({ "damage": { "base": 0.5, "boost": 1 } }),
        json!({ "Damage": { "base": 0.5 } }),
        json!({ "damage": "half" }),
    ] {
        assert!(
            serde_json::from_value::<Resist>(bad.clone()).is_err(),
            "{bad}"
        );
    }
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
