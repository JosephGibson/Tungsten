//! Physics debug overlay (M21, `F1`). Emits `DebugDraw` commands outlining
//! every `Position + Collider` entity in the world so collision geometry
//! can be sanity-checked at runtime without touching render code.
//!
//! The overlay reads from physics components (`Position`, `Collider`) — not
//! `Transform` — so what you see matches the authoritative collision state
//! per D-042. When `enabled = false` the emit system is a no-op.

use tungsten_core::physics::{Collider, Position, Shape};
use tungsten_core::{ActionMap, DebugDraw, InputState, World};

/// Per-overlay configuration. Colors are linear RGBA `[0..1]`; thickness is
/// world-space width (one world unit == one pixel at camera zoom 1x).
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

/// Engine system: flips `PhysicsDebugOverlay.enabled` on
/// `engine_toggle_physics_debug` action edge. Registered ahead of the HUD
/// toggle so the input edge is consumed on the same frame it arrives.
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
    if pressed {
        if let Some(overlay) = world.get_resource_mut::<PhysicsDebugOverlay>() {
            overlay.toggle();
        }
    }
}

/// Engine extract-stage system: walks every `(Position, Collider)` entity
/// and pushes outline commands into `DebugDraw`. Call this before the
/// DebugDraw drain in `App::render_redraw` so the commands reach the
/// renderer on the same frame they are produced.
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

    let mut emits: Vec<DebugEmit> = Vec::new();
    for (_entity, position, collider) in world.query2::<Position, Collider>() {
        let center = position.0 + collider.offset;
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
mod tests {
    use super::*;
    use glam::Vec2;
    use tungsten_core::input::KeyCode;

    #[test]
    fn emit_system_is_noop_when_disabled() {
        let mut world = World::new();
        world.insert_resource(DebugDraw::new());
        world.insert_resource(PhysicsDebugOverlay::default());
        let e = world.spawn();
        world.insert(e, Position(Vec2::new(4.0, 6.0)));
        world.insert(e, Collider::aabb(Vec2::splat(2.0)));

        physics_debug_emit_system(&mut world);

        assert_eq!(world.get_resource::<DebugDraw>().unwrap().len(), 0);
    }

    #[test]
    fn emit_system_pushes_one_command_per_collider_when_enabled() {
        let mut world = World::new();
        world.insert_resource(DebugDraw::new());
        world.insert_resource(PhysicsDebugOverlay {
            enabled: true,
            ..Default::default()
        });

        let a = world.spawn();
        world.insert(a, Position(Vec2::new(0.0, 0.0)));
        world.insert(a, Collider::aabb(Vec2::splat(2.0)));

        let b = world.spawn();
        world.insert(b, Position(Vec2::new(10.0, 0.0)));
        world.insert(b, Collider::circle(3.0));

        physics_debug_emit_system(&mut world);

        let dd = world.get_resource::<DebugDraw>().unwrap();
        assert_eq!(dd.len(), 2);
    }

    #[test]
    fn toggle_system_flips_on_f1_action() {
        let mut world = World::new();
        let mut input = InputState::new();
        input.key_down(KeyCode::F1);
        world.insert_resource(input);
        world.insert_resource(ActionMap::default_map());
        world.insert_resource(PhysicsDebugOverlay::default());

        physics_debug_toggle_system(&mut world);

        assert!(world.get_resource::<PhysicsDebugOverlay>().unwrap().enabled);
    }
}
