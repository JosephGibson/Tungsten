//! The speed limit: a cap on an entity's speed, its length and each axis.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{BodyKind, Position, PrevPosition, RigidBody, Velocity};
use tungsten_core::{Entity, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, SHMUP, describe,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::optional_positive;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::motion::drive::{Body, Skip, Warned, body_of};
use crate::motion::follow::FOLLOW;
use crate::motion::knockback::KNOCKBACK;
use crate::motion::linear::{LINEAR_MOTION, LinearMotion};
use crate::motion::orbit::ORBIT;
use crate::motion::oscillator::OSCILLATOR;
use crate::motion::path::PATH_FOLLOW;
use crate::motion::platformer::{PLATFORMER_CONTROLLER, PlatformerController};
use crate::motion::slide::step_dt;
use crate::motion::steering::{STEERING, Steering};
use crate::motion::top_down::TOP_DOWN_MOVER;

/// [`SpeedLimitPlugin`]'s system before the physics step, last in the move
/// phase.
pub const SPEED_LIMIT: &str = "speed_limit";

/// [`SpeedLimitPlugin`]'s system after the physics step, in the overlap
/// phase: a dynamic body's velocity capped again after the solver.
pub const SPEED_LIMIT_SETTLE: &str = "speed_limit_settle";

/// The scene component the plugin registers.
const SPEED_LIMIT_COMPONENT: &str = "speed_limit";

/// Caps its entity's speed: the length first, then each axis, each cap
/// optional. A falling hero's terminal speed, a ship's top speed, an
/// accelerating drifter that tops out.
///
/// [`SpeedLimitPlugin`]'s first system runs last in the move phase, after
/// the kit's movers: it caps every body's `Velocity` and the velocities the
/// movers keep (a platformer controller's, a linear motion's, a
/// steering's), so their next step starts capped, and a bodiless entity's
/// move this step (`Position − PrevPosition`), with its `Velocity` when it
/// has one. So a kinematic body or a bodiless entity never moves past the
/// cap in a step, and a dynamic body enters every step at or under it,
/// whatever a knockback or its spawn gave it. Its second system caps a
/// dynamic body's `Velocity` again after the solver, so the body also
/// leaves every step at or under it. Inside a step the solver adds gravity
/// and the contacts' impulses, so only a dynamic body in free fall,
/// touching and pushed by nothing, has a bound on its move: at most
/// `(cap + g·dt)·dt` a step.
///
/// Scene name `speed_limit`: `{ "speed": 600 }`, `{ "x": 240 }`,
/// `{ "y": 900 }` or any combination, at least one, each in world pixels
/// per second above 0. It is data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{PhysicsPlugin, Position, PrevPosition};
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{LinearMotion, LinearMotionPlugin, SpeedLimit, SpeedLimitPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// PhysicsPlugin.build(&mut schedule, &mut world);
/// LinearMotionPlugin.build(&mut schedule, &mut world);
/// SpeedLimitPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A drifter that speeds up at 600 px/s² and tops out at 150 px/s.
/// let drift = LinearMotion::new(Vec2::ZERO).with_acceleration(Vec2::new(0.0, 600.0));
/// let drifter = world.spawn_with((
///     drift,
///     SpeedLimit::speed(150.0),
///     Position::new(0.0, 0.0),
///     PrevPosition(Vec2::new(0.0, 0.0)),
/// ));
/// for _ in 0..60 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     let at = world.get::<Position>(drifter).unwrap().0;
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     let step = world.get::<Position>(drifter).unwrap().0 - at;
///     assert!(step.length() <= 150.0 / 60.0 + 1e-4, "never past the cap: {step}");
/// }
/// assert_eq!(world.get::<LinearMotion>(drifter).unwrap().velocity(), Vec2::new(0.0, 150.0));
///
/// let caps = SpeedLimit::x(240.0).with_y(900.0);
/// let saved = serde_json::to_value(caps).unwrap();
/// assert_eq!(saved, serde_json::json!({ "x": 240.0, "y": 900.0 }));
/// assert_eq!(serde_json::from_value::<SpeedLimit>(saved).unwrap(), caps);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(from = "SpeedLimitDef", into = "SpeedLimitDef")]
pub struct SpeedLimit {
    speed: Option<f32>,
    x: Option<f32>,
    y: Option<f32>,
}

impl SpeedLimit {
    /// A cap of `speed` world pixels per second on the velocity's length.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn speed(speed: f32) -> Self {
        Self::NONE.with_speed(speed)
    }

    /// A cap of `speed` world pixels per second on the horizontal speed.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn x(speed: f32) -> Self {
        Self::NONE.with_x(speed)
    }

    /// A cap of `speed` world pixels per second on the vertical speed.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn y(speed: f32) -> Self {
        Self::NONE.with_y(speed)
    }

    /// The limit, also capping the velocity's length at `speed`.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = Some(assert_cap("with_speed", speed));
        self
    }

    /// The limit, also capping the horizontal speed at `speed`.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_x(mut self, speed: f32) -> Self {
        self.x = Some(assert_cap("with_x", speed));
        self
    }

    /// The limit, also capping the vertical speed at `speed`.
    ///
    /// # Panics
    ///
    /// When `speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_y(mut self, speed: f32) -> Self {
        self.y = Some(assert_cap("with_y", speed));
        self
    }

    /// No cap yet: what the constructors start from.
    const NONE: Self = Self {
        speed: None,
        x: None,
        y: None,
    };

    /// `velocity` capped: its length first, then each axis.
    fn cap(&self, velocity: Vec2) -> Vec2 {
        let mut velocity = match self.speed {
            Some(speed) => velocity.clamp_length_max(speed),
            None => velocity,
        };
        if let Some(x) = self.x {
            velocity.x = velocity.x.clamp(-x, x);
        }
        if let Some(y) = self.y {
            velocity.y = velocity.y.clamp(-y, y);
        }
        velocity
    }
}

#[track_caller]
fn assert_cap(method: &str, speed: f32) -> f32 {
    assert!(
        speed.is_finite() && speed > 0.0,
        "SpeedLimit::{method}: speed must be finite and > 0 (got {speed})"
    );
    speed
}

/// Registers the `speed_limit` scene component, [`SPEED_LIMIT`] last in the
/// move phase, after the path follower, the linear motion, the oscillator,
/// both controllers, the steering, the follower, the orbit and the
/// knockback, each when registered, and [`SPEED_LIMIT_SETTLE`] in the
/// overlap phase.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpeedLimitPlugin;

impl Plugin for SpeedLimitPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<SpeedLimit>(world, "SpeedLimit");
        let cap = |name: &'static str, doc: &'static str| {
            FieldDescription::new(
                name,
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                doc,
            )
            .default("null")
        };
        describe(
            world,
            ComponentDescription::new(
                SPEED_LIMIT_COMPONENT,
                "Caps the entity's speed, the length first, then each axis; at least one of `speed`, `x` and `y`.",
            )
            .field(cap(
                "speed",
                "world pixels per second the velocity's length stays at or under",
            ))
            .field(cap(
                "x",
                "world pixels per second the horizontal speed stays at or under",
            ))
            .field(cap(
                "y",
                "world pixels per second the vertical speed stays at or under",
            ))
            .genre(PLATFORMER)
            .genre(SHMUP)
            .sample(r#"{ "speed": 600 }"#)
            .roundtrip(round_trip::<SpeedLimitDef>),
        );
        world.register_component_with(
            SPEED_LIMIT_COMPONENT,
            |def: SpeedLimitDef, _: &SpawnContext<'_>| (SpeedLimit::from(def),),
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(SPEED_LIMIT, move |world: &mut World| {
                    runner.run(world);
                }))
                .after_if_present(PATH_FOLLOW)
                .after_if_present(LINEAR_MOTION)
                .after_if_present(OSCILLATOR)
                .after_if_present(PLATFORMER_CONTROLLER)
                .after_if_present(TOP_DOWN_MOVER)
                .after_if_present(STEERING)
                .after_if_present(FOLLOW)
                .after_if_present(ORBIT)
                .after_if_present(KNOCKBACK),
        );
        schedule.add(
            Phase::Overlap.stage(),
            Phase::Overlap.join(system(SPEED_LIMIT_SETTLE, speed_limit_settle)),
        );
    }
}

/// `components.speed_limit` as a scene and a save write it, before at
/// least one cap is checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeedLimitFields {
    #[serde(default, deserialize_with = "optional_positive")]
    speed: Option<f32>,
    #[serde(default, deserialize_with = "optional_positive")]
    x: Option<f32>,
    #[serde(default, deserialize_with = "optional_positive")]
    y: Option<f32>,
}

/// `components.speed_limit`, checked: at least one cap.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "SpeedLimitFields")]
struct SpeedLimitDef {
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    x: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    y: Option<f32>,
}

impl TryFrom<SpeedLimitFields> for SpeedLimitDef {
    type Error = String;

    fn try_from(fields: SpeedLimitFields) -> Result<Self, Self::Error> {
        if fields.speed.is_none() && fields.x.is_none() && fields.y.is_none() {
            return Err("names no cap: one of `speed`, `x` and `y` at least".to_string());
        }
        Ok(Self {
            speed: fields.speed,
            x: fields.x,
            y: fields.y,
        })
    }
}

impl From<SpeedLimitDef> for SpeedLimit {
    fn from(def: SpeedLimitDef) -> Self {
        Self {
            speed: def.speed,
            x: def.x,
            y: def.y,
        }
    }
}

impl From<SpeedLimit> for SpeedLimitDef {
    fn from(limit: SpeedLimit) -> Self {
        Self {
            speed: limit.speed,
            x: limit.x,
            y: limit.y,
        }
    }
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's limited entities, kept so a step allocates nothing once
    /// warm.
    entities: Vec<Entity>,
    warned: Warned,
}

impl Runner {
    /// Caps every limited entity's speed before the step: its body's
    /// velocity, its movers' own and a bodiless entity's move.
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &SpeedLimit)>()
                .map(|(entity, _)| entity),
        );
        for &entity in &self.entities {
            let limit = *world.get::<SpeedLimit>(entity).unwrap();
            // The movers' own velocities, so their next step starts capped.
            if let Some(controller) = world.get_mut::<PlatformerController>(entity) {
                let velocity = controller.velocity_mut();
                *velocity = limit.cap(*velocity);
            }
            if let Some(motion) = world.get_mut::<LinearMotion>(entity) {
                let capped = limit.cap(motion.velocity());
                motion.set_velocity(capped);
            }
            if let Some(steering) = world.get_mut::<Steering>(entity) {
                let velocity = steering.velocity_mut();
                *velocity = limit.cap(*velocity);
            }
            match body_of(world, entity) {
                Body::Kinematic | Body::Dynamic => {
                    let velocity = world.get_mut::<Velocity>(entity).unwrap();
                    velocity.0 = limit.cap(velocity.0);
                }
                Body::Bodiless | Body::Static => {
                    // The step's move so far, from the snapshot.
                    if let Some(prev) = world.get::<PrevPosition>(entity).map(|prev| prev.0) {
                        let position = world.get_mut::<Position>(entity).unwrap();
                        position.0 = prev + limit.cap((position.0 - prev) / dt) * dt;
                    }
                    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
                        velocity.0 = limit.cap(velocity.0);
                    }
                }
                Body::Child => self.warned.skip("SpeedLimit", entity, Skip::Child),
                Body::Unplaced => self.warned.skip("SpeedLimit", entity, Skip::Unplaced),
            }
        }
    }
}

/// Caps every limited dynamic body's velocity after the solver, which adds
/// gravity and the contacts' impulses inside the step.
fn speed_limit_settle(world: &mut World) {
    for (limit, body, velocity) in world.query_mut::<(&SpeedLimit, &RigidBody, &mut Velocity)>() {
        if body.kind == BodyKind::Dynamic {
            velocity.0 = limit.cap(velocity.0);
        }
    }
}

impl Inspectable for SpeedLimit {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let cap = |cap: Option<f32>| cap.map_or_else(|| "none".to_string(), |c| format!("{c:.1}"));
        vec![
            ("speed", cap(self.speed)),
            ("x", cap(self.x)),
            ("y", cap(self.y)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/speed_limit.rs"]
mod tests;
