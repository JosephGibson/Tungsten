use glam::Vec2;
use std::f32::consts::FRAC_PI_2;
use tungsten_core::{
    CameraBounds, CameraController, CameraMode, CameraState, Time, Transform, World,
};

use crate::WindowSize;
use crate::camera::camera_update_system;

fn make_world(dt: f32) -> World {
    let mut world = World::new();
    let mut time = Time::new();
    time.advance_frame(dt);
    world.insert_resource(time);
    world.insert_resource(CameraState::new());
    world.insert_resource(WindowSize {
        width: 800,
        height: 600,
    });
    world
}

fn camera_position(world: &World) -> Vec2 {
    world.get_resource::<CameraState>().unwrap().position
}

#[test]
fn follow_mode_recenters_target_without_dead_zone() {
    let mut world = make_world(0.016);
    let target = world.spawn();
    world.insert(target, Transform::from_position(Vec2::new(1000.0, 500.0)));
    let mut controller = CameraController::default();
    controller.mode = CameraMode::Follow(target);
    world.insert_resource(controller);

    camera_update_system(&mut world);

    // Top-left anchor: target lands at the viewport center.
    assert_eq!(camera_position(&world), Vec2::new(600.0, 200.0));
}

#[test]
fn dead_zone_holds_camera_until_target_leaves_it() {
    let mut world = make_world(0.016);
    let target = world.spawn();
    world.insert(target, Transform::from_position(Vec2::new(420.0, 310.0)));
    let mut controller = CameraController::default();
    controller.mode = CameraMode::Follow(target);
    controller.dead_zone_size = Vec2::new(200.0, 150.0);
    world.insert_resource(controller);

    camera_update_system(&mut world);
    assert_eq!(camera_position(&world), Vec2::ZERO);

    world.get_mut::<Transform>(target).unwrap().position = Vec2::new(900.0, 310.0);
    camera_update_system(&mut world);
    assert_eq!(camera_position(&world).x, 900.0 - 300.0 - 200.0);
}

#[test]
fn bounds_clamp_still_applies_in_follow_mode() {
    let mut world = make_world(0.016);
    let target = world.spawn();
    world.insert(target, Transform::from_position(Vec2::new(5000.0, 5000.0)));
    let mut controller = CameraController::default();
    controller.mode = CameraMode::Follow(target);
    controller.bounds = Some(CameraBounds {
        min: Vec2::ZERO,
        max: Vec2::new(1000.0, 800.0),
    });
    world.insert_resource(controller);

    camera_update_system(&mut world);

    assert_eq!(camera_position(&world), Vec2::new(200.0, 200.0));
}

#[test]
fn shake_offset_reaches_camera_state() {
    let mut world = make_world(0.0);
    let mut controller = CameraController::default();
    controller.shake_max_offset = Vec2::new(12.0, 0.0);
    controller.shake_trauma = 1.0;
    controller.shake_phase = FRAC_PI_2;
    world.insert_resource(controller);

    camera_update_system(&mut world);

    let position = camera_position(&world);
    assert!((position.x - 12.0).abs() <= 1e-5, "position={position:?}");
}

#[test]
fn shake_does_not_accumulate_into_base_position_across_frames() {
    let mut world = make_world(0.0);
    let mut controller = CameraController::default();
    controller.shake_amplitude = Vec2::new(10.0, 10.0);
    controller.shake_max_offset = Vec2::new(20.0, 20.0);
    controller.shake_trauma = 1.0;
    controller.shake_phase = FRAC_PI_2;
    world.insert_resource(controller);

    // dt == 0 freezes the phase, so every frame emits the same offset. Without
    // `resolve_base_position` stripping it, the position would drift by that
    // offset each frame.
    camera_update_system(&mut world);
    let first = camera_position(&world);
    for _ in 0..8 {
        camera_update_system(&mut world);
    }
    assert_eq!(camera_position(&world), first);
}

#[test]
fn zero_trauma_leaves_position_at_the_base() {
    let mut world = make_world(0.016);
    world.insert_resource(CameraController::default());

    camera_update_system(&mut world);

    assert_eq!(camera_position(&world), Vec2::ZERO);
}

#[test]
fn missing_controller_leaves_camera_untouched() {
    let mut world = make_world(0.016);
    world.get_resource_mut::<CameraState>().unwrap().position = Vec2::new(5.0, 7.0);
    camera_update_system(&mut world);
    assert_eq!(camera_position(&world), Vec2::new(5.0, 7.0));
}
