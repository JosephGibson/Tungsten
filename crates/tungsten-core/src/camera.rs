//! 2D camera state; default matrix preserves pre-M10 y-down pixel space.
//!
//! Text pipeline is screen-space and ignores this camera.

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

/// Camera behavior mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraMode {
    Free,
    Follow(Entity),
    Scripted,
}

/// World-space camera clamp bounds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraBounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl CameraBounds {
    /// Clamp top-left anchor so visible AABB stays within bounds.
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

/// Gameplay-owned camera controller state.
#[derive(Debug, Clone, Copy)]
pub struct CameraController {
    pub mode: CameraMode,
    pub dead_zone_size: Vec2,
    pub smoothing_factor: f32,
    pub bounds: Option<CameraBounds>,
    pub zoom_multiplier: f32,
    pub shake_amplitude: Vec2,
    pub shake_frequency_hz: f32,
    /// Deterministic shake phase.
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
    /// Infer base zoom without compounding multiplier output.
    pub fn resolve_base_zoom(&self, current_zoom: f32) -> f32 {
        let current_zoom = current_zoom.max(f32::EPSILON);
        if self.last_output_zoom == Some(current_zoom) {
            if let Some(base) = self.last_base_zoom {
                return base.max(f32::EPSILON);
            }
        }
        current_zoom
    }

    /// Record zoom output and base.
    pub fn record_output_zoom(&mut self, base_zoom: f32, output_zoom: f32) {
        self.last_base_zoom = Some(base_zoom.max(f32::EPSILON));
        self.last_output_zoom = Some(output_zoom.max(f32::EPSILON));
    }

    /// Infer base position without accumulating shake output.
    pub fn resolve_base_position(&self, current_position: Vec2) -> Vec2 {
        if self.last_output_position == Some(current_position) {
            return current_position - self.last_shake_offset;
        }
        current_position
    }

    /// Record position output and shake offset.
    pub fn record_output_position(&mut self, position: Vec2, shake_offset: Vec2) {
        self.last_output_position = Some(position);
        self.last_shake_offset = shake_offset;
    }
}

/// World-space 2D camera resource.
#[derive(Debug, Clone, Copy)]
pub struct CameraState {
    /// Top-left world position; preserves pre-M10 pixel ortho.
    pub position: Vec2,
    /// Uniform zoom, applied around top-left.
    pub zoom: f32,
    /// Rotation around top-left anchor.
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

    /// View-projection matrix for physical viewport size.
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

    /// Visible world AABB; rotation returns conservative bounding box.
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
#[path = "tests/camera.rs"]
mod tests;
