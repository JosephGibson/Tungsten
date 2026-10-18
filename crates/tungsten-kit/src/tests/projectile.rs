use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, Stage};

use crate::lifecycle::ended::KIT_DESPAWN;
use crate::overlap::hit::HIT_GRID;

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
