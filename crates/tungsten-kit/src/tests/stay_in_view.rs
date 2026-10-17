use super::*;
use serde_json::json;
use tungsten_core::physics::{Collider, PrevPosition, RigidBodyBundle};
use tungsten_core::{Bundle, CameraController, CameraState, Stage, Time, WindowSize};

use crate::spawn_child;

fn rig() -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    let mut schedule = Schedule::new();
    StayInViewPlugin.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    (world, schedule)
}

fn run(world: &mut World, schedule: &mut Schedule) {
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
}

fn position(world: &World, entity: Entity) -> Vec2 {
    world.get::<Position>(entity).unwrap().0
}

fn room() -> StayInView {
    StayInView::region(Vec2::new(64.0, 64.0), Vec2::new(256.0, 192.0)).with_size(Vec2::splat(16.0))
}

#[test]
fn a_box_is_clamped_inside_and_its_outward_speed_stops() {
    let (mut world, mut schedule) = rig();
    let warden = world.spawn_with((
        room(),
        Position(Vec2::new(300.0, 40.0)),
        PrevPosition(Vec2::new(290.0, 45.0)),
        Velocity(Vec2::new(50.0, -20.0)),
    ));
    let inside = world.spawn_with((room(), Position(Vec2::new(100.0, 100.0)), Velocity(Vec2::X)));
    run(&mut world, &mut schedule);
    assert_eq!(position(&world, warden), Vec2::new(240.0, 64.0));
    assert_eq!(world.get::<Velocity>(warden).unwrap().0, Vec2::ZERO);
    assert_eq!(
        world.get::<PrevPosition>(warden).unwrap().0,
        Vec2::new(290.0, 45.0),
        "a clamp keeps the history"
    );
    assert_eq!(position(&world, inside), Vec2::new(100.0, 100.0));
    assert_eq!(world.get::<Velocity>(inside).unwrap().0, Vec2::X);
}

#[test]
fn inward_speed_and_the_other_axis_keep_going() {
    let (mut world, mut schedule) = rig();
    let warden = world.spawn_with((
        room(),
        Position(Vec2::new(30.0, 100.0)),
        Velocity(Vec2::new(40.0, 25.0)),
    ));
    run(&mut world, &mut schedule);
    assert_eq!(position(&world, warden), Vec2::new(64.0, 100.0));
    assert_eq!(
        world.get::<Velocity>(warden).unwrap().0,
        Vec2::new(40.0, 25.0)
    );
}

#[test]
fn a_platformers_own_velocity_stops_on_the_clamped_axis() {
    let (mut world, mut schedule) = rig();
    let mut controller = PlatformerController::new(200.0, 600.0);
    *controller.velocity_mut() = Vec2::new(-150.0, 90.0);
    let hero = world.spawn_with(
        RigidBodyBundle::kinematic(
            Position(Vec2::new(10.0, 100.0)),
            Collider::aabb(Vec2::splat(8.0)),
        )
        .with((controller, room())),
    );
    run(&mut world, &mut schedule);
    let controller = world.get_mut::<PlatformerController>(hero).unwrap();
    assert_eq!(*controller.velocity_mut(), Vec2::new(0.0, 90.0));
}

#[test]
fn a_box_larger_than_the_area_sits_on_its_top_left() {
    let (mut world, mut schedule) = rig();
    let giant = world.spawn_with((
        StayInView::region(Vec2::ZERO, Vec2::new(10.0, 10.0)).with_size(Vec2::splat(40.0)),
        Position(Vec2::new(25.0, -5.0)),
    ));
    run(&mut world, &mut schedule);
    assert_eq!(position(&world, giant), Vec2::ZERO);
}

#[test]
fn a_camera_shake_does_not_move_a_clamped_entity() {
    let (mut world, mut schedule) = rig();
    world.insert_resource(WindowSize {
        width: 640,
        height: 360,
    });
    let mut camera = CameraState::new();
    let mut controller = CameraController::default();
    let fighter = world.spawn_with((
        StayInView::view(8.0).with_size(Vec2::splat(24.0)),
        Position(Vec2::new(608.0, 100.0)),
    ));
    for shake in [Vec2::ZERO, Vec2::new(-12.0, 5.0), Vec2::new(9.0, -7.0)] {
        camera.position = shake;
        controller.record_output_position(camera.position, shake);
        world.insert_resource(camera);
        world.insert_resource(controller);
        run(&mut world, &mut schedule);
        assert_eq!(
            position(&world, fighter),
            Vec2::new(608.0, 100.0),
            "{shake}"
        );
    }
}

#[test]
fn a_child_and_an_entity_without_a_camera_are_left() {
    let (mut world, mut schedule) = rig();
    let parent = world.spawn_with((Transform::default(),));
    let pod = spawn_child(&mut world, parent, Vec2::new(900.0, 0.0), (room(),));
    let ship = world.spawn_with((StayInView::view(0.0), Position(Vec2::new(-500.0, 0.0))));
    run(&mut world, &mut schedule);
    assert_eq!(
        world.get::<Transform>(pod).unwrap().position,
        Vec2::new(900.0, 0.0)
    );
    assert_eq!(position(&world, ship), Vec2::new(-500.0, 0.0));
}

#[test]
fn a_save_round_trips_and_refuses_a_bad_area() {
    let held = room();
    let saved = serde_json::to_value(held).unwrap();
    assert_eq!(serde_json::from_value::<StayInView>(saved).unwrap(), held);
    for refused in [
        json!({}),
        json!({ "view": 1, "axes": "x" }),
        json!({ "view": f64::NAN }),
    ] {
        assert!(
            serde_json::from_value::<StayInView>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}
