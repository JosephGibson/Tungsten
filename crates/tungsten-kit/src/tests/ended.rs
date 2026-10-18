use super::*;
use glam::Vec2;
use tungsten_core::{EventQueue, Plugin, PrefabData, PrefabRegistry, Stage, Tag, Time, Transform};

use crate::foundation::trace::KitTrace;
use crate::lifecycle::despawn::{DespawnOutside, DespawnOutsidePlugin};
use crate::lifecycle::lifetime::{Lifetime, LifetimePlugin};
use crate::overlap::zone::ZoneEntered;
use crate::react::cues::{CueAction, Cues, CuesPlugin};
use crate::transform::hierarchy::spawn_child;

/// A World with lifetimes, despawn outside and cues, a command buffer, a
/// trace, the zone queue and the prefab `puff`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    world.register_event::<ZoneEntered>();
    let mut prefabs = PrefabRegistry::new();
    let puff: PrefabData = serde_json::from_value(serde_json::json!({ "tag": "puff" })).unwrap();
    prefabs.insert("puff".to_string(), puff);
    world.insert_resource(prefabs);
    let mut schedule = Schedule::new();
    LifetimePlugin.build(&mut schedule, &mut world);
    DespawnOutsidePlugin.build(&mut schedule, &mut world);
    CuesPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One fixed step of `dt`, without the flush.
fn step(world: &mut World, schedule: &mut Schedule, dt: f32) {
    world.resource_mut::<Time>().advance_frame(dt);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

/// The command and event flushes that end a frame.
fn flush(world: &mut World) {
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

/// A cue that spawns `puff` when its zone is entered, a zone event sent.
fn puffs_on_entry(world: &mut World, entity: Entity) {
    world.insert(
        entity,
        Cues::new().on("zone_entered", [CueAction::prefab("puff")]),
    );
    let other = world.spawn();
    world
        .resource_mut::<EventQueue<ZoneEntered>>()
        .send(ZoneEntered::new(entity, other, Vec2::ZERO));
}

fn despawns(world: &World, entity: Entity) -> Vec<String> {
    world
        .resource::<KitTrace>()
        .about(entity)
        .map(ToString::to_string)
        .filter(|line| line.contains("despawned by"))
        .collect()
}

#[test]
fn the_kit_despawn_follows_the_cues_in_the_react_phase() {
    let (_, schedule) = rig();
    let order: Vec<&str> = schedule.names(Stage::FixedUpdate).collect();
    let at = |name: &str| order.iter().position(|n| *n == name).unwrap();
    assert!(at(crate::CUES) < at(KIT_DESPAWN));
    assert_eq!(order.last(), Some(&KIT_DESPAWN));
}

#[test]
fn a_lifetimes_end_keeps_the_steps_cue_spawn_from_its_entity() {
    let (mut world, mut schedule) = rig();
    let bolt = world.spawn_with((
        Transform::from_position(Vec2::new(3.0, 4.0)),
        Lifetime::new(0.0),
    ));
    puffs_on_entry(&mut world, bolt);
    step(&mut world, &mut schedule, 1.0 / 60.0);
    // A frame's second step lists it again; the mark skips it.
    step(&mut world, &mut schedule, 1.0 / 60.0);
    flush(&mut world);
    assert!(!world.is_alive(bolt));
    assert_eq!(
        world.query::<&Tag>().count(),
        1,
        "the spark survived the flush"
    );
    let lines = despawns(&world, bolt);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].ends_with("despawned by lifetime"));
}

#[test]
fn an_exit_from_a_region_keeps_the_steps_cue_spawn_from_its_entity() {
    let (mut world, mut schedule) = rig();
    let region = DespawnOutside::region(Vec2::ZERO, Vec2::new(10.0, 10.0));
    let stray = world.spawn_with((Transform::from_position(Vec2::new(50.0, 5.0)), region));
    puffs_on_entry(&mut world, stray);
    step(&mut world, &mut schedule, 1.0 / 60.0);
    step(&mut world, &mut schedule, 1.0 / 60.0);
    flush(&mut world);
    assert!(!world.is_alive(stray));
    assert_eq!(world.query::<&Tag>().count(), 1);
    let lines = despawns(&world, stray);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].ends_with("despawned by despawn_outside"));
}

#[test]
fn the_kit_despawn_marks_the_ended_entity_and_its_descendants() {
    let (mut world, mut schedule) = rig();
    let parent = world.spawn_with((Transform::default(), Lifetime::new(0.0)));
    let child = spawn_child(&mut world, parent, Vec2::X, (Transform::default(),));
    let grandchild = spawn_child(&mut world, child, Vec2::X, (Transform::default(),));
    let bystander = world.spawn_with((Transform::default(),));
    step(&mut world, &mut schedule, 1.0 / 60.0);
    for entity in [parent, child, grandchild] {
        assert!(world.has::<Despawning>(entity), "{entity:?}");
    }
    assert!(!world.has::<Despawning>(bystander));
    flush(&mut world);
    assert!(!world.is_alive(grandchild) && world.is_alive(bystander));
}

#[test]
fn a_cue_despawn_marks_its_targets_descendants() {
    let (mut world, mut schedule) = rig();
    let door = world.spawn_with((Transform::default(),));
    let hinge = spawn_child(&mut world, door, Vec2::X, (Transform::default(),));
    world.insert(door, Cues::new().on("zone_entered", [CueAction::despawn()]));
    let hero = world.spawn();
    world
        .resource_mut::<EventQueue<ZoneEntered>>()
        .send(ZoneEntered::new(door, hero, Vec2::ZERO));
    step(&mut world, &mut schedule, 1.0 / 60.0);
    assert!(world.has::<Despawning>(door) && world.has::<Despawning>(hinge));
    flush(&mut world);
    assert!(!world.is_alive(hinge));
}

#[test]
fn without_a_command_buffer_nothing_is_marked_and_the_end_waits() {
    let (mut world, mut schedule) = rig();
    let commands = world.remove_resource::<CommandBuffer>().unwrap();
    let spark = world.spawn_with((Lifetime::new(0.0),));
    step(&mut world, &mut schedule, 1.0 / 60.0);
    assert!(!world.has::<Despawning>(spark));
    assert!(despawns(&world, spark).is_empty());
    world.insert_resource(commands);
    step(&mut world, &mut schedule, 1.0 / 60.0);
    flush(&mut world);
    assert!(!world.is_alive(spark));
    assert_eq!(despawns(&world, spark).len(), 1);
}

#[test]
fn an_end_listed_without_the_list_or_for_a_gone_entity_does_nothing() {
    let mut world = World::new();
    let ghost = world.spawn();
    end(&mut world, ghost, "test");
    assert!(!world.has_resource::<Ended>());
    let (mut world, mut schedule) = rig();
    let ghost = world.spawn();
    world.despawn(ghost);
    end(&mut world, ghost, "test");
    step(&mut world, &mut schedule, 1.0 / 60.0);
    assert!(world.resource::<Ended>().0.is_empty(), "drained");
    assert!(despawns(&world, ghost).is_empty());
}
