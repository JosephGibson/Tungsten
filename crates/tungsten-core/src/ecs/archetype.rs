use std::any::{Any, TypeId};
use std::collections::HashMap;

use super::entity::Entity;

/// Type-erased `Vec<T>` column interface.
///
/// Query cost: one downcast per archetype/type, then contiguous `Vec<T>` access.
#[allow(dead_code)]
pub(crate) trait AnyColumn: Any {
    fn push_erased(&mut self, val: Box<dyn Any>);
    fn swap_remove_erased(&mut self, row: usize) -> Box<dyn Any>;
    fn get_erased(&self, row: usize) -> &dyn Any;
    fn get_mut_erased(&mut self, row: usize) -> &mut dyn Any;
    fn len(&self) -> usize;
    fn type_id(&self) -> TypeId;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    /// Empty column of same concrete type.
    fn new_empty(&self) -> Box<dyn AnyColumn>;
}

/// Typed component column.
pub(crate) struct TypedVec<T: 'static>(pub Vec<T>);

impl<T: 'static> AnyColumn for TypedVec<T> {
    fn push_erased(&mut self, val: Box<dyn Any>) {
        self.0
            .push(*val.downcast::<T>().expect("push_erased: type mismatch"));
    }

    fn swap_remove_erased(&mut self, row: usize) -> Box<dyn Any> {
        Box::new(self.0.swap_remove(row))
    }

    fn get_erased(&self, row: usize) -> &dyn Any {
        &self.0[row]
    }

    fn get_mut_erased(&mut self, row: usize) -> &mut dyn Any {
        &mut self.0[row]
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn type_id(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn new_empty(&self) -> Box<dyn AnyColumn> {
        Box::new(TypedVec::<T>(Vec::new()))
    }
}

/// Identifies an archetype in the `Archetypes` registry.
pub(crate) type ArchetypeId = u32;

/// Fresh-spawn archetype.
pub(crate) const EMPTY_ARCHETYPE: ArchetypeId = 0;

/// Entities sharing one component set.
///
/// Invariant: every column and `entities` have equal length; rows compact via swap-remove.
pub(crate) struct Archetype {
    #[allow(dead_code)]
    pub id: ArchetypeId,
    /// Sorted component type key.
    pub component_types: Box<[TypeId]>,
    /// Columns allocated lazily on first transition into archetype.
    pub columns: HashMap<TypeId, Box<dyn AnyColumn>>,
    /// Entity per row.
    pub entities: Vec<Entity>,
    /// Lazy add-edge cache.
    pub add_edges: HashMap<TypeId, ArchetypeId>,
    /// Lazy remove-edge cache.
    pub remove_edges: HashMap<TypeId, ArchetypeId>,
}

impl Archetype {
    pub fn new(id: ArchetypeId, component_types: Box<[TypeId]>) -> Self {
        Self {
            id,
            component_types,
            columns: HashMap::new(),
            entities: Vec::new(),
            add_edges: HashMap::new(),
            remove_edges: HashMap::new(),
        }
    }

    pub fn has(&self, type_id: TypeId) -> bool {
        self.component_types.contains(&type_id)
    }

    #[allow(dead_code)]
    pub fn row_count(&self) -> usize {
        self.entities.len()
    }

    /// Swap-remove row from every column; caller updates displaced location.
    pub fn swap_remove_row(&mut self, row: usize) -> Option<Entity> {
        let last = self.entities.len().saturating_sub(1);
        for col in self.columns.values_mut() {
            col.swap_remove_erased(row);
        }
        self.entities.swap_remove(row);
        if row < last {
            Some(self.entities[row])
        } else {
            None
        }
    }

    /// Move row component data into `dest`; caller handles entity rows/locations.
    pub fn move_components_to(&mut self, row: usize, dest: &mut Archetype) {
        let types_to_move: Vec<TypeId> = self
            .component_types
            .iter()
            .copied()
            .filter(|tid| dest.component_types.contains(tid))
            .collect();

        // Create destination columns before source swap-remove.
        for &tid in &types_to_move {
            dest.columns.entry(tid).or_insert_with(|| {
                self.columns
                    .get(&tid)
                    .expect("move_components_to: source column missing for new_empty")
                    .new_empty()
            });
        }

        for tid in types_to_move {
            let val = self
                .columns
                .get_mut(&tid)
                .expect("move_components_to: source column missing")
                .swap_remove_erased(row);
            dest.columns
                .get_mut(&tid)
                .expect("move_components_to: dest column missing after creation")
                .push_erased(val);
        }
    }
}

#[cfg(test)]
#[path = "../tests/ecs/archetype.rs"]
mod tests;
