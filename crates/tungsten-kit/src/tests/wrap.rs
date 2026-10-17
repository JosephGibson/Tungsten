use super::*;
use serde_json::json;
use tungsten_core::{CameraController, CameraState, Stage, Time, WindowSize};

use crate::spawn_child;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    WrapPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn run(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn rock(world: &mut World, wrap: Wrap, at: Vec2, prev: Vec2) -> Entity {
    world.spawn_with((wrap, Position(at), PrevPosition(prev)))
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

fn arena() -> Wrap {
    Wrap::region(Vec2::ZERO, Vec2::new(640.0, 360.0)).with_size(Vec2::splat(32.0))
}

#[test]
fn a_box_wholly_past_a_side_comes_back_in_from_the_other_just_outside() {
    let (mut world, mut schedule) = rig();
    let right = rock(
        &mut world,
        arena(),
        Vec2::new(641.0, 100.0),
        Vec2::new(639.0, 100.0),
    );
    let up = rock(
        &mut world,
        arena(),
        Vec2::new(300.0, -33.0),
        Vec2::new(300.0, -31.0),
    );
    let leaving = rock(
        &mut world,
        arena(),
        Vec2::new(630.0, 100.0),
        Vec2::new(628.0, 100.0),
    );
    run(&mut world, &mut schedule);
    assert_eq!(position(&world, right), Vec2::new(-31.0, 100.0));
    assert_eq!(
        world.get::<PrevPosition>(right).unwrap().0,
        Vec2::new(-33.0, 100.0)
    );
    assert_eq!(position(&world, up), Vec2::new(300.0, 359.0));
    assert_eq!(
        position(&world, leaving),
        Vec2::new(630.0, 100.0),
        "partly inside"
    );
}

#[test]
fn one_axis_wraps_through_its_own_sides_only() {
    let (mut world, mut schedule) = rig();
    let tunnel = rock(
        &mut world,
        arena().only_x(),
        Vec2::new(-40.0, 400.0),
        Vec2::new(-40.0, 400.0),
    );
    let shaft = rock(
        &mut world,
        arena().only_y(),
        Vec2::new(-40.0, 400.0),
        Vec2::new(-40.0, 400.0),
    );
    run(&mut world, &mut schedule);
    assert_eq!(position(&world, tunnel), Vec2::new(632.0, 400.0));
    assert_eq!(position(&world, shaft), Vec2::new(-40.0, 8.0));
}

#[test]
fn the_view_is_the_cameras_without_its_shake_grown_by_the_margin() {
    let (mut world, mut schedule) = rig();
    world.insert_resource(WindowSize {
        width: 640,
        height: 360,
    });
    let shake = Vec2::new(30.0, 0.0);
    let mut camera = CameraState::new();
    camera.position = Vec2::new(100.0, 0.0) + shake;
    world.insert_resource(camera);
    let mut controller = CameraController::default();
    controller.record_output_position(camera.position, shake);
    world.insert_resource(controller);
    // Past the base view's right side, 740, by its margin and one: inside
    // the shaken view, which ends at 770.
    let ship = rock(
        &mut world,
        Wrap::view(8.0),
        Vec2::new(749.0, 50.0),
        Vec2::new(749.0, 50.0),
    );
    run(&mut world, &mut schedule);
    assert_eq!(position(&world, ship), Vec2::new(749.0 - 656.0, 50.0));
}

#[test]
fn a_view_without_a_camera_and_a_child_are_left() {
    let (mut world, mut schedule) = rig();
    let ship = rock(
        &mut world,
        Wrap::view(0.0),
        Vec2::new(5000.0, 0.0),
        Vec2::ZERO,
    );
    let parent = world.spawn_with((Transform::default(),));
    let pod = spawn_child(&mut world, parent, Vec2::new(900.0, 0.0), (arena(),));
    run(&mut world, &mut schedule);
    assert_eq!(position(&world, ship), Vec2::new(5000.0, 0.0));
    assert_eq!(
        world.get::<Transform>(pod).unwrap().position,
        Vec2::new(900.0, 0.0)
    );
}

#[test]
fn an_entity_without_a_position_wraps_its_transform() {
    let (mut world, mut schedule) = rig();
    let sign = world.spawn_with((Transform::from_position(Vec2::new(-50.0, 10.0)), arena()));
    run(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Transform>(sign).unwrap().position,
        Vec2::new(622.0, 10.0)
    );
}

#[test]
fn a_save_round_trips_and_refuses_a_bad_area_or_size() {
    let wrap = arena().only_y();
    let saved = serde_json::to_value(wrap).unwrap();
    assert_eq!(
        saved,
        json!({ "region": [[0.0, 0.0], [640.0, 360.0]], "size": [32.0, 32.0], "axes": "y" })
    );
    assert_eq!(serde_json::from_value::<Wrap>(saved).unwrap(), wrap);
    for refused in [
        json!({}),
        json!({ "view": 0, "region": [[0, 0], [1, 1]] }),
        json!({ "region": [[5, 0], [1, 1]] }),
        json!({ "view": -1 }),
        json!({ "view": 0, "size": [-1, 0] }),
        json!({ "view": 0, "axes": "z" }),
    ] {
        assert!(
            serde_json::from_value::<Wrap>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[test]
#[should_panic(expected = "lies above")]
fn an_upside_down_region_panics() {
    let _ = Wrap::region(Vec2::new(10.0, 0.0), Vec2::new(0.0, 10.0));
}
