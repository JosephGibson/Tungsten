//! Orbit: an entity placed on a circle about a centre, turning at a speed.

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
use crate::foundation::{finite, finite_vec2, non_negative, optional_vec2, vec2};
use crate::motion::drive::{Body, Skip, Warned, Write, body_of, step_position, write_velocity};
use crate::motion::linear::LINEAR_MOTION;
use crate::motion::oscillator::{OSCILLATOR, Oscillator};
use crate::motion::path::PATH_FOLLOW;
use crate::motion::platformer::PLATFORMER_CONTROLLER;
use crate::motion::slide::step_dt;
use crate::motion::steering::STEERING;
use crate::motion::target::{Measure, Resolver, TARGET_SHAPE, Target, TargetDef, describe_target};
use crate::motion::top_down::TOP_DOWN_MOVER;

/// [`OrbitPlugin`]'s system, in the move phase.
pub const ORBIT: &str = "orbit";

/// The scene component the plugin registers.
const ORBIT_COMPONENT: &str = "orbit";

/// Places its entity on a circle about a centre, turning at a speed: the
/// blades about a wielder, a firebar's ball about its pivot, a moon. Each
/// step [`OrbitPlugin`]'s system turns its angle by `speed · dt` on its own
/// fixed-step clock and places its point at
/// `centre + offset + radius · (cos θ, sin θ)`, so it circles the same way
/// at any frame rate. A positive speed turns clockwise on screen, since y
/// points down, and an angle of 0 points right.
///
/// Its centre is a [`Target`]'s point, taken where the target's movers
/// leave it this step, as a [`Follow`](crate::Follow)'s goal is; with no
/// target, the point it was at on its first step, so a prefab spawned at
/// its pivot circles it. When its target goes it keeps circling the last
/// centre. With an [`Oscillator`](crate::Oscillator) on the orbiter, its
/// offset joins the point.
///
/// Without a `RigidBody` it sets its `Position`, and a `Velocity` when it
/// has one; on a kinematic body it drives the body, its `Velocity` reaching
/// the point at the step's end. A platformer controller standing on an
/// orbiting platform reads that velocity a step late, since the orbit runs
/// after the controllers so that it stays on a moving centre. A dynamic
/// body keeps its own velocity and a child its parent's offset: each is
/// skipped, with one warning.
///
/// Scene name `orbit`:
/// `{ "center": { "source": {} }, "radius": 48, "speed": 3, "angle": 0, "offset": [0, 0] }`,
/// `center` its first step's point by default, `radius` in world pixels,
/// `speed` in radians per second, `angle` its start in radians, 0 by
/// default, and `offset` `[0, 0]`, which spawns a `Position` and a
/// `PrevPosition` at the spawn point, so a prefab without a `rigid_body`
/// moves bodiless. It is data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use std::f32::consts::{FRAC_PI_2, PI};
///
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition};
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Orbit, OrbitPlugin, Source, Target};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// OrbitPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A blade 48 px from the wielder that spawned it, half a turn a second,
/// // starting below it.
/// let wielder = world.spawn_with((Position::new(200.0, 100.0),));
/// let orbit = Orbit::new(48.0, PI).around(Target::source()).with_angle(FRAC_PI_2);
/// let blade = world.spawn_with((
///     orbit,
///     Source::new(wielder),
///     Position::new(200.0, 100.0),
///     PrevPosition(Vec2::new(200.0, 100.0)),
/// ));
/// for _ in 0..60 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// }
/// // A second later it has turned half a circle: above the wielder.
/// let at = world.get::<Position>(blade).unwrap().0;
/// assert!((at - Vec2::new(200.0, 52.0)).length() < 1e-3, "{at}");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OrbitFields", into = "OrbitFields")]
pub struct Orbit {
    center: Option<Target>,
    radius: f32,
    speed: f32,
    /// The angle at its last step, in `[0, 2π)`.
    angle: f32,
    offset: Vec2,
    /// The centre at its last step.
    centre_at: Option<Vec2>,
}

/// An orbit as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrbitFields {
    #[serde(default)]
    center: Option<Target>,
    radius: f32,
    speed: f32,
    #[serde(default)]
    angle: f32,
    #[serde(default, with = "vec2")]
    offset: Vec2,
    #[serde(default, with = "optional_vec2")]
    centre_at: Option<Vec2>,
}

impl TryFrom<OrbitFields> for Orbit {
    type Error = String;

    fn try_from(fields: OrbitFields) -> Result<Self, Self::Error> {
        if !(fields.radius.is_finite() && fields.radius >= 0.0) {
            return Err(format!(
                "radius: must be finite and >= 0 (got {})",
                fields.radius
            ));
        }
        for (field, value) in [("speed", fields.speed), ("angle", fields.angle)] {
            if !value.is_finite() {
                return Err(format!("{field}: must be finite (got {value})"));
            }
        }
        if !fields.offset.is_finite() {
            return Err(format!("offset: must be finite (got {})", fields.offset));
        }
        if let Some(centre) = fields.centre_at
            && !centre.is_finite()
        {
            return Err(format!("centre_at: must be finite (got {centre})"));
        }
        Ok(Self {
            center: fields.center,
            radius: fields.radius,
            speed: fields.speed,
            angle: fields.angle.rem_euclid(std::f32::consts::TAU),
            offset: fields.offset,
            centre_at: fields.centre_at,
        })
    }
}

impl From<Orbit> for OrbitFields {
    fn from(orbit: Orbit) -> Self {
        Self {
            center: orbit.center,
            radius: orbit.radius,
            speed: orbit.speed,
            angle: orbit.angle,
            offset: orbit.offset,
            centre_at: orbit.centre_at,
        }
    }
}

impl Orbit {
    /// An orbit `radius` world pixels about the point it is at on its first
    /// step, turning `speed` radians per second, clockwise on screen when
    /// positive, from an angle of 0 (pointing right).
    ///
    /// # Panics
    ///
    /// When `radius` is not finite and at least 0, or `speed` is not finite.
    #[must_use]
    #[track_caller]
    pub fn new(radius: f32, speed: f32) -> Self {
        assert!(
            radius.is_finite() && radius >= 0.0,
            "Orbit::new: radius must be finite and >= 0 (got {radius})"
        );
        assert!(
            speed.is_finite(),
            "Orbit::new: speed must be finite (got {speed})"
        );
        Self {
            center: None,
            radius,
            speed,
            angle: 0.0,
            offset: Vec2::ZERO,
            centre_at: None,
        }
    }

    /// The orbit, about `center`'s point.
    #[must_use]
    pub fn around(mut self, center: Target) -> Self {
        self.center = Some(center);
        self
    }

    /// The orbit, starting at `angle` radians.
    ///
    /// # Panics
    ///
    /// When `angle` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_angle(mut self, angle: f32) -> Self {
        assert!(
            angle.is_finite(),
            "Orbit::with_angle: angle must be finite (got {angle})"
        );
        self.angle = angle.rem_euclid(std::f32::consts::TAU);
        self
    }

    /// The orbit, its circle's centre `offset` world pixels from the
    /// centre's point.
    ///
    /// # Panics
    ///
    /// When `offset` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        assert!(
            offset.is_finite(),
            "Orbit::with_offset: offset must be finite (got {offset})"
        );
        self.offset = offset;
        self
    }

    /// Its angle at its last step, in radians in `[0, 2π)`: 0 points right
    /// and `π/2` down.
    #[must_use]
    pub fn angle(&self) -> f32 {
        self.angle
    }

    /// Its circle at its last step, centre and radius, for the debug
    /// drawing: `None` before it has a centre.
    pub(crate) fn circle(&self) -> Option<(Vec2, f32)> {
        self.centre_at
            .map(|centre| (centre + self.offset, self.radius))
    }
}

/// Registers the `orbit` scene component and [`ORBIT`] in the move phase,
/// after the path follower, the linear motion, the oscillator, both
/// controllers and the steering, each when registered, so an orbiter
/// stays on a centre those movers move. Its system turns every [`Orbit`] by
/// the step, `Time::delta` bounded by `PhysicsConfig::max_step_dt` as the
/// physics step takes it; a step of 0 moves nothing.
#[derive(Debug, Default, Clone, Copy)]
pub struct OrbitPlugin;

impl Plugin for OrbitPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Orbit>(world, "Orbit");
        describe_target(world);
        describe(
            world,
            ComponentDescription::new(
                ORBIT_COMPONENT,
                "Places the entity on a circle about a centre, turning at a speed: blades about a wielder, a firebar's ball.",
            )
            .field(
                FieldDescription::new(
                    "center",
                    FieldType::Shape(TARGET_SHAPE),
                    "the circle's centre; the point it is at on its first step when left out",
                )
                .default("null"),
            )
            .field(FieldDescription::new(
                "radius",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "world pixels from the centre",
            ))
            .field(FieldDescription::new(
                "speed",
                FieldType::F32 {
                    min: Bound::Open,
                    max: Bound::Open,
                },
                "radians per second, clockwise on screen when positive",
            ))
            .field(
                FieldDescription::new(
                    "angle",
                    FieldType::F32 {
                        min: Bound::Open,
                        max: Bound::Open,
                    },
                    "the angle it starts at, in radians: 0 points right, π/2 down",
                )
                .default("0.0"),
            )
            .field(
                FieldDescription::new(
                    "offset",
                    FieldType::Vec2,
                    "world pixels from the centre's point to the circle's centre",
                )
                .default("[0.0, 0.0]"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .sample(r#"{ "radius": 48, "speed": 3 }"#)
            .roundtrip(round_trip::<OrbitDef>),
        );
        world.register_component_with_assets(
            ORBIT_COMPONENT,
            |def: &OrbitDef, world: &World| {
                def.center
                    .as_ref()
                    .map_or(Ok(()), |center| center.check(world, "center"))
            },
            |def: OrbitDef, at: &SpawnContext<'_>| {
                let spawn = at.transform().position;
                let mut orbit = Orbit::new(def.radius, def.speed)
                    .with_angle(def.angle)
                    .with_offset(def.offset);
                if let Some(center) = def.center {
                    orbit = orbit.around(center.into());
                }
                (orbit, Position(spawn), PrevPosition(spawn))
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(ORBIT, move |world: &mut World| runner.run(world)))
                .after_if_present(PATH_FOLLOW)
                .after_if_present(LINEAR_MOTION)
                .after_if_present(OSCILLATOR)
                .after_if_present(PLATFORMER_CONTROLLER)
                .after_if_present(TOP_DOWN_MOVER)
                .after_if_present(STEERING),
        );
    }
}

/// `components.orbit` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OrbitDef {
    #[serde(default)]
    center: Option<TargetDef>,
    #[serde(deserialize_with = "non_negative")]
    radius: f32,
    #[serde(deserialize_with = "finite")]
    speed: f32,
    #[serde(default, deserialize_with = "finite")]
    angle: f32,
    #[serde(default, with = "finite_vec2")]
    offset: Vec2,
}

/// The system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's orbits, kept so a step allocates nothing once warm.
    entities: Vec<Entity>,
    resolver: Resolver,
    warned: Warned,
}

impl Runner {
    /// Turns every orbit by the step and places it on its circle.
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.resolver.begin();
        self.entities.clear();
        self.entities
            .extend(world.query::<(Entity, &Orbit)>().map(|(entity, _)| entity));
        for &entity in &self.entities {
            let body = body_of(world, entity);
            let skip = match body {
                Body::Child => Some(Skip::Child),
                Body::Dynamic => Some(Skip::Dynamic),
                Body::Unplaced => Some(Skip::Unplaced),
                Body::Bodiless | Body::Static | Body::Kinematic => None,
            };
            if let Some(skip) = skip {
                self.warned.skip("Orbit", entity, skip);
                continue;
            }
            let from = world.get::<Position>(entity).unwrap().0;
            let orbit = world.get::<Orbit>(entity).unwrap();
            let centre = match &orbit.center {
                Some(target) => self
                    .resolver
                    .resolve(world, target, entity, from, Measure::Placed(dt))
                    .map(|(_, at)| at)
                    .or(orbit.centre_at),
                None => Some(orbit.centre_at.unwrap_or(from)),
            };
            let angle = (orbit.angle + orbit.speed * dt).rem_euclid(std::f32::consts::TAU);
            let wave = world
                .get::<Oscillator>(entity)
                .map_or(Vec2::ZERO, Oscillator::displacement);
            let goal = centre.map(|centre| {
                centre + orbit.offset + orbit.radius * Vec2::from_angle(angle) + wave
            });
            let step = goal.map_or(Vec2::ZERO, |goal| goal - from);
            if body == Body::Kinematic {
                write_velocity(world, entity, step / dt, Write::Set);
            } else {
                step_position(world, entity, step, dt, Write::Set);
            }
            let orbit = world.get_mut::<Orbit>(entity).unwrap();
            orbit.angle = angle;
            if centre.is_some() {
                orbit.centre_at = centre;
            }
        }
    }
}

impl Inspectable for Orbit {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let centre = match (&self.center, self.centre_at) {
            (Some(target), Some(at)) => format!("{} at {}", target.summary(), point(at)),
            (Some(target), None) => target.summary(),
            (None, Some(at)) => format!("first point {}", point(at)),
            (None, None) => "first point".to_string(),
        };
        vec![
            ("centre", centre),
            ("radius", format!("{:.1}", self.radius)),
            (
                "angle",
                format!("{:.2} rad at {:.2} rad/s", self.angle, self.speed),
            ),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/orbit.rs"]
mod tests;
