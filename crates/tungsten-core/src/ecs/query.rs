//! Tuple queries: `world.query::<(Entity, &A, Option<&B>)>()`,
//! `world.query_mut::<(&mut A, &B)>()` and their `_filtered` forms with
//! [`With`] and [`Without`] (`D-130`).
//!
//! A query names its items as a tuple of up to eight of `&A`, `&mut A`,
//! `Option<&A>`, `Option<&mut A>` and `Entity`; a single item needs no
//! tuple. Rows come in archetype order, then row order, so a read query and
//! a mutable query over the same archetypes zip by row within a frame. Per
//! archetype every item resolves its column once and iterates a slice, so an
//! optional column costs nothing per row and an absent one yields `None`
//! for each row. A filter, the second type parameter of the `_filtered`
//! forms, is checked against each archetype's type key, never per row.
//!
//! Mutable columns are borrowed through `slice::get_disjoint_mut` on the
//! archetype's column vector, one range per item, so two items can never
//! alias a column and the compiler checks every borrow; a mutable query
//! that names one component twice is refused when it is built, by a panic
//! that names the query. A read-only query may name one twice. The items' per-archetype
//! slices (`&[A]`, `&mut [A]`, `Option<&[A]>`, `&[Entity]`) come from
//! [`World::query_slices`](super::World::query_slices) and its siblings.
//!
//! The traits are sealed: the engine implements them for the five items,
//! their tuples and the filters, and nothing else can.

use std::any::{TypeId, type_name};
use std::iter::{Copied, Zip};
use std::marker::PhantomData;
use std::ops::Range;
use std::slice;

use super::archetype::{AnyColumn, Archetype};
use super::entity::Entity;

mod sealed {
    pub trait Data {}
    pub trait Filter {}
}

/// One archetype's columns, shared, for a read-only query.
#[allow(clippy::struct_field_names)]
pub struct Columns<'w> {
    types: &'w [TypeId],
    columns: &'w [Box<dyn AnyColumn>],
    entities: &'w [Entity],
}

/// One archetype's columns for a query that writes some of them.
#[allow(clippy::struct_field_names)]
pub struct ColumnsMut<'w> {
    types: &'w [TypeId],
    columns: &'w mut [Box<dyn AnyColumn>],
    entities: &'w [Entity],
}

impl<'w> Columns<'w> {
    #[inline]
    pub(crate) fn of(arch: &'w Archetype) -> Self {
        Self {
            types: &arch.component_types,
            columns: &arch.columns,
            entities: &arch.entities,
        }
    }
}

impl<'w> ColumnsMut<'w> {
    #[inline]
    pub(crate) fn of(arch: &'w mut Archetype) -> Self {
        let Archetype {
            component_types,
            columns,
            entities,
            ..
        } = arch;
        Self {
            types: component_types,
            columns,
            entities,
        }
    }
}

/// An item's column in one archetype, when the archetype has it.
#[derive(Clone, Copy)]
pub struct Slot<'w>(Option<&'w dyn AnyColumn>);

/// Mutable counterpart of [`Slot`].
pub struct SlotMut<'w>(Option<&'w mut dyn AnyColumn>);

impl<'w> SlotMut<'w> {
    #[inline]
    fn shared(self) -> Slot<'w> {
        Slot(self.0.map(|column| -> &'w dyn AnyColumn { column }))
    }
}

/// One element of a query: what it takes from an archetype and how it
/// iterates it. Implemented for `&A`, `&mut A`, `Option<&A>`,
/// `Option<&mut A>` and `Entity`.
pub trait QueryItem<'w>: Sized + sealed::Data {
    /// The item's slice of one archetype.
    type Slice;
    /// The item's rows over that slice.
    type Iter: Iterator<Item = Self>;

    /// The component the item reads or writes; `None` for `Entity`.
    fn component() -> Option<TypeId>;
    /// Whether an archetype must hold the component to match.
    fn required() -> bool;
    /// The item's slice from its column slot and the archetype's entities.
    fn slice_mut(slot: SlotMut<'w>, entities: &'w [Entity]) -> Self::Slice;
    /// The rows of a slice; `rows` bounds an absent optional column.
    fn iter(slice: Self::Slice, rows: usize) -> Self::Iter;
}

/// An item that only reads, so it can come from a shared `World`.
pub trait ReadOnlyItem<'w>: QueryItem<'w> {
    /// [`slice_mut`](QueryItem::slice_mut) from a shared slot.
    fn slice(slot: Slot<'w>, entities: &'w [Entity]) -> Self::Slice;
}

/// The typed slice of a required column.
#[inline]
fn typed<A: 'static>(slot: Slot<'_>) -> &[A] {
    slot.0
        .expect("query: a required column is missing")
        .typed::<A>()
        .expect("column type matches its key")
        .0
        .as_slice()
}

/// Mutable counterpart of [`typed`].
#[inline]
fn typed_mut<A: 'static>(slot: SlotMut<'_>) -> &mut [A] {
    slot.0
        .expect("query_mut: a required column is missing")
        .typed_mut::<A>()
        .expect("column type matches its key")
        .0
        .as_mut_slice()
}

impl<A: 'static> sealed::Data for &A {}

impl<'w, A: 'static> QueryItem<'w> for &'w A {
    type Slice = &'w [A];
    type Iter = slice::Iter<'w, A>;

    #[inline]
    fn component() -> Option<TypeId> {
        Some(TypeId::of::<A>())
    }

    #[inline]
    fn required() -> bool {
        true
    }

    #[inline]
    fn slice_mut(slot: SlotMut<'w>, _: &'w [Entity]) -> Self::Slice {
        typed::<A>(slot.shared())
    }

    #[inline]
    fn iter(slice: Self::Slice, _: usize) -> Self::Iter {
        slice.iter()
    }
}

impl<'w, A: 'static> ReadOnlyItem<'w> for &'w A {
    #[inline]
    fn slice(slot: Slot<'w>, _: &'w [Entity]) -> Self::Slice {
        typed::<A>(slot)
    }
}

impl<A: 'static> sealed::Data for &mut A {}

impl<'w, A: 'static> QueryItem<'w> for &'w mut A {
    type Slice = &'w mut [A];
    type Iter = slice::IterMut<'w, A>;

    #[inline]
    fn component() -> Option<TypeId> {
        Some(TypeId::of::<A>())
    }

    #[inline]
    fn required() -> bool {
        true
    }

    #[inline]
    fn slice_mut(slot: SlotMut<'w>, _: &'w [Entity]) -> Self::Slice {
        typed_mut::<A>(slot)
    }

    #[inline]
    fn iter(slice: Self::Slice, _: usize) -> Self::Iter {
        slice.iter_mut()
    }
}

impl<A: 'static> sealed::Data for Option<&A> {}

impl<'w, A: 'static> QueryItem<'w> for Option<&'w A> {
    type Slice = Option<&'w [A]>;
    type Iter = OptionalRows<slice::Iter<'w, A>>;

    #[inline]
    fn component() -> Option<TypeId> {
        Some(TypeId::of::<A>())
    }

    #[inline]
    fn required() -> bool {
        false
    }

    #[inline]
    fn slice_mut(slot: SlotMut<'w>, _: &'w [Entity]) -> Self::Slice {
        let slot = slot.shared();
        slot.0.is_some().then(|| typed::<A>(slot))
    }

    #[inline]
    fn iter(slice: Self::Slice, rows: usize) -> Self::Iter {
        OptionalRows {
            column: slice.map(<[A]>::iter),
            rows,
        }
    }
}

impl<'w, A: 'static> ReadOnlyItem<'w> for Option<&'w A> {
    #[inline]
    fn slice(slot: Slot<'w>, _: &'w [Entity]) -> Self::Slice {
        slot.0.is_some().then(|| typed::<A>(slot))
    }
}

impl<A: 'static> sealed::Data for Option<&mut A> {}

impl<'w, A: 'static> QueryItem<'w> for Option<&'w mut A> {
    type Slice = Option<&'w mut [A]>;
    type Iter = OptionalRows<slice::IterMut<'w, A>>;

    #[inline]
    fn component() -> Option<TypeId> {
        Some(TypeId::of::<A>())
    }

    #[inline]
    fn required() -> bool {
        false
    }

    #[inline]
    fn slice_mut(slot: SlotMut<'w>, _: &'w [Entity]) -> Self::Slice {
        slot.0.is_some().then(|| typed_mut::<A>(slot))
    }

    #[inline]
    fn iter(slice: Self::Slice, rows: usize) -> Self::Iter {
        OptionalRows {
            column: slice.map(<[A]>::iter_mut),
            rows,
        }
    }
}

impl sealed::Data for Entity {}

impl<'w> QueryItem<'w> for Entity {
    type Slice = &'w [Entity];
    type Iter = Copied<slice::Iter<'w, Entity>>;

    #[inline]
    fn component() -> Option<TypeId> {
        None
    }

    #[inline]
    fn required() -> bool {
        false
    }

    #[inline]
    fn slice_mut(_: SlotMut<'w>, entities: &'w [Entity]) -> Self::Slice {
        entities
    }

    #[inline]
    fn iter(slice: Self::Slice, _: usize) -> Self::Iter {
        slice.iter().copied()
    }
}

impl<'w> ReadOnlyItem<'w> for Entity {
    #[inline]
    fn slice(_: Slot<'w>, entities: &'w [Entity]) -> Self::Slice {
        entities
    }
}

/// Rows of an optional column: `Some(item)` per row when the archetype has
/// the column, `None` for each of the archetype's rows when it does not.
pub struct OptionalRows<I> {
    column: Option<I>,
    rows: usize,
}

impl<I: Iterator> Iterator for OptionalRows<I> {
    type Item = Option<I::Item>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(column) = self.column.as_mut() {
            column.next().map(Some)
        } else {
            self.rows = self.rows.checked_sub(1)?;
            Some(None)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.column {
            Some(column) => column.size_hint(),
            None => (self.rows, Some(self.rows)),
        }
    }
}

/// Rows of an optional column for a loop written by hand over
/// [`World::query_slices`](super::World::query_slices):
/// `OptionalColumn(column.map(<[A]>::iter))` yields `Some(item)` per row when
/// the archetype has the column and `None` without end when it does not, so
/// it goes in a zip after a required column or the entities, which bound the
/// rows. [`OptionalRows`], what the tuple queries build, counts the rows
/// instead, so a query of optional items alone still ends.
pub struct OptionalColumn<I>(pub Option<I>);

impl<I: Iterator> Iterator for OptionalColumn<I> {
    type Item = Option<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.as_mut() {
            Some(iter) => iter.next().map(Some),
            None => Some(None),
        }
    }
}

/// Narrows a query to archetypes by components it does not read.
pub trait QueryFilter: sealed::Filter {
    /// Whether an archetype with this sorted type key passes.
    fn matches(types: &[TypeId]) -> bool;
}

/// Filter: the archetype holds `T`.
pub struct With<T>(PhantomData<fn() -> T>);

/// Filter: the archetype lacks `T`.
pub struct Without<T>(PhantomData<fn() -> T>);

impl<T: 'static> sealed::Filter for With<T> {}

impl<T: 'static> QueryFilter for With<T> {
    #[inline]
    fn matches(types: &[TypeId]) -> bool {
        types.contains(&TypeId::of::<T>())
    }
}

impl<T: 'static> sealed::Filter for Without<T> {}

impl<T: 'static> QueryFilter for Without<T> {
    #[inline]
    fn matches(types: &[TypeId]) -> bool {
        !types.contains(&TypeId::of::<T>())
    }
}

impl sealed::Filter for () {}

impl QueryFilter for () {
    #[inline]
    fn matches(_: &[TypeId]) -> bool {
        true
    }
}

macro_rules! filter_tuple {
    ($($F:ident),+) => {
        impl<$($F: QueryFilter),+> sealed::Filter for ($($F,)+) {}

        impl<$($F: QueryFilter),+> QueryFilter for ($($F,)+) {
            #[inline]
            fn matches(types: &[TypeId]) -> bool {
                $($F::matches(types))&&+
            }
        }
    };
}

filter_tuple!(F0);
filter_tuple!(F0, F1);
filter_tuple!(F0, F1, F2);
filter_tuple!(F0, F1, F2, F3);

/// What a query yields per row: a tuple of [`QueryItem`]s, or one item.
pub trait QueryData<'w>: Sized + sealed::Data {
    /// The items' slices of one archetype.
    type Slices;
    /// The rows of one archetype.
    type Iter: Iterator<Item = Self>;

    /// Whether an archetype with this sorted type key holds every required
    /// component.
    fn matches(types: &[TypeId]) -> bool;
    /// Panics when two items name one component, since the mutable borrows
    /// would alias; called once per mutable query.
    fn check_access();
    /// The items' slices, each column borrowed once.
    fn slices_mut(columns: ColumnsMut<'w>) -> Self::Slices;
    /// The rows over the slices of an archetype with `rows` entities.
    fn iter(slices: Self::Slices, rows: usize) -> Self::Iter;
}

/// Query data whose items only read, so it can come from a shared `World`.
pub trait ReadOnlyQueryData<'w>: QueryData<'w> {
    /// [`slices_mut`](QueryData::slices_mut) from shared columns.
    fn slices(columns: Columns<'w>) -> Self::Slices;
}

/// Whether an archetype with the type key `types` satisfies item `T`.
#[inline]
fn item_matches<'w, T: QueryItem<'w>>(types: &[TypeId]) -> bool {
    match T::component() {
        Some(component) if T::required() => types.contains(&component),
        _ => true,
    }
}

/// The column slot of `component` in an archetype's sorted type key: its
/// column index as a one-element range, or an empty range when the archetype
/// lacks it. Out of line, like `Archetype::column_index`, so the key scan
/// does not sit beside a query's row loop.
#[inline(never)]
fn column_range(types: &[TypeId], component: Option<TypeId>) -> Range<usize> {
    component
        .and_then(|component| types.iter().position(|&tid| tid == component))
        .map_or(0..0, |index| index..index + 1)
}

/// The shared column in `component`'s slot.
#[inline]
fn column_at<'w>(
    columns: &'w [Box<dyn AnyColumn>],
    types: &[TypeId],
    component: Option<TypeId>,
) -> Slot<'w> {
    Slot(
        columns[column_range(types, component)]
            .first()
            .map(|column| &**column),
    )
}

/// The mutable columns in `ranges`, each borrowed once: `get_disjoint_mut`
/// refuses two ranges on one column, which `check_access` has already ruled
/// out with a message that names the query. An absent column's empty range
/// sits at 0, where it overlaps no other range.
#[inline]
fn slots_mut<const N: usize>(
    columns: &mut [Box<dyn AnyColumn>],
    ranges: [Range<usize>; N],
) -> [SlotMut<'_>; N] {
    columns
        .get_disjoint_mut(ranges)
        .expect("query_mut: check_access leaves the column ranges disjoint")
        .map(|slot| SlotMut(slot.first_mut().map(|column| &mut **column)))
}

/// Panics when a component appears twice among `items`.
fn distinct(items: &[(Option<TypeId>, &'static str)], query: &str) {
    for (index, (component, name)) in items.iter().enumerate() {
        if let Some(component) = component {
            assert!(
                items[index + 1..]
                    .iter()
                    .all(|(other, _)| *other != Some(*component)),
                "{query}: component types must be distinct ({name} appears twice)"
            );
        }
    }
}

macro_rules! nest {
    ($x:ident) => { $x };
    ($x:ident, $($rest:ident),+) => { ($x, nest!($($rest),+)) };
}

macro_rules! zip_ty {
    ($T:ident) => { <$T as QueryItem<'w>>::Iter };
    ($T:ident, $($rest:ident),+) => { Zip<<$T as QueryItem<'w>>::Iter, zip_ty!($($rest),+)> };
}

macro_rules! zip_expr {
    ($x:ident) => { $x };
    ($x:ident, $($rest:ident),+) => { $x.zip(zip_expr!($($rest),+)) };
}

macro_rules! tuple_query {
    ($rows:ident: $($T:ident $t:ident),+) => {
        /// Flattens the zipped item iterators of a tuple query into its rows.
        pub struct $rows<I>(I);

        impl<I, $($T),+> Iterator for $rows<I>
        where
            I: Iterator<Item = nest!($($T),+)>,
        {
            type Item = ($($T,)+);

            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                let nest!($($t),+) = self.0.next()?;
                Some(($($t,)+))
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }
        }

        impl<'w, $($T: QueryItem<'w>),+> sealed::Data for ($($T,)+) {}

        impl<'w, $($T: QueryItem<'w>),+> QueryData<'w> for ($($T,)+) {
            type Slices = ($($T::Slice,)+);
            type Iter = $rows<zip_ty!($($T),+)>;

            #[inline]
            fn matches(types: &[TypeId]) -> bool {
                $(item_matches::<$T>(types))&&+
            }

            fn check_access() {
                distinct(
                    &[$(($T::component(), type_name::<$T>())),+],
                    type_name::<Self>(),
                );
            }

            #[inline]
            fn slices_mut(columns: ColumnsMut<'w>) -> Self::Slices {
                let ColumnsMut {
                    types,
                    columns,
                    entities,
                } = columns;
                let [$($t),+] =
                    slots_mut(columns, [$(column_range(types, $T::component())),+]);
                ($($T::slice_mut($t, entities),)+)
            }

            #[inline]
            fn iter(slices: Self::Slices, rows: usize) -> Self::Iter {
                let ($($t,)+) = slices;
                $(let $t = $T::iter($t, rows);)+
                $rows(zip_expr!($($t),+))
            }
        }

        impl<'w, $($T: ReadOnlyItem<'w>),+> ReadOnlyQueryData<'w> for ($($T,)+) {
            #[inline]
            fn slices(columns: Columns<'w>) -> Self::Slices {
                let Columns {
                    types,
                    columns,
                    entities,
                } = columns;
                ($($T::slice(column_at(columns, types, $T::component()), entities),)+)
            }
        }
    };
}

tuple_query!(Rows1: T0 t0);
tuple_query!(Rows2: T0 t0, T1 t1);
tuple_query!(Rows3: T0 t0, T1 t1, T2 t2);
tuple_query!(Rows4: T0 t0, T1 t1, T2 t2, T3 t3);
tuple_query!(Rows5: T0 t0, T1 t1, T2 t2, T3 t3, T4 t4);
tuple_query!(Rows6: T0 t0, T1 t1, T2 t2, T3 t3, T4 t4, T5 t5);
tuple_query!(Rows7: T0 t0, T1 t1, T2 t2, T3 t3, T4 t4, T5 t5, T6 t6);
tuple_query!(Rows8: T0 t0, T1 t1, T2 t2, T3 t3, T4 t4, T5 t5, T6 t6, T7 t7);

/// A single item as query data, with no tuple around it.
macro_rules! single_query {
    ($A:ident, $ty:ty) => {
        impl<'w, $A: 'static> QueryData<'w> for $ty {
            type Slices = <$ty as QueryItem<'w>>::Slice;
            type Iter = <$ty as QueryItem<'w>>::Iter;

            #[inline]
            fn matches(types: &[TypeId]) -> bool {
                item_matches::<$ty>(types)
            }

            fn check_access() {}

            #[inline]
            fn slices_mut(columns: ColumnsMut<'w>) -> Self::Slices {
                let ColumnsMut {
                    types,
                    columns,
                    entities,
                } = columns;
                let [slot] = slots_mut(
                    columns,
                    [column_range(types, <$ty as QueryItem<'w>>::component())],
                );
                <$ty as QueryItem<'w>>::slice_mut(slot, entities)
            }

            #[inline]
            fn iter(slices: Self::Slices, rows: usize) -> Self::Iter {
                <$ty as QueryItem<'w>>::iter(slices, rows)
            }
        }
    };
}

macro_rules! single_query_read_only {
    ($A:ident, $ty:ty) => {
        single_query!($A, $ty);

        impl<'w, $A: 'static> ReadOnlyQueryData<'w> for $ty {
            #[inline]
            fn slices(columns: Columns<'w>) -> Self::Slices {
                let Columns {
                    types,
                    columns,
                    entities,
                } = columns;
                <$ty as ReadOnlyItem<'w>>::slice(
                    column_at(columns, types, <$ty as QueryItem<'w>>::component()),
                    entities,
                )
            }
        }
    };
}

single_query_read_only!(A, &'w A);
single_query!(A, &'w mut A);
single_query_read_only!(A, Option<&'w A>);
single_query!(A, Option<&'w mut A>);

impl<'w> QueryData<'w> for Entity {
    type Slices = &'w [Entity];
    type Iter = Copied<slice::Iter<'w, Entity>>;

    #[inline]
    fn matches(_: &[TypeId]) -> bool {
        true
    }

    fn check_access() {}

    #[inline]
    fn slices_mut(columns: ColumnsMut<'w>) -> Self::Slices {
        columns.entities
    }

    #[inline]
    fn iter(slices: Self::Slices, _: usize) -> Self::Iter {
        slices.iter().copied()
    }
}

impl<'w> ReadOnlyQueryData<'w> for Entity {
    #[inline]
    fn slices(columns: Columns<'w>) -> Self::Slices {
        columns.entities
    }
}

#[cfg(test)]
#[path = "../tests/ecs/query.rs"]
mod tests;
