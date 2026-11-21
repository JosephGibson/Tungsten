//! One-off particle bursts (7.13c's foes and cues): a manifest particle
//! config fired once at a point, a jump's puff or an impact's sparks,
//! spawned at once so the frame's emit pass sees it, under a game's budget,
//! and despawned once its particles are gone.

use glam::Vec2;
use tungsten_core::schedule::names::PARTICLE_COUNT_REFRESH;
use tungsten_core::{
    AssetId, Bundle, Entity, Particle, ParticleConfig, ParticleConfigRegistry, ParticleEmitter,
    ParticleEmitterState, Plugin, Schedule, Stage, Transform, World, system,
};

use crate::foundation::phases::ensure_phases;

/// [`BurstPlugin`]'s sweep, in `post_update` before the frame's particle
/// passes, reading the last frame's tick.
pub const BURST_CLEANUP: &str = "burst_cleanup";

/// A one-off burst [`burst`] spawned: its place among the World's bursts,
/// which orders the oldest first; whether it is a key burst (a death, a
/// summon, a shatter), which the budget never drops; and whether it counts
/// against the budget, which a cosmetic burst that gave its slot to a key
/// one no longer does while its particles fade. Presentation: no save holds
/// it. Named apart from core's `EmissionKind::Burst`, its config's emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParticleBurst {
    /// Its place among the World's bursts, from 1.
    pub order: u64,
    /// A key burst, which the budget never drops.
    pub key: bool,
    /// It counts against the budget's cap.
    pub counted: bool,
}

/// The bursts' budget, a World resource: at most `cap` bursts count at
/// once, 16 by default, and the bursts spawned so far, which seed the next,
/// so a run's bursts look the same whatever entity IDs the World hands
/// out. A game sets the cap with [`with_cap`](Self::with_cap) and starts a
/// run's count again by inserting a fresh one; [`burst`] inserts the
/// default where the World has none.
///
/// # Examples
///
/// ```
/// use tungsten_kit::BurstBudget;
///
/// let budget = BurstBudget::new().with_cap(4);
/// assert_eq!((budget.cap(), budget.spawned()), (4, 0));
/// assert_eq!(BurstBudget::default().cap(), BurstBudget::DEFAULT_CAP);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BurstBudget {
    cap: usize,
    spawned: u64,
}

impl Default for BurstBudget {
    fn default() -> Self {
        Self {
            cap: Self::DEFAULT_CAP,
            spawned: 0,
        }
    }
}

impl BurstBudget {
    /// The cap a World's budget has until a game sets one.
    pub const DEFAULT_CAP: usize = 16;

    /// The default budget: [`DEFAULT_CAP`](Self::DEFAULT_CAP) bursts, none
    /// spawned.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The budget, `cap` bursts counting at once.
    #[must_use]
    pub fn with_cap(mut self, cap: usize) -> Self {
        self.cap = cap;
        self
    }

    /// How many bursts count at once.
    #[must_use]
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// The bursts spawned so far.
    #[must_use]
    pub fn spawned(&self) -> u64 {
        self.spawned
    }
}

/// Spawns a one-off burst of the manifest particle config `particles` at
/// `at`, at once, so the frame's emit pass sees it: an emitter seeded
/// `0x1a_0000` plus its order, the World's bursts counted from 1, marked
/// [`ParticleBurst`]. While the [`BurstBudget`]'s cap of bursts count, a
/// cosmetic burst (`key` false) is dropped, and a key one takes the oldest
/// counted cosmetic burst's slot, that emitter drained at once and no
/// longer counted, its particles left to fade, or is dropped when every
/// counted burst is a key one. `None` when it is dropped or the config is
/// unknown. [`BurstPlugin`]'s sweep despawns it once its emitter has
/// drained and it owns no particle.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use std::sync::Arc;
/// use glam::Vec2;
/// use tungsten_core::{ParticleConfig, ParticleConfigRegistry, World};
/// use tungsten_kit::{BurstBudget, ParticleBurst, burst};
///
/// let mut world = World::new();
/// let sparks: ParticleConfig = serde_json::from_value(serde_json::json!({
///     "sprite": "spark", "max_alive": 8,
///     "emission": { "kind": "continuous", "rate_hz": 30.0 },
///     "lifetime": { "min": 0.2, "max": 0.2 },
///     "initial_velocity": { "kind": "radial", "speed": { "min": 40.0, "max": 60.0 } }
/// }))
/// .unwrap();
/// let mut configs = ParticleConfigRegistry::new();
/// configs.register("sparks".into(), PathBuf::from("sparks.json"), Arc::new(sparks));
/// world.insert_resource(configs);
/// world.insert_resource(BurstBudget::new().with_cap(1));
/// let first = burst(&mut world, "sparks", Vec2::ZERO, false).unwrap();
/// assert_eq!(world.get::<ParticleBurst>(first).unwrap().order, 1);
/// assert!(burst(&mut world, "sparks", Vec2::ZERO, false).is_none(), "at the cap");
/// assert!(burst(&mut world, "sparks", Vec2::ZERO, true).is_some(), "a key one takes its slot");
/// assert!(!world.get::<ParticleBurst>(first).unwrap().counted);
/// ```
pub fn burst(world: &mut World, particles: &str, at: Vec2, key: bool) -> Option<Entity> {
    let (config, order) = admit(world, particles, key)?;
    Some(world.spawn_with(parts(config, order, at, key)))
}

/// [`burst`], its entity spawned with `bundle` too in the same archetype
/// move: a state's ownership, a game's marker.
pub fn burst_with<B: Bundle>(
    world: &mut World,
    particles: &str,
    at: Vec2,
    key: bool,
    bundle: B,
) -> Option<Entity> {
    let (config, order) = admit(world, particles, key)?;
    Some(world.spawn_with(parts(config, order, at, key).with(bundle)))
}

/// A burst's admission under the budget, as [`burst`] says: its config and
/// its order, the budget's count moved on; `None` when it is dropped or the
/// config is unknown.
fn admit(world: &mut World, particles: &str, key: bool) -> Option<(AssetId<ParticleConfig>, u64)> {
    if !world.has_resource::<BurstBudget>() {
        world.insert_resource(BurstBudget::default());
    }
    let cap = world.resource::<BurstBudget>().cap;
    if counted(world) >= cap {
        if !key {
            return None;
        }
        let oldest = world
            .query::<(Entity, &ParticleBurst)>()
            .filter(|(_, burst)| burst.counted && !burst.key)
            .min_by_key(|(_, burst)| burst.order)
            .map(|(entity, _)| entity)?;
        if let Some(burst) = world.get_mut::<ParticleBurst>(oldest) {
            burst.counted = false;
        }
        if let Some(state) = world.get_mut::<ParticleEmitterState>(oldest) {
            state.drained = true;
        }
    }
    let config = world
        .get_resource::<ParticleConfigRegistry>()?
        .id_for_name(particles)?;
    let budget = world.resource_mut::<BurstBudget>();
    budget.spawned = budget.spawned.wrapping_add(1);
    Some((config, budget.spawned))
}

/// A burst's own components.
fn parts(
    config: AssetId<ParticleConfig>,
    order: u64,
    at: Vec2,
    key: bool,
) -> (
    ParticleBurst,
    Transform,
    ParticleEmitter,
    ParticleEmitterState,
) {
    (
        ParticleBurst {
            order,
            key,
            counted: true,
        },
        Transform::from_position(at),
        ParticleEmitter::with_seed(config, 0x1a_0000 + order),
        ParticleEmitterState::default(),
    )
}

/// How many bursts count against the budget.
fn counted(world: &World) -> usize {
    world
        .query::<&ParticleBurst>()
        .filter(|burst| burst.counted)
        .count()
}

/// [`BURST_CLEANUP`]: despawns each burst whose emitter has ticked, drained
/// and reported it, and that owns no particle, one pass over the particles
/// for every drained burst.
fn cleanup(world: &mut World) {
    let drained: Vec<Entity> = world
        .query::<(Entity, &ParticleBurst, &ParticleEmitterState)>()
        .filter(|(_, _, state)| {
            state.first_tick_done
                && state.drained
                && state.drain_reported
                && state.active_count == 0
        })
        .map(|(entity, ..)| entity)
        .collect();
    if drained.is_empty() {
        return;
    }
    let owning: std::collections::HashSet<Entity> = world
        .query::<&Particle>()
        .filter_map(|particle| particle.emitter)
        .filter(|emitter| drained.contains(emitter))
        .collect();
    for entity in drained {
        if !owning.contains(&entity) {
            world.despawn(entity);
        }
    }
}

/// Inserts the default [`BurstBudget`] where the World has none and adds
/// [`BURST_CLEANUP`] in `post_update` before the frame's particle passes,
/// so a burst the last frame's tick drained, owning no particle, goes before
/// this frame emits: a burst drained in a frame is still there for that
/// frame's readers.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, World};
/// use tungsten_kit::{BURST_CLEANUP, BurstBudget, BurstPlugin};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// BurstPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// assert_eq!(schedule.stage_of(BURST_CLEANUP), Some(Stage::PostUpdate));
/// assert!(world.has_resource::<BurstBudget>());
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct BurstPlugin;

impl Plugin for BurstPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        if !world.has_resource::<BurstBudget>() {
            world.insert_resource(BurstBudget::default());
        }
        if schedule.contains(BURST_CLEANUP) {
            return;
        }
        schedule.add(
            Stage::PostUpdate,
            system(BURST_CLEANUP, cleanup).before_if_present(PARTICLE_COUNT_REFRESH),
        );
    }
}

#[cfg(test)]
#[path = "../tests/burst.rs"]
mod tests;
