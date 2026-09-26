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
    let expected =
        glam::camera::rh::proj::directx::orthographic(0.0, 1280.0, 720.0, 0.0, -1.0, 1.0);
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

#[test]
fn controller_default_shake_is_inert() {
    let c = CameraController::default();
    assert_eq!(c.shake_trauma, 0.0);
    assert_eq!(c.shake_decay, 1.0);
    assert_eq!(c.shake_max_offset, Vec2::ZERO);
    assert_eq!(c.shake_offset(), Vec2::ZERO);
}

#[test]
fn add_trauma_accumulates_and_saturates() {
    let mut c = CameraController::default();
    c.add_trauma(0.3);
    assert!((c.shake_trauma - 0.3).abs() <= 1e-6);
    c.add_trauma(0.4);
    assert!((c.shake_trauma - 0.7).abs() <= 1e-6);
    c.add_trauma(5.0);
    assert_eq!(c.shake_trauma, 1.0);
    c.add_trauma(-10.0);
    assert_eq!(c.shake_trauma, 0.0);
}

#[test]
fn shake_offset_reproduces_pre_m30_sine_at_zero_trauma() {
    let c = CameraController {
        shake_amplitude: Vec2::new(6.0, 4.0),
        shake_phase: 0.9,
        shake_max_offset: Vec2::splat(100.0),
        ..Default::default()
    };
    // Pre-M30 expression, verbatim.
    let expected = Vec2::new(
        c.shake_amplitude.x * c.shake_phase.sin(),
        c.shake_amplitude.y * (c.shake_phase + FRAC_PI_2).sin(),
    );
    assert_eq!(c.shake_offset(), expected);
}

#[test]
fn shake_offset_trauma_falloff_is_quadratic() {
    let mut c = CameraController {
        shake_max_offset: Vec2::new(16.0, 16.0),
        shake_phase: FRAC_PI_2,
        shake_trauma: 1.0,
        ..Default::default()
    };
    let full = c.shake_offset().x;
    c.shake_trauma = 0.5;
    let half = c.shake_offset().x;
    assert!((full - 16.0).abs() <= 1e-5, "full={full}");
    // Trauma-squared: half trauma is a quarter of the offset.
    assert!((half - 4.0).abs() <= 1e-5, "half={half}");
}

#[test]
fn shake_offset_scales_max_offset_per_axis() {
    let mut c = CameraController {
        shake_max_offset: Vec2::new(10.0, 40.0),
        shake_trauma: 1.0,
        shake_phase: FRAC_PI_2,
        ..Default::default()
    };
    let offset = c.shake_offset();
    // sin(PI/2) == 1 on x, sin(PI) == 0 on y.
    assert_vec2_close(offset, Vec2::new(10.0, 0.0));
    c.shake_phase = 0.0;
    assert_vec2_close(c.shake_offset(), Vec2::new(0.0, 40.0));
}

#[test]
fn shake_offset_adds_trauma_to_amplitude() {
    let c = CameraController {
        shake_amplitude: Vec2::new(2.0, 0.0),
        shake_max_offset: Vec2::new(8.0, 0.0),
        shake_trauma: 1.0,
        shake_phase: FRAC_PI_2,
        ..Default::default()
    };
    assert_vec2_close(c.shake_offset(), Vec2::new(10.0, 0.0));
}

#[test]
fn shake_offset_clamps_out_of_range_trauma() {
    let mut c = CameraController {
        shake_max_offset: Vec2::new(10.0, 0.0),
        shake_phase: FRAC_PI_2,
        shake_trauma: 4.0,
        ..Default::default()
    };
    assert_vec2_close(c.shake_offset(), Vec2::new(10.0, 0.0));
    c.shake_trauma = -1.0;
    assert_vec2_close(c.shake_offset(), Vec2::ZERO);
}
