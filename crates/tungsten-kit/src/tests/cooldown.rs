use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, Stage};

use crate::foundation::trace::KitTrace;

/// A World with the cooldown plugin and a trace.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    CooldownPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One frame of `dt`, its fixed steps run; the cooldowns that elapsed.
fn frame(world: &mut World, schedule: &mut Schedule, dt: f32) -> Vec<Entity> {
    world.resource_mut::<Time>().advance_frame(dt);
    let steps = world.resource::<Time>().fixed_steps_this_frame();
    for _ in 0..steps {
        world.resource_mut::<Time>().enter_fixed_step();
        schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    }
    world.resource_mut::<Time>().leave_fixed_steps();
    let elapsed = world
        .resource::<EventQueue<CooldownElapsed>>()
        .iter_current()
        .map(|event| event.entity)
        .collect();
    world.flush_events();
    elapsed
}

/// A cooldown midway: `left` seconds of its current cycle to go.
fn midway(seconds: f32, jitter: f32, left: f32) -> Cooldown {
    let mut cooldown = Cooldown::new(seconds).with_jitter(jitter);
    cooldown.left = left;
    cooldown.fresh = false;
    cooldown
}

#[test]
fn a_jittered_cooldown_draws_each_cycle_before_counting_the_next() {
    // 0.005 s left of a 0.005 s cycle and a 1/60 s step: the first elapse
    // draws an extra 0.02 s, so the next cycle (0.025 s) does not fit.
    let mut cooldown = midway(0.005, 0.03, 0.005);
    let (elapses, capped) = cooldown.advance(1.0 / 60.0, &mut |_| 0.02);
    assert_eq!((elapses, capped), (1, false));
    let expected = 0.005 - 1.0 / 60.0 + 0.025;
    assert!(
        (cooldown.left() - expected).abs() < 1e-6,
        "{}",
        cooldown.left()
    );
}

#[test]
fn a_cooldown_elapses_at_most_sixteen_times_a_step() {
    let mut cooldown = Cooldown::new(0.0001);
    let (elapses, capped) = cooldown.advance(1.0 / 60.0, &mut |_| 0.0);
    assert_eq!((elapses, capped), (Cooldown::MAX_ELAPSES_PER_STEP, true));
    assert_eq!(cooldown.left(), 0.0001, "the next cycle starts whole");

    // Through the system: a rate far above 1 sends 16 events and notes it.
    let (mut world, mut schedule) = rig();
    let mut fast = Cooldown::new(0.5);
    fast.set_rate(1e9);
    let turret = world.spawn_with((fast,));
    let elapsed = frame(&mut world, &mut schedule, 1.0 / 60.0);
    assert_eq!(elapsed.len(), 16);
    assert!(elapsed.iter().all(|&entity| entity == turret));
    let trace = world.resource::<KitTrace>();
    assert_eq!(
        trace
            .iter()
            .filter(|e| matches!(e.kind, TraceKind::Skipped { .. }))
            .count(),
        1
    );
}

#[test]
fn without_jitter_it_elapses_once_per_whole_cycle_in_the_step() {
    let mut cooldown = Cooldown::new(0.1);
    let (elapses, capped) = cooldown.advance(0.35, &mut |_| unreachable!("no jitter"));
    assert_eq!((elapses, capped), (3, false));
    assert!((cooldown.left() - 0.05).abs() < 1e-6);
}

#[test]
fn a_count_stops_it_and_zero_never_elapses() {
    let mut once = Cooldown::new(0.1).with_count(2);
    assert_eq!(once.advance(1.0, &mut |_| 0.0), (2, false));
    assert_eq!(once.remaining(), Some(0));
    assert_eq!(once.advance(1.0, &mut |_| 0.0), (0, false));
    let mut never = Cooldown::new(0.1).with_count(0);
    assert_eq!(never.advance(1.0, &mut |_| 0.0), (0, false));
}

#[test]
fn a_fresh_jittered_cooldown_draws_its_first_extra_at_its_first_step() {
    let mut cooldown = Cooldown::new(0.5).with_jitter(0.25);
    assert!(cooldown.draws_within(0.0));
    let (elapses, _) = cooldown.advance(0.1, &mut |jitter| jitter);
    assert_eq!(elapses, 0);
    assert!((cooldown.left() - 0.65).abs() < 1e-6, "0.5 + 0.25 - 0.1");
    cooldown.restart();
    assert_eq!(cooldown.left(), 0.5);
    assert!(cooldown.draws_within(0.0));
}

#[test]
fn the_system_counts_on_the_game_clock_and_its_rate() {
    let (mut world, mut schedule) = rig();
    let slow = world.spawn_with((Cooldown::new(0.25),));
    let mut quick = Cooldown::new(0.25);
    quick.set_rate(2.0);
    let fast = world.spawn_with((quick,));
    let mut held = Cooldown::new(0.25);
    held.set_rate(0.0);
    let frozen = world.spawn_with((held,));
    let mut counts = [0, 0, 0];
    for _ in 0..60 {
        for entity in frame(&mut world, &mut schedule, 1.0 / 60.0) {
            let at = [slow, fast, frozen]
                .iter()
                .position(|&e| e == entity)
                .unwrap();
            counts[at] += 1;
        }
    }
    assert_eq!(counts, [4, 8, 0], "one second of game time");

    // A paused clock holds every cooldown.
    world.resource_mut::<Time>().pause();
    let left = world.get::<Cooldown>(slow).unwrap().left();
    assert!(frame(&mut world, &mut schedule, 1.0 / 60.0).is_empty());
    assert_eq!(world.get::<Cooldown>(slow).unwrap().left(), left);
}

/// The frames on which a jittered cooldown elapsed over two seconds, under
/// `seed`.
fn hops(seed: u64) -> Vec<u32> {
    let (mut world, mut schedule) = rig();
    world.resource_mut::<KitRng>().reseed(seed);
    let hopper = world.spawn_with((Cooldown::new(0.25).with_jitter(0.25),));
    world.spawn_with((Cooldown::new(0.25).with_jitter(0.25),));
    (0..120)
        .filter(|_| frame(&mut world, &mut schedule, 1.0 / 60.0).contains(&hopper))
        .collect()
}

#[test]
fn jittered_cycles_repeat_under_a_seed_and_differ_under_another() {
    let a = hops(1);
    assert_eq!(a, hops(1));
    assert_ne!(a, hops(2));
    assert!(a.len() >= 4 && a.len() <= 8, "{a:?}");
    for pair in a.windows(2) {
        let gap = pair[1] - pair[0];
        assert!((15..=31).contains(&gap), "0.25 to 0.5 s apart: {a:?}");
    }
}

#[test]
fn the_scene_form_builds_and_the_save_form_round_trips() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    assert!(
        components
            .check_in(
                COOLDOWN,
                &json!({ "seconds": 0.5, "jitter": 0.25, "count": 3 }),
                &world
            )
            .is_ok()
    );
    for bad in [
        json!({ "seconds": 0 }),
        json!({ "seconds": 1, "jitter": -1 }),
        json!({ "seconds": 1, "count": -1 }),
        json!({ "seconds": 1, "every": 2 }),
    ] {
        assert!(
            components.check_in(COOLDOWN, &bad, &world).is_err(),
            "{bad}"
        );
    }
    let cooldown = Cooldown::new(0.5).with_jitter(0.1).with_count(3);
    let saved = serde_json::to_value(&cooldown).unwrap();
    assert_eq!(serde_json::from_value::<Cooldown>(saved).unwrap(), cooldown);
    assert!(serde_json::from_value::<Cooldown>(json!({ "seconds": 1, "left": -1 })).is_err());
}

#[test]
#[should_panic(expected = "seconds must be finite and > 0")]
fn a_zero_cycle_panics() {
    let _ = Cooldown::new(0.0);
}
