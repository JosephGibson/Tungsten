use std::any::{TypeId, type_name};
use std::collections::HashSet;

use super::bundle::{Bundle, BundleScratch, BundleSink};
use super::command_buffer::{Command, CommandBuffer, CommandTarget};
use super::entity::Entity;
use super::event_queue::EventQueue;
use super::query::{Columns, ColumnsMut, QueryData, QueryFilter, ReadOnlyQueryData};
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
    /// One deriver per type a spawn from a source takes, in registration
    /// order ([`inherit_from_source`](Self::inherit_from_source)); each
    /// stages its value for the target in `bundle_scratch`.
    source_derivers: Vec<fn(&mut World, Entity, Entity)>,
    source_types: HashSet<TypeId>,
    /// Staging for [`spawn_with`](Self::spawn_with) and
    /// [`insert_bundle`](Self::insert_bundle).
    bundle_scratch: BundleScratch,
}

/// The function a spawn from a source takes `T` through, a resource of the
/// World's own, which no other crate can name.
struct SourceDerive<T>(fn(&World, Entity) -> Option<T>);

/// What [`World::inherit_from_source`] derives: a clone of the source's `T`.
fn clone_from_source<T: Clone + 'static>(world: &World, source: Entity) -> Option<T> {
    world.get::<T>(source).cloned()
}

/// Stages `T` for `target` from `source` when `target` lacks one and the
/// registered function gives one.
fn derive_onto<T: 'static>(world: &mut World, source: Entity, target: Entity) {
    if world.has::<T>(target) {
        return;
    }
    let Some(&SourceDerive(derive)) = world.get_resource::<SourceDerive<T>>() else {
        return;
    };
    if let Some(value) = derive(world, source) {
        world.bundle_scratch.put(value);
    }
}

/// What [`World::set_fixed_event_view`], [`World::end_fixed_step_events`]
/// and [`World::hold_events_for_fixed_step`] do to each queue.
#[derive(Clone, Copy)]
enum EventStep {
    View(bool),
    End,
    Hold,
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
            EventStep::Hold => queue.hold_for_fixed_step(),
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
            source_derivers: Vec::new(),
            source_types: HashSet::new(),
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

    /// Marks a frame that runs no fixed step: every registered event
    /// queue's next [`flush_events`](Self::flush_events) holds the events
    /// no step has read for the next frame's first step instead of dropping
    /// them ([`EventQueue`], `D-139`). The app calls it in such a frame
    /// while the game clock advances and `fixed_update` has a system; a
    /// flush in a frame that did not call it drops what was held.
    pub fn hold_events_for_fixed_step(&mut self) {
        for i in 0..self.event_steppers.len() {
            (self.event_steppers[i])(self, EventStep::Hold);
        }
    }

    /// A spawn from a source
    /// ([`CommandBuffer::spawn_prefab_from`](crate::ecs::CommandBuffer::spawn_prefab_from))
    /// takes a clone of the source's `T` when the source has one and the
    /// spawn does not. Idempotent per type: the first registration of `T`,
    /// this or [`derive_from_source`](Self::derive_from_source), stands.
    /// Returns whether `T` was new.
    pub fn inherit_from_source<T: Clone + 'static>(&mut self) -> bool {
        self.derive_from_source::<T>(clone_from_source::<T>)
    }

    /// A spawn from a source takes `derive(world, source)` when that returns
    /// a value and the spawn has no `T`: what a spawn takes that is not a
    /// copy, such as the first entity of a spawn chain. Idempotent per type,
    /// as [`inherit_from_source`](Self::inherit_from_source).
    pub fn derive_from_source<T: 'static>(
        &mut self,
        derive: fn(&World, Entity) -> Option<T>,
    ) -> bool {
        if !self.source_types.insert(TypeId::of::<T>()) {
            return false;
        }
        self.resources.insert(SourceDerive(derive));
        self.source_derivers.push(derive_onto::<T>);
        true
    }

    /// Gives `target` what the spawn inheritance derives from `source`, in
    /// one archetype move after its own components, when anything is given.
    fn derive_from(&mut self, source: Entity, target: Entity) {
        self.bundle_scratch.reset();
        for i in 0..self.source_derivers.len() {
            (self.source_derivers[i])(self, source, target);
        }
        if !self.bundle_scratch.run.is_empty() {
            self.apply_bundle(target);
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

    /// The type and [`type_name`] of each component `entity` holds, sorted
    /// by name; empty for a dead entity.
    ///
    /// An introspection for tests and tools, such as a save's coverage test.
    /// The names are the compiler's spelling, which no release promises, and
    /// no system or frame calls it.
    #[must_use]
    pub fn component_types(&self, entity: Entity) -> Vec<(TypeId, &'static str)> {
        let Some(location) = self.archetypes.entities.get(entity) else {
            return Vec::new();
        };
        let archetype = &self.archetypes.archetypes[location.archetype_id as usize];
        let mut types: Vec<(TypeId, &'static str)> = archetype
            .component_types
            .iter()
            .zip(&archetype.columns)
            .map(|(&type_id, column)| (type_id, column.type_name()))
            .collect();
        types.sort_by_key(|&(_, name)| name);
        types
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
        // The matching archetypes' slices first, so the iterator a query
        // returns is a vector's and its rows' loops stay as tight as before.
        let matched = self.archetypes.matching(Q::matches, F::matches);
        let parts: Vec<(usize, Q::Slices)> = self
            .archetypes
            .archetypes
            .iter()
            .enumerate()
            .filter(|&(index, _)| matched[index / 64] >> (index % 64) & 1 == 1)
            .map(|(_, arch)| (arch.entities.len(), Q::slices(Columns::of(arch))))
            .collect();
        parts.into_iter()
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
        let matched = self.archetypes.matching(Q::matches, F::matches);
        let parts: Vec<(usize, Q::Slices)> = self
            .archetypes
            .archetypes
            .iter_mut()
            .enumerate()
            .filter(|&(index, _)| matched[index / 64] >> (index % 64) & 1 == 1)
            .map(|(_, arch)| {
                let rows = arch.entities.len();
                (rows, Q::slices_mut(ColumnsMut::of(arch)))
            })
            .collect();
        parts.into_iter()
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
            epoch,
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
                        CommandTarget::Live(entity) => entity,
                        CommandTarget::Pending(id) => pending_entities[id as usize],
                    };
                    // A despawn or an earlier `call` can kill either kind,
                    // a pending spawn included: its values drop in order.
                    if !self.is_alive(entity) {
                        run.for_each(|queue| values.drop_next(queue));
                        continue;
                    }
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
                Command::Derive { source, target } => {
                    index += 1;
                    let entity = match target {
                        CommandTarget::Live(entity) => entity,
                        CommandTarget::Pending(id) => pending_entities[id as usize],
                    };
                    if !self.is_alive(entity) {
                        continue;
                    }
                    // A source an earlier command despawned drops the spawn
                    // whole: its run has applied, so nothing of it remains.
                    if self.is_alive(source) {
                        self.derive_from(source, entity);
                    } else {
                        self.despawn(entity);
                    }
                }
            }
        }

        commands.clear();
        *pending_count = 0;
        *epoch = epoch.wrapping_add(1);
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

#[cfg(test)]
#[path = "../tests/ecs/world.rs"]
mod tests;
