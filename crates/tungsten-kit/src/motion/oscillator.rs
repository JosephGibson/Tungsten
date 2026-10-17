//! The oscillator: a sine wave on an entity's position, rotation or scale,
//! on the fixed step's clock.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::ecs::bundle::BundleSink;
use tungsten_core::physics::{Position, PrevPosition};
use tungsten_core::{
    Bundle, Entity, Inspectable, Plugin, Schedule, SpawnContext, Time, Transform, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, TOP_DOWN, describe,
    describe_shape, round_trip,
};
use crate::foundation::inspect::{point, register_rows};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{below_one, finite, finite_vec2, positive};
use crate::motion::drive::{
    Body, Driver, Skip, Warned, Write, body_of, driver_of, step_position, write_velocity,
};
use crate::motion::linear::LINEAR_MOTION;
use crate::motion::platformer::PLATFORMER_CONTROLLER;
use crate::motion::slide::step_dt;
use crate::motion::top_down::TOP_DOWN_MOVER;

/// [`OscillatorPlugin`]'s fixed-step system, in the move phase: the clock
/// and the position channel.
pub const OSCILLATOR: &str = "oscillator";

/// [`OscillatorPlugin`]'s frame system, in the present phase: the rotation
/// and scale channels.
pub const OSCILLATOR_PRESENT: &str = "oscillator_present";

/// The scene component the plugin registers.
const OSCILLATOR_COMPONENT: &str = "oscillator";

/// The shape of the position channel.
const POSITION_WAVE: &str = "position_wave";

/// The shape of the rotation and scale channels.
const WAVE: &str = "wave";

/// A sine wave on its entity's position, rotation or scale: a bobbing
/// pickup, a patrolling hazard, a pulsing gem, a swaying sign. Each channel
/// is `amplitude · sin(2π · (t / period + phase))` on the oscillator's own
/// clock, which [`OscillatorPlugin`]'s fixed-step system advances, so a
/// pause holds it and the wave is the same at any frame rate.
///
/// The position channel moves the entity by the wave's change each step:
/// an offset that starts at 0 wherever the phase starts the swing, so the
/// entity starts where it was spawned and comes back there each period,
/// and it adds to whatever else moves the entity in the step. Beside a
/// [`PathFollower`](crate::PathFollower), a [`Follow`](crate::Follow) or an
/// [`Orbit`](crate::Orbit), which bring the entity back onto their own goal
/// each step, it adds nothing itself: they fold its offset into their goal.
/// A bodiless or static entity moves its `Position`; on a kinematic body
/// with no other driver it drives the body's `Velocity`, as the path
/// follower does, and beside one that runs before it (a path follower or a
/// [`LinearMotion`](crate::LinearMotion)) it adds to that velocity. Beside a
/// controller or a [`Steering`](crate::Steering), which run after it and
/// set the body's velocity, it leaves the body, with one warning, as it
/// does a dynamic body and a child.
///
/// The rotation and scale channels write `Transform.rotation` and
/// `Transform.scale` once a frame, swinging about the values the entity
/// had at its first frame, at the clock drawn between its last two steps as
/// the physics sync draws positions. They work on children too. A tween or
/// a squash on the same channel is hidden: the oscillator writes after
/// them.
///
/// Scene name `oscillator`:
/// `{ "position": { "amplitude": [0, 3], "period": 1.2 }, "rotation": { "amplitude": 0.3, "period": 0.8 }, "scale": { "amplitude": 0.1, "period": 0.5 }, "phase": 0 }`,
/// at least one channel, each `period` in seconds above 0, the position's
/// amplitude in world pixels, the rotation's in radians, the scale's a
/// fraction in `[0, 1)` of the base scale, and `phase` in `[0, 1)` of a
/// period, shared by the channels. A position channel spawns a `Position`
/// and a `PrevPosition` at the spawn point, so a prefab without a
/// `rigid_body` moves bodiless. It is data: a game's save can hold it.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::physics::{Position, PrevPosition};
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Oscillator, OscillatorPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// OscillatorPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A gem that bobs 3 px over 1.2 s and pulses 10% over half a second.
/// let bob = Oscillator::position(Vec2::new(0.0, 3.0), 1.2).with_scale(0.1, 0.5);
/// let gem = world.spawn_with((bob, Position::new(50.0, 80.0), PrevPosition(Vec2::new(50.0, 80.0))));
/// let mut lowest = 80.0_f32;
/// for _ in 0..72 {
///     world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     lowest = lowest.max(world.get::<Position>(gem).unwrap().0.y);
/// }
/// // A quarter period in, 3 px down; a whole one, back where it started.
/// assert!((lowest - 83.0).abs() < 1e-3);
/// assert!((world.get::<Position>(gem).unwrap().0 - Vec2::new(50.0, 80.0)).length() < 1e-3);
///
/// let saved = serde_json::to_string(world.get::<Oscillator>(gem).unwrap()).unwrap();
/// assert_eq!(&serde_json::from_str::<Oscillator>(&saved).unwrap(), world.get::<Oscillator>(gem).unwrap());
/// // A scale amplitude of 1 would shrink the sprite to nothing.
/// assert!(std::panic::catch_unwind(|| Oscillator::scale(1.0, 0.5)).is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "OscillatorFields", into = "OscillatorFields")]
pub struct Oscillator {
    position: Option<PositionWave>,
    rotation: Option<Wave>,
    scale: Option<Wave>,
    phase: f32,
    /// Seconds on its clock at its last step.
    clock: f64,
    /// Seconds on its clock at the step before.
    prev_clock: f64,
    /// The rotation and scale the entity had at its first frame.
    base: Option<Base>,
}

/// The position channel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PositionWave {
    #[serde(with = "finite_vec2")]
    amplitude: Vec2,
    #[serde(deserialize_with = "positive")]
    period: f32,
}

/// A rotation or scale channel.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wave {
    #[serde(deserialize_with = "finite")]
    amplitude: f32,
    #[serde(deserialize_with = "positive")]
    period: f32,
}

/// What the rotation and scale channels swing about.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Base {
    #[serde(deserialize_with = "finite")]
    rotation: f32,
    #[serde(with = "finite_vec2")]
    scale: Vec2,
}

/// An oscillator as a save writes it, before its values are checked.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OscillatorFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    position: Option<PositionWave>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rotation: Option<Wave>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scale: Option<Wave>,
    #[serde(default)]
    phase: f32,
    #[serde(default)]
    clock: f64,
    #[serde(default)]
    prev_clock: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base: Option<Base>,
}

impl TryFrom<OscillatorFields> for Oscillator {
    type Error = String;

    fn try_from(fields: OscillatorFields) -> Result<Self, Self::Error> {
        check_channels(fields.position, fields.rotation, fields.scale, fields.phase)?;
        for (field, value) in [("clock", fields.clock), ("prev_clock", fields.prev_clock)] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            position: fields.position,
            rotation: fields.rotation,
            scale: fields.scale,
            phase: fields.phase,
            clock: fields.clock,
            prev_clock: fields.prev_clock,
            base: fields.base,
        })
    }
}

impl From<Oscillator> for OscillatorFields {
    fn from(oscillator: Oscillator) -> Self {
        Self {
            position: oscillator.position,
            rotation: oscillator.rotation,
            scale: oscillator.scale,
            phase: oscillator.phase,
            clock: oscillator.clock,
            prev_clock: oscillator.prev_clock,
            base: oscillator.base,
        }
    }
}

/// At least one channel, a scale amplitude in `[0, 1)` and a phase in
/// `[0, 1)`; each wave checked its own fields as it deserialized.
fn check_channels(
    position: Option<PositionWave>,
    rotation: Option<Wave>,
    scale: Option<Wave>,
    phase: f32,
) -> Result<(), String> {
    if position.is_none() && rotation.is_none() && scale.is_none() {
        return Err("names no channel: one of `position`, `rotation` and `scale` at least".into());
    }
    if let Some(scale) = scale
        && !(0.0..1.0).contains(&scale.amplitude)
    {
        return Err(format!(
            "scale.amplitude: must be in [0, 1) (got {})",
            scale.amplitude
        ));
    }
    if !(0.0..1.0).contains(&phase) {
        return Err(format!("phase: must be in [0, 1) (got {phase})"));
    }
    Ok(())
}

impl Oscillator {
    /// An oscillator with no channel yet.
    fn empty() -> Self {
        Self {
            position: None,
            rotation: None,
            scale: None,
            phase: 0.0,
            clock: 0.0,
            prev_clock: 0.0,
            base: None,
        }
    }

    /// An oscillator of the position alone, `amplitude` world pixels at its
    /// peak, a swing every `period` seconds.
    ///
    /// # Panics
    ///
    /// As [`with_position`](Self::with_position).
    #[must_use]
    #[track_caller]
    pub fn position(amplitude: Vec2, period: f32) -> Self {
        Self::empty().with_position(amplitude, period)
    }

    /// An oscillator of the rotation alone, `amplitude` radians at its
    /// peak, a swing every `period` seconds.
    ///
    /// # Panics
    ///
    /// As [`with_rotation`](Self::with_rotation).
    #[must_use]
    #[track_caller]
    pub fn rotation(amplitude: f32, period: f32) -> Self {
        Self::empty().with_rotation(amplitude, period)
    }

    /// An oscillator of the scale alone, `amplitude` a fraction of the base
    /// scale at its peak, a swing every `period` seconds.
    ///
    /// # Panics
    ///
    /// As [`with_scale`](Self::with_scale).
    #[must_use]
    #[track_caller]
    pub fn scale(amplitude: f32, period: f32) -> Self {
        Self::empty().with_scale(amplitude, period)
    }

    /// The oscillator, its position swinging `amplitude` world pixels at its
    /// peak every `period` seconds.
    ///
    /// # Panics
    ///
    /// When `amplitude` is not finite, or `period` not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_position(mut self, amplitude: Vec2, period: f32) -> Self {
        assert!(
            amplitude.is_finite(),
            "Oscillator::with_position: amplitude must be finite (got {amplitude})"
        );
        assert_period("with_position", period);
        self.position = Some(PositionWave { amplitude, period });
        self
    }

    /// The oscillator, its rotation swinging `amplitude` radians at its
    /// peak every `period` seconds.
    ///
    /// # Panics
    ///
    /// When `amplitude` is not finite, or `period` not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_rotation(mut self, amplitude: f32, period: f32) -> Self {
        assert!(
            amplitude.is_finite(),
            "Oscillator::with_rotation: amplitude must be finite (got {amplitude})"
        );
        assert_period("with_rotation", period);
        self.rotation = Some(Wave { amplitude, period });
        self
    }

    /// The oscillator, its scale swinging `amplitude`, a fraction of the
    /// base scale, at its peak every `period` seconds.
    ///
    /// # Panics
    ///
    /// When `amplitude` is outside `[0, 1)`, or `period` not finite and above
    /// 0.
    #[must_use]
    #[track_caller]
    pub fn with_scale(mut self, amplitude: f32, period: f32) -> Self {
        assert!(
            (0.0..1.0).contains(&amplitude),
            "Oscillator::with_scale: amplitude must be in [0, 1) (got {amplitude})"
        );
        assert_period("with_scale", period);
        self.scale = Some(Wave { amplitude, period });
        self
    }

    /// The oscillator, every channel starting `phase` of a period into its
    /// swing.
    ///
    /// # Panics
    ///
    /// When `phase` is outside `[0, 1)`.
    #[must_use]
    #[track_caller]
    pub fn with_phase(mut self, phase: f32) -> Self {
        assert!(
            (0.0..1.0).contains(&phase),
            "Oscillator::with_phase: phase must be in [0, 1) (got {phase})"
        );
        self.phase = phase;
        self
    }
}

impl Oscillator {
    /// Whether it has a position channel, which moves its entity.
    pub(crate) fn moves(&self) -> bool {
        self.position.is_some()
    }

    /// The position channel's offset from where it started, at its last
    /// step: what a path follower, a follower or an orbit folds into its
    /// goal, so their goal carries the whole wave.
    pub(crate) fn displacement(&self) -> Vec2 {
        self.displacement_at(self.clock)
    }

    /// A rotation or scale channel's value at clock `t`:
    /// `amplitude · sin(2π(t / period + phase))`.
    fn wave_at(&self, wave: Wave, t: f64) -> f32 {
        let turn = t / f64::from(wave.period) + f64::from(self.phase);
        wave.amplitude * (std::f64::consts::TAU * turn).sin() as f32
    }

    /// The position channel's offset from where it started at clock `t`:
    /// `amplitude · (sin(2π(t / period + phase)) − sin(2π · phase))`.
    fn displacement_at(&self, t: f64) -> Vec2 {
        let Some(wave) = self.position else {
            return Vec2::ZERO;
        };
        let phase = f64::from(self.phase);
        let now = (std::f64::consts::TAU * (t / f64::from(wave.period) + phase)).sin();
        let start = (std::f64::consts::TAU * phase).sin();
        wave.amplitude * (now - start) as f32
    }
}

#[track_caller]
fn assert_period(method: &str, period: f32) {
    assert!(
        period.is_finite() && period > 0.0,
        "Oscillator::{method}: period must be finite and > 0 (got {period})"
    );
}

/// The scene component's bundle: the oscillator, and a `Position` and a
/// `PrevPosition` at the spawn point when it has a position channel.
struct OscillatorSpawn {
    oscillator: Oscillator,
    at: Vec2,
}

impl Bundle for OscillatorSpawn {
    fn put<S: BundleSink>(self, sink: &mut S) {
        let moves = self.oscillator.position.is_some();
        sink.put(self.oscillator);
        if moves {
            sink.put(Position(self.at));
            sink.put(PrevPosition(self.at));
        }
    }
}

/// Registers the `oscillator` scene component, [`OSCILLATOR`] in the move
/// phase, after [`LINEAR_MOTION`](crate::LINEAR_MOTION) and before both
/// controllers when they are registered, so it adds to a linear motion's
/// velocity and a rider reads a bobbing platform's, and
/// [`OSCILLATOR_PRESENT`] in the present phase. The fixed-step system
/// advances every [`Oscillator`]'s clock by the step and moves its
/// position channel; the frame system writes its rotation and scale.
#[derive(Debug, Default, Clone, Copy)]
pub struct OscillatorPlugin;

impl Plugin for OscillatorPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Oscillator>(world, "Oscillator");
        let period = || FieldType::F32 {
            min: Bound::Exclusive(0.0),
            max: Bound::Open,
        };
        describe(
            world,
            ComponentDescription::new(
                OSCILLATOR_COMPONENT,
                "A sine wave on the entity's position, rotation or scale, on the fixed step's clock: a bobbing pickup, a patrolling hazard, a pulsing gem.",
            )
            .field(
                FieldDescription::new(
                    "position",
                    FieldType::Shape(POSITION_WAVE),
                    "an offset from where it starts, added to its other motion",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "rotation",
                    FieldType::Shape(WAVE),
                    "radians about the rotation it had at its first frame",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "scale",
                    FieldType::Shape(WAVE),
                    "a fraction of the scale it had at its first frame, its amplitude in [0, 1)",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "phase",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Exclusive(1.0),
                    },
                    "how far into its swing every channel starts, in periods",
                )
                .default("0.0"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .sample(r#"{ "position": { "amplitude": [0, 3], "period": 1.2 } }"#)
            .roundtrip(round_trip::<OscillatorDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                POSITION_WAVE,
                "A position channel: amplitude · sin(2π(t / period + phase)), less its value at the start.",
            )
            .field(FieldDescription::new(
                "amplitude",
                FieldType::Vec2,
                "world pixels at the peak",
            ))
            .field(FieldDescription::new(
                "period",
                period(),
                "seconds of game time a swing takes",
            ))
            .sample(r#"{ "amplitude": [0, 3], "period": 1.2 }"#)
            .roundtrip(round_trip::<PositionWave>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                WAVE,
                "A rotation or scale channel: amplitude · sin(2π(t / period + phase)) about the first frame's value.",
            )
            .field(FieldDescription::new(
                "amplitude",
                FieldType::F32 {
                    min: Bound::Open,
                    max: Bound::Open,
                },
                "the peak: radians for `rotation`, a fraction in [0, 1) of the base for `scale`",
            ))
            .field(FieldDescription::new(
                "period",
                period(),
                "seconds of game time a swing takes",
            ))
            .sample(r#"{ "amplitude": 0.3, "period": 0.8 }"#)
            .roundtrip(round_trip::<Wave>),
        );
        world.register_component_with(
            OSCILLATOR_COMPONENT,
            |def: OscillatorDef, at: &SpawnContext<'_>| {
                let mut oscillator = Oscillator::empty().with_phase(def.phase);
                if let Some(wave) = def.position {
                    oscillator = oscillator.with_position(wave.amplitude, wave.period);
                }
                if let Some(wave) = def.rotation {
                    oscillator = oscillator.with_rotation(wave.amplitude, wave.period);
                }
                if let Some(wave) = def.scale {
                    oscillator = oscillator.with_scale(wave.amplitude, wave.period);
                }
                OscillatorSpawn {
                    oscillator,
                    at: at.transform().position,
                }
            },
        );
        let mut runner = Runner::default();
        schedule.add(
            Phase::Move.stage(),
            Phase::Move
                .join(system(OSCILLATOR, move |world: &mut World| {
                    runner.run(world);
                }))
                .after_if_present(LINEAR_MOTION)
                .before_if_present(PLATFORMER_CONTROLLER)
                .before_if_present(TOP_DOWN_MOVER),
        );
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(OSCILLATOR_PRESENT, oscillator_present)),
        );
    }
}

/// `components.oscillator` as a scene writes it, before its channels are
/// checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OscillatorDefFields {
    #[serde(default)]
    position: Option<PositionWave>,
    #[serde(default)]
    rotation: Option<Wave>,
    #[serde(default)]
    scale: Option<Wave>,
    #[serde(default, deserialize_with = "below_one")]
    phase: f32,
}

/// `components.oscillator`, checked: at least one channel and a scale
/// amplitude in `[0, 1)`.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "OscillatorDefFields")]
struct OscillatorDef {
    position: Option<PositionWave>,
    rotation: Option<Wave>,
    scale: Option<Wave>,
    phase: f32,
}

impl TryFrom<OscillatorDefFields> for OscillatorDef {
    type Error = String;

    fn try_from(fields: OscillatorDefFields) -> Result<Self, Self::Error> {
        check_channels(fields.position, fields.rotation, fields.scale, fields.phase)?;
        Ok(Self {
            position: fields.position,
            rotation: fields.rotation,
            scale: fields.scale,
            phase: fields.phase,
        })
    }
}

/// The fixed-step system's state between steps.
#[derive(Default)]
struct Runner {
    /// The step's oscillators, kept so a step allocates nothing once warm.
    entities: Vec<Entity>,
    warned: Warned,
}

impl Runner {
    /// Advances every oscillator's clock by the step and moves its position
    /// channel by the wave's change.
    fn run(&mut self, world: &mut World) {
        let dt = step_dt(world);
        if dt <= 0.0 {
            return;
        }
        self.entities.clear();
        self.entities.extend(
            world
                .query::<(Entity, &Oscillator)>()
                .map(|(entity, _)| entity),
        );
        for &entity in &self.entities {
            let (change, moves) = {
                let oscillator = world.get_mut::<Oscillator>(entity).unwrap();
                let before = oscillator.displacement();
                oscillator.prev_clock = oscillator.clock;
                oscillator.clock += f64::from(dt);
                (oscillator.displacement() - before, oscillator.moves())
            };
            if !moves {
                continue;
            }
            // A follower or an orbit runs after it and sets its goal with
            // the displacement folded in, so it adds nothing beside one. A
            // path follower folds it too but may run either side: the change
            // added after it lands, and one added before it is replaced.
            let driver = driver_of(world, entity);
            if driver == Driver::Placer {
                continue;
            }
            match body_of(world, entity) {
                Body::Bodiless | Body::Static => {
                    let write = if driver == Driver::None {
                        Write::Set
                    } else {
                        Write::Add
                    };
                    step_position(world, entity, change, dt, write);
                }
                Body::Kinematic => match driver {
                    Driver::None => write_velocity(world, entity, change / dt, Write::Set),
                    Driver::Early | Driver::Placer => {
                        write_velocity(world, entity, change / dt, Write::Add);
                    }
                    Driver::Late => self.warned.skip("Oscillator", entity, Skip::LateDriver),
                },
                Body::Dynamic => self.warned.skip("Oscillator", entity, Skip::Dynamic),
                Body::Child => self.warned.skip("Oscillator", entity, Skip::Child),
                Body::Unplaced => self.warned.skip("Oscillator", entity, Skip::Unplaced),
            }
        }
    }
}

/// Writes every oscillator's rotation and scale once a frame, about the
/// values its entity had at its first frame, at the clock drawn between its
/// last two steps as the physics sync draws positions.
fn oscillator_present(world: &mut World) {
    let alpha = world
        .get_resource::<Time>()
        .filter(|time| time.interpolate())
        .map(Time::alpha);
    for (oscillator, transform) in world.query_mut::<(&mut Oscillator, &mut Transform)>() {
        if oscillator.rotation.is_none() && oscillator.scale.is_none() {
            continue;
        }
        let base = *oscillator.base.get_or_insert(Base {
            rotation: transform.rotation,
            scale: transform.scale,
        });
        let t = alpha.map_or(oscillator.clock, |alpha| {
            oscillator.prev_clock + (oscillator.clock - oscillator.prev_clock) * f64::from(alpha)
        });
        if let Some(wave) = oscillator.rotation {
            transform.rotation = base.rotation + oscillator.wave_at(wave, t);
        }
        if let Some(wave) = oscillator.scale {
            transform.scale = base.scale * (1.0 + oscillator.wave_at(wave, t));
        }
    }
}

impl Inspectable for Oscillator {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mut channels = Vec::new();
        if let Some(wave) = self.position {
            channels.push(format!(
                "position {} / {:.2} s",
                point(wave.amplitude),
                wave.period
            ));
        }
        if let Some(wave) = self.rotation {
            channels.push(format!(
                "rotation {:.2} / {:.2} s",
                wave.amplitude, wave.period
            ));
        }
        if let Some(wave) = self.scale {
            channels.push(format!(
                "scale {:.2} / {:.2} s",
                wave.amplitude, wave.period
            ));
        }
        vec![
            ("channels", channels.join(", ")),
            ("clock", format!("{:.2} s", self.clock)),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/oscillator.rs"]
mod tests;
