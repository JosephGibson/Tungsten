//! Despawn outside: an entity, with its children, despawned once it leaves
//! a world-space box or the camera's view.

use glam::Vec2;
use tungsten_core::physics::PHYSICS_SYNC;
use tungsten_core::schedule::names::CAMERA_UPDATE;
use tungsten_core::{
    CameraState, CommandBuffer, Entity, Plugin, Schedule, Stage, Transform, WindowSize, World,
    system,
};

use crate::hierarchy::{TRANSFORM_PROPAGATE_LATE, despawn_recursive};

/// [`DespawnOutsidePlugin`]'s system, in `PostUpdate` after the physics
/// sync, the late propagation pass and the camera.
pub const DESPAWN_OUTSIDE: &str = "despawn_outside";

/// Despawns its entity, with its children, once its `Transform.position`
/// leaves a world-space box or the camera's view: a bolt that left the
/// arena, debris that fell off the screen. [`DespawnOutsidePlugin`]'s
/// system tests the point the frame draws, after the physics sync, the
/// late propagation pass and the camera, and queues [`despawn_recursive`]
/// for the frame's command flush. It tests a point, so a margin stands for
/// the entity's extent.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{CommandBuffer, Plugin, Schedule, Stage, Transform, World};
/// use tungsten_kit::{DespawnOutside, DespawnOutsidePlugin};
///
/// let mut world = World::new();
/// world.insert_resource(CommandBuffer::new());
/// let mut schedule = Schedule::new();
/// DespawnOutsidePlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let arena = DespawnOutside::region(Vec2::ZERO, Vec2::new(640.0, 360.0));
/// let inside = world.spawn_with((Transform::from_position(Vec2::new(320.0, 0.0)), arena));
/// let escaped = world.spawn_with((Transform::from_position(Vec2::new(700.0, 10.0)), arena));
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
/// world.flush_reusing(&mut commands);
/// assert!(world.is_alive(inside) && !world.is_alive(escaped));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DespawnOutside {
    bounds: Bounds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Bounds {
    Region { min: Vec2, max: Vec2 },
    View { margin: f32 },
}

impl DespawnOutside {
    /// Despawns the entity when its position leaves the closed box
    /// `min..=max` in world units.
    ///
    /// # Panics
    ///
    /// When a coordinate is not finite, or `min` lies above `max` on an
    /// axis.
    #[must_use]
    #[track_caller]
    pub fn region(min: Vec2, max: Vec2) -> Self {
        assert!(
            min.is_finite() && max.is_finite(),
            "DespawnOutside::region: {min:?}..={max:?} is not finite"
        );
        assert!(
            min.x <= max.x && min.y <= max.y,
            "DespawnOutside::region: {min:?} lies above {max:?} on an axis"
        );
        Self {
            bounds: Bounds::Region { min, max },
        }
    }

    /// Despawns the entity when its position lies more than `margin` world
    /// units outside the camera's view: `CameraState::visible_world_aabb` at
    /// the `WindowSize`. Without either resource the entity stays.
    ///
    /// # Panics
    ///
    /// When `margin` is negative or not finite.
    #[must_use]
    #[track_caller]
    pub fn view(margin: f32) -> Self {
        assert!(
            margin.is_finite() && margin >= 0.0,
            "DespawnOutside::view: the margin {margin} is negative or not finite"
        );
        Self {
            bounds: Bounds::View { margin },
        }
    }
}

/// Registers [`DESPAWN_OUTSIDE`] in `PostUpdate`, after the physics sync,
/// the late propagation pass and the camera when those are registered: it
/// queues the despawn of every [`DespawnOutside`] entity, with its
/// children, whose `Transform.position` has left its box or the view.
/// Without a `CommandBuffer` it queues nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct DespawnOutsidePlugin;

impl Plugin for DespawnOutsidePlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        schedule.add(
            Stage::PostUpdate,
            system(DESPAWN_OUTSIDE, despawn_outside)
                .after_if_present(PHYSICS_SYNC)
                .after_if_present(TRANSFORM_PROPAGATE_LATE)
                .after_if_present(CAMERA_UPDATE),
        );
    }
}

fn outside(point: Vec2, min: Vec2, max: Vec2) -> bool {
    point.x < min.x || point.x > max.x || point.y < min.y || point.y > max.y
}

fn despawn_outside(world: &mut World) {
    let view = match (
        world.get_resource::<CameraState>(),
        world.get_resource::<WindowSize>(),
    ) {
        (Some(camera), Some(size)) => {
            Some(camera.visible_world_aabb(size.width as f32, size.height as f32))
        }
        _ => None,
    };
    let mut gone: Vec<Entity> = Vec::new();
    for (entity, rule, transform) in world.query::<(Entity, &DespawnOutside, &Transform)>() {
        let point = transform.position;
        let left = match rule.bounds {
            Bounds::Region { min, max } => outside(point, min, max),
            Bounds::View { margin } => view.is_some_and(|(min, max)| {
                outside(point, min - Vec2::splat(margin), max + Vec2::splat(margin))
            }),
        };
        if left {
            gone.push(entity);
        }
    }
    if gone.is_empty() {
        return;
    }
    let Some(commands) = world.get_resource_mut::<CommandBuffer>() else {
        return;
    };
    for entity in gone {
        despawn_recursive(commands, entity);
    }
}

#[cfg(test)]
#[path = "tests/despawn.rs"]
mod tests;
