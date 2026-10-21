//! What two features share: the save IDs every saved entity carries, the
//! short-lived particle bursts and the actions read off the input the UI
//! left to the game.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::{
    ActionMap, Entity, InputState, Particle, ParticleConfigRegistry, ParticleEmitter,
    ParticleEmitterState, RoutedInput, Transform, World,
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

/// At most this many bursts live at once; a burst past it is dropped.
pub const BURST_CAP: usize = 16;

/// A one-off particle burst at a point, a jump's puff or an impact's
/// sparks, despawned once its particles are gone. Presentation: never
/// saved.
#[derive(Debug, Clone, Copy)]
pub struct Burst;

/// The bursts a run has spawned, which seed the next, so a run's bursts
/// look the same whatever entity IDs the ECS hands out.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bursts(pub u64);

/// Spawns a burst of the particle config `name` at `at`, owned by
/// gameplay, unless [`BURST_CAP`] bursts live or the config is unknown. It
/// is spawned at once, so the engine's emit pass of this frame sees it.
pub fn spawn_burst(world: &mut World, name: &str, at: Vec2) -> Option<Entity> {
    if world.query::<&Burst>().count() >= BURST_CAP {
        return None;
    }
    let config = world
        .get_resource::<ParticleConfigRegistry>()?
        .id_for_name(name)?;
    let seed = match world.get_resource_mut::<Bursts>() {
        Some(bursts) => {
            bursts.0 = bursts.0.wrapping_add(1);
            0x1a_0000 + bursts.0
        }
        None => 0x1a_0000,
    };
    Some(world.spawn_with((
        Burst,
        Transform::from_position(at),
        ParticleEmitter::with_seed(config, seed),
        ParticleEmitterState::default(),
        SceneEntity { state_id: GAMEPLAY },
    )))
}

/// `burst_cleanup`: despawns each burst whose emitter has ticked, drained
/// and reported it, and that owns no particle.
pub fn burst_cleanup(world: &mut World) {
    let finished: Vec<Entity> = world
        .query::<(Entity, &Burst, &ParticleEmitterState)>()
        .filter(|(entity, _, state)| {
            state.first_tick_done
                && state.drained
                && state.drain_reported
                && state.active_count == 0
                && !world
                    .query::<&Particle>()
                    .any(|particle| particle.emitter == Some(*entity))
        })
        .map(|(entity, ..)| entity)
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
