use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

use super::entity::Entity;

/// Map keyed by `TypeId`, which is already a high-quality hash: the key's
/// `write_u64` passes straight through instead of being SipHashed again on
/// every edge lookup (Bevy's `TypeIdMap` + `NoOpHash`).
pub(crate) type TypeIdMap<V> = HashMap<TypeId, V, BuildHasherDefault<TypeIdHasher>>;

/// Pass-through hasher for [`TypeIdMap`].
#[derive(Default)]
pub(crate) struct TypeIdHasher(u64);

impl Hasher for TypeIdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write_u64(&mut self, n: u64) {
        self.0 = n;
    }

    /// Fallback in case a `TypeId` layout hashes through bytes: slower but
    /// still a full mix of every byte, so lookups stay correct.
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = self.0.rotate_left(8).wrapping_add(u64::from(byte)) ^ 0x9E37_79B9_7F4A_7C15;
        }
    }
}

/// The word a `TypeId` hashes to, which [`TypeIdHasher`] passes through.
#[inline]
fn type_hash(type_id: TypeId) -> u64 {
    use std::hash::BuildHasher;

    BuildHasherDefault::<TypeIdHasher>::default().hash_one(type_id)
}

/// An archetype's direct column lookup for random access by entity.
///
/// Slot `(hash >> shift) % SLOTS` holds the column index of the one key type
/// that hashes there; `shift` is picked when the archetype is created so
/// that no two key types share a slot. A lookup is then two loads inside the
/// archetype, with no branch that depends on which archetype it is: behind
/// a cache-missing entity lookup, a key scan's exit mispredicts and a hash
/// table's lines are two more misses, and either keeps the next lookup's
/// miss from starting early (D-083).
pub(crate) struct ColumnSlots {
    /// `NO_SHIFT` when no shift separates the key's types.
    shift: u8,
    slots: [u8; ColumnSlots::SLOTS],
}

impl ColumnSlots {
    const SLOTS: usize = 128;
    const EMPTY: u8 = u8::MAX;
    const NO_SHIFT: u8 = u8::MAX;

    /// Slots for a key whose types hash to `hashes`, in column order.
    fn new(hashes: &[u64]) -> Self {
        if hashes.len() < usize::from(Self::EMPTY) {
            // Seven bits fit at shifts 0 to 57.
            for shift in 0..=57 {
                let mut slots = [Self::EMPTY; Self::SLOTS];
                let separate = hashes.iter().enumerate().all(|(index, hash)| {
                    let slot = &mut slots[Self::slot(*hash, shift)];
                    let free = *slot == Self::EMPTY;
                    *slot = index as u8;
                    free
                });
                if separate {
                    return Self { shift, slots };
                }
            }
        }
        Self {
            shift: Self::NO_SHIFT,
            slots: [Self::EMPTY; Self::SLOTS],
        }
    }

    #[inline]
    fn slot(hash: u64, shift: u8) -> usize {
        (hash >> shift) as usize % Self::SLOTS
    }
}

/// Type-erased `Vec<T>` column interface.
///
/// Query cost: one downcast per archetype/type, then contiguous `Vec<T>` access.
/// Rows move between columns and drop in place, so no value is boxed (D-083).
pub(crate) trait AnyColumn: Any {
    /// Swap-remove `row` and push its value onto `dest`, a column of the same
    /// component type.
    fn move_row_to(&mut self, row: usize, dest: &mut dyn AnyColumn);
    /// Swap-remove `row` and drop its value.
    fn swap_remove_drop(&mut self, row: usize);
    /// Empty column of same concrete type.
    fn new_empty(&self) -> Box<dyn AnyColumn>;
}

impl dyn AnyColumn {
    /// Concrete column for component `T`; `None` when it stores another type.
    pub fn typed<T: 'static>(&self) -> Option<&TypedVec<T>> {
        (self as &dyn Any).downcast_ref()
    }

    /// Mutable counterpart of [`typed`](Self::typed).
    pub fn typed_mut<T: 'static>(&mut self) -> Option<&mut TypedVec<T>> {
        (self as &mut dyn Any).downcast_mut()
    }
}

/// Typed component column.
pub(crate) struct TypedVec<T: 'static>(pub Vec<T>);

impl<T: 'static> TypedVec<T> {
    /// Write `value` at `row`: a push when `row` is the next row, an
    /// overwrite when the row already holds a value.
    pub fn write(&mut self, row: usize, value: T) {
        if row == self.0.len() {
            self.0.push(value);
        } else {
            self.0[row] = value;
        }
    }
}

impl<T: 'static> AnyColumn for TypedVec<T> {
    fn move_row_to(&mut self, row: usize, dest: &mut dyn AnyColumn) {
        dest.typed_mut::<T>()
            .expect("move_row_to: column type mismatch")
            .0
            .push(self.0.swap_remove(row));
    }

    fn swap_remove_drop(&mut self, row: usize) {
        self.0.swap_remove(row);
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
    /// Sorted component type key.
    pub component_types: Box<[TypeId]>,
    /// One column per component type, in `component_types` order, created
    /// with the archetype.
    pub columns: Vec<Box<dyn AnyColumn>>,
    /// Type -> column for random access ([`slot_column`](Self::slot_column)).
    column_slots: ColumnSlots,
    /// Entity per row.
    pub entities: Vec<Entity>,
    /// Lazy add-edge cache.
    pub add_edges: TypeIdMap<ArchetypeId>,
    /// Lazy remove-edge cache.
    pub remove_edges: TypeIdMap<ArchetypeId>,
}

impl Archetype {
    /// `columns` holds one empty column per entry of `component_types`, in
    /// the same order.
    pub fn new(component_types: Box<[TypeId]>, columns: Vec<Box<dyn AnyColumn>>) -> Self {
        debug_assert_eq!(component_types.len(), columns.len());
        let hashes: Vec<u64> = component_types.iter().map(|&tid| type_hash(tid)).collect();
        Self {
            component_types,
            columns,
            column_slots: ColumnSlots::new(&hashes),
            entities: Vec::new(),
            add_edges: TypeIdMap::default(),
            remove_edges: TypeIdMap::default(),
        }
    }

    pub fn has(&self, type_id: TypeId) -> bool {
        self.component_types.contains(&type_id)
    }

    /// Index of `type_id`'s column: its position in the sorted type key,
    /// found by scanning the key. For structural changes and query setup,
    /// which read the key anyway.
    ///
    /// Out of line on purpose. Inlined into a query's per-archetype setup,
    /// the scan loop sat beside the caller's row loop and changed how that
    /// loop compiled: `bounds_wrap` rebuilt its constants on every row.
    #[inline(never)]
    pub fn column_index(&self, type_id: TypeId) -> Option<usize> {
        self.component_types.iter().position(|&tid| tid == type_id)
    }

    /// Index of the column in `type_id`'s slot, for random access.
    ///
    /// That is `type_id`'s own column when the archetype has the type. When
    /// it lacks the type the slot is empty (`None`) or belongs to another
    /// type, so the caller must downcast the column to the type it asked
    /// for; [`TypedVec`] of another type rejects it.
    #[inline]
    fn slot_index(&self, type_id: TypeId) -> Option<usize> {
        let slots = &self.column_slots;
        if slots.shift == ColumnSlots::NO_SHIFT {
            return self.column_index(type_id);
        }
        let index = slots.slots[ColumnSlots::slot(type_hash(type_id), slots.shift)];
        (index != ColumnSlots::EMPTY).then_some(usize::from(index))
    }

    /// Typed column for `T` through its slot; `None` when the archetype
    /// lacks `T`. The random-access counterpart of
    /// [`typed_column`](Self::typed_column).
    #[inline]
    pub fn slot_column<T: 'static>(&self) -> Option<&TypedVec<T>> {
        self.columns[self.slot_index(TypeId::of::<T>())?].typed::<T>()
    }

    /// The column in `type_id`'s slot, type-erased: `type_id`'s own column
    /// when the archetype has the type. As with
    /// [`slot_column`](Self::slot_column), only the caller's downcast tells
    /// it from another type's column.
    #[inline]
    pub fn slot_column_erased_mut(&mut self, type_id: TypeId) -> Option<&mut dyn AnyColumn> {
        let index = self.slot_index(type_id)?;
        Some(&mut *self.columns[index])
    }

    /// Mutable counterpart of [`slot_column`](Self::slot_column).
    #[inline]
    pub fn slot_column_mut<T: 'static>(&mut self) -> Option<&mut TypedVec<T>> {
        let index = self.slot_index(TypeId::of::<T>())?;
        self.columns[index].typed_mut::<T>()
    }

    /// Column storing `type_id`; `None` when the archetype lacks it.
    pub fn column(&self, type_id: TypeId) -> Option<&dyn AnyColumn> {
        self.column_index(type_id)
            .map(|index| &*self.columns[index])
    }

    /// Mutable counterpart of [`column`](Self::column).
    pub fn column_mut(&mut self, type_id: TypeId) -> Option<&mut dyn AnyColumn> {
        self.column_index(type_id)
            .map(|index| &mut *self.columns[index])
    }

    /// Typed column for component `T`; `None` when the archetype lacks it.
    pub fn typed_column<T: 'static>(&self) -> Option<&TypedVec<T>> {
        self.column(TypeId::of::<T>())
            .map(|column| column.typed().expect("column type matches its key"))
    }

    /// Mutable counterpart of [`typed_column`](Self::typed_column).
    pub fn typed_column_mut<T: 'static>(&mut self) -> Option<&mut TypedVec<T>> {
        self.column_mut(TypeId::of::<T>())
            .map(|column| column.typed_mut().expect("column type matches its key"))
    }

    /// Swap-remove row from every column; caller updates displaced location.
    pub fn swap_remove_row(&mut self, row: usize) -> Option<Entity> {
        let last = self.entities.len().saturating_sub(1);
        for column in &mut self.columns {
            column.swap_remove_drop(row);
        }
        self.entities.swap_remove(row);
        if row < last {
            Some(self.entities[row])
        } else {
            None
        }
    }

    /// Move row component data into `dest`; caller handles entity rows/locations.
    ///
    /// Both type keys are sorted, so one merge walk pairs the columns. A type
    /// `dest` lacks keeps its value in the source for the caller to take.
    pub fn move_components_to(&mut self, row: usize, dest: &mut Archetype) {
        let mut dest_index = 0;
        for (column, &tid) in self.columns.iter_mut().zip(self.component_types.iter()) {
            while dest
                .component_types
                .get(dest_index)
                .is_some_and(|&dest_tid| dest_tid < tid)
            {
                dest_index += 1;
            }
            if dest.component_types.get(dest_index) == Some(&tid) {
                column.move_row_to(row, &mut *dest.columns[dest_index]);
                dest_index += 1;
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/ecs/archetype.rs"]
mod tests;
