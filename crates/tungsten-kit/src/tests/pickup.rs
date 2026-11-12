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

/// A world with the physics, the oscillator and the pickups, its gravity
/// `gravity` px/s² down, and a still collector at the origin whose hurtbox,
/// a point, takes what reaches its centre, drawing within 300 px.
fn pickup_rig(gravity: f32) -> (World, Schedule, Entity) {
    use crate::{Hurtbox, Stats};
    use tungsten_core::physics::PhysicsPlugin;

    use tungsten_core::{PhysicsConfig, TilemapRegistry, Time, Transform};

    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(TilemapRegistry::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, gravity),
        ..PhysicsConfig::default()
    });
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    OscillatorPlugin.build(&mut schedule, &mut world);
    PickupPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let collector = world.spawn_with((
        Transform::from_position(Vec2::ZERO),
        Position(Vec2::ZERO),
        Hurtbox::new(HitShape::circle(0.25)),
        Stats::new().with(Stats::PICKUP_RADIUS, 300.0),
    ));
    (world, schedule, collector)
}

fn pickup_step(world: &mut World, schedule: &mut Schedule) {
    world
        .resource_mut::<tungsten_core::Time>()
        .advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

#[test]
fn a_magnet_draws_a_pickup_with_a_body() {
    use tungsten_core::Bundle;
    use tungsten_core::physics::{Collider, RigidBodyBundle};

    // Pickups 150 px above the collector drawn down at 300 px/s: a
    // kinematic one, and one bobbing sideways on an oscillator too, never
    // pass the collector along the pull; a dynamic one under gravity 2,000,
    // pulling the same way, by at most g · dt² plus 1e-3 px. Each is
    // collected.
    let dt = 1.0 / 60.0;
    for kind in ["kinematic", "bobbing", "dynamic"] {
        let dynamic = kind == "dynamic";
        let (mut world, mut schedule, _) = pickup_rig(if dynamic { 2_000.0 } else { 0.0 });
        let at = Vec2::new(0.0, -150.0);
        let pickup = Pickup::new(HitShape::circle(0.25)).with_magnet(300.0);
        let gem = if dynamic {
            world.spawn_with(
                RigidBodyBundle::dynamic(Position(at), Collider::circle(0.25)).with((pickup,)),
            )
        } else {
            let gem = world.spawn_with(
                RigidBodyBundle::kinematic(Position(at), Collider::circle(0.25)).with((pickup,)),
            );
            if kind == "bobbing" {
                world.insert(gem, crate::Oscillator::position(Vec2::new(12.0, 0.0), 0.5));
                world.insert(gem, PrevPosition(at));
            }
            gem
        };
        let bound = if dynamic {
            2_000.0 * dt * dt + 1e-3
        } else {
            1e-4
        };
        let mut farthest = f32::NEG_INFINITY;
        let mut collected = false;
        for _ in 0..120 {
            pickup_step(&mut world, &mut schedule);
            let Some(pickup) = world.get::<Pickup>(gem) else {
                collected = true;
                break;
            };
            if pickup.is_collected() {
                collected = true;
                break;
            }
            farthest = farthest.max(world.get::<Position>(gem).unwrap().0.y);
        }
        assert!(
            farthest <= bound,
            "{kind}: {farthest} px past the collector"
        );
        assert!(collected, "{kind}: collected");
    }
}

#[test]
fn a_pickup_falls_onto_the_ground() {
    use tungsten_core::physics::{Collider, PhysicsLayers, RigidBody, RigidBodyBundle};
    use tungsten_core::{LayerKind, TilemapData, TilemapInstance, TilemapRegistry};

    // A tile floor whose top is at y 0 under x 0 to 160, a static slab whose
    // top is at y −40 over x 200 to 240, and a dynamic and a kinematic body
    // in the first pickup's way: pickups falling at 60 px/s rest the skin
    // above the floor and the slab, passing the bodies.
    let (mut world, mut schedule, _) = pickup_rig(0.0);
    world.remove_resource::<PhysicsLayers>();
    let mut map = TilemapData::new(16, 16, 10, 1);
    let solid = map.add_tile("solid");
    let layer = map.add_layer("solid", LayerKind::Collision);
    map.fill(layer, solid);
    world
        .resource_mut::<TilemapRegistry>()
        .insert("floor".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("floor", Vec2::new(0.0, 0.0)));
    world.spawn_with(RigidBodyBundle::r#static(
        Position::new(220.0, -30.0),
        Collider::aabb(Vec2::new(20.0, 10.0)),
    ));
    world.spawn_with(RigidBodyBundle::dynamic(
        Position::new(80.0, -60.0),
        Collider::aabb(Vec2::new(10.0, 5.0)),
    ));
    world.spawn_with(RigidBodyBundle::kinematic(
        Position::new(80.0, -30.0),
        Collider::aabb(Vec2::new(10.0, 5.0)),
    ));
    let falling = |world: &mut World, at: Vec2| {
        let coin = Pickup::new(HitShape::circle(4.0))
            .with_filter(TagFilter::new().all(["team.none"]))
            .with_fall(60.0);
        world.spawn_with((Position(at), PrevPosition(at), coin))
    };
    let over_floor = falling(&mut world, Vec2::new(80.0, -100.0));
    let over_slab = falling(&mut world, Vec2::new(220.0, -100.0));
    for _ in 0..180 {
        pickup_step(&mut world, &mut schedule);
    }
    let y = |world: &World, entity| world.get::<Position>(entity).unwrap().0.y;
    assert!(
        (y(&world, over_floor) - (-4.0 - 0.01)).abs() < 1e-3,
        "on the floor: {}",
        y(&world, over_floor)
    );
    assert!(
        (y(&world, over_slab) - (-44.0 - 0.01)).abs() < 1e-3,
        "on the slab: {}",
        y(&world, over_slab)
    );
    pickup_step(&mut world, &mut schedule);
    assert!(
        (y(&world, over_floor) - (-4.0 - 0.01)).abs() < 1e-3,
        "at rest"
    );

    // Within a magnet's reach it is pulled instead of falling.
    let (mut world, mut schedule, _) = pickup_rig(0.0);
    let at = Vec2::new(-100.0, 0.0);
    let pulled = world.spawn_with((
        Position(at),
        PrevPosition(at),
        Pickup::new(HitShape::circle(0.25))
            .with_magnet(60.0)
            .with_fall(60.0),
    ));
    pickup_step(&mut world, &mut schedule);
    let moved = world.get::<Position>(pulled).unwrap().0;
    assert!(
        moved.abs_diff_eq(Vec2::new(-99.0, 0.0), 1e-4),
        "pulled, not falling: {moved}"
    );

    // Fall layers by number and by name: a coin landing on layer 1 passes a
    // static slab in layer 0 and lands on one in layer 1.
    let (mut world, mut schedule, _) = pickup_rig(0.0);
    world.insert_resource(PhysicsLayers::new([("ground".to_owned(), 1)]));
    world.spawn_with(
        RigidBodyBundle::r#static(
            Position::new(100.0, -30.0),
            Collider::aabb(Vec2::new(40.0, 10.0)),
        )
        .with_body(RigidBody::r#static().with_layers(1 << 0)),
    );
    world.spawn_with(
        RigidBodyBundle::r#static(
            Position::new(100.0, 30.0),
            Collider::aabb(Vec2::new(40.0, 10.0)),
        )
        .with_body(RigidBody::r#static().with_layers(1 << 1)),
    );
    let components = world.resource::<ComponentRegistry>();
    for layers in [json!([1]), json!(["ground"])] {
        components
            .check_in(
                PICKUP,
                &json!({ "shape": { "circle": 4 }, "fall": 60, "fall_layers": layers }),
                &world,
            )
            .unwrap();
    }
    let refused = components
        .check_in(
            PICKUP,
            &json!({ "shape": { "circle": 4 }, "fall_layers": [0, "sky"] }),
            &world,
        )
        .unwrap_err();
    assert!(
        matches!(
            &refused,
            tungsten_core::ComponentError::MissingAsset { field, kind: "physics layer", id, .. }
                if field == "fall_layers[1]" && id == "sky"
        ),
        "{refused}"
    );
    let coin: PrefabData = serde_json::from_value(json!({ "components": { "pickup": {
        "shape": { "circle": 4 }, "fall": 60, "fall_layers": ["ground"]
    } } }))
    .unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("coin".to_string(), coin);
    world.insert_resource(prefabs);
    let coin = world.spawn_prefab("coin", [100.0, -100.0]).unwrap();
    assert_eq!(world.get::<Pickup>(coin).unwrap().fall_layers(), 1 << 1);
    for _ in 0..180 {
        pickup_step(&mut world, &mut schedule);
    }
    let landed = world.get::<Position>(coin).unwrap().0.y;
    assert!(
        (landed - (20.0 - 4.0 - 0.01)).abs() < 1e-3,
        "on layer 1's slab: {landed}"
    );
}

#[test]
fn a_pickups_fall_round_trips_its_data_and_save() {
    let coin = Pickup::new(HitShape::circle(4.0))
        .with_fall(60.0)
        .with_fall_layers(0b110);
    assert_eq!((coin.fall(), coin.fall_layers()), (60.0, 0b110));
    let saved = serde_json::to_value(&coin).unwrap();
    assert_eq!(
        (saved["fall"].clone(), saved["fall_layers"].clone()),
        (json!(60.0), json!(6))
    );
    assert_eq!(
        serde_json::from_value::<Pickup>(saved.clone()).unwrap(),
        coin
    );
    let plain = serde_json::to_value(Pickup::new(HitShape::circle(4.0))).unwrap();
    assert!(plain.get("fall").is_none() && plain.get("fall_layers").is_none());
    let mut old = saved;
    let object = old.as_object_mut().unwrap();
    object.remove("fall");
    object.remove("fall_layers");
    let loaded: Pickup = serde_json::from_value(old).unwrap();
    assert_eq!(
        (loaded.fall(), loaded.fall_layers()),
        (0.0, tungsten_core::physics::RigidBody::ALL_LAYERS)
    );
    assert!(
        serde_json::from_value::<Pickup>(json!({ "shape": { "circle": 4 }, "fall": -1 })).is_err()
    );
}
