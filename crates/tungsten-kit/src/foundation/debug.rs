//! The kit's debug drawing: each item with a shape drawn through core's
//! `DebugDraw` while [`KitDebug`] is on.

use glam::Vec2;
use tungsten_core::physics::{Collider, Position, Shape};
use tungsten_core::{DebugDraw, Entity, Plugin, Schedule, Transform, World, system};

use crate::foundation::phases::{Phase, ensure_phases};
use crate::lifecycle::despawn::DespawnOutside;
use crate::lifecycle::spawner::Spawner;
use crate::motion::path::PathFollower;
use crate::overlap::zone::Zone;
use crate::present::camera::CameraTarget;

/// [`KitDebugPlugin`]'s system, in the present phase.
pub const KIT_DEBUG_DRAW: &str = "kit_debug_draw";

/// The lines' thickness, in world pixels.
const THICKNESS: f32 = 1.0;
/// A mark's half size, in world pixels.
const MARK: f32 = 4.0;

const PATH_COLOR: [f32; 4] = [0.3, 0.8, 1.0, 1.0];
const TARGET_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const REGION_COLOR: [f32; 4] = [1.0, 0.6, 0.2, 1.0];
const ZONE_COLOR: [f32; 4] = [0.6, 1.0, 0.4, 1.0];
const SPAWNER_COLOR: [f32; 4] = [1.0, 0.4, 0.8, 1.0];
const CAMERA_COLOR: [f32; 4] = [1.0, 1.0, 0.3, 1.0];

/// Whether the kit draws its debug shapes, a World resource, off by
/// default; a game's hotkey or console command flips it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Collider, PhysicsPlugin, Position, RigidBodyBundle};
/// use tungsten_core::{Bundle, DebugDraw, Plugin, Schedule, Stage, World};
/// use tungsten_kit::{KitDebug, KitDebugPlugin, PathFollower};
///
/// let mut world = World::new();
/// world.insert_resource(DebugDraw::new());
/// let mut schedule = Schedule::new();
/// PhysicsPlugin.build(&mut schedule, &mut world);
/// KitDebugPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// world.spawn_with(
///     RigidBodyBundle::kinematic(Position::new(0.0, 0.0), Collider::aabb(Vec2::splat(8.0)))
///         .with((PathFollower::new([Vec2::ZERO, Vec2::new(100.0, 0.0)], 50.0),)),
/// );
///
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert!(world.resource::<DebugDraw>().is_empty(), "off by default");
/// world.resource_mut::<KitDebug>().set_enabled(true);
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// assert!(!world.resource::<DebugDraw>().is_empty(), "the path and its target");
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct KitDebug {
    enabled: bool,
}

impl KitDebug {
    /// Whether the shapes draw.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Turns the shapes on or off.
    pub fn set_enabled(&mut self, on: bool) {
        self.enabled = on;
    }
}

/// Inserts [`KitDebug`] when the World has none and registers
/// [`KIT_DEBUG_DRAW`] in the present phase: while the resource is on and
/// the World has a `DebugDraw`, it draws a path follower's polyline and
/// current target, a `DespawnOutside` region, a zone's collider, a
/// spawner's mark and a camera target's cross, once a frame before the
/// camera.
#[derive(Debug, Default, Clone, Copy)]
pub struct KitDebugPlugin;

impl Plugin for KitDebugPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        if !world.has_resource::<KitDebug>() {
            world.insert_resource(KitDebug::default());
        }
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(KIT_DEBUG_DRAW, kit_debug_draw)),
        );
    }
}

/// Where `entity` is: its `Position`, else its `Transform.position`.
fn point_of(world: &World, entity: Entity) -> Option<Vec2> {
    world
        .get::<Position>(entity)
        .map(|position| position.0)
        .or_else(|| world.get::<Transform>(entity).map(|t| t.position))
}

fn cross(draw: &mut DebugDraw, at: Vec2, half: f32, color: [f32; 4]) {
    draw.draw_line(
        at - Vec2::new(half, 0.0),
        at + Vec2::new(half, 0.0),
        color,
        THICKNESS,
    );
    draw.draw_line(
        at - Vec2::new(0.0, half),
        at + Vec2::new(0.0, half),
        color,
        THICKNESS,
    );
}

fn kit_debug_draw(world: &mut World) {
    if !world
        .get_resource::<KitDebug>()
        .is_some_and(KitDebug::enabled)
    {
        return;
    }
    let Some(mut draw) = world.remove_resource::<DebugDraw>() else {
        return;
    };
    for (follower, position) in world.query::<(&PathFollower, &Position)>() {
        let points = follower.points();
        for pair in points.windows(2) {
            draw.draw_line(pair[0], pair[1], PATH_COLOR, THICKNESS);
        }
        if follower.is_looping()
            && let (Some(&first), Some(&last)) = (points.first(), points.last())
        {
            draw.draw_line(last, first, PATH_COLOR, THICKNESS);
        }
        cross(&mut draw, follower.point(), MARK, TARGET_COLOR);
        let _ = position;
    }
    for (rule, entity) in world.query::<(&DespawnOutside, Entity)>() {
        if let Some((min, max)) = rule.region_bounds() {
            draw.draw_aabb(min, max, REGION_COLOR, THICKNESS);
        }
        let _ = entity;
    }
    for (_, collider, position) in world.query::<(&Zone, &Collider, &Position)>() {
        let at = position.0 + collider.offset;
        match collider.shape {
            Shape::Aabb { half_extents } => {
                draw.draw_aabb(at - half_extents, at + half_extents, ZONE_COLOR, THICKNESS);
            }
            Shape::Circle { radius } => draw.draw_circle(at, radius, ZONE_COLOR, THICKNESS),
        }
    }
    let spawners: Vec<Entity> = world
        .query::<(Entity, &Spawner)>()
        .map(|(entity, _)| entity)
        .collect();
    for entity in spawners {
        if let Some(at) = point_of(world, entity) {
            draw.draw_circle(at, MARK, SPAWNER_COLOR, THICKNESS);
        }
    }
    let targets: Vec<Entity> = world
        .query::<(Entity, &CameraTarget)>()
        .map(|(entity, _)| entity)
        .collect();
    for entity in targets {
        if let Some(at) = point_of(world, entity) {
            cross(&mut draw, at, MARK * 2.0, CAMERA_COLOR);
        }
    }
    world.insert_resource(draw);
}

#[cfg(test)]
#[path = "../tests/debug.rs"]
mod tests;
