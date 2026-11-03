use super::*;
use crate::{KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_REACT, KIT_RESOLVE, KIT_TICK};
use serde_json::json;
use tungsten_core::Stage;
use tungsten_core::physics::{
    PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PhysicsBuffers, PhysicsPlugin, RigidBody, RigidBodyBundle,
};
use tungsten_core::{
    Bundle, CommandBuffer, ComponentRegistry, LayerKind, TilemapData, TilemapInstance,
    TilemapRegistry, Time, Transform,
};

const DT: f32 = 1.0 / 60.0;
const GRAVITY: f32 = 3600.0;
const RUN: f32 = 560.0;
const JUMP: f32 = 1280.0;
const HALF: Vec2 = Vec2::new(10.0, 20.0);

/// A world with gravity 3600 px/s², the physics and the controller.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(PhysicsConfig {
        gravity: Vec2::new(0.0, GRAVITY),
        ..PhysicsConfig::default()
    });
    world.insert_resource(TilemapRegistry::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    PlatformerControllerPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// A row of `cols` solid 16 px tiles whose top face is at y 0, from x 0.
fn tile_floor(world: &mut World, cols: u32) {
    let mut map = TilemapData::new(16, 16, cols, 1);
    let solid = map.add_tile("solid");
    let layer = map.add_layer("solid", LayerKind::Collision);
    map.fill(layer, solid);
    world
        .resource_mut::<TilemapRegistry>()
        .insert("floor".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("floor", Vec2::ZERO));
}

fn static_box(world: &mut World, center: Vec2, half: Vec2) -> Entity {
    world.spawn_with(RigidBodyBundle::r#static(
        Position(center),
        Collider::aabb(half),
    ))
}

/// A dynamic box of `mass` and half extents `half` with its foot at `foot`.
fn dynamic_box(world: &mut World, foot: Vec2, half: Vec2, mass: f32) -> Entity {
    world.spawn_with(
        RigidBodyBundle::dynamic(
            Position(foot - Vec2::new(0.0, half.y)),
            Collider::aabb(half),
        )
        .with_body(RigidBody::dynamic().with_mass(mass)),
    )
}

/// A dynamic circle of `radius` resting with its foot at `foot`.
fn ball(world: &mut World, foot: Vec2, radius: f32) -> Entity {
    world.spawn_with(RigidBodyBundle::dynamic(
        Position(foot - Vec2::new(0.0, radius)),
        Collider::circle(radius),
    ))
}

/// A controlled body standing with its feet at `feet`.
fn hero(world: &mut World, feet: Vec2, controller: PlatformerController) -> Entity {
    world.spawn_with(
        RigidBodyBundle::kinematic(
            Position(feet - Vec2::new(0.0, HALF.y)),
            Collider::aabb(HALF),
        )
        .with((controller,)),
    )
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(DT);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn steps(world: &mut World, schedule: &mut Schedule, count: usize) {
    for _ in 0..count {
        step(world, schedule);
    }
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

fn controller(world: &mut World, entity: Entity) -> &mut PlatformerController {
    world.get_mut::<PlatformerController>(entity).unwrap()
}

fn state(world: &World, entity: Entity) -> &PlatformerController {
    world.get::<PlatformerController>(entity).unwrap()
}

/// The highest point, as the least y, over `count` steps.
fn peak(world: &mut World, schedule: &mut Schedule, entity: Entity, count: usize) -> f32 {
    let mut top = position(world, entity).y;
    for _ in 0..count {
        step(world, schedule);
        top = top.min(position(world, entity).y);
    }
    top
}

/// A hero on a tile floor, grounded after its first two steps.
fn standing(controller: PlatformerController) -> (World, Schedule, Entity) {
    let (mut world, mut schedule) = rig();
    tile_floor(&mut world, 64);
    let entity = hero(&mut world, Vec2::new(200.0, 0.0), controller);
    steps(&mut world, &mut schedule, 2);
    assert!(state(&world, entity).grounded());
    (world, schedule, entity)
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
            PLATFORMER_CONTROLLER,
            PHYSICS_STEP,
            KIT_OVERLAP,
            KIT_RESOLVE,
            KIT_REACT
        ]
    );
}

#[test]
fn the_first_step_moves_nothing_then_it_runs_on_a_tile_floor() {
    let (mut world, mut schedule) = rig();
    tile_floor(&mut world, 64);
    let mut pc = PlatformerController::new(RUN, JUMP);
    pc.set_run(1.0);
    let entity = hero(&mut world, Vec2::new(100.0, 0.0), pc);
    step(&mut world, &mut schedule);
    assert_eq!(position(&world, entity), Vec2::new(100.0, -20.0));
    assert_eq!(world.get::<Velocity>(entity).unwrap().0, Vec2::ZERO);
    steps(&mut world, &mut schedule, 30);
    let at = position(&world, entity);
    assert!((at.x - (100.0 + RUN * 30.0 * DT)).abs() < 0.01, "{at}");
    assert_eq!(at.y, -20.0, "the floor holds it across the tile seams");
    assert!(state(&world, entity).grounded());
}

#[test]
fn it_stops_at_a_static_box_and_its_held_speed_goes() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    static_box(&mut world, Vec2::new(410.0, -50.0), Vec2::new(10.0, 50.0));
    controller(&mut world, entity).set_run(1.0);
    steps(&mut world, &mut schedule, 60);
    let at = position(&world, entity);
    assert!(at.x <= 390.0 && at.x > 389.9, "{at}");
    assert!(world.get::<Velocity>(entity).unwrap().0.length() < 0.01);
    assert_eq!(state(&world, entity).velocity.x, 0.0);
}

#[test]
fn a_held_jump_follows_the_parabola_to_its_peak() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    let start = position(&world, entity).y;
    controller(&mut world, entity).press_jump();
    step(&mut world, &mut schedule);
    assert!(state(&world, entity).jumped());
    assert!(!state(&world, entity).grounded());
    // At each step's end it lies on y = start - (J t - g t² / 2).
    for n in 1..15 {
        let t = n as f32 * DT;
        let expected = start - (JUMP * t - GRAVITY * t * t * 0.5);
        let y = position(&world, entity).y;
        assert!((y - expected).abs() < 0.01, "step {n}: {y} vs {expected}");
        step(&mut world, &mut schedule);
        assert!(!state(&world, entity).jumped());
    }
    let top = peak(&mut world, &mut schedule, entity, 20);
    let height = start - top;
    let exact = JUMP * JUMP / (2.0 * GRAVITY);
    assert!(
        height <= exact && exact - height < 0.25,
        "{height} vs {exact}"
    );
    steps(&mut world, &mut schedule, 30);
    assert!(state(&world, entity).grounded());
    assert!((position(&world, entity).y - start).abs() < 0.02);
}

#[test]
fn an_early_release_cuts_the_jump_once_and_a_cut_of_one_keeps_it() {
    let jump_after_release = |cut: f32| {
        let (mut world, mut schedule, entity) =
            standing(PlatformerController::new(RUN, JUMP).with_jump_cut(cut));
        let start = position(&world, entity).y;
        controller(&mut world, entity).press_jump();
        steps(&mut world, &mut schedule, 5);
        controller(&mut world, entity).set_jump_held(false);
        start - peak(&mut world, &mut schedule, entity, 30)
    };
    let full = JUMP * JUMP / (2.0 * GRAVITY);
    let cut = jump_after_release(0.5);
    // Five steps at full speed, then half the speed left.
    let t = 5.0 * DT;
    let v = (JUMP - GRAVITY * t) * 0.5;
    let expected = JUMP * t - GRAVITY * t * t * 0.5 + v * v / (2.0 * GRAVITY);
    assert!((cut - expected).abs() < 0.25, "{cut} vs {expected}");
    assert!(cut < full - 50.0);
    let kept = jump_after_release(1.0);
    assert!(full - kept < 0.25 && kept <= full, "{kept} vs {full}");
}

#[test]
fn a_press_alone_holds_the_jump_at_full_height() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    let start = position(&world, entity).y;
    controller(&mut world, entity).press_jump();
    let height = start - peak(&mut world, &mut schedule, entity, 30);
    assert!(JUMP * JUMP / (2.0 * GRAVITY) - height < 0.25, "{height}");
}

#[test]
fn coyote_time_takes_a_press_made_just_after_the_edge_and_not_later() {
    // Returns whether a press `late` steps after it left the edge jumped.
    let jumps_after = |late: usize| {
        let (mut world, mut schedule) = rig();
        tile_floor(&mut world, 10);
        let mut pc = PlatformerController::new(120.0, JUMP);
        pc.set_run(1.0);
        let entity = hero(&mut world, Vec2::new(140.0, 0.0), pc);
        steps(&mut world, &mut schedule, 2);
        while state(&world, entity).grounded() {
            step(&mut world, &mut schedule);
        }
        steps(&mut world, &mut schedule, late);
        let can = state(&world, entity).can_jump();
        controller(&mut world, entity).press_jump();
        step(&mut world, &mut schedule);
        assert_eq!(state(&world, entity).jumped(), can);
        can
    };
    assert!(jumps_after(0));
    assert!(jumps_after(4));
    assert!(!jumps_after(6));
    assert!(!jumps_after(12));
}

#[test]
fn the_jump_buffer_takes_a_press_made_just_before_landing_and_not_earlier() {
    // Drops from 120 px; returns whether a press `early` steps before the
    // landing step jumped right after it.
    let jumps_after_landing = |early: usize| {
        let mut landing = 0;
        {
            let (mut world, mut schedule) = rig();
            tile_floor(&mut world, 64);
            let entity = hero(
                &mut world,
                Vec2::new(200.0, -120.0),
                PlatformerController::new(RUN, JUMP),
            );
            while !state(&world, entity).grounded() {
                step(&mut world, &mut schedule);
                landing += 1;
            }
        }
        let (mut world, mut schedule) = rig();
        tile_floor(&mut world, 64);
        let entity = hero(
            &mut world,
            Vec2::new(200.0, -120.0),
            PlatformerController::new(RUN, JUMP),
        );
        steps(&mut world, &mut schedule, landing - early);
        controller(&mut world, entity).press_jump();
        steps(&mut world, &mut schedule, early + 1);
        let mut jumped = false;
        for _ in 0..2 {
            jumped |= state(&world, entity).jumped();
            step(&mut world, &mut schedule);
        }
        jumped
    };
    assert!(jumps_after_landing(1));
    assert!(jumps_after_landing(4));
    assert!(!jumps_after_landing(8));
}

#[test]
fn a_press_while_locked_is_dropped_and_the_horizontal_speed_holds() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    let start = position(&world, entity);
    {
        let pc = controller(&mut world, entity);
        pc.launch(Vec2::new(300.0, 0.0));
        pc.lock_input(0.2);
        pc.lock_input(0.1);
    }
    step(&mut world, &mut schedule);
    assert!(!state(&world, entity).can_jump(), "locked");
    controller(&mut world, entity).press_jump();
    steps(&mut world, &mut schedule, 5);
    assert!(!state(&world, entity).jumped());
    let at = position(&world, entity);
    assert!((at.x - start.x - 300.0 * 6.0 * DT).abs() < 0.01, "{at}");
    // The lock of 0.2 s, the longer, ends; the run input of 0 then stops
    // it, and the dropped press stays dropped.
    steps(&mut world, &mut schedule, 7);
    let x = position(&world, entity).x;
    steps(&mut world, &mut schedule, 3);
    assert_eq!(position(&world, entity).x, x);
    assert!(state(&world, entity).can_jump());
    assert!(state(&world, entity).grounded());
}

#[test]
fn a_launch_replaces_the_velocity_and_ungrounds_it() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    let start = position(&world, entity).y;
    {
        let pc = controller(&mut world, entity);
        pc.launch(Vec2::new(0.0, -100.0));
        pc.launch(Vec2::new(0.0, -600.0));
    }
    step(&mut world, &mut schedule);
    assert!(!state(&world, entity).grounded());
    assert!(!state(&world, entity).jumped(), "a launch is not a jump");
    let rise = start - position(&world, entity).y;
    let expected = (600.0 - GRAVITY * DT * 0.5) * DT;
    assert!((rise - expected).abs() < 0.01, "{rise} vs {expected}");
    // A release does not cut a launch.
    controller(&mut world, entity).set_jump_held(false);
    let height = start - peak(&mut world, &mut schedule, entity, 20);
    assert!(
        (height - 600.0 * 600.0 / (2.0 * GRAVITY)).abs() < 0.25,
        "{height}"
    );
}

#[test]
fn pushes_sum_and_add_after_the_input() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    let start = position(&world, entity);
    {
        let pc = controller(&mut world, entity);
        pc.set_run(1.0);
        pc.push(Vec2::new(-100.0, -300.0));
        pc.push(Vec2::new(-60.0, -300.0));
    }
    step(&mut world, &mut schedule);
    let at = position(&world, entity);
    assert!((at.x - start.x - (RUN - 160.0) * DT).abs() < 0.01, "{at}");
    let rise = (600.0 - GRAVITY * DT * 0.5) * DT;
    assert!((start.y - at.y - rise).abs() < 0.01, "{at}");
    assert!(!state(&world, entity).grounded());
    // The push lasts one step: the run input sets the speed again.
    step(&mut world, &mut schedule);
    let next = position(&world, entity);
    assert!((next.x - at.x - RUN * DT).abs() < 0.01, "{next}");
}

#[test]
fn a_ceiling_stops_its_rise() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    // A slab whose underside is 100 px above the hero's head.
    static_box(&mut world, Vec2::new(200.0, -150.0), Vec2::new(50.0, 10.0));
    step(&mut world, &mut schedule);
    controller(&mut world, entity).press_jump();
    let top = peak(&mut world, &mut schedule, entity, 20);
    assert!(top - HALF.y >= -140.0 && top - HALF.y < -139.9, "{top}");
    steps(&mut world, &mut schedule, 40);
    assert!(state(&world, entity).grounded());
}

#[test]
fn it_rides_a_kinematic_ground_and_leaves_it_on_a_jump() {
    let (mut world, mut schedule) = rig();
    let deck = world.spawn_with(RigidBodyBundle::kinematic(
        Position::new(200.0, 10.0),
        Collider::aabb(Vec2::new(60.0, 10.0)),
    ));
    let entity = hero(
        &mut world,
        Vec2::new(200.0, 0.0),
        PlatformerController::new(RUN, JUMP),
    );
    steps(&mut world, &mut schedule, 2);
    world.get_mut::<Velocity>(deck).unwrap().0 = Vec2::new(90.0, -30.0);
    steps(&mut world, &mut schedule, 30);
    let gap = position(&world, deck).y - 10.0 - (position(&world, entity).y + HALF.y);
    let offset = position(&world, entity).x - position(&world, deck).x;
    assert!(state(&world, entity).grounded());
    assert!((0.0..0.02).contains(&gap), "{gap}");
    assert!(offset.abs() < 0.02, "{offset}");
    assert!((position(&world, deck).x - 245.0).abs() < 0.02);
    controller(&mut world, entity).press_jump();
    steps(&mut world, &mut schedule, 2);
    assert!(!state(&world, entity).grounded());
    let before = position(&world, entity).x;
    step(&mut world, &mut schedule);
    assert_eq!(position(&world, entity).x, before, "no carry in the air");
}

#[test]
fn a_dynamic_body_blocks_and_holds_it_up() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    let crate_ = world.spawn_with(RigidBodyBundle::dynamic(
        Position::new(260.0, -15.0),
        Collider::aabb(Vec2::splat(15.0)),
    ));
    controller(&mut world, entity).set_run(1.0);
    steps(&mut world, &mut schedule, 20);
    let at = position(&world, entity);
    let crate_at = position(&world, crate_);
    assert!(at.x + HALF.x <= crate_at.x - 15.0 + 0.5, "{at} {crate_at}");
    assert!(
        (crate_at.x - 260.0).abs() < 0.5,
        "it never pushes: {crate_at}"
    );
}

#[test]
fn a_body_that_is_not_kinematic_is_skipped() {
    let (mut world, mut schedule) = rig();
    let mut pc = PlatformerController::new(RUN, JUMP);
    pc.set_run(1.0);
    let entity = world.spawn_with(
        RigidBodyBundle::dynamic(Position::new(0.0, 0.0), Collider::aabb(HALF)).with((pc,)),
    );
    steps(&mut world, &mut schedule, 3);
    assert!(!state(&world, entity).started);
    assert_eq!(position(&world, entity).x, 0.0);
    world.get_mut::<RigidBody>(entity).unwrap().kind = tungsten_core::BodyKind::Kinematic;
    steps(&mut world, &mut schedule, 2);
    assert!(state(&world, entity).started);
    assert!(position(&world, entity).x > 0.0);
}

#[test]
fn its_own_gravity_replaces_the_worlds() {
    let (mut world, mut schedule, entity) =
        standing(PlatformerController::new(RUN, JUMP).with_gravity(GRAVITY * 2.0));
    let start = position(&world, entity).y;
    controller(&mut world, entity).press_jump();
    let height = start - peak(&mut world, &mut schedule, entity, 20);
    let exact = JUMP * JUMP / (4.0 * GRAVITY);
    assert!(exact - height < 0.25 && height <= exact, "{height}");
}

#[test]
fn a_paused_clock_moves_nothing() {
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    controller(&mut world, entity).set_run(1.0);
    let start = position(&world, entity);
    world.resource_mut::<Time>().pause();
    steps(&mut world, &mut schedule, 5);
    assert_eq!(position(&world, entity), start);
}

#[test]
fn a_run_speed_from_a_stat_takes_zero_and_ignores_a_bad_one() {
    let mut pc = PlatformerController::new(RUN, JUMP);
    pc.set_run_speed(0.0);
    assert_eq!(pc.run_speed, 0.0);
    pc.set_run_speed(f32::NAN);
    pc.set_run_speed(-1.0);
    assert_eq!(pc.run_speed, 0.0);
    pc.set_run_speed(RUN * 2.0);
    assert_eq!(pc.run_speed, RUN * 2.0);
}

#[test]
fn the_run_input_is_clamped_and_nan_is_zero() {
    let mut pc = PlatformerController::new(RUN, JUMP);
    pc.set_run(3.0);
    assert_eq!(pc.run, 1.0);
    pc.set_run(-0.5);
    assert_eq!(pc.run, -0.5);
    pc.set_run(f32::NAN);
    assert_eq!(pc.run, 0.0);
}

#[test]
#[should_panic(expected = "run_speed must be finite and > 0")]
fn a_zero_run_speed_panics() {
    let _ = PlatformerController::new(0.0, JUMP);
}

#[test]
#[should_panic(expected = "jump_speed must be finite and > 0")]
fn a_non_finite_jump_speed_panics() {
    let _ = PlatformerController::new(RUN, f32::INFINITY);
}

#[test]
#[should_panic(expected = "factor must be in [0, 1]")]
fn a_jump_cut_above_one_panics() {
    let _ = PlatformerController::new(RUN, JUMP).with_jump_cut(1.5);
}

#[test]
#[should_panic(expected = "gravity must be finite and >= 0")]
fn a_negative_gravity_panics() {
    let _ = PlatformerController::new(RUN, JUMP).with_gravity(-1.0);
}

#[test]
#[should_panic(expected = "seconds must be finite and >= 0")]
fn a_negative_coyote_time_panics() {
    let _ = PlatformerController::new(RUN, JUMP).with_coyote_time(-0.1);
}

#[test]
#[should_panic(expected = "seconds must be finite and >= 0")]
fn a_nan_jump_buffer_panics() {
    let _ = PlatformerController::new(RUN, JUMP).with_jump_buffer(f32::NAN);
}

#[test]
#[should_panic(expected = "seconds must be finite and >= 0")]
fn a_negative_lock_panics() {
    PlatformerController::new(RUN, JUMP).lock_input(-1.0);
}

#[test]
#[should_panic(expected = "velocity must be finite")]
fn a_non_finite_launch_panics() {
    PlatformerController::new(RUN, JUMP).launch(Vec2::new(f32::NAN, 0.0));
}

#[test]
#[should_panic(expected = "velocity must be finite")]
fn a_non_finite_push_panics() {
    PlatformerController::new(RUN, JUMP).push(Vec2::new(0.0, f32::INFINITY));
}

#[test]
fn air_jumps_refill_on_landing_and_spend_once_per_press() {
    let (mut world, mut schedule, entity) =
        standing(PlatformerController::new(RUN, JUMP).with_air_jumps(1));
    assert_eq!(state(&world, entity).air_jumps_left(), 1);
    controller(&mut world, entity).press_jump();
    step(&mut world, &mut schedule);
    let pc = state(&world, entity);
    assert!(pc.jumped() && !pc.air_jumped(), "a ground jump");
    assert_eq!(pc.air_jumps_left(), 1, "the ground jump spends none");
    steps(&mut world, &mut schedule, 10);
    let before = position(&world, entity).y;
    controller(&mut world, entity).press_jump();
    step(&mut world, &mut schedule);
    let pc = state(&world, entity);
    assert!(pc.jumped() && pc.air_jumped(), "an air jump");
    assert_eq!(pc.air_jumps_left(), 0);
    let rise = before - position(&world, entity).y;
    let expected = (JUMP - GRAVITY * DT * 0.5) * DT;
    assert!(
        (rise - expected).abs() < 0.01,
        "at jump_speed: {rise} vs {expected}"
    );
    // Held, with none left, a second press does nothing.
    step(&mut world, &mut schedule);
    assert!(!state(&world, entity).jumped(), "once per press");
    controller(&mut world, entity).press_jump();
    step(&mut world, &mut schedule);
    assert!(!state(&world, entity).jumped(), "none left");
    // Landing refills it, and the buffered press jumps from the ground.
    while !state(&world, entity).grounded() {
        step(&mut world, &mut schedule);
    }
    assert_eq!(state(&world, entity).air_jumps_left(), 1, "refilled");
    // Off an edge, past coyote time, a press is an air jump.
    let (mut world, mut schedule) = rig();
    tile_floor(&mut world, 10);
    let mut pc = PlatformerController::new(120.0, JUMP).with_air_jumps(2);
    pc.set_run(1.0);
    let entity = hero(&mut world, Vec2::new(140.0, 0.0), pc);
    steps(&mut world, &mut schedule, 2);
    while state(&world, entity).grounded() {
        step(&mut world, &mut schedule);
    }
    steps(&mut world, &mut schedule, 8);
    assert!(!state(&world, entity).can_jump(), "past coyote time");
    for left in [1, 0] {
        controller(&mut world, entity).press_jump();
        step(&mut world, &mut schedule);
        let pc = state(&world, entity);
        assert!(pc.air_jumped(), "{left} left after it");
        assert_eq!(pc.air_jumps_left(), left);
        steps(&mut world, &mut schedule, 3);
    }
    // None by default: a press in the air does nothing, as before.
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    controller(&mut world, entity).press_jump();
    steps(&mut world, &mut schedule, 10);
    controller(&mut world, entity).press_jump();
    step(&mut world, &mut schedule);
    assert!(!state(&world, entity).jumped());
    assert_eq!(state(&world, entity).air_jumps_left(), 0);
}

/// A grounded hero with one free air jump, 100 mana and a paid air jump of
/// 25 mana, set at run time.
fn bounder() -> (World, Schedule, Entity) {
    let (mut world, schedule, entity) =
        standing(PlatformerController::new(RUN, JUMP).with_air_jumps(1));
    world.insert(entity, Stats::new().with("mana", 100.0));
    controller(&mut world, entity).set_air_jump_cost(Some(("mana", 25.0)));
    assert_eq!(state(&world, entity).air_jump_cost(), Some(("mana", 25.0)));
    (world, schedule, entity)
}

fn mana(world: &World, entity: Entity) -> f32 {
    world.get::<Stats>(entity).unwrap().base("mana").unwrap()
}

/// Presses jump and steps once.
fn press<'a>(
    world: &'a mut World,
    schedule: &mut Schedule,
    entity: Entity,
) -> &'a PlatformerController {
    controller(world, entity).press_jump();
    step(world, schedule);
    state(world, entity)
}

#[test]
fn past_the_free_air_jumps_each_press_pays_with_no_cap_and_below_the_cost_is_refused() {
    let (mut world, mut schedule, entity) = bounder();
    let pc = press(&mut world, &mut schedule, entity);
    assert!(
        pc.jumped() && !pc.air_jumped() && !pc.paid_air_jumped(),
        "a ground jump"
    );
    steps(&mut world, &mut schedule, 3);
    let pc = press(&mut world, &mut schedule, entity);
    assert!(
        pc.air_jumped() && !pc.paid_air_jumped(),
        "the free air jump"
    );
    assert_eq!(mana(&world, entity), 100.0, "both free");
    for left in [75.0, 50.0, 25.0, 0.0] {
        steps(&mut world, &mut schedule, 3);
        let before = position(&world, entity).y;
        let pc = press(&mut world, &mut schedule, entity);
        assert!(
            pc.jumped() && pc.air_jumped() && pc.paid_air_jumped(),
            "paid to {left}"
        );
        assert!(!pc.air_jump_refused());
        let rise = before - position(&world, entity).y;
        let expected = (JUMP - GRAVITY * DT * 0.5) * DT;
        assert!(
            (rise - expected).abs() < 0.01,
            "at jump_speed: {rise} vs {expected}"
        );
        assert_eq!(mana(&world, entity), left);
    }
    // At 24 a press is refused, reported on its fresh step alone.
    world
        .get_mut::<Stats>(entity)
        .unwrap()
        .set_base("mana", 24.0);
    steps(&mut world, &mut schedule, 3);
    let pc = press(&mut world, &mut schedule, entity);
    assert!(!pc.jumped() && pc.air_jump_refused(), "refused at 24");
    step(&mut world, &mut schedule);
    let pc = state(&world, entity);
    assert!(!pc.jumped() && !pc.air_jump_refused(), "once a press");
    let pc = press(&mut world, &mut schedule, entity);
    assert!(pc.air_jump_refused(), "a second press reports again");
    assert_eq!(mana(&world, entity), 24.0, "nothing spent");
    // The buffer keeps the refused press: at 25 it pays on that step.
    world
        .get_mut::<Stats>(entity)
        .unwrap()
        .set_base("mana", 25.0);
    step(&mut world, &mut schedule);
    let pc = state(&world, entity);
    assert!(
        pc.air_jumped() && pc.paid_air_jumped() && !pc.air_jump_refused(),
        "paid in the buffer"
    );
    assert_eq!(mana(&world, entity), 0.0);
    // Landing refills the free jump alone; without the stat a press is refused.
    for _ in 0..240 {
        if state(&world, entity).grounded() {
            break;
        }
        step(&mut world, &mut schedule);
    }
    assert!(state(&world, entity).grounded(), "it landed");
    assert_eq!(state(&world, entity).air_jumps_left(), 1);
    assert_eq!(mana(&world, entity), 0.0);
    world.remove_component::<Stats>(entity);
    press(&mut world, &mut schedule, entity);
    steps(&mut world, &mut schedule, 3);
    press(&mut world, &mut schedule, entity);
    steps(&mut world, &mut schedule, 3);
    let pc = press(&mut world, &mut schedule, entity);
    assert!(
        !pc.jumped() && pc.air_jump_refused(),
        "no stat, no paid jump"
    );
    // Taken away at run time, a press past the free ones does nothing.
    controller(&mut world, entity).set_air_jump_cost(None);
    steps(&mut world, &mut schedule, 10);
    let pc = press(&mut world, &mut schedule, entity);
    assert!(!pc.jumped() && !pc.air_jump_refused() && pc.air_jump_cost().is_none());
}

#[test]
fn the_air_jump_cost_and_its_flags_round_trip_and_an_old_save_has_none() {
    let (mut world, mut schedule, entity) = bounder();
    press(&mut world, &mut schedule, entity);
    steps(&mut world, &mut schedule, 3);
    press(&mut world, &mut schedule, entity);
    steps(&mut world, &mut schedule, 3);
    assert!(press(&mut world, &mut schedule, entity).paid_air_jumped());
    let saved = serde_json::to_value(state(&world, entity)).unwrap();
    assert_eq!(
        saved["air_jump_cost"],
        json!({ "stat": "mana", "amount": 25.0 })
    );
    assert_eq!(saved["paid_air_jumped"], json!(true));
    assert!(saved.get("air_jump_refused").is_none(), "false is left out");
    let loaded: PlatformerController = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(serde_json::to_value(&loaded).unwrap(), saved);
    assert!(loaded.paid_air_jumped() && loaded.air_jump_cost() == Some(("mana", 25.0)));
    world
        .get_mut::<Stats>(entity)
        .unwrap()
        .set_base("mana", 0.0);
    steps(&mut world, &mut schedule, 3);
    assert!(press(&mut world, &mut schedule, entity).air_jump_refused());
    let refused = serde_json::to_value(state(&world, entity)).unwrap();
    assert_eq!(refused["air_jump_refused"], json!(true));
    assert!(
        serde_json::from_value::<PlatformerController>(refused)
            .unwrap()
            .air_jump_refused()
    );
    // Off, none of the three is written, and a save from before them loads
    // with none.
    let off = serde_json::to_value(PlatformerController::new(RUN, JUMP)).unwrap();
    for field in ["air_jump_cost", "paid_air_jumped", "air_jump_refused"] {
        assert!(off.get(field).is_none(), "{field}");
    }
    let mut old = saved.clone();
    let object = old.as_object_mut().unwrap();
    object.remove("air_jump_cost");
    object.remove("paid_air_jumped");
    let loaded: PlatformerController = serde_json::from_value(old).unwrap();
    assert!(loaded.air_jump_cost().is_none() && !loaded.paid_air_jumped());
    for value in [
        json!({ "stat": "Mana", "amount": 25.0 }),
        json!({ "stat": "mana", "amount": -1.0 }),
        json!({ "stat": "mana" }),
        json!({ "stat": "mana", "amount": 25.0, "extra": 1 }),
    ] {
        let mut bad = saved.clone();
        bad["air_jump_cost"] = value.clone();
        assert!(
            serde_json::from_value::<PlatformerController>(bad).is_err(),
            "{value}"
        );
    }
}

#[test]
#[should_panic(expected = "is not a stat name")]
fn an_air_jump_cost_on_a_bad_stat_panics() {
    let _ = PlatformerController::new(RUN, JUMP).with_air_jump_cost("Mana", 25.0);
}

#[test]
#[should_panic(expected = "amount must be finite and >= 0")]
fn a_negative_air_jump_cost_panics() {
    PlatformerController::new(RUN, JUMP).set_air_jump_cost(Some(("mana", -1.0)));
}

#[test]
fn a_save_mid_jump_continues_exactly() {
    let (mut world, mut schedule, entity) =
        standing(PlatformerController::new(RUN, JUMP).with_air_jumps(1));
    controller(&mut world, entity).set_run(1.0);
    controller(&mut world, entity).press_jump();
    steps(&mut world, &mut schedule, 6);
    controller(&mut world, entity).press_jump();
    steps(&mut world, &mut schedule, 2);
    assert_eq!(state(&world, entity).air_jumps_left(), 0, "mid air jump");
    let saved = serde_json::to_value(state(&world, entity)).unwrap();
    let loaded: PlatformerController = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded).unwrap(),
        saved,
        "the save forms"
    );
    assert!(saved.get("filter").is_none() && saved.get("started").is_none());

    let (mut twin, mut twin_schedule) = rig();
    tile_floor(&mut twin, 64);
    let at = world.get::<Position>(entity).unwrap().0;
    let copy = twin
        .spawn_with(RigidBodyBundle::kinematic(Position(at), Collider::aabb(HALF)).with((loaded,)));
    step(&mut twin, &mut twin_schedule);
    assert_eq!(position(&twin, copy), at, "its still first step");
    // 30 steps of the air jump, then its landing and the refill.
    for n in 0..60 {
        step(&mut world, &mut schedule);
        step(&mut twin, &mut twin_schedule);
        assert_eq!(state(&twin, copy), state(&world, entity), "step {n}");
        assert_eq!(position(&twin, copy), position(&world, entity), "step {n}");
    }
    assert!(state(&world, entity).grounded(), "it landed in both");
    assert_eq!(state(&twin, copy).air_jumps_left(), 1);
}

#[test]
fn a_save_with_a_bad_value_is_refused() {
    let saved =
        serde_json::to_value(PlatformerController::new(RUN, JUMP).with_air_jumps(1)).unwrap();
    let with = |field: &str, value: serde_json::Value| {
        let mut save = saved.clone();
        save[field] = value;
        serde_json::from_value::<PlatformerController>(save)
    };
    assert!(
        with("run_speed", serde_json::json!(0.0)).is_ok(),
        "a stat's 0"
    );
    for (field, value) in [
        ("run_speed", serde_json::json!(-1.0)),
        ("jump_speed", serde_json::json!(0.0)),
        ("gravity", serde_json::json!(-1.0)),
        ("coyote_time", serde_json::json!(-0.1)),
        ("jump_cut", serde_json::json!(1.5)),
        ("run", serde_json::json!(2.0)),
        ("lock_left", serde_json::json!(-1.0)),
        ("air_jumps_left", serde_json::json!(2)),
        ("velocity", serde_json::json!([1.0])),
        ("extra", serde_json::json!(1)),
    ] {
        assert!(with(field, value.clone()).is_err(), "{field} = {value}");
    }
    let mut missing = saved.clone();
    missing.as_object_mut().unwrap().remove("air_jumps");
    assert!(serde_json::from_value::<PlatformerController>(missing).is_err());
}

#[test]
fn the_controller_reports_the_kinematic_body_it_stands_on() {
    let (mut world, mut schedule) = rig();
    tile_floor(&mut world, 64);
    // A deck 2 px over the tiles at x 600, the hero over it and one on the
    // tiles at x 200.
    let deck = world.spawn_with(RigidBodyBundle::kinematic(
        Position(Vec2::new(600.0, -40.0)),
        Collider::aabb(Vec2::new(40.0, 8.0)),
    ));
    let rider = hero(
        &mut world,
        Vec2::new(600.0, -48.0),
        PlatformerController::new(RUN, JUMP),
    );
    let walker = hero(
        &mut world,
        Vec2::new(200.0, 0.0),
        PlatformerController::new(RUN, JUMP),
    );
    assert_eq!(state(&world, rider).ground(), None, "none before a step");
    steps(&mut world, &mut schedule, 3);
    assert_eq!(state(&world, rider).ground(), Some(deck));
    assert!(state(&world, walker).grounded());
    assert_eq!(state(&world, walker).ground(), None, "none on tiles");
    controller(&mut world, rider).launch(Vec2::new(0.0, -600.0));
    step(&mut world, &mut schedule);
    assert_eq!(state(&world, rider).ground(), None, "none in the air");
    steps(&mut world, &mut schedule, 40);
    assert_eq!(state(&world, rider).ground(), Some(deck), "landed again");
    // Saved and loaded: none through the still first step, then the deck.
    let saved = serde_json::to_value(state(&world, rider)).unwrap();
    let loaded: PlatformerController = serde_json::from_value(saved).unwrap();
    assert_eq!(loaded.ground(), None);
    world.insert(rider, loaded);
    step(&mut world, &mut schedule);
    assert_eq!(state(&world, rider).ground(), None, "the still first step");
    step(&mut world, &mut schedule);
    assert_eq!(state(&world, rider).ground(), Some(deck));
}

/// A shove of 60 at 0.5: what the shove tests run with.
fn shover() -> PlatformerController {
    PlatformerController::new(RUN, JUMP).with_shove(60.0, 0.5)
}

#[test]
fn a_shove_moves_a_light_body_at_the_run_less_its_share() {
    let (mut world, mut schedule, entity) = standing(shover());
    let crate_ = dynamic_box(&mut world, Vec2::new(260.0, 0.0), Vec2::splat(15.0), 12.0);
    controller(&mut world, entity).set_run(1.0);
    let mut paces = Vec::new();
    for n in 0..30 {
        let before = position(&world, entity).x;
        step(&mut world, &mut schedule);
        let at = position(&world, entity);
        let crate_at = position(&world, crate_);
        assert!(
            at.x + HALF.x <= crate_at.x - 15.0 + 0.001,
            "step {n}: it overlaps the crate: {at} {crate_at}"
        );
        paces.push((at.x - before) / DT);
    }
    let speed = world.get::<Velocity>(crate_).unwrap().0.x;
    let expected = 0.8 * RUN;
    assert!(
        (speed - expected).abs() <= 0.05 * expected,
        "{speed} vs {expected}"
    );
    let pace = paces[20..].iter().sum::<f32>() / 10.0;
    assert!(
        (pace - expected).abs() <= 0.05 * expected,
        "it follows at the crate's pace: {pace}"
    );
    assert!(
        !world.resource::<PhysicsBuffers>().is_sleeping(crate_),
        "the crate is awake"
    );
}

#[test]
fn a_body_past_the_share_blocks_as_before() {
    let (mut world, mut schedule, entity) = standing(shover());
    let crate_ = dynamic_box(&mut world, Vec2::new(260.0, 0.0), Vec2::splat(15.0), 40.0);
    controller(&mut world, entity).set_run(1.0);
    steps(&mut world, &mut schedule, 30);
    let at = position(&world, entity);
    let crate_at = position(&world, crate_);
    assert!(at.x + HALF.x <= crate_at.x - 15.0 + 0.5, "{at} {crate_at}");
    assert!((crate_at.x - 260.0).abs() < 0.5, "it holds: {crate_at}");
    assert_eq!(world.get::<Velocity>(crate_).unwrap().0.x, 0.0);
}

#[test]
fn a_pinned_body_holds_the_shover() {
    let (mut world, mut schedule, entity) = standing(shover());
    let crate_ = dynamic_box(&mut world, Vec2::new(260.0, 0.0), Vec2::splat(15.0), 12.0);
    // A wall whose face touches the crate's far side.
    static_box(&mut world, Vec2::new(285.0, -50.0), Vec2::new(10.0, 50.0));
    controller(&mut world, entity).set_run(1.0);
    steps(&mut world, &mut schedule, 120);
    let at = position(&world, entity);
    let crate_at = position(&world, crate_);
    // The wall's soft contact takes the crate under a pixel into it.
    assert!((crate_at.x - 260.0).abs() < 1.0, "pinned: {crate_at}");
    assert!(at.x + HALF.x <= crate_at.x - 15.0 + 0.5, "{at} {crate_at}");
    assert!(at.x + HALF.x > 244.0, "it reached the crate: {at}");
}

#[test]
fn a_shove_moves_no_kinematic_or_static_body() {
    for kinematic in [true, false] {
        let (mut world, mut schedule, entity) = standing(shover());
        let center = Vec2::new(260.0, -15.0);
        let half = Vec2::splat(15.0);
        let block = if kinematic {
            world.spawn_with(RigidBodyBundle::kinematic(
                Position(center),
                Collider::aabb(half),
            ))
        } else {
            static_box(&mut world, center, half)
        };
        controller(&mut world, entity).set_run(1.0);
        steps(&mut world, &mut schedule, 20);
        let at = position(&world, entity);
        assert!(at.x + HALF.x <= 245.0 && at.x + HALF.x > 244.9, "{at}");
        assert_eq!(position(&world, block), center, "kinematic {kinematic}");
        if kinematic {
            assert_eq!(world.get::<Velocity>(block).unwrap().0, Vec2::ZERO);
        }
    }
}

#[test]
fn a_shove_keeps_a_faster_body_s_speed() {
    let (mut world, mut schedule, entity) = standing(shover());
    // A crate just ahead, already moving away at 500 px/s: faster than the
    // shove's 448, slower than the run.
    let crate_ = dynamic_box(&mut world, Vec2::new(226.0, 0.0), Vec2::splat(15.0), 12.0);
    world.get_mut::<Velocity>(crate_).unwrap().0.x = 500.0;
    controller(&mut world, entity).set_run(1.0);
    for n in 0..10 {
        step(&mut world, &mut schedule);
        let speed = world.get::<Velocity>(crate_).unwrap().0.x;
        assert!((speed - 500.0).abs() < 0.01, "step {n}: {speed}");
    }
    // It meets the crate each step, which moves a step's travel on.
    let at = position(&world, entity);
    let crate_at = position(&world, crate_);
    let gap = crate_at.x - 15.0 - (at.x + HALF.x);
    assert!(
        (0.0..500.0 * DT + 0.1).contains(&gap),
        "it follows the crate: {at} {crate_at}"
    );
}

#[test]
fn the_step_climbs_a_low_box_and_not_a_tall_one() {
    let after = |height: f32| {
        let (mut world, mut schedule, entity) =
            standing(PlatformerController::new(RUN, JUMP).with_step_height(16.0));
        static_box(
            &mut world,
            Vec2::new(300.0, -height * 0.5),
            Vec2::new(60.0, height * 0.5),
        );
        controller(&mut world, entity).set_run(1.0);
        steps(&mut world, &mut schedule, 12);
        let at = position(&world, entity);
        assert!(state(&world, entity).grounded(), "{height}: {at}");
        at
    };
    let low = after(12.0);
    assert!(
        (low.y + HALF.y + 12.0).abs() < 0.02,
        "on the low box: {low}"
    );
    assert!(low.x > 240.0 + HALF.x, "past its edge: {low}");
    let tall = after(20.0);
    assert_eq!(tall.y, -HALF.y, "on the floor at the tall box: {tall}");
    assert!(
        tall.x + HALF.x <= 240.0 && tall.x + HALF.x > 239.9,
        "{tall}"
    );
}

/// The highest a hero rose, whether it stood on `body`, where it ended and
/// how far `body` moved, walking at 50 px/s with a step of 32 into a ball
/// of radius 15 for 200 steps.
fn walk_into_a_ball(collider: Collider) -> (f32, bool, Vec2, f32) {
    let (mut world, mut schedule) = rig();
    tile_floor(&mut world, 64);
    let half_height = collider.shape.half_extents().y;
    let mut pc = PlatformerController::new(50.0, JUMP).with_step_height(32.0);
    pc.set_run(1.0);
    let entity = world.spawn_with(
        RigidBodyBundle::kinematic(Position::new(200.0, -half_height), collider).with((pc,)),
    );
    let ball = ball(&mut world, Vec2::new(240.0, 0.0), 15.0);
    let start = position(&world, ball);
    let (mut top, mut stood) = (0.0_f32, false);
    for _ in 0..200 {
        step(&mut world, &mut schedule);
        top = top.min(position(&world, entity).y + half_height);
        stood |= state(&world, entity).ground() == Some(ball);
    }
    let moved = position(&world, ball).distance(start);
    (top, stood, position(&world, entity), moved)
}

#[test]
fn the_step_climbs_a_ball_at_a_slow_walk() {
    let (top, stood, at, moved) = walk_into_a_ball(Collider::aabb(HALF));
    assert!(stood, "it stood on the ball");
    assert!(top < -20.0, "it rose over the ball: {top}");
    assert!(at.x - HALF.x > 255.0, "past the ball: {at}");
    assert!((at.y + HALF.y).abs() < 0.02, "on the floor again: {at}");
    assert!(moved <= 0.5, "the ball moved {moved} px");
}

#[test]
fn a_round_controller_climbs_a_ball_too() {
    let (top, stood, at, moved) = walk_into_a_ball(Collider::circle(15.0));
    assert!(stood, "it stood on the ball");
    assert!(top < -20.0, "it rose over the ball: {top}");
    assert!(at.x - 15.0 > 255.0, "past the ball: {at}");
    assert!((at.y + 15.0).abs() < 0.02, "on the floor again: {at}");
    assert!(moved <= 0.5, "the ball moved {moved} px");
}

#[test]
fn the_step_needs_the_ground_and_room_above() {
    // In the air: falling from 30 px into a box 12 px tall, it lands beside
    // it on the floor, though the step would climb it from the ground.
    let (mut world, mut schedule) = rig();
    tile_floor(&mut world, 64);
    let mut pc = PlatformerController::new(RUN, JUMP).with_step_height(16.0);
    pc.set_run(1.0);
    let entity = hero(&mut world, Vec2::new(200.0, -30.0), pc);
    // It reaches the box's face at x 278 with its feet under the box's top.
    static_box(&mut world, Vec2::new(338.0, -6.0), Vec2::new(60.0, 6.0));
    let mut landed = None;
    for _ in 0..30 {
        step(&mut world, &mut schedule);
        if state(&world, entity).grounded() {
            landed = Some(position(&world, entity));
            break;
        }
    }
    let landed = landed.expect("it landed");
    assert!((landed.y + HALF.y).abs() < 0.02, "on the floor: {landed}");
    assert!(landed.x + HALF.x <= 278.0, "beside the box: {landed}");
    // Under a ceiling 5 px over its head, a box 12 px tall stops it.
    let (mut world, mut schedule, entity) =
        standing(PlatformerController::new(RUN, JUMP).with_step_height(16.0));
    static_box(&mut world, Vec2::new(300.0, -6.0), Vec2::new(60.0, 6.0));
    static_box(&mut world, Vec2::new(300.0, -55.0), Vec2::new(150.0, 10.0));
    controller(&mut world, entity).set_run(1.0);
    steps(&mut world, &mut schedule, 20);
    let at = position(&world, entity);
    assert_eq!(at.y, -HALF.y, "still on the floor: {at}");
    assert!(at.x + HALF.x <= 240.0 && at.x + HALF.x > 239.9, "{at}");
}

#[test]
fn a_kinematic_body_is_never_a_step() {
    // A low kinematic box blocks it.
    let (mut world, mut schedule, entity) =
        standing(PlatformerController::new(RUN, JUMP).with_step_height(16.0));
    world.spawn_with(RigidBodyBundle::kinematic(
        Position::new(300.0, -6.0),
        Collider::aabb(Vec2::new(60.0, 6.0)),
    ));
    controller(&mut world, entity).set_run(1.0);
    steps(&mut world, &mut schedule, 20);
    let at = position(&world, entity);
    assert_eq!(at.y, -HALF.y, "{at}");
    assert!(at.x + HALF.x <= 240.0 && at.x + HALF.x > 239.9, "{at}");
    // A low static lip before a kinematic deck: the way down meets the
    // deck first, so it does not climb.
    let (mut world, mut schedule, entity) =
        standing(PlatformerController::new(RUN, JUMP).with_step_height(16.0));
    static_box(&mut world, Vec2::new(247.0, -6.0), Vec2::new(1.0, 6.0));
    world.spawn_with(RigidBodyBundle::kinematic(
        Position::new(288.0, -7.0),
        Collider::aabb(Vec2::new(40.0, 7.0)),
    ));
    controller(&mut world, entity).set_run(1.0);
    steps(&mut world, &mut schedule, 20);
    let at = position(&world, entity);
    assert_eq!(at.y, -HALF.y, "{at}");
    assert!(at.x + HALF.x <= 246.0 && at.x + HALF.x > 245.9, "{at}");
}

#[test]
fn a_step_comes_before_a_shove() {
    let (mut world, mut schedule, entity) = standing(shover().with_step_height(16.0));
    let block = dynamic_box(&mut world, Vec2::new(260.0, 0.0), Vec2::new(15.0, 6.0), 1.0);
    let start = position(&world, block);
    controller(&mut world, entity).set_run(1.0);
    let mut stood = false;
    for _ in 0..12 {
        step(&mut world, &mut schedule);
        stood |= state(&world, entity).ground() == Some(block);
    }
    assert!(stood, "it climbed the block");
    assert!(
        position(&world, block).distance(start) <= 0.5,
        "climbed, not pushed: {}",
        position(&world, block)
    );
}

#[test]
fn the_scene_fields_build_the_same_controller() {
    let (mut world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let fields = json!({
        "run_speed": RUN,
        "jump_speed": JUMP,
        "shove_mass": 60,
        "shove_share": 0.25,
        "step_height": 16
    });
    for bad in [
        json!({ "run_speed": RUN, "jump_speed": JUMP, "shove_mass": 0 }),
        json!({ "run_speed": RUN, "jump_speed": JUMP, "shove_mass": 60, "shove_share": 0 }),
        json!({ "run_speed": RUN, "jump_speed": JUMP, "shove_mass": 60, "shove_share": 1.5 }),
        json!({ "run_speed": RUN, "jump_speed": JUMP, "step_height": -1 }),
    ] {
        assert!(
            components
                .check(PLATFORMER_CONTROLLER_COMPONENT, &bad)
                .is_err(),
            "{bad}"
        );
    }
    let built = |world: &mut World, value: &serde_json::Value| {
        let prepared = world
            .resource::<ComponentRegistry>()
            .prepare(
                PLATFORMER_CONTROLLER_COMPONENT,
                value,
                &SpawnContext::new(world, Transform::default()),
            )
            .unwrap();
        let mut buffer = CommandBuffer::new();
        let pending = buffer.spawn();
        prepared.insert_pending(&mut buffer, pending);
        world.flush_reusing(&mut buffer);
        world
            .query::<&PlatformerController>()
            .last()
            .unwrap()
            .clone()
    };
    assert_eq!(
        built(&mut world, &fields),
        PlatformerController::new(RUN, JUMP)
            .with_shove(60.0, 0.25)
            .with_step_height(16.0)
    );
    let plain = json!({ "run_speed": RUN, "jump_speed": JUMP, "shove_share": 0.25 });
    let built_plain = built(&mut world, &plain);
    assert_eq!(built_plain.shove, None, "the share alone shoves nothing");
    assert_eq!(built_plain.step_height, 0.0);
}

#[test]
fn a_save_keeps_the_shove_and_the_step_and_an_old_one_loads_them_off() {
    let set = shover().with_step_height(32.0);
    let saved = serde_json::to_value(&set).unwrap();
    assert_eq!(saved["shove"], json!({ "mass": 60.0, "share": 0.5 }));
    assert_eq!(saved["step_height"], json!(32.0));
    assert_eq!(
        serde_json::from_value::<PlatformerController>(saved.clone()).unwrap(),
        set
    );
    // Off, the saved form is as before: neither field is written.
    let off = serde_json::to_value(PlatformerController::new(RUN, JUMP)).unwrap();
    assert!(off.get("shove").is_none() && off.get("step_height").is_none());
    // A save from before them loads with both off.
    let mut old = saved.clone();
    let object = old.as_object_mut().unwrap();
    object.remove("shove");
    object.remove("step_height");
    let loaded: PlatformerController = serde_json::from_value(old).unwrap();
    assert_eq!(loaded.shove, None);
    assert_eq!(loaded.step_height, 0.0);
    for (field, value) in [
        ("shove", json!({ "mass": 0.0, "share": 0.5 })),
        ("shove", json!({ "mass": 60.0, "share": 0.0 })),
        ("shove", json!({ "mass": 60.0, "share": 1.5 })),
        ("shove", json!({ "mass": 60.0 })),
        ("step_height", json!(-1.0)),
    ] {
        let mut bad = saved.clone();
        bad[field] = value.clone();
        assert!(
            serde_json::from_value::<PlatformerController>(bad).is_err(),
            "{field} = {value}"
        );
    }
}

#[test]
#[should_panic(expected = "mass must be finite and > 0")]
fn a_zero_shove_mass_panics() {
    let _ = PlatformerController::new(RUN, JUMP).with_shove(0.0, 0.5);
}

#[test]
#[should_panic(expected = "share must be in (0, 1]")]
fn a_shove_share_above_one_panics() {
    let _ = PlatformerController::new(RUN, JUMP).with_shove(60.0, 1.5);
}

#[test]
#[should_panic(expected = "height must be finite and >= 0")]
fn a_negative_step_height_panics() {
    let _ = PlatformerController::new(RUN, JUMP).with_step_height(-1.0);
}

#[test]
fn the_controller_jumps_up_through_a_one_way_edge_and_lands_on_it() {
    // A one-way ledge of 16 px tiles, its top face at y -128, over the hero
    // standing on the tile floor: the hero jumps through it to its full
    // peak and lands on it.
    let (mut world, mut schedule, entity) = standing(PlatformerController::new(RUN, JUMP));
    let mut ledge = TilemapData::new(16, 16, 64, 1);
    let tile = ledge.add_tile("ledge");
    let mut props = tungsten_core::TileProps::default();
    props.one_way = true;
    ledge.set_tile_props(tile, props);
    let layer = ledge.add_layer("ledge", LayerKind::Collision);
    ledge.fill(layer, tile);
    world
        .resource_mut::<TilemapRegistry>()
        .insert("ledge".into(), ledge);
    let instance = world.spawn();
    world.insert(
        instance,
        TilemapInstance::new("ledge", Vec2::new(0.0, -128.0)),
    );
    steps(&mut world, &mut schedule, 2);
    assert!(state(&world, entity).grounded(), "still on the floor");
    let start = position(&world, entity).y;
    controller(&mut world, entity).press_jump();
    let top = peak(&mut world, &mut schedule, entity, 25);
    let exact = JUMP * JUMP / (2.0 * GRAVITY);
    assert!(
        start - top > exact - 0.25,
        "it rose through the ledge to {top}"
    );
    steps(&mut world, &mut schedule, 40);
    let feet = position(&world, entity).y + HALF.y;
    assert!(state(&world, entity).grounded(), "it stands");
    assert!((feet + 128.0).abs() < 0.05, "on the ledge: feet at {feet}");
}

/// A world as [`rig`]'s with a map of 16 px cells at the origin drawn by
/// rows of letters: `#` solid, `a` to `l` slope kinds 1 to 12 (`45 br` to
/// `2:1 high tl`), `=` one-way, `.` empty.
fn slope_rig(rows: &[String]) -> (World, Schedule) {
    let (mut world, schedule) = rig();
    let names = [
        "45 br",
        "45 bl",
        "45 tr",
        "45 tl",
        "2:1 low br",
        "2:1 low bl",
        "2:1 low tr",
        "2:1 low tl",
        "2:1 high br",
        "2:1 high bl",
        "2:1 high tr",
        "2:1 high tl",
    ];
    let (width, height) = (rows[0].len() as u32, rows.len() as u32);
    let mut map = TilemapData::new(16, 16, width, height);
    let solid = map.add_tile("solid");
    let slopes: Vec<i32> = names
        .iter()
        .map(|name| {
            let tile = map.add_tile(format!("slope {name}"));
            let mut props = tungsten_core::TileProps::default();
            props.slope = Some(tungsten_core::Slope::parse(name).unwrap());
            map.set_tile_props(tile, props);
            tile
        })
        .collect();
    let ledge = map.add_tile("ledge");
    let mut props = tungsten_core::TileProps::default();
    props.one_way = true;
    map.set_tile_props(ledge, props);
    let layer = map.add_layer("solid", LayerKind::Collision);
    for (row, letters) in rows.iter().enumerate() {
        for (col, letter) in letters.chars().enumerate() {
            let tile = match letter {
                '#' => solid,
                '=' => ledge,
                'a'..='l' => slopes[usize::from(letter as u8 - b'a')],
                _ => continue,
            };
            map.set_tile(layer, col as u32, row as u32, tile);
        }
    }
    world
        .resource_mut::<TilemapRegistry>()
        .insert("slopes".into(), map);
    let instance = world.spawn();
    world.insert(instance, TilemapInstance::new("slopes", Vec2::ZERO));
    (world, schedule)
}

/// A 40-cell map whose floor's top is at y 240, with a ramp from x 160
/// rising to the right to a plateau: six `45 br` cells to one at y 144, or
/// five pairs of `2:1 low br` and `2:1 high br` cells to one at y 160; or,
/// with `down`, a plateau at that height ending at x 160 and the mirrored
/// ramp, `45 bl` or `2:1 high bl` and `2:1 low bl`, falling to the right.
fn ramp(two_to_one: bool, down: bool) -> Vec<String> {
    let rise: i32 = if two_to_one { 5 } else { 6 };
    let width = if two_to_one { 2 * rise } else { rise };
    let plateau = 15 - rise;
    (0..16)
        .map(|row: i32| {
            (0..40)
                .map(|col: i32| {
                    if row == 15 {
                        return '#';
                    }
                    let at = col - 10;
                    if (0..width).contains(&at) {
                        let (surface, letter) = match (two_to_one, down) {
                            (false, false) => (14 - at, 'a'),
                            (false, true) => (plateau + at, 'b'),
                            (true, false) => (14 - at / 2, if at % 2 == 0 { 'e' } else { 'i' }),
                            (true, true) => (plateau + at / 2, if at % 2 == 0 { 'j' } else { 'f' }),
                        };
                        return match row.cmp(&surface) {
                            std::cmp::Ordering::Equal => letter,
                            std::cmp::Ordering::Greater => '#',
                            std::cmp::Ordering::Less => '.',
                        };
                    }
                    let high = if down { at < 0 } else { at >= width };
                    if high && row >= plateau { '#' } else { '.' }
                })
                .collect()
        })
        .collect()
}

/// Each step's start and end position and its grounded state and floor
/// normal at the end, for `count` steps.
fn walk(
    world: &mut World,
    schedule: &mut Schedule,
    entity: Entity,
    count: usize,
) -> Vec<(Vec2, Vec2, bool, Option<Vec2>)> {
    (0..count)
        .map(|_| {
            let from = position(world, entity);
            step(world, schedule);
            let pc = state(world, entity);
            (
                from,
                position(world, entity),
                pc.grounded(),
                pc.floor_normal(),
            )
        })
        .collect()
}

#[test]
fn the_controller_walks_slopes_at_its_run_speed() {
    // Up a ramp and back down, 45° and 2:1: grounded every step, and every
    // step that starts and ends on the face moves along it at the run speed.
    for two_to_one in [false, true] {
        let face = if two_to_one {
            Vec2::new(-1.0, -2.0).normalize()
        } else {
            Vec2::new(-1.0, -1.0).normalize()
        };
        let (mut world, mut schedule) = slope_rig(&ramp(two_to_one, false));
        let entity = hero(
            &mut world,
            Vec2::new(60.0, 240.0),
            PlatformerController::new(RUN, JUMP),
        );
        steps(&mut world, &mut schedule, 2);
        let mut on_face = 0;
        for run in [1.0, -1.0] {
            controller(&mut world, entity).set_run(run);
            let mut last = state(&world, entity).floor_normal();
            for (from, to, grounded, normal) in walk(&mut world, &mut schedule, entity, 45) {
                let at = format!("2:1 {two_to_one}, run {run}: {from} to {to}");
                assert!(grounded, "{at}: off the ground");
                let face_both = [last, normal]
                    .iter()
                    .all(|n| n.is_some_and(|n| n.abs_diff_eq(face, 1e-4)));
                if face_both {
                    let speed = from.distance(to) / DT;
                    assert!(
                        (speed - RUN).abs() < 0.01 * RUN,
                        "{at}: {speed} px/s along the face"
                    );
                    on_face += 1;
                }
                last = normal;
            }
        }
        assert!(
            on_face > 20,
            "2:1 {two_to_one}: {on_face} steps on the face"
        );
        // Back where it started, on the floor.
        let feet = position(&world, entity).y + HALF.y;
        assert!(
            (feet - 240.0).abs() < 0.05,
            "2:1 {two_to_one}: feet at {feet}"
        );
    }
}

#[test]
fn the_controller_stands_and_slides_by_its_limits() {
    // Halfway up a 45° ramp, its face y = 400 − x: the default stands
    // still; a slide limit of 30° slides it down along the face, its speed
    // along it growing by g · sin 45° · dt a sliding step, its own gravity
    // 600; on a 2:1 ramp, gentler than 30°, it stands.
    let place = |two_to_one: bool, controller: PlatformerController| {
        let (mut world, mut schedule) = slope_rig(&ramp(two_to_one, false));
        let feet = if two_to_one {
            // The 2:1 face: y = 240 − (x − 160) / 2.
            Vec2::new(220.0, 240.0 - (230.0 - 160.0) / 2.0 - 0.5)
        } else {
            Vec2::new(220.0, 400.0 - 230.0 - 0.5)
        };
        let entity = hero(&mut world, feet, controller);
        steps(&mut world, &mut schedule, 3);
        assert!(state(&world, entity).grounded(), "on the face");
        (world, schedule, entity)
    };
    let (mut world, mut schedule, entity) = place(false, PlatformerController::new(RUN, JUMP));
    let still = position(&world, entity);
    steps(&mut world, &mut schedule, 60);
    assert_eq!(position(&world, entity), still, "the default stands");

    let sliding = PlatformerController::new(RUN, JUMP)
        .with_gravity(600.0)
        .with_slide_limit(30.0);
    let (mut world, mut schedule, entity) = place(false, sliding.clone());
    let face = Vec2::new(-1.0, -1.0).normalize();
    let growth = 600.0 * std::f32::consts::FRAC_1_SQRT_2 * DT;
    let mut last: Option<f32> = None;
    let mut sliding_steps = 0;
    for (from, to, grounded, normal) in walk(&mut world, &mut schedule, entity, 30) {
        assert!(
            grounded && to.x < from.x && to.y > from.y,
            "down the face: {from} to {to}"
        );
        assert!(
            normal.is_some_and(|n| n.abs_diff_eq(face, 1e-4)),
            "{normal:?}"
        );
        let speed = from.distance(to) / DT;
        if let Some(last) = last {
            assert!(
                (speed - last - growth).abs() < 0.01 * growth,
                "{last} to {speed} px/s, g · sin 45° · dt gives {growth} a step"
            );
            sliding_steps += 1;
        }
        last = Some(speed);
    }
    assert!(sliding_steps >= 20, "{sliding_steps} sliding steps");

    let (mut world, mut schedule, entity) = place(true, sliding);
    let still = position(&world, entity);
    steps(&mut world, &mut schedule, 60);
    assert_eq!(position(&world, entity), still, "2:1 is gentler than 30°");
}

#[test]
fn the_controller_snaps_down_a_slope_and_over_its_crest() {
    // Off a plateau down a ramp falling to the right, 45° and 2:1, and up a
    // ramp over its crest onto a plateau: grounded every step.
    for two_to_one in [false, true] {
        let top = if two_to_one { 160.0 } else { 144.0 };
        let (mut world, mut schedule) = slope_rig(&ramp(two_to_one, true));
        let entity = hero(
            &mut world,
            Vec2::new(100.0, top),
            PlatformerController::new(RUN, JUMP),
        );
        steps(&mut world, &mut schedule, 2);
        controller(&mut world, entity).set_run(1.0);
        for (from, to, grounded, _) in walk(&mut world, &mut schedule, entity, 40) {
            assert!(
                grounded,
                "2:1 {two_to_one}, down: {from} to {to} off the ground"
            );
        }
        let feet = position(&world, entity).y + HALF.y;
        assert!(
            (feet - 240.0).abs() < 0.05,
            "2:1 {two_to_one}: down on the floor at {feet}"
        );

        let (mut world, mut schedule) = slope_rig(&ramp(two_to_one, false));
        let entity = hero(
            &mut world,
            Vec2::new(60.0, 240.0),
            PlatformerController::new(RUN, JUMP),
        );
        steps(&mut world, &mut schedule, 2);
        controller(&mut world, entity).set_run(1.0);
        for (from, to, grounded, _) in walk(&mut world, &mut schedule, entity, 40) {
            assert!(
                grounded,
                "2:1 {two_to_one}, up: {from} to {to} off the ground"
            );
        }
        let feet = position(&world, entity).y + HALF.y;
        assert!(
            (feet - top).abs() < 0.05,
            "2:1 {two_to_one}: on the plateau at {feet}"
        );
        assert_eq!(state(&world, entity).floor_normal(), Some(Vec2::NEG_Y));
    }
}

#[test]
fn the_controller_leaves_a_flat_ledge_as_before() {
    // A ledge whose top is at y 160 ending at x 160, a floor 3 px lower past
    // it, inside the snap reach: grounded while its box overlaps the ledge
    // at a step's start, and off the ground at the first step whose box
    // starts clear of it, as the 1 px probe gives.
    let (mut world, mut schedule) = rig();
    static_box(&mut world, Vec2::new(-40.0, 170.0), Vec2::new(200.0, 10.0));
    static_box(&mut world, Vec2::new(360.0, 173.0), Vec2::new(200.0, 10.0));
    let entity = hero(
        &mut world,
        Vec2::new(100.0, 160.0),
        PlatformerController::new(RUN, JUMP),
    );
    steps(&mut world, &mut schedule, 2);
    controller(&mut world, entity).set_run(1.0);
    let mut left = false;
    for (from, to, grounded, _) in walk(&mut world, &mut schedule, entity, 12) {
        let over = from.x - HALF.x < 160.0;
        if over {
            assert!(grounded, "{from} to {to}: over the ledge");
        } else {
            assert!(!grounded, "{from} to {to}: clear of the ledge");
            left = true;
            break;
        }
    }
    assert!(left, "it reached the ledge's end");
}

#[test]
fn the_controller_stops_at_a_slope_past_its_limit() {
    // A slope limit of 30°: a 45° ramp, its face y = 400 − x from its foot
    // at (160, 240), stops it at the foot as a wall, where the default climbs
    // it; a 2:1 ramp, 26.6°, it climbs. Its feet start exactly on the floor,
    // a 0 px contact as a spawn on it leaves them, and 0.01 px over it: its
    // box never lies more than 1e-3 px into the 45° ramp.
    let depth = |at: Vec2| {
        let corner = at + HALF;
        if (160.0..=256.0).contains(&corner.x) {
            (corner.x + corner.y - 400.0) * std::f32::consts::FRAC_1_SQRT_2
        } else {
            f32::NEG_INFINITY
        }
    };
    let run_at = |two_to_one: bool, above: f32, controller: PlatformerController| {
        let (mut world, mut schedule) = slope_rig(&ramp(two_to_one, false));
        let entity = hero(&mut world, Vec2::new(60.0, 240.0 - above), controller);
        steps(&mut world, &mut schedule, 2);
        controller_of(&mut world, entity).set_run(1.0);
        let mut deepest = f32::NEG_INFINITY;
        let mut held = Vec2::ZERO;
        for k in 0..40 {
            step(&mut world, &mut schedule);
            if !two_to_one {
                deepest = deepest.max(depth(position(&world, entity)));
            }
            if k == 29 {
                held = position(&world, entity);
            }
        }
        (held, position(&world, entity), deepest)
    };
    for above in [0.0, 0.01] {
        let limited = PlatformerController::new(RUN, JUMP).with_slope_limit(30.0);
        let (held, at, deepest) = run_at(false, above, limited.clone());
        assert_eq!(held, at, "{above} px up: it stays stopped");
        assert!(
            (at.y + HALF.y - 240.0).abs() < 0.05,
            "{above} px up: on the floor at {at}"
        );
        assert!(
            at.x + HALF.x < 160.0 + 1e-3,
            "{above} px up: at the ramp's foot: {at}"
        );
        assert!(deepest <= 1e-3, "{above} px up: {deepest} px into the ramp");
        let (_, climbed, deepest) = run_at(false, above, PlatformerController::new(RUN, JUMP));
        assert!(
            climbed.y + HALF.y < 150.0,
            "{above} px up: the default climbs: {climbed}"
        );
        assert!(deepest <= 1e-3, "{above} px up: {deepest} px into the ramp");
        let (_, gentle, _) = run_at(true, above, limited);
        assert!(
            gentle.y + HALF.y < 170.0,
            "{above} px up: 2:1 is within 30°: {gentle}"
        );
    }
}

fn controller_of(world: &mut World, entity: Entity) -> &mut PlatformerController {
    controller(world, entity)
}

#[test]
fn the_controller_drops_through_a_one_way_edge() {
    // A one-way ledge whose top is at y 160 over a floor at y 464: dropped
    // through by call and by a jump press with the movement down, at
    // gravity 10 and 2,000 and steps of 1/60 and 1/240 s, it is never on
    // the ground for half a second and ends below the ledge, falling.
    let mut rows = vec![".".repeat(40); 30];
    rows[10] = format!("{}{}{}", ".".repeat(5), "=".repeat(25), ".".repeat(10));
    rows[29] = "#".repeat(40);
    for gravity in [10.0, 2000.0] {
        for dt in [1.0 / 60.0, 1.0 / 240.0] {
            for by_intent in [false, true] {
                let at = format!("gravity {gravity}, dt {dt}, intent {by_intent}");
                let (mut world, mut schedule) = slope_rig(&rows);
                let entity = hero(
                    &mut world,
                    Vec2::new(200.0, 160.0),
                    PlatformerController::new(RUN, JUMP).with_gravity(gravity),
                );
                let mut tick = |world: &mut World| {
                    world.resource_mut::<Time>().advance_frame(dt);
                    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
                };
                for _ in 0..3 {
                    tick(&mut world);
                }
                assert!(state(&world, entity).grounded(), "{at}: on the ledge");
                if by_intent {
                    let mut intent = crate::Intent::default();
                    intent.set_movement(Vec2::new(0.0, 1.0));
                    intent.set_slot("jump", true, true);
                    world.insert(entity, intent);
                } else {
                    controller(&mut world, entity).drop_through();
                }
                let start = position(&world, entity).y;
                let mut last = start;
                for k in 0..(0.5 / dt).round() as usize {
                    tick(&mut world);
                    if k == 0 && by_intent {
                        world
                            .get_mut::<crate::Intent>(entity)
                            .unwrap()
                            .set_slot("jump", false, false);
                    }
                    let y = position(&world, entity).y;
                    assert!(
                        !state(&world, entity).grounded(),
                        "{at}, step {k}: grounded at {y}"
                    );
                    assert!(y >= last, "{at}, step {k}: rose to {y}");
                    last = y;
                }
                let feet = last + HALF.y;
                let falling = world.get::<Velocity>(entity).unwrap().0.y;
                assert!(feet > 160.0 + 1.0, "{at}: feet at {feet}");
                assert!(falling > 0.0, "{at}: moving at {falling}");
            }
        }
    }
}

#[test]
fn the_controllers_filter_round_trips_its_data_and_save() {
    // `mask` and `without` by number and by physics layer name build its
    // filter; a save writes the filter's layers when they differ from the
    // defaults, and a save from before them loads with today's filter.
    let (mut world, _) = rig();
    world.insert_resource(tungsten_core::physics::PhysicsLayers::new([(
        "ground".to_owned(),
        2,
    )]));
    let fields = json!({
        "run_speed": RUN,
        "jump_speed": JUMP,
        "mask": [0, "ground"],
        "without": [3],
        "slope_limit": 50,
        "slide_limit": 40
    });
    let components = world.resource::<ComponentRegistry>();
    components
        .check_in(PLATFORMER_CONTROLLER_COMPONENT, &fields, &world)
        .unwrap();
    let unknown = json!({ "run_speed": RUN, "jump_speed": JUMP, "mask": [0, "sky"] });
    let refused = components
        .check_in(PLATFORMER_CONTROLLER_COMPONENT, &unknown, &world)
        .unwrap_err();
    assert!(
        matches!(
            &refused,
            tungsten_core::ComponentError::MissingAsset { field, kind: "physics layer", id, .. }
                if field == "mask[1]" && id == "sky"
        ),
        "{refused}"
    );
    let unknown = json!({ "run_speed": RUN, "jump_speed": JUMP, "without": ["ground", "sea"] });
    let refused = components
        .check_in(PLATFORMER_CONTROLLER_COMPONENT, &unknown, &world)
        .unwrap_err();
    assert!(
        matches!(
            &refused,
            tungsten_core::ComponentError::MissingAsset { field, id, .. }
                if field == "without[1]" && id == "sea"
        ),
        "{refused}"
    );
    for bad in [
        json!({ "run_speed": RUN, "jump_speed": JUMP, "mask": [32] }),
        json!({ "run_speed": RUN, "jump_speed": JUMP, "without": [-1] }),
        json!({ "run_speed": RUN, "jump_speed": JUMP, "slope_limit": 90 }),
        json!({ "run_speed": RUN, "jump_speed": JUMP, "slide_limit": 0 }),
    ] {
        assert!(
            components
                .check(PLATFORMER_CONTROLLER_COMPONENT, &bad)
                .is_err(),
            "{bad}"
        );
    }
    let prepared = world
        .resource::<ComponentRegistry>()
        .prepare(
            PLATFORMER_CONTROLLER_COMPONENT,
            &fields,
            &SpawnContext::new(&world, Transform::default()),
        )
        .unwrap();
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();
    prepared.insert_pending(&mut buffer, pending);
    world.flush_reusing(&mut buffer);
    let built = world
        .query::<&PlatformerController>()
        .last()
        .unwrap()
        .clone();
    let filter = SpatialFilter::new().with_mask(0b101).without_layers(0b1000);
    let expected = PlatformerController::new(RUN, JUMP)
        .with_filter(filter)
        .with_slope_limit(50.0)
        .with_slide_limit(40.0);
    assert_eq!(built, expected);

    let saved = serde_json::to_value(&built).unwrap();
    assert_eq!(
        (saved["mask"].clone(), saved["without"].clone()),
        (json!(5), json!(8))
    );
    assert_eq!(
        serde_json::from_value::<PlatformerController>(saved.clone()).unwrap(),
        built
    );
    let plain = serde_json::to_value(PlatformerController::new(RUN, JUMP)).unwrap();
    for key in [
        "mask",
        "without",
        "slope_limit",
        "slide_limit",
        "drop_edge",
        "slope_normal",
    ] {
        assert!(plain.get(key).is_none(), "{key} is written at its default");
    }
    let mut old = saved;
    for key in ["mask", "without", "slope_limit", "slide_limit"] {
        old.as_object_mut().unwrap().remove(key);
    }
    let loaded: PlatformerController = serde_json::from_value(old).unwrap();
    assert_eq!(loaded.filter, SpatialFilter::new(), "today's filter");

    // Standing on flat ground it writes the fields it wrote before; on a
    // slope face it keeps the face's normal through a load.
    let (world, _, entity) = standing(PlatformerController::new(RUN, JUMP));
    let saved = serde_json::to_value(state(&world, entity)).unwrap();
    for key in ["slope_normal", "slide_speed", "drop_edge", "drop_asked"] {
        assert!(saved.get(key).is_none(), "{key} written on flat ground");
    }
    let (mut world, mut schedule) = slope_rig(&ramp(false, false));
    let entity = hero(
        &mut world,
        Vec2::new(220.0, 400.0 - 230.0 - 0.5),
        PlatformerController::new(RUN, JUMP),
    );
    steps(&mut world, &mut schedule, 3);
    let face = Vec2::new(-1.0, -1.0).normalize();
    let on_face = state(&world, entity).clone();
    assert!(
        on_face
            .floor_normal()
            .is_some_and(|n| n.abs_diff_eq(face, 1e-4))
    );
    let loaded: PlatformerController =
        serde_json::from_value(serde_json::to_value(&on_face).unwrap()).unwrap();
    assert_eq!(loaded.floor_normal(), on_face.floor_normal());
}

/// A controlled body standing with its feet at `feet`, its kinematic body of
/// mass `mass` when given.
fn weighed_hero(
    world: &mut World,
    feet: Vec2,
    controller: PlatformerController,
    mass: Option<f32>,
) -> Entity {
    let body = mass.map_or(RigidBody::kinematic(), |mass| {
        RigidBody::kinematic().with_mass(mass)
    });
    world.spawn_with(
        RigidBodyBundle::kinematic(
            Position(feet - Vec2::new(0.0, HALF.y)),
            Collider::aabb(HALF),
        )
        .with_body(body)
        .with((controller,)),
    )
}

/// A dynamic circle of `radius` and `mass` resting with its foot at `foot`,
/// moving at `velocity`.
fn thrown_ball(
    world: &mut World,
    foot: Vec2,
    radius: f32,
    mass: f32,
    velocity: Vec2,
    restitution: f32,
) -> Entity {
    world.spawn_with(
        RigidBodyBundle::dynamic(
            Position(foot - Vec2::new(0.0, radius)),
            Collider::circle(radius),
        )
        .with_body(
            RigidBody::dynamic()
                .with_mass(mass)
                .with_restitution(restitution),
        )
        .with_velocity(Velocity(velocity)),
    )
}

/// Each step's `pushed()` over `count` steps.
fn pushes_taken(
    world: &mut World,
    schedule: &mut Schedule,
    entity: Entity,
    count: usize,
) -> Vec<Vec2> {
    (0..count)
        .map(|_| {
            step(world, schedule);
            state(world, entity).pushed()
        })
        .collect()
}

#[test]
fn bodies_and_impulses_push_a_controller_with_a_mass() {
    // A ball of mass 10 at 300 px/s into a controller whose body weighs 60:
    // its begin pushes the body by 3,000 / 70 px/s, which the controller
    // joins at its next step and `pushed` reads; the ball keeps the
    // solver's stop. Without a mass, nothing.
    for mass in [Some(60.0), None] {
        let (mut world, mut schedule) = rig();
        tile_floor(&mut world, 64);
        let entity = weighed_hero(
            &mut world,
            Vec2::new(300.0, 0.0),
            PlatformerController::new(RUN, JUMP),
            mass,
        );
        thrown_ball(
            &mut world,
            Vec2::new(200.0, 0.0),
            6.0,
            10.0,
            Vec2::new(300.0, 0.0),
            0.0,
        );
        steps(&mut world, &mut schedule, 2);
        let taken = pushes_taken(&mut world, &mut schedule, entity, 60);
        let moved: Vec<_> = taken.iter().filter(|push| **push != Vec2::ZERO).collect();
        if mass.is_none() {
            assert!(moved.is_empty(), "no mass: {moved:?}");
            continue;
        }
        assert_eq!(moved.len(), 1, "{taken:?}");
        let expected = 3_000.0 / 70.0;
        assert!(
            (moved[0].x - expected).abs() < 0.01 * expected && moved[0].y.abs() < 1e-3,
            "{}",
            moved[0]
        );
    }
    // A controller weighing 1 hit by mass 10 at 100 px/s: the inelastic
    // share, never the ball's speed or more, at either restitution.
    for restitution in [0.0, 1.0] {
        let (mut world, mut schedule) = rig();
        tile_floor(&mut world, 64);
        let entity = weighed_hero(
            &mut world,
            Vec2::new(300.0, 0.0),
            PlatformerController::new(RUN, JUMP),
            Some(1.0),
        );
        thrown_ball(
            &mut world,
            Vec2::new(270.0, 0.0),
            6.0,
            10.0,
            Vec2::new(100.0, 0.0),
            restitution,
        );
        steps(&mut world, &mut schedule, 2);
        let taken = pushes_taken(&mut world, &mut schedule, entity, 60);
        let total: Vec2 = taken.iter().copied().sum();
        let expected = 1_000.0 / 11.0;
        assert!(
            (total.x - expected).abs() < 0.01 * expected,
            "restitution {restitution}: {total}"
        );
        assert!(
            taken.iter().all(|push| push.length() <= 100.0),
            "restitution {restitution}: {taken:?}"
        );
    }
    // A radial impulse of 600 whose radius reaches twice as far as the
    // controller's side: 300 on a body of 60, 5 px/s.
    let (mut world, mut schedule) = rig();
    tile_floor(&mut world, 64);
    let entity = weighed_hero(
        &mut world,
        Vec2::new(300.0, 0.0),
        PlatformerController::new(RUN, JUMP),
        Some(60.0),
    );
    steps(&mut world, &mut schedule, 3);
    let center = Vec2::new(300.0 - HALF.x - 50.0, -HALF.y);
    let took = tungsten_core::physics::apply_radial_impulse(
        &mut world,
        center,
        100.0,
        600.0,
        &SpatialFilter::new(),
    );
    assert_eq!(took, 1);
    let before = position(&world, entity).x;
    step(&mut world, &mut schedule);
    let pushed = state(&world, entity).pushed();
    assert!(pushed.abs_diff_eq(Vec2::new(5.0, 0.0), 1e-4), "{pushed}");
    let moved = position(&world, entity).x - before;
    assert!((moved - 5.0 * DT).abs() < 1e-4, "{moved} px");
    step(&mut world, &mut schedule);
    assert_eq!(state(&world, entity).pushed(), Vec2::ZERO);
}

#[test]
fn a_locked_controller_ploughs_through_a_body() {
    // Launched along the floor under locked input into a crate of mass 12
    // it cannot shove, a controller whose body weighs 60 gives the crate the
    // inelastic share of their closing speed and keeps the speed they then
    // share, 400 · 60 / 72 px/s; a static wall and a tile still stop it,
    // and without a mass it stops at the crate as before.
    let shared = 400.0 * 60.0 / 72.0;
    let run = |mass: Option<f32>, wall: &str| {
        let (mut world, mut schedule) = rig();
        tile_floor(&mut world, 64);
        let entity = weighed_hero(
            &mut world,
            Vec2::new(100.0, 0.0),
            PlatformerController::new(RUN, JUMP),
            mass,
        );
        let crate_body = match wall {
            "crate" => Some(dynamic_box(
                &mut world,
                Vec2::new(200.0, 0.0),
                Vec2::splat(10.0),
                12.0,
            )),
            "static" => {
                static_box(&mut world, Vec2::new(200.0, -30.0), Vec2::new(10.0, 30.0));
                None
            }
            _ => {
                let mut map = TilemapData::new(16, 16, 1, 4);
                let solid = map.add_tile("solid");
                let layer = map.add_layer("solid", LayerKind::Collision);
                map.fill(layer, solid);
                world
                    .resource_mut::<TilemapRegistry>()
                    .insert("post".into(), map);
                let instance = world.spawn();
                world.insert(
                    instance,
                    TilemapInstance::new("post", Vec2::new(192.0, -64.0)),
                );
                None
            }
        };
        steps(&mut world, &mut schedule, 2);
        {
            let controller = controller(&mut world, entity);
            controller.launch(Vec2::new(400.0, 0.0));
            controller.lock_input(1.0);
        }
        for _ in 0..30 {
            step(&mut world, &mut schedule);
            let crate_speed =
                crate_body.map_or(0.0, |body| world.get::<Velocity>(body).unwrap().0.x);
            if crate_speed != 0.0 || state(&world, entity).velocity.x != 400.0 {
                return (state(&world, entity).velocity.x, crate_speed);
            }
        }
        panic!("{wall}: it never met the wall");
    };
    let (own, crate_speed) = run(Some(60.0), "crate");
    assert!((own - shared).abs() < 0.01 * shared, "controller {own}");
    assert!(
        (crate_speed - shared).abs() < 0.01 * shared,
        "crate {crate_speed}"
    );
    for wall in ["static", "tile"] {
        assert_eq!(run(Some(60.0), wall), (0.0, 0.0), "{wall}");
    }
    assert_eq!(run(None, "crate"), (0.0, 0.0), "no mass");
}

#[test]
fn a_riders_carry_stops_at_a_ceiling() {
    // A deck rising under a ceiling carries its rider until the rider's
    // head is the skin under the ceiling, where the carry stops; the cast
    // passes the deck itself. An open carry moves the rider by the deck's
    // move exactly, as the uncast carry did.
    let (mut world, mut schedule) = rig();
    let deck = world.spawn_with(RigidBodyBundle::kinematic(
        Position::new(200.0, 10.0),
        Collider::aabb(Vec2::new(60.0, 10.0)),
    ));
    let ceiling = -2.0 * HALF.y - 12.0;
    static_box(
        &mut world,
        Vec2::new(200.0, ceiling - 10.0),
        Vec2::new(100.0, 10.0),
    );
    let entity = hero(
        &mut world,
        Vec2::new(200.0, 0.0),
        PlatformerController::new(RUN, JUMP),
    );
    steps(&mut world, &mut schedule, 2);
    world.get_mut::<Velocity>(deck).unwrap().0 = Vec2::new(0.0, -60.0);
    let mut highest = f32::INFINITY;
    for _ in 0..20 {
        step(&mut world, &mut schedule);
        highest = highest.min(position(&world, entity).y - HALF.y);
    }
    world.get_mut::<Velocity>(deck).unwrap().0 = Vec2::ZERO;
    let head = position(&world, entity).y - HALF.y;
    assert!(
        (head - (ceiling + SKIN)).abs() < 1e-3 && highest >= ceiling,
        "head at {head}, highest {highest}, the ceiling at {ceiling}"
    );

    let shape = Shape::Aabb { half_extents: HALF };
    let filter = SpatialFilter::new().excluding(entity);
    let from = Vec2::new(200.25, -3.375);
    for delta in [
        Vec2::new(1.5, -0.62),
        Vec2::new(-0.031, 0.0),
        Vec2::new(0.0, 1e-5),
    ] {
        assert_eq!(
            crate::motion::slide::carry(&world, shape, from, delta, &filter, deck),
            from + delta,
            "{delta}"
        );
    }
}

#[test]
fn a_kinematic_body_pushes_the_controller_out() {
    // A deck coming down at 120 px/s, 2 px a step, onto a floating
    // controller and a platform sliding across one standing on a floor: at
    // each step the controller is pushed out of what overlaps it, so its box
    // never lies deeper in theirs than their move in a step plus 1e-3 px.
    let deepest = |deck_velocity: Vec2,
                   deck_at: Vec2,
                   half: Vec2,
                   controller: PlatformerController,
                   floor: bool| {
        let (mut world, mut schedule) = rig();
        if floor {
            tile_floor(&mut world, 64);
        }
        let entity = hero(&mut world, Vec2::new(200.0, 0.0), controller);
        let deck = world.spawn_with(RigidBodyBundle::kinematic(
            Position(deck_at),
            Collider::aabb(half),
        ));
        steps(&mut world, &mut schedule, 2);
        world.get_mut::<Velocity>(deck).unwrap().0 = deck_velocity;
        let mut deepest = 0.0_f32;
        let mut crushed = 0;
        for _ in 0..40 {
            step(&mut world, &mut schedule);
            let (own, theirs) = (position(&world, entity), position(&world, deck));
            let overlap = (HALF + half) - (own - theirs).abs();
            deepest = deepest.max(overlap.min_element());
            crushed += usize::from(state(&world, entity).crushed());
        }
        (deepest, crushed, position(&world, entity))
    };
    let floating = PlatformerController::new(RUN, JUMP).with_gravity(0.0);
    let (depth, crushed, at) = deepest(
        Vec2::new(0.0, 120.0),
        Vec2::new(200.0, -2.0 * HALF.y - 12.0),
        Vec2::new(60.0, 10.0),
        floating,
        false,
    );
    assert!(
        depth <= 2.0 + 1e-3 && crushed == 0,
        "down: {depth} px deep, {crushed} crushed"
    );
    assert!(at.y > 20.0, "pushed down: {at}");
    let (depth, crushed, at) = deepest(
        Vec2::new(-120.0, 0.0),
        Vec2::new(200.0 + HALF.x + 12.0, -HALF.y),
        Vec2::new(10.0, 10.0),
        PlatformerController::new(RUN, JUMP),
        true,
    );
    assert!(
        depth <= 2.0 + 1e-3 && crushed == 0,
        "across: {depth} px deep, {crushed} crushed"
    );
    assert!(at.x < 190.0, "pushed along: {at}");
    // Down on one that stands on a floor, the push is cut short: crushed.
    let (_, crushed, _) = deepest(
        Vec2::new(0.0, 120.0),
        Vec2::new(200.0, -2.0 * HALF.y - 12.0),
        Vec2::new(60.0, 10.0),
        PlatformerController::new(RUN, JUMP),
        true,
    );
    assert!(crushed > 0, "the floor cuts the push short");

    // Another controller, a top-down mover and its own ground push nothing.
    let (mut world, mut schedule) = rig();
    tile_floor(&mut world, 64);
    let entity = hero(
        &mut world,
        Vec2::new(200.0, 0.0),
        PlatformerController::new(RUN, JUMP),
    );
    let other = hero(
        &mut world,
        Vec2::new(205.0, 0.0),
        PlatformerController::new(RUN, JUMP),
    );
    world.spawn_with(
        RigidBodyBundle::kinematic(
            Position::new(195.0, -10.0),
            Collider::aabb(Vec2::splat(8.0)),
        )
        .with((crate::TopDownMover::new(100.0),)),
    );
    steps(&mut world, &mut schedule, 2);
    let (still, also) = (position(&world, entity), position(&world, other));
    steps(&mut world, &mut schedule, 10);
    assert_eq!(
        (position(&world, entity), position(&world, other)),
        (still, also)
    );
    assert!(!state(&world, entity).crushed() && !state(&world, other).crushed());

    // A rider on its ground deck moves as before, bit for bit: its path's
    // hash as the controller gave it before the push-out (bd6145e7).
    let (mut world, mut schedule) = rig();
    let deck = world.spawn_with(RigidBodyBundle::kinematic(
        Position::new(200.0, 10.0),
        Collider::aabb(Vec2::new(60.0, 10.0)),
    ));
    let entity = hero(
        &mut world,
        Vec2::new(200.0, 0.0),
        PlatformerController::new(RUN, JUMP),
    );
    steps(&mut world, &mut schedule, 2);
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for (velocity, count) in [
        (Vec2::new(90.0, -30.0), 30),
        (Vec2::new(-60.0, 45.0), 30),
        (Vec2::new(0.0, -120.0), 20),
    ] {
        world.get_mut::<Velocity>(deck).unwrap().0 = velocity;
        for _ in 0..count {
            step(&mut world, &mut schedule);
            let at = position(&world, entity);
            for word in [at.x.to_bits(), at.y.to_bits()] {
                for byte in word.to_le_bytes() {
                    hash = (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
                }
            }
            assert!(state(&world, entity).grounded());
        }
    }
    assert_eq!(
        hash,
        0xfde1_ce61_8bed_9ac6,
        "the rider's path at {}",
        position(&world, entity)
    );
}
