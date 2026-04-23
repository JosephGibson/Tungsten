use std::any::TypeId;

use super::archetype::TypedVec;
use super::command_buffer::{Command, CommandBuffer, CommandTarget};
use super::entity::Entity;
use super::resource::ResourceMap;
use super::storage::Archetypes;

/// Entity/component/resource container.
///
/// D-036: archetypal storage with contiguous columns and edge-cached transitions.
pub struct World {
    archetypes: Archetypes,
    resources: ResourceMap,
}

impl World {
    #[must_use]
    pub fn new() -> Self {
        Self {
            archetypes: Archetypes::new(),
            resources: ResourceMap::new(),
        }
    }

    /// Spawn entity in empty archetype.
    pub fn spawn(&mut self) -> Entity {
        self.archetypes.spawn()
    }

    /// Despawn entity; dead entity is no-op for deferred idempotence.
    pub fn despawn(&mut self, entity: Entity) {
        if self.is_alive(entity) {
            self.archetypes.despawn(entity);
        }
    }

    #[must_use]
    pub fn is_alive(&self, entity: Entity) -> bool {
        self.archetypes.entities.is_alive(entity)
    }

    /// Live entity count. O(1).
    #[must_use]
    pub fn entity_count(&self) -> u32 {
        self.archetypes.entities.live_count()
    }

    /// Attach component; D-022 dead entity panics.
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        self.archetypes.insert(entity, component);
    }

    pub fn remove_component<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        self.archetypes.remove::<T>(entity)
    }

    #[must_use]
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.archetypes.get::<T>(entity)
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.archetypes.get_mut::<T>(entity)
    }

    #[must_use]
    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        self.archetypes.has::<T>(entity)
    }

    /// Iterate `(Entity, &T)` in archetype/row order.
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

    /// Collect entities with `T`; use before mixed mutable access.
    #[must_use]
    pub fn query_entities<T: 'static>(&self) -> Vec<Entity> {
        self.archetypes
            .archetypes_with::<T>()
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    /// Immutable two-component query; one downcast per archetype/type.
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

    /// Collect entities with `A` and `B`; use before mutable access.
    #[must_use]
    pub fn query2_entities<A: 'static, B: 'static>(&self) -> Vec<Entity> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        self.archetypes
            .archetypes_with_two(a_id, b_id)
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    /// Immutable three-component query.
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

    /// Collect entities with `A`, `B`, and `C`; use before mutable access.
    #[must_use]
    pub fn query3_entities<A: 'static, B: 'static, C: 'static>(&self) -> Vec<Entity> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        self.archetypes
            .archetypes_with_three(a_id, b_id, c_id)
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    pub fn insert_resource<T: 'static>(&mut self, resource: T) {
        self.resources.insert(resource);
    }

    #[must_use]
    pub fn get_resource<T: 'static>(&self) -> Option<&T> {
        self.resources.get::<T>()
    }

    pub fn get_resource_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.resources.get_mut::<T>()
    }

    #[must_use]
    pub fn has_resource<T: 'static>(&self) -> bool {
        self.resources.contains::<T>()
    }

    pub fn remove_resource<T: 'static>(&mut self) -> Option<T> {
        self.resources.remove::<T>()
    }

    /// Flush queued commands: allocate pending spawns, then replay mutations in order.
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

#[cfg(test)]
#[path = "../tests/ecs/world.rs"]
mod tests;
