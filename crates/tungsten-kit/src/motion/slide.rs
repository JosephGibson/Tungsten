//! The controllers' shared motion: the step's length as the physics step
//! takes it, the ground probe, the cast loop that moves a body and slides it
//! along what it meets, and the climb over a step.

use glam::Vec2;
use tungsten_core::physics::{BodyKind, RigidBody, Shape, SpatialFilter, SpatialHit, cast_shape};
use tungsten_core::{Entity, PhysicsConfig, Time, World};

/// The gap a move leaves before the surface it meets, in px.
pub(crate) const SKIN: f32 = 0.01;

/// How far below its feet a body looks for the ground, in px.
pub(crate) const GROUND_PROBE: f32 = 1.0;

/// The most casts one move takes.
const MAX_SLIDES: usize = 4;

/// A move shorter than this, in px, is no move.
const MIN_MOVE: f32 = 1e-4;

/// A surface whose normal's y is at most this is a floor; y points down.
pub(crate) const FLOOR_NORMAL_Y: f32 = -0.7;

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
// What each cast met, a floor, a ceiling and a wall at once among them.
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Slide {
    /// How far the shape moved.
    pub(crate) moved: Vec2,
    /// It met a floor.
    pub(crate) floor: bool,
    /// The floor's body; `None` for a tile.
    pub(crate) floor_body: Option<Entity>,
    /// The floor's outward normal; zero with no floor.
    pub(crate) floor_normal: Vec2,
    /// The floor is a slope tile's sloped face.
    pub(crate) floor_slope: bool,
    /// It met a ceiling.
    pub(crate) ceiling: bool,
    /// It met a wall.
    pub(crate) wall: bool,
    /// The first wall's body; `None` for a tile, or with no wall.
    pub(crate) wall_body: Option<Entity>,
    /// The first wall's outward normal; zero with no wall.
    pub(crate) wall_normal: Vec2,
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
    move_and_slide_on(world, shape, origin, delta, filter, FLOOR_NORMAL_Y, false)
}

/// [`move_and_slide`] with a surface whose normal's y is at most `floor_y` a
/// floor; with `no_climb`, a slope tile's face met as a wall drops the rest
/// of the move along x where sliding along it would climb, so a slope too
/// steep to walk stops a walker as an upright wall does, and one falling
/// onto it still slides down.
pub(crate) fn move_and_slide_on(
    world: &World,
    shape: Shape,
    origin: Vec2,
    delta: Vec2,
    filter: &SpatialFilter,
    floor_y: f32,
    no_climb: bool,
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
        let mut wall = false;
        if hit.normal.y <= floor_y {
            slide.floor = true;
            slide.floor_body = hit.entity;
            slide.floor_normal = hit.normal;
            slide.floor_slope = hit.slope;
        } else if hit.normal.y >= -FLOOR_NORMAL_Y {
            slide.ceiling = true;
        } else {
            if !slide.wall {
                slide.wall_body = hit.entity;
                slide.wall_normal = hit.normal;
            }
            slide.wall = true;
            wall = true;
        }
        rest = direction * (length - travel);
        let along = rest - hit.normal * rest.dot(hit.normal);
        rest = if no_climb && wall && hit.slope && along.y < 0.0 {
            Vec2::new(0.0, rest.y)
        } else {
            along
        };
    }
    slide.moved = at - origin;
    slide
}

/// The floor within `reach` below `shape` at `origin`, when the surface met
/// is a floor by `floor_y`, as [`move_and_slide_on`] takes it: within
/// [`GROUND_PROBE`] and by [`FLOOR_NORMAL_Y`] before slopes.
pub(crate) fn probe_ground_on(
    world: &World,
    shape: Shape,
    origin: Vec2,
    filter: &SpatialFilter,
    reach: f32,
    floor_y: f32,
) -> Option<SpatialHit> {
    cast_shape(world, shape, origin, Vec2::Y, reach, filter).filter(|hit| hit.normal.y <= floor_y)
}

/// Where `shape` at `origin` lands climbing a step: up by at most `height`,
/// as far as there is room, across by `across` px (signed, along x) as far
/// as nothing stops it, then down by what it rose plus [`GROUND_PROBE`].
/// The landing's shape position and the floor's body (`None` for a tile),
/// when the way down meets a floor that is not a kinematic body; `None`
/// when it rises or goes across by no move, or lands on nothing.
pub(crate) fn climb(
    world: &World,
    shape: Shape,
    origin: Vec2,
    height: f32,
    across: f32,
    filter: &SpatialFilter,
) -> Option<(Vec2, Option<Entity>)> {
    let rise = cast_shape(world, shape, origin, Vec2::NEG_Y, height, filter)
        .map_or(height, |hit| (hit.distance - SKIN).max(0.0));
    if rise <= MIN_MOVE {
        return None;
    }
    let up = origin - Vec2::new(0.0, rise);
    let direction = Vec2::new(across.signum(), 0.0);
    let reach = across.abs();
    let travel = cast_shape(world, shape, up, direction, reach, filter)
        .map_or(reach, |hit| (hit.distance - SKIN).max(0.0));
    if travel <= MIN_MOVE {
        return None;
    }
    let over = up + direction * travel;
    let floor = cast_shape(world, shape, over, Vec2::Y, rise + GROUND_PROBE, filter)?;
    if floor.normal.y > FLOOR_NORMAL_Y || floor.entity.is_some_and(|body| is_kinematic(world, body))
    {
        return None;
    }
    Some((
        over + Vec2::Y * (floor.distance - SKIN).max(0.0),
        floor.entity,
    ))
}

/// Whether `entity` is a kinematic body.
pub(crate) fn is_kinematic(world: &World, entity: Entity) -> bool {
    world
        .get::<RigidBody>(entity)
        .is_some_and(|body| body.kind == BodyKind::Kinematic)
}
