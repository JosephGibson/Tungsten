//! Spawners: a prefab spawned at an entity's position once per interval of
//! game time.

use std::collections::HashSet;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::{
    CommandBuffer, Entity, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext, Time, Timer,
    Transform, World, system,
};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, GRID, SHMUP, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::positive;
use crate::foundation::trace::{TraceKind, trace};

/// [`SpawnerPlugin`]'s system, in the react phase.
pub const SPAWNER_TICK: &str = "spawner_tick";

/// The scene component the plugin registers.
const SPAWNER: &str = "spawner";

/// Spawns a prefab at its entity's position once per interval of game
/// time: an enemy wave, a stream of pickups. [`SpawnerPlugin`]'s system
/// ticks it once a fixed step, so a pause holds it and the time scale
/// stretches it. The first spawn comes after one interval, then one per
/// finished period, so a long frame does not swallow one. Each spawn
/// prepares the prefab at the spawner's `Position`, or its
/// `Transform.position` when it has none, and queues it on the
/// `CommandBuffer`, so it lands at that frame's command flush, before the
/// extract. A spawner that is a child of a moving parent carries a
/// `Transform` only, written by the last frame's propagation, so its
/// spawns land one frame behind the parent.
///
/// A prefab that does not prepare (an unknown ID, a component that fails)
/// logs one error per prefab ID and spawns nothing; the period still
/// counts. Each spawn comes from its spawner
/// ([`CommandBuffer::spawn_prefab_from`]) and takes what the World's spawn
/// inheritance gives, a state's `SceneEntity` under the umbrella's
/// `StatePlugin`, so it leaves with the spawner's state, and the kit's
/// [`Source`](crate::Source); a spawner despawned earlier in the frame's
/// commands, as by its state's exit, spawns nothing.
///
/// Scene name `spawner`: `{ "prefab": "slime", "interval": 0.5, "count": 3 }`,
/// `count` optional; the load checks the prefab ID.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// use tungsten_core::{
///     CommandBuffer, Plugin, PrefabData, PrefabRegistry, Schedule, Stage, Tag, Time,
///     Transform, World,
/// };
/// use tungsten_kit::{Spawner, SpawnerPlugin};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// world.insert_resource(CommandBuffer::new());
/// let slime: PrefabData = serde_json::from_value(json!({ "tag": "slime" })).unwrap();
/// let mut prefabs = PrefabRegistry::new();
/// prefabs.insert("slime".to_string(), slime);
/// world.insert_resource(prefabs);
/// let mut schedule = Schedule::new();
/// SpawnerPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // A nest that releases a slime every half second, three in all.
/// world.spawn_with((
///     Transform::from_position([64.0, 32.0].into()),
///     Spawner::new("slime", 0.5).with_count(3),
/// ));
/// for _ in 0..20 {
///     world.resource_mut::<Time>().advance_frame(0.1);
///     schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
///     let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
///     world.flush_reusing(&mut commands);
///     world.insert_resource(commands);
/// }
/// assert_eq!(world.query::<&Tag>().count(), 3);
/// ```
#[derive(Debug, Clone)]
pub struct Spawner {
    prefab: String,
    timer: Timer,
    remaining: Option<u32>,
}

impl Spawner {
    /// Spawns prefab `prefab`, a manifest `prefabs` ID, every `interval`
    /// seconds of game time, the first after one interval, without end.
    ///
    /// # Panics
    ///
    /// When `interval` is not finite and above 0, as [`Timer::repeating`].
    #[must_use]
    #[track_caller]
    pub fn new(prefab: impl Into<String>, interval: f32) -> Self {
        assert!(
            interval.is_finite() && interval > 0.0,
            "Spawner::new: interval must be finite and > 0 (got {interval})"
        );
        Self {
            prefab: prefab.into(),
            timer: Timer::repeating(interval),
            remaining: None,
        }
    }

    /// Stops after `count` spawns; 0 spawns nothing.
    #[must_use]
    pub fn with_count(mut self, count: u32) -> Self {
        self.remaining = Some(count);
        self
    }

    /// The prefab it spawns.
    #[must_use]
    pub fn prefab(&self) -> &str {
        &self.prefab
    }

    /// The spawns left; `None` without end.
    #[must_use]
    pub fn remaining(&self) -> Option<u32> {
        self.remaining
    }

    /// The countdown to the next spawn: its `remaining` reads the time
    /// left.
    #[must_use]
    pub fn timer(&self) -> &Timer {
        &self.timer
    }
}

/// Registers the `spawner` scene component and [`SPAWNER_TICK`] in the
/// react phase, last in the fixed step. Its system ticks every [`Spawner`]
/// with a `Position` or a `Transform` and queues the spawns that are due
/// on the `CommandBuffer`; without a `CommandBuffer` nothing spawns and the
/// spawners hold.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpawnerPlugin;

impl Plugin for SpawnerPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Spawner>(world, "Spawner");
        describe(
            world,
            ComponentDescription::new(
                SPAWNER,
                "Spawns a prefab at the entity's position once per interval of game time.",
            )
            .field(FieldDescription::new(
                "prefab",
                FieldType::Prefab,
                "what it spawns; the load checks the ID",
            ))
            .field(FieldDescription::new(
                "interval",
                FieldType::F32 {
                    min: Bound::Exclusive(0.0),
                    max: Bound::Open,
                },
                "seconds between spawns; the first after one interval",
            ))
            .field(
                FieldDescription::new(
                    "count",
                    FieldType::U32 {
                        min: None,
                        max: None,
                    },
                    "how many it spawns in all; without end when left out",
                )
                .default("null"),
            )
            .genre(TOP_DOWN)
            .genre(SHMUP)
            .genre(GRID)
            .sample(r#"{ "prefab": "slime", "interval": 0.5 }"#)
            .roundtrip(round_trip::<SpawnerDef>),
        );
        world.register_component_with_assets(
            SPAWNER,
            |def: &SpawnerDef, world: &World| -> Result<(), MissingAsset> {
                world.check_prefab_id("prefab", &def.prefab)
            },
            |def: SpawnerDef, _: &SpawnContext<'_>| {
                let spawner = Spawner::new(def.prefab, def.interval);
                (def.count
                    .map_or(spawner.clone(), |count| spawner.with_count(count)),)
            },
        );
        let mut ticker = Ticker::default();
        schedule.add(
            Phase::React.stage(),
            Phase::React.join(system(SPAWNER_TICK, move |world: &mut World| {
                ticker.run(world);
            })),
        );
    }
}

/// `components.spawner` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnerDef {
    prefab: String,
    #[serde(deserialize_with = "positive")]
    interval: f32,
    #[serde(default)]
    count: Option<u32>,
}

/// The system's state between frames.
#[derive(Default)]
struct Ticker {
    /// The prefab IDs whose failure was logged.
    warned: HashSet<String>,
    /// This frame's spawns and their spawners, kept so a frame allocates no
    /// list once warm.
    due: Vec<(Entity, String, Vec2)>,
}

impl Ticker {
    fn run(&mut self, world: &mut World) {
        if !world.has_resource::<CommandBuffer>() {
            return;
        }
        let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
        let Self { warned, due } = self;
        for (entity, spawner, position, transform) in
            world.query_mut::<(Entity, &mut Spawner, Option<&Position>, Option<&Transform>)>()
        {
            if spawner.remaining == Some(0) {
                continue;
            }
            let Some(at) = position
                .map(|position| position.0)
                .or_else(|| transform.map(|transform| transform.position))
            else {
                continue;
            };
            let mut count = spawner.timer.tick(dt);
            if let Some(remaining) = &mut spawner.remaining {
                count = count.min(*remaining);
                *remaining -= count;
            }
            for _ in 0..count {
                due.push((entity, spawner.prefab.clone(), at));
            }
        }
        for (spawner, prefab, position) in due.drain(..) {
            match world.prepare_prefab(&prefab, position) {
                Ok(prepared) => {
                    world
                        .resource_mut::<CommandBuffer>()
                        .spawn_prefab_from(spawner, prepared);
                    trace(world, spawner, || TraceKind::Spawn {
                        prefab,
                        by: "spawner",
                    });
                }
                Err(err) => {
                    if !warned.contains(&prefab) {
                        log::error!("Spawner: prefab `{prefab}` spawns nothing: {err}");
                        warned.insert(prefab.clone());
                    }
                    trace(world, spawner, || TraceKind::SpawnFailed {
                        prefab,
                        by: "spawner",
                        error: err.to_string(),
                    });
                }
            }
        }
    }
}

impl Inspectable for Spawner {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("prefab", self.prefab.clone()),
            ("next", format!("{:.2} s", self.timer.remaining())),
            (
                "remaining",
                self.remaining
                    .map_or_else(|| "unbounded".to_string(), |n| n.to_string()),
            ),
        ]
    }
}

#[cfg(test)]
#[path = "../tests/spawner.rs"]
mod tests;
