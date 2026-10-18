//! Lifetimes: an entity, with its children, despawned once its time on the
//! game clock has passed.

use serde::{Deserialize, Serialize};
use tungsten_core::{
    Entity, Inspectable, Plugin, Schedule, SpawnContext, Time, Timer, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, PLATFORMER, SHMUP, TOP_DOWN,
    describe, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::non_negative;
use crate::foundation::phases::{Phase, ensure_phases};
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
/// Scene name `lifetime`: `{ "seconds": 0.5 }`.
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
        }
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
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(GRID)
            .sample(r#"{ "seconds": 0.5 }"#)
            .roundtrip(round_trip::<LifetimeDef>),
        );
        world.register_component_with(LIFETIME, |def: LifetimeDef, _: &SpawnContext<'_>| {
            (Lifetime::new(def.seconds),)
        });
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick.join(system(LIFETIME_TICK, lifetime_tick)),
        );
        ensure_kit_despawn(schedule, world);
    }
}

/// `components.lifetime` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LifetimeDef {
    #[serde(deserialize_with = "non_negative")]
    seconds: f32,
}

fn lifetime_tick(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let mut finished: Vec<Entity> = Vec::new();
    for (entity, lifetime) in world.query_mut::<(Entity, &mut Lifetime)>() {
        lifetime.timer.tick(dt);
        if lifetime.timer.finished() {
            finished.push(entity);
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
        vec![
            ("left", format!("{:.2} s", self.timer.remaining())),
            ("fraction", format!("{:.2}", self.timer.fraction())),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/lifetime.rs"]
mod tests;
