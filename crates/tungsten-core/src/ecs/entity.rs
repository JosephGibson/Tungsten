use std::fmt;

/// Opaque entity identifier with a generational index.
///
/// `index` is the slot in the entity table; `generation` distinguishes
/// entities that reuse the same slot after despawn. `entity.id()` returns the
/// index for backward-compatible display and external bookkeeping.
///
/// D-021 (upgrade to generational IDs deferred until bugs appear) is resolved
/// by M12: the entity table is being rewritten anyway, and M13 parent/child
/// relationships will expose stale-handle aliasing bugs without it (D-036).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl Entity {
    /// Returns the slot index. Identical semantics to the pre-M12 `Entity(u32).id()`.
    pub fn id(self) -> u32 {
        self.index
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Intentionally prints only the index so that Display output is
        // identical to pre-M12 (`Entity(N)`), keeping log output unchanged.
        write!(f, "Entity({})", self.index)
    }
}

// ---------------------------------------------------------------------------
// Internal entity table
// ---------------------------------------------------------------------------

/// Per-slot bookkeeping inside the entity table.
#[derive(Debug, Clone)]
pub(crate) struct EntityMeta {
    /// Current generation for this slot. Incremented on every free.
    pub generation: u32,
    /// Where the entity currently lives in the archetype storage, or `None`
    /// if the slot is free / not yet used.
    pub location: Option<EntityLocation>,
}

/// Locates an entity's row within the archetype storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntityLocation {
    pub archetype_id: u32,
    pub row: u32,
}

/// Allocator for entity slots. Uses a free-list to recycle indices.
#[derive(Debug, Default)]
pub(crate) struct Entities {
    meta: Vec<EntityMeta>,
    free: Vec<u32>,
}

impl Entities {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a new entity. Reuses a freed slot (with a bumped generation)
    /// if one is available, otherwise grows the table.
    pub fn alloc(&mut self) -> Entity {
        if let Some(index) = self.free.pop() {
            // Slot was freed — generation was already bumped during `free`.
            let meta = &self.meta[index as usize];
            Entity {
                index,
                generation: meta.generation,
            }
        } else {
            let index = self.meta.len() as u32;
            self.meta.push(EntityMeta {
                generation: 0,
                location: None,
            });
            Entity {
                index,
                generation: 0,
            }
        }
    }

    /// Mark an entity as dead. Bumps the generation so any live handles to
    /// this slot become stale and are detected by future `get` / `is_alive`
    /// calls.
    ///
    /// Panics if the entity is already dead (double-free is a programmer
    /// error per D-022).
    pub fn free(&mut self, entity: Entity) {
        let meta = &mut self.meta[entity.index as usize];
        debug_assert_eq!(
            meta.generation, entity.generation,
            "free called with stale entity handle"
        );
        // Wrapping add: at u32::MAX the generation wraps. Noted in D-036 as a
        // theoretical limit that is not a practical concern at hobby scale.
        meta.generation = meta.generation.wrapping_add(1);
        meta.location = None;
        self.free.push(entity.index);
    }

    /// Return the current location for a live entity, or `None` if the handle
    /// is stale (generation mismatch) or the entity has no location yet.
    pub fn get(&self, entity: Entity) -> Option<EntityLocation> {
        let meta = self.meta.get(entity.index as usize)?;
        if meta.generation != entity.generation {
            return None;
        }
        meta.location
    }

    /// Update the stored location for a live entity.
    ///
    /// Panics on generation mismatch (dead entity — D-022).
    pub fn set_location(&mut self, entity: Entity, loc: EntityLocation) {
        let meta = &mut self.meta[entity.index as usize];
        assert_eq!(
            meta.generation, entity.generation,
            "set_location on dead entity {entity}"
        );
        meta.location = Some(loc);
    }

    /// Returns `true` if the entity handle is live (generation matches and
    /// the slot has not been freed).
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.meta
            .get(entity.index as usize)
            .is_some_and(|m| m.generation == entity.generation && m.location.is_some())
    }

    /// Number of currently live entity slots. O(1).
    pub fn live_count(&self) -> u32 {
        (self.meta.len() - self.free.len()) as u32
    }
}

#[cfg(test)]
#[path = "../tests/ecs/entity.rs"]
mod tests;
