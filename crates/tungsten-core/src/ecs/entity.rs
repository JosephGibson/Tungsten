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
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_gives_distinct_entities() {
        let mut e = Entities::new();
        let a = e.alloc();
        let b = e.alloc();
        assert_ne!(a, b);
        assert_eq!(a.index, 0);
        assert_eq!(b.index, 1);
    }

    #[test]
    fn alloc_free_reuses_index_with_bumped_generation() {
        let mut e = Entities::new();
        let a = e.alloc();
        e.set_location(
            a,
            EntityLocation {
                archetype_id: 0,
                row: 0,
            },
        );
        e.free(a);

        let b = e.alloc();
        assert_eq!(b.index, a.index, "slot should be reused");
        assert_eq!(b.generation, a.generation + 1, "generation must be bumped");
    }

    #[test]
    fn stale_handle_returns_none() {
        let mut e = Entities::new();
        let a = e.alloc();
        e.set_location(
            a,
            EntityLocation {
                archetype_id: 0,
                row: 0,
            },
        );
        e.free(a);
        let _b = e.alloc(); // reuses the slot

        // Original handle has old generation — should not resolve.
        assert!(e.get(a).is_none());
        assert!(!e.is_alive(a));
    }

    #[test]
    fn is_alive_false_after_free() {
        let mut e = Entities::new();
        let a = e.alloc();
        e.set_location(
            a,
            EntityLocation {
                archetype_id: 0,
                row: 0,
            },
        );
        assert!(e.is_alive(a));
        e.free(a);
        assert!(!e.is_alive(a));
    }

    #[test]
    fn is_alive_false_before_location_set() {
        let mut e = Entities::new();
        let a = e.alloc();
        // Freshly allocated slot has no location yet.
        assert!(!e.is_alive(a));
    }

    #[test]
    fn set_and_get_location() {
        let mut e = Entities::new();
        let a = e.alloc();
        let loc = EntityLocation {
            archetype_id: 3,
            row: 7,
        };
        e.set_location(a, loc);
        assert_eq!(e.get(a), Some(loc));
        assert!(e.is_alive(a));
    }

    #[test]
    fn free_list_order_is_lifo() {
        let mut e = Entities::new();
        let a = e.alloc();
        let b = e.alloc();
        e.set_location(
            a,
            EntityLocation {
                archetype_id: 0,
                row: 0,
            },
        );
        e.set_location(
            b,
            EntityLocation {
                archetype_id: 0,
                row: 1,
            },
        );
        e.free(b);
        e.free(a);

        let c = e.alloc(); // should reuse a's slot (LIFO)
        assert_eq!(c.index, a.index);
        assert_eq!(c.generation, 1);
    }

    #[test]
    fn entity_id_returns_index() {
        let e = Entity {
            index: 42,
            generation: 7,
        };
        assert_eq!(e.id(), 42);
    }

    #[test]
    fn entity_display_matches_pre_m12() {
        let e = Entity {
            index: 5,
            generation: 3,
        };
        assert_eq!(format!("{e}"), "Entity(5)");
    }
}
