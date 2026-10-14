//! Lifetimes: an entity, with its children, despawned once its time on the
//! game clock has passed.

use tungsten_core::{CommandBuffer, Entity, Plugin, Schedule, Stage, Time, Timer, World, system};

use crate::hierarchy::despawn_recursive;

/// [`LifetimePlugin`]'s system, in `PostUpdate`.
pub const LIFETIME_TICK: &str = "lifetime_tick";

/// Despawns its entity, with its children, once its time has passed on the
/// game clock: a bolt, a spark, a floating score. [`LifetimePlugin`]'s
/// system ticks it by [`Time::delta`], so a pause holds every lifetime and
/// the time scale stretches them, and queues [`despawn_recursive`] once it
/// has finished, so the entity goes at that frame's command flush, before
/// the extract. Its [`Timer`] reads how far the entity has come, for a fade.
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
/// // Frames of 0.1 s: the third passes 0.25 s, and its flush takes the spark.
/// for alive in [true, true, false] {
///     world.resource_mut::<Time>().advance_frame(0.1);
///     schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
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

/// Registers [`LIFETIME_TICK`] in `PostUpdate`: it ticks every [`Lifetime`]
/// by the game clock and queues the despawn of each finished one, with its
/// children, for the frame's command flush. Without a `CommandBuffer` it
/// queues nothing; a finished lifetime queues again on the next frame.
#[derive(Debug, Default, Clone, Copy)]
pub struct LifetimePlugin;

impl Plugin for LifetimePlugin {
    fn build(&self, schedule: &mut Schedule, _world: &mut World) {
        schedule.add(Stage::PostUpdate, system(LIFETIME_TICK, lifetime_tick));
    }
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
    if finished.is_empty() {
        return;
    }
    let Some(commands) = world.get_resource_mut::<CommandBuffer>() else {
        return;
    };
    for entity in finished {
        despawn_recursive(commands, entity);
    }
}

#[cfg(test)]
#[path = "tests/lifetime.rs"]
mod tests;
