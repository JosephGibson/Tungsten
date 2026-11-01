//! D-042 physics debug overlay reads `Position + Collider`, not `Transform`,
//! drawn at the point `physics_sync` draws the body (`D-137`), and the last
//! step's tile outline.

use tungsten_core::physics::{
    BodyKind, Collider, PhysicsBuffers, Position, PrevPosition, RigidBody, Shape,
};
use tungsten_core::{ActionMap, DebugDraw, Entity, InputState, Time, World};

/// Physics debug overlay config. A kinematic body or a sensor is outlined in
/// its kind's colour, whatever its shape; every other collider in its
/// shape's colour. Each exposed face of the tile outline is a line in the
/// box colour, a one-way tile's in the sensor colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsDebugOverlay {
    /// Outlines are drawn.
    pub enabled: bool,
    /// Box colliders of static and dynamic bodies and of colliders without
    /// a body, and the tile outline's faces.
    pub color_aabb: [f32; 4],
    /// Circle colliders of static and dynamic bodies and of colliders
    /// without a body.
    pub color_circle: [f32; 4],
    /// Kinematic bodies, either shape.
    pub color_kinematic: [f32; 4],
    /// Sensors, either shape, and one-way tiles' faces.
    pub color_sensor: [f32; 4],
    /// Outline width.
    pub thickness: f32,
}

impl Default for PhysicsDebugOverlay {
    fn default() -> Self {
        Self {
            enabled: false,
            color_aabb: [0.0, 1.0, 0.0, 0.9],
            color_circle: [0.0, 0.8, 1.0, 0.9],
            color_kinematic: [1.0, 0.55, 0.0, 0.9],
            color_sensor: [1.0, 0.25, 0.9, 0.9],
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
/// `Position`. Then a line for each exposed face of the last step's tile
/// outline ([`PhysicsBuffers::terrain_faces`]).
pub(crate) fn physics_debug_emit_system(world: &mut World) {
    let Some(overlay) = world.get_resource::<PhysicsDebugOverlay>() else {
        return;
    };
    if !overlay.enabled {
        return;
    }
    let color_aabb = overlay.color_aabb;
    let color_circle = overlay.color_circle;
    let color_kinematic = overlay.color_kinematic;
    let color_sensor = overlay.color_sensor;
    let thickness = overlay.thickness;
    let alpha = world
        .get_resource::<Time>()
        .filter(|time| time.interpolate())
        .map(Time::alpha);

    let mut emits: Vec<DebugEmit> = Vec::new();
    for (_entity, position, collider, prev, body) in world.query::<(
        Entity,
        &Position,
        &Collider,
        Option<&PrevPosition>,
        Option<&RigidBody>,
    )>() {
        let drawn = match (alpha, prev) {
            (Some(alpha), Some(prev)) => prev.0 + (position.0 - prev.0) * alpha,
            _ => position.0,
        };
        let center = drawn + collider.offset;
        let kind_color = match body.map(|body| body.kind) {
            Some(BodyKind::Kinematic) => Some(color_kinematic),
            Some(BodyKind::Sensor) => Some(color_sensor),
            _ => None,
        };
        // A circle, and any other shape as its box.
        if let Shape::Circle { radius } = collider.shape {
            emits.push(DebugEmit::Circle {
                center,
                radius,
                color: kind_color.unwrap_or(color_circle),
                thickness,
            });
        } else {
            let half = collider.shape.half_extents();
            emits.push(DebugEmit::Aabb {
                min: center - half,
                max: center + half,
                color: kind_color.unwrap_or(color_aabb),
                thickness,
            });
        }
    }
    if let Some(buffers) = world.get_resource::<PhysicsBuffers>() {
        emits.extend(buffers.terrain_faces().map(|face| DebugEmit::Line {
            a: face.start,
            b: face.end,
            color: if face.one_way {
                color_sensor
            } else {
                color_aabb
            },
            thickness,
        }));
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
            DebugEmit::Line {
                a,
                b,
                color,
                thickness,
            } => debug.draw_line(a, b, color, thickness),
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
    Line {
        a: glam::Vec2,
        b: glam::Vec2,
        color: [f32; 4],
        thickness: f32,
    },
}

#[cfg(test)]
#[path = "tests/physics_debug.rs"]
mod tests;
