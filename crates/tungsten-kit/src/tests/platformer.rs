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
    let half_height = match collider.shape {
        tungsten_core::physics::Shape::Circle { radius } => radius,
        tungsten_core::physics::Shape::Aabb { half_extents } => half_extents.y,
    };
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
