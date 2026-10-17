use super::*;
use serde_json::json;
use tungsten_core::{ComponentRegistry, PrefabData, Stage, Tag, Time};

use crate::foundation::trace::KitTrace;
use crate::{Children, LocalOffset, Parent};

fn prefab(json: serde_json::Value) -> PrefabData {
    serde_json::from_value(json).unwrap()
}

/// A World with the plugin, a command buffer, a trace and the prefabs
/// `glow`, `wing` and the bodied `crate`.
fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("glow".to_string(), prefab(json!({ "tag": "glow" })));
    prefabs.insert("wing".to_string(), prefab(json!({ "tag": "wing" })));
    prefabs.insert(
        "crate".to_string(),
        prefab(json!({ "tag": "crate", "components": { "rigid_body": {} } })),
    );
    world.insert_resource(prefabs);
    let mut schedule = Schedule::new();
    SpawnChildrenPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

/// One step, then the command flush.
fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
}

fn tag(world: &World, entity: Entity) -> &str {
    &world.get::<Tag>(entity).unwrap().name
}

#[test]
fn children_spawn_once_at_their_offsets_and_join_the_parents_list_in_order() {
    let (mut world, mut schedule) = rig();
    let gunship = world.spawn_with((
        Transform::from_position(Vec2::new(100.0, 50.0)),
        SpawnChildren::new()
            .with_child("wing", Vec2::new(-24.0, 0.0))
            .with_child("glow", Vec2::new(0.0, 8.0))
            .with_child("wing", Vec2::new(24.0, 0.0)),
    ));
    step(&mut world, &mut schedule);
    let children: Vec<Entity> = world.get::<Children>(gunship).unwrap().iter().collect();
    let tags: Vec<&str> = children.iter().map(|&child| tag(&world, child)).collect();
    assert_eq!(tags, ["wing", "glow", "wing"]);
    for (&child, offset) in children.iter().zip([
        Vec2::new(-24.0, 0.0),
        Vec2::new(0.0, 8.0),
        Vec2::new(24.0, 0.0),
    ]) {
        assert_eq!(world.get::<Parent>(child).unwrap().entity(), gunship);
        assert_eq!(world.get::<LocalOffset>(child), Some(&LocalOffset(offset)));
        assert_eq!(
            world.get::<Transform>(child).unwrap().position,
            Vec2::new(100.0, 50.0) + offset
        );
    }
    assert!(!world.has::<SpawnChildren>(gunship));
    step(&mut world, &mut schedule);
    assert_eq!(world.query::<&Tag>().count(), 3, "spawned once");
    let spawns = world
        .resource::<KitTrace>()
        .about(gunship)
        .filter(|entry| matches!(entry.kind, TraceKind::Spawn { .. }))
        .count();
    assert_eq!(spawns, 3);
}

#[test]
fn a_parent_despawned_before_the_flush_spawns_none() {
    let (mut world, mut schedule) = rig();
    let torch = world.spawn_with((
        Transform::default(),
        SpawnChildren::new().with_child("glow", Vec2::ZERO),
    ));
    world.resource_mut::<CommandBuffer>().despawn(torch);
    step(&mut world, &mut schedule);
    assert_eq!(world.query::<&Tag>().count(), 0);
}

#[test]
fn the_load_refuses_an_unknown_prefab_or_one_with_a_body() {
    let (world, _) = rig();
    let components = world.resource::<ComponentRegistry>();
    let check = |value| components.check_in(SPAWN_CHILDREN, &value, &world);
    assert!(check(json!([{ "prefab": "glow", "offset": [0, -12] }])).is_ok());
    let unknown = check(json!([{ "prefab": "glo" }])).unwrap_err().to_string();
    assert_eq!(
        unknown,
        "component `spawn_children`, field `0.prefab`: unknown prefab `glo`"
    );
    let body = check(json!([{ "prefab": "glow" }, { "prefab": "crate" }]))
        .unwrap_err()
        .to_string();
    assert!(
        body.contains("field `1.prefab`") && body.contains("rigid_body"),
        "{body}"
    );
    assert!(check(json!([{ "prefab": "glow", "angle": 1 }])).is_err());
}

#[test]
fn a_bodied_prefab_inserted_in_code_spawns_nothing_and_traces_why() {
    let (mut world, mut schedule) = rig();
    let cart = world.spawn_with((
        Transform::default(),
        SpawnChildren::new()
            .with_child("crate", Vec2::ZERO)
            .with_child("ruby", Vec2::ZERO),
    ));
    step(&mut world, &mut schedule);
    assert_eq!(world.query::<&Tag>().count(), 0);
    let failed = world
        .resource::<KitTrace>()
        .about(cart)
        .filter(|entry| matches!(entry.kind, TraceKind::SpawnFailed { .. }))
        .count();
    assert_eq!(failed, 2);
}

#[test]
fn the_list_round_trips_as_data() {
    let spawn = SpawnChildren::new().with_child("glow", Vec2::new(0.0, -12.0));
    let data = serde_json::to_value(&spawn).unwrap();
    assert_eq!(data, json!([{ "prefab": "glow", "offset": [0.0, -12.0] }]));
    assert_eq!(
        serde_json::from_value::<SpawnChildren>(data).unwrap(),
        spawn
    );
    let children: Vec<(&str, Vec2)> = spawn.children().collect();
    assert_eq!(children, [("glow", Vec2::new(0.0, -12.0))]);
}
