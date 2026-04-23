use glam::{Mat4, Vec2};
use tungsten::camera_update_system;
use tungsten::core::{
    CameraBounds, CameraController, CameraMode, CameraState, DeltaTime, Transform, World,
};
use tungsten::WindowSize;

fn seed_world() -> World {
    let mut world = World::new();
    world.insert_resource(CameraState::new());
    world.insert_resource(CameraController::default());
    world.insert_resource(DeltaTime { dt: 1.0 / 60.0 });
    world.insert_resource(WindowSize {
        width: 800,
        height: 600,
    });
    world
}

fn assert_vec2_close(actual: Vec2, expected: Vec2) {
    let delta = (actual - expected).abs();
    assert!(
        delta.x <= 1e-5 && delta.y <= 1e-5,
        "expected {expected:?}, got {actual:?}, delta={delta:?}"
    );
}

#[test]
fn follow_camera_tracks_target_across_fixed_dt_frames() {
    let mut world = seed_world();
    let player = world.spawn();
    world.insert(player, Transform::from_position(Vec2::new(300.0, 150.0)));

    {
        let controller = world.get_resource_mut::<CameraController>().unwrap();
        controller.mode = CameraMode::Follow(player);
        controller.dead_zone_size = Vec2::ZERO;
        controller.smoothing_factor = 0.5;
    }

    camera_update_system(&mut world);
    let after_first = world.get_resource::<CameraState>().unwrap().position;
    assert_vec2_close(after_first, Vec2::new(-50.0, -75.0));

    world.get_mut::<Transform>(player).unwrap().position = Vec2::new(500.0, 300.0);
    camera_update_system(&mut world);
    let after_second = world.get_resource::<CameraState>().unwrap().position;
    assert_vec2_close(after_second, Vec2::new(25.0, -37.5));
}

#[test]
fn bounds_clamp_limits_follow_camera_to_world_rect() {
    let mut world = seed_world();
    world.get_resource_mut::<WindowSize>().unwrap().width = 480;
    world.get_resource_mut::<WindowSize>().unwrap().height = 288;

    let player = world.spawn();
    world.insert(
        player,
        Transform::from_position(Vec2::new(9_999.0, 9_999.0)),
    );

    {
        let controller = world.get_resource_mut::<CameraController>().unwrap();
        controller.mode = CameraMode::Follow(player);
        controller.dead_zone_size = Vec2::ZERO;
        controller.smoothing_factor = 1.0;
        controller.bounds = Some(CameraBounds {
            min: Vec2::ZERO,
            max: Vec2::new(768.0, 288.0),
        });
    }

    camera_update_system(&mut world);
    let camera = world.get_resource::<CameraState>().unwrap();
    assert_eq!(camera.position, Vec2::new(288.0, 0.0));
}

#[test]
fn scripted_mode_preserves_pose_and_applies_zoom_multiplier() {
    let mut world = seed_world();
    {
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.position = Vec2::new(32.0, 48.0);
        camera.zoom = 2.0;
    }
    {
        let controller = world.get_resource_mut::<CameraController>().unwrap();
        controller.mode = CameraMode::Scripted;
        controller.zoom_multiplier = 1.5;
    }

    camera_update_system(&mut world);
    let camera = world.get_resource::<CameraState>().unwrap();
    assert_eq!(camera.position, Vec2::new(32.0, 48.0));
    assert_eq!(camera.zoom, 3.0);
}

#[test]
fn zero_rotation_camera_remains_pre_m10_ortho_through_shared_path() {
    let mut world = seed_world();
    camera_update_system(&mut world);

    let camera = world.get_resource::<CameraState>().unwrap();
    let got = camera.view_projection(1280.0, 720.0);
    let expected = Mat4::orthographic_rh(0.0, 1280.0, 720.0, 0.0, -1.0, 1.0);
    assert_eq!(got, expected);
}

#[test]
fn zoom_multiplier_change_applies_when_gameplay_rewrites_base_each_frame() {
    // Regression: gameplay base-zoom rewrite must not hide multiplier changes.
    let mut world = seed_world();
    let base_zoom: f32 = 3.0;

    for _ in 0..2 {
        world.get_resource_mut::<CameraState>().unwrap().zoom = base_zoom;
        camera_update_system(&mut world);
    }
    let steady = world.get_resource::<CameraState>().unwrap().zoom;
    assert_eq!(steady, base_zoom);

    world
        .get_resource_mut::<CameraController>()
        .unwrap()
        .zoom_multiplier = 1.25;
    world.get_resource_mut::<CameraState>().unwrap().zoom = base_zoom;
    camera_update_system(&mut world);
    let zoomed = world.get_resource::<CameraState>().unwrap().zoom;
    assert_eq!(zoomed, base_zoom * 1.25);

    world
        .get_resource_mut::<CameraController>()
        .unwrap()
        .zoom_multiplier = 0.75;
    world.get_resource_mut::<CameraState>().unwrap().zoom = base_zoom;
    camera_update_system(&mut world);
    let zoomed_out = world.get_resource::<CameraState>().unwrap().zoom;
    assert_eq!(zoomed_out, base_zoom * 0.75);
}

#[test]
fn shake_is_deterministic_for_identical_worlds() {
    let mut world_a = seed_world();
    let mut world_b = seed_world();

    for world in [&mut world_a, &mut world_b] {
        let camera = world.get_resource_mut::<CameraState>().unwrap();
        camera.position = Vec2::new(120.0, 75.0);

        let controller = world.get_resource_mut::<CameraController>().unwrap();
        controller.mode = CameraMode::Scripted;
        controller.shake_amplitude = Vec2::new(4.0, 2.0);
        controller.shake_frequency_hz = 3.0;
        controller.shake_phase = 0.25;
    }

    let mut positions_a = Vec::new();
    let mut positions_b = Vec::new();
    for _ in 0..5 {
        camera_update_system(&mut world_a);
        camera_update_system(&mut world_b);
        positions_a.push(world_a.get_resource::<CameraState>().unwrap().position);
        positions_b.push(world_b.get_resource::<CameraState>().unwrap().position);
    }

    assert_eq!(positions_a, positions_b);
    assert_ne!(positions_a[0], Vec2::new(120.0, 75.0));
}
