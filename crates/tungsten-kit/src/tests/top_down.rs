use super::*;
use crate::{KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_REACT, KIT_RESOLVE, KIT_TICK};
use serde_json::json;
use tungsten_core::Stage;
use tungsten_core::physics::{
    Collider, PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PhysicsPlugin, Position, RigidBodyBundle,
};
use tungsten_core::{Bundle, ComponentRegistry, PrefabData, PrefabRegistry, Time};

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
fn a_speed_set_to_zero_stands_it_still_and_a_bad_one_is_ignored() {
    let (mut world, mut schedule) = rig();
    let mover = mover_at(&mut world, Vec2::ZERO, Vec2::X);
    world.get_mut::<TopDownMover>(mover).unwrap().set_speed(0.0);
    steps(&mut world, &mut schedule, 10);
    assert_eq!(position(&world, mover), Vec2::ZERO);
    let state = world.get_mut::<TopDownMover>(mover).unwrap();
    state.set_speed(f32::NAN);
    state.set_speed(-5.0);
    assert_eq!(state.speed, 0.0);
    state.set_speed(60.0);
    steps(&mut world, &mut schedule, 2);
    assert!(position(&world, mover).x > 0.0);
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

/// A bodiless mover at 120 px/s with `rates` (acceleration, deceleration)
/// heading along `direction`.
fn bodiless(world: &mut World, rates: (Option<f32>, Option<f32>), direction: Vec2) -> Entity {
    let mut mover = TopDownMover::new(120.0);
    if let Some(rate) = rates.0 {
        mover = mover.with_acceleration(rate);
    }
    if let Some(rate) = rates.1 {
        mover = mover.with_deceleration(rate);
    }
    mover.set_direction(direction);
    world.spawn_with((
        Position(Vec2::ZERO),
        PrevPosition(Vec2::ZERO),
        Velocity(Vec2::ZERO),
        mover,
    ))
}

fn velocity(world: &World, entity: Entity) -> Vec2 {
    world.get::<Velocity>(entity).unwrap().0
}

fn steer(world: &mut World, entity: Entity, direction: Vec2) {
    world
        .get_mut::<TopDownMover>(entity)
        .unwrap()
        .set_direction(direction);
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a - b).length() < 1e-3
}

#[test]
fn acceleration_ramps_to_speed() {
    let (mut world, mut schedule) = rig();
    // 1,800 px/s² gains 30 px/s a step at 1/60 s.
    let mut mover = TopDownMover::new(120.0).with_acceleration(1800.0);
    mover.set_direction(Vec2::X);
    let entity = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::splat(10.0)))
            .with((mover,)),
    );
    steps(&mut world, &mut schedule, 1);
    assert_eq!(
        velocity(&world, entity),
        Vec2::ZERO,
        "the first step is still"
    );
    for expected in [30.0, 60.0, 90.0, 120.0, 120.0, 120.0] {
        steps(&mut world, &mut schedule, 1);
        assert!(
            close(velocity(&world, entity), Vec2::new(expected, 0.0)),
            "{} for {expected}",
            velocity(&world, entity)
        );
    }
    let rows = world.get::<TopDownMover>(entity).unwrap().inspect_rows();
    let names: Vec<_> = rows.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        [
            "speed",
            "direction",
            "acceleration",
            "deceleration",
            "velocity"
        ]
    );
    assert_eq!(rows[2].1, "1800.0");
    assert_eq!(rows[3].1, "none");
}

#[test]
fn deceleration_ramps_to_rest() {
    let (mut world, mut schedule) = rig();
    // 2,400 px/s² sheds 40 px/s a step.
    let entity = bodiless(&mut world, (None, Some(2400.0)), Vec2::X);
    steps(&mut world, &mut schedule, 2);
    assert!(close(velocity(&world, entity), Vec2::new(120.0, 0.0)));
    steer(&mut world, entity, Vec2::ZERO);
    let mut at = position(&world, entity).x;
    for expected in [80.0, 40.0, 0.0, 0.0] {
        steps(&mut world, &mut schedule, 1);
        assert!(
            close(velocity(&world, entity), Vec2::new(expected, 0.0)),
            "{} for {expected}",
            velocity(&world, entity)
        );
        let now = position(&world, entity).x;
        assert!(
            (now - at - expected * DT).abs() < 1e-3,
            "it slides by its velocity"
        );
        at = now;
    }
}

#[test]
fn an_unset_rate_is_instant() {
    let (mut world, mut schedule) = rig();
    // Only a deceleration: it starts at full speed at once.
    let decelerating = bodiless(&mut world, (None, Some(600.0)), Vec2::Y);
    // Only an acceleration: it stops at once.
    let accelerating = bodiless(&mut world, (Some(600.0), None), Vec2::Y);
    // Neither: as before the rates, with the rows it had.
    let plain = bodiless(&mut world, (None, None), Vec2::Y);
    steps(&mut world, &mut schedule, 1);
    assert!(close(velocity(&world, decelerating), Vec2::new(0.0, 120.0)));
    assert!(close(velocity(&world, accelerating), Vec2::new(0.0, 10.0)));
    assert!(close(velocity(&world, plain), Vec2::new(0.0, 120.0)));
    steps(&mut world, &mut schedule, 20);
    for entity in [decelerating, accelerating, plain] {
        steer(&mut world, entity, Vec2::ZERO);
    }
    steps(&mut world, &mut schedule, 1);
    assert!(close(velocity(&world, decelerating), Vec2::new(0.0, 110.0)));
    assert_eq!(velocity(&world, accelerating), Vec2::ZERO);
    assert_eq!(velocity(&world, plain), Vec2::ZERO);
    let rows = world.get::<TopDownMover>(plain).unwrap().inspect_rows();
    assert_eq!(rows.len(), 2, "no rate, no new rows: {rows:?}");
}

#[test]
fn a_turn_accelerates_through_the_new_direction() {
    let (mut world, mut schedule) = rig();
    let entity = bodiless(&mut world, (Some(1800.0), None), Vec2::X);
    steps(&mut world, &mut schedule, 4);
    assert!(close(velocity(&world, entity), Vec2::new(120.0, 0.0)));
    // A reversal passes through rest at 30 px/s a step.
    steer(&mut world, entity, -Vec2::X);
    for expected in [90.0, 60.0, 30.0, 0.0, -30.0, -60.0, -90.0, -120.0, -120.0] {
        steps(&mut world, &mut schedule, 1);
        assert!(
            close(velocity(&world, entity), Vec2::new(expected, 0.0)),
            "{} for {expected}",
            velocity(&world, entity)
        );
    }
    // A right angle cuts the corner: slower mid-turn, then full speed.
    steer(&mut world, entity, Vec2::Y);
    let mut slowest = f32::MAX;
    for _ in 0..6 {
        let before = velocity(&world, entity);
        steps(&mut world, &mut schedule, 1);
        let now = velocity(&world, entity);
        assert!(
            now.x >= before.x - 1e-3 && now.y >= before.y - 1e-3,
            "{before} → {now}"
        );
        assert!((now - before).length() <= 30.0 + 1e-3);
        slowest = slowest.min(now.length());
    }
    assert!(close(velocity(&world, entity), Vec2::new(0.0, 120.0)));
    assert!(slowest < 100.0, "{slowest}");
}

#[test]
fn a_lower_speed_stat_is_approached_at_the_acceleration() {
    let (mut world, mut schedule) = rig();
    let entity = bodiless(&mut world, (Some(1800.0), Some(2400.0)), Vec2::X);
    steps(&mut world, &mut schedule, 4);
    assert!(close(velocity(&world, entity), Vec2::new(120.0, 0.0)));
    // What a `move_speed` stat writes: half the speed.
    world
        .get_mut::<TopDownMover>(entity)
        .unwrap()
        .set_speed(60.0);
    for expected in [90.0, 60.0, 60.0] {
        steps(&mut world, &mut schedule, 1);
        assert!(
            close(velocity(&world, entity), Vec2::new(expected, 0.0)),
            "{} for {expected}",
            velocity(&world, entity)
        );
    }
}

#[test]
fn a_wall_absorbs_the_kept_velocity() {
    let (mut world, mut schedule) = rig();
    // The wall's left face is at x 100; the mover's right face starts at 70.
    world.spawn_with(RigidBodyBundle::r#static(
        Position::new(110.0, 0.0),
        Collider::aabb(Vec2::new(10.0, 500.0)),
    ));
    let mut mover = TopDownMover::new(120.0).with_acceleration(1800.0);
    mover.set_direction(Vec2::X);
    let entity = world.spawn_with(
        RigidBodyBundle::kinematic(Position::new(60.0, 0.0), Collider::aabb(Vec2::splat(10.0)))
            .with((mover,)),
    );
    steps(&mut world, &mut schedule, 60);
    assert!(position(&world, entity).x <= 90.0 && position(&world, entity).x > 89.9);
    assert!(velocity(&world, entity).length() < 0.01);
    assert!(
        world.get::<TopDownMover>(entity).unwrap().velocity.length() < 0.01,
        "the slid velocity is kept, not the 120 px/s it pushed with"
    );
    // So it leaves the wall from rest.
    steer(&mut world, entity, -Vec2::X);
    steps(&mut world, &mut schedule, 1);
    assert!(close(velocity(&world, entity), Vec2::new(-30.0, 0.0)));
}

#[test]
fn rates_refuse_zero_negative_and_non_finite() {
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(
            std::panic::catch_unwind(|| TopDownMover::new(1.0).with_acceleration(bad)).is_err(),
            "acceleration {bad}"
        );
        assert!(
            std::panic::catch_unwind(|| TopDownMover::new(1.0).with_deceleration(bad)).is_err(),
            "deceleration {bad}"
        );
    }
    let (mut world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    for refused in [
        json!({ "speed": 240, "acceleration": 0 }),
        json!({ "speed": 240, "deceleration": -5 }),
        json!({ "speed": 240, "acceleration": "fast" }),
        json!({ "speed": 240, "friction": 3 }),
    ] {
        assert!(
            components.check("top_down_mover", &refused).is_err(),
            "{refused}"
        );
    }
    let accepted = json!({ "speed": 240, "acceleration": 1800, "deceleration": 2400 });
    assert!(components.check("top_down_mover", &accepted).is_ok());
    let player: PrefabData =
        serde_json::from_value(json!({ "components": { "top_down_mover": accepted } })).unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("player".to_string(), player);
    world.insert_resource(prefabs);
    let entity = world
        .prepare_prefab("player", [0.0, 0.0])
        .unwrap()
        .spawn(&mut world);
    let mover = world.get::<TopDownMover>(entity).unwrap();
    assert_eq!(
        (mover.acceleration, mover.deceleration),
        (Some(1800.0), Some(2400.0))
    );
    let plain: TopDownMoverDef = serde_json::from_value(json!({ "speed": 240 })).unwrap();
    assert_eq!((plain.acceleration, plain.deceleration), (None, None));
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({ "speed": 240.0 })
    );
}
