//! The fixed loop through the harness (M41): the step dt inside
//! `fixed_update`, one step per edge and per event, interpolated bodies,
//! the step count and summed timings, a paused game, a world without
//! `Time` and the `time` section.

use super::Harness;
use crate::{App, FrameTimings};
use glam::Vec2;
use tungsten_core::physics::PHYSICS_STEP;
use tungsten_core::{
    ActionMap, Binding, Bundle, Collider, CollisionEvent, Config, Entity, EventQueue, InputState,
    KeyCode, PhysicsConfig, Position, PrevPosition, RigidBodyBundle, Stage, Time, Transform,
    Velocity, World, system,
};

const PIN: f32 = 1.0 / 60.0;
const FAST: f32 = 1.0 / 144.0;
const SLOW: f32 = 1.0 / 30.0;
const SPEED: f32 = 600.0;

fn app() -> App {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.world_mut()
        .get_resource_mut::<PhysicsConfig>()
        .unwrap()
        .gravity = Vec2::ZERO;
    app
}

/// A `Vec<T>` resource a test system appends to.
struct Seen<T>(Vec<T>);

fn seen<T: 'static>(harness: &Harness) -> &[T] {
    &harness.world().get_resource::<Seen<T>>().unwrap().0
}

fn push<T: 'static>(world: &mut World, value: T) {
    world.get_resource_mut::<Seen<T>>().unwrap().0.push(value);
}

/// What a `fixed_update` (`true`) or `update` reader saw.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Dts {
    fixed: bool,
    delta: f32,
    delta_time: f32,
}

#[test]
fn fixed_update_reads_the_step_and_update_the_frame_dt() {
    let mut app = app();
    app.world_mut().insert_resource(Seen::<Dts>(Vec::new()));
    for (stage, fixed) in [(Stage::FixedUpdate, true), (Stage::Update, false)] {
        let name = if fixed { "fixed_dts" } else { "update_dts" };
        app.add_system_to(
            stage,
            system(name, move |world: &mut World| {
                let delta = world.get_resource::<Time>().unwrap().delta();
                #[allow(deprecated)]
                let delta_time = world.get_resource::<tungsten_core::DeltaTime>().unwrap().dt;
                push(
                    world,
                    Dts {
                        fixed,
                        delta,
                        delta_time,
                    },
                );
            }),
        );
    }
    let mut harness = Harness::new(app);
    harness.set_dt(FAST);
    harness.step(12);
    let dts = seen::<Dts>(&harness);
    let fixed: Vec<_> = dts.iter().filter(|d| d.fixed).collect();
    let update: Vec<_> = dts.iter().filter(|d| !d.fixed).collect();
    assert_eq!(update.len(), 12);
    assert_eq!(fixed.len(), 4, "12 frames at 1/144 s owe 4 whole steps");
    assert!(fixed.iter().all(|d| d.delta == PIN && d.delta_time == PIN));
    assert!(
        update
            .iter()
            .all(|d| d.delta == FAST && d.delta_time == FAST)
    );
}

/// Per frame: how many fixed steps saw `jump` just pressed through
/// `InputState` and through `ActionMap`, and whether `update` did.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Presses {
    fixed_input: u32,
    fixed_action: u32,
    update: u32,
}

fn jump_key(world: &World) -> KeyCode {
    match world.get_resource::<ActionMap>().unwrap().bindings("jump")[0] {
        Binding::Key { code } => code,
        other => panic!("jump's first binding is not a key: {other:?}"),
    }
}

fn press_counts(dt: f32) -> Vec<Presses> {
    let mut app = app();
    let key = jump_key(app.world_mut());
    app.world_mut().insert_resource(Presses::default());
    app.add_system_to(
        Stage::FixedUpdate,
        system("fixed_jump", move |world: &mut World| {
            let input = world.get_resource::<InputState>().unwrap();
            let by_input = input.just_pressed(key);
            let by_action = world
                .get_resource::<ActionMap>()
                .unwrap()
                .just_pressed(input, "jump");
            let counts = world.get_resource_mut::<Presses>().unwrap();
            counts.fixed_input += u32::from(by_input);
            counts.fixed_action += u32::from(by_action);
        }),
    );
    app.add_system_to(
        Stage::Update,
        system("update_jump", |world: &mut World| {
            let input = world.get_resource::<InputState>().unwrap();
            let pressed = world
                .get_resource::<ActionMap>()
                .unwrap()
                .just_pressed(input, "jump");
            world.get_resource_mut::<Presses>().unwrap().update += u32::from(pressed);
        }),
    );
    let mut harness = Harness::new(app);
    harness.set_dt(dt);
    let mut cycles = Vec::new();
    for _ in 0..100 {
        *harness.world_mut().get_resource_mut::<Presses>().unwrap() = Presses::default();
        harness.press_action("jump");
        harness.step(1);
        harness.release_action("jump");
        // Four frames at 1/144 s hold more than one step, so a step follows
        // the press whichever frame of the pattern it lands in.
        harness.step(3);
        cycles.push(*harness.world().get_resource::<Presses>().unwrap());
    }
    cycles
}

#[test]
fn each_press_reaches_one_fixed_step_and_one_update_frame() {
    for dt in [FAST, SLOW] {
        let cycles = press_counts(dt);
        let once = Presses {
            fixed_input: 1,
            fixed_action: 1,
            update: 1,
        };
        assert!(
            cycles.iter().all(|c| *c == once),
            "at dt {dt}: {:?}",
            cycles.iter().find(|c| **c != once)
        );
    }
}

/// The ids a `fixed_update` (`true`) or `update` reader read in one frame.
#[derive(Debug, Clone, PartialEq)]
struct Read {
    fixed: bool,
    ids: Vec<u32>,
}

fn collision(a: Entity, id: u32) -> CollisionEvent {
    CollisionEvent {
        a,
        b: None,
        normal: Vec2::Y,
        penetration: id as f32,
    }
}

#[test]
fn a_fixed_reader_reads_each_steps_collision_once_and_update_reads_both() {
    let mut app = app();
    app.world_mut().insert_resource(Seen::<Read>(Vec::new()));
    app.world_mut().insert_resource(0_u32);
    let a = app.world_mut().spawn();
    app.add_system_to(
        Stage::FixedUpdate,
        system("send_collision", move |world: &mut World| {
            let id = *world.get_resource::<u32>().unwrap();
            *world.get_resource_mut::<u32>().unwrap() += 1;
            world
                .get_resource_mut::<EventQueue<CollisionEvent>>()
                .unwrap()
                .send(collision(a, id));
        })
        .after(PHYSICS_STEP),
    );
    for (stage, fixed) in [(Stage::FixedUpdate, true), (Stage::Update, false)] {
        let mut desc = system(
            if fixed { "fixed_read" } else { "update_read" },
            move |world: &mut World| {
                let ids = world
                    .get_resource::<EventQueue<CollisionEvent>>()
                    .unwrap()
                    .iter_current()
                    .map(|e| e.penetration as u32)
                    .collect();
                push(world, Read { fixed, ids });
            },
        );
        if fixed {
            desc = desc.after("send_collision");
        }
        app.add_system_to(stage, desc);
    }
    let mut harness = Harness::new(app);
    harness.set_dt(SLOW);
    harness.step(3);
    let read = |fixed, ids: &[u32]| Read {
        fixed,
        ids: ids.to_vec(),
    };
    assert_eq!(
        seen::<Read>(&harness),
        [
            read(true, &[0]),
            read(true, &[1]),
            read(false, &[0, 1]),
            read(true, &[2]),
            read(true, &[3]),
            read(false, &[2, 3]),
            read(true, &[4]),
            read(true, &[5]),
            read(false, &[4, 5]),
        ]
    );
}

fn ids_sent(harness: &Harness) -> u32 {
    *harness.world().get_resource::<u32>().unwrap()
}

fn set_paused(harness: &mut Harness, paused: bool) {
    harness
        .world_mut()
        .get_resource_mut::<Time>()
        .unwrap()
        .set_paused(paused);
}

#[test]
fn at_144_hz_a_fixed_reader_reads_each_update_event_once_and_a_pause_drops_them() {
    let mut app = app();
    app.world_mut().insert_resource(Seen::<u32>(Vec::new()));
    app.world_mut().insert_resource(0_u32);
    let a = app.world_mut().spawn();
    app.add_system_to(
        Stage::Update,
        system("send_collision", move |world: &mut World| {
            let id = *world.get_resource::<u32>().unwrap();
            *world.get_resource_mut::<u32>().unwrap() += 1;
            world
                .get_resource_mut::<EventQueue<CollisionEvent>>()
                .unwrap()
                .send(collision(a, id));
        }),
    );
    app.add_system_to(
        Stage::FixedUpdate,
        system("fixed_read", |world: &mut World| {
            let ids: Vec<u32> = world
                .get_resource::<EventQueue<CollisionEvent>>()
                .unwrap()
                .iter()
                .map(|e| e.penetration as u32)
                .collect();
            for id in ids {
                push(world, id);
            }
        }),
    );
    let mut harness = Harness::new(app);
    harness.set_dt(FAST);
    harness.step(30);
    let read = seen::<u32>(&harness).to_vec();
    assert_eq!(
        read,
        (0..read.len() as u32).collect::<Vec<_>>(),
        "each id once, in order, through the frames with no step"
    );
    // At most the frames since the last step wait for the next.
    assert!(ids_sent(&harness) as usize - read.len() <= 3, "{read:?}");

    set_paused(&mut harness, true);
    harness.step(1);
    let first_paused = ids_sent(&harness) - 1;
    harness.step(4);
    let last_paused = ids_sent(&harness) - 1;
    set_paused(&mut harness, false);
    while timings(&harness).fixed_steps == 0 {
        harness.step(1);
    }
    let after = &seen::<u32>(&harness)[read.len()..];
    assert!(!after.contains(&first_paused), "{after:?}");
    assert_eq!(
        after.first(),
        Some(&last_paused),
        "only the last paused frame's event reaches the next step"
    );
    assert!(after.windows(2).all(|w| w[1] == w[0] + 1), "{after:?}");
}

/// A body moving right at `SPEED`, spawned through the bundle (with
/// history) or as a tuple (without), drawn through a `Transform`.
fn spawn_mover(world: &mut World, history: bool) -> Entity {
    let at = Position::new(100.0, 50.0);
    let collider = Collider::aabb(Vec2::splat(4.0));
    let drawn = (Transform::from_position(at.0),);
    if history {
        world.spawn_with(
            RigidBodyBundle::dynamic(at, collider)
                .with_velocity(Velocity(Vec2::new(SPEED, 0.0)))
                .with(drawn),
        )
    } else {
        world.spawn_with((
            at,
            Velocity(Vec2::new(SPEED, 0.0)),
            collider,
            tungsten_core::RigidBody::dynamic(),
            drawn.0,
        ))
    }
}

fn drawn_x(harness: &Harness, body: Entity) -> f32 {
    harness.world().get::<Transform>(body).unwrap().position.x
}

/// The drawn x moves of `body` over `frames` frames at `dt`, from the frame
/// after the first step.
fn drawn_moves(harness: &mut Harness, body: Entity, frames: usize) -> Vec<f32> {
    while harness.world().get::<Position>(body).unwrap().0.x == 100.0 {
        harness.step(1);
    }
    let mut last = drawn_x(harness, body);
    (0..frames)
        .map(|_| {
            harness.step(1);
            let x = drawn_x(harness, body);
            let moved = x - last;
            last = x;
            moved
        })
        .collect()
}

#[test]
fn an_interpolated_body_moves_every_frame_and_one_without_history_in_steps() {
    let per_frame = SPEED * FAST;
    let per_step = SPEED * PIN;
    for (history, interpolate) in [(true, true), (false, true), (true, false)] {
        let mut app = app();
        let body = spawn_mover(app.world_mut(), history);
        app.world_mut()
            .get_resource_mut::<Time>()
            .unwrap()
            .set_interpolate(interpolate);
        let mut harness = Harness::new(app);
        harness.set_dt(FAST);
        let moves = drawn_moves(&mut harness, body, 40);
        if history && interpolate {
            assert!(
                moves.iter().all(|m| (m - per_frame).abs() <= 1e-3),
                "{moves:?}"
            );
        } else {
            assert!(
                moves
                    .iter()
                    .all(|m| m.abs() <= 1e-3 || (m - per_step).abs() <= 1e-3),
                "history {history}, interpolate {interpolate}: {moves:?}"
            );
            assert!(moves.iter().any(|m| m.abs() <= 1e-3));
        }
    }
}

#[test]
fn at_the_pin_a_body_with_history_draws_its_previous_steps_position() {
    let mut app = app();
    let body = spawn_mover(app.world_mut(), true);
    let mut harness = Harness::new(app);
    for _ in 0..10 {
        let before = harness.world().get::<Position>(body).unwrap().0;
        harness.step(1);
        assert_eq!(harness.world().get_resource::<Time>().unwrap().alpha(), 0.0);
        assert_eq!(
            harness.world().get::<Transform>(body).unwrap().position,
            before
        );
        assert_ne!(harness.world().get::<Position>(body).unwrap().0, before);
    }
}

#[test]
fn a_teleport_writing_both_draws_the_new_point_that_frame() {
    let mut app = app();
    let body = spawn_mover(app.world_mut(), true);
    let target = Vec2::new(-400.0, 300.0);
    app.world_mut().insert_resource(false);
    app.add_system_to(
        Stage::Update,
        system("teleport", move |world: &mut World| {
            if !*world.get_resource::<bool>().unwrap() {
                return;
            }
            world.get_mut::<Position>(body).unwrap().0 = target;
            world.get_mut::<PrevPosition>(body).unwrap().0 = target;
        }),
    );
    let mut harness = Harness::new(app);
    harness.set_dt(FAST);
    harness.step(7);
    assert!(harness.world().get_resource::<Time>().unwrap().alpha() > 0.0);
    *harness.world_mut().get_resource_mut::<bool>().unwrap() = true;
    harness.step(1);
    assert_eq!(
        harness.world().get::<Transform>(body).unwrap().position,
        target
    );
}

fn timings(harness: &Harness) -> &FrameTimings {
    harness.world().get_resource::<FrameTimings>().unwrap()
}

fn step_entries(timings: &FrameTimings) -> Vec<f32> {
    timings
        .system_timings
        .iter()
        .filter(|(name, _)| name == PHYSICS_STEP)
        .map(|(_, ms)| *ms)
        .collect()
}

#[test]
fn a_two_step_frame_counts_two_steps_and_lists_the_step_once() {
    let mut harness = Harness::new(app());
    harness.set_dt(SLOW);
    harness.step(3);
    assert_eq!(timings(&harness).fixed_steps, 2);
    assert_eq!(step_entries(timings(&harness)).len(), 1);
}

#[test]
fn a_frame_with_no_step_lists_the_step_at_zero() {
    let mut harness = Harness::new(app());
    harness.set_dt(FAST);
    harness.step(1);
    assert_eq!(timings(&harness).fixed_steps, 0);
    assert_eq!(step_entries(timings(&harness)), [0.0]);
}

#[test]
fn a_paused_game_runs_no_step_and_its_bodies_hold() {
    let mut app = app();
    let body = spawn_mover(app.world_mut(), true);
    let mut harness = Harness::new(app);
    harness.step(3);
    harness
        .world_mut()
        .get_resource_mut::<Time>()
        .unwrap()
        .pause();
    let at = harness.world().get::<Position>(body).unwrap().0;
    let drawn = harness.world().get::<Transform>(body).unwrap().position;
    for _ in 0..5 {
        harness.step(1);
        assert_eq!(timings(&harness).fixed_steps, 0);
        assert_eq!(harness.world().get::<Position>(body).unwrap().0, at);
        assert_eq!(
            harness.world().get::<Transform>(body).unwrap().position,
            drawn
        );
    }
}

#[test]
fn a_world_without_time_runs_fixed_update_once_a_frame() {
    let mut app = app();
    app.world_mut().insert_resource(0_u32);
    app.add_system_to(
        Stage::FixedUpdate,
        system("count_runs", |world: &mut World| {
            *world.get_resource_mut::<u32>().unwrap() += 1;
        }),
    );
    app.world_mut().remove_resource::<Time>();
    let mut harness = Harness::new(app);
    harness.set_dt(SLOW);
    harness.step(4);
    assert_eq!(*harness.world().get_resource::<u32>().unwrap(), 4);
    assert_eq!(timings(&harness).fixed_steps, 1);
}

#[test]
fn app_new_applies_the_time_section_and_refuses_a_step_under_30_hz() {
    let mut config = Config::default();
    config.time.fixed_step_hz = 120;
    config.time.max_steps_per_frame = 3;
    config.time.interpolate = false;
    let mut app = App::new(config).unwrap();
    let time = app.world_mut().get_resource::<Time>().unwrap();
    assert_eq!(time.fixed_step(), 1.0_f32 / 120.0);
    assert_eq!(time.max_steps_per_frame(), 3);
    assert!(!time.interpolate());

    let mut config = Config::default();
    config.time.fixed_step_hz = 20;
    let err = App::new(config).err().expect("20 Hz is refused");
    assert!(err.to_string().contains("time.fixed_step_hz"), "{err}");
}
