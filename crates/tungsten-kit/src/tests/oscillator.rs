use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PhysicsPlugin, RigidBodyBundle, Velocity};
use tungsten_core::{Bundle, PrefabData, PrefabRegistry, Stage, Time, Transform};

use crate::{LinearMotion, LinearMotionPlugin, TopDownMover, TopDownMoverPlugin, spawn_child};

const DT: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    LinearMotionPlugin.build(&mut schedule, &mut world);
    OscillatorPlugin.build(&mut schedule, &mut world);
    TopDownMoverPlugin.build(&mut schedule, &mut world);
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

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).length() < 1e-3
}

#[test]
fn the_position_swings_from_its_start_and_comes_back_each_period() {
    let (mut world, mut schedule) = rig();
    let start = Vec2::new(50.0, 80.0);
    let gem = world.spawn_with((
        Oscillator::position(Vec2::new(0.0, 4.0), 1.0).with_phase(0.25),
        Position(start),
        PrevPosition(start),
    ));
    // A quarter period in from the peak: back through the middle, 4 px up.
    steps(&mut world, &mut schedule, 15);
    assert!(
        close(position(&world, gem), start - Vec2::new(0.0, 4.0)),
        "{}",
        position(&world, gem)
    );
    steps(&mut world, &mut schedule, 15);
    assert!(close(position(&world, gem), start - Vec2::new(0.0, 8.0)));
    steps(&mut world, &mut schedule, 30);
    assert!(
        close(position(&world, gem), start),
        "{}",
        position(&world, gem)
    );
    for _ in 0..3 {
        steps(&mut world, &mut schedule, 60);
        assert!(
            close(position(&world, gem), start),
            "{}",
            position(&world, gem)
        );
    }
}

#[test]
fn alone_on_a_kinematic_body_it_drives_the_velocity_with_no_drift() {
    let (mut world, mut schedule) = rig();
    let start = Vec2::new(0.0, 200.0);
    let lift = world.spawn_with(
        RigidBodyBundle::kinematic(Position(start), Collider::aabb(Vec2::new(40.0, 8.0)))
            .with((Oscillator::position(Vec2::new(0.0, 24.0), 2.0),)),
    );
    for _ in 0..5 {
        steps(&mut world, &mut schedule, 120);
        assert!(
            close(position(&world, lift), start),
            "{}",
            position(&world, lift)
        );
    }
    // Its velocity is the last step's change over the step.
    let velocity = world.get::<Velocity>(lift).unwrap().0;
    let expected = 24.0 * ((std::f32::consts::TAU * DT / 2.0).sin()) / DT;
    assert!(
        (velocity.y - expected).abs() < 1e-2,
        "{velocity} vs {expected}"
    );
}

#[test]
fn beside_a_linear_motion_it_adds_to_its_velocity() {
    let (mut world, mut schedule) = rig();
    let belt = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(40.0, 8.0)))
            .with((
                LinearMotion::new(Vec2::new(30.0, 0.0)),
                Oscillator::position(Vec2::new(0.0, 10.0), 1.0),
            )),
    );
    steps(&mut world, &mut schedule, 60);
    assert!(
        close(position(&world, belt), Vec2::new(30.0, 0.0)),
        "{}",
        position(&world, belt)
    );
    steps(&mut world, &mut schedule, 15);
    assert!(
        close(position(&world, belt), Vec2::new(37.5, 10.0)),
        "{}",
        position(&world, belt)
    );
}

#[test]
fn a_bodiless_mover_beside_it_moves_both_ways() {
    let (mut world, mut schedule) = rig();
    let mut mover = TopDownMover::new(60.0);
    mover.set_direction(Vec2::X);
    let pet = world.spawn_with((
        mover,
        Oscillator::position(Vec2::new(0.0, 5.0), 1.0),
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
    ));
    steps(&mut world, &mut schedule, 15);
    assert!(
        close(position(&world, pet), Vec2::new(15.0, 5.0)),
        "{}",
        position(&world, pet)
    );
}

#[test]
fn a_kinematic_body_a_later_driver_sets_and_a_dynamic_body_are_left() {
    let (mut world, mut schedule) = rig();
    let mut mover = TopDownMover::new(60.0);
    mover.set_direction(Vec2::X);
    let brawler = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(8.0)))
            .with((mover, Oscillator::position(Vec2::new(0.0, 5.0), 1.0))),
    );
    let ball = world.spawn_with(
        RigidBodyBundle::dynamic(Position(Vec2::new(200.0, 0.0)), Collider::circle(4.0))
            .with((Oscillator::position(Vec2::new(0.0, 5.0), 1.0),)),
    );
    steps(&mut world, &mut schedule, 16);
    // The mover's first step stands still; fifteen steps at 1 px.
    assert!(
        close(position(&world, brawler), Vec2::new(15.0, 0.0)),
        "{}",
        position(&world, brawler)
    );
    assert_eq!(position(&world, ball), Vec2::new(200.0, 0.0));
    assert_eq!(world.get::<Velocity>(ball).unwrap().0, Vec2::ZERO);
    // Its clock runs all the same.
    assert!((world.get::<Oscillator>(ball).unwrap().clock - 16.0 * f64::from(DT)).abs() < 1e-6);
}

#[test]
fn a_childs_clock_runs_but_its_position_stays_the_parents() {
    let (mut world, mut schedule) = rig();
    let parent = world.spawn_with((Transform::from_position(Vec2::new(10.0, 10.0)),));
    let sign = spawn_child(
        &mut world,
        parent,
        Vec2::new(0.0, -8.0),
        (Oscillator::position(Vec2::new(0.0, 3.0), 1.0).with_rotation(0.2, 1.0),),
    );
    steps(&mut world, &mut schedule, 10);
    assert!(world.get::<Position>(sign).is_none());
    assert!((world.get::<Oscillator>(sign).unwrap().clock - 10.0 * f64::from(DT)).abs() < 1e-6);
}

#[test]
fn the_displacement_is_the_folds_offset_from_the_start() {
    let mut oscillator = Oscillator::position(Vec2::new(6.0, 0.0), 2.0).with_phase(0.5);
    assert_eq!(
        oscillator.displacement(),
        Vec2::ZERO,
        "it starts where it is"
    );
    oscillator.clock = 0.5;
    // sin(2π(0.25 + 0.5)) − sin(π) = −1.
    assert!(close(oscillator.displacement(), Vec2::new(-6.0, 0.0)));
    assert!(!Oscillator::rotation(0.3, 1.0).moves());
    assert_eq!(Oscillator::scale(0.1, 1.0).displacement(), Vec2::ZERO);
}

#[test]
fn only_a_position_channel_spawns_a_point() {
    let (mut world, _) = rig();
    let mut prefabs = PrefabRegistry::new();
    for (id, value) in [
        (
            "gem",
            json!({ "components": { "oscillator": { "position": { "amplitude": [0, 3], "period": 1.2 } } } }),
        ),
        (
            "sign",
            json!({ "components": { "oscillator": { "rotation": { "amplitude": 0.2, "period": 1 } } } }),
        ),
    ] {
        let prefab: PrefabData = serde_json::from_value(value).unwrap();
        prefabs.insert(id.to_string(), prefab);
    }
    world.insert_resource(prefabs);
    let gem = world
        .prepare_prefab("gem", [4.0, 5.0])
        .unwrap()
        .spawn(&mut world);
    let sign = world
        .prepare_prefab("sign", [4.0, 5.0])
        .unwrap()
        .spawn(&mut world);
    assert_eq!(position(&world, gem), Vec2::new(4.0, 5.0));
    assert_eq!(
        world.get::<PrevPosition>(gem).unwrap().0,
        Vec2::new(4.0, 5.0)
    );
    assert!(world.get::<Position>(sign).is_none() && world.get::<PrevPosition>(sign).is_none());
}

#[test]
fn a_save_round_trips_its_clock_and_refuses_what_its_constructors_refuse() {
    let mut oscillator = Oscillator::position(Vec2::new(0.0, 3.0), 1.2)
        .with_rotation(0.3, 0.8)
        .with_scale(0.1, 0.5)
        .with_phase(0.25);
    oscillator.clock = 1.5;
    oscillator.prev_clock = 1.5 - f64::from(DT);
    let saved = serde_json::to_value(&oscillator).unwrap();
    assert_eq!(
        serde_json::from_value::<Oscillator>(saved).unwrap(),
        oscillator
    );
    for refused in [
        json!({}),
        json!({ "scale": { "amplitude": 1.0, "period": 1 } }),
        json!({ "rotation": { "amplitude": 0.1, "period": 0 } }),
        json!({ "rotation": { "amplitude": 0.1, "period": 1 }, "phase": 1.0 }),
        json!({ "rotation": { "amplitude": 0.1, "period": 1 }, "clock": -1.0 }),
        json!({ "rotation": { "amplitude": 0.1, "period": 1, "speed": 2 } }),
    ] {
        assert!(
            serde_json::from_value::<Oscillator>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
#[should_panic(expected = "period must be finite and > 0")]
fn a_period_of_zero_panics() {
    let _ = Oscillator::position(Vec2::X, 0.0);
}

#[test]
#[should_panic(expected = "phase must be in [0, 1)")]
fn a_phase_of_one_panics() {
    let _ = Oscillator::rotation(0.1, 1.0).with_phase(1.0);
}

fn present_rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    OscillatorPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn present(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

#[test]
fn rotation_and_scale_swing_about_the_first_frames_values_at_the_drawn_clock() {
    let (mut world, mut schedule) = present_rig();
    let mut base = Transform::from_position(Vec2::ZERO);
    base.rotation = 0.5;
    base.scale = Vec2::new(2.0, 1.0);
    let sign = world.spawn_with((Oscillator::rotation(0.3, 0.8).with_scale(0.1, 0.5), base));
    present(&mut world, &mut schedule);
    let transform = *world.get::<Transform>(sign).unwrap();
    assert_eq!(
        (transform.rotation, transform.scale),
        (0.5, Vec2::new(2.0, 1.0))
    );
    // A frame that ran one step and left half a step over.
    world.resource_mut::<Time>().advance_frame(0.025);
    let alpha = world.resource::<Time>().alpha();
    assert!(alpha > 0.0 && alpha < 1.0, "{alpha}");
    {
        let oscillator = world.get_mut::<Oscillator>(sign).unwrap();
        oscillator.prev_clock = 1.0;
        oscillator.clock = 1.0 + f64::from(DT);
    }
    present(&mut world, &mut schedule);
    let t = 1.0 + DT * alpha;
    let rotation = 0.5 + 0.3 * (std::f32::consts::TAU * t / 0.8).sin();
    let scale = Vec2::new(2.0, 1.0) * (1.0 + 0.1 * (std::f32::consts::TAU * t / 0.5).sin());
    let transform = *world.get::<Transform>(sign).unwrap();
    assert!(
        (transform.rotation - rotation).abs() < 1e-4,
        "{} vs {rotation}",
        transform.rotation
    );
    assert!((transform.scale - scale).length() < 1e-4);
    // Paused, the clock and alpha hold, and so do the channels.
    world.resource_mut::<Time>().pause();
    world.resource_mut::<Time>().advance_frame(0.025);
    present(&mut world, &mut schedule);
    assert_eq!(*world.get::<Transform>(sign).unwrap(), transform);
    // Without interpolation it draws the last step's clock.
    world.resource_mut::<Time>().set_interpolate(false);
    present(&mut world, &mut schedule);
    let at_step = 0.5 + 0.3 * (std::f32::consts::TAU * (1.0 + DT) / 0.8).sin();
    assert!((world.get::<Transform>(sign).unwrap().rotation - at_step).abs() < 1e-4);
}

#[test]
fn a_child_and_a_position_only_oscillator_are_turned_and_left_as_they_should_be() {
    let (mut world, mut schedule) = present_rig();
    let parent = world.spawn_with((Transform::default(),));
    let flag = spawn_child(
        &mut world,
        parent,
        Vec2::new(0.0, -8.0),
        (Oscillator::rotation(0.2, 1.0).with_phase(0.25),),
    );
    let gem = world.spawn_with((
        Oscillator::position(Vec2::Y, 1.0),
        Transform::from_position(Vec2::ONE),
    ));
    present(&mut world, &mut schedule);
    assert!((world.get::<Transform>(flag).unwrap().rotation - 0.2).abs() < 1e-6);
    assert_eq!(world.get::<Transform>(gem).unwrap().rotation, 0.0);
    assert!(world.get::<Oscillator>(gem).unwrap().base.is_none());
}
