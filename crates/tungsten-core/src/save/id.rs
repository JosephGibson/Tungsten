//! Save identities: the stable name a save gives each entity it keeps,
//! which the ECS's reused `Entity` handles cannot be.
//!
//! An entity a game names takes [`SaveId::authored`] from its name; a run's
//! spawn from a saved source takes the run's next ID, counted below
//! [`AUTHORED`]. A World where nothing was named holds no [`SaveState`], and
//! nothing in it pays for saves.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::registry::{Holder, SaveRegistry};
use crate::ecs::{Entity, World};

/// The bit every authored save ID sets and no run's spawn reaches.
pub const AUTHORED: u64 = 1 << 31;

/// A saved entity's stable identity (`D-222`'s): a save names entities by
/// it, never by `Entity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SaveId(pub u64);

impl SaveId {
    /// The ID of the entity named `name`: the name's 32-bit FNV-1a hash with
    /// [`AUTHORED`] set, so a map's order moves no save.
    #[must_use]
    pub fn authored(name: &str) -> Self {
        let mut hash: u32 = 0x811c_9dc5;
        for byte in name.bytes() {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
        Self(u64::from(hash) | AUTHORED)
    }

    /// Whether a name gave it, rather than a run's counter.
    #[must_use]
    pub fn is_authored(self) -> bool {
        self.0 & AUTHORED != 0
    }
}

/// The component a saved entity carries: its ID, its area (none for a
/// global entity) and, for a run's spawn, the prefab it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saved {
    pub(crate) id: SaveId,
    pub(crate) area: Option<String>,
    pub(crate) prefab: Option<String>,
}

impl Saved {
    /// Its save ID.
    #[must_use]
    pub fn id(&self) -> SaveId {
        self.id
    }

    /// The area it belongs to; `None` for a global entity.
    #[must_use]
    pub fn area(&self) -> Option<&str> {
        self.area.as_deref()
    }

    /// The prefab a run's spawn came from; `None` for a named entity.
    #[must_use]
    pub fn prefab(&self) -> Option<&str> {
        self.prefab.as_deref()
    }
}

/// An entity a game named: its name, area and entity, and its registered
/// components as they were at the naming, which a capture writes changes
/// over.
#[derive(Debug, Clone)]
// The capture (redo 7.9's step 2) reads the area and the baseline.
#[allow(dead_code)]
pub(crate) struct Named {
    pub(crate) name: String,
    pub(crate) area: Option<String>,
    pub(crate) entity: Entity,
    pub(crate) baseline: BTreeMap<String, serde_json::Value>,
}

/// A World's save state, inserted at its first naming: the run's counter
/// and every named entity.
#[derive(Debug, Default)]
pub struct SaveState {
    pub(crate) next_id: u64,
    pub(crate) named: BTreeMap<SaveId, Named>,
}

impl SaveState {
    /// The ID the run's next spawn takes.
    #[must_use]
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// The entity last named `id`, live or not.
    #[must_use]
    pub fn named_entity(&self, id: SaveId) -> Option<Entity> {
        self.named.get(&id).map(|named| named.entity)
    }

    /// The name `id` was given.
    #[must_use]
    pub fn name_of(&self, id: SaveId) -> Option<&str> {
        self.named.get(&id).map(|named| named.name.as_str())
    }

    /// Takes the run's next ID; `None` once the count reaches [`AUTHORED`],
    /// which it never does.
    // A run's spawns (redo 7.9's step 2) take it.
    #[allow(dead_code)]
    pub(crate) fn take_id(&mut self) -> Option<SaveId> {
        (self.next_id < AUTHORED).then(|| {
            let id = SaveId(self.next_id);
            self.next_id += 1;
            id
        })
    }
}

/// Why [`name`] refused a naming.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum NameError {
    /// The entity is not alive.
    #[error("cannot name a dead entity `{name}`")]
    Dead {
        /// The name asked for.
        name: String,
    },
    /// The name is empty.
    #[error("a save name cannot be empty")]
    Empty,
    /// The entity already has a save identity.
    #[error("the entity already has save ID {id:?}; it cannot also be `{name}`")]
    AlreadySaved {
        /// The name asked for.
        name: String,
        /// The ID it has.
        id: SaveId,
    },
    /// A live entity holds the name.
    #[error("save name `{name}` is held by a live entity")]
    Taken {
        /// The name.
        name: String,
    },
    /// Another name hashes to the same ID.
    #[error("save name `{name}` takes the ID of `{other}`; rename one")]
    Clash {
        /// The name asked for.
        name: String,
        /// The name that has its ID.
        other: String,
    },
    /// A registered component failed to serialize.
    #[error("save name `{name}`: a component failed to serialize: {message}")]
    Serialize {
        /// The name asked for.
        name: String,
        /// Why.
        message: String,
    },
}

/// Names `entity` `name` in `area` (`None` for a global entity) and returns
/// its [`SaveId::authored`] ID: the game calls it once the entity's setup
/// from the run's start is done and before anything random or the run's
/// play touches it. The first naming inserts the World's [`SaveState`].
///
/// A name a live entity holds is refused; a dead one's is free, so an
/// entity that returns takes its name again.
///
/// # Errors
///
/// [`NameError`] for a dead entity, an empty name, an entity already saved,
/// a name a live entity holds, or one whose ID another name has.
pub fn name(
    world: &mut World,
    entity: Entity,
    area: Option<&str>,
    name: &str,
) -> Result<SaveId, NameError> {
    if name.is_empty() {
        return Err(NameError::Empty);
    }
    if !world.is_alive(entity) {
        return Err(NameError::Dead {
            name: name.to_owned(),
        });
    }
    if let Some(saved) = world.get::<Saved>(entity) {
        return Err(NameError::AlreadySaved {
            name: name.to_owned(),
            id: saved.id,
        });
    }
    let id = SaveId::authored(name);
    if let Some(named) = world
        .get_resource::<SaveState>()
        .and_then(|state| state.named.get(&id))
    {
        if named.name != name {
            return Err(NameError::Clash {
                name: name.to_owned(),
                other: named.name.clone(),
            });
        }
        if world.is_alive(named.entity) {
            return Err(NameError::Taken {
                name: name.to_owned(),
            });
        }
    }
    let baseline = baseline(world, entity).map_err(|source| NameError::Serialize {
        name: name.to_owned(),
        message: source.to_string(),
    })?;
    if !world.has_resource::<SaveState>() {
        world.insert_resource(SaveState::default());
    }
    world.resource_mut::<SaveState>().named.insert(
        id,
        Named {
            name: name.to_owned(),
            area: area.map(str::to_owned),
            entity,
            baseline,
        },
    );
    world.insert(
        entity,
        Saved {
            id,
            area: area.map(str::to_owned),
            prefab: None,
        },
    );
    Ok(id)
}

/// `entity`'s registered components, serialized: what a capture writes
/// its changes over.
fn baseline(
    world: &World,
    entity: Entity,
) -> serde_json::Result<BTreeMap<String, serde_json::Value>> {
    let Some(registry) = world.get_resource::<SaveRegistry>() else {
        return Ok(BTreeMap::new());
    };
    let mut baseline = BTreeMap::new();
    for (name, entry) in &registry.components.entries {
        if let Some(value) = entry.saved.to_value(world, Holder::Entity(entity)) {
            baseline.insert(name.clone(), value?);
        }
    }
    Ok(baseline)
}

/// Whether the World has save state: whether anything was ever named in it.
#[must_use]
pub fn saving(world: &World) -> bool {
    world.has_resource::<SaveState>()
}

/// The key a step's work sorts by while the World saves: `(false, id)` for
/// a saved entity, so saved entities go first by save ID, and
/// `(true, index)` for the rest, by entity index.
#[must_use]
pub fn order_key(world: &World, entity: Entity) -> (bool, u64) {
    match world.get::<Saved>(entity) {
        Some(saved) => (false, saved.id.0),
        None => (true, u64::from(entity.id())),
    }
}

#[cfg(test)]
#[path = "../tests/save/id.rs"]
mod tests;
