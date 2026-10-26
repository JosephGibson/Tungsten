//! A walker's way over what lies ahead (e01foes, from the goblin's): it
//! hops a body or a low step, steps down a short drop and, with a leap,
//! crosses a gap to ground it can land on; at a keg, a bomb, a high wall, a
//! deep ledge or a gap too wide it stops. Each kind's [`Hops`] sets how
//! high and how far, its body's half size from its kind.

use glam::Vec2;
use tungsten::core::{BodyKind, Entity, RigidBody, Shape, World};
use tungsten::kit::{PlatformerController, Stats};
use tungsten::physics::{SpatialFilter, cast_ray, overlap};

use super::centre;
use super::components::Enemy;
use super::systems::{blocked, rest, solid, stride};
use crate::features::level::GRAVITY;
use crate::features::props::components::{Bomb, Keg};

/// A walker's hops: its hop's speed up, px/s, which clears its square over
/// twice the gravity; the highest it hops, px over its feet; the deepest
/// drop it steps down, px under them; and its leap over a gap, if it has
/// one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hops {
    pub rise: f32,
    pub clear: f32,
    pub step_down: f32,
    pub leap: Option<Leap>,
}

/// A leap over a gap: its speed across and up, px/s, and the seconds its
/// input stays locked, holding its speed across; its full run carries it
/// after.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leap {
    pub across: f32,
    pub lift: f32,
    pub lock: f32,
}

/// The highest ground over its feet a leap lands on, px, and the slack its
/// reach keeps past where the far ground begins.
pub const LEAP_UP: f32 = 24.0;
pub const LEAP_SLACK: f32 = 16.0;
/// The step between the columns a leap's look for ground tries, px.
const LEAP_LOOK: f32 = 2.0;

/// What lies ahead of a walker, and what [`scurry`] does there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ahead {
    Clear,
    Hop,
    Leap,
    Step,
    Stop,
}

impl Ahead {
    /// Whether it goes on: walking, hopping, leaping or stepping down.
    #[must_use]
    pub fn moves(self) -> bool {
        self != Ahead::Stop
    }
}

/// `walker`'s body's half size, by its kind.
fn half(world: &World, walker: Entity) -> Option<Vec2> {
    world.get::<Enemy>(walker).map(|mine| mine.kind.info().half)
}

/// Whether `walker` stands on something.
#[must_use]
pub fn grounded(world: &World, walker: Entity) -> bool {
    world
        .get::<PlatformerController>(walker)
        .is_some_and(PlatformerController::grounded)
}

/// The first dynamic body just ahead of `walker` along `heading`, at its
/// height, probed from inside its own edge, so a body it already presses
/// against counts.
#[must_use]
pub fn body_ahead(world: &World, walker: Entity, heading: f32) -> Option<Entity> {
    let (at, half) = (centre(world, walker)?, half(world, walker)?);
    let probe = at + Vec2::new(heading * (half.x + 4.0), 0.0);
    let shape = Shape::Aabb {
        half_extents: Vec2::new(8.0, half.y - 4.0),
    };
    let filter = SpatialFilter::new().with_tiles(false).excluding(walker);
    let mut met = Vec::new();
    overlap(world, shape, probe, &filter, &mut met);
    met.into_iter().flatten().find(|&entity| {
        world
            .get::<RigidBody>(entity)
            .is_some_and(|body| body.kind == BodyKind::Dynamic)
    })
}

/// What lies ahead of `walker` along `heading`: a body at its height that
/// is no keg or bomb, or a wall no higher than its hop clears, with room
/// over it and ground beyond, to hop; a ledge with ground within its step
/// down under its feet, to step down; a gap its leap crosses, to leap; a
/// keg, a bomb, a higher wall, a deep ledge or a gap too wide, to stop at;
/// else a clear way.
#[must_use]
pub fn ahead(world: &World, walker: Entity, heading: f32, hops: &Hops) -> Ahead {
    let (Some(at), Some(half)) = (centre(world, walker), half(world, walker)) else {
        return Ahead::Stop;
    };
    if let Some(body) = body_ahead(world, walker, heading) {
        let hop = !world.has::<Keg>(body)
            && !world.has::<Bomb>(body)
            && room(world, walker, heading, hops);
        return if hop { Ahead::Hop } else { Ahead::Stop };
    }
    if !blocked(world, walker, heading) {
        return Ahead::Clear;
    }
    let wall = solid(world, at, Vec2::new(heading, 0.0), half.x + 8.0, walker);
    if wall {
        return if room(world, walker, heading, hops) {
            Ahead::Hop
        } else {
            Ahead::Stop
        };
    }
    // A ledge: a short drop it steps down, a gap it may leap, else it stops.
    let toes = Vec2::new(at.x + heading * (half.x + 6.0), at.y);
    let filter = SpatialFilter::new().excluding(walker);
    if cast_ray(world, toes, Vec2::Y, half.y + hops.step_down, &filter).is_some() {
        return Ahead::Step;
    }
    match hops.leap {
        Some(leap) if leaps(world, walker, heading, hops, leap) => Ahead::Leap,
        _ => Ahead::Stop,
    }
}

/// Whether a hop along `heading` has room: nothing over its head for the
/// hop's height, nothing ahead its clear over its feet, and ground beyond.
fn room(world: &World, walker: Entity, heading: f32, hops: &Hops) -> bool {
    let (Some(at), Some(half)) = (centre(world, walker), half(world, walker)) else {
        return false;
    };
    let filter = SpatialFilter::new().excluding(walker);
    let rise = hops.rise * hops.rise / (2.0 * GRAVITY);
    let head = cast_ray(world, at, Vec2::NEG_Y, half.y + rise, &filter).is_none();
    let over = Vec2::new(at.x, at.y + half.y - hops.clear - 6.0);
    let clear = cast_ray(world, over, Vec2::new(heading, 0.0), half.x + 64.0, &filter).is_none();
    let beyond = Vec2::new(at.x + heading * (half.x + 72.0), over.y);
    let ground = cast_ray(
        world,
        beyond,
        Vec2::Y,
        hops.clear + half.y * 2.0 + 64.0,
        &filter,
    )
    .is_some();
    head && clear && ground
}

/// How far across `leap` carries a walker, at `run` px/s once its lock
/// ends, before it comes down to `up` px over its feet; none when it never
/// rises that high.
#[must_use]
pub fn reach(leap: Leap, up: f32, run: f32) -> Option<f32> {
    let left = leap.lift * leap.lift - 2.0 * GRAVITY * up;
    if left < 0.0 {
        return None;
    }
    let flight = (leap.lift + left.sqrt()) / GRAVITY;
    Some(leap.across * flight.min(leap.lock) + run * (flight - leap.lock).max(0.0))
}

/// Whether `leap` takes `walker` over the gap along `heading`: room over
/// its head for the leap's height, and ground across whose top lies from
/// [`LEAP_UP`] over its feet to its step down under them, beginning within
/// the leap's reach at that height, less [`LEAP_SLACK`], of its leading
/// edge, its full run carrying it once the lock ends; ground higher than
/// that before it stops the leap.
fn leaps(world: &World, walker: Entity, heading: f32, hops: &Hops, leap: Leap) -> bool {
    let (Some(at), Some(half)) = (centre(world, walker), half(world, walker)) else {
        return false;
    };
    let filter = SpatialFilter::new().excluding(walker);
    let apex = leap.lift * leap.lift / (2.0 * GRAVITY);
    if cast_ray(world, at, Vec2::NEG_Y, half.y + apex, &filter).is_some() {
        return false;
    }
    let run = world
        .get::<Stats>(walker)
        .and_then(|stats| stats.get(Stats::MOVE_SPEED))
        .unwrap_or(0.0);
    let Some(farthest) = reach(leap, -hops.step_down, run) else {
        return false;
    };
    let feet = at.y + half.y;
    let front = at.x + heading * half.x;
    // Ground higher than it lands on, across its way, ends the look.
    let over = Vec2::new(front, feet - LEAP_UP - 1.0);
    let wall = cast_ray(world, over, Vec2::new(heading, 0.0), farthest, &filter)
        .map_or(farthest, |hit| hit.distance);
    // From its toes, where the ledge's look found no ground.
    let mut look = 6.0;
    while look <= wall {
        let column = Vec2::new(front + heading * look, over.y);
        if let Some(hit) = cast_ray(
            world,
            column,
            Vec2::Y,
            LEAP_UP + 1.0 + hops.step_down,
            &filter,
        ) {
            let up = feet - hit.point.y;
            return reach(leap, up, run).is_some_and(|reach| look <= reach - LEAP_SLACK);
        }
        look += LEAP_LOOK;
    }
    false
}

/// Hops along `heading`: up at its hop's rise, running at its full speed,
/// so its run carries it over once it is clear of what it hops. Whether it
/// hopped: only from the ground.
pub fn hop(world: &mut World, walker: Entity, heading: f32, hops: &Hops) -> bool {
    if !grounded(world, walker) {
        return false;
    }
    if let Some(mine) = world.get_mut::<Enemy>(walker) {
        mine.heading = heading;
    }
    if let Some(controller) = world.get_mut::<PlatformerController>(walker) {
        controller.launch(Vec2::new(0.0, -hops.rise));
        controller.set_run(heading);
    }
    true
}

/// Leaps along `heading`: across and up at `leap`'s speeds, its input
/// locked for its lock, its full run carrying it after. Whether it leapt:
/// only from the ground.
fn leap(world: &mut World, walker: Entity, heading: f32, leap: Leap) -> bool {
    if !grounded(world, walker) {
        return false;
    }
    if let Some(mine) = world.get_mut::<Enemy>(walker) {
        mine.heading = heading;
    }
    if let Some(controller) = world.get_mut::<PlatformerController>(walker) {
        controller.launch(Vec2::new(heading * leap.across, -leap.lift));
        controller.lock_input(leap.lock);
        controller.set_run(heading);
    }
    true
}

/// Walks `walker` along `heading` at `pace` of its run speed, as a walker's
/// stride does, hopping a body or a low step ahead, leaping a gap it can
/// cross and stepping down a short drop; stopped, it stands. What it did:
/// [`Ahead::Stop`] when it stands, or a hop or a leap it could not start
/// from the air.
pub fn scurry(world: &mut World, walker: Entity, heading: f32, pace: f32, hops: &Hops) -> Ahead {
    match ahead(world, walker, heading, hops) {
        Ahead::Clear if stride(world, walker, heading, pace) => Ahead::Clear,
        Ahead::Hop if hop(world, walker, heading, hops) => Ahead::Hop,
        Ahead::Leap if hops.leap.is_some_and(|it| leap(world, walker, heading, it)) => Ahead::Leap,
        Ahead::Step => {
            // Off the short drop, which the walk's ledge check refuses.
            if let Some(mine) = world.get_mut::<Enemy>(walker) {
                mine.heading = heading;
            }
            if let Some(controller) = world.get_mut::<PlatformerController>(walker) {
                controller.set_run(heading * pace);
            }
            Ahead::Step
        }
        Ahead::Stop => {
            rest(world, walker);
            Ahead::Stop
        }
        _ => Ahead::Stop,
    }
}
