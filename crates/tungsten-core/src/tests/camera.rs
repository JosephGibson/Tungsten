use super::*;
use std::f32::consts::FRAC_PI_2;

fn assert_vec2_close(actual: Vec2, expected: Vec2) {
    let delta = (actual - expected).abs();
    assert!(
        delta.x <= 1e-5 && delta.y <= 1e-5,
        "expected {expected:?}, got {actual:?}, delta={delta:?}"
    );
}

#[test]
fn default_matches_pre_m10_ortho() {
    // Pre-M10 default ortho compatibility.
    let cam = CameraState::new();
    let got = cam.view_projection(1280.0, 720.0);
    let expected = Mat4::orthographic_rh(0.0, 1280.0, 720.0, 0.0, -1.0, 1.0);
    assert_eq!(got, expected);
}

#[test]
fn translation_shifts_view() {
    let mut cam = CameraState::new();
    cam.position = Vec2::new(100.0, 50.0);
    let (min, max) = cam.visible_world_aabb(800.0, 600.0);
    assert_eq!(min, Vec2::new(100.0, 50.0));
    assert_eq!(max, Vec2::new(900.0, 650.0));
}

#[test]
fn zoom_shrinks_visible_area() {
    let mut cam = CameraState::new();
    cam.zoom = 2.0;
    let (min, max) = cam.visible_world_aabb(800.0, 600.0);
    assert_eq!(min, Vec2::ZERO);
    assert_eq!(max, Vec2::new(400.0, 300.0));
}

#[test]
fn zero_zoom_does_not_panic() {
    let mut cam = CameraState::new();
    cam.zoom = 0.0;
    let _ = cam.view_projection(800.0, 600.0);
    let _ = cam.visible_world_aabb(800.0, 600.0);
}

#[test]
fn rotated_visible_world_aabb_over_covers_rotated_view() {
    let mut cam = CameraState::new();
    cam.position = Vec2::new(10.0, 20.0);
    cam.rotation = FRAC_PI_2;
    let (min, max) = cam.visible_world_aabb(4.0, 2.0);
    assert_vec2_close(min, Vec2::new(8.0, 20.0));
    assert_vec2_close(max, Vec2::new(10.0, 24.0));
}

#[test]
fn bounds_clamp_pins_camera_to_world_rect() {
    let bounds = CameraBounds {
        min: Vec2::ZERO,
        max: Vec2::new(100.0, 80.0),
    };
    let clamped = bounds.clamp_position(Vec2::new(90.0, 70.0), 40.0, 20.0, 1.0, 0.0);
    assert_eq!(clamped, Vec2::new(60.0, 60.0));
}
