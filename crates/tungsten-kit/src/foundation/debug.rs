//! The kit's debug drawing: each item with a shape drawn through core's
//! `DebugDraw` while [`KitDebug`] is on.

use glam::Vec2;
use tungsten_core::physics::{Collider, Position, Shape};
use tungsten_core::{DebugDraw, Entity, Plugin, Schedule, Transform, World, system};

use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::trace::{KIT_TRACE_STEP, KitTrace};
use crate::intent::aim::Aim;
use crate::lifecycle::children::SPAWN_CHILDREN;
use crate::lifecycle::despawn::DespawnOutside;
use crate::lifecycle::ended::KIT_DESPAWN;
use crate::lifecycle::spawner::{SPAWNER_TICK, Spawner};
use crate::motion::follow::Follow;
use crate::motion::orbit::Orbit;
use crate::motion::path::PathFollower;
use crate::motion::stay_in_view::StayInView;
use crate::motion::wrap::Wrap;
use crate::overlap::contact::ContactDamage;
use crate::overlap::hit::{Hitbox, Hurtbox};
use crate::overlap::pickup::Pickup;
use crate::overlap::zone::Zone;
use crate::present::camera::CameraTarget;
use crate::react::cues::CUES;

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
const ORBIT_COLOR: [f32; 4] = [0.7, 0.5, 1.0, 1.0];
const FOLLOW_COLOR: [f32; 4] = [0.4, 1.0, 1.0, 1.0];
const AIM_COLOR: [f32; 4] = [1.0, 0.3, 0.3, 1.0];
const HURTBOX_COLOR: [f32; 4] = [0.3, 0.9, 0.3, 1.0];
const HITBOX_COLOR: [f32; 4] = [1.0, 0.15, 0.15, 1.0];
const CONTACT_COLOR: [f32; 4] = [1.0, 0.55, 0.0, 1.0];
const PICKUP_COLOR: [f32; 4] = [1.0, 0.85, 0.2, 1.0];

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

/// Inserts [`KitDebug`] and a [`KitTrace`] when the World has none,
/// registers [`KIT_TRACE_STEP`] last in the react phase, after the spawner,
/// the child spawns and the cues when they are registered, which counts
/// each ended step for the trace, and registers [`KIT_DEBUG_DRAW`] in the
/// present phase: while the resource is on and the World has a
/// `DebugDraw`, it draws a path follower's polyline and current target, a
/// `DespawnOutside` region, a zone's collider, a spawner's mark, a camera
/// target's cross, an orbit's circle and centre, a follower's goal, a
/// `Wrap`'s and a `StayInView`'s region, an aim's ray (32 px, or to its
/// target), and each hurtbox's, hitbox's, contact damage's own and
/// pickup's shape at its entity's point, once a frame before the camera.
/// While the
/// resource is on it also logs each new [`KitTrace`] entry once, at `info`
/// under target `tungsten_kit::trace`, which the log console shows.
#[derive(Debug, Default, Clone, Copy)]
pub struct KitDebugPlugin;

impl Plugin for KitDebugPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        if !world.has_resource::<KitDebug>() {
            world.insert_resource(KitDebug::default());
        }
        if !world.has_resource::<KitTrace>() {
            world.insert_resource(KitTrace::default());
        }
        schedule.add(
            Phase::React.stage(),
            Phase::React.join(
                system(KIT_TRACE_STEP, kit_trace_step)
                    .after_if_present(SPAWNER_TICK)
                    .after_if_present(SPAWN_CHILDREN)
                    .after_if_present(CUES)
                    .after_if_present(KIT_DESPAWN),
            ),
        );
        let mut logged = 0;
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(KIT_DEBUG_DRAW, move |world: &mut World| {
                log_trace(world, &mut logged);
                kit_debug_draw(world);
            })),
        );
    }
}

/// The trace's log target.
pub(crate) const TRACE_TARGET: &str = "tungsten_kit::trace";

/// Logs the trace entries pushed since `logged`, while [`KitDebug`] is on,
/// and moves `logged` past them; while it is off, `logged` follows the
/// trace, so turning it on logs from then on.
pub(crate) fn log_trace(world: &World, logged: &mut u64) {
    let Some(trace) = world.get_resource::<KitTrace>() else {
        return;
    };
    let on = world
        .get_resource::<KitDebug>()
        .is_some_and(KitDebug::enabled);
    if on {
        for entry in trace.since(*logged) {
            log::info!(target: TRACE_TARGET, "{entry}");
        }
    }
    *logged = trace.pushed();
}

/// Counts the step that just ended in the World's [`KitTrace`].
fn kit_trace_step(world: &mut World) {
    if let Some(trace) = world.get_resource_mut::<KitTrace>() {
        trace.end_step();
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
    for orbit in world.query::<&Orbit>() {
        if let Some((centre, radius)) = orbit.circle() {
            draw.draw_circle(centre, radius, ORBIT_COLOR, THICKNESS);
            cross(&mut draw, centre, MARK, ORBIT_COLOR);
        }
    }
    for follow in world.query::<&Follow>() {
        if let Some(goal) = follow.goal() {
            cross(&mut draw, goal, MARK, FOLLOW_COLOR);
        }
    }
    let regions = world
        .query::<&Wrap>()
        .filter_map(Wrap::region_bounds)
        .chain(
            world
                .query::<&StayInView>()
                .filter_map(StayInView::region_bounds),
        );
    for (min, max) in regions {
        draw.draw_aabb(min, max, REGION_COLOR, THICKNESS);
    }
    for aim in world.query::<&Aim>() {
        if let Some((origin, end)) = aim.ray() {
            draw.draw_line(origin, end, AIM_COLOR, THICKNESS);
        }
    }
    draw_hit_shapes(world, &mut draw);
    world.insert_resource(draw);
}

/// `shape` centred at `at`.
fn draw_shape(draw: &mut DebugDraw, shape: Shape, at: Vec2, color: [f32; 4]) {
    match shape {
        Shape::Aabb { half_extents } => {
            draw.draw_aabb(at - half_extents, at + half_extents, color, THICKNESS);
        }
        Shape::Circle { radius } => draw.draw_circle(at, radius, color, THICKNESS),
    }
}

/// Each hit item's shape at its entity's point plus its offset.
fn draw_hit_shapes(world: &World, draw: &mut DebugDraw) {
    for (entity, hurtbox) in world.query::<(Entity, &Hurtbox)>() {
        if let Some(at) = point_of(world, entity) {
            draw_shape(
                draw,
                hurtbox.shape().shape(),
                at + hurtbox.offset(),
                HURTBOX_COLOR,
            );
        }
    }
    for (entity, hitbox) in world.query::<(Entity, &Hitbox)>() {
        if let Some(at) = point_of(world, entity) {
            draw_shape(
                draw,
                hitbox.shape().shape(),
                at + hitbox.offset(),
                HITBOX_COLOR,
            );
        }
    }
    for (entity, contact) in world.query::<(Entity, &ContactDamage)>() {
        if let (Some(shape), Some(at)) = (contact.shape(), point_of(world, entity)) {
            draw_shape(draw, shape.shape(), at + contact.offset(), CONTACT_COLOR);
        }
    }
    for (entity, pickup) in world.query::<(Entity, &Pickup)>() {
        if let Some(at) = point_of(world, entity) {
            draw_shape(
                draw,
                pickup.shape().shape(),
                at + pickup.offset(),
                PICKUP_COLOR,
            );
        }
    }
}

#[cfg(test)]
#[path = "../tests/debug.rs"]
mod tests;
