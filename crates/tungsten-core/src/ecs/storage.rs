use std::any::TypeId;
use std::collections::HashMap;

use super::archetype::{AnyColumn, Archetype, ArchetypeId, EMPTY_ARCHETYPE, TypedVec};
use super::command_buffer::InsertQueues;
use super::entity::{Entities, Entity, EntityLocation};

/// Entity/component storage and archetype registry.
///
/// D-036: `ArchetypeId` indexes `archetypes`; 0 is empty archetype.
#[allow(clippy::struct_field_names)]
pub(crate) struct Archetypes {
    pub archetypes: Vec<Archetype>,
    /// Sorted `TypeId` set -> archetype.
    index: HashMap<Box<[TypeId]>, ArchetypeId>,
    pub entities: Entities,
}

impl Archetypes {
    pub fn new() -> Self {
        let empty = Archetype::new(Box::new([]), Vec::new());
        let mut index = HashMap::new();
        index.insert(Box::new([]) as Box<[TypeId]>, EMPTY_ARCHETYPE);
        Self {
            archetypes: vec![empty],
            index,
            entities: Entities::new(),
        }
    }

    /// Find the archetype for sorted `types`, or create it with `columns`:
    /// one empty column per type, in the same order.
    fn find_or_create(
        &mut self,
        types: &[TypeId],
        columns: impl FnOnce(&Self) -> Vec<Box<dyn AnyColumn>>,
    ) -> ArchetypeId {
        if let Some(&id) = self.index.get(types) {
            return id;
        }
        let id = self.archetypes.len() as ArchetypeId;
        let arch = Archetype::new(types.into(), columns(self));
        self.archetypes.push(arch);
        self.index.insert(types.into(), id);
        id
    }

    /// Archetype `from` plus `t_id`: the cached add edge, or the archetype
    /// for the extended key, created with `new_column` as `t_id`'s column.
    fn add_edge(
        &mut self,
        from: ArchetypeId,
        t_id: TypeId,
        new_column: impl FnOnce() -> Box<dyn AnyColumn>,
    ) -> ArchetypeId {
        if let Some(&cached) = self.archetypes[from as usize].add_edges.get(&t_id) {
            return cached;
        }
        let mut new_types: Vec<TypeId> = self.archetypes[from as usize].component_types.to_vec();
        new_types.push(t_id);
        new_types.sort();
        let id = self.find_or_create(&new_types, |store| {
            let mut columns: Vec<Box<dyn AnyColumn>> = store.archetypes[from as usize]
                .columns
                .iter()
                .map(|column| column.new_empty())
                .collect();
            let position = new_types.partition_point(|&tid| tid < t_id);
            columns.insert(position, new_column());
            columns
        });
        self.archetypes[from as usize].add_edges.insert(t_id, id);
        id
    }

    /// Archetype `from` minus `t_id`: the cached remove edge, or the
    /// archetype for the reduced key.
    fn remove_edge(&mut self, from: ArchetypeId, t_id: TypeId) -> ArchetypeId {
        if let Some(&cached) = self.archetypes[from as usize].remove_edges.get(&t_id) {
            return cached;
        }
        let new_types: Vec<TypeId> = self.archetypes[from as usize]
            .component_types
            .iter()
            .copied()
            .filter(|&tid| tid != t_id)
            .collect();
        let id = self.find_or_create(&new_types, |store| {
            let old = &store.archetypes[from as usize];
            old.component_types
                .iter()
                .zip(&old.columns)
                .filter(|&(&tid, _)| tid != t_id)
                .map(|(_, column)| column.new_empty())
                .collect()
        });
        self.archetypes[from as usize].remove_edges.insert(t_id, id);
        id
    }

    /// Spawn entity in empty archetype.
    pub fn spawn(&mut self) -> Entity {
        let entity = self.entities.alloc();
        let row = self.archetypes[EMPTY_ARCHETYPE as usize].entities.len() as u32;
        self.archetypes[EMPTY_ARCHETYPE as usize]
            .entities
            .push(entity);
        self.entities.set_location(
            entity,
            EntityLocation {
                archetype_id: EMPTY_ARCHETYPE,
                row,
            },
        );
        entity
    }

    /// Despawn live entity; dead entity panics.
    pub fn despawn(&mut self, entity: Entity) {
        let loc = self
            .entities
            .get(entity)
            .expect("despawn: entity is not alive");

        let arch = &mut self.archetypes[loc.archetype_id as usize];
        let displaced = arch.swap_remove_row(loc.row as usize);

        if let Some(displaced_entity) = displaced {
            self.entities.set_location(
                displaced_entity,
                EntityLocation {
                    archetype_id: loc.archetype_id,
                    row: loc.row,
                },
            );
        }

        self.entities.free(entity);
    }

    /// Insert or overwrite component; D-022 dead entity panics.
    pub fn insert<T: 'static>(&mut self, entity: Entity, value: T) {
        assert!(
            self.entities.is_alive(entity),
            "insert on dead entity {entity}"
        );

        let loc = self.entities.get(entity).unwrap();
        let old_arch_id = loc.archetype_id;
        let row = loc.row as usize;

        let t_id = TypeId::of::<T>();

        if let Some(column) = self.archetypes[old_arch_id as usize].typed_column_mut::<T>() {
            column.0[row] = value;
            return;
        }

        let new_arch_id = self.add_edge(old_arch_id, t_id, || Box::new(TypedVec::<T>(Vec::new())));

        // Borrow split: old/new archetypes must differ because `T` was absent.
        debug_assert_ne!(old_arch_id, new_arch_id);

        self.move_row(entity, old_arch_id, row, new_arch_id);
        self.archetypes[new_arch_id as usize]
            .typed_column_mut::<T>()
            .expect("insert: destination column missing")
            .0
            .push(value);
    }

    /// Apply a run of inserts on one live entity as a single archetype move
    /// (D-084), with the result of inserting the values one by one.
    ///
    /// `run` yields the queue of each insert in command order; the values
    /// are the oldest ones left in those queues. The add edges are walked in
    /// that order, so every archetype the one-by-one inserts would pass
    /// through exists afterwards, created in the same order; the
    /// intermediate ones get no row. The row then moves once, and each value
    /// is pushed or, for a type the entity already had or the run names
    /// twice, overwritten in command order.
    pub fn insert_run(
        &mut self,
        entity: Entity,
        run: impl Iterator<Item = u32> + Clone,
        values: &mut InsertQueues,
    ) {
        let loc = self
            .entities
            .get(entity)
            .unwrap_or_else(|| panic!("insert on dead entity {entity}"));
        let old_arch_id = loc.archetype_id;

        let mut new_arch_id = old_arch_id;
        for queue in run.clone() {
            let t_id = values.component_type(queue);
            if !self.archetypes[new_arch_id as usize].has(t_id) {
                new_arch_id = self.add_edge(new_arch_id, t_id, || values.new_column(queue));
            }
        }

        let row = if new_arch_id == old_arch_id {
            loc.row as usize
        } else {
            self.move_row(entity, old_arch_id, loc.row as usize, new_arch_id)
        };

        // Every type of the run is in the archetype now, so its slot holds
        // its own column, and the queue downcasts the column before writing.
        let arch = &mut self.archetypes[new_arch_id as usize];
        for queue in run {
            let column = arch
                .slot_column_erased_mut(values.component_type(queue))
                .expect("insert_run: destination column missing");
            values.write_next(queue, column, row);
        }
    }

    /// Move `entity`'s row from `old_arch_id` to another archetype and fix
    /// the locations of the entity and of the row its swap-remove displaced.
    /// Returns the new row.
    ///
    /// Only the components both archetypes hold move. The caller pushes the
    /// values the destination adds, or takes the ones it drops from `row` of
    /// their source columns.
    fn move_row(
        &mut self,
        entity: Entity,
        old_arch_id: ArchetypeId,
        row: usize,
        new_arch_id: ArchetypeId,
    ) -> usize {
        let (old_arch, new_arch) = split_two_mut(&mut self.archetypes, old_arch_id, new_arch_id);
        old_arch.move_components_to(row, new_arch);

        let new_row = new_arch.entities.len();
        new_arch.entities.push(entity);

        let last = old_arch.entities.len().saturating_sub(1);
        old_arch.entities.swap_remove(row);

        self.entities.set_location(
            entity,
            EntityLocation {
                archetype_id: new_arch_id,
                row: new_row as u32,
            },
        );

        if row < last {
            let displaced = self.archetypes[old_arch_id as usize].entities[row];
            self.entities.set_location(
                displaced,
                EntityLocation {
                    archetype_id: old_arch_id,
                    row: row as u32,
                },
            );
        }

        new_row
    }

    /// Remove component and transition to archetype without `T`.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        let loc = self.entities.get(entity)?;
        let old_arch_id = loc.archetype_id;
        let row = loc.row as usize;

        let t_id = TypeId::of::<T>();
        let t_index = self.archetypes[old_arch_id as usize].column_index(t_id)?;

        let new_arch_id = self.remove_edge(old_arch_id, t_id);

        self.move_row(entity, old_arch_id, row, new_arch_id);
        let t_value = self.archetypes[old_arch_id as usize].columns[t_index]
            .typed_mut::<T>()
            .expect("remove: column type mismatch")
            .0
            .swap_remove(row);

        Some(t_value)
    }

    /// Random access: the entity's location, its archetype's slot for `T`,
    /// one column downcast and the row (D-083).
    #[inline]
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let loc = self.entities.get(entity)?;
        let arch = &self.archetypes[loc.archetype_id as usize];
        arch.slot_column::<T>()?.0.get(loc.row as usize)
    }

    #[inline]
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let loc = self.entities.get(entity)?;
        let arch = &mut self.archetypes[loc.archetype_id as usize];
        arch.slot_column_mut::<T>()?.0.get_mut(loc.row as usize)
    }

    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        let Some(loc) = self.entities.get(entity) else {
            return false;
        };
        self.archetypes[loc.archetype_id as usize].has(TypeId::of::<T>())
    }

    /// Archetypes containing component `T`, each with `T`'s column index.
    pub fn archetypes_with<T: 'static>(&self) -> impl Iterator<Item = (&Archetype, usize)> {
        let t_id = TypeId::of::<T>();
        self.archetypes
            .iter()
            .filter_map(move |arch| Some((arch, arch.column_index(t_id)?)))
    }

    /// Archetypes containing `a` and `b`.
    pub fn archetypes_with_two(&self, a: TypeId, b: TypeId) -> impl Iterator<Item = &Archetype> {
        self.archetypes
            .iter()
            .filter(move |arch| arch.has(a) && arch.has(b))
    }

    /// Archetypes containing `a`, `b`, and `c`.
    pub fn archetypes_with_three(
        &self,
        a: TypeId,
        b: TypeId,
        c: TypeId,
    ) -> impl Iterator<Item = &Archetype> {
        self.archetypes
            .iter()
            .filter(move |arch| arch.has(a) && arch.has(b) && arch.has(c))
    }

    /// Mutable archetypes containing component `T`, each with `T`'s column
    /// index. Test-only since `World::query_mut` took query data (`D-130`).
    #[cfg(test)]
    pub fn archetypes_with_mut<T: 'static>(
        &mut self,
    ) -> impl Iterator<Item = (&mut Archetype, usize)> {
        let t_id = TypeId::of::<T>();
        self.archetypes.iter_mut().filter_map(move |arch| {
            let index = arch.column_index(t_id)?;
            Some((arch, index))
        })
    }

    /// Mutable archetypes containing `a` and `b`.
    pub fn archetypes_with_two_mut(
        &mut self,
        a: TypeId,
        b: TypeId,
    ) -> impl Iterator<Item = &mut Archetype> {
        self.archetypes
            .iter_mut()
            .filter(move |arch| arch.has(a) && arch.has(b))
    }

    /// Mutable archetypes containing `a`, `b`, and `c`.
    pub fn archetypes_with_three_mut(
        &mut self,
        a: TypeId,
        b: TypeId,
        c: TypeId,
    ) -> impl Iterator<Item = &mut Archetype> {
        self.archetypes
            .iter_mut()
            .filter(move |arch| arch.has(a) && arch.has(b) && arch.has(c))
    }
}

/// Mutable refs to two distinct archetypes.
fn split_two_mut(
    archetypes: &mut [Archetype],
    a: ArchetypeId,
    b: ArchetypeId,
) -> (&mut Archetype, &mut Archetype) {
    let (a, b) = (a as usize, b as usize);
    assert_ne!(a, b, "split_two_mut: indices must differ");
    if a < b {
        let (left, right) = archetypes.split_at_mut(b);
        (&mut left[a], &mut right[0])
    } else {
        let (left, right) = archetypes.split_at_mut(a);
        (&mut right[0], &mut left[b])
    }
}

#[cfg(test)]
#[path = "../tests/ecs/storage.rs"]
mod tests;
