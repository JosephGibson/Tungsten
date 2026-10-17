//! Follow: an entity placed on a target's point plus an offset, closing
//! the gap with a lag and a top speed.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, PrevPosition};
use tungsten_core::{Entity, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{finite_vec2, non_negative, optional_positive, vec2};
use crate::motion::drive::{Body, Skip, Warned, Write, body_of, step_position, write_velocity};
use crate::motion::linear::LINEAR_MOTION;
use crate::motion::oscillator::{OSCILLATOR, Oscillator};
use crate::motion::path::PATH_FOLLOW;
use crate::motion::platformer::PLATFORMER_CONTROLLER;
use crate::motion::slide::step_dt;
use crate::motion::steering::STEERING;
use crate::motion::target::{Measure, Resolver, TARGET_SHAPE, Target, TargetDef, describe_target};
use crate::motion::top_down::TOP_DOWN_MOVER;

/// [`FollowPlugin`]'s system, in the move phase.
pub const FOLLOW: &str = "follow";

/// The scene component the plugin registers.
const FOLLOW_COMPONENT: &str = "follow";

/// Places its entity on a [`Target`]'s point plus an offset: a pet at its
/// owner's heels, a firefly trailing the hero, a health bar over a
/// monster. Each step [`FollowPlugin`]'s system closes `1 − e^(−dt/lag)` of
/// the gap to its goal, all of it at a lag of 0, and at most
/// `max_speed · dt` when it has a top speed, so the follower eases in the
/// same way at any frame rate; while no target resolves it holds.
///
/// Its goal is the target's point (its `Position`, else its
/// `Transform.position`) where the target's movers leave it this step: the
/// system runs after the kit's movers, and a kinematic or dynamic target's
/// point is taken at the step's end. A knockback, a speed cap, a wrap or a
/// clamp that corrects the target later in the step reaches the follower
/// on the next one. With an [`Oscillator`](crate::Oscillator) on the
/// follower, its offset joins the goal, so a follower can bob about its
/// follow point.
///
/// Without a `RigidBody` it sets its `Position`, and a `Velocity` when it
/// has one; on a kinematic body it drives the body, its `Velocity` reaching
/// the goal at the step's end, zero while it holds. A dynamic body keeps
/// its own velocity and a child its parent's offset: each is skipped, with
/// one warning.
///
/// Scene name `follow`:
/// `{ "target": { "source": {} }, "offset": [0, -24], "lag": 0.15, "max_speed": 300 }`,
/// `offset` `[0, 0]`, `lag` 0 and `max_speed` none by default, which spawns
/// a `Position` and a `PrevPosition` at the spawn point, so a prefab
/// without a `rigid_body` moves bodiless. It is data: a game's save can
/// hold it, without the entity it followed, which the next step resolves
/// again.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition};
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Follow, FollowPlugin, TagFilter, Tags, Target};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// FollowPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A pet that trails the nearest player a little above its point.
/// let hero = world.spawn_with((Tags::new(["team.player"]), Position::new(100.0, 50.0)));
/// let follow = Follow::new(Target::nearest(TagFilter::new().all(["team.player"])))
///     .with_offset(Vec2::new(0.0, -24.0))
///     .with_lag(0.15);
/// let pet = world.spawn_with((follow, Position::new(0.0, 0.0), PrevPosition(Vec2::new(0.0, 0.0))));
/// assert_eq!(world.get::<Follow>(pet).unwrap().followed(), None, "nothing before its first step");
/// for _ in 0..120 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// // Two seconds of a 0.15 s lag: on its goal, 24 px above the hero.
/// let at = world.get::<Position>(pet).unwrap().0;
/// assert!((at - Vec2::new(100.0, 26.0)).length() < 0.01, "{at}");
/// assert_eq!(world.get::<Follow>(pet).unwrap().followed(), Some(hero));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "FollowFields", into = "FollowFields")]
pub struct Follow {
    target: Target,
    offset: Vec2,
    lag: f32,
    max_speed: Option<f32>,
    /// The entity it followed at its last step.
    followed: Option<Entity>,
    /// Its goal at its last step, for the debug drawing.
    goal: Option<Vec2>,
}

/// A follower as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FollowFields {
    target: Target,
    #[serde(default, with = "vec2")]
    offset: Vec2,
    #[serde(default)]
    lag: f32,
    #[serde(default)]
    max_speed: Option<f32>,
}

impl TryFrom<FollowFields> for Follow {
    type Error = String;

    fn try_from(fields: FollowFields) -> Result<Self, Self::Error> {
        if !fields.offset.is_finite() {
            return Err(format!("offset: must be finite (got {})", fields.offset));
        }
        if !(fields.lag.is_finite() && fields.lag >= 0.0) {
            return Err(format!("lag: must be finite and >= 0 (got {})", fields.lag));
        }
        if let Some(speed) = fields.max_speed
            && !(speed.is_finite() && speed > 0.0)
        {
            return Err(format!("max_speed: must be finite and > 0 (got {speed})"));
        }
        Ok(Self {
            target: fields.target,
            offset: fields.offset,
            lag: fields.lag,
            max_speed: fields.max_speed,
            followed: None,
            goal: None,
        })
    }
}

impl From<Follow> for FollowFields {
    fn from(follow: Follow) -> Self {
        Self {
            target: follow.target,
            offset: follow.offset,
            lag: follow.lag,
            max_speed: follow.max_speed,
        }
    }
}

impl Follow {
    /// A follower that sits on `target`'s point, with no offset, no lag and
    /// no top speed.
    #[must_use]
    pub fn new(target: Target) -> Self {
        Self {
            target,
            offset: Vec2::ZERO,
            lag: 0.0,
            max_speed: None,
            followed: None,
            goal: None,
        }
    }

    /// The follower, aiming `offset` world pixels from the target's point.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        assert!(
            offset.is_finite(),
            "Follow::with_offset: offset must be finite (got {offset})"
        );
        self.offset = offset;
        self
    }

    /// The follower, closing `1 − e^(−dt/lag)` of the gap each step: about
    /// 63% of it each `lag` seconds.
    ///
    /// # Panics
    ///
    /// When `lag` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_lag(mut self, lag: f32) -> Self {
        assert!(
            lag.is_finite() && lag >= 0.0,
            "Follow::with_lag: lag must be finite and >= 0 (got {lag})"
        );
        self.lag = lag;
        self
    }

    /// The follower, moving at most `speed` world pixels per second.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_max_speed(mut self, speed: f32) -> Self {
        assert!(
            speed.is_finite() && speed > 0.0,
            "Follow::with_max_speed: speed must be finite and > 0 (got {speed})"
        );
        self.max_speed = Some(speed);
        self
    }

    /// The entity it followed at its last step: `None` before its first
    /// step, while a point is its target, or while no target resolves.
    #[must_use]
    pub fn followed(&self) -> Option<Entity> {
        self.followed
    }

    /// Its goal at its last step: `None` before the first, or while no
    /// target resolves.
    pub(crate) fn goal(&self) -> Option<Vec2> {
        self.goal
    }
}

/// Registers the `follow` scene component and [`FOLLOW`] in the move phase,
/// after the path follower, the linear motion, the oscillator, both
/// controllers and the steering, each when registered, so a follower takes
/// its target where those movers leave it. Its system moves every
/// [`Follow`] by the step, `Time::delta` bounded by
/// `PhysicsConfig::max_step_dt` as the physics step takes it; a step of 0
/// moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct FollowPlugin;

impl Plugin for FollowPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Follow>(world, "Follow");
        describe_target(world);
        describe(
            world,
            ComponentDescription::new(
                FOLLOW_COMPONENT,
                "Places the entity on a target's point plus an offset, closing the gap with a lag and a top speed.",
            )
            .field(FieldDescription::new(
                "target",
                FieldType::Shape(TARGET_SHAPE),
                "what it follows; the load checks its tags",
            ))
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::Vec2,
                    "world pixels from the target's point",
                )
                .default("[0.0, 0.0]"),
            )
            .field(
                FieldDescription::new(
                    "lag",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds to close about 63% of the gap; 0 sits on the goal",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "max_speed",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second at most; none when left out",
                )
                .default("null"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .sample(r#"{ "target": { "source": {} } }"#)
            .roundtrip(round_trip::<FollowDef>),
        );
        world.register_component_with_assets(
            FOLLOW_COMPONENT,
            |def: &FollowDef, world: &World| def.target.check(world, "target"),
            |def: FollowDef, at: &SpawnContext<'_>| {
                let spawn = at.transform().position;
                let mut follow = Follow::new(def.target.into())
                    .with_offset(def.offset)
                    .with_lag(def.lag);
                if let Some(speed) = def.max_speed {
                    follow = follow.with_max_speed(speed);
                }
                (follow, Position(spawn), PrevPosition(spawn))
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(FOLLOW, move |world: &mut World| runner.run(world)))
                .after_if_present(PATH_FOLLOW)
                .after_if_present(LINEAR_MOTION)
                .after_if_present(OSCILLATOR)
                .after_if_present(PLATFORMER_CONTROLLER)
                .after_if_present(TOP_DOWN_MOVER)
                .after_if_present(STEERING),
        );
    }
}

/// `components.follow` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FollowDef {
    target: TargetDef,
    #[serde(default, with = "finite_vec2")]
    offset: Vec2,
    #[serde(default, deserialize_with = "non_negative")]
    lag: f32,
    #[serde(default, deserialize_with = "optional_positive")]
    max_speed: Option<f32>,
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's followers, kept so a step allocates nothing once warm.
    entities: Vec<Entity>,
    resolver: Resolver,
    warned: Warned,
}

impl Runner {
    /// Moves every follower toward its goal by the step.
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.resolver.begin();
        self.entities.clear();
        self.entities
            .extend(world.query::<(Entity, &Follow)>().map(|(entity, _)| entity));
        for &entity in &self.entities {
            let body = body_of(world, entity);
            let skip = match body {
                Body::Child => Some(Skip::Child),
                Body::Dynamic => Some(Skip::Dynamic),
                Body::Unplaced => Some(Skip::Unplaced),
                Body::Bodiless | Body::Static | Body::Kinematic => None,
            };
            if let Some(skip) = skip {
                self.warned.skip("Follow", entity, skip);
                continue;
            }
            let from = world.get::<Position>(entity).unwrap().0;
            let follow = world.get::<Follow>(entity).unwrap();
            let resolved =
                self.resolver
                    .resolve(world, &follow.target, entity, from, Measure::Placed(dt));
            let wave = world
                .get::<Oscillator>(entity)
                .map_or(Vec2::ZERO, Oscillator::displacement);
            let goal = resolved.map(|(_, at)| at + follow.offset + wave);
            let step = goal.map_or(Vec2::ZERO, |goal| {
                let gap = goal - from;
                let closed = if follow.lag > 0.0 {
                    gap * (1.0 - (-dt / follow.lag).exp())
                } else {
                    gap
                };
                follow
                    .max_speed
                    .map_or(closed, |speed| closed.clamp_length_max(speed * dt))
            });
            if body == Body::Kinematic {
                write_velocity(world, entity, step / dt, Write::Set);
            } else {
                step_position(world, entity, step, dt, Write::Set);
            }
            let follow = world.get_mut::<Follow>(entity).unwrap();
            follow.followed = resolved.and_then(|(followed, _)| followed);
            follow.goal = goal;
        }
    }
}

impl Inspectable for Follow {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let followed = self
            .followed
            .map_or_else(|| "none".to_string(), |entity| format!("{entity:?}"));
        vec![
            ("target", format!("{}: {followed}", self.target.summary())),
            ("offset", point(self.offset)),
            ("lag", format!("{:.2} s", self.lag)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/follow.rs"]
mod tests;
