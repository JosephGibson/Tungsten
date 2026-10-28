//! Knockback: a push that throws an entity back from a hit, falling to 0
//! over its seconds.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{
    BodyKind, Position, PrevPosition, RigidBody, SpatialFilter, Velocity,
};
use tungsten_core::{Entity, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::count_down;
use crate::damage::health::Health;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{fraction, positive, vec2};
use crate::motion::drive::{
    Body, Skip, Warned, Write, body_of, driven, step_position, write_velocity,
};
use crate::motion::follow::FOLLOW;
use crate::motion::linear::LINEAR_MOTION;
use crate::motion::orbit::ORBIT;
use crate::motion::oscillator::OSCILLATOR;
use crate::motion::path::PATH_FOLLOW;
use crate::motion::platformer::{PlatformerController, kinematic_body};
use crate::motion::slide::{move_and_slide, step_dt};
use crate::motion::steering::STEERING;
use crate::motion::top_down::TOP_DOWN_MOVER;

/// [`KnockbackPlugin`]'s system, in the move phase.
pub const KNOCKBACK: &str = "knockback";

/// The scene component the plugin registers.
const KNOCKBACK_COMPONENT: &str = "knockback";

/// The default push time, in seconds.
const DEFAULT_SECONDS: f32 = 0.2;

/// How its entity takes a push from [`knock_back`]: a hero thrown back from
/// a hazard, an enemy staggered by a blow. A push of `velocity × (1 −
/// resist)` replaces one that runs:
///
/// - on a [`PlatformerController`] it launches the controller and locks
///   its input for the seconds, so the controller's gravity and walls
///   carry it;
/// - on a dynamic body it adds the push to the body's `Velocity` once;
/// - otherwise [`KnockbackPlugin`]'s system adds the push, falling linearly
///   to 0 over the seconds, to the entity's motion each step, after the
///   kit's movers: a bodiless entity's `Position` (and its `Velocity` when
///   it has one) with no cast; a kinematic body slides by a cast from where
///   its driver's velocity ends, so a top-down mover's walls hold it, and
///   the slide joins the body's `Velocity`, which is the slide alone when
///   no kit mover drives the body, and zero on the step the push ends.
///
/// A static body and a child take no push, with one warning.
///
/// Scene name `knockback`: `{ "seconds": 0.2, "resist": 0 }`, both with
/// those defaults, `resist` in `[0, 1]`, which spawns a `Position` and a
/// `PrevPosition` at the spawn point, so a prefab without a `rigid_body`
/// is pushed bodiless. It is data: a game's save can hold it, a push that
/// runs included.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition};
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Knockback, KnockbackPlugin, knock_back};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// KnockbackPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let take = Knockback::new(0.25).with_resist(0.5);
/// let brute = world.spawn_with((take, Position::new(0.0, 0.0), PrevPosition(Vec2::new(0.0, 0.0))));
/// assert!(knock_back(&mut world, brute, Vec2::new(-400.0, 0.0)));
/// let knockback = world.get::<Knockback>(brute).unwrap();
/// assert!(knockback.is_active());
/// assert_eq!(knockback.push(), Vec2::new(-200.0, 0.0), "half resisted");
///
/// // The push falls to 0 over its quarter second: 200 · 0.25 / 2 px back.
/// for _ in 0..20 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// let at = world.get::<Position>(brute).unwrap().0;
/// assert!((at.x + 25.0).abs() < 1e-3 && at.y == 0.0, "{at}");
/// assert!(!world.get::<Knockback>(brute).unwrap().is_active());
///
/// // An entity without a `Knockback` takes no push.
/// let wall = world.spawn();
/// assert!(!knock_back(&mut world, wall, Vec2::new(-400.0, 0.0)));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "KnockbackFields", into = "KnockbackFields")]
pub struct Knockback {
    seconds: f32,
    resist: f32,
    /// The push's velocity as it started.
    push: Vec2,
    /// Seconds of the push left.
    left: f32,
}

/// A knockback as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KnockbackFields {
    #[serde(default = "default_seconds")]
    seconds: f32,
    #[serde(default)]
    resist: f32,
    #[serde(default, with = "vec2")]
    push: Vec2,
    #[serde(default)]
    left: f32,
}

fn default_seconds() -> f32 {
    DEFAULT_SECONDS
}

impl TryFrom<KnockbackFields> for Knockback {
    type Error = String;

    fn try_from(fields: KnockbackFields) -> Result<Self, Self::Error> {
        if !(fields.seconds.is_finite() && fields.seconds > 0.0) {
            return Err(format!(
                "seconds: must be finite and > 0 (got {})",
                fields.seconds
            ));
        }
        if !(0.0..=1.0).contains(&fields.resist) {
            return Err(format!("resist: must be in [0, 1] (got {})", fields.resist));
        }
        if !fields.push.is_finite() {
            return Err(format!("push: must be finite (got {})", fields.push));
        }
        if !(0.0..=fields.seconds).contains(&fields.left) {
            return Err(format!(
                "left: must be in [0, seconds] (got {})",
                fields.left
            ));
        }
        Ok(Self {
            seconds: fields.seconds,
            resist: fields.resist,
            push: fields.push,
            left: fields.left,
        })
    }
}

impl From<Knockback> for KnockbackFields {
    fn from(knockback: Knockback) -> Self {
        Self {
            seconds: knockback.seconds,
            resist: knockback.resist,
            push: knockback.push,
            left: knockback.left,
        }
    }
}

impl Knockback {
    /// A knockback whose pushes fall to 0 over `seconds` of game time,
    /// resisting nothing.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn new(seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds > 0.0,
            "Knockback::new: seconds must be finite and > 0 (got {seconds})"
        );
        Self {
            seconds,
            resist: 0.0,
            push: Vec2::ZERO,
            left: 0.0,
        }
    }

    /// The knockback, taking `1 − resist` of each push: 1 resists it all.
    ///
    /// # Panics
    ///
    /// When `resist` is outside `[0, 1]`.
    #[must_use]
    #[track_caller]
    pub fn with_resist(mut self, resist: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&resist),
            "Knockback::with_resist: resist must be in [0, 1] (got {resist})"
        );
        self.resist = resist;
        self
    }

    /// The push's velocity now: its start, falling linearly to 0 over the
    /// seconds; zero when no push runs.
    #[must_use]
    pub fn push(&self) -> Vec2 {
        self.push * (self.left / self.seconds)
    }

    /// Whether a push runs.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.left > 0.0
    }
}

/// Pushes `entity` back by `velocity` world pixels per second, as its
/// [`Knockback`] takes a push: `velocity × (1 − resist)`, replacing a push
/// that runs. On a [`PlatformerController`] it launches the controller and
/// locks its input for the knockback's seconds; on a dynamic body it adds
/// the push to the body's `Velocity` at once; otherwise the knockback's
/// system adds the push to the entity's motion over its seconds. A whole
/// resist (`resist` 1) stands firm: true, with no push, launch or lock.
/// False, pushing nothing, for an entity without a `Knockback`, a dead one
/// (its [`Health`] at 0) or a velocity that is not finite.
pub fn knock_back(world: &mut World, entity: Entity, velocity: Vec2) -> bool {
    if !velocity.is_finite() || world.get::<Health>(entity).is_some_and(Health::is_dead) {
        return false;
    }
    let Some(knockback) = world.get_mut::<Knockback>(entity) else {
        return false;
    };
    if knockback.resist >= 1.0 {
        return true;
    }
    let push = velocity * (1.0 - knockback.resist);
    let seconds = knockback.seconds;
    knockback.push = push;
    knockback.left = seconds;
    if let Some(controller) = world.get_mut::<PlatformerController>(entity) {
        controller.launch(push);
        controller.lock_input(seconds);
    } else if world
        .get::<RigidBody>(entity)
        .is_some_and(|body| body.kind == BodyKind::Dynamic)
        && let Some(body_velocity) = world.get_mut::<Velocity>(entity)
    {
        body_velocity.0 += push;
    }
    true
}

/// Registers the `knockback` scene component and [`KNOCKBACK`] in the move
/// phase, after the path follower, the linear motion, the oscillator, the
/// top-down mover, the steering, the follower and the orbit, each when
/// registered, whose velocity it adds to. Its system runs every
/// [`Knockback`]'s push by the step, `Time::delta` bounded by
/// `PhysicsConfig::max_step_dt` as the physics step takes it; a step of 0
/// moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct KnockbackPlugin;

impl Plugin for KnockbackPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Knockback>(world, "Knockback");
        describe(
            world,
            ComponentDescription::new(
                KNOCKBACK_COMPONENT,
                "How the entity takes a push from `knock_back`: a push that falls to 0 over its seconds.",
            )
            .field(
                FieldDescription::new(
                    "seconds",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds of game time a push lasts, and a platformer's input stays locked",
                )
                .default("0.2"),
            )
            .field(
                FieldDescription::new(
                    "resist",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Inclusive(1.0),
                    },
                    "the part of each push it ignores",
                )
                .default("0.0"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .sample("{}")
            .roundtrip(round_trip::<KnockbackDef>),
        );
        world.register_component_with(
            KNOCKBACK_COMPONENT,
            |def: KnockbackDef, at: &SpawnContext<'_>| {
                let spawn = at.transform().position;
                (
                    Knockback::new(def.seconds).with_resist(def.resist),
                    Position(spawn),
                    PrevPosition(spawn),
                )
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(KNOCKBACK, move |world: &mut World| {
                    runner.run(world);
                }))
                .after_if_present(PATH_FOLLOW)
                .after_if_present(LINEAR_MOTION)
                .after_if_present(OSCILLATOR)
                .after_if_present(TOP_DOWN_MOVER)
                .after_if_present(STEERING)
                .after_if_present(FOLLOW)
                .after_if_present(ORBIT),
        );
    }
}

/// `components.knockback` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct KnockbackDef {
    #[serde(default = "default_seconds", deserialize_with = "positive")]
    seconds: f32,
    #[serde(default, deserialize_with = "fraction")]
    resist: f32,
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's knockbacks, kept so a step allocates nothing once warm.
    entities: Vec<Entity>,
    warned: Warned,
}

impl Runner {
    /// Runs every push by the step: its time down, and its part of the
    /// push, falling linearly to 0, added to the entity's motion.
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &Knockback)>()
                .map(|(entity, _)| entity),
        );
        for &entity in &self.entities {
            let (moved, settling) = {
                let knockback = world.get_mut::<Knockback>(entity).unwrap();
                let (left, push) = (knockback.left, knockback.push);
                if left <= 0.0 && push == Vec2::ZERO {
                    continue;
                }
                // The push's velocity falls linearly from `push` at the full
                // seconds to 0: this step's part is its integral over the
                // time it spans.
                let after = count_down(left, dt);
                let moved = push / knockback.seconds * ((left * left - after * after) * 0.5);
                knockback.left = after;
                // The step after the push ends: a driverless body is stopped,
                // and the push forgotten.
                let settling = left <= 0.0;
                if settling {
                    knockback.push = Vec2::ZERO;
                }
                (moved, settling)
            };
            // A controller's launch and a dynamic body's kick were the
            // push, given at `knock_back`.
            if world.has::<PlatformerController>(entity) {
                continue;
            }
            let body = body_of(world, entity);
            let skip = match body {
                Body::Child => Some(Skip::Child),
                Body::Static => Some(Skip::Static),
                Body::Unplaced => Some(Skip::Unplaced),
                Body::Dynamic | Body::Bodiless | Body::Kinematic => None,
            };
            if let Some(skip) = skip {
                self.warned.skip("Knockback", entity, skip);
                continue;
            }
            let write = if driven(world, entity) {
                Write::Add
            } else {
                Write::Set
            };
            match body {
                Body::Bodiless => step_position(world, entity, moved, dt, write),
                Body::Kinematic if settling => {
                    if write == Write::Set {
                        write_velocity(world, entity, Vec2::ZERO, Write::Set);
                    }
                }
                Body::Kinematic => {
                    let Some((position, collider)) = kinematic_body(world, entity) else {
                        continue;
                    };
                    // From where the driver's velocity ends, so its walls
                    // hold the push too.
                    let ends = match write {
                        Write::Add => world.get::<Velocity>(entity).unwrap().0 * dt,
                        Write::Set => Vec2::ZERO,
                    };
                    let slide = move_and_slide(
                        world,
                        collider.shape,
                        position + collider.offset + ends,
                        moved,
                        &SpatialFilter::new().excluding(entity),
                    );
                    write_velocity(world, entity, slide.moved / dt, write);
                }
                _ => {}
            }
        }
    }
}

impl Inspectable for Knockback {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("push", point(self.push())),
            ("left", format!("{:.2} of {:.2} s", self.left, self.seconds)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/knockback.rs"]
mod tests;
