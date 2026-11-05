//! Spawners: a prefab spawned at an entity's position once per interval of
//! game time.

use std::collections::HashSet;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::Position;
use tungsten_core::save::{RefCodec, RefReader, RefWriter, SaveOptions};
use tungsten_core::{
    CommandBuffer, Entity, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext, Time, Timer,
    Transform, World, system,
};

use crate::TIME_EPSILON;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, RPG, SHOOTER, TOP_DOWN, describe,
    round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::saved::{order_of, saving};
use crate::foundation::trace::{TraceKind, trace};
use crate::foundation::{non_negative, positive};
use crate::lifecycle::cap::admit;
use crate::overlap::grid::takes_part;

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
/// With `alive` (redo 7.10g), it keeps at most that many of its spawns
/// live, alive, not despawning and not dead by their health, those queued
/// this frame counted: while it is full its countdown holds at 0, and once
/// one goes it runs, so `alive: 1` spawns again an interval after its spawn
/// dies. Each of its spawns carries the spawner's name from its
/// preparation, saved as `spawned` by save ID, so a capture between the
/// frame's flush and its next tick keeps its count. `first` sets the delay
/// of its first spawn, from 0 to its interval, so a nest can stand stocked
/// at the start.
///
/// Scene name `spawner`: `{ "prefab": "slime", "interval": 0.5, "count": 3, "alive": 1, "first": 0 }`,
/// `count`, `alive` and `first` optional; the load checks the prefab ID
/// and refuses a `first` past the interval.
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
    /// At most this many of its spawns live at once.
    alive: Option<u32>,
    /// Game seconds left to its first spawn, while a `first` delay runs.
    first_left: Option<f32>,
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
            alive: None,
            first_left: None,
        }
    }

    /// Stops after `count` spawns; 0 spawns nothing.
    #[must_use]
    pub fn with_count(mut self, count: u32) -> Self {
        self.remaining = Some(count);
        self
    }

    /// Keeps at most `alive` of its spawns live: while that many live, its
    /// countdown holds at 0, so it spawns again an interval after one goes.
    ///
    /// # Panics
    ///
    /// When `alive` is 0.
    #[must_use]
    #[track_caller]
    pub fn with_alive(mut self, alive: u32) -> Self {
        assert!(alive >= 1, "Spawner::with_alive: alive must be at least 1");
        self.alive = Some(alive);
        self
    }

    /// Spawns first after `seconds`, from 0, at its first tick, to its
    /// interval, the default; the interval after that.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite, or outside 0 to its interval.
    #[must_use]
    #[track_caller]
    pub fn with_first(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && (0.0..=self.timer.duration()).contains(&seconds),
            "Spawner::with_first: seconds must be within 0..={} (got {seconds})",
            self.timer.duration()
        );
        self.first_left = Some(seconds);
        self
    }

    /// The most of its spawns it keeps live, when it holds to a number.
    #[must_use]
    pub fn alive(&self) -> Option<u32> {
        self.alive
    }

    /// The spawns due this step after `dt`: its first, once a `first`
    /// delay has run, its countdown starting from that step; then one per
    /// finished period.
    fn tick(&mut self, dt: f32) -> u32 {
        let Some(left) = self.first_left.as_mut() else {
            return self.timer.tick(dt);
        };
        *left -= dt;
        if *left > TIME_EPSILON {
            return 0;
        }
        self.first_left = None;
        self.timer.reset();
        1
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

/// A [`Spawner`] in a save: its prefab, interval and spawns left, and how
/// far its countdown has come.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnerSave {
    prefab: String,
    interval: f32,
    elapsed: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    remaining: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alive: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    first_left: Option<f32>,
}

impl From<&Spawner> for SpawnerSave {
    fn from(spawner: &Spawner) -> Self {
        Self {
            prefab: spawner.prefab.clone(),
            interval: spawner.timer.duration(),
            elapsed: spawner.timer.elapsed(),
            remaining: spawner.remaining,
            alive: spawner.alive,
            first_left: spawner.first_left,
        }
    }
}

impl From<SpawnerSave> for Spawner {
    /// An interval that is not finite and above 0 reads as one second.
    fn from(saved: SpawnerSave) -> Self {
        let interval = if saved.interval.is_finite() && saved.interval > 0.0 {
            saved.interval
        } else {
            1.0
        };
        let mut spawner = Self::new(saved.prefab, interval);
        spawner.remaining = saved.remaining;
        spawner.alive = saved.alive.filter(|alive| *alive >= 1);
        spawner.first_left = saved
            .first_left
            .filter(|left| left.is_finite() && *left >= 0.0);
        if saved.elapsed.is_finite() && saved.elapsed > 0.0 {
            spawner.timer.tick(saved.elapsed);
        }
        spawner
    }
}

/// The name a save holds [`Spawned`] under, and its reference's key.
const SPAWNED: &str = "spawned";

/// The spawner an `alive` spawner's spawn came from, what its count reads
/// (redo 7.10g); `None` for one loaded without its spawner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Spawned {
    by: Option<Entity>,
}

/// A [`Spawned`] in a save: nothing but its spawner, which the codec writes
/// by save ID.
#[derive(Serialize, Deserialize)]
struct SpawnedSave;

impl Spawned {
    /// The spawner it came from, when it resolved.
    pub(crate) fn by(&self) -> Option<Entity> {
        self.by
    }
}

impl From<&Spawned> for SpawnedSave {
    fn from(_: &Spawned) -> Self {
        Self
    }
}

impl From<SpawnedSave> for Spawned {
    fn from(_: SpawnedSave) -> Self {
        Self { by: None }
    }
}

/// `Spawned` saved as its spawner's reference; a spawner that does not
/// resolve leaves it naming none.
fn spawned_saved() -> SaveOptions<Spawned> {
    SaveOptions::<Spawned>::form::<SpawnedSave>().refs(RefCodec {
        save: |spawned, out: &mut RefWriter<'_>| {
            if let Some(by) = spawned.by {
                out.put(SPAWNED, by);
            }
        },
        load: |spawned, refs: &RefReader<'_>| {
            spawned.by = refs.first(SPAWNED);
        },
        keys: |_| vec![SPAWNED.to_owned()],
    })
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
        world.register_saved_with(SPAWNER, SaveOptions::<Spawner>::form::<SpawnerSave>());
        world.register_saved_with(SPAWNED, spawned_saved());
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
            .field(
                FieldDescription::new(
                    "alive",
                    FieldType::U32 {
                        min: Some(1),
                        max: None,
                    },
                    "the most of its spawns it keeps live; while full its countdown holds at 0, so it spawns again an interval after one goes",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "first",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "seconds to its first spawn, up to its interval, which the load checks; one interval when left out",
                )
                .default("null"),
            )
            .genre(TOP_DOWN)
            .genre(RPG)
            .genre(SHOOTER)
            .sample(r#"{ "prefab": "slime", "interval": 0.5 }"#)
            .roundtrip(round_trip::<SpawnerDef>),
        );
        world.register_component_with_assets(
            SPAWNER,
            |def: &SpawnerDef, world: &World| -> Result<(), MissingAsset> {
                if let Some(first) = def.first
                    && first > def.interval
                {
                    return Err(MissingAsset::new(
                        "first",
                        "delay within the interval",
                        first.to_string(),
                    ));
                }
                world.check_prefab_id("prefab", &def.prefab)
            },
            |def: SpawnerDef, _: &SpawnContext<'_>| {
                let mut spawner = Spawner::new(def.prefab, def.interval);
                spawner.remaining = def.count;
                spawner.alive = def.alive;
                spawner.first_left = def.first.map(|first| first.min(def.interval));
                (spawner,)
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
    #[serde(default, deserialize_with = "at_least_one")]
    alive: Option<u32>,
    #[serde(default, deserialize_with = "optional_non_negative")]
    first: Option<f32>,
}

/// An optional count of at least 1.
fn at_least_one<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<u32>, D::Error> {
    let value = Option::<u32>::deserialize(deserializer)?;
    if value == Some(0) {
        return Err(serde::de::Error::custom("alive: must be at least 1"));
    }
    Ok(value)
}

/// An optional number, finite and at least 0.
fn optional_non_negative<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f32>, D::Error> {
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct Checked(#[serde(deserialize_with = "non_negative")] f32);
    Ok(Option::<Checked>::deserialize(deserializer)?.map(|checked| checked.0))
}

/// The system's state between frames.
#[derive(Default)]
struct Ticker {
    /// The prefab IDs whose failure was logged.
    warned: HashSet<String>,
    /// This frame's spawns, their spawners and whether each is an `alive`
    /// spawner's, kept so a frame allocates no list once warm.
    due: Vec<(Entity, String, Vec2, bool)>,
    /// The step's `alive` spawners, with their points.
    holding: Vec<(Entity, Vec2)>,
    /// The step's live spawns, by spawner.
    live: Vec<(Entity, u32)>,
    /// The spawns each `alive` spawner queued, by frame.
    queued: Vec<(Entity, u64, u32)>,
}

impl Ticker {
    fn run(&mut self, world: &mut World) {
        if !world.has_resource::<CommandBuffer>() {
            return;
        }
        let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
        let frame = world.get_resource::<Time>().map_or(0, Time::frame);
        let Self {
            warned,
            due,
            holding,
            live,
            queued,
        } = self;
        holding.clear();
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
            if spawner.alive.is_some() {
                holding.push((entity, at));
                continue;
            }
            let mut count = spawner.tick(dt);
            if let Some(remaining) = &mut spawner.remaining {
                count = count.min(*remaining);
                *remaining -= count;
            }
            for _ in 0..count {
                due.push((entity, spawner.prefab.clone(), at, false));
            }
        }
        // The `alive` spawners, after one pass over the spawns, which runs
        // only while one exists.
        if !holding.is_empty() {
            live.clear();
            for (spawn, spawned) in world.query::<(Entity, &Spawned)>() {
                let Some(by) = spawned.by() else {
                    continue;
                };
                if !takes_part(world, spawn) {
                    continue;
                }
                match live.iter_mut().find(|(spawner, _)| *spawner == by) {
                    Some((_, count)) => *count += 1,
                    None => live.push((by, 1)),
                }
            }
            queued.retain(|&(_, at, _)| at == frame);
            for &(entity, at) in holding.iter() {
                let alive_now = live
                    .iter()
                    .find(|(spawner, _)| *spawner == entity)
                    .map_or(0, |&(_, count)| count)
                    + queued
                        .iter()
                        .filter(|&&(spawner, _, _)| spawner == entity)
                        .map(|&(_, _, count)| count)
                        .sum::<u32>();
                let spawner = world.get_mut::<Spawner>(entity).unwrap();
                let alive = spawner.alive.unwrap_or(u32::MAX);
                if alive_now >= alive {
                    spawner.timer.reset();
                    continue;
                }
                let mut count = spawner.tick(dt).min(alive - alive_now);
                if let Some(remaining) = &mut spawner.remaining {
                    count = count.min(*remaining);
                    *remaining -= count;
                }
                if count == 0 {
                    continue;
                }
                match queued.iter_mut().find(|(spawner, _, _)| *spawner == entity) {
                    Some((_, _, queued)) => *queued += count,
                    None => queued.push((entity, frame, count)),
                }
                for _ in 0..count {
                    due.push((entity, spawner.prefab.clone(), at, true));
                }
            }
        }
        // While saving, by the spawner's save ID, so two spawners due in
        // one step give their spawns the same IDs whatever their rows.
        if saving(world) {
            let order = order_of(world);
            due.sort_by_key(|&(spawner, ..)| order.key(spawner));
        }
        for (spawner, prefab, position, holds) in due.drain(..) {
            match world
                .prepare_prefab(&prefab, position)
                .map(|prepared| {
                    if holds {
                        prepared.with((Spawned { by: Some(spawner) },))
                    } else {
                        prepared
                    }
                })
                .map(|prepared| admit(world, &prefab, Some(spawner), prepared))
            {
                Ok(Ok(prepared)) => {
                    world
                        .resource_mut::<CommandBuffer>()
                        .spawn_prefab_from(spawner, prepared);
                    trace(world, spawner, || TraceKind::Spawn {
                        prefab,
                        by: "spawner",
                    });
                }
                // Refused at its cap: the spawner gets the spawn back.
                Ok(Err(why)) => {
                    if let Some(remaining) = world
                        .get_mut::<Spawner>(spawner)
                        .and_then(|spawner| spawner.remaining.as_mut())
                    {
                        *remaining += 1;
                    }
                    if holds
                        && let Some((_, _, count)) =
                            queued.iter_mut().find(|(by, _, _)| *by == spawner)
                    {
                        *count = count.saturating_sub(1);
                    }
                    trace(world, spawner, || TraceKind::Skipped {
                        event: SPAWNER,
                        action: "spawn",
                        why,
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
        let mut rows = vec![
            ("prefab", self.prefab.clone()),
            ("next", format!("{:.2} s", self.timer.remaining())),
            (
                "remaining",
                self.remaining
                    .map_or_else(|| "unbounded".to_string(), |n| n.to_string()),
            ),
        ];
        if let Some(alive) = self.alive {
            rows.push(("alive", format!("at most {alive}")));
        }
        rows
    }
}

#[cfg(test)]
#[path = "../tests/spawner.rs"]
mod tests;
