use super::*;
use glam::Vec2;
use tungsten_core::physics::{Collider, Position, RigidBodyBundle};
use tungsten_core::{Bundle, InspectRegistry, Schedule, Transform};

use crate::{
    ActionInput, CameraTarget, Cooldown, Counters, CueAction, Cues, Flags, Health, HitFlash,
    Intent, KitDescriptions, KitPlugins, Lifetime, Modifier, PathFollower, SpawnChildren, Spawner,
    Stats, TagFilter, Tags, TopDownMover, Zone, spawn_child,
};

fn world() -> World {
    let mut world = World::new();
    let mut schedule = Schedule::new();
    KitPlugins::set().build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    world
}

/// The rows every registered label gives for `entity`, flattened.
fn rows_of(world: &World, entity: Entity) -> Vec<(&'static str, &'static str, String)> {
    world
        .resource::<InspectRegistry>()
        .iter()
        .flat_map(|(label, rows)| {
            rows(world, entity)
                .into_iter()
                .map(move |(name, value)| (label, name, value))
        })
        .collect()
}

#[test]
fn a_world_from_the_kit_plugins_has_rows_for_each_described_component() {
    let world = world();
    let labels: Vec<&str> = world
        .resource::<InspectRegistry>()
        .iter()
        .map(|(label, _)| label)
        .collect();
    let described = world.resource::<KitDescriptions>();
    for (scene, label) in [
        ("tags", "Tags"),
        ("zone", "Zone"),
        ("action_input", "ActionInput"),
        ("camera_target", "CameraTarget"),
        ("lifetime", "Lifetime"),
        ("despawn_outside", "DespawnOutside"),
        ("spawner", "Spawner"),
        ("health", "Health"),
        ("hit_flash", "HitFlash"),
        ("path_follower", "PathFollower"),
        ("top_down_mover", "TopDownMover"),
        ("platformer_controller", "PlatformerController"),
        ("animation", "AnimationState"),
        ("stats", "Stats"),
        ("cues", "Cues"),
        ("cooldown", "Cooldown"),
        ("counters", "Counters"),
        ("flags", "Flags"),
        ("spawn_children", "SpawnChildren"),
    ] {
        assert!(described.get(scene).is_some(), "{scene} is described");
        assert!(labels.contains(&label), "{scene} has rows under {label}");
    }
    for label in ["Intent", "Source", "Parent", "Children", "LocalOffset"] {
        assert!(labels.contains(&label), "{label}");
    }
    assert_eq!(described.iter().count(), 19);
}

#[test]
fn the_rows_print_each_components_state() {
    let mut world = world();
    let hero = world.spawn_with(
        RigidBodyBundle::kinematic(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(8.0))).with(
            (
                Transform::default(),
                Tags::new(["team.player", "flying"]),
                ActionInput::new("left", "right").with_slot("jump", "jump"),
                Intent::default(),
                Health::new(5.0),
                HitFlash::new([1.0; 4], 0.2),
                Lifetime::new(2.0),
                Spawner::new("slime", 1.0).with_count(3),
                PathFollower::new([Vec2::ZERO, Vec2::X * 10.0], 5.0),
                TopDownMover::new(120.0),
                Zone::new(TagFilter::new().all(["team.player"])),
                CameraTarget,
            ),
        ),
    );
    let pod = spawn_child(
        &mut world,
        hero,
        Vec2::new(4.0, 0.0),
        (Transform::default(),),
    );
    let rows = rows_of(&world, hero);
    let find = |label: &str, name: &str| -> String {
        rows.iter()
            .find(|(l, n, _)| *l == label && *n == name)
            .map_or_else(
                || panic!("no row {label}.{name} in {rows:?}"),
                |(_, _, value)| value.clone(),
            )
    };
    assert_eq!(find("Tags", "tags"), "flying, team.player");
    assert_eq!(find("ActionInput", "move"), "left, right");
    assert_eq!(find("ActionInput", "slots"), "jump = jump");
    assert_eq!(find("Intent", "movement"), "(0.00, 0.00)");
    assert_eq!(find("Health", "hp"), "5 / 5");
    assert!(find("HitFlash", "flash").starts_with("off"));
    assert_eq!(find("Lifetime", "left"), "2.00 s");
    assert_eq!(find("Spawner", "remaining"), "3");
    assert_eq!(find("PathFollower", "mode"), "stop");
    assert_eq!(find("TopDownMover", "speed"), "120.0");
    assert_eq!(find("Zone", "filter"), "all: team.player");
    assert_eq!(find("CameraTarget", "camera"), "follows");
    assert_eq!(find("Children", "children"), "1");
    let mut stats = Stats::new().with("armor", 2.0);
    stats.add_modifier("armor", "stone", Modifier::Add(1.0));
    let keeper = world.spawn_with((
        stats,
        Cues::new().on("damaged", [CueAction::flash(), CueAction::shake(0.1)]),
        Cooldown::new(0.5).with_count(2),
        Counters::new().with("coins", 3),
        Flags::new().with("door.locked").with("has_key"),
        SpawnChildren::new().with_child("glow", Vec2::ZERO),
    ));
    let rows = rows_of(&world, keeper);
    let find = |label: &str, name: &str| -> String {
        rows.iter()
            .find(|(l, n, _)| *l == label && *n == name)
            .map_or_else(
                || panic!("no row {label}.{name} in {rows:?}"),
                |(_, _, value)| value.clone(),
            )
    };
    assert_eq!(find("Stats", "stat"), "armor = 3 (base 2, 1 mods)");
    assert_eq!(find("Cues", "cues"), "damaged: 2");
    assert_eq!(find("Cooldown", "next"), "0.50 s");
    assert_eq!(find("Cooldown", "remaining"), "2");
    assert_eq!(find("Counters", "counter"), "coins = 3");
    assert_eq!(find("Flags", "flags"), "door.locked, has_key");
    assert_eq!(find("SpawnChildren", "children"), "glow");
    let pod_rows = rows_of(&world, pod);
    assert!(
        pod_rows
            .iter()
            .any(|(l, n, v)| *l == "LocalOffset" && *n == "offset" && v == "(4.00, 0.00)")
    );
    assert!(pod_rows.iter().any(|(l, ..)| *l == "Parent"));
}

#[test]
fn a_bare_world_gets_a_registry_and_an_app_keeps_its_own() {
    let mut world = World::new();
    let mut registry = InspectRegistry::new();
    registry.register_with("Engine", |_, _| vec![("engine", "first".to_string())]);
    world.insert_resource(registry);
    let mut schedule = Schedule::new();
    KitPlugins::set().build(&mut schedule, &mut world);
    let labels: Vec<&str> = world
        .resource::<InspectRegistry>()
        .iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(labels[0], "Engine", "the engine's rows stay first");
    assert!(labels.contains(&"Tags"));
}
