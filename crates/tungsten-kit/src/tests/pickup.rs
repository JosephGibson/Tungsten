use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, PrefabData, PrefabRegistry, Stage};

use crate::foundation::tags::TagsPlugin;
use crate::lifecycle::ended::KIT_DESPAWN;
use crate::{OscillatorPlugin, SpeedLimitPlugin};

#[test]
fn a_pickup_reads_back_what_it_was_built_with_and_is_data() {
    let gem = Pickup::new(HitShape::circle(6.0))
        .with_offset(Vec2::new(0.0, -2.0))
        .with_filter(TagFilter::new().all(["team.player"]))
        .with_magnet(240.0);
    assert_eq!(
        (gem.shape(), gem.offset(), gem.magnet()),
        (HitShape::circle(6.0), Vec2::new(0.0, -2.0), 240.0)
    );
    assert!(!gem.filter().is_everything() && !gem.is_collected());
    let saved = serde_json::to_value(&gem).unwrap();
    assert_eq!(
        saved,
        json!({
            "shape": { "circle": 6.0 }, "offset": [0.0, -2.0],
            "filter": { "all": ["team.player"] }, "magnet": 240.0, "collected": false
        })
    );
    assert_eq!(serde_json::from_value::<Pickup>(saved).unwrap(), gem);
    for bad in [
        json!({ "shape": { "circle": 6 }, "magnet": -1 }),
        json!({ "magnet": 1 }),
        json!({ "shape": { "circle": 6 }, "worth": 5 }),
    ] {
        assert!(
            serde_json::from_value::<Pickup>(bad.clone()).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn the_scene_form_spawns_a_position_and_its_history_at_the_spawn_point() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    TagsPlugin::new()
        .with_tags(["team.player"])
        .build(&mut schedule, &mut world);
    PickupPlugin.build(&mut schedule, &mut world);
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in(
                PICKUP,
                &json!({ "shape": { "circle": 6 }, "filter": { "all": ["team.player"] }, "magnet": 240 }),
                &world
            )
            .is_ok()
    );
    assert_eq!(
        components
            .check_in(
                PICKUP,
                &json!({ "shape": { "circle": 6 }, "filter": { "any": ["team.npc"] } }),
                &world
            )
            .unwrap_err()
            .to_string(),
        "component `pickup`, field `filter.any.0`: unknown tag `team.npc`"
    );
    let coin: PrefabData =
        serde_json::from_value(json!({ "components": { "pickup": { "shape": { "circle": 6 } } } }))
            .unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("coin".to_string(), coin);
    world.insert_resource(prefabs);
    let entity = world.spawn_prefab("coin", [30.0, 40.0]).unwrap();
    assert_eq!(
        world.get::<Position>(entity).unwrap().0,
        Vec2::new(30.0, 40.0)
    );
    assert_eq!(
        world.get::<PrevPosition>(entity).unwrap().0,
        Vec2::new(30.0, 40.0)
    );
}

#[test]
fn the_magnet_moves_after_the_oscillator_and_before_the_speed_limit() {
    for reversed in [false, true] {
        let mut world = World::new();
        let mut schedule = Schedule::new();
        if reversed {
            PickupPlugin.build(&mut schedule, &mut world);
            SpeedLimitPlugin.build(&mut schedule, &mut world);
            OscillatorPlugin.build(&mut schedule, &mut world);
        } else {
            OscillatorPlugin.build(&mut schedule, &mut world);
            SpeedLimitPlugin.build(&mut schedule, &mut world);
            PickupPlugin.build(&mut schedule, &mut world);
        }
        schedule.resolve().unwrap();
        assert!(world.has_event::<PickedUp>());
        let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
        let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
        assert!(at("oscillator") < at(PICKUP_MAGNET) && at(PICKUP_MAGNET) < at("speed_limit"));
        assert!(at(PICKUP_TICK) < at(KIT_DESPAWN), "{order:?}");
    }
}

#[test]
fn a_picked_up_reads_back_its_fields() {
    let mut world = World::new();
    let (gem, hero) = (world.spawn(), world.spawn());
    let event = PickedUp::new(gem, hero, Vec2::new(5.0, 6.0));
    assert_eq!(
        (event.entity, event.by, event.position),
        (gem, hero, Vec2::new(5.0, 6.0))
    );
}

#[test]
#[should_panic(expected = "speed must be finite and >= 0")]
fn a_negative_magnet_panics() {
    let _ = Pickup::new(HitShape::circle(6.0)).with_magnet(-1.0);
}

#[test]
fn the_magnet_takes_the_nearest_collector_in_reach_per_filter_as_a_full_scan_would() {
    use crate::{Hurtbox, Stats, Tags};
    use tungsten_core::{Time, Transform};

    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PickupPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    // Collectors on a ring of radii, two teams; pickups between them.
    let mut collectors = Vec::new();
    for i in 0..40 {
        let angle = i as f32 * 0.7;
        let at = Vec2::new(angle.cos(), angle.sin()) * (50.0 + 7.0 * i as f32);
        let team = if i % 3 == 0 { "team.red" } else { "team.blue" };
        let radius = 20.0 + (i % 5) as f32 * 30.0;
        collectors.push((
            world.spawn_with((
                Transform::from_position(at),
                Hurtbox::new(HitShape::circle(2.0)),
                Tags::new([team]),
                Stats::new().with(Stats::PICKUP_RADIUS, radius),
            )),
            at,
            radius,
            team,
        ));
    }
    let mut pickups = Vec::new();
    for i in 0..30 {
        let at = Vec2::new(i as f32 * 13.0 - 190.0, (i * 7 % 11) as f32 * 30.0 - 150.0);
        let team = if i % 2 == 0 { "team.red" } else { "team.blue" };
        let pickup = Pickup::new(HitShape::circle(1.0))
            .with_filter(TagFilter::new().all([team]))
            .with_magnet(60.0);
        pickups.push((
            world.spawn_with((Position(at), PrevPosition(at), pickup)),
            at,
            team,
        ));
    }
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    let mut pulled = 0;
    for (entity, start, team) in pickups {
        let nearest = collectors
            .iter()
            .filter(|(_, at, radius, collects)| *collects == team && start.distance(*at) <= *radius)
            .min_by(|a, b| start.distance(a.1).total_cmp(&start.distance(b.1)))
            .map(|(_, at, ..)| *at);
        let moved = world.get::<Position>(entity).unwrap().0;
        match nearest {
            Some(at) => {
                pulled += 1;
                let step = (at - start).normalize() * 1.0_f32.min(start.distance(at));
                assert!(
                    (moved - (start + step)).length() < 1e-4,
                    "{moved} against {}",
                    start + step
                );
            }
            None => assert_eq!(moved, start, "no collector reaches it"),
        }
    }
    assert!(
        (1..30).contains(&pulled),
        "some pulled and some left: {pulled}"
    );
}
