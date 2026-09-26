use std::any::TypeId;
use std::collections::HashMap;

use super::archetype::{AnyColumn, Archetype, TypedVec};
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

    /// Immutable two-required + two-optional component query. Optional
    /// column presence resolves once per archetype — a columnar pass with no
    /// per-entity lookups — so rows in archetypes lacking `C` or `D` yield
    /// `None` at zero cost. Iterates the same archetype set in the same
    /// order as `query2::<A, B>`.
    pub fn query2_opt2<A: 'static, B: 'static, C: 'static, D: 'static>(
        &self,
    ) -> impl Iterator<Item = (Entity, &A, &B, Option<&C>, Option<&D>)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        let d_id = TypeId::of::<D>();
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
                let col_c = arch
                    .columns
                    .get(&c_id)
                    .map(|col| col.as_any().downcast_ref::<TypedVec<C>>().unwrap());
                let col_d = arch
                    .columns
                    .get(&d_id)
                    .map(|col| col.as_any().downcast_ref::<TypedVec<D>>().unwrap());
                arch.entities
                    .iter()
                    .zip(col_a.0.iter())
                    .zip(col_b.0.iter())
                    .zip(OptionalColumn(col_c.map(|col| col.0.iter())))
                    .zip(OptionalColumn(col_d.map(|col| col.0.iter())))
                    .map(|((((&e, a), b), c), d)| (e, a, b, c, d))
            })
    }

    /// Mutable counterpart of [`query2_opt2`](Self::query2_opt2) in the
    /// read-filter/write-state shape: `A` and optional `C` are shared reads,
    /// `B` and optional `D` are mutable. Iterates the same archetype set in
    /// the same order as `query2_opt2::<A, B, C, D>`, which makes zipping the
    /// two by row sound within a frame without structural changes.
    ///
    /// # Panics
    /// Panics if any two of `A`, `B`, `C`, `D` are the same type.
    pub fn query2_opt2_mut<A: 'static, B: 'static, C: 'static, D: 'static>(
        &mut self,
    ) -> impl Iterator<Item = (Entity, &A, &mut B, Option<&C>, Option<&mut D>)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        let d_id = TypeId::of::<D>();
        let ids = [a_id, b_id, c_id, d_id];
        for i in 0..ids.len() {
            for j in i + 1..ids.len() {
                assert_ne!(
                    ids[i], ids[j],
                    "query2_opt2_mut: component types must be distinct"
                );
            }
        }
        self.archetypes
            .archetypes_with_two_mut(a_id, b_id)
            .flat_map(move |arch| {
                let Archetype {
                    columns, entities, ..
                } = arch;
                let (col_a, col_b, col_c, col_d) =
                    split2_opt2_columns_mut::<A, B, C, D>(columns, a_id, b_id, c_id, d_id);
                entities
                    .iter()
                    .zip(col_a.0.iter())
                    .zip(col_b.0.iter_mut())
                    .zip(OptionalColumn(col_c.map(|col| col.0.iter())))
                    .zip(OptionalColumn(col_d.map(|col| col.0.iter_mut())))
                    .map(|((((&e, a), b), c), d)| (e, a, b, c, d))
            })
    }

    /// Iterate `(Entity, &mut T)` in archetype/row order.
    pub fn query_mut<T: 'static>(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        let t_id = TypeId::of::<T>();
        self.archetypes
            .archetypes_with_mut::<T>()
            .flat_map(move |arch| {
                let Archetype {
                    columns, entities, ..
                } = arch;
                let col = columns
                    .get_mut(&t_id)
                    .unwrap()
                    .as_any_mut()
                    .downcast_mut::<TypedVec<T>>()
                    .unwrap();
                entities.iter().zip(col.0.iter_mut()).map(|(&e, v)| (e, v))
            })
    }

    /// Mutable two-component query; per-archetype split column borrows (D-036).
    ///
    /// Both refs are mutable; distinct `TypeId`s guarantee disjoint columns.
    ///
    /// # Panics
    /// Panics if `A` and `B` are the same type.
    pub fn query2_mut<A: 'static, B: 'static>(
        &mut self,
    ) -> impl Iterator<Item = (Entity, &mut A, &mut B)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        assert_ne!(a_id, b_id, "query2_mut: component types must be distinct");
        self.archetypes
            .archetypes_with_two_mut(a_id, b_id)
            .flat_map(move |arch| {
                let Archetype {
                    columns, entities, ..
                } = arch;
                let (col_a, col_b) = split2_columns_mut::<A, B>(columns, a_id, b_id);
                entities
                    .iter()
                    .zip(col_a.0.iter_mut())
                    .zip(col_b.0.iter_mut())
                    .map(|((&e, a), b)| (e, a, b))
            })
    }

    /// Mutable three-component query; per-archetype split column borrows (D-036).
    ///
    /// All refs are mutable; distinct `TypeId`s guarantee disjoint columns.
    ///
    /// # Panics
    /// Panics if any two of `A`, `B`, `C` are the same type.
    pub fn query3_mut<A: 'static, B: 'static, C: 'static>(
        &mut self,
    ) -> impl Iterator<Item = (Entity, &mut A, &mut B, &mut C)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        assert_ne!(a_id, b_id, "query3_mut: component types must be distinct");
        assert_ne!(a_id, c_id, "query3_mut: component types must be distinct");
        assert_ne!(b_id, c_id, "query3_mut: component types must be distinct");
        self.archetypes
            .archetypes_with_three_mut(a_id, b_id, c_id)
            .flat_map(move |arch| {
                let Archetype {
                    columns, entities, ..
                } = arch;
                let (col_a, col_b, col_c) =
                    split3_columns_mut::<A, B, C>(columns, a_id, b_id, c_id);
                entities
                    .iter()
                    .zip(col_a.0.iter_mut())
                    .zip(col_b.0.iter_mut())
                    .zip(col_c.0.iter_mut())
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

/// Zip adapter for optional columns: yields `Some(item)` per row when the
/// column exists and `None` forever when it does not; the required-column
/// zip bounds the iteration either way.
struct OptionalColumn<I>(Option<I>);

impl<I: Iterator> Iterator for OptionalColumn<I> {
    type Item = Option<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.as_mut() {
            Some(iter) => iter.next().map(Some),
            None => Some(None),
        }
    }
}

/// Column borrows returned by `split2_opt2_columns_mut`: shared `A`/`C`,
/// mutable `B`/`D`, with `C`/`D` optional.
type SplitOpt2Columns<'c, A, B, C, D> = (
    &'c TypedVec<A>,
    &'c mut TypedVec<B>,
    Option<&'c TypedVec<C>>,
    Option<&'c mut TypedVec<D>>,
);

/// Column borrows for `query2_opt2_mut`: shared `A`/`C`, mutable `B`/`D`;
/// `C`/`D` may be absent from the archetype. Ids must all differ.
fn split2_opt2_columns_mut<A: 'static, B: 'static, C: 'static, D: 'static>(
    columns: &mut HashMap<TypeId, Box<dyn AnyColumn>>,
    a_id: TypeId,
    b_id: TypeId,
    c_id: TypeId,
    d_id: TypeId,
) -> SplitOpt2Columns<'_, A, B, C, D> {
    let mut col_a = None;
    let mut col_b = None;
    let mut col_c = None;
    let mut col_d = None;
    for (&tid, col) in columns.iter_mut() {
        if tid == a_id {
            col_a = (**col).as_any().downcast_ref::<TypedVec<A>>();
        } else if tid == b_id {
            col_b = col.as_any_mut().downcast_mut::<TypedVec<B>>();
        } else if tid == c_id {
            col_c = (**col).as_any().downcast_ref::<TypedVec<C>>();
        } else if tid == d_id {
            col_d = col.as_any_mut().downcast_mut::<TypedVec<D>>();
        }
    }
    (
        col_a.expect("split2_opt2_columns_mut: column A missing"),
        col_b.expect("split2_opt2_columns_mut: column B missing"),
        col_c,
        col_d,
    )
}

/// Disjoint mutable borrows of two typed columns; ids must differ.
fn split2_columns_mut<A: 'static, B: 'static>(
    columns: &mut HashMap<TypeId, Box<dyn AnyColumn>>,
    a_id: TypeId,
    b_id: TypeId,
) -> (&mut TypedVec<A>, &mut TypedVec<B>) {
    let mut col_a = None;
    let mut col_b = None;
    for (&tid, col) in columns.iter_mut() {
        if tid == a_id {
            col_a = col.as_any_mut().downcast_mut::<TypedVec<A>>();
        } else if tid == b_id {
            col_b = col.as_any_mut().downcast_mut::<TypedVec<B>>();
        }
    }
    (
        col_a.expect("split2_columns_mut: column A missing"),
        col_b.expect("split2_columns_mut: column B missing"),
    )
}

/// Disjoint mutable borrows of three typed columns; ids must differ.
fn split3_columns_mut<A: 'static, B: 'static, C: 'static>(
    columns: &mut HashMap<TypeId, Box<dyn AnyColumn>>,
    a_id: TypeId,
    b_id: TypeId,
    c_id: TypeId,
) -> (&mut TypedVec<A>, &mut TypedVec<B>, &mut TypedVec<C>) {
    let mut col_a = None;
    let mut col_b = None;
    let mut col_c = None;
    for (&tid, col) in columns.iter_mut() {
        if tid == a_id {
            col_a = col.as_any_mut().downcast_mut::<TypedVec<A>>();
        } else if tid == b_id {
            col_b = col.as_any_mut().downcast_mut::<TypedVec<B>>();
        } else if tid == c_id {
            col_c = col.as_any_mut().downcast_mut::<TypedVec<C>>();
        }
    }
    (
        col_a.expect("split3_columns_mut: column A missing"),
        col_b.expect("split3_columns_mut: column B missing"),
        col_c.expect("split3_columns_mut: column C missing"),
    )
}

#[cfg(test)]
#[path = "../tests/ecs/world.rs"]
mod tests;
