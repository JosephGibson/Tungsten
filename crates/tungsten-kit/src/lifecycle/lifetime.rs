//! Lifetimes: an entity, with its children, despawned once its time on the
//! game clock has passed.

use serde::{Deserialize, Serialize};
use tungsten_core::save::SaveOptions;
use tungsten_core::{
    Entity, Inspectable, Plugin, Schedule, SpawnContext, Time, Timer, Visibility, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, RPG, SHOOTER, TOP_DOWN,
    describe, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::{non_negative, positive};
use crate::lifecycle::ended::{end, ensure_kit_despawn};

/// [`LifetimePlugin`]'s system, in the tick phase.
pub const LIFETIME_TICK: &str = "lifetime_tick";

/// The scene component the plugin registers.
const LIFETIME: &str = "lifetime";

/// Despawns its entity, with its children, once its time has passed on the
/// game clock: a bolt, a spark, a floating score. [`LifetimePlugin`]'s
/// system ticks it once a fixed step by the step, so a lifetime ends on a
/// step whatever the frame rate, a pause holds every lifetime and the time
/// scale stretches them; once it has finished the system lists it for the
/// kit's end-of-step despawn ([`KIT_DESPAWN`](crate::KIT_DESPAWN)), which
/// queues [`despawn_recursive`](crate::despawn_recursive) after the step's
/// spawns from it, so the entity goes at that frame's command flush, before
/// the extract, and a cue spawn from it in its last step survives. Its
/// [`Timer`] reads how far the entity has come, for a fade.
///
/// With a blink ([`with_blink`](Self::with_blink)), over its last seconds
/// the entity's `Visibility` is off for the second half of each period,
/// the rate rising from its first to its second, as a dropped coin warns
/// that it is going: the tick writes it after its pass, inserting one at
/// the first hide, as a function of the elapsed time, so a load blinks on
/// in step.
///
/// Scene name `lifetime`: `{ "seconds": 0.5 }`, or
/// `{ "seconds": 6, "blink": { "last": 2, "rate": [5, 10] } }`, `last`
/// above 0 and each rate above 0, `[5, 5]` by default; saved with its
/// seconds and elapsed time, the blink skipped when none.
///
/// # Examples
///
/// ```
/// use tungsten_core::{CommandBuffer, Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Lifetime, LifetimePlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// world.insert_resource(CommandBuffer::new());
/// let mut schedule = Schedule::new();
/// LifetimePlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let spark = world.spawn_with((Lifetime::new(0.25),));
/// // Steps of 0.1 s: the third passes 0.25 s, and its flush takes the spark.
/// for alive in [true, true, false] {
///     world.resource_mut::<Time>().advance_frame(0.1);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
///     world.flush_reusing(&mut commands);
///     world.insert_resource(commands);
///     assert_eq!(world.is_alive(spark), alive);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Lifetime {
    timer: Timer,
    blink: Option<Blink>,
}

/// A lifetime's blink: over its last `last` seconds, hidden for the second
/// half of each period, the periods a second rising linearly from
/// `rate[0]` to `rate[1]`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Blink {
    last: f32,
    rate: [f32; 2],
}

impl Blink {
    /// Whether its entity shows at `elapsed` of a lifetime of `seconds`:
    /// before the blink, or in the first half of a period by the phase
    /// `r0·τ + (r1 − r0)·τ²/(2·last)`, `τ` seconds into the blink.
    fn shows(self, seconds: f32, elapsed: f32) -> bool {
        let tau = elapsed - (seconds - self.last);
        if tau < 0.0 {
            return true;
        }
        let [r0, r1] = self.rate;
        let phase = r0 * tau + (r1 - r0) * tau * tau / (2.0 * self.last);
        phase.fract() < 0.5
    }
}

/// Whether a blink's values are finite and above 0.
fn valid_blink(last: f32, rate: [f32; 2]) -> bool {
    last.is_finite() && last > 0.0 && rate.iter().all(|r| r.is_finite() && *r > 0.0)
}

impl Lifetime {
    /// A lifetime of `seconds` on the game clock; zero despawns the entity
    /// at its first tick.
    ///
    /// # Panics
    ///
    /// When `seconds` is negative or not finite, as [`Timer::once`].
    #[must_use]
    pub fn new(seconds: f32) -> Self {
        Self {
            timer: Timer::once(seconds),
            blink: None,
        }
    }

    /// The lifetime, blinking over its last `last` seconds: hidden for the
    /// second half of each period, `from` periods a second at the blink's
    /// start rising linearly to `to` at the end.
    ///
    /// # Panics
    ///
    /// When `last`, `from` or `to` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_blink(mut self, last: f32, from: f32, to: f32) -> Self {
        assert!(
            valid_blink(last, [from, to]),
            "Lifetime::with_blink: each must be finite and > 0 (got {last}, {from}, {to})"
        );
        self.blink = Some(Blink {
            last,
            rate: [from, to],
        });
        self
    }

    /// Whether its entity shows now: always without a blink.
    #[must_use]
    pub fn shows(&self) -> bool {
        self.blink
            .is_none_or(|blink| blink.shows(self.timer.duration(), self.timer.elapsed()))
    }

    /// The countdown: its `remaining` and `fraction` read how far the
    /// entity has come.
    #[must_use]
    pub fn timer(&self) -> &Timer {
        &self.timer
    }
}

/// Registers the `lifetime` scene component, [`LIFETIME_TICK`] in the tick
/// phase and the kit's end-of-step despawn when the schedule lacks it: each
/// fixed step the tick ticks every [`Lifetime`] by the step and lists each
/// finished one for the despawn, which queues it, with its children, for
/// the frame's command flush. Without a `CommandBuffer` nothing is queued;
/// a finished lifetime is listed again on the next step.
#[derive(Debug, Default, Clone, Copy)]
pub struct LifetimePlugin;

impl Plugin for LifetimePlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Lifetime>(world, "Lifetime");
        world.register_saved_with(LIFETIME, SaveOptions::<Lifetime>::form::<LifetimeSave>());
        describe(
            world,
            ComponentDescription::new(
                LIFETIME,
                "Despawns the entity, with its children, once its time on the game clock has passed.",
            )
            .field(FieldDescription::new(
                "seconds",
                FieldType::F32 {
                    min: Bound::Inclusive(0.0),
                    max: Bound::Open,
                },
                "how long it lives; 0 despawns it at its first step",
            ))
            .field(
                FieldDescription::new(
                    "blink",
                    FieldType::Shape(BLINK),
                    "over its last seconds its visibility off for the second half of each period, the rate rising; saved with it",
                )
                .default("null"),
            )
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(RPG)
            .genre(SHOOTER)
            .sample(r#"{ "seconds": 0.5 }"#)
            .roundtrip(round_trip::<LifetimeDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(
                BLINK,
                "A lifetime's blink: hidden for the second half of each period over its last seconds.",
            )
            .field(FieldDescription::new(
                "last",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "the seconds before the end it blinks over",
            ))
            .field(
                FieldDescription::new(
                    "rate",
                    FieldType::Vec2,
                    "periods a second at the blink's start and at the end, each above 0, rising linearly between",
                )
                .default("[5.0, 5.0]"),
            )
            .sample(r#"{ "last": 2 }"#)
            .roundtrip(round_trip::<BlinkDef>),
        );
        world.register_component_with(LIFETIME, |def: LifetimeDef, _: &SpawnContext<'_>| {
            let mut lifetime = Lifetime::new(def.seconds);
            lifetime.blink = def.blink.map(Blink::from);
            (lifetime,)
        });
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick.join(system(LIFETIME_TICK, lifetime_tick)),
        );
        ensure_kit_despawn(schedule, world);
    }
}

/// The shape of a lifetime's `blink`.
const BLINK: &str = "blink";

/// A [`Lifetime`] in a save: its seconds, how far it has come and its
/// blink, when it has one.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifetimeSave {
    seconds: f32,
    elapsed: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    blink: Option<BlinkDef>,
}

impl From<&Lifetime> for LifetimeSave {
    fn from(lifetime: &Lifetime) -> Self {
        Self {
            seconds: lifetime.timer.duration(),
            elapsed: lifetime.timer.elapsed(),
            blink: lifetime.blink.map(|blink| BlinkDef {
                last: blink.last,
                rate: blink.rate,
            }),
        }
    }
}

impl From<LifetimeSave> for Lifetime {
    /// A seconds value that is not finite and at least 0 reads as 0.
    fn from(saved: LifetimeSave) -> Self {
        let seconds = finite_at_least_zero(saved.seconds);
        let mut lifetime = Self::new(seconds);
        lifetime.timer.tick(finite_at_least_zero(saved.elapsed));
        lifetime.blink = saved.blink.map(Blink::from);
        lifetime
    }
}

/// A blink as data writes it, checked.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "BlinkFields")]
struct BlinkDef {
    last: f32,
    rate: [f32; 2],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BlinkFields {
    #[serde(deserialize_with = "positive")]
    last: f32,
    #[serde(default = "steady_rate")]
    rate: [f32; 2],
}

/// A blink's rate by default: five periods a second throughout.
fn steady_rate() -> [f32; 2] {
    [5.0, 5.0]
}

impl TryFrom<BlinkFields> for BlinkDef {
    type Error = String;

    fn try_from(fields: BlinkFields) -> Result<Self, Self::Error> {
        if !valid_blink(fields.last, fields.rate) {
            return Err(format!(
                "rate: each must be finite and > 0 (got {:?})",
                fields.rate
            ));
        }
        Ok(Self {
            last: fields.last,
            rate: fields.rate,
        })
    }
}

impl From<BlinkDef> for Blink {
    fn from(def: BlinkDef) -> Self {
        Self {
            last: def.last,
            rate: def.rate,
        }
    }
}

fn finite_at_least_zero(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// `components.lifetime` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifetimeDef {
    #[serde(deserialize_with = "non_negative")]
    seconds: f32,
    #[serde(default)]
    blink: Option<BlinkDef>,
}

fn lifetime_tick(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let mut finished: Vec<Entity> = Vec::new();
    // Blinkers listed beside the finished, so the pass's query stays the
    // two columns it reads.
    let mut blinking: Vec<(Entity, bool)> = Vec::new();
    for (entity, lifetime) in world.query_mut::<(Entity, &mut Lifetime)>() {
        lifetime.timer.tick(dt);
        if lifetime.timer.finished() {
            finished.push(entity);
        }
        if lifetime.blink.is_some() {
            blinking.push((entity, lifetime.shows()));
        }
    }
    for (entity, shows) in blinking {
        match world.get_mut::<Visibility>(entity) {
            Some(visibility) => visibility.visible = shows,
            None if !shows => world.insert(entity, Visibility { visible: false }),
            None => {}
        }
    }
    // Listed each step until the flush; the despawn marks it the first
    // time and skips it after.
    for entity in finished {
        end(world, entity, "lifetime");
    }
}

impl Inspectable for Lifetime {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = vec![
            ("left", format!("{:.2} s", self.timer.remaining())),
            ("fraction", format!("{:.2}", self.timer.fraction())),
        ];
        if let Some(blink) = self.blink {
            rows.push((
                "blink",
                format!(
                    "last {:.2} s at {}..{} a second",
                    blink.last, blink.rate[0], blink.rate[1]
                ),
            ));
        }
        rows
    }
}

#[cfg(test)]
#[path = "../tests/lifetime.rs"]
mod tests;
