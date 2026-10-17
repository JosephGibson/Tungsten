//! The kit's motion write rule: what kind of body an entity is, which kit
//! mover drives a kinematic body, and the writes a mover makes, with the
//! warning it gives once per entity and reason when it skips one.
//!
//! Movers move roots before the physics step: a bodiless or static
//! entity's `Position`, by an increment, or a kinematic or dynamic body's
//! `Velocity`; never a `Transform.position` that the sync or the hierarchy
//! writes. A kinematic body has one driver, which writes its velocity every
//! step; the adders (the oscillator's position, a knockback) add to it, or
//! drive in its place when it has none.

use std::collections::HashSet;

use glam::Vec2;
use tungsten_core::physics::{BodyKind, Position, RigidBody, Velocity};
use tungsten_core::{Entity, Transform, World};

use crate::motion::follow::Follow;
use crate::motion::linear::LinearMotion;
use crate::motion::orbit::Orbit;
use crate::motion::oscillator::Oscillator;
use crate::motion::path::PathFollower;
use crate::motion::platformer::PlatformerController;
use crate::motion::steering::Steering;
use crate::motion::top_down::TopDownMover;
use crate::transform::hierarchy::Parent;

/// What a mover may write on an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Body {
    /// A child: the hierarchy writes its drawn point after the sync.
    Child,
    /// No body: its `Position` moves, with no cast.
    Bodiless,
    /// A static body: its `Position` moves as a bodiless entity's does.
    Static,
    /// A kinematic body with a `Velocity`, which its driver writes.
    Kinematic,
    /// A dynamic body with a `Velocity`, which the solver keeps.
    Dynamic,
    /// No `Position`, or a moving body without a `Velocity`.
    Unplaced,
}

/// The kind of body `entity` is, for a mover.
pub(crate) fn body_of(world: &World, entity: Entity) -> Body {
    if world.has::<Parent>(entity) {
        return Body::Child;
    }
    if !world.has::<Position>(entity) {
        return Body::Unplaced;
    }
    let moving = world.has::<Velocity>(entity);
    match world.get::<RigidBody>(entity).map(|body| body.kind) {
        None => Body::Bodiless,
        Some(BodyKind::Static) => Body::Static,
        Some(BodyKind::Kinematic) if moving => Body::Kinematic,
        Some(BodyKind::Dynamic) if moving => Body::Dynamic,
        Some(_) => Body::Unplaced,
    }
}

/// The kit mover that drives an entity's motion each step, read from its
/// components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Driver {
    /// None: an oscillator or a knockback drives in its place.
    None,
    /// A path follower or a linear motion, which run before the oscillator
    /// and write the velocity it adds to.
    Early,
    /// A follower or an orbit, which run after the oscillator and set a
    /// goal with its offset folded in.
    Placer,
    /// A controller, a top-down mover or a steering, which run after the
    /// oscillator and set the velocity outright.
    Late,
}

/// The kit mover that drives `entity`: a placer first, since it runs last
/// of the drivers and sets the point outright.
pub(crate) fn driver_of(world: &World, entity: Entity) -> Driver {
    if world.has::<Follow>(entity) || world.has::<Orbit>(entity) {
        Driver::Placer
    } else if world.has::<PlatformerController>(entity)
        || world.has::<TopDownMover>(entity)
        || world.has::<Steering>(entity)
    {
        Driver::Late
    } else if world.has::<PathFollower>(entity) || world.has::<LinearMotion>(entity) {
        Driver::Early
    } else {
        Driver::None
    }
}

/// Whether a kit mover wrote `entity`'s motion before the knockback this
/// step: a driver, or an oscillator with a position channel.
pub(crate) fn driven(world: &World, entity: Entity) -> bool {
    driver_of(world, entity) != Driver::None
        || world
            .get::<Oscillator>(entity)
            .is_some_and(Oscillator::moves)
}

/// How a mover writes its step into a velocity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Write {
    /// It drives: its velocity replaces the last.
    Set,
    /// It adds to what a driver wrote this step.
    Add,
}

/// Moves a bodiless or static entity's `Position` by `delta`, and writes
/// `delta / dt` into its `Velocity` when it has one, so the readers of its
/// speed see the step's move.
pub(crate) fn step_position(world: &mut World, entity: Entity, delta: Vec2, dt: f32, write: Write) {
    if let Some(position) = world.get_mut::<Position>(entity) {
        position.0 += delta;
    }
    write_velocity(world, entity, delta / dt, write);
}

/// Writes `velocity` into `entity`'s `Velocity`, when it has one.
pub(crate) fn write_velocity(world: &mut World, entity: Entity, velocity: Vec2, write: Write) {
    if let Some(body) = world.get_mut::<Velocity>(entity) {
        match write {
            Write::Set => body.0 = velocity,
            Write::Add => body.0 += velocity,
        }
    }
}

/// Where `entity` is: its `Position`, else its `Transform.position`.
pub(crate) fn point_of(world: &World, entity: Entity) -> Option<Vec2> {
    world
        .get::<Position>(entity)
        .map(|position| position.0)
        .or_else(|| world.get::<Transform>(entity).map(|t| t.position))
}

/// Where `entity`'s movers leave it at the end of this step of `dt`: a
/// kinematic or dynamic body's `Position` plus its `Velocity` times the
/// step, since the step integrates those two kinds; a static body's or a
/// bodiless entity's `Position`, which the step leaves; else its
/// `Transform.position`.
pub(crate) fn placed_point(world: &World, entity: Entity, dt: f32) -> Option<Vec2> {
    let Some(position) = world.get::<Position>(entity) else {
        return world.get::<Transform>(entity).map(|t| t.position);
    };
    let integrated = world
        .get::<RigidBody>(entity)
        .is_some_and(|body| matches!(body.kind, BodyKind::Kinematic | BodyKind::Dynamic));
    match world.get::<Velocity>(entity) {
        Some(velocity) if integrated => Some(position.0 + velocity.0 * dt),
        _ => Some(position.0),
    }
}

/// Why a mover skipped an entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Skip {
    Child,
    Unplaced,
    Static,
    Dynamic,
    LateDriver,
}

impl Skip {
    fn why(self) -> &'static str {
        match self {
            Skip::Child => "a child, whose point its parent's offset places",
            Skip::Unplaced => "no Position, or a moving body without a Velocity",
            Skip::Static => "a static body",
            Skip::Dynamic => "a dynamic body, which keeps its own velocity",
            Skip::LateDriver => {
                "a kinematic body whose controller, top-down mover or steering sets its velocity after it"
            }
        }
    }
}

/// The skips a system has warned about, once per entity and reason.
#[derive(Debug, Default)]
pub(crate) struct Warned(HashSet<(Entity, Skip)>);

impl Warned {
    /// Warns that `item` skipped `entity`, the first time for that reason.
    pub(crate) fn skip(&mut self, item: &str, entity: Entity, skip: Skip) {
        if self.0.insert((entity, skip)) {
            log::warn!("{item} on {entity:?}: {}; skipped", skip.why());
        }
    }
}
