//! The path follower: a body moved along points, such as a moving
//! platform or a patrol.

use glam::Vec2;
use tungsten_core::physics::{PHYSICS_PREV_SNAPSHOT, PHYSICS_STEP, Position, Velocity};
use tungsten_core::{Easing, Plugin, Schedule, Stage, World, system};

use crate::platformer::PLATFORMER_CONTROLLER;
use crate::slide::step_dt;
use crate::top_down::TOP_DOWN_MOVER;

/// [`PathFollowerPlugin`]'s system, in `FixedUpdate`.
pub const PATH_FOLLOW: &str = "path_follow";

/// What a follower does after its last point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Stop,
    Loop,
    PingPong,
}

/// Moves its body along world points at a speed: a moving platform, a
/// patrol, a lift. [`PathFollowerPlugin`]'s system advances it by the step
/// and sets the body's `Velocity` to reach the path's point at the step's
/// end, so a kinematic body ends each step on the path and pushes the
/// dynamic bodies it meets, and a [`PlatformerController`] standing on it
/// rides it.
///
/// Each segment takes its length over the speed, its progress eased by the
/// follower's easing, and time left over at a point carries into the next
/// segment. The body starts at the first point, so spawn it there. By
/// default the follower stops at the last point, where it sets `Velocity`
/// to zero; [`looping`](Self::looping) goes on to the first point and
/// [`ping_pong`](Self::ping_pong) back along the points.
///
/// [`PlatformerController`]: crate::PlatformerController
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Collider, PhysicsPlugin, Position, RigidBodyBundle};
/// use tungsten_core::{Bundle, Easing, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{PathFollower, PathFollowerPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// PhysicsPlugin.build(&mut schedule, &mut world);
/// PathFollowerPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A lift that takes a second each way between two points.
/// let lift = world.spawn_with(
///     RigidBodyBundle::kinematic(Position::new(0.0, 200.0), Collider::aabb(Vec2::new(32.0, 8.0)))
///         .with((PathFollower::new([Vec2::new(0.0, 200.0), Vec2::new(0.0, 100.0)], 100.0)
///             .ping_pong()
///             .with_easing(Easing::SineInOut),)),
/// );
/// let mut heights = Vec::new();
/// for _ in 0..4 {
///     for _ in 0..30 {
///         world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///         schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     }
///     heights.push(world.get::<Position>(lift).unwrap().0.y.round());
/// }
/// assert_eq!(heights, [150.0, 100.0, 150.0, 200.0]);
/// ```
#[derive(Debug, Clone)]
pub struct PathFollower {
    points: Vec<Vec2>,
    speed: f32,
    mode: Mode,
    easing: Easing,
    /// Time along the path's legs, below one cycle unless it stopped.
    time: f32,
}

impl PathFollower {
    /// A follower along `points` at `speed` px/s, linear, stopping at the
    /// last point.
    ///
    /// # Panics
    ///
    /// When there are fewer than two points, a point is not finite, two
    /// neighbours are equal, or `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(points: impl IntoIterator<Item = Vec2>, speed: f32) -> Self {
        let points: Vec<Vec2> = points.into_iter().collect();
        assert!(
            points.len() >= 2,
            "PathFollower::new: needs at least 2 points (got {})",
            points.len()
        );
        assert!(
            points.iter().all(|point| point.is_finite()),
            "PathFollower::new: every point must be finite (got {points:?})"
        );
        assert!(
            points.windows(2).all(|pair| pair[0] != pair[1]),
            "PathFollower::new: neighbouring points must differ (got {points:?})"
        );
        assert!(
            speed.is_finite() && speed > 0.0,
            "PathFollower::new: speed must be finite and > 0 (got {speed})"
        );
        Self {
            points,
            speed,
            mode: Mode::Stop,
            easing: Easing::Linear,
            time: 0.0,
        }
    }

    /// The follower, going on from the last point to the first and round
    /// again; it replaces [`ping_pong`](Self::ping_pong).
    ///
    /// # Panics
    ///
    /// When the last point equals the first.
    #[must_use]
    #[track_caller]
    pub fn looping(mut self) -> Self {
        assert!(
            self.points.first() != self.points.last(),
            "PathFollower::looping: the last point must differ from the first"
        );
        self.mode = Mode::Loop;
        self
    }

    /// The follower, going back along the points from the last and forward
    /// again from the first; it replaces [`looping`](Self::looping).
    #[must_use]
    pub fn ping_pong(mut self) -> Self {
        self.mode = Mode::PingPong;
        self
    }

    /// The follower with each segment's progress eased by `easing`:
    /// `Easing::SineInOut` slows into and out of every point.
    #[must_use]
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// It stopped at its last point: a follower that neither loops nor
    /// ping-pongs, at its end.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.mode == Mode::Stop && self.time >= self.total()
    }

    /// The number of legs one pass takes.
    fn legs(&self) -> usize {
        let segments = self.points.len() - 1;
        match self.mode {
            Mode::Stop => segments,
            Mode::Loop => segments + 1,
            Mode::PingPong => 2 * segments,
        }
    }

    /// Leg `k`'s start and end.
    fn leg(&self, k: usize) -> (Vec2, Vec2) {
        let points = &self.points;
        let segments = points.len() - 1;
        match self.mode {
            Mode::Stop => (points[k], points[k + 1]),
            Mode::Loop => (points[k], points[(k + 1) % points.len()]),
            Mode::PingPong if k < segments => (points[k], points[k + 1]),
            Mode::PingPong => {
                let back = 2 * segments - 1 - k;
                (points[back + 1], points[back])
            }
        }
    }

    /// The time one pass takes.
    fn total(&self) -> f32 {
        (0..self.legs())
            .map(|k| {
                let (from, to) = self.leg(k);
                from.distance(to) / self.speed
            })
            .sum()
    }

    /// Advances its time by `dt`.
    fn advance(&mut self, dt: f32) {
        let total = self.total();
        self.time += dt;
        if self.mode == Mode::Stop {
            self.time = self.time.min(total);
        } else if self.time >= total {
            self.time %= total;
        }
    }

    /// The point on the path at its time.
    fn point(&self) -> Vec2 {
        let mut left = self.time;
        for k in 0..self.legs() {
            let (from, to) = self.leg(k);
            let duration = from.distance(to) / self.speed;
            if left < duration {
                return from + (to - from) * self.easing.apply(left / duration);
            }
            left -= duration;
        }
        self.leg(self.legs() - 1).1
    }
}

/// Registers [`PATH_FOLLOW`] in `FixedUpdate`: after the history snapshot,
/// and before both controllers and `physics_step`, each when registered,
/// so a rider reads where its platform goes this step. Its system advances
/// every [`PathFollower`] with a `Position` and a `Velocity` by the step,
/// `Time::delta` bounded by `PhysicsConfig::max_step_dt` as the physics
/// step takes it; a step of 0 moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct PathFollowerPlugin;

impl Plugin for PathFollowerPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        schedule.add(
            Stage::FixedUpdate,
            system(PATH_FOLLOW, path_follow)
                .after_if_present(PHYSICS_PREV_SNAPSHOT)
                .before_if_present(PLATFORMER_CONTROLLER)
                .before_if_present(TOP_DOWN_MOVER)
                .before_if_present(PHYSICS_STEP),
        );
    }
}

fn path_follow(world: &mut World) {
    let dt = step_dt(world);
    if dt <= 0.0 {
        return;
    }
    for (follower, position, velocity) in
        world.query_mut::<(&mut PathFollower, &Position, &mut Velocity)>()
    {
        if follower.is_finished() {
            velocity.0 = Vec2::ZERO;
            continue;
        }
        follower.advance(dt);
        velocity.0 = (follower.point() - position.0) / dt;
    }
}

#[cfg(test)]
#[path = "tests/path.rs"]
mod tests;
