use glam::Vec2;
use tungsten::WindowSize;
use tungsten::camera_update_system;
use tungsten::core::{
    CameraBounds, CameraController, CameraRig, CameraState, FixedRig, FollowRig, Time, Transform,
    World,
};

fn seed_world() -> World {
    let mut world = World::new();
    world.insert_resource(CameraState::new());
    world.insert_resource(CameraController::default());
    let mut time = Time::new();
    time.advance_frame(1.0 / 60.0);
    world.insert_resource(time);
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

    world.resource_mut::<CameraController>().rig =
        CameraRig::follow(FollowRig::of(player).with_smoothing(Vec2::splat(0.5)));

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

    world.resource_mut::<CameraController>().rig = CameraRig::follow(FollowRig::of(player))
        .with_bounds(CameraBounds::new(Vec2::ZERO, Vec2::new(768.0, 288.0)));

    camera_update_system(&mut world);
    let camera = world.get_resource::<CameraState>().unwrap();
    assert_eq!(camera.position, Vec2::new(288.0, 0.0));
}

#[test]
fn a_fixed_rig_holds_its_corner_at_its_zoom() {
    let mut world = seed_world();
    world.resource_mut::<CameraController>().rig =
        CameraRig::fixed(FixedRig::Corner(Vec2::new(32.0, 48.0))).with_zoom(3.0);

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
    let expected =
        glam::camera::rh::proj::directx::orthographic(0.0, 1280.0, 720.0, 0.0, -1.0, 1.0);
    assert_eq!(got, expected);
}

#[test]
fn a_rigs_zoom_applies_whatever_the_game_writes_each_frame() {
    // A game that writes the state's zoom each frame draws at the rig's,
    // never the product (A1's base-squared trap).
    let mut world = seed_world();
    for zoom in [1.0, 1.25, 0.75] {
        world.resource_mut::<CameraController>().rig.zoom = zoom;
        for _ in 0..2 {
            world.get_resource_mut::<CameraState>().unwrap().zoom = 3.0;
            camera_update_system(&mut world);
            assert_eq!(world.resource::<CameraState>().zoom, zoom);
        }
    }
}

#[test]
fn shake_is_deterministic_for_identical_worlds() {
    let mut world_a = seed_world();
    let mut world_b = seed_world();

    for world in [&mut world_a, &mut world_b] {
        let controller = world.get_resource_mut::<CameraController>().unwrap();
        controller.rig = CameraRig::fixed(FixedRig::Corner(Vec2::new(120.0, 75.0)));
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

#[test]
fn smoothing_converges_the_same_at_60_and_120_hz() {
    // B5: one second of smoothing toward a fixed target ends at the same place
    // at 60 and 120 Hz. At 0.05 about 5% of the distance is left after a
    // second at 60 Hz; at 0.5 both rates reach f32 precision within the
    // second and cannot show a difference.
    let run = |frames: usize, dt: f32| {
        let mut world = seed_world();
        let mut time = Time::new();
        time.advance_frame(dt);
        world.insert_resource(time);
        let target = world.spawn();
        world.insert(target, Transform::from_position(Vec2::new(300.0, 150.0)));
        world.resource_mut::<CameraController>().rig =
            CameraRig::follow(FollowRig::of(target).with_smoothing(Vec2::splat(0.05)));
        for _ in 0..frames {
            camera_update_system(&mut world);
        }
        world.get_resource::<CameraState>().unwrap().position
    };
    let at_60 = run(60, 1.0 / 60.0);
    let at_120 = run(120, 1.0 / 120.0);
    let delta = (at_60 - at_120).abs();
    assert!(
        delta.x <= 1e-2 && delta.y <= 1e-2,
        "60 Hz ended at {at_60:?}, 120 Hz at {at_120:?}"
    );
}
