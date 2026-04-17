//! Shared camera update system for the umbrella crate.
//!
//! Gameplay code owns the camera resources (`CameraState` and
//! `CameraController`) and registers this system explicitly wherever the
//! frame order should place camera updates.

use std::f32::consts::{FRAC_PI_2, TAU};

use glam::Vec2;
use tungsten_core::{CameraController, CameraMode, CameraState, DeltaTime, Transform, World};

use crate::WindowSize;

fn solve_follow_position(
    current_position: Vec2,
    target_position: Vec2,
    viewport_size: Vec2,
    dead_zone_size: Vec2,
) -> Vec2 {
    let mut next = current_position;
    let dead_zone_offset = (viewport_size - dead_zone_size) * 0.5;
    let dead_zone_min = current_position + dead_zone_offset;
    let dead_zone_max = dead_zone_min + dead_zone_size;

    if target_position.x < dead_zone_min.x {
        next.x = target_position.x - dead_zone_offset.x;
    } else if target_position.x > dead_zone_max.x {
        next.x = target_position.x - dead_zone_offset.x - dead_zone_size.x;
    }

    if target_position.y < dead_zone_min.y {
        next.y = target_position.y - dead_zone_offset.y;
    } else if target_position.y > dead_zone_max.y {
        next.y = target_position.y - dead_zone_offset.y - dead_zone_size.y;
    }

    next
}

/// Shared engine-side camera system. Reads `CameraController`, `DeltaTime`,
/// `WindowSize`, and a followed entity `Transform`, then writes the
/// authoritative `CameraState` for the frame.
pub fn camera_update_system(world: &mut World) {
    let mut camera = match world.get_resource::<CameraState>().copied() {
        Some(camera) => camera,
        None => return,
    };
    let mut controller = match world.get_resource::<CameraController>().copied() {
        Some(controller) => controller,
        None => return,
    };
    let window = world
        .get_resource::<WindowSize>()
        .copied()
        .unwrap_or(WindowSize {
            width: 1280,
            height: 720,
        });
    let dt = world
        .get_resource::<DeltaTime>()
        .map(|delta| delta.seconds())
        .unwrap_or(0.0);

    let base_position = controller.resolve_base_position(camera.position);
    let base_zoom = controller.resolve_base_zoom(camera.zoom);
    let zoom_multiplier = controller.zoom_multiplier.max(f32::EPSILON);
    let effective_zoom = (base_zoom * zoom_multiplier).max(f32::EPSILON);
    let viewport_size = Vec2::new(
        window.width as f32 / effective_zoom,
        window.height as f32 / effective_zoom,
    );
    let dead_zone_size = controller.dead_zone_size.clamp(Vec2::ZERO, viewport_size);

    let desired_position = match controller.mode {
        CameraMode::Follow(entity) => world
            .get::<Transform>(entity)
            .map(|transform| {
                solve_follow_position(
                    base_position,
                    transform.position,
                    viewport_size,
                    dead_zone_size,
                )
            })
            .unwrap_or(base_position),
        CameraMode::Free | CameraMode::Scripted => base_position,
    };

    let smoothing = controller.smoothing_factor.clamp(0.0, 1.0);
    let mut next_position = base_position.lerp(desired_position, smoothing);

    if let Some(bounds) = controller.bounds {
        next_position = bounds.clamp_position(
            next_position,
            window.width as f32,
            window.height as f32,
            effective_zoom,
            camera.rotation,
        );
    }

    let shake = Vec2::new(
        controller.shake_amplitude.x * controller.shake_phase.sin(),
        controller.shake_amplitude.y * (controller.shake_phase + FRAC_PI_2).sin(),
    );
    camera.position = next_position + shake;
    camera.zoom = effective_zoom;

    controller.shake_phase = (controller.shake_phase
        + controller.shake_frequency_hz.max(0.0) * TAU * dt)
        .rem_euclid(TAU);
    controller.record_output_position(camera.position, shake);
    controller.record_output_zoom(base_zoom, camera.zoom);

    if let Some(camera_state) = world.get_resource_mut::<CameraState>() {
        *camera_state = camera;
    }
    if let Some(camera_controller) = world.get_resource_mut::<CameraController>() {
        *camera_controller = controller;
    }
}
