use super::*;
use glam::Vec2;
use serde_json::json;
use tungsten_core::{ComponentRegistry, PrefabData, PrefabRegistry, Stage, Transform};

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(CameraController::default());
    let mut schedule = Schedule::new();
    CameraTargetPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn frame(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::PostUpdate, world, |_, _| {});
}

fn mode(world: &World) -> CameraMode {
    world.resource::<CameraController>().mode
}

#[test]
fn the_plugin_registers_its_component_and_system() {
    let (world, schedule) = rig();
    assert_eq!(schedule.stage_of(CAMERA_TARGET), Some(Stage::PostUpdate));
    assert!(
        world
            .resource::<ComponentRegistry>()
            .contains(CAMERA_TARGET)
    );
}

#[test]
fn the_camera_follows_the_lowest_live_target_and_frees_when_none_is_left() {
    let (mut world, mut schedule) = rig();
    assert_eq!(mode(&world), CameraMode::Free);
    let hero = world.spawn_with((Transform::default(), CameraTarget));
    let decoy = world.spawn_with((Transform::default(), CameraTarget));
    frame(&mut world, &mut schedule);
    assert_eq!(mode(&world), CameraMode::Follow(hero));

    world.despawn(hero);
    frame(&mut world, &mut schedule);
    assert_eq!(mode(&world), CameraMode::Follow(decoy), "the next live one");

    world.remove_component::<CameraTarget>(decoy);
    frame(&mut world, &mut schedule);
    assert_eq!(mode(&world), CameraMode::Free);
}

#[test]
fn a_mode_the_game_set_stays() {
    let (mut world, mut schedule) = rig();
    let hero = world.spawn_with((Transform::default(), CameraTarget));
    frame(&mut world, &mut schedule);
    assert_eq!(mode(&world), CameraMode::Follow(hero));

    // A cutscene: the game scripts the camera, with the target still alive.
    world.resource_mut::<CameraController>().mode = CameraMode::Scripted;
    frame(&mut world, &mut schedule);
    assert_eq!(mode(&world), CameraMode::Scripted);
    world.remove_component::<CameraTarget>(hero);
    frame(&mut world, &mut schedule);
    assert_eq!(
        mode(&world),
        CameraMode::Scripted,
        "not the plugin's to free"
    );

    // The game frees it: the next target is taken up again.
    world.resource_mut::<CameraController>().mode = CameraMode::Free;
    world.insert(hero, CameraTarget);
    frame(&mut world, &mut schedule);
    assert_eq!(mode(&world), CameraMode::Follow(hero));

    // A follow of the game's own choosing is left alone too.
    let other = world.spawn_with((Transform::default(),));
    world.resource_mut::<CameraController>().mode = CameraMode::Follow(other);
    frame(&mut world, &mut schedule);
    assert_eq!(mode(&world), CameraMode::Follow(other));
}

#[test]
fn without_a_controller_nothing_happens() {
    let (mut world, mut schedule) = rig();
    world.remove_resource::<CameraController>();
    world.spawn_with((Transform::default(), CameraTarget));
    frame(&mut world, &mut schedule);
    assert!(!world.has_resource::<CameraController>());
}

#[test]
fn the_scene_component_spawns_the_marker() {
    let (mut world, _) = rig();
    let hero: PrefabData =
        serde_json::from_value(json!({ "components": { "camera_target": null } })).unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("hero".to_string(), hero);
    world.insert_resource(prefabs);
    let entity = world
        .prepare_prefab("hero", Vec2::ZERO)
        .unwrap()
        .spawn(&mut world);
    assert!(world.has::<CameraTarget>(entity));
    assert!(
        world
            .resource::<ComponentRegistry>()
            .check_in(CAMERA_TARGET, &json!({ "zoom": 2 }), &world)
            .is_err()
    );
}
