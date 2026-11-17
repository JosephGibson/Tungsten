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

/// A rig with the oscillator built before the path follower when `swapped`,
/// so the fold is checked with the two systems in either order.
fn rig_with_oscillator(swapped: bool) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    if swapped {
        crate::OscillatorPlugin.build(&mut schedule, &mut world);
        PathFollowerPlugin.build(&mut schedule, &mut world);
    } else {
        PathFollowerPlugin.build(&mut schedule, &mut world);
        crate::OscillatorPlugin.build(&mut schedule, &mut world);
    }
    schedule.resolve().unwrap();
    (world, schedule)
}

#[test]
fn an_oscillating_platform_rides_the_path_with_the_whole_wave_in_either_order() {
    for swapped in [false, true] {
        let (mut world, mut schedule) = rig_with_oscillator(swapped);
        let follower = PathFollower::new([Vec2::ZERO, Vec2::new(120.0, 0.0)], 60.0).ping_pong();
        let lift = world.spawn_with(
            RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(30.0, 5.0)))
                .with((
                    follower,
                    crate::Oscillator::position(Vec2::new(0.0, 10.0), 1.0),
                )),
        );
        for n in 1..=240 {
            step(&mut world, &mut schedule);
            let t = n as f32 * DT;
            // Two seconds each way along 120 px, and the bob on top.
            let leg = t % 4.0;
            let x = if leg <= 2.0 {
                60.0 * leg
            } else {
                120.0 - 60.0 * (leg - 2.0)
            };
            let y = 10.0 * (std::f32::consts::TAU * t).sin();
            let at = position(&world, lift);
            assert!(
                (at - Vec2::new(x, y)).length() < 1e-2,
                "{swapped} {n}: {at} vs ({x}, {y})"
            );
        }
    }
}

#[test]
fn a_stopped_follower_keeps_its_wave_and_one_without_stops_still() {
    let (mut world, mut schedule) = rig_with_oscillator(false);
    let bobbing = world.spawn_with(
        RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(30.0, 5.0)))
            .with((
                PathFollower::new([Vec2::ZERO, Vec2::new(6.0, 0.0)], 60.0),
                crate::Oscillator::position(Vec2::new(0.0, 4.0), 1.0),
            )),
    );
    let still = platform(
        &mut world,
        PathFollower::new([Vec2::ZERO, Vec2::new(6.0, 0.0)], 60.0),
    );
    for _ in 0..75 {
        step(&mut world, &mut schedule);
    }
    // Past its end at 0.1 s: at 1.25 s the bob is at its peak.
    assert!((position(&world, bobbing) - Vec2::new(6.0, 4.0)).length() < 1e-2);
    assert_eq!(world.get::<Velocity>(still).unwrap().0, Vec2::ZERO);
    assert!((position(&world, still) - Vec2::new(6.0, 0.0)).length() < 1e-4);
}

#[test]
fn a_save_mid_cycle_continues_exactly() {
    for follower in [
        PathFollower::new(square(), 90.0).looping(),
        PathFollower::new(square(), 90.0)
            .ping_pong()
            .with_easing(Easing::SineInOut),
    ] {
        let (mut world, mut schedule) = rig();
        let lift = platform(&mut world, follower);
        for _ in 0..100 {
            step(&mut world, &mut schedule);
        }
        let saved = serde_json::to_value(world.get::<PathFollower>(lift).unwrap()).unwrap();
        let loaded: PathFollower = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(&loaded, world.get::<PathFollower>(lift).unwrap());
        assert_eq!(serde_json::to_value(&loaded).unwrap(), saved);
        let (mut twin, mut twin_schedule) = rig();
        let at = position(&world, lift);
        let copy = twin.spawn_with(
            RigidBodyBundle::kinematic(Position(at), Collider::aabb(Vec2::new(30.0, 5.0)))
                .with((loaded,)),
        );
        for n in 0..240 {
            step(&mut world, &mut schedule);
            step(&mut twin, &mut twin_schedule);
            assert_eq!(
                twin.get::<PathFollower>(copy),
                world.get::<PathFollower>(lift),
                "{saved}: step {n}"
            );
            assert_eq!(position(&twin, copy), position(&world, lift), "step {n}");
        }
    }
}

#[test]
fn a_save_outside_its_path_is_refused() {
    let saved = serde_json::to_value(PathFollower::new(square(), 60.0)).unwrap();
    let with = |field: &str, value: serde_json::Value| {
        let mut save = saved.clone();
        save[field] = value;
        serde_json::from_value::<PathFollower>(save)
    };
    // A stopping square of three 60 px sides at 60 px/s: 3 s in all.
    assert!(with("time", serde_json::json!(3.0)).is_ok(), "its end");
    for (field, value) in [
        ("time", serde_json::json!(3.5)),
        ("time", serde_json::json!(-0.1)),
        ("speed", serde_json::json!(0.0)),
        ("points", serde_json::json!([[0, 0]])),
        ("points", serde_json::json!([[0, 0], [0, 0]])),
        ("mode", serde_json::json!("bounce")),
        ("extra", serde_json::json!(1)),
    ] {
        assert!(with(field, value.clone()).is_err(), "{field} = {value}");
    }
    let looping = serde_json::to_value(PathFollower::new(square(), 60.0).looping()).unwrap();
    let mut at_cycle = looping.clone();
    at_cycle["time"] = serde_json::json!(4.0);
    assert!(
        serde_json::from_value::<PathFollower>(at_cycle).is_err(),
        "a loop's time is below one cycle"
    );
}

/// Steps until `done` holds, at most `limit` steps; the steps taken.
fn steps_until(
    world: &mut World,
    schedule: &mut Schedule,
    limit: u32,
    done: impl Fn(&World) -> bool,
) -> u32 {
    for taken in 1..=limit {
        step(world, schedule);
        if done(world) {
            return taken;
        }
    }
    panic!("not within {limit} steps");
}

#[test]
fn a_waiting_ping_pong_holds_at_each_end() {
    let (mut world, mut schedule) = rig();
    let points = [Vec2::ZERO, Vec2::new(60.0, 0.0)];
    let guard = platform(
        &mut world,
        PathFollower::new(points, 60.0).ping_pong().with_wait(0.5),
    );
    // Its first 0.5 s at its first point, then a second along its leg.
    for _ in 0..30 {
        step(&mut world, &mut schedule);
        assert_eq!(position(&world, guard), Vec2::ZERO, "holding at the start");
    }
    let out = steps_until(&mut world, &mut schedule, 120, |world| {
        position(world, guard).x >= 60.0 - 1e-3
    });
    assert!((59..=61).contains(&out), "a second out: {out}");
    // Held 0.5 s at its last point, the leftover of the arrival dropped,
    // then back.
    let mut held = 0;
    loop {
        step(&mut world, &mut schedule);
        if (position(&world, guard).x - 60.0).abs() >= 1e-3 {
            break;
        }
        held += 1;
        assert!(held < 60, "it comes back");
    }
    assert!(
        (30..=31).contains(&held),
        "half a second at the end: {held}"
    );
    // Refused: a wait on a stop path, a rider beside a wait.
    assert!(std::panic::catch_unwind(|| PathFollower::new(points, 60.0).with_wait(1.0)).is_err());
    assert!(
        std::panic::catch_unwind(|| PathFollower::new(points, 60.0)
            .ping_pong()
            .with_wait(1.0)
            .with_rider(0.3, 4.0))
        .is_err()
    );
}

/// The lift up from (0, 0) to (0, -60) at 60 px/s, boarding after 0.3 s and
/// coming back 1 s after its last rider.
fn lift() -> PathFollower {
    PathFollower::new([Vec2::ZERO, Vec2::new(0.0, -60.0)], 60.0)
        .ping_pong()
        .with_rider(0.3, 1.0)
}

/// One step, its rider standing on the lift after its move when `riding`,
/// as a controller marks its ground.
fn ride(world: &mut World, schedule: &mut Schedule, lift: Entity, riding: bool) {
    step(world, schedule);
    if riding {
        world.get_mut::<PathFollower>(lift).unwrap().mark_ridden();
    }
}

#[test]
fn a_rider_boards_rides_and_the_lift_comes_back() {
    let (mut world, mut schedule) = rig();
    let lift = platform(&mut world, lift());
    let y = |world: &World| position(world, lift).y;
    for _ in 0..120 {
        ride(&mut world, &mut schedule, lift, false);
    }
    assert_eq!(y(&world), 0.0, "no rider, it waits");
    // A rider: 0.3 s aboard, read a step late, then up.
    let mut boarding = 0;
    while y(&world) == 0.0 {
        ride(&mut world, &mut schedule, lift, true);
        boarding += 1;
        assert!(boarding < 40, "it boards");
    }
    assert_eq!(
        boarding, 20,
        "18 steps aboard after the mark's step, then it goes"
    );
    for _ in 0..70 {
        ride(&mut world, &mut schedule, lift, true);
    }
    assert!((y(&world) + 60.0).abs() < 1e-3, "up, and held while ridden");
    // The rider steps off: 1 s later it comes back.
    for _ in 0..60 {
        ride(&mut world, &mut schedule, lift, false);
        assert!(
            (y(&world) + 60.0).abs() < 1e-3,
            "held a second after its rider"
        );
    }
    let back = steps_until(&mut world, &mut schedule, 120, |world| {
        position(world, lift).y >= -1e-3
    });
    assert!((60..=63).contains(&back), "a second down: {back}");
    for _ in 0..60 {
        ride(&mut world, &mut schedule, lift, false);
        assert!(y(&world).abs() < 1e-3, "waiting for the next");
    }
    // A plain follower ignores the mark.
    let mut plain = PathFollower::new([Vec2::ZERO, Vec2::X], 1.0).ping_pong();
    plain.mark_ridden();
    assert!(plain.hold.is_none(), "a plain follower holds nothing");
    assert_eq!(
        std::mem::size_of::<PathFollower>(),
        40,
        "the size a game's capture of its platforms copies"
    );
}

#[test]
fn a_hold_round_trips_and_a_plain_follower_keeps_its_bytes() {
    let plain = PathFollower::new([Vec2::ZERO, Vec2::new(60.0, 0.0)], 60.0).ping_pong();
    let saved = serde_json::to_value(&plain).unwrap();
    for key in ["wait", "rider", "hold", "ridden"] {
        assert!(saved.get(key).is_none(), "a plain follower writes no {key}");
    }
    let (mut world, mut schedule) = rig();
    let lift = platform(&mut world, lift());
    for _ in 0..10 {
        ride(&mut world, &mut schedule, lift, true);
    }
    let boarding = world.get::<PathFollower>(lift).unwrap().clone();
    let hold = boarding.hold.as_ref().unwrap();
    assert!(hold.held > 0.0 && hold.ridden);
    let saved = serde_json::to_value(&boarding).unwrap();
    let board = saved["rider"]["board"].as_f64().unwrap();
    assert!((board - 0.3).abs() < 1e-6 && saved["rider"]["back"] == 1.0);
    assert!(saved["hold"].as_f64().unwrap() > 0.0 && saved["ridden"] == true);
    let loaded: PathFollower = serde_json::from_value(saved).unwrap();
    assert_eq!(loaded, boarding);
    let refused = serde_json::json!({
        "points": [[0, 0], [60, 0]], "speed": 60, "mode": "stop", "easing": "linear",
        "time": 0, "wait": 1
    });
    assert!(serde_json::from_value::<PathFollower>(refused).is_err());
}

#[test]
fn a_loaded_lift_boards_and_returns_as_the_uninterrupted_one() {
    // The rider stands on from step 5 to step 200; a save at each step
    // between 5 and 40, restored into a fresh World, runs on as the
    // uninterrupted lift does.
    let riding = |at: u32| (5..200).contains(&at);
    let (mut kept, mut kept_schedule) = rig();
    let kept_lift = platform(&mut kept, lift());
    let mut heights = Vec::new();
    for at in 0..300 {
        ride(&mut kept, &mut kept_schedule, kept_lift, riding(at));
        heights.push(position(&kept, kept_lift).y);
    }
    for save_at in [6, 12, 20, 24] {
        let (mut world, mut schedule) = rig();
        let lift = platform(&mut world, lift());
        for at in 0..save_at {
            ride(&mut world, &mut schedule, lift, riding(at));
        }
        let saved = serde_json::to_value(world.get::<PathFollower>(lift).unwrap()).unwrap();
        let at_save = position(&world, lift);
        let (mut loaded, mut loaded_schedule) = rig();
        let follower: PathFollower = serde_json::from_value(saved).unwrap();
        let restored = platform(&mut loaded, follower);
        loaded.get_mut::<Position>(restored).unwrap().0 = at_save;
        for at in save_at..300 {
            ride(&mut loaded, &mut loaded_schedule, restored, riding(at));
            let height = position(&loaded, restored).y;
            assert!(
                (height - heights[at as usize]).abs() < 1e-3,
                "saved at {save_at}, step {at}: {height} against {}",
                heights[at as usize]
            );
        }
    }
}

/// A rider's script for the lift, a mark or none after each step: aboard
/// and carried up, to stand 3 s and more at the top; off for a step and on
/// again, down, to stand a second and more at the bottom; off and on, up;
/// off 59 steps, short of the lift's second alone; a boarding broken by a
/// step off and begun again; then off for good.
fn script() -> Vec<bool> {
    [
        (true, 270),
        (false, 1),
        (true, 160),
        (false, 1),
        (true, 100),
        (false, 59),
        (true, 10),
        (false, 1),
        (true, 10),
        (false, 200),
    ]
    .into_iter()
    .flat_map(|(riding, steps)| std::iter::repeat_n(riding, steps))
    .collect()
}

/// `follower`'s heights over [`script`], after each step.
fn script_heights(follower: PathFollower) -> Vec<f32> {
    let (mut world, mut schedule) = rig();
    let lift = platform(&mut world, follower);
    script()
        .into_iter()
        .map(|riding| {
            ride(&mut world, &mut schedule, lift, riding);
            position(&world, lift).y
        })
        .collect()
}

#[test]
fn a_two_way_lift_takes_a_fresh_rider_back_and_holds_for_the_one_it_carried() {
    const TOP: f32 = -60.0;
    // The first step after `from` at which the lift has left `end`.
    let leaves = |heights: &[f32], from: usize, end: f32| {
        (from..heights.len())
            .find(|&at| (heights[at] - end).abs() > 1e-3)
            .expect("it leaves")
    };
    let stands = |heights: &[f32], steps: std::ops::Range<usize>, end: f32| {
        steps.clone().all(|at| (heights[at] - end).abs() < 1e-3)
    };
    let two_way = script_heights(lift().two_way());
    // Aboard 0.3 s, read a step late, then up, a second.
    assert_eq!(leaves(&two_way, 0, 0.0), 19, "it boards as a one-way lift");
    // Carried up, it holds under its rider: 3 s and more.
    assert!(stands(&two_way, 85..271, TOP), "held under its rider");
    // One step off, then 0.3 s of marks: it leaves, and carries the rider
    // down, where it holds again, a second and more.
    let down = leaves(&two_way, 271, TOP);
    assert!(
        (289..=291).contains(&down),
        "a fresh rider's boarding: {down}"
    );
    assert!(stands(&two_way, 356..432, 0.0), "held at the bottom");
    // Off and on, it goes up.
    let up = leaves(&two_way, 432, 0.0);
    assert!((450..=452).contains(&up), "a fresh rider's boarding: {up}");
    // Alone 59 steps, short of its second; then a boarding broken by a step
    // off, which starts over: 20 marks either side of it never add up.
    // Alone for good, it comes back a second later.
    assert!(
        stands(&two_way, 517..672, TOP),
        "alone, then a boarding broken"
    );
    let back = leaves(&two_way, 612, TOP);
    assert!((672..=674).contains(&back), "a second alone: {back}");
    assert!(stands(&two_way, 740..812, 0.0), "back down, waiting");

    // The same script on a one-way lift: it never leaves its last point
    // under a rider, and comes back a second after the last one left.
    let one_way = script_heights(lift());
    assert_eq!(leaves(&one_way, 0, 0.0), 19);
    assert!(stands(&one_way, 85..672, TOP), "held while ridden");
    let back = leaves(&one_way, 85, TOP);
    assert!((672..=674).contains(&back), "a second alone: {back}");
    assert!(stands(&one_way, 740..812, 0.0), "back down, waiting");
}

#[test]
fn a_two_way_lift_saved_at_any_step_resumes_as_the_uninterrupted_one() {
    let script = script();
    let (mut kept, mut kept_schedule) = rig();
    let kept_lift = platform(&mut kept, lift().two_way());
    // A twin whose follower is saved and read back before every step.
    let (mut twin, mut twin_schedule) = rig();
    let twin_lift = platform(&mut twin, lift().two_way());
    let (mut heights, mut saves) = (Vec::new(), Vec::new());
    let mut flags = [false; 2];
    for (at, &riding) in script.iter().enumerate() {
        let follower = kept.get::<PathFollower>(kept_lift).unwrap().clone();
        let saved = serde_json::to_value(&follower).unwrap();
        let loaded: PathFollower = serde_json::from_value(saved.clone()).unwrap();
        assert_eq!(loaded, follower, "step {at}: read back, it is itself");
        assert_eq!(serde_json::to_value(&loaded).unwrap(), saved, "step {at}");
        flags[0] |= saved.get("carried").is_some();
        flags[1] |= saved.get("boarding").is_some();
        saves.push((saved, position(&kept, kept_lift)));
        let own = twin.get::<PathFollower>(twin_lift).unwrap();
        let own: PathFollower = serde_json::from_value(serde_json::to_value(own).unwrap()).unwrap();
        *twin.get_mut::<PathFollower>(twin_lift).unwrap() = own;
        ride(&mut kept, &mut kept_schedule, kept_lift, riding);
        ride(&mut twin, &mut twin_schedule, twin_lift, riding);
        assert_eq!(
            twin.get::<PathFollower>(twin_lift),
            kept.get::<PathFollower>(kept_lift),
            "step {at}"
        );
        assert_eq!(
            position(&twin, twin_lift),
            position(&kept, kept_lift),
            "step {at}"
        );
        heights.push(position(&kept, kept_lift).y);
    }
    assert_eq!(
        flags, [true; 2],
        "a save held its carried rider and a boarding"
    );
    // Every seventh save, restored into a fresh World, runs on as the
    // uninterrupted lift does.
    for save_at in (0..script.len()).step_by(7) {
        let (saved, at_save) = &saves[save_at];
        let (mut loaded, mut loaded_schedule) = rig();
        let follower: PathFollower = serde_json::from_value(saved.clone()).unwrap();
        let restored = platform(&mut loaded, follower);
        loaded.get_mut::<Position>(restored).unwrap().0 = *at_save;
        for at in save_at..script.len() {
            ride(&mut loaded, &mut loaded_schedule, restored, script[at]);
            let height = position(&loaded, restored).y;
            assert!(
                (height - heights[at]).abs() < 1e-3,
                "saved at {save_at}, step {at}: {height} against {}",
                heights[at]
            );
        }
    }

    // A one-way lift over the same script and a plain follower write the
    // keys they wrote.
    let (mut world, mut schedule) = rig();
    let one_way = platform(&mut world, lift());
    let plain = platform(
        &mut world,
        PathFollower::new([Vec2::new(100.0, 0.0), Vec2::new(160.0, 0.0)], 60.0).ping_pong(),
    );
    let keys = |world: &World, entity: Entity| -> (Vec<String>, serde_json::Value) {
        let saved = serde_json::to_value(world.get::<PathFollower>(entity).unwrap()).unwrap();
        let mut keys: Vec<String> = saved.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        (keys, saved)
    };
    for (at, &riding) in script.iter().enumerate() {
        ride(&mut world, &mut schedule, one_way, riding);
        let (lift_keys, saved) = keys(&world, one_way);
        assert!(
            lift_keys.iter().all(|key| [
                "points", "speed", "mode", "easing", "time", "rider", "hold", "ridden"
            ]
            .contains(&key.as_str())),
            "step {at}: {lift_keys:?}"
        );
        let mut rule: Vec<&String> = saved["rider"].as_object().unwrap().keys().collect();
        rule.sort();
        assert_eq!(rule, ["back", "board"], "step {at}");
        assert_eq!(
            keys(&world, plain).0,
            ["easing", "mode", "points", "speed", "time"],
            "step {at}"
        );
    }
}

#[test]
fn a_filter_and_a_ride_back_need_a_rider_rule() {
    let points = [Vec2::ZERO, Vec2::new(0.0, -60.0)];
    // In code: neither without a rider rule, a waiting follower among them.
    assert!(
        std::panic::catch_unwind(|| PathFollower::new(points, 60.0)
            .ping_pong()
            .with_rider_filter(crate::TagFilter::new().any(["team.player"])))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| PathFollower::new(points, 60.0).ping_pong().two_way()).is_err()
    );
    assert!(
        std::panic::catch_unwind(|| PathFollower::new(points, 60.0)
            .ping_pong()
            .with_wait(1.0)
            .two_way())
        .is_err()
    );
    // In data they are the rule's fields: beside it they are unknown, and
    // the rule checks them.
    let scene = |extra: serde_json::Value| {
        let mut fields = serde_json::json!({
            "points": [[0, 0], [0, -60]], "speed": 60, "mode": "ping_pong"
        });
        for (key, value) in extra.as_object().unwrap() {
            fields[key] = value.clone();
        }
        serde_json::from_value::<PathFollowerDef>(fields).map(|def| def.rider)
    };
    let rule = scene(serde_json::json!({
        "rider": { "filter": { "any": ["team.player"] }, "two_way": true }
    }))
    .expect("a filtered two-way rule")
    .expect("its rule");
    let built = lift()
        .with_rider_filter(crate::TagFilter::new().any(["team.player"]))
        .two_way();
    assert_eq!(
        (rule.filter.clone(), rule.two_way),
        (built.hold_rule().1.unwrap().filter.clone(), true)
    );
    assert!(!built.takes(None) && built.takes(Some(&crate::Tags::new(["team.player"]))));
    assert!(lift().takes(None), "no filter takes every controller");
    assert!(!PathFollower::new(points, 60.0).ping_pong().takes(None));
    for refused in [
        serde_json::json!({ "filter": { "any": ["team.player"] } }),
        serde_json::json!({ "two_way": true }),
        serde_json::json!({ "rider": { "two_way": "yes" } }),
        serde_json::json!({ "rider": { "filter": { "any": ["Team Player"] } } }),
        serde_json::json!({ "rider": { "filter": { "some": ["team.player"] } } }),
    ] {
        assert!(scene(refused.clone()).is_err(), "{refused}");
    }
    // A save holds the rule's fields, and a two-way lift's state on no
    // other follower.
    let saved = serde_json::to_value(&built).unwrap();
    assert_eq!(saved["rider"]["two_way"], true);
    assert_eq!(
        saved["rider"]["filter"],
        serde_json::json!({ "any": ["team.player"] })
    );
    assert_eq!(
        serde_json::from_value::<PathFollower>(saved.clone()).unwrap(),
        built
    );
    for key in ["carried", "boarding"] {
        let mut held = saved.clone();
        held[key] = serde_json::json!(true);
        assert!(
            serde_json::from_value::<PathFollower>(held).is_ok(),
            "a two-way lift's {key}"
        );
        for other in [
            lift(),
            PathFollower::new(points, 60.0).ping_pong(),
            PathFollower::new(points, 60.0).ping_pong().with_wait(1.0),
        ] {
            let mut save = serde_json::to_value(&other).unwrap();
            save[key] = serde_json::json!(true);
            assert!(
                serde_json::from_value::<PathFollower>(save).is_err(),
                "{key} on a follower that is no two-way lift"
            );
        }
    }
}

/// The kit cues the followers of a [`cue_rig`] sent, by entity and event,
/// as the cue system finds them each step.
#[derive(Default)]
struct SeenCues(Vec<(Entity, &'static str)>);

/// [`rig`] with the cue system, and a system before it that lists the
/// step's kit cues in [`SeenCues`].
fn cue_rig() -> (World, Schedule) {
    use crate::react::cues::{CUES, CuesPlugin, KitCues};
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(tungsten_core::CommandBuffer::new());
    world.insert_resource(SeenCues::default());
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    PathFollowerPlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    schedule.add(
        Phase::React.stage(),
        Phase::React
            .join(system("seen", |world: &mut World| {
                let cues: Vec<(Entity, &'static str)> = world
                    .resource::<KitCues>()
                    .0
                    .iter()
                    .map(|cue| (cue.entity, cue.event))
                    .collect();
                world.resource_mut::<SeenCues>().0.extend(cues);
            }))
            .before(CUES),
    );
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One step of a [`cue_rig`], inside a fixed step as the app runs it.
fn cue_step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(DT);
    world.resource_mut::<Time>().enter_fixed_step();
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    world.resource_mut::<Time>().leave_fixed_steps();
}

/// Cues naming a follower's two events, so it sends them.
fn naming_its_ends() -> crate::Cues {
    crate::Cues::new()
        .on(
            ARRIVED,
            [crate::CueAction::counter_add("arrivals", 1).for_game()],
        )
        .on(
            DEPARTED,
            [crate::CueAction::counter_add("departures", 1).for_game()],
        )
}

#[test]
fn a_paused_follower_stands_and_counts_nothing() {
    // Mid-leg: its time and its body's point hold and its velocity is zero
    // for 30 steps, with no cue; then it goes on from there.
    let (mut world, mut schedule) = cue_rig();
    let path = PathFollower::new([Vec2::ZERO, Vec2::new(60.0, 0.0)], 60.0).ping_pong();
    let walker = platform(&mut world, path.clone());
    world.insert(walker, naming_its_ends());
    let mut reference = path;
    for _ in 0..20 {
        cue_step(&mut world, &mut schedule);
        reference.advance(DT);
    }
    assert_eq!(
        world.resource::<SeenCues>().0,
        [(walker, DEPARTED)],
        "out of its first point"
    );
    let (time, at) = (
        world.get::<PathFollower>(walker).unwrap().time,
        position(&world, walker),
    );
    world
        .get_mut::<PathFollower>(walker)
        .unwrap()
        .set_paused(true);
    assert!(world.get::<PathFollower>(walker).unwrap().is_paused());
    for n in 0..30 {
        cue_step(&mut world, &mut schedule);
        assert_eq!(
            world.get::<PathFollower>(walker).unwrap().time,
            time,
            "step {n}"
        );
        assert_eq!(position(&world, walker), at, "step {n}");
        assert_eq!(
            world.get::<Velocity>(walker).unwrap().0,
            Vec2::ZERO,
            "step {n}"
        );
    }
    assert_eq!(
        world.resource::<SeenCues>().0.len(),
        1,
        "no cue while paused"
    );
    // Saved paused, it loads paused; let go, it writes no pause.
    let saved = serde_json::to_value(world.get::<PathFollower>(walker).unwrap()).unwrap();
    assert_eq!(saved["paused"], true);
    let loaded: PathFollower = serde_json::from_value(saved).unwrap();
    assert!(loaded.is_paused());
    assert_eq!(&loaded, world.get::<PathFollower>(walker).unwrap());
    world
        .get_mut::<PathFollower>(walker)
        .unwrap()
        .set_paused(false);
    let saved = serde_json::to_value(world.get::<PathFollower>(walker).unwrap()).unwrap();
    assert!(saved.get("paused").is_none() && saved.get("turned").is_none());
    for n in 0..30 {
        cue_step(&mut world, &mut schedule);
        reference.advance(DT);
        let at = position(&world, walker);
        assert!(
            (at - reference.point()).length() < 0.01,
            "step {n}: on from where it stood, {at}"
        );
    }

    // A lift paused at its first point counts no boarding under its rider's
    // marks, and leaves 0.3 s after it resumes, the mark it left unread its
    // first step's.
    let (mut world, mut schedule) = rig();
    let waiting = platform(&mut world, lift());
    world
        .get_mut::<PathFollower>(waiting)
        .unwrap()
        .set_paused(true);
    for _ in 0..60 {
        ride(&mut world, &mut schedule, waiting, true);
        let hold = world.get::<PathFollower>(waiting).unwrap().hold.clone();
        let hold = hold.expect("a lift's hold");
        assert_eq!(hold.held, 0.0, "no boarding counted");
        assert!(hold.ridden, "its rider's mark left unread");
        assert_eq!(position(&world, waiting).y, 0.0);
    }
    world
        .get_mut::<PathFollower>(waiting)
        .unwrap()
        .set_paused(false);
    let mut boarding = 0;
    while position(&world, waiting).y == 0.0 {
        ride(&mut world, &mut schedule, waiting, true);
        boarding += 1;
        assert!(boarding < 40, "it boards");
    }
    assert_eq!(boarding, 19, "0.3 s from its resume, the unread mark first");

    // With a position oscillator on the body, in either system order: the
    // paused body rides its wave about the held point, never farther from
    // it than the wave's amplitude, and resumes the path with the whole
    // wave.
    for swapped in [false, true] {
        let (mut world, mut schedule) = rig_with_oscillator(swapped);
        let follower = PathFollower::new([Vec2::ZERO, Vec2::new(120.0, 0.0)], 60.0).ping_pong();
        let bobbing = world.spawn_with(
            RigidBodyBundle::kinematic(Position(Vec2::ZERO), Collider::aabb(Vec2::new(30.0, 5.0)))
                .with((
                    follower,
                    crate::Oscillator::position(Vec2::new(0.0, 10.0), 1.0),
                )),
        );
        for _ in 0..45 {
            step(&mut world, &mut schedule);
        }
        world
            .get_mut::<PathFollower>(bobbing)
            .unwrap()
            .set_paused(true);
        let held = Vec2::new(45.0, 0.0);
        let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
        for n in 46..=165 {
            step(&mut world, &mut schedule);
            let wave = 10.0 * (std::f32::consts::TAU * n as f32 * DT).sin();
            let at = position(&world, bobbing);
            assert!(
                (at - (held + Vec2::new(0.0, wave))).length() < 1e-2,
                "{swapped} {n}: {at} about {held}"
            );
            assert!((at - held).length() <= 10.0 + 1e-2, "{swapped} {n}: {at}");
            (low, high) = (low.min(at.y), high.max(at.y));
        }
        assert!(
            low < -9.9 && high > 9.9,
            "{swapped}: the whole wave, {low} to {high}"
        );
        world
            .get_mut::<PathFollower>(bobbing)
            .unwrap()
            .set_paused(false);
        for n in 166..=225 {
            step(&mut world, &mut schedule);
            let x = 45.0 + 60.0 * (n - 165) as f32 * DT;
            let y = 10.0 * (std::f32::consts::TAU * n as f32 * DT).sin();
            let at = position(&world, bobbing);
            assert!(
                (at - Vec2::new(x, y)).length() < 1e-2,
                "{swapped} {n}: {at} vs ({x}, {y})"
            );
        }
    }
}

#[test]
fn a_ping_pong_turns_back_where_it_stands() {
    for easing in [Easing::Linear, Easing::SineInOut] {
        // On its way out, and on its way back.
        for back in [false, true] {
            let case = format!("{easing:?}, back {back}");
            let (mut world, mut schedule) = cue_rig();
            // 120 px at 60 px/s: 120 steps a way.
            let path = PathFollower::new([Vec2::ZERO, Vec2::new(120.0, 0.0)], 60.0)
                .ping_pong()
                .with_easing(easing);
            let named = platform(&mut world, path.clone());
            world.insert(named, naming_its_ends());
            let plain = platform(&mut world, path);
            // 20 steps from the end it left.
            for _ in 0..if back { 140 } else { 20 } {
                cue_step(&mut world, &mut schedule);
            }
            let read = |world: &World| world.get::<PathFollower>(named).unwrap().clone();
            assert_eq!(read(&world).returning(), back, "{case}");
            let before = position(&world, named);
            let point = read(&world).point();
            let seen = world.resource::<SeenCues>().0.len();
            for entity in [named, plain] {
                let follower = world.get_mut::<PathFollower>(entity).unwrap();
                assert!(follower.turn_back(), "{case}");
                follower.set_paused(true);
            }
            let turned = read(&world);
            assert!(
                (turned.point() - point).length() < 1e-3,
                "{case}: where it stands, {} against {point}",
                turned.point()
            );
            assert_eq!(turned.returning(), !back, "{case}: the way it came");
            // Paused, it sends nothing and stands; its `departed` is saved.
            for _ in 0..3 {
                cue_step(&mut world, &mut schedule);
            }
            assert_eq!(world.resource::<SeenCues>().0.len(), seen, "{case}");
            assert_eq!(position(&world, named), before, "{case}");
            let saved = serde_json::to_value(read(&world)).unwrap();
            assert_eq!(saved["turned"], true, "{case}");
            assert_eq!(
                serde_json::from_value::<PathFollower>(saved).unwrap(),
                read(&world)
            );
            // Its next unpaused step: toward the end it left, with one
            // `departed`, the named follower's alone.
            for entity in [named, plain] {
                world
                    .get_mut::<PathFollower>(entity)
                    .unwrap()
                    .set_paused(false);
            }
            cue_step(&mut world, &mut schedule);
            let after = position(&world, named);
            let toward = if back { 1.0 } else { -1.0 };
            assert!(
                (after.x - before.x) * toward > 0.0,
                "{case}: {before} to {after}"
            );
            assert_eq!(
                world.resource::<SeenCues>().0[seen..],
                [(named, DEPARTED)],
                "{case}"
            );
            assert!(
                serde_json::to_value(read(&world))
                    .unwrap()
                    .get("turned")
                    .is_none(),
                "{case}: sent"
            );
            // That end reached in the time it had travelled, within a step,
            // with no cue on the way.
            let mut steps = 1;
            while read(&world).returning() != back {
                assert_eq!(world.resource::<SeenCues>().0.len(), seen + 1, "{case}");
                cue_step(&mut world, &mut schedule);
                steps += 1;
                assert!(steps < 40, "{case}: it reaches the end");
            }
            assert!((19..=21).contains(&steps), "{case}: {steps} steps back");
            let seen = &world.resource::<SeenCues>().0;
            assert!(
                seen.iter().all(|&(entity, _)| entity == named),
                "{case}: none unnamed, {seen:?}"
            );
        }
    }
}

#[test]
fn a_turn_back_is_refused_at_an_end_off_a_ping_pong_and_on_a_lopsided_easing() {
    let points = [Vec2::ZERO, Vec2::new(120.0, 0.0)];
    let refused = |follower: &mut PathFollower, why: &str| {
        let before = follower.clone();
        assert!(!follower.turn_back(), "{why}");
        assert_eq!(*follower, before, "{why}: unchanged");
    };
    // At each end: a fresh ping-pong at its first point, and a lift standing
    // at its first and at its last.
    refused(
        &mut PathFollower::new(points, 60.0).ping_pong(),
        "at its first point",
    );
    let (mut world, mut schedule) = rig();
    let entity = platform(&mut world, lift());
    refused(
        world.get_mut::<PathFollower>(entity).unwrap(),
        "a lift at its first point",
    );
    for _ in 0..100 {
        ride(&mut world, &mut schedule, entity, true);
    }
    assert_eq!(
        world.get::<PathFollower>(entity).unwrap().end(),
        Some(PathEnd::Last)
    );
    refused(
        world.get_mut::<PathFollower>(entity).unwrap(),
        "a lift at its last point",
    );
    // Between its points: a stop path, a loop and a lopsided easing.
    for (mut follower, why) in [
        (PathFollower::new(points, 60.0), "a stop path"),
        (PathFollower::new(points, 60.0).looping(), "a loop"),
        (
            PathFollower::new(points, 60.0)
                .ping_pong()
                .with_easing(Easing::QuadIn),
            "quad_in",
        ),
        (
            PathFollower::new(points, 60.0)
                .ping_pong()
                .with_easing(Easing::CubicOut),
            "cubic_out",
        ),
    ] {
        follower.advance(0.5);
        refused(&mut follower, why);
    }
    // There, a ping-pong on a symmetric easing turns.
    for easing in [Easing::Linear, Easing::SineInOut, Easing::QuadInOut] {
        let mut follower = PathFollower::new(points, 60.0)
            .ping_pong()
            .with_easing(easing);
        follower.advance(0.5);
        assert!(follower.turn_back(), "{easing:?}");
    }
}

#[test]
fn a_follower_reads_its_end_its_way_its_points_and_speed() {
    let (mut world, mut schedule) = rig();
    let entity = platform(&mut world, lift());
    let read = |world: &World| {
        let follower = world.get::<PathFollower>(entity).unwrap();
        (follower.end(), follower.returning())
    };
    let follower = world.get::<PathFollower>(entity).unwrap();
    assert_eq!(follower.points(), [Vec2::ZERO, Vec2::new(0.0, -60.0)]);
    assert_eq!(follower.speed(), 60.0);
    assert_eq!(read(&world), (Some(PathEnd::First), false), "waiting");
    // Aboard, then half its way out.
    for _ in 0..50 {
        ride(&mut world, &mut schedule, entity, true);
    }
    assert_eq!(read(&world), (None, false), "on its way out");
    for _ in 0..60 {
        ride(&mut world, &mut schedule, entity, true);
    }
    assert_eq!(
        read(&world),
        (Some(PathEnd::Last), false),
        "held at its last"
    );
    // Alone a second, then half its way back.
    for _ in 0..92 {
        ride(&mut world, &mut schedule, entity, false);
    }
    assert_eq!(read(&world), (None, true), "on its way back");
    for _ in 0..60 {
        ride(&mut world, &mut schedule, entity, false);
    }
    assert_eq!(read(&world), (Some(PathEnd::First), false), "back");

    // A stop path stands at no end: `is_finished` reads its last point.
    let mut stop = PathFollower::new([Vec2::ZERO, Vec2::new(6.0, 0.0)], 60.0);
    assert_eq!((stop.end(), stop.returning()), (None, false));
    stop.advance(1.0);
    assert!(stop.is_finished());
    assert_eq!((stop.end(), stop.returning()), (None, false));
    // A loop stands at its first point alone, and never returns.
    let mut looping = PathFollower::new(square(), 60.0).looping();
    assert_eq!(looping.end(), Some(PathEnd::First));
    looping.advance(0.5);
    assert_eq!((looping.end(), looping.returning()), (None, false));
    // A ping-pong that holds at no end passes them: out, back, out.
    let mut passing = PathFollower::new([Vec2::ZERO, Vec2::new(6.0, 0.0)], 60.0).ping_pong();
    assert_eq!(passing.end(), Some(PathEnd::First));
    passing.advance(0.05);
    assert_eq!((passing.end(), passing.returning()), (None, false));
    passing.advance(0.1);
    assert_eq!((passing.end(), passing.returning()), (None, true));
    passing.advance(0.1);
    assert_eq!((passing.end(), passing.returning()), (None, false));
}
