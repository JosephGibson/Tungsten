//! Spawners: a prefab spawned at an entity's position once per interval of
//! game time.

use std::collections::HashSet;

use glam::Vec2;
use tungsten_core::physics::PHYSICS_SYNC;
use tungsten_core::{
    CommandBuffer, Plugin, Schedule, Stage, Time, Timer, Transform, World, system,
};

use crate::hierarchy::TRANSFORM_PROPAGATE_LATE;

/// [`SpawnerPlugin`]'s system, in `PostUpdate`.
pub const SPAWNER_TICK: &str = "spawner_tick";

/// Spawns a prefab at its entity's position once per interval of game
/// time: an enemy wave, a stream of pickups. [`SpawnerPlugin`]'s system
/// ticks it by [`Time::delta`], so a pause holds it and the time scale
/// stretches it. The first spawn comes after one interval, then one per
/// finished period, so a long frame does not swallow one. Each spawn
/// prepares the prefab at the spawner's `Transform.position` and queues it
/// on the `CommandBuffer`, so it lands at that frame's command flush, before
/// the extract.
///
/// A prefab that does not prepare (an unknown ID, a component that fails)
/// logs one error per prefab ID and spawns nothing; the period still
/// counts. The spawned entities carry no state marker, so a state's exit
/// leaves them.
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
///     schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
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

/// Registers [`SPAWNER_TICK`] in `PostUpdate`, after the physics sync and
/// the late propagation pass when they are registered, so a spawner that
/// moves spawns where it was drawn. Its system ticks every [`Spawner`] with
/// a `Transform` and queues the spawns that are due on the
/// `CommandBuffer`; without a `CommandBuffer` nothing spawns and the
/// spawners hold.
#[derive(Debug, Default, Clone, Copy)]
pub struct SpawnerPlugin;

impl Plugin for SpawnerPlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        let mut ticker = Ticker::default();
        schedule.add(
            Stage::PostUpdate,
            system(SPAWNER_TICK, move |world: &mut World| ticker.run(world))
                .after_if_present(PHYSICS_SYNC)
                .after_if_present(TRANSFORM_PROPAGATE_LATE),
        );
    }
}

/// The system's state between frames.
#[derive(Default)]
struct Ticker {
    /// The prefab IDs whose failure was logged.
    warned: HashSet<String>,
    /// This frame's spawns, kept so a frame allocates no list once warm.
    due: Vec<(String, Vec2)>,
}

impl Ticker {
    fn run(&mut self, world: &mut World) {
        if !world.has_resource::<CommandBuffer>() {
            return;
        }
        let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
        let Self { warned, due } = self;
        for (spawner, transform) in world.query_mut::<(&mut Spawner, &Transform)>() {
            if spawner.remaining == Some(0) {
                continue;
            }
            let mut count = spawner.timer.tick(dt);
            if let Some(remaining) = &mut spawner.remaining {
                count = count.min(*remaining);
                *remaining -= count;
            }
            for _ in 0..count {
                due.push((spawner.prefab.clone(), transform.position));
            }
        }
        for (prefab, position) in due.drain(..) {
            match world.prepare_prefab(&prefab, position) {
                Ok(prepared) => {
                    world.resource_mut::<CommandBuffer>().spawn_prefab(prepared);
                }
                Err(err) => {
                    if !warned.contains(&prefab) {
                        log::error!("Spawner: prefab `{prefab}` spawns nothing: {err}");
                        warned.insert(prefab);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/spawner.rs"]
mod tests;
