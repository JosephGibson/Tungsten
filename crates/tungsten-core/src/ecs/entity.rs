use std::fmt;

/// Generational entity handle; `id()` returns slot index.
///
/// D-036: generation rejects stale handles after slot reuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl Entity {
    /// Slot index.
    pub fn id(self) -> u32 {
        self.index
    }
}

impl fmt::Display for Entity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Display compatibility: index only.
        write!(f, "Entity({})", self.index)
    }
}

/// Entity-table slot metadata.
#[derive(Debug, Clone)]
pub(crate) struct EntityMeta {
    /// Slot generation, incremented on free.
    pub generation: u32,
    /// Archetype row for live slot.
    pub location: Option<EntityLocation>,
}

/// Entity row inside archetype storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntityLocation {
    pub archetype_id: u32,
    pub row: u32,
}

/// Free-list entity allocator.
#[derive(Debug, Default)]
pub(crate) struct Entities {
    meta: Vec<EntityMeta>,
    free: Vec<u32>,
}

impl Entities {
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate entity slot.
    pub fn alloc(&mut self) -> Entity {
        if let Some(index) = self.free.pop() {
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

    /// Free live entity; D-022 double-free panics.
    pub fn free(&mut self, entity: Entity) {
        let meta = &mut self.meta[entity.index as usize];
        debug_assert_eq!(
            meta.generation, entity.generation,
            "free called with stale entity handle"
        );
        // D-036: u32 wrap is theoretical at project scale.
        meta.generation = meta.generation.wrapping_add(1);
        meta.location = None;
        self.free.push(entity.index);
    }

    /// Live location; `None` for stale or unplaced handles.
    pub fn get(&self, entity: Entity) -> Option<EntityLocation> {
        let meta = self.meta.get(entity.index as usize)?;
        if meta.generation != entity.generation {
            return None;
        }
        meta.location
    }

    /// Set live location; D-022 dead handle panics.
    pub fn set_location(&mut self, entity: Entity, loc: EntityLocation) {
        let meta = &mut self.meta[entity.index as usize];
        assert_eq!(
            meta.generation, entity.generation,
            "set_location on dead entity {entity}"
        );
        meta.location = Some(loc);
    }

    /// Generation matches and slot has a location.
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.meta
            .get(entity.index as usize)
            .is_some_and(|m| m.generation == entity.generation && m.location.is_some())
    }

    /// Live slot count. O(1).
    pub fn live_count(&self) -> u32 {
        (self.meta.len() - self.free.len()) as u32
    }
}

#[cfg(test)]
#[path = "../tests/ecs/entity.rs"]
mod tests;
