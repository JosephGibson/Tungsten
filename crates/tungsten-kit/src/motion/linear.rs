//! Linear motion: an entity moved by a velocity that an acceleration
//! changes, such as a bullet, a drifting asteroid or a conveyor.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, PrevPosition};
use tungsten_core::{Entity, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::foundation::describe::{
    ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHMUP, describe, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{finite_vec2, vec2};
use crate::motion::drive::{Body, Skip, Warned, Write, body_of, step_position, write_velocity};
use crate::motion::platformer::PLATFORMER_CONTROLLER;
use crate::motion::slide::step_dt;
use crate::motion::top_down::TOP_DOWN_MOVER;

/// [`LinearMotionPlugin`]'s system, in the move phase.
pub const LINEAR_MOTION: &str = "linear_motion";

/// The scene component the plugin registers.
const LINEAR_MOTION_COMPONENT: &str = "linear_motion";

/// Moves its entity by a velocity that its acceleration changes: a bullet,
/// a drifting asteroid, a conveyor. [`LinearMotionPlugin`]'s system moves
/// it once a fixed step by `(v + a·dt/2)·dt`, then adds `a·dt` to `v`, which
/// is exact for a constant acceleration, so its path is the same at any
/// frame rate.
///
/// Without a `RigidBody` it moves its `Position`, with no cast, and writes
/// its velocity into a `Velocity` when the entity has one; the physics sync
/// draws it between its last two steps. A static body moves the same way.
/// On a kinematic body it is the body's driver: it writes the `Velocity`
/// the physics step moves the body by, so a platformer controller standing
/// on it rides it. A dynamic body keeps its own velocity and a child its
/// parent's offset: each is skipped, with one warning.
///
/// Scene name `linear_motion`: `{ "velocity": [0, 120], "acceleration": [0, 0] }`,
/// in world pixels per second and per second squared, `acceleration`
/// `[0, 0]` by default, which spawns a `Position` and a `PrevPosition` at
/// the spawn point, so a prefab without a `rigid_body` moves bodiless. It
/// is data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition};
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{LinearMotion, LinearMotionPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// LinearMotionPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A bomb dropped with a little forward speed, falling under gravity.
/// let fall = LinearMotion::new(Vec2::new(40.0, 0.0)).with_acceleration(Vec2::new(0.0, 980.0));
/// let bomb = world.spawn_with((fall, Position::new(0.0, 0.0), PrevPosition(Vec2::new(0.0, 0.0))));
/// for _ in 0..30 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// // Half a second: 20 px across and 980 · 0.5² / 2 down, exactly.
/// let at = world.get::<Position>(bomb).unwrap().0;
/// assert!((at - Vec2::new(20.0, 122.5)).length() < 1e-3, "{at}");
/// assert!((world.get::<LinearMotion>(bomb).unwrap().velocity().y - 490.0).abs() < 1e-3);
///
/// let saved = serde_json::to_string(world.get::<LinearMotion>(bomb).unwrap()).unwrap();
/// assert_eq!(&serde_json::from_str::<LinearMotion>(&saved).unwrap(), world.get(bomb).unwrap());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LinearMotionFields")]
pub struct LinearMotion {
    #[serde(with = "vec2")]
    velocity: Vec2,
    #[serde(with = "vec2")]
    acceleration: Vec2,
}

/// A linear motion as a save writes it, before its values are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinearMotionFields {
    #[serde(with = "vec2")]
    velocity: Vec2,
    #[serde(default, with = "vec2")]
    acceleration: Vec2,
}

impl TryFrom<LinearMotionFields> for LinearMotion {
    type Error = String;

    fn try_from(fields: LinearMotionFields) -> Result<Self, Self::Error> {
        for (field, value) in [
            ("velocity", fields.velocity),
            ("acceleration", fields.acceleration),
        ] {
            if !value.is_finite() {
                return Err(format!("{field}: must be finite (got {value})"));
            }
        }
        Ok(Self {
            velocity: fields.velocity,
            acceleration: fields.acceleration,
        })
    }
}

impl LinearMotion {
    /// A motion at `velocity` world pixels per second, unaccelerated.
    ///
    /// # Panics
    ///
    /// When `velocity` is not finite.
    #[must_use]
    #[track_caller]
    pub fn new(velocity: Vec2) -> Self {
        assert!(
            velocity.is_finite(),
            "LinearMotion::new: velocity must be finite (got {velocity})"
        );
        Self {
            velocity,
            acceleration: Vec2::ZERO,
        }
    }

    /// The motion, its velocity changed by `acceleration` world pixels per
    /// second squared.
    ///
    /// # Panics
    ///
    /// When `acceleration` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_acceleration(mut self, acceleration: Vec2) -> Self {
        assert!(
            acceleration.is_finite(),
            "LinearMotion::with_acceleration: acceleration must be finite (got {acceleration})"
        );
        self.acceleration = acceleration;
        self
    }

    /// The velocity it moves at from its next step.
    #[must_use]
    pub fn velocity(&self) -> Vec2 {
        self.velocity
    }

    /// Sets the velocity it moves at from its next step: a bounce, a turn.
    ///
    /// # Panics
    ///
    /// When `velocity` is not finite.
    #[track_caller]
    pub fn set_velocity(&mut self, velocity: Vec2) {
        assert!(
            velocity.is_finite(),
            "LinearMotion::set_velocity: velocity must be finite (got {velocity})"
        );
        self.velocity = velocity;
    }

    /// The acceleration.
    #[must_use]
    pub fn acceleration(&self) -> Vec2 {
        self.acceleration
    }
}

/// Registers the `linear_motion` scene component and [`LINEAR_MOTION`] in
/// the move phase, before both controllers when they are registered, so a
/// rider reads where its platform goes this step. Its system moves every
/// [`LinearMotion`] by the step, `Time::delta` bounded by
/// `PhysicsConfig::max_step_dt` as the physics step takes it; a step of 0
/// moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct LinearMotionPlugin;

impl Plugin for LinearMotionPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<LinearMotion>(world, "LinearMotion");
        describe(
            world,
            ComponentDescription::new(
                LINEAR_MOTION_COMPONENT,
                "Moves the entity by a velocity that an acceleration changes, bodiless or as a kinematic body's driver.",
            )
            .field(FieldDescription::new(
                "velocity",
                FieldType::Vec2,
                "world pixels per second at its first step",
            ))
            .field(
                FieldDescription::new(
                    "acceleration",
                    FieldType::Vec2,
                    "world pixels per second squared, changing the velocity",
                )
                .default("[0.0, 0.0]"),
            )
            .genre(PLATFORMER)
            .genre(SHMUP)
            .sample(r#"{ "velocity": [0, 120] }"#)
            .roundtrip(round_trip::<LinearMotionDef>),
        );
        world.register_component_with(
            LINEAR_MOTION_COMPONENT,
            |def: LinearMotionDef, at: &SpawnContext<'_>| {
                let spawn = at.transform().position;
                (
                    LinearMotion::new(def.velocity).with_acceleration(def.acceleration),
                    Position(spawn),
                    PrevPosition(spawn),
                )
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(LINEAR_MOTION, move |world: &mut World| {
                    runner.run(world);
                }))
                .before_if_present(PLATFORMER_CONTROLLER)
                .before_if_present(TOP_DOWN_MOVER),
        );
    }
}

/// `components.linear_motion` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LinearMotionDef {
    #[serde(with = "finite_vec2")]
    velocity: Vec2,
    #[serde(default, with = "finite_vec2")]
    acceleration: Vec2,
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's motions, kept so a step allocates nothing once warm.
    entities: Vec<Entity>,
    warned: Warned,
}

impl Runner {
    /// Moves every linear motion by the step and changes its velocity by
    /// its acceleration.
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &LinearMotion)>()
                .map(|(entity, _)| entity),
        );
        for &entity in &self.entities {
            let (velocity, acceleration) = {
                let motion = world.get::<LinearMotion>(entity).unwrap();
                (motion.velocity, motion.acceleration)
            };
            // The step's mean velocity: exact for a constant acceleration.
            let mean = velocity + acceleration * (dt * 0.5);
            match body_of(world, entity) {
                Body::Bodiless | Body::Static => {
                    step_position(world, entity, mean * dt, dt, Write::Set);
                }
                Body::Kinematic => write_velocity(world, entity, mean, Write::Set),
                Body::Dynamic => {
                    self.warned.skip("LinearMotion", entity, Skip::Dynamic);
                    continue;
                }
                Body::Child => {
                    self.warned.skip("LinearMotion", entity, Skip::Child);
                    continue;
                }
                Body::Unplaced => {
                    self.warned.skip("LinearMotion", entity, Skip::Unplaced);
                    continue;
                }
            }
            world.get_mut::<LinearMotion>(entity).unwrap().velocity += acceleration * dt;
        }
    }
}

impl Inspectable for LinearMotion {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("velocity", point(self.velocity)),
            ("acceleration", point(self.acceleration)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/linear.rs"]
mod tests;
