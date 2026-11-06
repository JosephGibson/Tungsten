use super::*;
use tungsten_core::physics::{
    Collider, PhysicsBuffers, PhysicsConfig, PhysicsPlugin, RigidBodyBundle,
};
use tungsten_core::{Bundle, Plugin, Schedule, Stage, TilemapRegistry, Time};

use crate::{
    FollowPlugin, Health, HealthPlugin, Hit, LinearMotion, LinearMotionPlugin,
    PlatformerControllerPlugin, TagsPlugin, kill,
};

const DT: f32 = 1.0 / 60.0;
const OFFSET: Vec2 = Vec2::new(8.0, -30.0);

/// The physics at `gravity` px/s² down, the tags, health, a linear mover,
/// the platformer controller and the follow system, which places held
/// bodies.
fn rig(gravity: f32) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, gravity),
        ..PhysicsConfig::default()
    });
    world.insert_resource(TilemapRegistry::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TagsPlugin::new()
        .with_tags(["status.frozen"])
        .build(&mut schedule, &mut world);
    HealthPlugin.build(&mut schedule, &mut world);
    LinearMotionPlugin.build(&mut schedule, &mut world);
    PlatformerControllerPlugin.build(&mut schedule, &mut world);
    FollowPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn steps(world: &mut World, schedule: &mut Schedule, count: usize) {
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(DT);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    }
}

/// A kinematic porter at `at` moving at `velocity`, facing right, with
/// tags and health.
fn porter(world: &mut World, at: Vec2, velocity: Vec2) -> Entity {
    world.spawn_with(
        RigidBodyBundle::kinematic(Position(at), Collider::aabb(Vec2::new(10.0, 20.0))).with((
            LinearMotion::new(velocity),
            Facing::new(),
            Tags::new(Vec::<String>::new()),
            Health::new(5.0),
        )),
    )
}

/// A dynamic crate at `at`.
fn crate_at(world: &mut World, at: Vec2) -> Entity {
    world.spawn_with(RigidBodyBundle::dynamic(
        Position(at),
        Collider::aabb(Vec2::splat(6.0)),
    ))
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

fn velocity(world: &World, entity: Entity) -> Vec2 {
    world.get::<Velocity>(entity).unwrap().0
}

fn kind(world: &World, entity: Entity) -> BodyKind {
    world.get::<RigidBody>(entity).unwrap().kind
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).length() < 1e-3
}

#[test]
fn a_held_body_rides_at_its_carriers_offset_mirrored() {
    let (mut world, mut schedule) = rig(0.0);
    let porter = porter(&mut world, Vec2::new(100.0, 100.0), Vec2::new(120.0, 0.0));
    let load = crate_at(&mut world, Vec2::ZERO);
    assert!(carry(&mut world, porter, load, Hold::new(OFFSET)));
    // Placed at once, a sensor whose last point is its new one.
    assert_eq!(position(&world, load), Vec2::new(108.0, 70.0));
    assert_eq!(
        world.get::<PrevPosition>(load).unwrap().0,
        Vec2::new(108.0, 70.0)
    );
    assert_eq!(kind(&world, load), BodyKind::Sensor);
    assert_eq!(carried_by(&world, porter), Some(load));
    for step in 0..30 {
        steps(&mut world, &mut schedule, 1);
        assert!(
            close(position(&world, load), position(&world, porter) + OFFSET),
            "step {step}: at its offset"
        );
        assert_eq!(velocity(&world, load), Vec2::new(120.0, 0.0), "step {step}");
    }
    assert!(close(position(&world, porter), Vec2::new(160.0, 100.0)));
    // Facing left, the offset is mirrored; facing up, it is as written.
    world.get_mut::<Facing>(porter).unwrap().face(Vec2::NEG_X);
    steps(&mut world, &mut schedule, 1);
    assert!(close(
        position(&world, load),
        position(&world, porter) + Vec2::new(-8.0, -30.0)
    ));
    world.get_mut::<Facing>(porter).unwrap().face(Vec2::NEG_Y);
    steps(&mut world, &mut schedule, 1);
    assert!(close(
        position(&world, load),
        position(&world, porter) + OFFSET
    ));
    // A carrier without a `Facing` holds it as written too.
    world.remove_component::<Facing>(porter);
    steps(&mut world, &mut schedule, 1);
    assert!(close(
        position(&world, load),
        position(&world, porter) + OFFSET
    ));
}

#[test]
fn a_release_restores_wakes_and_launches() {
    let (mut world, mut schedule) = rig(600.0);
    // A crate asleep on a floor.
    world.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(100.0, 110.0)),
        Collider::aabb(Vec2::new(200.0, 10.0)),
    ));
    let load = crate_at(&mut world, Vec2::new(100.0, 94.0));
    steps(&mut world, &mut schedule, 90);
    assert!(
        world.resource::<PhysicsBuffers>().is_sleeping(load),
        "asleep before the carry"
    );
    let own = *world.get::<RigidBody>(load).unwrap();
    let troll = world.spawn_with((Position(Vec2::new(100.0, 60.0)),));
    assert!(carry(
        &mut world,
        troll,
        load,
        Hold::new(Vec2::new(0.0, -20.0))
    ));
    // Let go before a step: its own body back, its velocity set, woken.
    let throw = Vec2::new(150.0, -200.0);
    assert_eq!(release(&mut world, troll, throw), Some(load));
    assert_eq!(*world.get::<RigidBody>(load).unwrap(), own);
    assert_eq!(velocity(&world, load), throw);
    assert!(
        !world.resource::<PhysicsBuffers>().is_sleeping(load),
        "woken"
    );
    assert_eq!(carried_by(&world, troll), None);
    assert_eq!(release(&mut world, troll, throw), None, "nothing held");
    steps(&mut world, &mut schedule, 1);
    let at = position(&world, load);
    assert!(at.x > 100.0 && at.y < 40.0, "thrown: {at}");

    // A held controller takes the release as a launch: thrown up, it rises
    // where it would have fallen.
    let hero = world.spawn_with(
        RigidBodyBundle::kinematic(
            Position(Vec2::new(300.0, 0.0)),
            Collider::aabb(Vec2::new(10.0, 20.0)),
        )
        .with((PlatformerController::new(200.0, 400.0),)),
    );
    let giant = world.spawn_with((Position(Vec2::new(300.0, -100.0)),));
    // Its still first step taken, as a controller's is.
    steps(&mut world, &mut schedule, 1);
    assert!(carry(&mut world, giant, hero, Hold::new(Vec2::ZERO)));
    steps(&mut world, &mut schedule, 10);
    assert!(close(position(&world, hero), Vec2::new(300.0, -100.0)));
    assert_eq!(kind(&world, hero), BodyKind::Sensor);
    assert_eq!(
        release(&mut world, giant, Vec2::new(0.0, -300.0)),
        Some(hero)
    );
    assert_eq!(kind(&world, hero), BodyKind::Kinematic);
    steps(&mut world, &mut schedule, 1);
    assert!(
        position(&world, hero).y < -100.0,
        "launched up: {}",
        position(&world, hero)
    );
}

#[test]
fn a_carry_drops_at_death_or_its_filter() {
    let (mut world, mut schedule) = rig(0.0);
    let frozen = TagFilter::new().any(["status.frozen"]);
    let porters: Vec<Entity> = (0..3)
        .map(|n| {
            let at = Vec2::new(100.0 + 100.0 * n as f32, 100.0);
            porter(&mut world, at, Vec2::new(60.0, 0.0))
        })
        .collect();
    let loads: Vec<Entity> = (0..3)
        .map(|n| crate_at(&mut world, Vec2::new(100.0 * n as f32, 0.0)))
        .collect();
    for (&porter, &load) in porters.iter().zip(&loads) {
        let hold = Hold::new(OFFSET).with_drop(frozen.clone());
        assert!(carry(&mut world, porter, load, hold));
    }
    steps(&mut world, &mut schedule, 5);
    // The first dies, the second freezes, the third is despawned.
    assert_eq!(kill(&mut world, porters[0]), Hit::Killed);
    world
        .get_mut::<Tags>(porters[1])
        .unwrap()
        .insert("status.frozen");
    world.despawn(porters[2]);
    let held: Vec<Vec2> = loads.iter().map(|&load| position(&world, load)).collect();
    steps(&mut world, &mut schedule, 3);
    for (n, &load) in loads.iter().enumerate() {
        assert!(!world.has::<Carried>(load), "{n}: let go");
        assert_eq!(kind(&world, load), BodyKind::Dynamic, "{n}: its own body");
        assert_eq!(velocity(&world, load), Vec2::ZERO, "{n}: at rest");
        assert_eq!(position(&world, load), held[n], "{n}: where it was held");
    }
    assert_eq!(carried_by(&world, porters[0]), None);
    assert_eq!(carried_by(&world, porters[1]), None);
    // Nothing held in a World that does not save: the pass stops.
    assert!(!world.has_resource::<Carrying>());

    // Without a drop filter, the carrier's tags hold on.
    let calm = porter(&mut world, Vec2::new(500.0, 100.0), Vec2::ZERO);
    let load = crate_at(&mut world, Vec2::new(500.0, 0.0));
    assert!(carry(&mut world, calm, load, Hold::new(OFFSET)));
    world.get_mut::<Tags>(calm).unwrap().insert("status.frozen");
    steps(&mut world, &mut schedule, 3);
    assert_eq!(carried_by(&world, calm), Some(load));
    assert!(close(position(&world, load), Vec2::new(508.0, 70.0)));
}

#[test]
fn a_carry_refuses_itself_a_second_a_chain_and_a_cycle() {
    let (mut world, _) = rig(0.0);
    let hold = || Hold::new(OFFSET);
    let [a, b, c] = [0.0, 50.0, 100.0].map(|x| crate_at(&mut world, Vec2::new(x, 0.0)));
    let pointless = world.spawn();
    let bodiless = world.spawn_with((Position(Vec2::ZERO),));
    let placeless = world.spawn_with((RigidBody::dynamic(),));
    assert!(!carry(&mut world, a, a, hold()), "itself");
    assert!(
        !carry(&mut world, pointless, b, hold()),
        "a carrier without a point"
    );
    assert!(
        !carry(&mut world, a, bodiless, hold()),
        "a body without a RigidBody"
    );
    assert!(
        !carry(&mut world, a, placeless, hold()),
        "a body without a Position"
    );
    assert!(carry(&mut world, a, b, hold()));
    assert!(!carry(&mut world, c, b, hold()), "a body already held");
    assert!(!carry(&mut world, a, c, hold()), "a second load");
    assert!(!carry(&mut world, b, c, hold()), "a held carrier: a chain");
    assert!(
        !carry(&mut world, c, a, hold()),
        "a body that carries: a cycle"
    );
    // Each refusal left everything as it was.
    assert_eq!(carried_by(&world, a), Some(b));
    assert_eq!(carried_by(&world, b), None);
    assert_eq!(carried_by(&world, c), None);
    assert_eq!(kind(&world, a), BodyKind::Dynamic);
    assert_eq!(kind(&world, c), BodyKind::Dynamic);
    assert_eq!(kind(&world, b), BodyKind::Sensor);
}
