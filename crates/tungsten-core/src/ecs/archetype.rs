use std::any::{Any, TypeId};
use std::collections::HashMap;

use super::entity::Entity;

// ---------------------------------------------------------------------------
// Type-erased column trait
// ---------------------------------------------------------------------------

/// Type-erased interface for a `Vec<T>` column inside an [`Archetype`].
///
/// Every method operates on heap-boxed `dyn Any` values so the archetype
/// transition logic in `storage.rs` can move components between archetypes
/// without knowing the concrete type. The one-downcast-per-archetype pattern
/// used by the query iterators (`as_any().downcast_ref::<TypedVec<T>>()`) pays
/// the type-erasure cost once, then accesses elements via `col.0[i]` over a
/// contiguous `Vec<T>` — the cache-friendly win vs. the old
/// `HashMap<u32, Box<dyn Any>>` layout.
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
    /// Create a new, empty column of the same concrete type.
    ///
    /// Used by `Archetype::move_components_to` when the destination archetype
    /// has not yet allocated a column for this type (first-time transition).
    fn new_empty(&self) -> Box<dyn AnyColumn>;
}

// ---------------------------------------------------------------------------
// Concrete column implementation
// ---------------------------------------------------------------------------

/// A concrete, typed column: just a `Vec<T>` with type-erased access.
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

// ---------------------------------------------------------------------------
// Archetype table
// ---------------------------------------------------------------------------

/// Identifies an archetype in the `Archetypes` registry.
pub(crate) type ArchetypeId = u32;

/// The archetype that freshly-spawned entities start in (zero components).
pub(crate) const EMPTY_ARCHETYPE: ArchetypeId = 0;

/// A table of entities that all share exactly the same component set.
///
/// Each component type occupies one column (`TypedVec<T>` behind a
/// `Box<dyn AnyColumn>`). Every column and `entities` always have the same
/// length. Rows are kept compact via swap-remove: removing row `r` moves the
/// last row to `r` in every column and in `entities` simultaneously.
pub(crate) struct Archetype {
    #[allow(dead_code)]
    pub id: ArchetypeId,
    /// Sorted list of `TypeId`s that uniquely identifies this archetype.
    pub component_types: Box<[TypeId]>,
    /// One column per component type. Populated lazily on first use — a newly
    /// created archetype starts with an empty map; columns are added during
    /// the first entity transition into this archetype.
    pub columns: HashMap<TypeId, Box<dyn AnyColumn>>,
    /// The entity at each row. Always the same length as every column.
    pub entities: Vec<Entity>,
    /// Lazy add-edges: `TypeId` → target `ArchetypeId` after inserting that type.
    pub add_edges: HashMap<TypeId, ArchetypeId>,
    /// Lazy remove-edges: `TypeId` → target `ArchetypeId` after removing that type.
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

    /// Swap-remove the row at `row` from **all columns and the entity list**.
    ///
    /// Returns the entity that was displaced from the last position into `row`
    /// (i.e., the entity that was at `self.entities[old_last]`), or `None` if
    /// the removed row was already the last row.
    ///
    /// The caller is responsible for updating the displaced entity's
    /// `EntityLocation` to `(self.id, row)`.
    ///
    /// Used during `despawn`.
    pub fn swap_remove_row(&mut self, row: usize) -> Option<Entity> {
        let last = self.entities.len().saturating_sub(1);
        for col in self.columns.values_mut() {
            col.swap_remove_erased(row);
        }
        self.entities.swap_remove(row);
        if row < last {
            // The entity that was at `last` is now at `row`.
            Some(self.entities[row])
        } else {
            None
        }
    }

    /// Move all component data for `row` into `dest` for the types that
    /// `dest` declares in its `component_types`.
    ///
    /// For each `TypeId` in `self.component_types`:
    /// - If `dest` should have it (`dest.component_types.contains`): create
    ///   the column in `dest` if absent (via `new_empty`), then
    ///   `swap_remove_erased` from `self` and `push_erased` into `dest`.
    /// - Otherwise (the type is being removed, e.g. during `remove<T>`):
    ///   skip it — the caller extracts it separately.
    ///
    /// **Does not touch `self.entities` or `dest.entities`** — the caller
    /// handles those.
    ///
    /// After this call the source's columns for the moved types have
    /// `n − 1` elements (swap-remove semantics). The caller must also do
    /// `self.entities.swap_remove(row)` and update the displaced entity's
    /// location.
    pub fn move_components_to(&mut self, row: usize, dest: &mut Archetype) {
        let types_to_move: Vec<TypeId> = self
            .component_types
            .iter()
            .copied()
            .filter(|tid| dest.component_types.contains(tid))
            .collect();

        // Pass 1 — create any missing columns in dest. `new_empty()` doesn't
        // depend on column contents, so it's safe to call before we
        // swap-remove from self.
        for &tid in &types_to_move {
            dest.columns.entry(tid).or_insert_with(|| {
                self.columns
                    .get(&tid)
                    .expect("move_components_to: source column missing for new_empty")
                    .new_empty()
            });
        }

        // Pass 2 — move data: swap-remove from self, push into dest.
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
