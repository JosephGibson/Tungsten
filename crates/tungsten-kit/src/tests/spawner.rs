use super::*;
use serde_json::json;
use tungsten_core::Stage;
use tungsten_core::{PrefabData, PrefabRegistry, Tag};

/// A World with the clock, a buffer and prefab `slime`, and the plugin's
/// schedule.
fn setup() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    let slime: PrefabData = serde_json::from_value(json!({ "tag": "slime" })).unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("slime".to_string(), slime);
    world.insert_resource(prefabs);
    let mut schedule = Schedule::new();
    SpawnerPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One step of `dt` seconds: the clock, `fixed_update`, the command flush.
fn frame(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

/// The spawned slimes' positions.
fn slimes(world: &World) -> Vec<Vec2> {
    world
        .query::<(&Tag, &Transform)>()
        .filter(|(tag, _)| tag.name == "slime")
        .map(|(_, transform)| transform.position)
        .collect()
}

fn nest(world: &mut World, spawner: Spawner) -> tungsten_core::Entity {
    world.spawn_with((Transform::from_position(Vec2::new(64.0, 32.0)), spawner))
}

#[test]
fn the_plugin_registers_its_system_in_the_react_phase() {
    let (_, schedule) = setup();
    assert_eq!(schedule.stage_of(SPAWNER_TICK), Some(Stage::FixedUpdate));
    assert_eq!(schedule.len(), 8, "the seven anchors and the tick");
}

#[test]
fn the_first_spawn_comes_after_one_interval_at_the_spawners_position() {
    let (mut world, mut schedule) = setup();
    let nest = nest(&mut world, Spawner::new("slime", 0.25));
    frame(&mut world, &mut schedule, 0.1);
    frame(&mut world, &mut schedule, 0.1);
    assert!(slimes(&world).is_empty());
    let left = world.get::<Spawner>(nest).unwrap().timer().remaining();
    assert!((left - 0.05).abs() < 1e-5, "{left}");
    frame(&mut world, &mut schedule, 0.1);
    assert_eq!(slimes(&world), [Vec2::new(64.0, 32.0)]);
    assert_eq!(world.get::<Spawner>(nest).unwrap().remaining(), None);

    // The spawner moved: the next one lands where it is now.
    world.get_mut::<Transform>(nest).unwrap().position = Vec2::new(10.0, 0.0);
    frame(&mut world, &mut schedule, 0.25);
    let mut found = slimes(&world);
    found.sort_by(|a, b| a.x.total_cmp(&b.x));
    assert_eq!(found, [Vec2::new(10.0, 0.0), Vec2::new(64.0, 32.0)]);
}

#[test]
fn a_long_frame_spawns_each_finished_period_up_to_the_count() {
    let (mut world, mut schedule) = setup();
    let nest = nest(&mut world, Spawner::new("slime", 0.25).with_count(3));
    frame(&mut world, &mut schedule, 0.5);
    assert_eq!(slimes(&world).len(), 2);
    assert_eq!(world.get::<Spawner>(nest).unwrap().remaining(), Some(1));
    frame(&mut world, &mut schedule, 1.0);
    assert_eq!(slimes(&world).len(), 3, "four periods, one spawn left");
    assert_eq!(world.get::<Spawner>(nest).unwrap().remaining(), Some(0));
    frame(&mut world, &mut schedule, 1.0);
    assert_eq!(slimes(&world).len(), 3);
}

#[test]
fn a_count_of_zero_spawns_nothing() {
    let (mut world, mut schedule) = setup();
    nest(&mut world, Spawner::new("slime", 0.1).with_count(0));
    frame(&mut world, &mut schedule, 1.0);
    assert!(slimes(&world).is_empty());
}

#[test]
fn a_pause_holds_the_spawner_and_the_time_scale_stretches_it() {
    let (mut world, mut schedule) = setup();
    nest(&mut world, Spawner::new("slime", 0.25));
    world.resource_mut::<Time>().pause();
    frame(&mut world, &mut schedule, 1.0);
    assert!(slimes(&world).is_empty());
    world.resource_mut::<Time>().resume();
    world.resource_mut::<Time>().set_scale(0.5);
    frame(&mut world, &mut schedule, 0.4);
    assert!(slimes(&world).is_empty(), "0.2 s of game time");
    frame(&mut world, &mut schedule, 0.2);
    assert_eq!(slimes(&world).len(), 1);
}

#[test]
fn an_unknown_prefab_spawns_nothing_and_the_rest_still_spawn() {
    let (mut world, mut schedule) = setup();
    let ghost = nest(&mut world, Spawner::new("ghost", 0.25).with_count(2));
    nest(&mut world, Spawner::new("slime", 0.25));
    frame(&mut world, &mut schedule, 0.25);
    frame(&mut world, &mut schedule, 0.25);
    assert_eq!(slimes(&world).len(), 2);
    assert_eq!(world.entity_count(), 4, "two nests and two slimes");
    assert_eq!(
        world.get::<Spawner>(ghost).unwrap().remaining(),
        Some(0),
        "a period that spawned nothing still counts"
    );
}

#[test]
fn without_a_command_buffer_the_spawners_hold() {
    let (mut world, mut schedule) = setup();
    let nest = nest(&mut world, Spawner::new("slime", 0.25));
    world.remove_resource::<CommandBuffer>();
    world.resource_mut::<Time>().advance_frame(1.0);
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    assert!(slimes(&world).is_empty());
    assert!(
        world.get::<Spawner>(nest).unwrap().timer().elapsed() < 1e-6,
        "nothing ticked"
    );
}

/// What a spawner passes on to its spawns in the test below.
#[derive(Debug, Clone, PartialEq)]
struct Owner(u8);

#[test]
fn a_spawn_takes_what_its_spawner_passes_on_and_a_despawned_spawner_spawns_nothing() {
    let (mut world, mut schedule) = setup();
    world.inherit_from_source::<Owner>();
    let owned = world.spawn_with((
        Transform::from_position(Vec2::new(10.0, 0.0)),
        Spawner::new("slime", 0.25),
        Owner(3),
    ));
    let control = nest(&mut world, Spawner::new("slime", 0.25));
    let owners = |world: &World| {
        let mut found: Vec<(f32, Option<Owner>)> = world
            .query::<(&Tag, &Transform, Option<&Owner>)>()
            .filter(|(tag, ..)| tag.name == "slime")
            .map(|(_, transform, owner)| (transform.position.x, owner.cloned()))
            .collect();
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found
    };
    frame(&mut world, &mut schedule, 0.25);
    assert_eq!(owners(&world), [(10.0, Some(Owner(3))), (64.0, None)]);

    // A due frame whose commands despawn the spawner before its tick.
    world.resource_mut::<CommandBuffer>().despawn(owned);
    frame(&mut world, &mut schedule, 0.25);
    assert!(!world.is_alive(owned));
    assert_eq!(
        owners(&world),
        [(10.0, Some(Owner(3))), (64.0, None), (64.0, None)],
        "the despawned spawner spawned nothing; the other did"
    );
    assert!(world.is_alive(control));
    assert_eq!(world.entity_count(), 4, "one nest and three slimes");
}

#[test]
#[should_panic(expected = "interval must be finite and > 0")]
fn a_zero_interval_panics() {
    let _ = Spawner::new("slime", 0.0);
}

#[test]
fn spawns_and_failures_go_in_the_trace() {
    use crate::foundation::trace::{KitTrace, TraceKind};

    let (mut world, mut schedule) = setup();
    world.insert_resource(KitTrace::default());
    let nest = world.spawn_with((Transform::default(), Spawner::new("slime", 0.1)));
    let broken = world.spawn_with((Transform::default(), Spawner::new("ruby", 0.1)));
    frame(&mut world, &mut schedule, 0.1);
    let trace = world.resource::<KitTrace>();
    let kinds: Vec<&TraceKind> = trace.about(nest).map(|entry| &entry.kind).collect();
    assert_eq!(
        kinds,
        [&TraceKind::Spawn {
            prefab: "slime".to_string(),
            by: "spawner"
        }]
    );
    let failed: Vec<&TraceKind> = trace.about(broken).map(|entry| &entry.kind).collect();
    assert!(
        matches!(failed[..], [TraceKind::SpawnFailed { prefab, .. }] if prefab == "ruby"),
        "{failed:?}"
    );
}

#[test]
fn an_alive_spawner_waits_an_interval_after_its_spawn_dies() {
    let (mut world, mut schedule) = setup();
    nest(&mut world, Spawner::new("slime", 0.5).with_alive(1));
    let slime = |world: &World| {
        world
            .query::<(Entity, &Tag)>()
            .find(|(_, tag)| tag.name == "slime")
            .map(|(entity, _)| entity)
    };
    // Its first after one interval: four steps of 1/8 s.
    for _ in 0..3 {
        frame(&mut world, &mut schedule, 0.125);
    }
    assert!(slime(&world).is_none());
    frame(&mut world, &mut schedule, 0.125);
    let first = slime(&world).expect("the first after one interval");
    // Full, it holds however long.
    for _ in 0..40 {
        frame(&mut world, &mut schedule, 0.125);
    }
    assert_eq!(slimes(&world).len(), 1, "one live at most");
    // Its spawn gone, the next comes an interval later.
    world.despawn(first);
    for _ in 0..3 {
        frame(&mut world, &mut schedule, 0.125);
        assert!(slime(&world).is_none(), "an interval after it died");
    }
    frame(&mut world, &mut schedule, 0.125);
    assert!(slime(&world).is_some());
    assert_eq!(slimes(&world).len(), 1);
}

#[test]
fn two_steps_a_frame_queue_one_spawn() {
    let (mut world, mut schedule) = setup();
    nest(
        &mut world,
        Spawner::new("slime", 1.0 / 60.0)
            .with_alive(1)
            .with_first(0.0),
    );
    // Two fixed steps before the flush: the second counts the first's queued
    // spawn by the frame.
    world.resource_mut::<Time>().advance_frame(1.0 / 30.0);
    world.resource_mut::<Time>().enter_fixed_step();
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    world.resource_mut::<Time>().leave_fixed_steps();
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    assert_eq!(slimes(&world).len(), 1);
    // A plain spawner at that interval queues one a step.
    let (mut world, mut schedule) = setup();
    nest(
        &mut world,
        Spawner::new("slime", 1.0 / 60.0).with_first(0.0),
    );
    world.resource_mut::<Time>().advance_frame(1.0 / 30.0);
    world.resource_mut::<Time>().enter_fixed_step();
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    world.resource_mut::<Time>().leave_fixed_steps();
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    assert_eq!(slimes(&world).len(), 2, "unheld, one a step");
}

#[test]
fn a_first_delay_spawns_at_once() {
    let (mut world, mut schedule) = setup();
    nest(&mut world, Spawner::new("slime", 0.5).with_first(0.0));
    frame(&mut world, &mut schedule, 0.125);
    assert_eq!(slimes(&world).len(), 1, "at its first tick");
    for _ in 0..3 {
        frame(&mut world, &mut schedule, 0.125);
    }
    assert_eq!(slimes(&world).len(), 1);
    frame(&mut world, &mut schedule, 0.125);
    assert_eq!(slimes(&world).len(), 2, "then one an interval");
    let (mut world, mut schedule) = setup();
    nest(&mut world, Spawner::new("slime", 0.5).with_first(0.25));
    frame(&mut world, &mut schedule, 0.125);
    assert!(slimes(&world).is_empty());
    frame(&mut world, &mut schedule, 0.125);
    assert_eq!(slimes(&world).len(), 1, "after its quarter second");
    // The load refuses a first past the interval and an alive of 0.
    let components = world.resource::<tungsten_core::ComponentRegistry>();
    let check = |value: serde_json::Value| components.check_in("spawner", &value, &world);
    assert!(check(json!({ "prefab": "slime", "interval": 0.5, "alive": 1, "first": 0 })).is_ok());
    for refused in [
        json!({ "prefab": "slime", "interval": 0.5, "first": 0.75 }),
        json!({ "prefab": "slime", "interval": 0.5, "first": -1 }),
        json!({ "prefab": "slime", "interval": 0.5, "alive": 0 }),
    ] {
        assert!(check(refused.clone()).is_err(), "{refused}");
    }
    assert!(std::panic::catch_unwind(|| Spawner::new("slime", 0.5).with_first(1.0)).is_err());
    // A plain spawner's save keeps its bytes.
    let saved = serde_json::to_value(SpawnerSave::from(&Spawner::new("slime", 0.5))).unwrap();
    assert!(saved.get("alive").is_none() && saved.get("first_left").is_none());
}
