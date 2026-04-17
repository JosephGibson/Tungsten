//! 2D camera state and controller data. The render path still consumes a
//! single view-projection matrix each frame, but M16 centralizes camera
//! ownership and follow behavior behind shared engine types.
//!
//! Coordinate convention matches the pre-M10 implicit camera: y-down
//! pixel space, origin at top-left. A default [`CameraState`] (position
//! zero, zoom 1.0, rotation 0.0) yields the same matrix the sprite
//! pipeline built before M10, so examples 01–08 stay pixel-identical
//! after the refactor.
//!
//! Text rendered via `TextPipeline` is intentionally *not* affected by
//! this camera — glyphon manages its own screen-space viewport. That's
//! usually what game code wants (HUDs and overlays stay put while the
//! world scrolls); if an example needs in-world text it has to handle
//! it itself.

use glam::{Mat4, Vec2, Vec3};

use crate::ecs::Entity;

fn rotate_vec(vec: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(vec.x * cos - vec.y * sin, vec.x * sin + vec.y * cos)
}

fn viewport_size(viewport_w: f32, viewport_h: f32, zoom: f32) -> Vec2 {
    let zoom = zoom.max(f32::EPSILON);
    Vec2::new(viewport_w / zoom, viewport_h / zoom)
}

fn rotated_view_extents(view_size: Vec2, rotation: f32) -> (Vec2, Vec2) {
    let corners = [
        Vec2::ZERO,
        rotate_vec(Vec2::new(view_size.x, 0.0), rotation),
        rotate_vec(Vec2::new(0.0, view_size.y), rotation),
        rotate_vec(view_size, rotation),
    ];
    let mut min = corners[0];
    let mut max = corners[0];
    for corner in corners.into_iter().skip(1) {
        min = min.min(corner);
        max = max.max(corner);
    }
    (min, max)
}

/// Camera behavior mode selected by gameplay code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Free,
    Follow(Entity),
    Scripted,
}

/// World-space bounds that camera movement can clamp against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraBounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl CameraBounds {
    /// Clamp the camera's top-left anchor so the visible camera AABB stays
    /// within the bounds rectangle. If the visible area is larger than the
    /// bounds on an axis, the camera pins to `min` on that axis.
    pub fn clamp_position(
        &self,
        position: Vec2,
        viewport_w: f32,
        viewport_h: f32,
        zoom: f32,
        rotation: f32,
    ) -> Vec2 {
        let view_size = viewport_size(viewport_w, viewport_h, zoom);
        let (offset_min, offset_max) = rotated_view_extents(view_size, rotation);
        let min_x = self.min.x - offset_min.x;
        let min_y = self.min.y - offset_min.y;
        let max_x = (self.max.x - offset_max.x).max(min_x);
        let max_y = (self.max.y - offset_max.y).max(min_y);
        Vec2::new(
            position.x.clamp(min_x, max_x),
            position.y.clamp(min_y, max_y),
        )
    }
}

/// Controller-side camera data owned by gameplay code and consumed by the
/// umbrella crate's shared camera update system.
#[derive(Debug, Clone, Copy)]
pub struct CameraController {
    pub mode: CameraMode,
    pub dead_zone_size: Vec2,
    pub smoothing_factor: f32,
    pub bounds: Option<CameraBounds>,
    pub zoom_multiplier: f32,
    pub shake_amplitude: Vec2,
    pub shake_frequency_hz: f32,
    /// Phase in radians for deterministic shake sampling.
    pub shake_phase: f32,
    last_output_zoom: Option<f32>,
    last_base_zoom: Option<f32>,
    last_output_position: Option<Vec2>,
    last_shake_offset: Vec2,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            mode: CameraMode::Free,
            dead_zone_size: Vec2::ZERO,
            smoothing_factor: 1.0,
            bounds: None,
            zoom_multiplier: 1.0,
            shake_amplitude: Vec2::ZERO,
            shake_frequency_hz: 0.0,
            shake_phase: 0.0,
            last_output_zoom: None,
            last_base_zoom: None,
            last_output_position: None,
            last_shake_offset: Vec2::ZERO,
        }
    }
}

impl CameraController {
    /// Infer the current base zoom from the camera state, avoiding
    /// multiplier compounding when no system rewrites the base zoom.
    ///
    /// When `current_zoom` equals the value we wrote to `CameraState` last
    /// frame, gameplay has not overridden the base, so we carry over the
    /// base zoom recorded then. Carrying over the recorded base (rather
    /// than dividing by the current multiplier) keeps the result stable
    /// when `zoom_multiplier` changes between frames.
    pub fn resolve_base_zoom(&self, current_zoom: f32) -> f32 {
        let current_zoom = current_zoom.max(f32::EPSILON);
        if self.last_output_zoom == Some(current_zoom) {
            if let Some(base) = self.last_base_zoom {
                return base.max(f32::EPSILON);
            }
        }
        current_zoom
    }

    /// Record the last zoom value written back into [`CameraState`] and the
    /// base zoom that produced it.
    pub fn record_output_zoom(&mut self, base_zoom: f32, output_zoom: f32) {
        self.last_base_zoom = Some(base_zoom.max(f32::EPSILON));
        self.last_output_zoom = Some(output_zoom.max(f32::EPSILON));
    }

    /// Infer the current base position from the camera state, avoiding
    /// shake accumulation when no system rewrites the base camera pose.
    pub fn resolve_base_position(&self, current_position: Vec2) -> Vec2 {
        if self.last_output_position == Some(current_position) {
            return current_position - self.last_shake_offset;
        }
        current_position
    }

    /// Record the last position written back into [`CameraState`] and the
    /// shake offset that was applied to produce it.
    pub fn record_output_position(&mut self, position: Vec2, shake_offset: Vec2) {
        self.last_output_position = Some(position);
        self.last_shake_offset = shake_offset;
    }
}

/// World-space 2D camera. Stored as a resource in the `World`.
#[derive(Debug, Clone, Copy)]
pub struct CameraState {
    /// World-space position of the camera's *top-left* corner.
    ///
    /// Top-left (rather than centre) keeps the default camera
    /// `(0, 0)` equivalent to the pre-M10 pixel ortho, so existing
    /// examples don't shift by half a screen when the camera is added.
    pub position: Vec2,
    /// Uniform scale factor. `1.0` = 1:1 pixels. `>1.0` zooms in,
    /// `<1.0` zooms out. Applied around the top-left.
    pub zoom: f32,
    /// Rotation in radians around [`Self::position`], which remains the
    /// camera's top-left anchor so the D-032 default-matrix invariant holds.
    pub rotation: f32,
}

impl CameraState {
    pub fn new() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
            rotation: 0.0,
        }
    }

    /// Compute the view-projection matrix for a given physical viewport size.
    ///
    /// At `position = (0, 0)` and `zoom = 1.0` this equals
    /// `Mat4::orthographic_rh(0.0, width, height, 0.0, -1.0, 1.0)`,
    /// which is what the sprite/quad pipelines used before M10.
    pub fn view_projection(&self, viewport_w: f32, viewport_h: f32) -> Mat4 {
        let zoom = self.zoom.max(f32::EPSILON);
        if self.rotation == 0.0 {
            let half_w = viewport_w / zoom;
            let half_h = viewport_h / zoom;
            let left = self.position.x;
            let right = self.position.x + half_w;
            let top = self.position.y;
            let bottom = self.position.y + half_h;
            return Mat4::orthographic_rh(left, right, bottom, top, -1.0, 1.0);
        }

        let ortho = Mat4::orthographic_rh(0.0, viewport_w, viewport_h, 0.0, -1.0, 1.0);
        let scale = Mat4::from_scale(Vec3::new(zoom, zoom, 1.0));
        let rotate = Mat4::from_rotation_z(-self.rotation);
        let translate = Mat4::from_translation(Vec3::new(-self.position.x, -self.position.y, 0.0));
        ortho * scale * rotate * translate
    }

    /// World-space AABB of everything visible through this camera at the
    /// given viewport size. Under non-zero rotation this is the axis-aligned
    /// bounding box of the rotated view rectangle, so callers like tilemap
    /// extraction may intentionally over-cover slightly.
    pub fn visible_world_aabb(&self, viewport_w: f32, viewport_h: f32) -> (Vec2, Vec2) {
        let view_size = viewport_size(viewport_w, viewport_h, self.zoom);
        let (offset_min, offset_max) = rotated_view_extents(view_size, self.rotation);
        (self.position + offset_min, self.position + offset_max)
    }
}

impl Default for CameraState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
        // The pre-M10 pipeline built:
        //   Mat4::orthographic_rh(0.0, width, height, 0.0, -1.0, 1.0)
        // CameraState::default() must produce the same matrix, otherwise
        // examples 01–08 would shift after the refactor.
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
        // Should clamp internally, not divide by zero.
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
}
