//! D-042 physics debug overlay reads `Position + Collider`, not `Transform`,
//! drawn at the point `physics_sync` draws the body (`D-137`).

use tungsten_core::physics::{Collider, Position, PrevPosition, Shape};
use tungsten_core::{ActionMap, DebugDraw, Entity, InputState, Time, World};

/// Physics debug overlay config.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsDebugOverlay {
    pub enabled: bool,
    pub color_aabb: [f32; 4],
    pub color_circle: [f32; 4],
    pub thickness: f32,
}

impl Default for PhysicsDebugOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            color_aabb: [0.0, 1.0, 0.0, 0.9],
            color_circle: [0.0, 0.8, 1.0, 0.9],
            thickness: 1.5,
        }
    }
}

impl PhysicsDebugOverlay {
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }
}

/// Toggle on `engine_toggle_physics_debug` edge.
pub(crate) fn physics_debug_toggle_system(world: &mut World) {
    let pressed = {
        let Some(input) = world.get_resource::<InputState>() else {
            return;
        };
        let Some(actions) = world.get_resource::<ActionMap>() else {
            return;
        };
        actions.just_pressed(input, "engine_toggle_physics_debug")
    };
    if pressed && let Some(overlay) = world.get_resource_mut::<PhysicsDebugOverlay>() {
        overlay.toggle();
    }
}

/// Emit collider outlines before `DebugDraw` drain. A body with a
/// [`PrevPosition`] is outlined at `prev + (cur - prev) * alpha` while
/// [`Time::interpolate`] is on, where its sprite is drawn, so the outline
/// does not lead the sprite by up to a step; every other collider at its
/// `Position`.
pub(crate) fn physics_debug_emit_system(world: &mut World) {
    let Some(overlay) = world.get_resource::<PhysicsDebugOverlay>() else {
        return;
    };
    if !overlay.enabled {
        return;
    }
    let color_aabb = overlay.color_aabb;
    let color_circle = overlay.color_circle;
    let thickness = overlay.thickness;
    let alpha = world
        .get_resource::<Time>()
        .filter(|time| time.interpolate())
        .map(Time::alpha);

    let mut emits: Vec<DebugEmit> = Vec::new();
    for (_entity, position, collider, prev) in
        world.query::<(Entity, &Position, &Collider, Option<&PrevPosition>)>()
    {
        let drawn = match (alpha, prev) {
            (Some(alpha), Some(prev)) => prev.0 + (position.0 - prev.0) * alpha,
            _ => position.0,
        };
        let center = drawn + collider.offset;
        match collider.shape {
            Shape::Aabb { half_extents } => {
                emits.push(DebugEmit::Aabb {
                    min: center - half_extents,
                    max: center + half_extents,
                    color: color_aabb,
                    thickness,
                });
            }
            Shape::Circle { radius } => {
                emits.push(DebugEmit::Circle {
                    center,
                    radius,
                    color: color_circle,
                    thickness,
                });
            }
        }
    }

    if emits.is_empty() {
        return;
    }
    let Some(debug) = world.get_resource_mut::<DebugDraw>() else {
        return;
    };
    for emit in emits {
        match emit {
            DebugEmit::Aabb {
                min,
                max,
                color,
                thickness,
            } => debug.draw_aabb(min, max, color, thickness),
            DebugEmit::Circle {
                center,
                radius,
                color,
                thickness,
            } => debug.draw_circle(center, radius, color, thickness),
        }
    }
}

enum DebugEmit {
    Aabb {
        min: glam::Vec2,
        max: glam::Vec2,
        color: [f32; 4],
        thickness: f32,
    },
    Circle {
        center: glam::Vec2,
        radius: f32,
        color: [f32; 4],
        thickness: f32,
    },
}

#[cfg(test)]
#[path = "tests/physics_debug.rs"]
mod tests;
