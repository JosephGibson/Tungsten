use super::*;
use crate::{KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_REACT, KIT_RESOLVE, KIT_TICK};
use tungsten_core::Stage;
use tungsten_core::physics::{
    Collider, PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PhysicsPlugin, Position, RigidBodyBundle,
};
use tungsten_core::{Bundle, Time};

const DT: f32 = 1.0 / 60.0;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
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

fn mover_at(world: &mut World, at: Vec2, direction: Vec2) -> Entity {
    let mut mover = TopDownMover::new(120.0);
    mover.set_direction(direction);
    world.spawn_with(
        RigidBodyBundle::kinematic(Position(at), Collider::aabb(Vec2::splat(10.0))).with((mover,)),
    )
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
            TOP_DOWN_MOVER,
            PHYSICS_STEP,
            KIT_OVERLAP,
            KIT_RESOLVE,
            KIT_REACT
        ]
    );
}

#[test]
fn spawned_against_a_wall_it_never_enters_it_and_slides_along() {
    let (mut world, mut schedule) = rig();
    world.spawn_with(RigidBodyBundle::r#static(
        Position::new(110.0, 0.0),
        Collider::aabb(Vec2::new(10.0, 500.0)),
    ));
    let entity = mover_at(&mut world, Vec2::new(90.0, 0.0), Vec2::new(1.0, -1.0));
    steps(&mut world, &mut schedule, 1);
    assert_eq!(position(&world, entity), Vec2::new(90.0, 0.0));
    for _ in 0..30 {
        steps(&mut world, &mut schedule, 1);
        assert!(position(&world, entity).x <= 90.0);
    }
    let up = 120.0 * std::f32::consts::FRAC_1_SQRT_2 * 30.0 * DT;
    assert!((position(&world, entity).y + up).abs() < 0.01);
}

#[test]
fn it_stops_short_of_a_box_in_its_way() {
    let (mut world, mut schedule) = rig();
    world.spawn_with(RigidBodyBundle::r#static(
        Position::new(0.0, 100.0),
        Collider::aabb(Vec2::splat(20.0)),
    ));
    let entity = mover_at(&mut world, Vec2::ZERO, Vec2::Y);
    steps(&mut world, &mut schedule, 60);
    let at = position(&world, entity);
    assert_eq!(at.x, 0.0);
    assert!(at.y <= 70.0 && at.y > 69.9, "{at}");
    assert!(world.get::<Velocity>(entity).unwrap().0.length() < 0.01);
}

#[test]
fn it_moves_freely_at_its_speed() {
    let (mut world, mut schedule) = rig();
    let entity = mover_at(&mut world, Vec2::ZERO, Vec2::new(-0.5, 0.0));
    steps(&mut world, &mut schedule, 11);
    let at = position(&world, entity);
    assert!((at.x + 60.0 * 10.0 * DT).abs() < 0.01, "{at}");
    assert_eq!(world.get::<Velocity>(entity).unwrap().0.y, 0.0);
}

#[test]
fn the_direction_is_normalized_past_one_and_stops_when_not_finite() {
    let mut mover = TopDownMover::new(50.0);
    mover.set_direction(Vec2::new(3.0, 4.0));
    assert!((mover.direction() - Vec2::new(0.6, 0.8)).length() < 1e-6);
    mover.set_direction(Vec2::new(0.3, 0.0));
    assert_eq!(mover.direction(), Vec2::new(0.3, 0.0));
    mover.set_direction(Vec2::new(f32::NAN, 1.0));
    assert_eq!(mover.direction(), Vec2::ZERO);
}

#[test]
fn a_body_that_is_not_kinematic_is_skipped() {
    let (mut world, mut schedule) = rig();
    let mut mover = TopDownMover::new(120.0);
    mover.set_direction(Vec2::X);
    let entity = world.spawn_with(
        RigidBodyBundle::r#static(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(10.0)))
            .with((mover,)),
    );
    steps(&mut world, &mut schedule, 3);
    assert!(!world.get::<TopDownMover>(entity).unwrap().started);
    assert_eq!(position(&world, entity), Vec2::ZERO);
}

#[test]
#[should_panic(expected = "speed must be finite and > 0")]
fn a_negative_speed_panics() {
    let _ = TopDownMover::new(-1.0);
}
