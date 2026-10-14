use super::*;
use tungsten_core::physics::PhysicsPlugin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        [PHYSICS_PREV_SNAPSHOT, HEALTH_TICK, "modes", PHYSICS_STEP]
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
