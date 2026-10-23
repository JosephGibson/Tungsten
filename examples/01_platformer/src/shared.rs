//! What two features share: the save IDs every saved entity carries, a
//! body's centre and the distance to its surface, the short-lived particle
//! bursts, cosmetic and key, and the actions read off the input the UI left
//! to the game.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{
    ActionMap, Collider, Entity, InputState, Particle, ParticleConfigRegistry, ParticleEmitter,
    ParticleEmitterState, Position, RoutedInput, Shape, Transform, World,
};

use tungsten::kit::Children;

use crate::game::GAMEPLAY;

/// A saved entity's stable identity, from the run's [`SaveIds`]: a save
/// names entities by it, never by `Entity`, which the ECS reuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SaveId(pub u64);

/// The run's next save ID, a `World` resource: the map's objects take theirs
/// in the order they spawn, and anything spawned later the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveIds {
    next: u64,
}

impl SaveIds {
    /// A counter whose next ID is `next`, as a save holds it.
    #[must_use]
    pub fn starting_at(next: u64) -> Self {
        Self { next }
    }

    /// The ID the next spawn takes.
    #[must_use]
    pub fn peek(&self) -> u64 {
        self.next
    }

    /// Takes the next ID.
    pub fn take(&mut self) -> SaveId {
        let id = SaveId(self.next);
        self.next += 1;
        id
    }
}

/// `entity`'s body centre: its `Position` plus its collider's offset, the
/// point itself for an entity with no collider. None without a position.
#[must_use]
pub fn body_centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// How far `at` lies from a body of `shape` about `centre`: 0 inside it.
#[must_use]
pub fn surface_distance(at: Vec2, centre: Vec2, shape: Shape) -> f32 {
    let offset = at - centre;
    match shape {
        Shape::Aabb { half_extents } => (offset.abs() - half_extents).max(Vec2::ZERO).length(),
        Shape::Circle { radius } => (offset.length() - radius).max(0.0),
    }
}

/// At most this many bursts count at once; a cosmetic burst past it is
/// dropped, and a key burst takes the oldest cosmetic one's slot.
pub const BURST_CAP: usize = 16;

/// A one-off particle burst at a point, a jump's puff or an impact's
/// sparks, despawned once its particles are gone. Presentation: never
/// saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Burst {
    /// Its place in the run's bursts, which orders the oldest first.
    pub order: u64,
    /// A key burst: a death, gore, a summon, a shatter or a refusal, which
    /// the cap never drops.
    pub key: bool,
    /// It counts against [`BURST_CAP`]; a cosmetic burst that gave its
    /// slot to a key one fades out uncounted.
    pub counted: bool,
}

/// The bursts a run has spawned, which seed the next, so a run's bursts
/// look the same whatever entity IDs the ECS hands out.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bursts(pub u64);

/// Spawns a cosmetic burst of the particle config `name` at `at`, owned by
/// gameplay, unless [`BURST_CAP`] bursts count or the config is unknown. It
/// is spawned at once, so the engine's emit pass of this frame sees it.
pub fn spawn_burst(world: &mut World, name: &str, at: Vec2) -> Option<Entity> {
    if counted_bursts(world) >= BURST_CAP {
        return None;
    }
    burst(world, name, at, false)
}

/// Spawns a key burst of the particle config `name` at `at`: a death,
/// gore, a summon, a shatter or a refusal. At the cap, the oldest cosmetic
/// burst gives up its slot, drained at once and no longer counted, its
/// particles left to fade, and `burst_cleanup` sweeps it as it does a burst;
/// with every slot a key burst's, it is dropped.
pub fn spawn_key_burst(world: &mut World, name: &str, at: Vec2) -> Option<Entity> {
    if counted_bursts(world) >= BURST_CAP {
        let oldest = world
            .query::<(Entity, &Burst)>()
            .filter(|(_, burst)| burst.counted && !burst.key)
            .min_by_key(|(_, burst)| burst.order)
            .map(|(entity, _)| entity)?;
        if let Some(burst) = world.get_mut::<Burst>(oldest) {
            burst.counted = false;
        }
        if let Some(state) = world.get_mut::<ParticleEmitterState>(oldest) {
            state.drained = true;
        }
    }
    burst(world, name, at, true)
}

/// How many bursts count against [`BURST_CAP`].
fn counted_bursts(world: &World) -> usize {
    world
        .query::<&Burst>()
        .filter(|burst| burst.counted)
        .count()
}

/// A burst of `name` at `at`, the run's next.
fn burst(world: &mut World, name: &str, at: Vec2, key: bool) -> Option<Entity> {
    let config = world
        .get_resource::<ParticleConfigRegistry>()?
        .id_for_name(name)?;
    let order = match world.get_resource_mut::<Bursts>() {
        Some(bursts) => {
            bursts.0 = bursts.0.wrapping_add(1);
            bursts.0
        }
        None => 0,
    };
    Some(world.spawn_with((
        Burst {
            order,
            key,
            counted: true,
        },
        Transform::from_position(at),
        ParticleEmitter::with_seed(config, 0x1a_0000 + order),
        ParticleEmitterState::default(),
        SceneEntity { state_id: GAMEPLAY },
    )))
}

/// `burst_cleanup`: despawns each burst whose emitter has ticked, drained
/// and reported it, and that owns no particle.
pub fn burst_cleanup(world: &mut World) {
    let drained: Vec<Entity> = world
        .query::<(Entity, &Burst, &ParticleEmitterState)>()
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
    // One pass over the particles for every drained burst, not one each
    // (P6).
    let owning: std::collections::HashSet<Entity> = world
        .query::<&Particle>()
        .filter_map(|particle| particle.emitter)
        .filter(|emitter| drained.contains(emitter))
        .collect();
    let finished: Vec<Entity> = drained
        .into_iter()
        .filter(|entity| !owning.contains(entity))
        .collect();
    for entity in finished {
        world.despawn(entity);
    }
}

/// Despawns `entity` and every entity that hangs from it, down the tree,
/// at once: a body that leaves mid-step takes its art and emitters along.
pub fn despawn_tree(world: &mut World, entity: Entity) {
    let mut stack = vec![entity];
    while let Some(entity) = stack.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
        world.despawn(entity);
    }
}

/// Whether `action` is held, through the `ActionMap`, in the input the UI
/// left to the game.
#[must_use]
pub fn pressed(world: &World, action: &str) -> bool {
    let Some(actions) = world.get_resource::<ActionMap>() else {
        return false;
    };
    match world.get_resource::<RoutedInput>() {
        Some(routed) => actions.is_pressed(routed, action),
        None => world
            .get_resource::<InputState>()
            .is_some_and(|input| actions.is_pressed(input, action)),
    }
}

/// Whether `action` was just pressed, through the `ActionMap`, in the input
/// the UI left to the game; in the fixed step, since the last step.
#[must_use]
pub fn just_pressed(world: &World, action: &str) -> bool {
    let Some(actions) = world.get_resource::<ActionMap>() else {
        return false;
    };
    match world.get_resource::<RoutedInput>() {
        Some(routed) => actions.just_pressed(routed, action),
        None => world
            .get_resource::<InputState>()
            .is_some_and(|input| actions.just_pressed(input, action)),
    }
}
