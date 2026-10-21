//! The path follower: a body moved along points, such as a moving
//! platform or a patrol.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, Velocity};
use tungsten_core::{Easing, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::foundation::describe::{
    Bound, ComponentDescription, EASING_NAMES, FieldDescription, FieldType, PLATFORMER, SHMUP,
    TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::positive;
use crate::motion::oscillator::Oscillator;
use crate::motion::platformer::PLATFORMER_CONTROLLER;
use crate::motion::slide::step_dt;
use crate::motion::top_down::TOP_DOWN_MOVER;

/// [`PathFollowerPlugin`]'s system, in the move phase.
pub const PATH_FOLLOW: &str = "path_follow";

/// The scene component the plugin registers.
const PATH_FOLLOWER: &str = "path_follower";

/// What a follower does after its last point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    #[default]
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
/// [`ping_pong`](Self::ping_pong) back along the points. With an
/// [`Oscillator`](crate::Oscillator) on the body, its goal is the path's
/// point plus the wave's offset, so the platform rides the path with the
/// whole wave, and at a stopped follower's end the wave goes on.
///
/// It is data: a save holds its world points, speed, mode, easing and
/// time, so a loaded follower continues where it was, and a load checks
/// what [`new`](Self::new) checks plus a finite time within the path. A
/// game restoring a level object inserts the saved follower over the
/// prefab's.
///
/// Scene name `path_follower`:
/// `{ "points": [[0, 0], [0, -100]], "speed": 100, "mode": "ping_pong", "easing": "sine_in_out" }`,
/// the points offsets from the spawn position, `mode` one of `stop`
/// (default), `loop` and `ping_pong`, and `easing` one of core's `Easing`
/// names (`linear` by default).
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FollowerSave", into = "FollowerSave")]
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

    /// The points, in order.
    pub(crate) fn points(&self) -> &[Vec2] {
        &self.points
    }

    /// Whether it goes on from the last point to the first.
    pub(crate) fn is_looping(&self) -> bool {
        self.mode == Mode::Loop
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
    pub(crate) fn point(&self) -> Vec2 {
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

/// Registers the `path_follower` scene component and [`PATH_FOLLOW`] in the
/// move phase, before both controllers when they are registered, so a rider
/// reads where its platform goes this step, one of the kit's within-phase
/// edges (the [`Phase`] table lists them). Its system advances every
/// [`PathFollower`] with a `Position` and a
/// `Velocity` by the step, `Time::delta` bounded by
/// `PhysicsConfig::max_step_dt` as the physics step takes it; a step of 0
/// moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct PathFollowerPlugin;

impl Plugin for PathFollowerPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<PathFollower>(world, "PathFollower");
        describe(
            world,
            ComponentDescription::new(
                PATH_FOLLOWER,
                "Moves a kinematic body along points at a speed: a moving platform, a patrol.",
            )
            .field(FieldDescription::new(
                "points",
                FieldType::List(Box::new(FieldType::Vec2)),
                "two or more offsets from the spawn position, neighbours differing",
            ))
            .field(FieldDescription::new(
                "speed",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels per second along the path",
            ))
            .field(
                FieldDescription::new(
                    "mode",
                    FieldType::Enum(&["stop", "loop", "ping_pong"]),
                    "what it does after the last point",
                )
                .default("\"stop\""),
            )
            .field(
                FieldDescription::new(
                    "easing",
                    FieldType::Enum(EASING_NAMES),
                    "each segment's progress, one of core's easings",
                )
                .default("\"linear\""),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .sample(r#"{ "points": [[0, 0], [0, -100]], "speed": 100 }"#)
            .roundtrip(round_trip::<PathFollowerDef>),
        );
        world.register_component_with(
            PATH_FOLLOWER,
            |def: PathFollowerDef, at: &SpawnContext<'_>| {
                let origin = at.transform().position;
                let follower = PathFollower::new(
                    def.points.iter().map(|&p| origin + Vec2::from(p)),
                    def.speed,
                )
                .with_easing(def.easing);
                let follower = match def.mode {
                    Mode::Stop => follower,
                    Mode::Loop => follower.looping(),
                    Mode::PingPong => follower.ping_pong(),
                };
                (follower,)
            },
        );
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(PATH_FOLLOW, path_follow))
                .before_if_present(PLATFORMER_CONTROLLER)
                .before_if_present(TOP_DOWN_MOVER),
        );
    }
}

/// `components.path_follower` as a scene writes it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathFollowerFields {
    points: Vec<[f32; 2]>,
    #[serde(deserialize_with = "positive")]
    speed: f32,
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    easing: Easing,
}

/// `components.path_follower`, checked: the points as offsets.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "PathFollowerFields")]
struct PathFollowerDef {
    points: Vec<[f32; 2]>,
    speed: f32,
    mode: Mode,
    easing: Easing,
}

impl TryFrom<PathFollowerFields> for PathFollowerDef {
    type Error = String;

    fn try_from(fields: PathFollowerFields) -> Result<Self, Self::Error> {
        let points: Vec<Vec2> = fields.points.iter().copied().map(Vec2::from).collect();
        if points.len() < 2 {
            return Err(format!("points: needs at least 2 (got {})", points.len()));
        }
        if !points.iter().all(|point| point.is_finite()) {
            return Err(format!(
                "points: every point must be finite (got {points:?})"
            ));
        }
        if !points.windows(2).all(|pair| pair[0] != pair[1]) {
            return Err(format!(
                "points: neighbouring points must differ (got {points:?})"
            ));
        }
        if fields.mode == Mode::Loop && points.first() == points.last() {
            return Err(
                "points: with `loop`, the last point must differ from the first".to_string(),
            );
        }
        Ok(Self {
            points: fields.points,
            speed: fields.speed,
            mode: fields.mode,
            easing: fields.easing,
        })
    }
}

/// A follower as a save writes it: world points and its time.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FollowerSave {
    points: Vec<[f32; 2]>,
    speed: f32,
    mode: Mode,
    easing: Easing,
    time: f32,
}

impl TryFrom<FollowerSave> for PathFollower {
    type Error = String;

    fn try_from(save: FollowerSave) -> Result<Self, Self::Error> {
        let PathFollowerDef {
            points,
            speed,
            mode,
            easing,
        } = PathFollowerDef::try_from(PathFollowerFields {
            points: save.points,
            speed: save.speed,
            mode: save.mode,
            easing: save.easing,
        })?;
        if !(speed.is_finite() && speed > 0.0) {
            return Err(format!("speed: must be finite and > 0 (got {speed})"));
        }
        let follower = Self {
            points: points.into_iter().map(Vec2::from).collect(),
            speed,
            mode,
            easing,
            time: save.time,
        };
        let total = follower.total();
        let within = if mode == Mode::Stop {
            (0.0..=total).contains(&save.time)
        } else {
            (0.0..total).contains(&save.time)
        };
        if !within {
            let end = if mode == Mode::Stop { "]" } else { ")" };
            return Err(format!(
                "time: must be in [0, {total}{end} for this path (got {})",
                save.time
            ));
        }
        Ok(follower)
    }
}

impl From<PathFollower> for FollowerSave {
    fn from(follower: PathFollower) -> Self {
        Self {
            points: follower.points.iter().map(Vec2::to_array).collect(),
            speed: follower.speed,
            mode: follower.mode,
            easing: follower.easing,
            time: follower.time,
        }
    }
}

fn path_follow(world: &mut World) {
    let dt = step_dt(world);
    if dt <= 0.0 {
        return;
    }
    for (follower, position, velocity, oscillator) in world.query_mut::<(
        &mut PathFollower,
        &Position,
        &mut Velocity,
        Option<&Oscillator>,
    )>() {
        match oscillator.filter(|oscillator| oscillator.moves()) {
            None if follower.is_finished() => velocity.0 = Vec2::ZERO,
            None => {
                follower.advance(dt);
                velocity.0 = (follower.point() - position.0) / dt;
            }
            // The wave's offset at the oscillator's clock: when the
            // oscillator runs after this, it adds the step's change.
            Some(oscillator) => {
                if !follower.is_finished() {
                    follower.advance(dt);
                }
                velocity.0 = (follower.point() + oscillator.displacement() - position.0) / dt;
            }
        }
    }
}

impl Inspectable for PathFollower {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mode = match self.mode {
            Mode::Stop => "stop",
            Mode::Loop => "loop",
            Mode::PingPong => "ping_pong",
        };
        vec![
            ("points", self.points.len().to_string()),
            ("speed", format!("{:.1}", self.speed)),
            ("mode", mode.to_string()),
            ("time", format!("{:.2} of {:.2} s", self.time, self.total())),
            ("finished", self.is_finished().to_string()),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/path.rs"]
mod tests;
