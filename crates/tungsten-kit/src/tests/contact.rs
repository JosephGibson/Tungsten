use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, Stage};

use crate::foundation::tags::TagsPlugin;
use crate::overlap::hit::{HIT_GRID, MAX_EXTENT};

/// A World with contact damage and the tags `team.player` and
/// `damage.spike`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.player", "damage.spike"])
        .build(&mut schedule, &mut world);
    ContactDamagePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// A World with contact damage, health and the damage types
/// `physical.slash` and `elemental.fire` declared.
fn typed_rig() -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["physical.slash", "elemental.fire"])
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    ContactDamagePlugin.build(&mut schedule, &mut world);
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(json!({ "physical.slash": {}, "elemental.fire": {} })).unwrap();
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("game", types);
    world
}

fn check(world: &World, value: &serde_json::Value) -> Result<(), String> {
    world
        .resource::<ComponentRegistry>()
        .check_in(CONTACT_DAMAGE_COMPONENT, value, world)
        .map_err(|e| e.to_string())
}

#[test]
fn contact_damage_takes_the_hurtboxs_shape_unless_it_is_an_aura() {
    let spikes = ContactDamage::new(1.0);
    assert_eq!(
        (spikes.interval(), spikes.shape(), spikes.offset()),
        (0.5, None, Vec2::ZERO)
    );
    let aura = ContactDamage::new(2.0)
        .with_interval(0.25)
        .with_shape(HitShape::circle(28.0), Vec2::new(0.0, 4.0))
        .with_tags(Tags::new(["damage.spike"]))
        .with_filter(TagFilter::new().all(["team.player"]))
        .with_knockback(420.0);
    assert_eq!(aura.shape(), Some(HitShape::circle(28.0)));
    assert_eq!(aura.offset(), Vec2::new(0.0, 4.0));
    assert_eq!(
        (aura.damage(), aura.interval(), aura.knockback()),
        (2.0, 0.25, 420.0)
    );
    assert!(aura.tags().has("damage") && !aura.filter().is_everything());
}

#[test]
fn contact_damage_is_data_without_its_timed_targets() {
    let mut world = World::new();
    let target = world.spawn();
    let mut aura = ContactDamage::new(1.0).with_shape(HitShape::circle(28.0), Vec2::ZERO);
    aura.clock = 1.0;
    aura.timed.push(Timed {
        target,
        hit: 0.5,
        met: 0.5,
    });
    let saved = serde_json::to_value(&aura).unwrap();
    assert_eq!(
        saved,
        json!({
            "damage": 1.0, "interval": 0.5, "shape": { "circle": 28.0 }, "offset": [0.0, 0.0],
            "tags": [], "filter": {}, "knockback": 0.0
        })
    );
    let loaded: ContactDamage = serde_json::from_value(saved).unwrap();
    assert!(
        loaded.timed.is_empty() && loaded.clock == 0.0,
        "it starts over"
    );
    for bad in [
        json!({ "damage": 1, "offset": [1, 0] }),
        json!({ "damage": 1, "interval": 0 }),
        json!({ "damage": -1 }),
        json!({ "damage": 1, "every": 2 }),
    ] {
        assert!(
            serde_json::from_value::<ContactDamage>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_scene_form_pairs_an_offset_with_its_shape_and_checks_its_tags() {
    let (world, _) = rig();
    assert!(check(&world, &json!({ "damage": 1 })).is_ok());
    assert!(
        check(
            &world,
            &json!({ "damage": 1, "interval": 0.5, "filter": { "all": ["team.player"] }, "knockback": 420, "shape": { "circle": 28 }, "offset": [0, -2] })
        )
        .is_ok()
    );
    let lone = check(&world, &json!({ "damage": 1, "offset": [0, -2] })).unwrap_err();
    assert!(
        lone.contains("an `offset` comes only with its own `shape`"),
        "{lone}"
    );
    assert_eq!(
        check(&world, &json!({ "damage": 1, "tags": ["damage.lava"] })).unwrap_err(),
        "component `contact_damage`, field `tags.0`: unknown tag `damage.lava`"
    );
    assert!(check(&world, &json!({ "damage": 1, "interval": -1 })).is_err());
    assert!(
        check(
            &world,
            &json!({ "damage": 1, "shape": { "circle": 2 }, "offset": [MAX_EXTENT + 1.0, 0] })
        )
        .is_err()
    );
}

#[test]
fn damage_takes_a_number_or_a_map_and_names_the_field_in_errors() {
    let world = typed_rig();
    assert!(check(&world, &json!({ "damage": 1 })).is_ok());
    let typed = json!({
        "damage": { "physical.slash": 2, "elemental.fire": 1 },
        "pen": { "elemental.fire": 0.5 },
        "convert": [{ "from": "physical.slash", "to": "elemental.fire", "pct": 0.25 }],
        "interval": 0.25
    });
    assert!(check(&world, &typed).is_ok());
    for (value, error) in [
        (
            json!({ "damage": { "elemental.fire": -2 } }),
            "component `contact_damage`, field `damage.elemental.fire`: must be finite and >= 0 (got -2)",
        ),
        (
            json!({ "damage": { "elemental.cold": 2 } }),
            "component `contact_damage`, field `damage.elemental.cold`: unknown damage type `elemental.cold`",
        ),
        (
            json!({ "damage": 1, "convert": [{ "from": "chaos", "to": "elemental.fire", "pct": 0.5 }] }),
            "component `contact_damage`, field `convert.0.from`: unknown damage type `chaos`",
        ),
    ] {
        assert_eq!(check(&world, &value).unwrap_err(), error);
    }
    let def: ContactDamageDef = serde_json::from_value(typed).unwrap();
    let lava = ContactDamage::from(def);
    assert_eq!((lava.damage(), lava.interval()), (3.0, 0.25));
    assert_eq!(lava.parts().len(), 2);
    assert_eq!(lava.parts()[0].tags().iter().next(), Some("elemental.fire"));
    assert!(lava.penetration().is_some() && lava.conversion().is_some());
    let saved = serde_json::to_value(&lava).unwrap();
    assert_eq!(
        serde_json::from_value::<ContactDamage>(saved).unwrap(),
        lava
    );
    assert_eq!(
        lava.inspect_rows()[1].1,
        "3 of elemental.fire 1, physical.slash 2 every 0.25 s"
    );
}

#[test]
fn a_map_beside_tags_or_an_all_zero_map_fails() {
    let world = typed_rig();
    for value in [
        json!({ "damage": { "physical.slash": 2 }, "tags": ["elemental.fire"] }),
        json!({ "damage": { "physical.slash": 0 } }),
        json!({ "damage": {} }),
    ] {
        assert!(check(&world, &value).is_err(), "{value}");
    }
    assert!(
        std::panic::catch_unwind(|| {
            let _ = ContactDamage::new(1.0).with_parts([DamagePart::new(0.0)]);
        })
        .is_err()
    );
}

#[test]
fn the_plugin_registers_its_queue_the_grid_and_its_system() {
    let (world, schedule) = rig();
    assert!(world.has_event::<Struck>());
    let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
    assert!(at(HIT_GRID) < at(CONTACT_DAMAGE));
}

#[test]
fn the_damage_stat_writes_only_a_valid_damage() {
    let mut contact = ContactDamage::new(1.0);
    contact.set_damage(3.0);
    contact.set_damage(f32::INFINITY);
    assert_eq!(contact.damage(), 3.0);
}

#[test]
#[should_panic(expected = "seconds must be finite and > 0")]
fn a_zero_interval_panics() {
    let _ = ContactDamage::new(1.0).with_interval(0.0);
}

#[test]
#[should_panic(expected = "damage must be finite and >= 0")]
fn a_nan_contact_damage_panics() {
    let _ = ContactDamage::new(f32::NAN);
}

#[test]
#[should_panic(expected = "offset must be finite and within ±16384")]
fn an_aura_far_off_panics() {
    let _ = ContactDamage::new(1.0).with_shape(HitShape::circle(2.0), Vec2::new(0.0, -20_000.0));
}

#[test]
fn a_target_is_due_at_once_then_each_interval_and_forgotten_after_one_unmet() {
    let mut world = World::new();
    let target = world.spawn();
    let mut spikes = ContactDamage::new(1.0).with_interval(0.5);
    let mut step = 0;
    let mut run = |spikes: &mut ContactDamage, steps: u32, met: bool| {
        let mut hits = Vec::new();
        for _ in 0..steps {
            step += 1;
            spikes.clock += 1.0 / 60.0;
            if met && spikes.due(target) {
                hits.push(step);
            }
            spikes.forget();
        }
        hits
    };
    // Met each step: at once, then 30 steps later.
    assert_eq!(run(&mut spikes, 31, true), [1, 31]);
    // It leaves for ten steps and returns inside the interval: it waits
    // the interval out from its last hit, step 61.
    assert!(run(&mut spikes, 10, false).is_empty());
    assert_eq!(spikes.timed.len(), 1, "kept: met inside the interval");
    assert_eq!(run(&mut spikes, 20, true), [61]);
    // Unmet for a whole interval: forgotten, then hit at once.
    assert!(run(&mut spikes, 30, false).is_empty());
    assert!(spikes.timed.is_empty());
    assert_eq!(run(&mut spikes, 1, true), [92]);
}

/// A World running contact damage, stats and the damage pipeline at 1/60 s steps,
/// with `types` declared as tags and as damage types.
fn contact_rig(types: &[&str]) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(tungsten_core::Time::new());
    world.insert_resource(tungsten_core::CommandBuffer::new());
    let mut schedule = Schedule::new();
    crate::TagsPlugin::new()
        .with_tags(types.iter().copied())
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    crate::StatsPlugin::new().build(&mut schedule, &mut world);
    ContactDamagePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let entry: serde_json::Map<String, serde_json::Value> = types
        .iter()
        .map(|name| ((*name).to_owned(), serde_json::json!({})))
        .collect();
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(serde_json::Value::Object(entry)).unwrap();
    world
        .resource_mut::<tungsten_core::DataRegistry>()
        .insert("game", types);
    (world, schedule)
}

/// One fixed step of 1/60 s.
fn fixed_step(world: &mut World, schedule: &mut Schedule) {
    world
        .resource_mut::<tungsten_core::Time>()
        .advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

/// Each part sent so far: its amount and its kinds by name.
fn parts_sent(world: &World) -> Vec<(f32, Vec<&str>)> {
    world
        .resource::<tungsten_core::EventQueue<crate::DamagedPart>>()
        .iter()
        .map(|part| (part.amount, part.kinds.names(world)))
        .collect()
}

fn typed(amount: f32, name: &str) -> DamagePart {
    DamagePart::new(amount).with_tags(Tags::new([name]))
}

/// A target at the origin: a hurtbox of radius 8 and plenty of health.
fn target(world: &mut World) -> Entity {
    world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::ZERO),
        Hurtbox::new(HitShape::circle(8.0)),
        crate::Health::new(1000.0),
    ))
}

#[test]
fn typed_contact_damage_carries_its_parts_penetration_and_conversion() {
    let (mut world, mut schedule) = contact_rig(&["fire", "slash"]);
    let target = target(&mut world);
    world.insert(target, crate::Resist::new().with_factor("fire", 0.5));
    let lava = ContactDamage::new(0.0)
        .with_shape(HitShape::circle(4.0), Vec2::ZERO)
        .with_parts([typed(2.0, "fire"), typed(4.0, "slash")])
        .with_penetration(crate::Penetration::new().with_points("fire", 0.5))
        .with_conversion(crate::Conversion::new().with_rule("slash", "fire", 0.5));
    let pool = world.spawn_with((tungsten_core::Transform::from_position(Vec2::ZERO), lava));
    fixed_step(&mut world, &mut schedule);
    assert_eq!(
        parts_sent(&world),
        [(4.0, vec!["fire"]), (2.0, vec!["slash"])],
        "fire 2 and half the slash as fire, pierced to a factor of 1; slash 2"
    );
    let sent = world
        .resource::<tungsten_core::EventQueue<crate::Damaged>>()
        .iter()
        .next()
        .copied()
        .unwrap();
    assert_eq!((sent.amount, sent.source), (6.0, Some(pool)));
}

#[test]
fn a_loaded_contact_given_its_touches_keeps_their_schedule() {
    use tungsten_core::physics::{Position, PrevPosition};

    // The target's health each step over 100 steps of an aura's touch: in
    // it, out for ten steps, back, then gone for good. `load` saves the
    // aura at step 40 and puts it back, naming its touches again or not.
    let run = |load: Option<bool>| -> Vec<f32> {
        let (mut world, mut schedule) = contact_rig(&[]);
        let aura = world.spawn_with((
            Position(Vec2::ZERO),
            PrevPosition(Vec2::ZERO),
            ContactDamage::new(1.0)
                .with_interval(0.5)
                .with_shape(HitShape::circle(20.0), Vec2::ZERO),
        ));
        let target = world.spawn_with((
            Position(Vec2::new(10.0, 0.0)),
            PrevPosition(Vec2::new(10.0, 0.0)),
            Hurtbox::new(HitShape::circle(4.0)),
            crate::Health::new(1000.0),
        ));
        let mut health = Vec::new();
        for step in 1..=100 {
            if step == 41
                && let Some(named_again) = load
            {
                let contact = world.get::<ContactDamage>(aura).unwrap();
                // The game's save: the item, and its touches by its own IDs.
                let touches: Vec<(usize, f32, f32)> = contact
                    .touches()
                    .map(|touch| {
                        assert_eq!(touch.target(), target);
                        (0, touch.since_hit(), touch.since_met())
                    })
                    .collect();
                let mut loaded: ContactDamage =
                    serde_json::from_value(serde_json::to_value(contact).unwrap()).unwrap();
                assert_eq!(loaded.touches().count(), 0, "a save holds no entity");
                if named_again {
                    loaded.set_touches(
                        touches
                            .iter()
                            .map(|&(_, hit, met)| ContactTouch::new(target, hit, met)),
                    );
                }
                world.insert(aura, loaded);
            }
            let x = if (46..=55).contains(&step) || step >= 70 {
                100.0
            } else {
                10.0
            };
            world.get_mut::<Position>(target).unwrap().0 = Vec2::new(x, 0.0);
            world.get_mut::<PrevPosition>(target).unwrap().0 = Vec2::new(x, 0.0);
            fixed_step(&mut world, &mut schedule);
            health.push(world.get::<crate::Health>(target).unwrap().current());
        }
        health
    };
    let hits = |health: &[f32]| -> Vec<usize> {
        let mut last = 1000.0;
        let mut at = Vec::new();
        for (i, &now) in health.iter().enumerate() {
            if now < last {
                at.push(i + 1);
            }
            last = now;
        }
        at
    };
    let uninterrupted = run(None);
    // At once, an interval on, the interval out from step 31 once back.
    assert_eq!(hits(&uninterrupted), [1, 31, 61]);
    assert_eq!(run(Some(true)), uninterrupted, "the schedule kept");
    // Unnamed, it starts over: at once at the load, and back at 56 inside
    // that hit's interval, then gone.
    assert_eq!(hits(&run(Some(false))), [1, 31, 41]);
}

#[test]
fn a_contact_touch_refuses_negative_or_non_finite_seconds() {
    let mut world = World::new();
    let target = world.spawn();
    let touch = ContactTouch::new(target, 0.0, 0.25);
    assert_eq!(
        (touch.target(), touch.since_hit(), touch.since_met()),
        (target, 0.0, 0.25)
    );
    for (hit, met) in [
        (-0.1, 0.0),
        (0.0, -0.1),
        (f32::NAN, 0.0),
        (0.0, f32::INFINITY),
    ] {
        assert!(
            std::panic::catch_unwind(|| ContactTouch::new(target, hit, met)).is_err(),
            "{hit}, {met}"
        );
    }
    // Touches go back sorted by target and each once.
    let other = world.spawn();
    let mut spikes = ContactDamage::new(1.0);
    spikes.set_touches([
        ContactTouch::new(other, 0.1, 0.0),
        ContactTouch::new(target, 0.2, 0.0),
        ContactTouch::new(other, 0.4, 0.0),
    ]);
    let back: Vec<(Entity, f32)> = spikes
        .touches()
        .map(|touch| (touch.target(), touch.since_hit()))
        .collect();
    assert_eq!(back, [(target, 0.2), (other, 0.1)]);
}
