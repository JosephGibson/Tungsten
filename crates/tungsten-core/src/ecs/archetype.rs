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
            if !dest.columns.contains_key(&tid) {
                let empty = self
                    .columns
                    .get(&tid)
                    .expect("move_components_to: source column missing for new_empty")
                    .new_empty();
                dest.columns.insert(tid, empty);
            }
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_arch(id: ArchetypeId, types: &[TypeId]) -> Archetype {
        let mut sorted = types.to_vec();
        sorted.sort();
        Archetype::new(id, sorted.into_boxed_slice())
    }

    fn seed_columns<A: 'static, B: 'static>(arch: &mut Archetype) {
        arch.columns
            .insert(TypeId::of::<A>(), Box::new(TypedVec::<A>(Vec::new())));
        arch.columns
            .insert(TypeId::of::<B>(), Box::new(TypedVec::<B>(Vec::new())));
    }

    fn push_row<A: 'static, B: 'static>(arch: &mut Archetype, entity: Entity, a: A, b: B) {
        arch.columns
            .get_mut(&TypeId::of::<A>())
            .unwrap()
            .push_erased(Box::new(a));
        arch.columns
            .get_mut(&TypeId::of::<B>())
            .unwrap()
            .push_erased(Box::new(b));
        arch.entities.push(entity);
    }

    fn make_entity(index: u32) -> Entity {
        Entity {
            index,
            generation: 0,
        }
    }

    // ------------------------------------------------------------------

    #[test]
    fn push_and_get() {
        let mut arch = make_arch(1, &[TypeId::of::<u32>(), TypeId::of::<f32>()]);
        seed_columns::<u32, f32>(&mut arch);
        push_row::<u32, f32>(&mut arch, make_entity(0), 42u32, 1.5f32);

        let col = arch.columns[&TypeId::of::<u32>()]
            .as_any()
            .downcast_ref::<TypedVec<u32>>()
            .unwrap();
        assert_eq!(col.0[0], 42u32);
    }

    #[test]
    fn swap_remove_row_middle() {
        let mut arch = make_arch(1, &[TypeId::of::<u32>(), TypeId::of::<f32>()]);
        seed_columns::<u32, f32>(&mut arch);

        let e0 = make_entity(0);
        let e1 = make_entity(1);
        let e2 = make_entity(2);

        push_row::<u32, f32>(&mut arch, e0, 0u32, 0.0f32);
        push_row::<u32, f32>(&mut arch, e1, 1u32, 1.0f32);
        push_row::<u32, f32>(&mut arch, e2, 2u32, 2.0f32);

        // Remove row 1 (e1). e2 should displace into row 1.
        let displaced = arch.swap_remove_row(1);
        assert_eq!(displaced, Some(e2));
        assert_eq!(arch.row_count(), 2);
        assert_eq!(arch.entities[1], e2);

        // Check column consistency.
        let u32_col = arch.columns[&TypeId::of::<u32>()]
            .as_any()
            .downcast_ref::<TypedVec<u32>>()
            .unwrap();
        assert_eq!(u32_col.0, vec![0u32, 2u32]);

        let f32_col = arch.columns[&TypeId::of::<f32>()]
            .as_any()
            .downcast_ref::<TypedVec<f32>>()
            .unwrap();
        assert_eq!(f32_col.0, vec![0.0f32, 2.0f32]);
    }

    #[test]
    fn swap_remove_last_row_returns_none() {
        let mut arch = make_arch(1, &[TypeId::of::<u32>()]);
        arch.columns
            .insert(TypeId::of::<u32>(), Box::new(TypedVec::<u32>(Vec::new())));
        push_row::<u32, u32>(&mut arch, make_entity(0), 99u32, 99u32); // only one col
                                                                       // Manually fix: just one column
        arch.columns.clear();
        arch.columns
            .insert(TypeId::of::<u32>(), Box::new(TypedVec::<u32>(vec![99u32])));
        arch.entities = vec![make_entity(0)];

        let displaced = arch.swap_remove_row(0);
        assert_eq!(displaced, None);
        assert_eq!(arch.row_count(), 0);
    }

    #[test]
    fn move_components_to_transfers_matching_types() {
        // Source: {u32, f32, i32}. Dest: {u32, f32} (removing i32).
        let mut src = make_arch(
            1,
            &[
                TypeId::of::<u32>(),
                TypeId::of::<f32>(),
                TypeId::of::<i32>(),
            ],
        );
        src.columns.insert(
            TypeId::of::<u32>(),
            Box::new(TypedVec::<u32>(vec![10u32, 20u32])),
        );
        src.columns.insert(
            TypeId::of::<f32>(),
            Box::new(TypedVec::<f32>(vec![1.0f32, 2.0f32])),
        );
        src.columns.insert(
            TypeId::of::<i32>(),
            Box::new(TypedVec::<i32>(vec![-1i32, -2i32])),
        );
        src.entities = vec![make_entity(0), make_entity(1)];

        let mut dst = make_arch(2, &[TypeId::of::<u32>(), TypeId::of::<f32>()]);
        // dst columns are initially empty (lazy creation).

        // Move row 0 (value 10 / 1.0 / -1).
        src.move_components_to(0, &mut dst);

        // dst should now have row 0.
        let dst_u32 = dst.columns[&TypeId::of::<u32>()]
            .as_any()
            .downcast_ref::<TypedVec<u32>>()
            .unwrap();
        assert_eq!(dst_u32.0, vec![10u32]);

        let dst_f32 = dst.columns[&TypeId::of::<f32>()]
            .as_any()
            .downcast_ref::<TypedVec<f32>>()
            .unwrap();
        assert_eq!(dst_f32.0, vec![1.0f32]);

        // i32 was NOT in dst.component_types, so it was left in src.
        let src_i32 = src.columns[&TypeId::of::<i32>()]
            .as_any()
            .downcast_ref::<TypedVec<i32>>()
            .unwrap();
        // src had 2 rows; after move_components_to(0, …) the moved columns are
        // now 1 row with the last element at index 0.
        assert_eq!(src_i32.0.len(), 2, "i32 column should be untouched");

        // src u32/f32 columns should each have 1 row remaining.
        let src_u32 = src.columns[&TypeId::of::<u32>()]
            .as_any()
            .downcast_ref::<TypedVec<u32>>()
            .unwrap();
        assert_eq!(src_u32.0, vec![20u32]);
    }

    #[test]
    fn columns_consistent_length_after_multiple_removals() {
        let mut arch = make_arch(1, &[TypeId::of::<u32>(), TypeId::of::<bool>()]);
        seed_columns::<u32, bool>(&mut arch);

        for i in 0u32..5 {
            push_row::<u32, bool>(&mut arch, make_entity(i), i, i % 2 == 0);
        }

        // Remove rows 2, then 0.
        arch.swap_remove_row(2);
        arch.swap_remove_row(0);

        let u32_len = arch.columns[&TypeId::of::<u32>()].len();
        let bool_len = arch.columns[&TypeId::of::<bool>()].len();
        assert_eq!(u32_len, arch.entities.len());
        assert_eq!(bool_len, arch.entities.len());
        assert_eq!(u32_len, 3);
    }
}
