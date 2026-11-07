use super::*;
use glam::Vec2;
use serde_json::{Value, json};
use tungsten_core::{
    ComponentRegistry, FollowRig, PrefabData, PrefabRegistry, RigKind, Stage, Time, Transform,
};

use tungsten_core::physics::Position;

use crate::{AimPlugin, Intent, KitDescriptions};

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

fn target(world: &World) -> Option<Entity> {
    world.resource::<CameraController>().rig.target()
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
fn the_base_rig_follows_the_lowest_live_target_and_holds_when_none_is_left() {
    let (mut world, mut schedule) = rig();
    assert_eq!(target(&world), None);
    let hero = world.spawn_with((Transform::default(), CameraTarget));
    let decoy = world.spawn_with((Transform::default(), CameraTarget));
    frame(&mut world, &mut schedule);
    assert_eq!(target(&world), Some(hero));

    world.despawn(hero);
    frame(&mut world, &mut schedule);
    assert_eq!(target(&world), Some(decoy), "the next live one");

    world.remove_component::<CameraTarget>(decoy);
    frame(&mut world, &mut schedule);
    assert_eq!(target(&world), None, "a follow with no target holds");
    assert!(matches!(
        world.resource::<CameraController>().rig.kind,
        RigKind::Follow(_)
    ));
}

#[test]
fn a_base_rig_the_game_set_stays() {
    let (mut world, mut schedule) = rig();
    let hero = world.spawn_with((Transform::default(), CameraTarget));
    frame(&mut world, &mut schedule);
    assert_eq!(target(&world), Some(hero));

    // A cutscene: the game scripts the base rig, with the target alive.
    let scripted = RigKind::Scripted(Vec2::new(10.0, 20.0));
    world.resource_mut::<CameraController>().rig.kind = scripted;
    frame(&mut world, &mut schedule);
    assert_eq!(world.resource::<CameraController>().rig.kind, scripted);
    world.remove_component::<CameraTarget>(hero);
    frame(&mut world, &mut schedule);
    assert_eq!(
        world.resource::<CameraController>().rig.kind,
        scripted,
        "not the plugin's to clear"
    );

    // The game sets a follow with no target again: the next target is
    // taken up.
    world.resource_mut::<CameraController>().rig.kind = RigKind::Follow(FollowRig::default());
    world.insert(hero, CameraTarget);
    frame(&mut world, &mut schedule);
    assert_eq!(target(&world), Some(hero));

    // A follow of the game's own choosing is left alone too.
    let other = world.spawn_with((Transform::default(),));
    world.resource_mut::<CameraController>().rig.kind = RigKind::Follow(FollowRig::of(other));
    frame(&mut world, &mut schedule);
    assert_eq!(target(&world), Some(other));
    world.remove_component::<CameraTarget>(hero);
    frame(&mut world, &mut schedule);
    assert_eq!(target(&world), Some(other), "nor cleared");
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

/// A World with the rig plugin built, and the aim's when `aim`.
fn rigs(aim: bool) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CameraController::default());
    let mut schedule = Schedule::new();
    CameraRigPlugin.build(&mut schedule, &mut world);
    if aim {
        AimPlugin.build(&mut schedule, &mut world);
    }
    schedule.resolve().unwrap();
    (world, schedule)
}

/// The entity a prefab of `components` spawns at the origin.
fn spawn_prefab(world: &mut World, components: Value) -> Entity {
    let mut prefab = json!({});
    prefab["components"] = components;
    let prefab: PrefabData = serde_json::from_value(prefab).unwrap();
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert("rigged".to_string(), prefab);
    world.insert_resource(prefabs);
    world
        .prepare_prefab("rigged", Vec2::ZERO)
        .unwrap()
        .spawn(world)
}

/// The rig `rig` spawns, its form checked against the registry first.
fn spawned(world: &mut World, rig: Value) -> CameraRig {
    let registry = world.resource::<ComponentRegistry>();
    registry
        .check_in(CAMERA_RIG, &rig, world)
        .unwrap_or_else(|e| panic!("{rig} refused: {e}"));
    let mut components = json!({});
    components[CAMERA_RIG] = rig;
    let entity = spawn_prefab(world, components);
    *world.get::<CameraRig>(entity).unwrap()
}

/// `value` through the component's description's round trip.
fn round_trip_rig(world: &World, value: Value) -> Result<Value, String> {
    world
        .resource::<KitDescriptions>()
        .get(CAMERA_RIG)
        .unwrap()
        .round_trip(value)
        .unwrap()
}

#[test]
fn a_camera_rig_round_trips_every_form() {
    let (mut world, _) = rigs(false);
    let lead = RigLead::velocity(120.0, 0.4).with_smoothing(0.25);
    let follow = FollowRig::default()
        .with_offset(Vec2::new(16.0, 8.0))
        .with_dead_zone(Vec2::new(320.0, 180.0))
        .with_smoothing(Vec2::new(0.2, 1.0))
        .with_lead(lead);
    let bounds = CameraBounds::new(Vec2::ZERO, Vec2::new(4000.0, 1200.0));
    let cases = [
        (
            json!({
                "follow": {
                    "offset": [16, 8],
                    "dead_zone": [320, 180],
                    "smoothing": [0.2, 1],
                    "lead": { "from": "velocity", "max": 120, "time": 0.4, "smoothing": 0.25 }
                },
                "view_height": 540,
                "zoom": 2,
                "bounds": { "min": [0, 0], "max": [4000, 1200] },
                "priority": -3,
                "blend": { "time": 0.4, "ease": "quad_out" },
                "live": false
            }),
            CameraRig::follow(follow)
                .with_view_height(540.0)
                .with_zoom(2.0)
                .with_bounds(bounds)
                .with_priority(-3)
                .with_blend(RigBlend::over(0.4).with_ease(Easing::QuadOut))
                .lowered(),
        ),
        (
            json!({ "follow": { "smoothing": 0.5, "lead": { "from": "aim", "max": 96 } } }),
            CameraRig::follow(
                FollowRig::default()
                    .with_smoothing(Vec2::splat(0.5))
                    .with_lead(RigLead::aim(96.0)),
            ),
        ),
        (
            json!({ "follow": {} }),
            CameraRig::follow(FollowRig::default()),
        ),
        (
            json!({ "fixed": { "at": [640, 360] }, "blend": 0.5 }),
            CameraRig::fixed(FixedRig::At(Vec2::new(640.0, 360.0))).with_blend(RigBlend::over(0.5)),
        ),
        (
            json!({ "fixed": { "corner": [0, -3424] }, "view_height": 1152 }),
            CameraRig::fixed(FixedRig::Corner(Vec2::new(0.0, -3424.0))).with_view_height(1152.0),
        ),
        (
            json!({ "fixed": { "rect": { "min": [100, 50], "max": [900, 500] } }, "priority": 10 }),
            CameraRig::fixed(FixedRig::Rect(CameraBounds::new(
                Vec2::new(100.0, 50.0),
                Vec2::new(900.0, 500.0),
            )))
            .with_priority(10),
        ),
        (
            json!({ "scripted": [12.5, -40] }),
            CameraRig::scripted(Vec2::new(12.5, -40.0)),
        ),
    ];
    for (form, expected) in cases {
        assert_eq!(spawned(&mut world, form.clone()), expected, "{form}");
        // Through the DTO and back, and back again: the same value.
        let once = round_trip_rig(&world, form.clone()).unwrap();
        assert_eq!(
            round_trip_rig(&world, once.clone()).unwrap(),
            once,
            "{form}"
        );
        assert_eq!(spawned(&mut world, once), expected, "{form}'s round trip");
    }
    // A follow in data follows its own entity.
    let rig = spawned(&mut world, json!({ "follow": {} }));
    assert_eq!(rig.target(), None);
}

#[test]
fn a_camera_rig_refuses_two_kinds_and_bad_values() {
    let (world, _) = rigs(false);
    let registry = world.resource::<ComponentRegistry>();
    for bad in [
        json!({}),
        json!({ "follow": {}, "scripted": [0, 0] }),
        json!({ "follow": {}, "fixed": { "at": [0, 0] } }),
        json!({ "follow": {}, "view_height": 0 }),
        json!({ "follow": {}, "view_height": -720 }),
        json!({ "follow": {}, "zoom": 0 }),
        json!({ "follow": {}, "blend": -0.5 }),
        json!({ "follow": {}, "blend": { "time": 0.5, "ease": "wobble" } }),
        json!({ "follow": {}, "bounds": { "min": [10, 0], "max": [0, 10] } }),
        json!({ "follow": {}, "id": "Room Hall" }),
        json!({ "follow": {}, "priority": 0.5 }),
        json!({ "follow": {}, "pan": true }),
        json!({ "follow": { "smoothing": 1.5 } }),
        json!({ "follow": { "smoothing": [0.5, -0.1] } }),
        json!({ "follow": { "dead_zone": [-1, 0] } }),
        json!({ "follow": { "lead": { "from": "velocity", "max": -1 } } }),
        json!({ "follow": { "lead": { "from": "sideways", "max": 96 } } }),
        json!({ "follow": { "lead": { "from": "aim", "max": 96, "smoothing": 2 } } }),
        json!({ "follow": { "target": 3 } }),
        json!({ "fixed": {} }),
        json!({ "fixed": { "at": [0, 0], "corner": [0, 0] } }),
        json!({ "fixed": { "rect": { "min": [0, 0], "max": [100, 0] } } }),
        json!({ "scripted": [0] }),
    ] {
        assert!(
            registry.check_in(CAMERA_RIG, &bad, &world).is_err(),
            "took {bad}"
        );
    }
}

#[test]
fn an_id_rides_beside_the_rig() {
    let (mut world, _) = rigs(false);
    let named = spawn_prefab(
        &mut world,
        json!({ CAMERA_RIG: { "fixed": { "at": [0, 0] }, "id": "vista.cliff", "live": false } }),
    );
    assert_eq!(
        world.get::<CameraRigId>(named).map(CameraRigId::as_str),
        Some("vista.cliff")
    );
    assert!(!world.get::<CameraRig>(named).unwrap().is_live());
    let plain = spawn_prefab(&mut world, json!({ CAMERA_RIG: { "follow": {} } }));
    assert!(world.get::<CameraRig>(plain).is_some());
    assert!(
        world.get::<CameraRigId>(plain).is_none(),
        "no ID, none kept"
    );
    assert_eq!(CameraRigId::new("room.hall").as_str(), "room.hall");
    assert!(std::panic::catch_unwind(|| CameraRigId::new("Room")).is_err());
}

#[test]
fn the_aim_source_reads_the_kits_aim() {
    let (mut world, mut schedule) = rigs(true);
    let aim = *world.resource::<CameraAim>();
    let mut intent = Intent::default();
    intent.set_aim(Some(Vec2::new(0.0, 3.0)));
    let gunner = world.spawn_with((Aim::intent(), intent, Position(Vec2::ZERO)));
    let bare = world.spawn_with((Position(Vec2::ZERO),));
    assert_eq!((aim.0)(&world, gunner), None, "no step yet");
    schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
    assert_eq!((aim.0)(&world, gunner), Some(Vec2::Y), "its unit direction");
    assert_eq!((aim.0)(&world, bare), None, "no aim");
    // A game's own source, inserted first, stays.
    let mut world = World::new();
    world.insert_resource(CameraAim(|_, _| Some(Vec2::X)));
    let mut schedule = Schedule::new();
    CameraRigPlugin.build(&mut schedule, &mut world);
    assert_eq!(
        (world.resource::<CameraAim>().0)(&world, bare),
        Some(Vec2::X)
    );
}
