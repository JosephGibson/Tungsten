use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, Stage};

use crate::damage::pipeline::DamagePart;
use crate::lifecycle::ended::KIT_DESPAWN;
use crate::overlap::hit::{HIT_GRID, HitShape, Hitbox, Hurtbox};

#[test]
fn a_projectile_reads_back_its_pierce_range_and_bounces() {
    let plain = Projectile::new();
    assert_eq!(plain, Projectile::default());
    assert_eq!(
        (plain.pierce(), plain.range(), plain.bounces()),
        (0, None, 0)
    );
    assert_eq!(
        (plain.hits(), plain.travelled(), plain.is_spent()),
        (0, 0.0, false)
    );
    let lance = Projectile::new()
        .with_pierce(2)
        .with_range(300.0)
        .with_bounces(1);
    assert_eq!(
        (lance.pierce(), lance.range(), lance.bounces()),
        (2, Some(300.0), 1)
    );
}

#[test]
fn a_projectile_is_data_with_its_progress() {
    let mut bolt = Projectile::new().with_range(600.0);
    bolt.hits = 1;
    bolt.travelled = 120.5;
    bolt.bounced = 1;
    bolt.spent = true;
    let saved = serde_json::to_value(&bolt).unwrap();
    assert_eq!(
        saved,
        json!({ "pierce": 0, "range": 600.0, "bounces": 0, "hits": 1, "travelled": 120.5, "bounced": 1, "spent": true })
    );
    assert_eq!(serde_json::from_value::<Projectile>(saved).unwrap(), bolt);
    for bad in [
        json!({ "range": 0 }),
        json!({ "travelled": -1 }),
        json!({ "pierce": -1 }),
        json!({ "speed": 3 }),
    ] {
        assert!(
            serde_json::from_value::<Projectile>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_scene_form_takes_each_field_optionally() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    ProjectilePlugin.build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    for good in [
        json!({}),
        json!({ "pierce": 1, "range": 600, "bounces": 2 }),
    ] {
        assert!(
            components.check_in(PROJECTILE, &good, &world).is_ok(),
            "{good}"
        );
    }
    for bad in [
        json!({ "range": -5 }),
        json!({ "hits": 1 }),
        json!({ "bounces": 1.5 }),
    ] {
        assert!(
            components.check_in(PROJECTILE, &bad, &world).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_plugin_registers_its_queues_its_tick_after_the_grid_and_the_despawn() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    ProjectilePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    assert!(world.has_event::<ProjectileExpired>() && world.has_event::<Struck>());
    let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
    assert!(at(HIT_GRID) < at(PROJECTILE_TICK) && at(PROJECTILE_TICK) < at(KIT_DESPAWN));
    let expired = ProjectileExpired::new(world.spawn(), glam::Vec2::new(1.0, 2.0));
    assert_eq!(expired.position, glam::Vec2::new(1.0, 2.0));
}

#[test]
#[should_panic(expected = "distance must be finite and > 0")]
fn a_zero_range_panics() {
    let _ = Projectile::new().with_range(0.0);
}

/// A World running projectiles, stats and the damage pipeline at 1/60 s steps,
/// with `types` declared as tags and as damage types.
fn projectile_rig(types: &[&str]) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(tungsten_core::Time::new());
    world.insert_resource(tungsten_core::CommandBuffer::new());
    let mut schedule = Schedule::new();
    crate::TagsPlugin::new()
        .with_tags(types.iter().copied())
        .build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    crate::StatsPlugin::new().build(&mut schedule, &mut world);
    ProjectilePlugin.build(&mut schedule, &mut world);
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
fn a_typed_projectiles_hits_carry_its_hitboxs_parts() {
    let (mut world, mut schedule) = projectile_rig(&["fire", "ice"]);
    target(&mut world);
    let bolt = world.spawn_with((
        tungsten_core::Transform::from_position(Vec2::ZERO),
        Hitbox::new(HitShape::circle(2.0), 0.0)
            .with_parts([typed(3.0, "fire"), DamagePart::new(1.0)])
            .with_tags(Tags::new(["ice"])),
        Projectile::new(),
    ));
    fixed_step(&mut world, &mut schedule);
    assert_eq!(
        parts_sent(&world),
        [(3.0, vec!["fire"]), (1.0, vec!["ice"])],
        "its hitbox's parts, the untyped one of its tags"
    );
    assert_eq!(world.get::<Projectile>(bolt).unwrap().hits(), 1);
}
