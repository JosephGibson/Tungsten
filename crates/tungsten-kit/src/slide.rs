//! The controllers' shared motion: the step's length as the physics step
//! takes it, the ground probe and the cast loop that moves a body and slides
//! it along what it meets.

use glam::Vec2;
use tungsten_core::physics::{BodyKind, RigidBody, Shape, SpatialFilter, SpatialHit, cast_shape};
use tungsten_core::{Entity, PhysicsConfig, Time, World};

/// The gap a move leaves before the surface it meets, in px.
pub(crate) const SKIN: f32 = 0.01;

/// How far below its feet a body looks for the ground, in px.
const GROUND_PROBE: f32 = 1.0;

/// The most casts one move takes.
const MAX_SLIDES: usize = 4;

/// A move shorter than this, in px, is no move.
const MIN_MOVE: f32 = 1e-4;

/// A surface whose normal's y is at most this is a floor; y points down.
const FLOOR_NORMAL_Y: f32 = -0.7;

/// The step a fixed system moves a body by: [`Time::delta`] bounded by
/// `PhysicsConfig::max_step_dt`, as `physics_step` takes it.
pub(crate) fn step_dt(world: &World) -> f32 {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let bound = world
        .get_resource::<PhysicsConfig>()
        .copied()
        .unwrap_or_default()
        .max_step_dt;
    if bound > 0.0 { dt.min(bound) } else { dt }
}

/// What a move met.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Slide {
    /// How far the shape moved.
    pub(crate) moved: Vec2,
    /// It met a floor.
    pub(crate) floor: bool,
    /// The floor's body; `None` for a tile.
    pub(crate) floor_body: Option<Entity>,
    /// It met a ceiling.
    pub(crate) ceiling: bool,
    /// It met a wall.
    pub(crate) wall: bool,
}

/// Moves `shape` from `origin` by `delta` against the last step's positions
/// in up to [`MAX_SLIDES`] casts: each hit stops it [`SKIN`] short and
/// projects the rest of the move onto the surface met.
pub(crate) fn move_and_slide(
    world: &World,
    shape: Shape,
    origin: Vec2,
    delta: Vec2,
    filter: &SpatialFilter,
) -> Slide {
    let mut slide = Slide::default();
    let mut at = origin;
    let mut rest = delta;
    for _ in 0..MAX_SLIDES {
        let length = rest.length();
        if length.is_nan() || length <= MIN_MOVE {
            break;
        }
        let direction = rest / length;
        let Some(hit) = cast_shape(world, shape, at, direction, length, filter) else {
            at += rest;
            break;
        };
        let travel = (hit.distance - SKIN).max(0.0);
        at += direction * travel;
        if hit.normal.y <= FLOOR_NORMAL_Y {
            slide.floor = true;
            slide.floor_body = hit.entity;
        } else if hit.normal.y >= -FLOOR_NORMAL_Y {
            slide.ceiling = true;
        } else {
            slide.wall = true;
        }
        rest = direction * (length - travel);
        rest -= hit.normal * rest.dot(hit.normal);
    }
    slide.moved = at - origin;
    slide
}

/// The floor within [`GROUND_PROBE`] below `shape` at `origin`, when the
/// surface met faces up.
pub(crate) fn probe_ground(
    world: &World,
    shape: Shape,
    origin: Vec2,
    filter: &SpatialFilter,
) -> Option<SpatialHit> {
    cast_shape(world, shape, origin, Vec2::Y, GROUND_PROBE, filter)
        .filter(|hit| hit.normal.y <= FLOOR_NORMAL_Y)
}

/// Whether `entity` is a kinematic body.
pub(crate) fn is_kinematic(world: &World, entity: Entity) -> bool {
    world
        .get::<RigidBody>(entity)
        .is_some_and(|body| body.kind == BodyKind::Kinematic)
}
