use super::*;
use serde_json::json;
use tungsten_core::{CommandBuffer, PrefabData, PrefabRegistry, Tag};

/// A World with the plugin built and prefab `slime`.
fn rig() -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    SourcePlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let slime: PrefabData = serde_json::from_value(json!({ "tag": "slime" })).unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("slime".to_string(), slime);
    world.insert_resource(prefabs);
    world
}

/// Spawns `slime` from `source` and flushes.
fn spawn_from(world: &mut World, source: Entity) -> Entity {
    let before: Vec<Entity> = world.query::<(Entity, &Tag)>().map(|(e, _)| e).collect();
    let mut commands = CommandBuffer::new();
    let prepared = world.prepare_prefab("slime", [0.0, 0.0]).unwrap();
    commands.spawn_prefab_from(source, prepared);
    world.flush_reusing(&mut commands);
    world
        .query::<(Entity, &Tag)>()
        .map(|(e, _)| e)
        .find(|e| !before.contains(e))
        .expect("the slime spawned")
}

#[test]
fn a_spawn_names_its_source_and_a_chain_its_origin() {
    let mut world = rig();
    let nest = world.spawn();
    let slime = spawn_from(&mut world, nest);
    assert_eq!(world.get::<Source>(slime), Some(&Source::new(nest)));
    assert_eq!(world.get::<Source>(slime).unwrap().entity(), nest);
    let child = spawn_from(&mut world, slime);
    assert_eq!(
        world.get::<Source>(child),
        Some(&Source::new(nest)),
        "the chain's origin, not the immediate spawner"
    );
    assert!(world.get::<Source>(nest).is_none(), "a root has none");
}

#[test]
fn a_hand_set_source_passes_down_the_chain() {
    let mut world = rig();
    let boss = world.spawn();
    let turret = world.spawn_with((Source::new(boss),));
    let bolt = spawn_from(&mut world, turret);
    assert_eq!(world.get::<Source>(bolt), Some(&Source::new(boss)));
}

#[test]
fn building_the_plugin_twice_registers_once() {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    SourcePlugin.build(&mut schedule, &mut world);
    SourcePlugin.build(&mut schedule, &mut world);
    assert!(
        !world.derive_from_source::<Source>(|_, _| None),
        "already registered"
    );
}
