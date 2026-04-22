use std::any::TypeId;

use super::archetype::TypedVec;
use super::command_buffer::{Command, CommandBuffer, CommandTarget};
use super::entity::Entity;
use super::resource::ResourceMap;
use super::storage::Archetypes;

/// The World owns all entities, components, and resources.
///
/// The public API is identical to the pre-M12 naive implementation so that
/// all examples compile without changes. The storage engine behind it is now
/// an archetypal layout: contiguous `Vec<T>` columns per archetype, with O(1)
/// edge-cached archetype transitions on component add/remove (D-036).
pub struct World {
    archetypes: Archetypes,
    resources: ResourceMap,
}

impl World {
    pub fn new() -> Self {
        Self {
            archetypes: Archetypes::new(),
            resources: ResourceMap::new(),
        }
    }

    // ------------------------------------------------------------------
    // Entity lifecycle
    // ------------------------------------------------------------------

    /// Spawn a new entity. Starts in the empty archetype with no components.
    pub fn spawn(&mut self) -> Entity {
        self.archetypes.spawn()
    }

    /// Despawn an entity, removing all of its components.
    ///
    /// No-op if the entity is already dead, so that deferred `CommandBuffer`
    /// despawns remain idempotent when multiple systems target the same
    /// entity in a single frame.
    pub fn despawn(&mut self, entity: Entity) {
        if self.is_alive(entity) {
            self.archetypes.despawn(entity);
        }
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.archetypes.entities.is_alive(entity)
    }

    /// Number of currently live entities in the world. O(1).
    pub fn entity_count(&self) -> u32 {
        self.archetypes.entities.live_count()
    }

    // ------------------------------------------------------------------
    // Component access
    // ------------------------------------------------------------------

    /// Attach a component to an entity. Panics if the entity is not alive
    /// (programmer error per D-022).
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        self.archetypes.insert(entity, component);
    }

    pub fn remove_component<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        self.archetypes.remove::<T>(entity)
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.archetypes.get::<T>(entity)
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.archetypes.get_mut::<T>(entity)
    }

    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        self.archetypes.has::<T>(entity)
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Iterate all entities that have component type `T`.
    /// Returns `(Entity, &T)` pairs.
    ///
    /// Iteration order is archetype-stable: entities within an archetype are
    /// visited in row order (stable until a swap-remove occurs). Across
    /// archetypes the order follows the archetype creation order.
    pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.archetypes.archetypes_with::<T>().flat_map(|arch| {
            let t_id = TypeId::of::<T>();
            let col = arch.columns[&t_id]
                .as_any()
                .downcast_ref::<TypedVec<T>>()
                .unwrap();
            arch.entities.iter().zip(col.0.iter()).map(|(&e, v)| (e, v))
        })
    }

    /// Collect entity IDs that have component type `T`.
    ///
    /// Useful when you need to mutate components of multiple types in the
    /// same loop (collect entity IDs first, then call `get_mut` per entity).
    pub fn query_entities<T: 'static>(&self) -> Vec<Entity> {
        self.archetypes
            .archetypes_with::<T>()
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    /// Immutable 2-component query. Iterates all entities that have both `A`
    /// and `B`, visiting each archetype's contiguous column data directly.
    ///
    /// One downcast per archetype per type — not per element. This is the
    /// primary cache-friendly iteration path added in M12 (D-036 Expansion 1).
    pub fn query2<A: 'static, B: 'static>(&self) -> impl Iterator<Item = (Entity, &A, &B)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        self.archetypes
            .archetypes_with_two(a_id, b_id)
            .flat_map(move |arch| {
                let col_a = arch.columns[&a_id]
                    .as_any()
                    .downcast_ref::<TypedVec<A>>()
                    .unwrap();
                let col_b = arch.columns[&b_id]
                    .as_any()
                    .downcast_ref::<TypedVec<B>>()
                    .unwrap();
                arch.entities
                    .iter()
                    .zip(col_a.0.iter())
                    .zip(col_b.0.iter())
                    .map(|((&e, a), b)| (e, a, b))
            })
    }

    /// Collect entity IDs that have both `A` and `B`.
    ///
    /// Use this with `get_mut` when mutation of either type is needed.
    pub fn query2_entities<A: 'static, B: 'static>(&self) -> Vec<Entity> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        self.archetypes
            .archetypes_with_two(a_id, b_id)
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    /// Immutable 3-component query. Iterates all entities that have `A`, `B`,
    /// and `C`.
    pub fn query3<A: 'static, B: 'static, C: 'static>(
        &self,
    ) -> impl Iterator<Item = (Entity, &A, &B, &C)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        self.archetypes
            .archetypes_with_three(a_id, b_id, c_id)
            .flat_map(move |arch| {
                let col_a = arch.columns[&a_id]
                    .as_any()
                    .downcast_ref::<TypedVec<A>>()
                    .unwrap();
                let col_b = arch.columns[&b_id]
                    .as_any()
                    .downcast_ref::<TypedVec<B>>()
                    .unwrap();
                let col_c = arch.columns[&c_id]
                    .as_any()
                    .downcast_ref::<TypedVec<C>>()
                    .unwrap();
                arch.entities
                    .iter()
                    .zip(col_a.0.iter())
                    .zip(col_b.0.iter())
                    .zip(col_c.0.iter())
                    .map(|(((&e, a), b), c)| (e, a, b, c))
            })
    }

    /// Collect entity IDs that have `A`, `B`, and `C`.
    ///
    /// Use this with `get_mut` when mutation is needed.
    pub fn query3_entities<A: 'static, B: 'static, C: 'static>(&self) -> Vec<Entity> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        self.archetypes
            .archetypes_with_three(a_id, b_id, c_id)
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    // ------------------------------------------------------------------
    // Resources
    // ------------------------------------------------------------------

    pub fn insert_resource<T: 'static>(&mut self, resource: T) {
        self.resources.insert(resource);
    }

    pub fn get_resource<T: 'static>(&self) -> Option<&T> {
        self.resources.get::<T>()
    }

    pub fn get_resource_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.resources.get_mut::<T>()
    }

    pub fn has_resource<T: 'static>(&self) -> bool {
        self.resources.contains::<T>()
    }

    pub fn remove_resource<T: 'static>(&mut self) -> Option<T> {
        self.resources.remove::<T>()
    }

    /// Apply all queued commands in `buffer` to the world, then drop it.
    ///
    /// Pass 1 allocates real entities for every queued spawn, building a
    /// `pending_id -> Entity` table. Pass 2 replays all mutations in their
    /// original registration order.
    pub fn flush(&mut self, buffer: CommandBuffer) {
        let mut pending_entities: Vec<Entity> = Vec::with_capacity(buffer.pending_count as usize);
        for cmd in &buffer.commands {
            if let Command::Spawn { pending_id } = cmd {
                debug_assert_eq!(
                    *pending_id as usize,
                    pending_entities.len(),
                    "pending_id must be allocated sequentially"
                );
                pending_entities.push(self.spawn());
            }
        }

        for cmd in buffer.commands {
            match cmd {
                Command::Spawn { .. } => {}
                Command::Insert { target, setter } => {
                    let entity = match target {
                        CommandTarget::Live(entity) => {
                            if !self.is_alive(entity) {
                                continue;
                            }
                            entity
                        }
                        CommandTarget::Pending(id) => pending_entities[id as usize],
                    };
                    setter.apply(self, entity);
                }
                Command::Remove(remove) => remove(self),
                Command::Despawn(entity) => {
                    if self.is_alive(entity) {
                        self.despawn(entity);
                    }
                }
            }
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests — the 9 original pre-M12 tests, byte-for-byte unchanged

#[cfg(test)]
#[path = "../tests/ecs/world.rs"]
mod tests;
