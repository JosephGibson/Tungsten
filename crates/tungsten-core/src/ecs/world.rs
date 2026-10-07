use std::any::{TypeId, type_name};
use std::collections::HashSet;

use super::archetype::{AnyColumn, Archetype, TypedVec};
use super::bundle::{Bundle, BundleScratch};
use super::command_buffer::{Command, CommandBuffer, CommandTarget};
use super::entity::Entity;
use super::event_queue::EventQueue;
use super::query::{
    Columns, ColumnsMut, OptionalColumn, QueryData, QueryFilter, ReadOnlyQueryData,
};
use super::resource::ResourceMap;
use super::storage::Archetypes;

/// Entity/component/resource container.
///
/// D-036: archetypal storage with contiguous columns and edge-cached transitions.
pub struct World {
    pub(super) archetypes: Archetypes,
    resources: ResourceMap,
    /// One flusher per event type `register_event` registered, in order;
    /// `flush_events` runs them once per frame (`D-040`).
    event_flushers: Vec<fn(&mut World)>,
    /// Beside each flusher, the fixed steps' view switch for the same queue.
    event_steppers: Vec<fn(&mut World, EventStep)>,
    event_types: HashSet<TypeId>,
    /// Staging for [`spawn_with`](Self::spawn_with) and
    /// [`insert_bundle`](Self::insert_bundle).
    bundle_scratch: BundleScratch,
}

/// What [`World::set_fixed_event_view`] and [`World::end_fixed_step_events`]
/// do to each queue.
#[derive(Clone, Copy)]
enum EventStep {
    View(bool),
    End,
}

fn flush_event_queue<T: 'static>(world: &mut World) {
    if let Some(queue) = world.get_resource_mut::<EventQueue<T>>() {
        queue.flush();
    }
}

fn step_event_queue<T: 'static>(world: &mut World, step: EventStep) {
    if let Some(queue) = world.get_resource_mut::<EventQueue<T>>() {
        match step {
            EventStep::View(on) => queue.set_fixed_view(on),
            EventStep::End => queue.end_fixed_step(),
        }
    }
}

impl World {
    #[must_use]
    pub fn new() -> Self {
        Self {
            archetypes: Archetypes::new(),
            resources: ResourceMap::new(),
            event_flushers: Vec::new(),
            event_steppers: Vec::new(),
            event_types: HashSet::new(),
            bundle_scratch: BundleScratch::default(),
        }
    }

    /// Registers `EventQueue<T>` as a resource that [`World::flush_events`]
    /// rotates once per frame. Idempotent; returns whether `T` was new.
    pub fn register_event<T: 'static>(&mut self) -> bool {
        if !self.event_types.insert(TypeId::of::<T>()) {
            return false;
        }
        self.insert_resource(EventQueue::<T>::new());
        self.event_flushers.push(flush_event_queue::<T>);
        self.event_steppers.push(step_event_queue::<T>);
        true
    }

    /// Whether `register_event::<T>` has run.
    #[must_use]
    pub fn has_event<T: 'static>(&self) -> bool {
        self.event_types.contains(&TypeId::of::<T>())
    }

    /// How many event types are registered.
    #[must_use]
    pub fn registered_event_count(&self) -> usize {
        self.event_flushers.len()
    }

    /// Rotates every registered event queue: the frame's events become the
    /// previous window, the previous window is dropped (`D-040`).
    pub fn flush_events(&mut self) {
        for i in 0..self.event_flushers.len() {
            (self.event_flushers[i])(self);
        }
    }

    /// Turns every registered event queue's step view on or off. The app
    /// turns it on for the frame's fixed steps and off after the last, so a
    /// step after the first reads only the events sent since the step before
    /// it ended ([`EventQueue`]).
    pub fn set_fixed_event_view(&mut self, on: bool) {
        for i in 0..self.event_steppers.len() {
            (self.event_steppers[i])(self, EventStep::View(on));
        }
    }

    /// Ends a fixed step in every registered event queue: the next step's
    /// readers start after the events sent so far. The app calls it after
    /// each step; [`flush_events`](Self::flush_events) starts the next frame
    /// over.
    pub fn end_fixed_step_events(&mut self) {
        for i in 0..self.event_steppers.len() {
            (self.event_steppers[i])(self, EventStep::End);
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

    /// Spawn an entity with `bundle`'s components, moved into their
    /// archetype in one step: `world.spawn_with((Player, transform, sprite))`.
    /// The entity ends as a run of [`insert`](Self::insert)s would leave it,
    /// so a type the bundle names twice keeps the later value
    /// ([`Bundle`](super::Bundle)).
    pub fn spawn_with<B: Bundle>(&mut self, bundle: B) -> Entity {
        self.stage_bundle(bundle);
        let entity = self.archetypes.spawn();
        self.apply_bundle(entity);
        entity
    }

    /// Insert `bundle`'s components on `entity` in one archetype move,
    /// overwriting the types it already has, as a run of
    /// [`insert`](Self::insert)s would.
    ///
    /// # Panics
    /// Panics if `entity` is dead (D-022), as `insert` does.
    pub fn insert_bundle<B: Bundle>(&mut self, entity: Entity, bundle: B) {
        self.stage_bundle(bundle);
        self.apply_bundle(entity);
    }

    /// Stage `bundle`'s values in the scratch. The World itself is untouched
    /// until [`apply_bundle`](Self::apply_bundle), so a panic in the
    /// bundle's `put` leaves it as it was.
    fn stage_bundle<B: Bundle>(&mut self, bundle: B) {
        self.bundle_scratch.reset();
        bundle.put(&mut self.bundle_scratch);
    }

    /// Apply the staged bundle to `entity` as one archetype move (D-084).
    fn apply_bundle(&mut self, entity: Entity) {
        let BundleScratch { values, run } = &mut self.bundle_scratch;
        self.archetypes
            .insert_run(entity, run.iter().copied(), values);
        run.clear();
    }

    pub fn remove_component<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        self.archetypes.remove::<T>(entity)
    }

    #[must_use]
    #[inline]
    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.archetypes.get::<T>(entity)
    }

    #[inline]
    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.archetypes.get_mut::<T>(entity)
    }

    #[must_use]
    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        self.archetypes.has::<T>(entity)
    }

    /// Rows matching the query data `Q`, read-only: one item (`&A`,
    /// `Option<&A>`, `Entity`) or a tuple of up to eight, as in
    /// `world.query::<(Entity, &A, Option<&B>)>()`. Rows come in archetype
    /// order, then row order ([`query`](super::query)).
    pub fn query<'w, Q: ReadOnlyQueryData<'w>>(&'w self) -> impl Iterator<Item = Q> {
        self.query_filtered::<Q, ()>()
    }

    /// [`query`](Self::query) over the archetypes that pass `F`: `With<T>`,
    /// `Without<T>`, `()` or a tuple of up to four of them.
    pub fn query_filtered<'w, Q: ReadOnlyQueryData<'w>, F: QueryFilter>(
        &'w self,
    ) -> impl Iterator<Item = Q> {
        self.query_slices_filtered::<Q, F>()
            .flat_map(|(rows, slices)| Q::iter(slices, rows))
    }

    /// The row count and `Q`'s column slices of each matching archetype, for
    /// a pass over whole columns.
    pub fn query_slices<'w, Q: ReadOnlyQueryData<'w>>(
        &'w self,
    ) -> impl Iterator<Item = (usize, Q::Slices)> {
        self.query_slices_filtered::<Q, ()>()
    }

    /// [`query_slices`](Self::query_slices) over the archetypes that pass
    /// `F`.
    pub fn query_slices_filtered<'w, Q: ReadOnlyQueryData<'w>, F: QueryFilter>(
        &'w self,
    ) -> impl Iterator<Item = (usize, Q::Slices)> {
        self.archetypes.archetypes.iter().filter_map(|arch| {
            if Q::matches(&arch.component_types) && F::matches(&arch.component_types) {
                Some((arch.entities.len(), Q::slices(Columns::of(arch))))
            } else {
                None
            }
        })
    }

    /// Rows matching the query data `Q`, which may write: `&mut A` or
    /// `Option<&mut A>` alone or in a tuple with the read-only items, as in
    /// `world.query_mut::<(&mut A, &B, Option<&mut C>)>()`. Each column is
    /// borrowed once per archetype.
    ///
    /// # Panics
    /// Panics, naming `Q`, if two items name the same component.
    pub fn query_mut<'w, Q: QueryData<'w>>(&'w mut self) -> impl Iterator<Item = Q> {
        self.query_mut_filtered::<Q, ()>()
    }

    /// [`query_mut`](Self::query_mut) over the archetypes that pass `F`.
    ///
    /// # Panics
    /// Panics, naming `Q`, if two items name the same component.
    pub fn query_mut_filtered<'w, Q: QueryData<'w>, F: QueryFilter>(
        &'w mut self,
    ) -> impl Iterator<Item = Q> {
        self.query_mut_slices_filtered::<Q, F>()
            .flat_map(|(rows, slices)| Q::iter(slices, rows))
    }

    /// Mutable counterpart of [`query_slices`](Self::query_slices).
    ///
    /// # Panics
    /// Panics, naming `Q`, if two items name the same component.
    pub fn query_mut_slices<'w, Q: QueryData<'w>>(
        &'w mut self,
    ) -> impl Iterator<Item = (usize, Q::Slices)> {
        self.query_mut_slices_filtered::<Q, ()>()
    }

    /// [`query_mut_slices`](Self::query_mut_slices) over the archetypes that
    /// pass `F`.
    ///
    /// # Panics
    /// Panics, naming `Q`, if two items name the same component.
    pub fn query_mut_slices_filtered<'w, Q: QueryData<'w>, F: QueryFilter>(
        &'w mut self,
    ) -> impl Iterator<Item = (usize, Q::Slices)> {
        Q::check_access();
        self.archetypes.archetypes.iter_mut().filter_map(|arch| {
            if Q::matches(&arch.component_types) && F::matches(&arch.component_types) {
                let rows = arch.entities.len();
                Some((rows, Q::slices_mut(ColumnsMut::of(arch))))
            } else {
                None
            }
        })
    }

    /// Collect entities with `T`; use before mixed mutable access.
    #[must_use]
    #[deprecated(
        since = "0.54.0",
        note = "use `query_filtered::<Entity, With<T>>()` and collect"
    )]
    pub fn query_entities<T: 'static>(&self) -> Vec<Entity> {
        self.archetypes
            .archetypes_with::<T>()
            .flat_map(|(arch, _)| arch.entities.iter().copied())
            .collect()
    }

    /// Immutable two-component query; one downcast per archetype/type.
    #[deprecated(since = "0.54.0", note = "use `query::<(Entity, &A, &B)>()`")]
    pub fn query2<A: 'static, B: 'static>(&self) -> impl Iterator<Item = (Entity, &A, &B)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        self.archetypes
            .archetypes_with_two(a_id, b_id)
            .flat_map(|arch| {
                let col_a = arch.typed_column::<A>().unwrap();
                let col_b = arch.typed_column::<B>().unwrap();
                arch.entities
                    .iter()
                    .zip(col_a.0.iter())
                    .zip(col_b.0.iter())
                    .map(|((&e, a), b)| (e, a, b))
            })
    }

    /// Collect entities with `A` and `B`; use before mutable access.
    #[must_use]
    #[deprecated(
        since = "0.54.0",
        note = "use `query_filtered::<Entity, (With<A>, With<B>)>()` and collect"
    )]
    pub fn query2_entities<A: 'static, B: 'static>(&self) -> Vec<Entity> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        self.archetypes
            .archetypes_with_two(a_id, b_id)
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    /// Immutable three-component query.
    #[deprecated(since = "0.54.0", note = "use `query::<(Entity, &A, &B, &C)>()`")]
    pub fn query3<A: 'static, B: 'static, C: 'static>(
        &self,
    ) -> impl Iterator<Item = (Entity, &A, &B, &C)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        self.archetypes
            .archetypes_with_three(a_id, b_id, c_id)
            .flat_map(|arch| {
                let col_a = arch.typed_column::<A>().unwrap();
                let col_b = arch.typed_column::<B>().unwrap();
                let col_c = arch.typed_column::<C>().unwrap();
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
    #[deprecated(
        since = "0.54.0",
        note = "use `query::<(Entity, &A, &B, Option<&C>, Option<&D>)>()`"
    )]
    pub fn query2_opt2<A: 'static, B: 'static, C: 'static, D: 'static>(
        &self,
    ) -> impl Iterator<Item = (Entity, &A, &B, Option<&C>, Option<&D>)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        self.archetypes
            .archetypes_with_two(a_id, b_id)
            .flat_map(|arch| {
                let col_a = arch.typed_column::<A>().unwrap();
                let col_b = arch.typed_column::<B>().unwrap();
                let col_c = arch.typed_column::<C>();
                let col_d = arch.typed_column::<D>();
                arch.entities
                    .iter()
                    .zip(col_a.0.iter())
                    .zip(col_b.0.iter())
                    .zip(OptionalColumn(col_c.map(|col| col.0.iter())))
                    .zip(OptionalColumn(col_d.map(|col| col.0.iter())))
                    .map(|((((&e, a), b), c), d)| (e, a, b, c, d))
            })
    }

    /// Immutable three-required + two-optional component query, the
    /// three-column sibling of [`query2_opt2`](Self::query2_opt2): optional
    /// column presence resolves once per archetype, so rows need no
    /// per-entity lookups. Iterates the same archetype set in the same order
    /// as `query3::<A, B, C>`.
    #[allow(clippy::type_complexity)]
    #[deprecated(
        since = "0.54.0",
        note = "use `query::<(Entity, &A, &B, &C, Option<&D>, Option<&E>)>()`"
    )]
    pub fn query3_opt2<A: 'static, B: 'static, C: 'static, D: 'static, E: 'static>(
        &self,
    ) -> impl Iterator<Item = (Entity, &A, &B, &C, Option<&D>, Option<&E>)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        self.archetypes
            .archetypes_with_three(a_id, b_id, c_id)
            .flat_map(|arch| {
                let col_a = arch.typed_column::<A>().unwrap();
                let col_b = arch.typed_column::<B>().unwrap();
                let col_c = arch.typed_column::<C>().unwrap();
                let col_d = arch.typed_column::<D>();
                let col_e = arch.typed_column::<E>();
                arch.entities
                    .iter()
                    .zip(col_a.0.iter())
                    .zip(col_b.0.iter())
                    .zip(col_c.0.iter())
                    .zip(OptionalColumn(col_d.map(|col| col.0.iter())))
                    .zip(OptionalColumn(col_e.map(|col| col.0.iter())))
                    .map(|(((((&e, a), b), c), d), opt_e)| (e, a, b, c, d, opt_e))
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
    #[deprecated(
        since = "0.54.0",
        note = "use `query_mut::<(Entity, &A, &mut B, Option<&C>, Option<&mut D>)>()`"
    )]
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
                    component_types,
                    columns,
                    entities,
                    ..
                } = arch;
                let (col_a, col_b, col_c, col_d) = split2_opt2_columns_mut::<A, B, C, D>(
                    component_types,
                    columns,
                    a_id,
                    b_id,
                    c_id,
                    d_id,
                );
                entities
                    .iter()
                    .zip(col_a.0.iter())
                    .zip(col_b.0.iter_mut())
                    .zip(OptionalColumn(col_c.map(|col| col.0.iter())))
                    .zip(OptionalColumn(col_d.map(|col| col.0.iter_mut())))
                    .map(|((((&e, a), b), c), d)| (e, a, b, c, d))
            })
    }

    /// Mutable two-component query; per-archetype split column borrows (D-036).
    ///
    /// Both refs are mutable; distinct `TypeId`s guarantee disjoint columns.
    ///
    /// # Panics
    /// Panics if `A` and `B` are the same type.
    #[deprecated(
        since = "0.54.0",
        note = "use `query_mut::<(Entity, &mut A, &mut B)>()`"
    )]
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
                    component_types,
                    columns,
                    entities,
                    ..
                } = arch;
                let (col_a, col_b) =
                    split2_columns_mut::<A, B>(component_types, columns, a_id, b_id);
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
    #[deprecated(
        since = "0.54.0",
        note = "use `query_mut::<(Entity, &mut A, &mut B, &mut C)>()`"
    )]
    pub fn query3_mut<A: 'static, B: 'static, C: 'static>(
        &mut self,
    ) -> impl Iterator<Item = (Entity, &mut A, &mut B, &mut C)> {
        self.query3_mut_excluding::<A, B, C>(None)
    }

    /// [`query3_mut`](Self::query3_mut) over archetypes that lack `X`. The
    /// exclusion resolves once per archetype, so rows need no per-entity
    /// `get::<X>` lookups (the columnar replacement for gather-then-lookup
    /// loops, like `query2_opt2` in D-066).
    ///
    /// # Panics
    /// Panics if any two of `A`, `B`, `C`, `X` are the same type.
    #[deprecated(
        since = "0.54.0",
        note = "use `query_mut_filtered::<(Entity, &mut A, &mut B, &mut C), Without<X>>()`"
    )]
    pub fn query3_mut_without<A: 'static, B: 'static, C: 'static, X: 'static>(
        &mut self,
    ) -> impl Iterator<Item = (Entity, &mut A, &mut B, &mut C)> {
        let x_id = TypeId::of::<X>();
        for id in [TypeId::of::<A>(), TypeId::of::<B>(), TypeId::of::<C>()] {
            assert_ne!(
                id, x_id,
                "query3_mut_without: excluded type must differ from queried types"
            );
        }
        self.query3_mut_excluding::<A, B, C>(Some(x_id))
    }

    fn query3_mut_excluding<A: 'static, B: 'static, C: 'static>(
        &mut self,
        exclude: Option<TypeId>,
    ) -> impl Iterator<Item = (Entity, &mut A, &mut B, &mut C)> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        assert_ne!(a_id, b_id, "query3_mut: component types must be distinct");
        assert_ne!(a_id, c_id, "query3_mut: component types must be distinct");
        assert_ne!(b_id, c_id, "query3_mut: component types must be distinct");
        self.archetypes
            .archetypes_with_three_mut(a_id, b_id, c_id)
            .filter(move |arch| exclude.is_none_or(|x_id| !arch.has(x_id)))
            .flat_map(move |arch| {
                let Archetype {
                    component_types,
                    columns,
                    entities,
                    ..
                } = arch;
                let (col_a, col_b, col_c) =
                    split3_columns_mut::<A, B, C>(component_types, columns, a_id, b_id, c_id);
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
    #[deprecated(
        since = "0.54.0",
        note = "use `query_filtered::<Entity, (With<A>, With<B>, With<C>)>()` and collect"
    )]
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

    /// The resource `T`, for one the World always holds (what `App`
    /// inserts); [`get_resource`](Self::get_resource) is the `Option` form.
    ///
    /// # Panics
    /// Panics, naming `T`, if the World has no `T`.
    #[must_use]
    #[track_caller]
    pub fn resource<T: 'static>(&self) -> &T {
        match self.resources.get::<T>() {
            Some(resource) => resource,
            None => missing_resource::<T>(),
        }
    }

    /// Mutable counterpart of [`resource`](Self::resource);
    /// [`get_resource_mut`](Self::get_resource_mut) is the `Option` form.
    ///
    /// # Panics
    /// Panics, naming `T`, if the World has no `T`.
    #[track_caller]
    pub fn resource_mut<T: 'static>(&mut self) -> &mut T {
        match self.resources.get_mut::<T>() {
            Some(resource) => resource,
            None => missing_resource::<T>(),
        }
    }

    #[must_use]
    pub fn has_resource<T: 'static>(&self) -> bool {
        self.resources.contains::<T>()
    }

    pub fn remove_resource<T: 'static>(&mut self) -> Option<T> {
        self.resources.remove::<T>()
    }

    /// Flush queued commands: allocate pending spawns, then replay mutations in order.
    ///
    /// Consecutive inserts on one entity apply as one archetype move (D-084).
    pub fn flush(&mut self, mut buffer: CommandBuffer) {
        self.flush_reusing(&mut buffer);
    }

    /// [`flush`](Self::flush) for a buffer the caller keeps: it comes back
    /// empty with its storage, so recording into it again does not allocate.
    pub fn flush_reusing(&mut self, buffer: &mut CommandBuffer) {
        let CommandBuffer {
            commands,
            pending_count,
            values,
            pending_entities,
        } = buffer;

        pending_entities.clear();
        pending_entities.reserve(*pending_count as usize);
        for cmd in commands.iter() {
            if let Command::Spawn { pending_id } = cmd {
                debug_assert_eq!(
                    *pending_id as usize,
                    pending_entities.len(),
                    "pending_id must be allocated sequentially"
                );
                pending_entities.push(self.spawn());
            }
        }

        let mut index = 0;
        while index < commands.len() {
            match commands[index] {
                Command::Spawn { .. } => index += 1,
                Command::Insert { target, .. } => {
                    // The run: this insert and the ones that follow it
                    // directly on the same target.
                    let mut end = index + 1;
                    while commands
                        .get(end)
                        .is_some_and(|next| next.inserts_on(target))
                    {
                        end += 1;
                    }
                    let run = commands[index..end].iter().map(|cmd| match cmd {
                        Command::Insert { queue, .. } => *queue,
                        _ => unreachable!("a run holds insert commands only"),
                    });
                    index = end;
                    let entity = match target {
                        CommandTarget::Live(entity) => {
                            if !self.is_alive(entity) {
                                run.for_each(|queue| values.drop_next(queue));
                                continue;
                            }
                            entity
                        }
                        CommandTarget::Pending(id) => pending_entities[id as usize],
                    };
                    self.archetypes.insert_run(entity, run, values);
                }
                Command::Call { entity, call } => {
                    call(self, entity);
                    index += 1;
                }
                Command::Despawn(entity) => {
                    if self.is_alive(entity) {
                        self.despawn(entity);
                    }
                    index += 1;
                }
            }
        }

        commands.clear();
        *pending_count = 0;
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

/// The panic of [`World::resource`] and [`World::resource_mut`], out of
/// line so the accessors stay small.
#[cold]
#[track_caller]
fn missing_resource<T>() -> ! {
    panic!("resource `{}` is not in the World", type_name::<T>())
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
///
/// `types` is the archetype's sorted type key and `columns` its columns in
/// the same order, as for every helper below.
fn split2_opt2_columns_mut<'c, A: 'static, B: 'static, C: 'static, D: 'static>(
    types: &[TypeId],
    columns: &'c mut [Box<dyn AnyColumn>],
    a_id: TypeId,
    b_id: TypeId,
    c_id: TypeId,
    d_id: TypeId,
) -> SplitOpt2Columns<'c, A, B, C, D> {
    let mut col_a = None;
    let mut col_b = None;
    let mut col_c = None;
    let mut col_d = None;
    for (&tid, col) in types.iter().zip(columns.iter_mut()) {
        if tid == a_id {
            col_a = (**col).typed::<A>();
        } else if tid == b_id {
            col_b = col.typed_mut::<B>();
        } else if tid == c_id {
            col_c = (**col).typed::<C>();
        } else if tid == d_id {
            col_d = col.typed_mut::<D>();
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
fn split2_columns_mut<'c, A: 'static, B: 'static>(
    types: &[TypeId],
    columns: &'c mut [Box<dyn AnyColumn>],
    a_id: TypeId,
    b_id: TypeId,
) -> (&'c mut TypedVec<A>, &'c mut TypedVec<B>) {
    let mut col_a = None;
    let mut col_b = None;
    for (&tid, col) in types.iter().zip(columns.iter_mut()) {
        if tid == a_id {
            col_a = col.typed_mut::<A>();
        } else if tid == b_id {
            col_b = col.typed_mut::<B>();
        }
    }
    (
        col_a.expect("split2_columns_mut: column A missing"),
        col_b.expect("split2_columns_mut: column B missing"),
    )
}

/// Disjoint mutable borrows of three typed columns; ids must differ.
fn split3_columns_mut<'c, A: 'static, B: 'static, C: 'static>(
    types: &[TypeId],
    columns: &'c mut [Box<dyn AnyColumn>],
    a_id: TypeId,
    b_id: TypeId,
    c_id: TypeId,
) -> (
    &'c mut TypedVec<A>,
    &'c mut TypedVec<B>,
    &'c mut TypedVec<C>,
) {
    let mut col_a = None;
    let mut col_b = None;
    let mut col_c = None;
    for (&tid, col) in types.iter().zip(columns.iter_mut()) {
        if tid == a_id {
            col_a = col.typed_mut::<A>();
        } else if tid == b_id {
            col_b = col.typed_mut::<B>();
        } else if tid == c_id {
            col_c = col.typed_mut::<C>();
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
