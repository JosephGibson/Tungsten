//! The path follower: a body moved along points, such as a moving
//! platform or a patrol.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, Velocity};
use tungsten_core::{Easing, Entity, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::TIME_EPSILON;
use crate::foundation::describe::{
    Bound, ComponentDescription, EASING_NAMES, FieldDescription, FieldType, PLATFORMER, SHOOTER,
    TOP_DOWN, describe, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{non_negative, positive};
use crate::motion::oscillator::Oscillator;
use crate::motion::platformer::PLATFORMER_CONTROLLER;
use crate::motion::slide::step_dt;
use crate::motion::top_down::TOP_DOWN_MOVER;
use crate::react::cues::{ARRIVED, DEPARTED, KitCue, cue_named, push_kit_cue};

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
/// A waiting follower ([`with_wait`](Self::with_wait), redo 7.10g) holds
/// its seconds at each end: a ping-pong's first and last points, a loop's
/// first, the time left over at an arrival dropped. A lift
/// ([`with_rider`](Self::with_rider)), ping-pong only, holds at its first
/// point until a rider has stood on it `board` seconds, runs to its last,
/// holds there while a rider stands on it and `back` seconds after the
/// last one left, and runs back for the next. A rider is a
/// [`PlatformerController`] whose ground it is: the controller marks it
/// after its move, and the follower reads the mark at its next step, one
/// step late, as a game's own lift reads its riders. It sends the cue
/// events `arrived` as it reaches an end, a stop path's last point
/// included, and `departed` as it leaves one, about itself at its point,
/// when a cue names them.
///
/// It is data: a save holds its world points, speed, mode, easing and
/// time, its wait, rider rule, the seconds of its hold and the rider's
/// mark, each skipped when none, so a loaded follower continues where it
/// was, and a load checks what [`new`](Self::new) checks plus a finite time
/// within the path. A game restoring a level object inserts the saved
/// follower over the prefab's.
///
/// Scene name `path_follower`:
/// `{ "points": [[0, 0], [0, -100]], "speed": 100, "mode": "ping_pong", "easing": "sine_in_out", "wait": 1, "rider": { "board": 0.3, "back": 4 } }`,
/// the points offsets from the spawn position, `mode` one of `stop`
/// (default), `loop` and `ping_pong`, `easing` one of core's `Easing`
/// names (`linear` by default), `wait` 0 by default and refused on a stop
/// path, and `rider` ping-pong only, none by default and refused beside a
/// `wait`.
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
    /// Boxed, so the follower keeps the size a game's capture of its
    /// platforms copies each frame.
    points: Box<[Vec2]>,
    speed: f32,
    mode: Mode,
    easing: Easing,
    /// Time along the path's legs, below one cycle unless it stopped.
    time: f32,
    /// A waiting or riding follower's hold; a plain follower has none.
    hold: Option<Box<Hold>>,
}

/// A follower's hold at its ends: its wait, its riders' rule, the seconds
/// counted at the end it holds at and the rider's mark.
#[derive(Debug, Clone, PartialEq)]
struct Hold {
    /// Seconds it holds at each end.
    wait: f32,
    /// A lift's rule for its riders.
    rider: Option<Rider>,
    /// Seconds counted at the end it holds at: held, boarded or alone.
    held: f32,
    /// A rider stood on it at its last step: a controller's mark, read and
    /// cleared each step.
    ridden: bool,
}

/// A lift's rule: it boards after a rider has stood on it `board` seconds,
/// and comes back `back` seconds after its last rider left.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rider {
    #[serde(default = "default_board", deserialize_with = "non_negative")]
    board: f32,
    #[serde(default = "default_back", deserialize_with = "non_negative")]
    back: f32,
}

fn default_board() -> f32 {
    0.3
}

fn default_back() -> f32 {
    4.0
}

/// The shape of a follower's `rider`.
const PATH_RIDER: &str = "path_rider";

/// What a step did at the path's ends, for its cue events.
#[derive(Debug, Clone, Copy, Default)]
struct Ends {
    departed: bool,
    arrived: bool,
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
            points: points.into_boxed_slice(),
            speed,
            mode: Mode::Stop,
            easing: Easing::Linear,
            time: 0.0,
            hold: None,
        }
    }

    /// Its hold's wait and rider rule, 0 and none without one.
    fn hold_rule(&self) -> (f32, Option<Rider>) {
        self.hold
            .as_ref()
            .map_or((0.0, None), |hold| (hold.wait, hold.rider))
    }

    /// Sets its hold's rule: none for a 0 wait without a rider.
    fn set_hold_rule(&mut self, wait: f32, rider: Option<Rider>) {
        self.hold = (wait > 0.0 || rider.is_some()).then(|| {
            Box::new(Hold {
                wait,
                rider,
                held: 0.0,
                ridden: false,
            })
        });
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

    /// The follower, holding `seconds` at each end: a ping-pong's first and
    /// last points, a loop's first; a patrol that pauses to look around.
    /// Set its mode first.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0, on a stop path, or
    /// beside a rider rule.
    #[must_use]
    #[track_caller]
    pub fn with_wait(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "PathFollower::with_wait: seconds must be finite and >= 0 (got {seconds})"
        );
        assert!(
            self.mode != Mode::Stop && self.hold_rule().1.is_none(),
            "PathFollower::with_wait: a loop or a ping-pong without a rider rule waits"
        );
        self.set_hold_rule(seconds, None);
        self
    }

    /// The ping-pong follower as a lift: it holds at its first point until
    /// a rider has stood on it `board` seconds, runs to its last, holds
    /// there while a rider stands on it and `back` seconds after the last
    /// one left, and runs back to wait for the next.
    ///
    /// # Panics
    ///
    /// When either is not finite and at least 0, on a follower that does
    /// not ping-pong, or beside a wait.
    #[must_use]
    #[track_caller]
    pub fn with_rider(mut self, board: f32, back: f32) -> Self {
        assert!(
            [board, back].iter().all(|s| s.is_finite() && *s >= 0.0),
            "PathFollower::with_rider: board and back must be finite and >= 0 (got {board}, {back})"
        );
        assert!(
            self.mode == Mode::PingPong && self.hold_rule().0 == 0.0,
            "PathFollower::with_rider: a ping-pong without a wait takes riders"
        );
        self.set_hold_rule(0.0, Some(Rider { board, back }));
        self
    }

    /// Marks it ridden for its next step, when it takes riders: what a
    /// [`PlatformerController`](crate::PlatformerController) standing on it
    /// does after its move.
    pub(crate) fn mark_ridden(&mut self) {
        if let Some(hold) = self.hold.as_mut()
            && hold.rider.is_some()
        {
            hold.ridden = true;
        }
    }

    /// Whether it holds at its ends.
    fn holds(&self) -> bool {
        self.hold.is_some()
    }

    /// The time it reaches its last point at, on a ping-pong's way out.
    fn out_time(&self) -> f32 {
        (0..self.points.len() - 1)
            .map(|k| {
                let (from, to) = self.leg(k);
                from.distance(to) / self.speed
            })
            .sum()
    }

    /// The end it stands at: `Some(false)` its first point, `Some(true)` a
    /// ping-pong's last, `None` between them or on a stop path.
    fn at_end(&self) -> Option<bool> {
        match self.mode {
            Mode::Stop => None,
            _ if self.time == 0.0 => Some(false),
            Mode::PingPong if self.time == self.out_time() => Some(true),
            _ => None,
        }
    }

    /// Whether its hold at the end `far` lets it go this step, counting
    /// `dt` and the rider's mark `ridden` toward it.
    fn release(&mut self, far: bool, dt: f32, ridden: bool) -> bool {
        let Some(hold) = self.hold.as_mut() else {
            return true;
        };
        let Some(rider) = hold.rider else {
            if hold.held + TIME_EPSILON >= hold.wait {
                hold.held = 0.0;
                return true;
            }
            hold.held += dt;
            return false;
        };
        let (due, counts) = if far {
            (rider.back, !ridden)
        } else {
            (rider.board, ridden)
        };
        if counts && hold.held + TIME_EPSILON >= due {
            hold.held = 0.0;
            return true;
        }
        hold.held = if counts { hold.held + dt } else { 0.0 };
        false
    }

    /// Its step: a hold at an end while it lasts, else `dt` along the path,
    /// stopping at an end it holds at; what it did at the ends.
    fn step(&mut self, dt: f32) -> Ends {
        let ridden = self
            .hold
            .as_mut()
            .is_some_and(|hold| std::mem::take(&mut hold.ridden));
        let mut ends = Ends::default();
        let from_end = self.at_end().is_some();
        if self.holds()
            && let Some(far) = self.at_end()
            && !self.release(far, dt, ridden)
        {
            return ends;
        }
        let before = self.time;
        self.advance(dt);
        ends.departed = from_end && self.time != before;
        let total = self.total();
        match self.mode {
            Mode::Stop => ends.arrived = before < total && self.time >= total,
            Mode::Loop => {
                if self.time < before {
                    ends.arrived = true;
                    if self.holds() {
                        self.time = 0.0;
                    } else {
                        ends.departed = true;
                    }
                }
            }
            Mode::PingPong => {
                let out = self.out_time();
                let wrapped = self.time < before;
                if !wrapped && before < out && self.time >= out {
                    ends.arrived = true;
                    if self.holds() {
                        self.time = out;
                    } else {
                        ends.departed = true;
                    }
                } else if wrapped {
                    ends.arrived = true;
                    if self.holds() {
                        self.time = 0.0;
                    } else {
                        ends.departed = true;
                    }
                }
            }
        }
        ends
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
        world.register_saved::<PathFollower>(PATH_FOLLOWER);
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
            .field(
                FieldDescription::new(
                    "wait",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds it holds at each end, a loop's first point or a ping-pong's two, the time left over at an arrival dropped; refused on a stop path",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "rider",
                    FieldType::Shape(PATH_RIDER),
                    "a ping-pong lift: it boards after a rider has stood on it, and comes back after its last rider left; refused beside a `wait`",
                )
                .default("null"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHOOTER)
            .sample(r#"{ "points": [[0, 0], [0, -100]], "speed": 100, "mode": "ping_pong" }"#)
            .roundtrip(round_trip::<PathFollowerDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                PATH_RIDER,
                "A lift's rule: it boards once a rider has stood on it, and comes back once its last rider has gone.",
            )
            .field(
                FieldDescription::new(
                    "board",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds a rider stands on it at its first point before it goes",
                )
                .default("0.3"),
            )
            .field(
                FieldDescription::new(
                    "back",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds after its last rider left its last point before it comes back",
                )
                .default("4.0"),
            )
            .sample("{}")
            .roundtrip(round_trip::<Rider>),
        );
        // The step's followers and what each did at its ends, reserved so
        // a step allocates nothing.
        let mut followers: Vec<(Entity, Ends)> = Vec::with_capacity(64);
        world.register_component_with(
            PATH_FOLLOWER,
            |def: PathFollowerDef, at: &SpawnContext<'_>| {
                let origin = at.transform().position;
                let follower = PathFollower::new(
                    def.points.iter().map(|&p| origin + Vec2::from(p)),
                    def.speed,
                )
                .with_easing(def.easing);
                let mut follower = match def.mode {
                    Mode::Stop => follower,
                    Mode::Loop => follower.looping(),
                    Mode::PingPong => follower.ping_pong(),
                };
                follower.set_hold_rule(def.wait, def.rider);
                (follower,)
            },
        );
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(PATH_FOLLOW, move |world: &mut World| {
                    path_follow(world, &mut followers);
                }))
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
    #[serde(default, deserialize_with = "non_negative")]
    wait: f32,
    #[serde(default)]
    rider: Option<Rider>,
}

/// `components.path_follower`, checked: the points as offsets.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "PathFollowerFields")]
struct PathFollowerDef {
    points: Vec<[f32; 2]>,
    speed: f32,
    mode: Mode,
    easing: Easing,
    wait: f32,
    rider: Option<Rider>,
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
        if fields.wait > 0.0 && fields.mode == Mode::Stop {
            return Err("wait: a stop path does not wait".to_string());
        }
        if fields.rider.is_some() && fields.mode != Mode::PingPong {
            return Err("rider: a lift ping-pongs".to_string());
        }
        if fields.rider.is_some() && fields.wait > 0.0 {
            return Err("rider: a lift does not wait beside its rider rule".to_string());
        }
        Ok(Self {
            points: fields.points,
            speed: fields.speed,
            mode: fields.mode,
            easing: fields.easing,
            wait: fields.wait,
            rider: fields.rider,
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
    #[serde(default, skip_serializing_if = "is_zero")]
    wait: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rider: Option<Rider>,
    #[serde(default, skip_serializing_if = "is_zero")]
    hold: f32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    ridden: bool,
}

// The reference is serde's `skip_serializing_if` signature.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &f32) -> bool {
    *value == 0.0
}

impl TryFrom<FollowerSave> for PathFollower {
    type Error = String;

    fn try_from(save: FollowerSave) -> Result<Self, Self::Error> {
        let PathFollowerDef {
            points,
            speed,
            mode,
            easing,
            wait,
            rider,
        } = PathFollowerDef::try_from(PathFollowerFields {
            points: save.points,
            speed: save.speed,
            mode: save.mode,
            easing: save.easing,
            wait: save.wait,
            rider: save.rider,
        })?;
        if !(save.hold.is_finite() && save.hold >= 0.0) {
            return Err(format!("hold: must be finite and >= 0 (got {})", save.hold));
        }
        if !(speed.is_finite() && speed > 0.0) {
            return Err(format!("speed: must be finite and > 0 (got {speed})"));
        }
        let mut follower = Self {
            points: points.into_iter().map(Vec2::from).collect(),
            speed,
            mode,
            easing,
            time: save.time,
            hold: None,
        };
        follower.set_hold_rule(wait, rider);
        if let Some(hold) = follower.hold.as_mut() {
            hold.held = save.hold;
            hold.ridden = save.ridden;
        }
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
            wait: follower.hold_rule().0,
            rider: follower.hold_rule().1,
            hold: follower.hold.as_ref().map_or(0.0, |hold| hold.held),
            ridden: follower.hold.as_ref().is_some_and(|hold| hold.ridden),
        }
    }
}

/// Steps every follower, then adds an oscillating body's wave to its
/// velocity and sends the step's `arrived` and `departed` cue events that
/// something names, through the kit's list. The query stays four columns
/// wide, the entity in the oscillator's place, since each column widens
/// the list a query collects each call, which example 01's guard pins;
/// `followers` is the step's scratch list, reserved at build.
fn path_follow(world: &mut World, followers: &mut Vec<(Entity, Ends)>) {
    let dt = step_dt(world);
    if dt <= 0.0 {
        return;
    }
    followers.clear();
    for (entity, follower, position, velocity) in
        world.query_mut::<(Entity, &mut PathFollower, &Position, &mut Velocity)>()
    {
        let moved = if follower.is_finished() {
            velocity.0 = Vec2::ZERO;
            Ends::default()
        } else {
            let moved = follower.step(dt);
            velocity.0 = (follower.point() - position.0) / dt;
            moved
        };
        followers.push((entity, moved));
    }
    for &(entity, moved) in followers.iter() {
        let Some(at) = world.get::<Position>(entity).map(|position| position.0) else {
            continue;
        };
        // The wave's offset at the oscillator's clock: when the
        // oscillator runs after this, it adds the step's change.
        let wave = world
            .get::<Oscillator>(entity)
            .filter(|oscillator| oscillator.moves())
            .map(Oscillator::displacement);
        let point = world.get::<PathFollower>(entity).map(PathFollower::point);
        if let (Some(wave), Some(point)) = (wave, point)
            && let Some(velocity) = world.get_mut::<Velocity>(entity)
        {
            velocity.0 = (point + wave - at) / dt;
        }
        for (happened, event, place) in [
            (moved.departed, DEPARTED, Some(at)),
            (moved.arrived, ARRIVED, point),
        ] {
            if happened
                && let Some(place) = place
                && cue_named(world, entity, event)
            {
                push_kit_cue(world, KitCue::about(event, entity, place));
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
