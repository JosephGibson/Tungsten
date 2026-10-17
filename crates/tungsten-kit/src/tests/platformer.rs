use super::*;
use crate::{KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_REACT, KIT_RESOLVE, KIT_TICK};
use tungsten_core::Stage;
use tungsten_core::physics::{
    PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PhysicsPlugin, RigidBody, RigidBodyBundle,
};
use tungsten_core::{Bundle, LayerKind, TilemapData, TilemapInstance, TilemapRegistry, Time};

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
