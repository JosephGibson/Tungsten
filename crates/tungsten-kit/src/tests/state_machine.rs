use super::*;
use crate::{DAMAGE_RESOLVE, HEALTH_TICK, KIT_DESPAWN, SHIELD_TICK};
use crate::{KIT_INTENT, KIT_MOVE, KIT_OVERLAP, KIT_REACT, KIT_RESOLVE, KIT_TICK};
use serde_json::json;
use tungsten_core::Stage;
use tungsten_core::physics::{PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, PhysicsPlugin};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Mode {
    Idle,
    Walk,
    Jump,
}

/// What the hooks ran, in order.
#[derive(Default)]
struct Log(Vec<String>);

fn log(world: &mut World, line: String) {
    world.resource_mut::<Log>().0.push(line);
}

fn enter_idle(world: &mut World, e: Entity) {
    log(world, format!("{} enter idle", e.id()));
}
fn exit_idle(world: &mut World, e: Entity) {
    log(world, format!("{} exit idle", e.id()));
}
fn update_idle(world: &mut World, e: Entity) {
    log(world, format!("{} update idle", e.id()));
}
fn enter_walk(world: &mut World, e: Entity) {
    log(world, format!("{} enter walk", e.id()));
    // A go made during a visit waits for the next step.
    world
        .get_mut::<StateMachine<Mode>>(e)
        .unwrap()
        .go(Mode::Jump);
}
fn exit_walk(world: &mut World, e: Entity) {
    log(world, format!("{} exit walk", e.id()));
}
fn update_walk(world: &mut World, e: Entity) {
    log(world, format!("{} update walk", e.id()));
}
fn enter_jump(world: &mut World, e: Entity) {
    log(world, format!("{} enter jump", e.id()));
    world.despawn(e);
}

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(Log::default());
    let mut schedule = Schedule::new();
    StateMachinePlugin::<Mode>::new("modes")
        .on_enter(Mode::Idle, enter_idle)
        .on_exit(Mode::Idle, exit_idle)
        .on_update(Mode::Idle, update_idle)
        .on_enter(Mode::Walk, enter_walk)
        .on_exit(Mode::Walk, exit_walk)
        .on_update(Mode::Walk, update_walk)
        .on_enter(Mode::Jump, enter_jump)
        .build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) -> Vec<String> {
    world.resource_mut::<Time>().advance_frame(0.25);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    std::mem::take(&mut world.resource_mut::<Log>().0)
}

fn machine(world: &mut World, e: Entity) -> &mut StateMachine<Mode> {
    world.get_mut::<StateMachine<Mode>>(e).unwrap()
}

#[test]
fn the_first_visit_enters_then_updates_and_elapsed_follows_the_clock() {
    let (mut world, mut schedule) = rig();
    let e = world.spawn_with((StateMachine::new(Mode::Idle),));
    assert_eq!(
        step(&mut world, &mut schedule),
        ["0 enter idle", "0 update idle"]
    );
    assert_eq!(step(&mut world, &mut schedule), ["0 update idle"]);
    let m = machine(&mut world, e);
    assert_eq!(
        (m.state(), m.previous(), m.elapsed()),
        (Mode::Idle, None, 0.5)
    );
}

#[test]
fn a_go_before_the_first_visit_enters_that_state() {
    let (mut world, mut schedule) = rig();
    let e = world.spawn_with((StateMachine::new(Mode::Jump),));
    machine(&mut world, e).go(Mode::Idle);
    assert_eq!(
        step(&mut world, &mut schedule),
        ["0 enter idle", "0 update idle"]
    );
    assert_eq!(machine(&mut world, e).previous(), None);
}

#[test]
fn a_transition_runs_exit_then_enter_then_update_and_resets_elapsed() {
    let (mut world, mut schedule) = rig();
    let e = world.spawn_with((StateMachine::new(Mode::Idle),));
    step(&mut world, &mut schedule);
    machine(&mut world, e).go(Mode::Jump);
    machine(&mut world, e).go(Mode::Walk);
    assert_eq!(
        step(&mut world, &mut schedule),
        ["0 exit idle", "0 enter walk", "0 update walk"]
    );
    let m = machine(&mut world, e);
    assert_eq!(
        (m.state(), m.previous(), m.elapsed()),
        (Mode::Walk, Some(Mode::Idle), 0.25)
    );
    // The go made in walk's enter hook waits for this step, and jump's
    // enter hook despawns the entity, which ends the visit.
    assert_eq!(
        step(&mut world, &mut schedule),
        ["0 exit walk", "0 enter jump"]
    );
    assert!(!world.is_alive(e));
}

#[test]
fn a_go_to_the_current_state_enters_it_again() {
    let (mut world, mut schedule) = rig();
    let e = world.spawn_with((StateMachine::new(Mode::Idle),));
    step(&mut world, &mut schedule);
    step(&mut world, &mut schedule);
    machine(&mut world, e).go(Mode::Idle);
    assert_eq!(
        step(&mut world, &mut schedule),
        ["0 exit idle", "0 enter idle", "0 update idle"]
    );
    assert_eq!(machine(&mut world, e).elapsed(), 0.25);
    assert_eq!(machine(&mut world, e).previous(), Some(Mode::Idle));
}

#[test]
fn machines_are_visited_in_entity_order() {
    let (mut world, mut schedule) = rig();
    // The query reads `second` before `third`, which reuses the lower slot.
    let first = world.spawn_with((StateMachine::new(Mode::Idle),));
    let second = world.spawn_with((StateMachine::new(Mode::Idle), 7_u8));
    world.despawn(first);
    let third = world.spawn_with((StateMachine::new(Mode::Idle), 9_u8));
    assert!(third.id() < second.id());
    assert_eq!(
        step(&mut world, &mut schedule),
        [
            "0 enter idle",
            "0 update idle",
            "1 enter idle",
            "1 update idle"
        ]
    );
}

#[test]
fn a_paused_clock_visits_nothing() {
    let (mut world, mut schedule) = rig();
    world.spawn_with((StateMachine::new(Mode::Idle),));
    world.resource_mut::<Time>().pause();
    assert!(step(&mut world, &mut schedule).is_empty());
}

#[test]
fn it_runs_after_the_window_count_and_before_the_step() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    PhysicsPlugin.build(&mut schedule, &mut world);
    StateMachinePlugin::<Mode>::new("modes").build(&mut schedule, &mut world);
    crate::HealthPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    assert_eq!(
        schedule.names(Stage::FixedUpdate).collect::<Vec<_>>(),
        [
            PHYSICS_PREV_SNAPSHOT,
            KIT_TICK,
            HEALTH_TICK,
            SHIELD_TICK,
            KIT_INTENT,
            "modes",
            KIT_MOVE,
            PHYSICS_STEP,
            KIT_OVERLAP,
            KIT_RESOLVE,
            DAMAGE_RESOLVE,
            KIT_REACT,
            KIT_DESPAWN
        ]
    );
}

#[test]
#[should_panic(expected = "StateMachinePlugin::on_update: the state already has a hook")]
fn a_second_hook_of_one_kind_for_one_state_panics() {
    let _ = StateMachinePlugin::<Mode>::new("modes")
        .on_update(Mode::Idle, update_idle)
        .on_update(Mode::Idle, update_walk);
}

#[test]
fn the_plugin_prints_its_name_and_hook_counts() {
    let plugin = StateMachinePlugin::<Mode>::new("modes").on_enter(Mode::Walk, enter_walk);
    assert_eq!(
        format!("{:?}", plugin.clone()),
        "StateMachinePlugin { name: \"modes\", enter_hooks: 1, exit_hooks: 0, update_hooks: 0 }"
    );
}

/// `e`'s machine through a save: written, read back and spawned in a
/// fresh rig as the load does, with the log of the steps before it gone.
fn loaded(world: &World, e: Entity) -> (World, Schedule, Entity) {
    let saved = serde_json::to_value(world.get::<StateMachine<Mode>>(e).unwrap()).unwrap();
    let machine: StateMachine<Mode> = serde_json::from_value(saved.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&machine).unwrap(),
        saved,
        "the save forms"
    );
    let (mut twin, schedule) = rig();
    let copy = twin.spawn_with((machine,));
    (twin, schedule, copy)
}

#[test]
fn a_machine_saved_with_a_pending_go_enters_it_once_after_the_load() {
    let (mut world, mut schedule) = rig();
    let e = world.spawn_with((StateMachine::new(Mode::Idle),));
    step(&mut world, &mut schedule);
    machine(&mut world, e).go(Mode::Walk);
    assert_eq!(machine(&mut world, e).pending(), Some(Mode::Walk));
    let (mut twin, mut twin_schedule, copy) = loaded(&world, e);
    assert_eq!(machine(&mut twin, copy).pending(), Some(Mode::Walk));
    // The first visit after the load is the uninterrupted run's next one:
    // idle's exit, walk's enter once, then walk's own go to jump.
    for n in 0..2 {
        let expected = step(&mut world, &mut schedule);
        assert_eq!(step(&mut twin, &mut twin_schedule), expected, "step {n}");
    }
    assert!(
        !world.is_alive(e) && !twin.is_alive(copy),
        "jump's enter ran in both"
    );
}

#[test]
fn a_loaded_machine_runs_no_enter_hook_again_and_keeps_its_elapsed() {
    let (mut world, mut schedule) = rig();
    let e = world.spawn_with((StateMachine::new(Mode::Idle),));
    for _ in 0..3 {
        step(&mut world, &mut schedule);
    }
    let saved = serde_json::to_value(world.get::<StateMachine<Mode>>(e).unwrap()).unwrap();
    assert_eq!(
        saved,
        json!({
            "state": "Idle", "previous": null, "elapsed": 0.75, "next": null,
            "entered": true, "held": false
        })
    );
    let (mut twin, mut twin_schedule, copy) = loaded(&world, e);
    assert_eq!(machine(&mut twin, copy).elapsed(), 0.75);
    assert_eq!(step(&mut twin, &mut twin_schedule), ["0 update idle"]);
    assert_eq!(step(&mut world, &mut schedule), ["0 update idle"]);
    assert_eq!(
        twin.get::<StateMachine<Mode>>(copy),
        world.get::<StateMachine<Mode>>(e)
    );
    assert_eq!(machine(&mut twin, copy).elapsed(), 1.0);
    // A machine saved before its first visit enters at its first visit.
    let (mut world, _) = rig();
    let fresh = world.spawn_with((StateMachine::new(Mode::Idle),));
    let (mut twin, mut twin_schedule, copy) = loaded(&world, fresh);
    assert_eq!(
        step(&mut twin, &mut twin_schedule),
        ["0 enter idle", "0 update idle"]
    );
    assert_eq!(machine(&mut twin, copy).elapsed(), 0.25);
}

#[test]
fn a_machine_save_refuses_a_negative_or_non_finite_elapsed() {
    let saved = serde_json::to_value(StateMachine::new(Mode::Walk)).unwrap();
    let with = |field: &str, value: serde_json::Value| {
        let mut save = saved.clone();
        save[field] = value;
        serde_json::from_value::<StateMachine<Mode>>(save)
    };
    assert!(with("elapsed", json!(2.5)).is_ok());
    for (field, value) in [
        ("elapsed", json!(-0.1)),
        // Past f32's range: read as infinity.
        ("elapsed", json!(1e39)),
        ("state", json!("Run")),
        ("held", json!(1)),
        ("extra", json!(true)),
    ] {
        assert!(with(field, value.clone()).is_err(), "{field} = {value}");
    }
    let mut missing = saved.clone();
    missing.as_object_mut().unwrap().remove("entered");
    assert!(serde_json::from_value::<StateMachine<Mode>>(missing).is_err());
}

#[test]
fn a_held_machine_keeps_its_pending_go_its_hooks_and_its_elapsed_until_released() {
    let (mut world, mut schedule) = rig();
    let e = world.spawn_with((StateMachine::new(Mode::Idle),));
    step(&mut world, &mut schedule);
    machine(&mut world, e).go(Mode::Walk);
    machine(&mut world, e).set_held(true);
    for _ in 0..3 {
        assert!(step(&mut world, &mut schedule).is_empty(), "no hook runs");
    }
    let m = machine(&mut world, e);
    assert!(m.is_held());
    assert_eq!(
        (m.state(), m.pending(), m.elapsed()),
        (Mode::Idle, Some(Mode::Walk), 0.25),
        "the go waits and the elapsed stands"
    );
    // A save holds the hold.
    let (mut twin, mut twin_schedule, copy) = loaded(&world, e);
    assert!(machine(&mut twin, copy).is_held());
    assert!(step(&mut twin, &mut twin_schedule).is_empty());
    for (world, schedule, e) in [
        (&mut world, &mut schedule, e),
        (&mut twin, &mut twin_schedule, copy),
    ] {
        machine(world, e).set_held(false);
        assert_eq!(
            step(world, schedule),
            ["0 exit idle", "0 enter walk", "0 update walk"]
        );
    }
    // Held before its first visit, it enters once let go.
    let (mut world, mut schedule) = rig();
    let e = world.spawn_with((StateMachine::new(Mode::Idle),));
    machine(&mut world, e).set_held(true);
    assert!(step(&mut world, &mut schedule).is_empty());
    machine(&mut world, e).set_held(false);
    assert_eq!(
        step(&mut world, &mut schedule),
        ["0 enter idle", "0 update idle"]
    );
}
