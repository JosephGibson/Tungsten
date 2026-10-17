use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle, Velocity};
use tungsten_core::{Bundle, PrefabData, PrefabRegistry, Stage, Time, Transform};

use crate::spawn_child;

const DT: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    LinearMotionPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn steps(world: &mut World, schedule: &mut Schedule, count: usize, dt: f32) {
    for _ in 0..count {
        world.resource_mut::<Time>().advance_frame(dt);
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    }
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

fn bodiless(world: &mut World, motion: LinearMotion) -> Entity {
    world.spawn_with((motion, Position(Vec2::ZERO), PrevPosition(Vec2::ZERO)))
}

#[test]
fn a_bodiless_path_is_exact_and_the_same_at_either_step() {
    let motion =
        || LinearMotion::new(Vec2::new(40.0, -10.0)).with_acceleration(Vec2::new(0.0, 80.0));
    let mut ends = Vec::new();
    for (count, dt) in [(60, DT), (30, 2.0 * DT)] {
        let (mut world, mut schedule) = rig();
        let rock = bodiless(&mut world, motion());
        steps(&mut world, &mut schedule, count, dt);
        ends.push((
            position(&world, rock),
            world.get::<LinearMotion>(rock).unwrap().velocity(),
        ));
    }
    // One second: v·t + a·t²/2, and v + a·t.
    for (at, velocity) in &ends {
        assert!((*at - Vec2::new(40.0, 30.0)).length() < 1e-3, "{at}");
        assert!(
            (*velocity - Vec2::new(40.0, 70.0)).length() < 1e-3,
            "{velocity}"
        );
    }
    assert!((ends[0].0 - ends[1].0).length() < 1e-3);
}

#[test]
fn a_bodiless_velocity_reads_the_steps_move() {
    let (mut world, mut schedule) = rig();
    let rock = world.spawn_with((
        LinearMotion::new(Vec2::new(30.0, 0.0)).with_acceleration(Vec2::new(60.0, 0.0)),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
        Velocity(Vec2::new(-500.0, 0.0)),
    ));
    steps(&mut world, &mut schedule, 1, DT);
    let mean = Vec2::new(30.0 + 60.0 * DT * 0.5, 0.0);
    assert!((world.get::<Velocity>(rock).unwrap().0 - mean).length() < 1e-4);
    assert!((position(&world, rock) - mean * DT).length() < 1e-5);
}

#[test]
fn on_a_kinematic_body_it_drives_the_velocity_the_step_moves_by() {
    let (mut world, mut schedule) = rig();
    let belt = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(60.0, 8.0)))
            .with((LinearMotion::new(Vec2::new(40.0, 0.0)),)),
    );
    steps(&mut world, &mut schedule, 30, DT);
    assert_eq!(world.get::<Velocity>(belt).unwrap().0, Vec2::new(40.0, 0.0));
    assert!((position(&world, belt) - Vec2::new(20.0, 0.0)).length() < 1e-3);
}

#[test]
fn a_dynamic_body_and_a_child_are_skipped() {
    let (mut world, mut schedule) = rig();
    let ball = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::ZERO), Collider::circle(4.0))
            .with((LinearMotion::new(Vec2::new(90.0, 0.0)),)),
    );
    let parent = world.spawn_with((Transform::from_position(Vec2::new(5.0, 5.0)),));
    let pod = spawn_child(
        &mut world,
        parent,
        Vec2::new(4.0, 0.0),
        (LinearMotion::new(Vec2::new(0.0, 50.0)).with_acceleration(Vec2::Y),),
    );
    steps(&mut world, &mut schedule, 10, DT);
    assert_eq!(
        world.get::<Velocity>(ball).unwrap().0,
        Vec2::ZERO,
        "the solver's"
    );
    assert_eq!(position(&world, ball), Vec2::ZERO);
    let motion = world.get::<LinearMotion>(pod).unwrap();
    assert_eq!(
        motion.velocity(),
        Vec2::new(0.0, 50.0),
        "a skipped motion holds"
    );
    assert!(world.get::<Position>(pod).is_none());
}

#[test]
fn the_scene_component_spawns_a_point_and_no_velocity() {
    let (mut world, _) = rig();
    let rock: PrefabData = serde_json::from_value(json!({
        "components": { "linear_motion": { "velocity": [0, 120] } }
    }))
    .unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("rock".to_string(), rock);
    world.insert_resource(prefabs);
    let rock = world
        .prepare_prefab("rock", [10.0, 20.0])
        .unwrap()
        .spawn(&mut world);
    assert_eq!(position(&world, rock), Vec2::new(10.0, 20.0));
    assert_eq!(
        world.get::<PrevPosition>(rock).unwrap().0,
        Vec2::new(10.0, 20.0)
    );
    assert!(
        world.get::<Velocity>(rock).is_none(),
        "a Velocity stays a body's"
    );
    assert_eq!(
        world.get::<LinearMotion>(rock).unwrap().acceleration(),
        Vec2::ZERO
    );
}

#[test]
fn a_save_round_trips_and_refuses_what_is_not_finite() {
    let mut motion = LinearMotion::new(Vec2::new(1.0, 2.0)).with_acceleration(Vec2::new(0.0, 9.0));
    motion.set_velocity(Vec2::new(-3.0, 4.0));
    let saved = serde_json::to_value(&motion).unwrap();
    assert_eq!(
        saved,
        json!({ "velocity": [-3.0, 4.0], "acceleration": [0.0, 9.0] })
    );
    assert_eq!(
        serde_json::from_value::<LinearMotion>(saved).unwrap(),
        motion
    );
    assert!(serde_json::from_value::<LinearMotion>(json!({ "velocity": [1e39, 0] })).is_err());
    assert!(
        serde_json::from_value::<LinearMotion>(json!({ "velocity": [0, 0], "spin": 1 })).is_err()
    );
}

#[test]
#[should_panic(expected = "velocity must be finite")]
fn a_velocity_that_is_not_finite_panics() {
    LinearMotion::new(Vec2::ZERO).set_velocity(Vec2::new(f32::NAN, 0.0));
}

#[test]
#[should_panic(expected = "acceleration must be finite")]
fn an_acceleration_that_is_not_finite_panics() {
    let _ = LinearMotion::new(Vec2::ZERO).with_acceleration(Vec2::splat(f32::INFINITY));
}
