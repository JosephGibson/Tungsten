//! Steering: an entity that seeks, flees or arrives at a target, its
//! velocity turned toward the one it wants by a bounded acceleration.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::{Position, PrevPosition, Velocity};
use tungsten_core::{Entity, Inspectable, Plugin, Schedule, SpawnContext, World, system};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, SHMUP, TOP_DOWN, describe, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{optional_positive, positive, vec2};
use crate::motion::drive::{Body, Skip, Warned, Write, body_of, write_velocity};
use crate::motion::slide::step_dt;
use crate::motion::target::{Measure, Resolver, TARGET_SHAPE, Target, TargetDef, describe_target};

/// [`SteeringPlugin`]'s system, in the move phase.
pub const STEERING: &str = "steering";

/// The scene component the plugin registers.
const STEERING_COMPONENT: &str = "steering";

/// The default radius, in world pixels.
const DEFAULT_RADIUS: f32 = 64.0;

/// What a steering entity wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    /// Full speed at the target.
    #[default]
    Seek,
    /// Full speed away inside the radius, braking outside it.
    Flee,
    /// At the target, slowing inside the radius and stopping on it.
    Arrive,
}

/// Seeks, flees or arrives at a [`Target`]: an enemy that chases the hero,
/// a coward that runs from it, a mine that drifts onto the ship, a homing
/// bolt. Each step [`SteeringPlugin`]'s system turns its velocity toward the
/// one it wants by at most `acceleration · dt`, or at once without an
/// acceleration, and caps it at its top speed:
///
/// - `seek` wants full speed at the target;
/// - `flee` wants full speed away from it inside its radius, and brakes
///   outside it;
/// - `arrive` wants the least of the top speed, the top speed scaled by its
///   distance over the radius and, with an acceleration,
///   `√(2 · acceleration · distance)`, so it brakes in time, and it never
///   goes further in a step than the distance left, so it lands on the
///   target and stops however small the radius; on that last approach its
///   stop may exceed its acceleration, an emergency stop.
///
/// With no target it brakes. Its velocity is the body's `Velocity` when it
/// has one, so a dynamic body's contacts carry into its next step, else its
/// own. A `move_speed` stat sets its top speed, written by
/// [`StatsPlugin`](crate::StatsPlugin)'s `stats_apply`.
///
/// Without a `RigidBody` it moves its `Position` by its velocity, with no
/// cast, and writes a `Velocity` when it has one; on a kinematic body it
/// drives the body's `Velocity`, and on a dynamic body it steers the
/// body's `Velocity`, which the solver then moves and stops at walls. A
/// static body and a child are skipped, with one warning.
///
/// Scene name `steering`:
/// `{ "target": { "nearest": { "all": ["team.player"] } }, "mode": "seek", "max_speed": 90, "acceleration": 600, "radius": 64 }`,
/// `mode` one of `seek` (default), `flee` and `arrive`, `max_speed` in
/// world pixels per second, `acceleration` in world pixels per second
/// squared (at once when left out) and `radius` 64 by default, which spawns
/// a `Position` and a `PrevPosition` at the spawn point, so a prefab
/// without a `rigid_body` moves bodiless. It is data: a game's save can
/// hold it, without the entity it steered at, which the next step resolves
/// again.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition};
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Steering, SteeringPlugin, TagFilter, Tags, Target};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// SteeringPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A mine that drifts onto the nearest ship and stops on it, slowing in
/// // time for its acceleration and inside its last 8 px.
/// let ship = world.spawn_with((Tags::new(["team.player"]), Position::new(300.0, 0.0)));
/// let arrive = Steering::arrive(Target::nearest(TagFilter::new().all(["team.player"])), 90.0)
///     .with_acceleration(240.0)
///     .with_radius(8.0);
/// let mine = world.spawn_with((arrive, Position::new(0.0, 0.0), PrevPosition(Vec2::new(0.0, 0.0))));
/// for _ in 0..600 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// let steering = world.get::<Steering>(mine).unwrap();
/// let at = world.get::<Position>(mine).unwrap().0;
/// assert!((at - Vec2::new(300.0, 0.0)).length() < 1e-3, "on it: {at}");
/// assert!(steering.velocity().length() < 1e-2, "and stopped");
/// assert_eq!(steering.target_entity(), Some(ship));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SteeringFields", into = "SteeringFields")]
pub struct Steering {
    target: Target,
    mode: Mode,
    max_speed: f32,
    acceleration: Option<f32>,
    radius: f32,
    /// Its own velocity at the end of its last step.
    velocity: Vec2,
    /// The entity it steered at at its last step.
    target_entity: Option<Entity>,
}

/// A steering as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SteeringFields {
    target: Target,
    #[serde(default)]
    mode: Mode,
    max_speed: f32,
    #[serde(default)]
    acceleration: Option<f32>,
    #[serde(default = "default_radius")]
    radius: f32,
    #[serde(default, with = "vec2")]
    velocity: Vec2,
}

fn default_radius() -> f32 {
    DEFAULT_RADIUS
}

impl TryFrom<SteeringFields> for Steering {
    type Error = String;

    fn try_from(fields: SteeringFields) -> Result<Self, Self::Error> {
        for (field, value) in [
            ("acceleration", fields.acceleration),
            ("radius", Some(fields.radius)),
        ] {
            if let Some(value) = value
                && !(value.is_finite() && value > 0.0)
            {
                return Err(format!("{field}: must be finite and > 0 (got {value})"));
            }
        }
        // A `move_speed` stat of 0 stops it, so a save may hold 0.
        if !(fields.max_speed.is_finite() && fields.max_speed >= 0.0) {
            return Err(format!(
                "max_speed: must be finite and >= 0 (got {})",
                fields.max_speed
            ));
        }
        if !fields.velocity.is_finite() {
            return Err(format!(
                "velocity: must be finite (got {})",
                fields.velocity
            ));
        }
        Ok(Self {
            target: fields.target,
            mode: fields.mode,
            max_speed: fields.max_speed,
            acceleration: fields.acceleration,
            radius: fields.radius,
            velocity: fields.velocity,
            target_entity: None,
        })
    }
}

impl From<Steering> for SteeringFields {
    fn from(steering: Steering) -> Self {
        Self {
            target: steering.target,
            mode: steering.mode,
            max_speed: steering.max_speed,
            acceleration: steering.acceleration,
            radius: steering.radius,
            velocity: steering.velocity,
        }
    }
}

impl Steering {
    #[track_caller]
    fn new(target: Target, mode: Mode, max_speed: f32, method: &str) -> Self {
        assert!(
            max_speed.is_finite() && max_speed > 0.0,
            "Steering::{method}: max_speed must be finite and > 0 (got {max_speed})"
        );
        Self {
            target,
            mode,
            max_speed,
            acceleration: None,
            radius: DEFAULT_RADIUS,
            velocity: Vec2::ZERO,
            target_entity: None,
        }
    }

    /// Full speed at `target`, at most `max_speed` world pixels per second,
    /// turning at once until [`with_acceleration`](Self::with_acceleration).
    ///
    /// # Panics
    ///
    /// When `max_speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn seek(target: Target, max_speed: f32) -> Self {
        Self::new(target, Mode::Seek, max_speed, "seek")
    }

    /// Full speed away from `target` inside the radius (64 world pixels
    /// until [`with_radius`](Self::with_radius)), braking outside it.
    ///
    /// # Panics
    ///
    /// When `max_speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn flee(target: Target, max_speed: f32) -> Self {
        Self::new(target, Mode::Flee, max_speed, "flee")
    }

    /// At `target`, slowing inside the radius (64 world pixels until
    /// [`with_radius`](Self::with_radius)) and stopping on it.
    ///
    /// # Panics
    ///
    /// When `max_speed` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn arrive(target: Target, max_speed: f32) -> Self {
        Self::new(target, Mode::Arrive, max_speed, "arrive")
    }

    /// The steering, turning its velocity by at most `acceleration` world
    /// pixels per second squared.
    ///
    /// # Panics
    ///
    /// When `acceleration` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_acceleration(mut self, acceleration: f32) -> Self {
        assert!(
            acceleration.is_finite() && acceleration > 0.0,
            "Steering::with_acceleration: acceleration must be finite and > 0 (got {acceleration})"
        );
        self.acceleration = Some(acceleration);
        self
    }

    /// The steering, with `radius` world pixels as the distance `arrive`
    /// slows inside and `flee` flees inside.
    ///
    /// # Panics
    ///
    /// When `radius` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_radius(mut self, radius: f32) -> Self {
        assert!(
            radius.is_finite() && radius > 0.0,
            "Steering::with_radius: radius must be finite and > 0 (got {radius})"
        );
        self.radius = radius;
        self
    }

    /// Its velocity at the end of its last step.
    #[must_use]
    pub fn velocity(&self) -> Vec2 {
        self.velocity
    }

    /// The entity it steered at at its last step: `None` before its first
    /// step, while a point is its target, or while no target resolves.
    #[must_use]
    pub fn target_entity(&self) -> Option<Entity> {
        self.target_entity
    }

    /// Sets the top speed, 0 standing it still: what a `move_speed` stat
    /// writes. One that is not finite and at least 0 is ignored.
    pub(crate) fn set_max_speed(&mut self, speed: f32) {
        if speed.is_finite() && speed >= 0.0 {
            self.max_speed = speed;
        }
    }

    /// Its own velocity, which a speed limit caps.
    pub(crate) fn velocity_mut(&mut self) -> &mut Vec2 {
        &mut self.velocity
    }

    /// The velocity it wants toward a target `to` away, or with none.
    fn desired(&self, to: Option<Vec2>) -> Vec2 {
        let Some(to) = to else {
            return Vec2::ZERO;
        };
        let distance = to.length();
        let direction = to.normalize_or_zero();
        match self.mode {
            Mode::Seek => direction * self.max_speed,
            Mode::Flee if distance < self.radius => -direction * self.max_speed,
            Mode::Flee => Vec2::ZERO,
            Mode::Arrive => {
                let mut speed = self.max_speed.min(self.max_speed * distance / self.radius);
                if let Some(acceleration) = self.acceleration {
                    speed = speed.min((2.0 * acceleration * distance).sqrt());
                }
                direction * speed
            }
        }
    }

    /// Its velocity after a step of `dt` from `current`, toward a target
    /// `to` away: turned by at most its acceleration, capped at its top
    /// speed, and, arriving, never further than the distance left.
    fn steer(&self, current: Vec2, to: Option<Vec2>, dt: f32) -> Vec2 {
        let desired = self.desired(to);
        let turned = match self.acceleration {
            Some(acceleration) => current + (desired - current).clamp_length_max(acceleration * dt),
            None => desired,
        };
        let velocity = turned.clamp_length_max(self.max_speed);
        match to {
            // The last approach: land on the target, braking harder than
            // the acceleration if it must.
            Some(to) if self.mode == Mode::Arrive && velocity.length() * dt > to.length() => {
                to / dt
            }
            _ => velocity,
        }
    }
}

/// Registers the `steering` scene component and [`STEERING`] in the move
/// phase. Its system steers every [`Steering`] by the step, `Time::delta`
/// bounded by `PhysicsConfig::max_step_dt` as the physics step takes it; a
/// step of 0 moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct SteeringPlugin;

impl Plugin for SteeringPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Steering>(world, "Steering");
        describe_target(world);
        describe(
            world,
            ComponentDescription::new(
                STEERING_COMPONENT,
                "Seeks, flees or arrives at a target, its velocity turned toward the one it wants by a bounded acceleration.",
            )
            .field(FieldDescription::new(
                "target",
                FieldType::Shape(TARGET_SHAPE),
                "what it steers at; the load checks its tags",
            ))
            .field(
                FieldDescription::new(
                    "mode",
                    FieldType::Enum(&["seek", "flee", "arrive"]),
                    "full speed at it, away from it inside the radius, or onto it and stopping",
                )
                .default("\"seek\""),
            )
            .field(FieldDescription::new(
                "max_speed",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels per second at most; a `move_speed` stat sets it",
            ))
            .field(
                FieldDescription::new(
                    "acceleration",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels per second squared it turns by; at once when left out",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "radius",
                    FieldType::F32 {
                        min: Bound::Exclusive(0.0),
                        max: Bound::Open,
                    },
                    "world pixels: `arrive` slows inside it, `flee` flees inside it",
                )
                .default("64.0"),
            )
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .sample(r#"{ "target": { "nearest": { "all": ["team.player"] } }, "max_speed": 90 }"#)
            .roundtrip(round_trip::<SteeringDef>),
        );
        world.register_component_with_assets(
            STEERING_COMPONENT,
            |def: &SteeringDef, world: &World| def.target.check(world, "target"),
            |def: SteeringDef, at: &SpawnContext<'_>| {
                let spawn = at.transform().position;
                let mut steering = Steering::new(def.target.into(), def.mode, def.max_speed, "new")
                    .with_radius(def.radius);
                if let Some(acceleration) = def.acceleration {
                    steering = steering.with_acceleration(acceleration);
                }
                (steering, Position(spawn), PrevPosition(spawn))
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move.join(system(STEERING, move |world: &mut World| runner.run(world))),
        );
    }
}

/// `components.steering` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SteeringDef {
    target: TargetDef,
    #[serde(default)]
    mode: Mode,
    #[serde(deserialize_with = "positive")]
    max_speed: f32,
    #[serde(default, deserialize_with = "optional_positive")]
    acceleration: Option<f32>,
    #[serde(default = "default_radius", deserialize_with = "positive")]
    radius: f32,
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's steering entities, kept so a step allocates nothing once
    /// warm.
    entities: Vec<Entity>,
    resolver: Resolver,
    warned: Warned,
}

impl Runner {
    /// Steers every steering entity by the step.
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.resolver.begin();
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &Steering)>()
                .map(|(entity, _)| entity),
        );
        for &entity in &self.entities {
            let body = body_of(world, entity);
            let skip = match body {
                Body::Child => Some(Skip::Child),
                Body::Static => Some(Skip::Static),
                Body::Unplaced => Some(Skip::Unplaced),
                Body::Bodiless | Body::Kinematic | Body::Dynamic => None,
            };
            if let Some(skip) = skip {
                self.warned.skip("Steering", entity, skip);
                continue;
            }
            let from = world.get::<Position>(entity).unwrap().0;
            let steering = world.get::<Steering>(entity).unwrap();
            let resolved =
                self.resolver
                    .resolve(world, &steering.target, entity, from, Measure::Now);
            // A body's own velocity, so a dynamic body's contacts carry over.
            let current = match body {
                Body::Kinematic | Body::Dynamic => world.get::<Velocity>(entity).unwrap().0,
                _ => steering.velocity,
            };
            let velocity = steering.steer(current, resolved.map(|(_, at)| at - from), dt);
            if body == Body::Bodiless {
                world.get_mut::<Position>(entity).unwrap().0 += velocity * dt;
            }
            write_velocity(world, entity, velocity, Write::Set);
            let steering = world.get_mut::<Steering>(entity).unwrap();
            steering.velocity = velocity;
            steering.target_entity = resolved.and_then(|(target, _)| target);
        }
    }
}

impl Inspectable for Steering {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mode = match self.mode {
            Mode::Seek => "seek",
            Mode::Flee => "flee",
            Mode::Arrive => "arrive",
        };
        let at = self
            .target_entity
            .map_or_else(|| "none".to_string(), |entity| format!("{entity:?}"));
        vec![
            (
                "mode",
                format!("{mode} at up to {:.1} px/s", self.max_speed),
            ),
            ("target", format!("{}: {at}", self.target.summary())),
            ("velocity", point(self.velocity)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/steering.rs"]
mod tests;
