use std::any::TypeId;
use std::collections::HashMap;

use super::archetype::{Archetype, ArchetypeId, TypedVec, EMPTY_ARCHETYPE};
use super::entity::{Entities, Entity, EntityLocation};

/// Entity/component storage and archetype registry.
///
/// D-036: `ArchetypeId` indexes `archetypes`; 0 is empty archetype.
pub(crate) struct Archetypes {
    pub archetypes: Vec<Archetype>,
    /// Sorted `TypeId` set -> archetype.
    index: HashMap<Box<[TypeId]>, ArchetypeId>,
    pub entities: Entities,
}

impl Archetypes {
    pub fn new() -> Self {
        let empty = Archetype::new(EMPTY_ARCHETYPE, Box::new([]));
        let mut index = HashMap::new();
        index.insert(Box::new([]) as Box<[TypeId]>, EMPTY_ARCHETYPE);
        Self {
            archetypes: vec![empty],
            index,
            entities: Entities::new(),
        }
    }

    /// Find or create archetype for sorted `types`.
    pub fn find_or_create(&mut self, types: &[TypeId]) -> ArchetypeId {
        if let Some(&id) = self.index.get(types) {
            return id;
        }
        let id = self.archetypes.len() as ArchetypeId;
        let arch = Archetype::new(id, types.into());
        self.archetypes.push(arch);
        self.index.insert(types.into(), id);
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

        if self.archetypes[old_arch_id as usize].has(t_id) {
            *self.archetypes[old_arch_id as usize]
                .columns
                .get_mut(&t_id)
                .unwrap()
                .get_mut_erased(row)
                .downcast_mut::<T>()
                .unwrap() = value;
            return;
        }

        let new_arch_id = {
            if let Some(&cached) = self.archetypes[old_arch_id as usize].add_edges.get(&t_id) {
                cached
            } else {
                let mut new_types: Vec<TypeId> = self.archetypes[old_arch_id as usize]
                    .component_types
                    .to_vec();
                new_types.push(t_id);
                new_types.sort();
                let id = self.find_or_create(&new_types);
                self.archetypes[old_arch_id as usize]
                    .add_edges
                    .insert(t_id, id);
                id
            }
        };

        // Borrow split: old/new archetypes must differ because `T` was absent.
        debug_assert_ne!(old_arch_id, new_arch_id);

        let (old_arch, new_arch) = split_two_mut(&mut self.archetypes, old_arch_id, new_arch_id);
        old_arch.move_components_to(row, new_arch);

        new_arch
            .columns
            .entry(t_id)
            .or_insert_with(|| Box::new(TypedVec::<T>(Vec::new())))
            .push_erased(Box::new(value));

        let new_row = new_arch.entities.len() as u32;
        new_arch.entities.push(entity);

        let last = self.archetypes[old_arch_id as usize]
            .entities
            .len()
            .saturating_sub(1);
        self.archetypes[old_arch_id as usize]
            .entities
            .swap_remove(row);

        self.entities.set_location(
            entity,
            EntityLocation {
                archetype_id: new_arch_id,
                row: new_row,
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
    }

    /// Remove component and transition to archetype without `T`.
    pub fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        let loc = self.entities.get(entity)?;
        let old_arch_id = loc.archetype_id;
        let row = loc.row as usize;

        let t_id = TypeId::of::<T>();
        if !self.archetypes[old_arch_id as usize].has(t_id) {
            return None;
        }

        let new_arch_id = {
            if let Some(&cached) = self.archetypes[old_arch_id as usize]
                .remove_edges
                .get(&t_id)
            {
                cached
            } else {
                let new_types: Vec<TypeId> = self.archetypes[old_arch_id as usize]
                    .component_types
                    .iter()
                    .copied()
                    .filter(|&tid| tid != t_id)
                    .collect();
                let id = self.find_or_create(&new_types);
                self.archetypes[old_arch_id as usize]
                    .remove_edges
                    .insert(t_id, id);
                id
            }
        };

        let (old_arch, new_arch) = split_two_mut(&mut self.archetypes, old_arch_id, new_arch_id);
        old_arch.move_components_to(row, new_arch);

        let t_boxed = self.archetypes[old_arch_id as usize]
            .columns
            .get_mut(&t_id)
            .unwrap()
            .swap_remove_erased(row);
        let t_value = *t_boxed.downcast::<T>().unwrap();

        let new_row = self.archetypes[new_arch_id as usize].entities.len() as u32;
        self.archetypes[new_arch_id as usize].entities.push(entity);

        let last = self.archetypes[old_arch_id as usize]
            .entities
            .len()
            .saturating_sub(1);
        self.archetypes[old_arch_id as usize]
            .entities
            .swap_remove(row);

        self.entities.set_location(
            entity,
            EntityLocation {
                archetype_id: new_arch_id,
                row: new_row,
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

        Some(t_value)
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        let loc = self.entities.get(entity)?;
        let arch = &self.archetypes[loc.archetype_id as usize];
        arch.columns
            .get(&TypeId::of::<T>())?
            .get_erased(loc.row as usize)
            .downcast_ref::<T>()
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        let loc = self.entities.get(entity)?;
        let arch = &mut self.archetypes[loc.archetype_id as usize];
        arch.columns
            .get_mut(&TypeId::of::<T>())?
            .get_mut_erased(loc.row as usize)
            .downcast_mut::<T>()
    }

    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        let Some(loc) = self.entities.get(entity) else {
            return false;
        };
        self.archetypes[loc.archetype_id as usize].has(TypeId::of::<T>())
    }

    /// Archetypes containing component `T`.
    pub fn archetypes_with<T: 'static>(&self) -> impl Iterator<Item = &Archetype> {
        let t_id = TypeId::of::<T>();
        self.archetypes.iter().filter(move |a| a.has(t_id))
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
