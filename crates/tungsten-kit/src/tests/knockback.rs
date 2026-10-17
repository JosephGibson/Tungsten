use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle};
use tungsten_core::{Bundle, PhysicsConfig, Stage, Time, Transform};

use crate::{
    Health, PlatformerControllerPlugin, TopDownMover, TopDownMoverPlugin, damage, spawn_child,
};

const DT: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    TopDownMoverPlugin.build(&mut schedule, &mut world);
    PlatformerControllerPlugin.build(&mut schedule, &mut world);
    KnockbackPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn steps(world: &mut World, schedule: &mut Schedule, count: usize) {
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(DT);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    }
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

#[test]
fn it_refuses_an_entity_without_a_knockback_a_dead_one_and_a_push_that_is_not_finite() {
    let mut world = World::new();
    let wall = world.spawn();
    assert!(!knock_back(&mut world, wall, Vec2::X));
    let brute = world.spawn_with((Knockback::new(0.2), Health::new(2.0)));
    assert!(!knock_back(&mut world, brute, Vec2::new(f32::NAN, 0.0)));
    assert!(!world.get::<Knockback>(brute).unwrap().is_active());
    assert!(knock_back(&mut world, brute, Vec2::X));
    damage(&mut world, brute, 2.0);
    assert!(
        !knock_back(&mut world, brute, Vec2::new(-5.0, 0.0)),
        "a dead one"
    );
    assert_eq!(
        world.get::<Knockback>(brute).unwrap().push(),
        Vec2::X,
        "the old push"
    );
}

#[test]
fn a_bodiless_push_falls_linearly_to_zero_over_its_seconds() {
    let (mut world, mut schedule) = rig();
    let rover = world.spawn_with((
        Knockback::new(0.25).with_resist(0.2),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    assert!(knock_back(&mut world, rover, Vec2::new(500.0, 0.0)));
    let mut last = f32::INFINITY;
    for n in 1..=15 {
        let before = position(&world, rover).x;
        steps(&mut world, &mut schedule, 1);
        let moved = position(&world, rover).x - before;
        assert!(moved > 0.0 && moved < last, "{n}: {moved}");
        last = moved;
    }
    // 400 px/s falling to 0 over a quarter second: 50 px.
    assert!(
        (position(&world, rover).x - 50.0).abs() < 1e-3,
        "{}",
        position(&world, rover)
    );
    assert!(!world.get::<Knockback>(rover).unwrap().is_active());
    steps(&mut world, &mut schedule, 5);
    assert!((position(&world, rover).x - 50.0).abs() < 1e-3);
}

#[test]
fn a_kinematic_body_with_no_driver_slides_while_pushed_and_stops_when_it_ends() {
    let (mut world, mut schedule) = rig();
    let crate_ = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(8.0)))
            .with((Knockback::new(0.2),)),
    );
    assert!(knock_back(&mut world, crate_, Vec2::new(0.0, -300.0)));
    steps(&mut world, &mut schedule, 1);
    assert!(world.get::<Velocity>(crate_).unwrap().0.y < 0.0);
    steps(&mut world, &mut schedule, 12);
    // 300 px/s falling to 0 over 0.2 s: 30 px, then still.
    assert!(
        (position(&world, crate_).y + 30.0).abs() < 1e-2,
        "{}",
        position(&world, crate_)
    );
    assert_eq!(world.get::<Velocity>(crate_).unwrap().0, Vec2::ZERO);
    let at = position(&world, crate_);
    steps(&mut world, &mut schedule, 10);
    assert_eq!(position(&world, crate_), at);
}

#[test]
fn a_kinematic_top_down_mover_pushed_into_a_wall_slides_along_it() {
    let (mut world, mut schedule) = rig();
    // A wall whose left face is at x 100.
    world.spawn_with(RigidBodyBundle::r#static(
        Position(Vec2::new(110.0, 0.0)),
        Collider::aabb(Vec2::new(10.0, 500.0)),
    ));
    let mut mover = TopDownMover::new(60.0);
    mover.set_direction(Vec2::new(0.0, 1.0));
    let brawler = world.spawn_with(
        RigidBodyBundle::kinematic(
            Position(Vec2::new(80.0, 0.0)),
            Collider::aabb(Vec2::splat(10.0)),
        )
        .with((mover, Knockback::new(0.3))),
    );
    steps(&mut world, &mut schedule, 2);
    assert!(knock_back(&mut world, brawler, Vec2::new(900.0, -300.0)));
    for _ in 0..30 {
        steps(&mut world, &mut schedule, 1);
        assert!(
            position(&world, brawler).x <= 90.0 + 1e-3,
            "{}",
            position(&world, brawler)
        );
    }
    // Against the wall, it slid up the push's part along it and on down its
    // own way.
    let at = position(&world, brawler);
    assert!((at.x - 90.0).abs() < 0.05, "{at}");
    assert!(at.y < 30.0, "the push lifted it: {at}");
}

#[test]
fn a_platformer_is_launched_with_its_input_locked() {
    let (mut world, mut schedule) = rig();
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, 1000.0),
        ..PhysicsConfig::default()
    });
    let acrobat = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(10.0, 20.0)))
            .with((
                PlatformerController::new(200.0, 600.0),
                Knockback::new(0.25),
            )),
    );
    steps(&mut world, &mut schedule, 1);
    assert!(knock_back(&mut world, acrobat, Vec2::new(-300.0, -200.0)));
    world
        .get_mut::<PlatformerController>(acrobat)
        .unwrap()
        .set_run(1.0);
    steps(&mut world, &mut schedule, 1);
    let velocity = world.get::<Velocity>(acrobat).unwrap().0;
    assert!(
        (velocity.x + 300.0).abs() < 1e-3,
        "the launch, not the run: {velocity}"
    );
    assert!(
        !world
            .get::<PlatformerController>(acrobat)
            .unwrap()
            .can_jump()
    );
    steps(&mut world, &mut schedule, 20);
    assert!(
        world.get::<Velocity>(acrobat).unwrap().0.x > 0.0,
        "the run once the lock ends"
    );
}

#[test]
fn a_dynamic_body_takes_the_push_once_and_a_child_none() {
    let (mut world, mut schedule) = rig();
    let ball = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::ZERO), Collider::circle(4.0))
            .with((Knockback::new(0.2),)),
    );
    assert!(knock_back(&mut world, ball, Vec2::new(120.0, 0.0)));
    assert_eq!(
        world.get::<Velocity>(ball).unwrap().0,
        Vec2::new(120.0, 0.0)
    );
    steps(&mut world, &mut schedule, 5);
    assert!((world.get::<Velocity>(ball).unwrap().0.x - 120.0).abs() < 1e-3);
    let parent = world.spawn_with((Transform::default(),));
    let pod = spawn_child(&mut world, parent, Vec2::ONE, (Knockback::new(0.2),));
    assert!(knock_back(&mut world, pod, Vec2::X));
    steps(&mut world, &mut schedule, 3);
    assert!(world.get::<Position>(pod).is_none());
}

#[test]
fn a_save_keeps_a_running_push_and_refuses_bad_values() {
    let mut world = World::new();
    let brute = world.spawn_with((Knockback::new(0.4).with_resist(0.25),));
    knock_back(&mut world, brute, Vec2::new(0.0, 80.0));
    let knockback = world.get::<Knockback>(brute).unwrap().clone();
    let saved = serde_json::to_value(&knockback).unwrap();
    assert_eq!(
        serde_json::from_value::<Knockback>(saved).unwrap(),
        knockback
    );
    for refused in [
        json!({ "seconds": 0 }),
        json!({ "resist": 1.5 }),
        json!({ "seconds": 0.2, "left": 0.3 }),
        json!({ "push": [1e39, 0] }),
    ] {
        assert!(
            serde_json::from_value::<Knockback>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
#[should_panic(expected = "resist must be in [0, 1]")]
fn a_resist_over_one_panics() {
    let _ = Knockback::new(0.2).with_resist(1.5);
}
