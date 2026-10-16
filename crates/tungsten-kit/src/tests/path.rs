use super::*;
use crate::{KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_REACT, KIT_RESOLVE, KIT_TICK};
use tungsten_core::Stage;
use tungsten_core::physics::{
    Collider, PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PhysicsPlugin, RigidBodyBundle,
};
use tungsten_core::{Bundle, Entity, Time};

const DT: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    PathFollowerPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn platform(world: &mut World, follower: PathFollower) -> Entity {
    let start = follower.points[0];
    world.spawn_with(
        RigidBodyBundle::kinematic(Position(start), Collider::aabb(Vec2::new(30.0, 5.0)))
            .with((follower,)),
    )
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(DT);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

fn square() -> [Vec2; 4] {
    [
        Vec2::new(0.0, 0.0),
        Vec2::new(60.0, 0.0),
        Vec2::new(60.0, 60.0),
        Vec2::new(0.0, 60.0),
    ]
}

#[test]
fn the_plugin_runs_between_the_snapshot_and_the_step() {
    let (_, schedule) = rig();
    assert_eq!(
        schedule.names(Stage::FixedUpdate).collect::<Vec<_>>(),
        [
            PHYSICS_PREV_SNAPSHOT,
            KIT_TICK,
            KIT_INTENT,
            KIT_MOVE,
            PATH_FOLLOW,
            PHYSICS_STEP,
            KIT_OVERLAP,
            KIT_RESOLVE,
            KIT_REACT
        ]
    );
}

#[test]
fn a_stopping_path_ends_each_step_on_it_and_stops_at_its_end() {
    let (mut world, mut schedule) = rig();
    let lift = platform(&mut world, PathFollower::new(square(), 120.0));
    // 180 px at 120 px/s: 90 steps.
    for n in 1..=90 {
        step(&mut world, &mut schedule);
        let travelled = 120.0 * DT * n as f32;
        let expected = if travelled <= 60.0 {
            Vec2::new(travelled, 0.0)
        } else if travelled <= 120.0 {
            Vec2::new(60.0, travelled - 60.0)
        } else {
            Vec2::new(180.0 - travelled, 60.0)
        };
        let at = position(&world, lift);
        assert!(
            (at - expected).length() < 0.01,
            "step {n}: {at} vs {expected}"
        );
    }
    // The step that reaches the end may fall a rounding short of it.
    step(&mut world, &mut schedule);
    step(&mut world, &mut schedule);
    assert!(world.get::<PathFollower>(lift).unwrap().is_finished());
    assert_eq!(world.get::<Velocity>(lift).unwrap().0, Vec2::ZERO);
    let end = position(&world, lift);
    step(&mut world, &mut schedule);
    assert_eq!(position(&world, lift), end);
    assert!((end - Vec2::new(0.0, 60.0)).length() < 0.01);
}

#[test]
fn leftover_time_carries_past_a_point() {
    let mut follower = PathFollower::new(square(), 100.0);
    // 0.7 s at 100 px/s: past the first point by 10 px.
    follower.advance(0.7);
    assert!((follower.point() - Vec2::new(60.0, 10.0)).length() < 1e-4);
}

#[test]
fn a_loop_closes_back_to_the_first_point() {
    let mut follower = PathFollower::new(square(), 100.0).looping();
    // The loop is 240 px: 2.4 s a lap.
    follower.advance(2.3);
    assert!((follower.point() - Vec2::new(0.0, 10.0)).length() < 1e-3);
    follower.advance(0.2);
    assert!((follower.point() - Vec2::new(10.0, 0.0)).length() < 1e-3);
    assert!(!follower.is_finished());
}

#[test]
fn a_ping_pong_returns_along_the_points() {
    let mut follower = PathFollower::new(square(), 100.0).ping_pong();
    // Forward 180 px, then back along the same legs: 3.6 s a round.
    follower.advance(1.9);
    assert!((follower.point() - Vec2::new(10.0, 60.0)).length() < 1e-3);
    follower.advance(0.75);
    assert!((follower.point() - Vec2::new(60.0, 35.0)).length() < 1e-3);
    follower.advance(0.9);
    assert!((follower.point() - Vec2::new(5.0, 0.0)).length() < 1e-3);
    follower.advance(0.2);
    assert!((follower.point() - Vec2::new(15.0, 0.0)).length() < 1e-3);
    assert!(!follower.is_finished());
}

#[test]
fn the_last_mode_call_wins() {
    let follower = PathFollower::new(square(), 1.0).looping().ping_pong();
    assert_eq!(follower.mode, Mode::PingPong);
    let follower = PathFollower::new(square(), 1.0).ping_pong().looping();
    assert_eq!(follower.mode, Mode::Loop);
}

#[test]
fn easing_shapes_each_segment_and_going_back_eases_from_its_start() {
    let points = [Vec2::ZERO, Vec2::new(100.0, 0.0)];
    let mut follower = PathFollower::new(points, 100.0)
        .ping_pong()
        .with_easing(Easing::QuadIn);
    follower.advance(0.5);
    assert!((follower.point().x - 25.0).abs() < 1e-3);
    // Going back, a quarter of the way from 100 after half the leg.
    follower.advance(1.0);
    assert!((follower.point().x - 75.0).abs() < 1e-3);
    let mut sine = PathFollower::new(points, 100.0).with_easing(Easing::SineInOut);
    sine.advance(0.25);
    let expected = 50.0 * (1.0 - (std::f32::consts::PI * 0.25).cos());
    assert!((sine.point().x - expected).abs() < 1e-3);
}

#[test]
fn a_looping_platform_ends_every_step_on_the_path() {
    let (mut world, mut schedule) = rig();
    let lift = platform(
        &mut world,
        PathFollower::new(square(), 150.0)
            .looping()
            .with_easing(Easing::SineInOut),
    );
    let mut follower = world.get::<PathFollower>(lift).unwrap().clone();
    for n in 0..200 {
        step(&mut world, &mut schedule);
        follower.advance(DT);
        let at = position(&world, lift);
        assert!((at - follower.point()).length() < 0.01, "step {n}: {at}");
    }
}

#[test]
fn a_paused_clock_holds_it() {
    let (mut world, mut schedule) = rig();
    let lift = platform(&mut world, PathFollower::new(square(), 120.0));
    step(&mut world, &mut schedule);
    let at = position(&world, lift);
    world.resource_mut::<Time>().pause();
    for _ in 0..5 {
        step(&mut world, &mut schedule);
    }
    assert_eq!(position(&world, lift), at);
    assert!((world.get::<PathFollower>(lift).unwrap().time - DT).abs() < 1e-6);
}

#[test]
#[should_panic(expected = "needs at least 2 points")]
fn one_point_panics() {
    let _ = PathFollower::new([Vec2::ZERO], 1.0);
}

#[test]
#[should_panic(expected = "neighbouring points must differ")]
fn a_zero_length_segment_panics() {
    let _ = PathFollower::new([Vec2::ZERO, Vec2::X, Vec2::X], 1.0);
}

#[test]
#[should_panic(expected = "every point must be finite")]
fn a_non_finite_point_panics() {
    let _ = PathFollower::new([Vec2::ZERO, Vec2::new(f32::NAN, 0.0)], 1.0);
}

#[test]
#[should_panic(expected = "the last point must differ from the first")]
fn a_loop_with_a_zero_closing_segment_panics() {
    let _ = PathFollower::new([Vec2::ZERO, Vec2::X, Vec2::ZERO], 1.0).looping();
}

#[test]
#[should_panic(expected = "speed must be finite and > 0")]
fn a_zero_speed_panics() {
    let _ = PathFollower::new([Vec2::ZERO, Vec2::X], 0.0);
}
