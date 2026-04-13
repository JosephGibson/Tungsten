//! 2D camera resource. Produces the view-projection matrix that the
//! sprite and quad pipelines upload to their camera uniforms each frame.
//!
//! Coordinate convention matches the pre-M10 implicit camera: y-down
//! pixel space, origin at top-left. A default `Camera2D` (position zero,
//! zoom 1.0) yields the same matrix the sprite pipeline built before M10,
//! so examples 01–08 stay pixel-identical after the refactor.
//!
//! Text rendered via `TextPipeline` is intentionally *not* affected by
//! this camera — glyphon manages its own screen-space viewport. That's
//! usually what game code wants (HUDs and overlays stay put while the
//! world scrolls); if an example needs in-world text it has to handle
//! it itself.

use glam::{Mat4, Vec2};

/// World-space 2D camera. Stored as a resource in the `World`.
#[derive(Debug, Clone, Copy)]
pub struct Camera2D {
    /// World-space position of the camera's *top-left* corner.
    ///
    /// Top-left (rather than centre) keeps the default camera
    /// `(0, 0)` equivalent to the pre-M10 pixel ortho, so existing
    /// examples don't shift by half a screen when the camera is added.
    pub position: Vec2,
    /// Uniform scale factor. `1.0` = 1:1 pixels. `>1.0` zooms in,
    /// `<1.0` zooms out. Applied around the top-left.
    pub zoom: f32,
}

impl Camera2D {
    pub fn new() -> Self {
        Self {
            position: Vec2::ZERO,
            zoom: 1.0,
        }
    }

    /// Compute the view-projection matrix for a given physical viewport size.
    ///
    /// At `position = (0, 0)` and `zoom = 1.0` this equals
    /// `Mat4::orthographic_rh(0.0, width, height, 0.0, -1.0, 1.0)`,
    /// which is what the sprite/quad pipelines used before M10.
    pub fn view_projection(&self, viewport_w: f32, viewport_h: f32) -> Mat4 {
        let zoom = self.zoom.max(f32::EPSILON);
        let half_w = viewport_w / zoom;
        let half_h = viewport_h / zoom;
        let left = self.position.x;
        let right = self.position.x + half_w;
        let top = self.position.y;
        let bottom = self.position.y + half_h;
        Mat4::orthographic_rh(left, right, bottom, top, -1.0, 1.0)
    }

    /// World-space AABB of everything visible through this camera at the
    /// given viewport size. Returns `(min, max)` with `min.x <= max.x` and
    /// `min.y <= max.y` (y-down convention — `min.y` is the top edge).
    pub fn visible_world_aabb(&self, viewport_w: f32, viewport_h: f32) -> (Vec2, Vec2) {
        let zoom = self.zoom.max(f32::EPSILON);
        let w = viewport_w / zoom;
        let h = viewport_h / zoom;
        (
            self.position,
            Vec2::new(self.position.x + w, self.position.y + h),
        )
    }
}

impl Default for Camera2D {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matches_pre_m10_ortho() {
        // The pre-M10 pipeline built:
        //   Mat4::orthographic_rh(0.0, width, height, 0.0, -1.0, 1.0)
        // Camera2D::default() must produce the same matrix, otherwise
        // examples 01–08 would shift after the refactor.
        let cam = Camera2D::new();
        let got = cam.view_projection(1280.0, 720.0);
        let expected = Mat4::orthographic_rh(0.0, 1280.0, 720.0, 0.0, -1.0, 1.0);
        assert_eq!(got, expected);
    }

    #[test]
    fn translation_shifts_view() {
        let mut cam = Camera2D::new();
        cam.position = Vec2::new(100.0, 50.0);
        let (min, max) = cam.visible_world_aabb(800.0, 600.0);
        assert_eq!(min, Vec2::new(100.0, 50.0));
        assert_eq!(max, Vec2::new(900.0, 650.0));
    }

    #[test]
    fn zoom_shrinks_visible_area() {
        let mut cam = Camera2D::new();
        cam.zoom = 2.0;
        let (min, max) = cam.visible_world_aabb(800.0, 600.0);
        assert_eq!(min, Vec2::ZERO);
        assert_eq!(max, Vec2::new(400.0, 300.0));
    }

    #[test]
    fn zero_zoom_does_not_panic() {
        let mut cam = Camera2D::new();
        cam.zoom = 0.0;
        // Should clamp internally, not divide by zero.
        let _ = cam.view_projection(800.0, 600.0);
        let _ = cam.visible_world_aabb(800.0, 600.0);
    }
}
