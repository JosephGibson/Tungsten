use std::any::TypeId;

use super::archetype::TypedVec;
use super::command_buffer::{Command, CommandBuffer, CommandTarget};
use super::entity::Entity;
use super::resource::ResourceMap;
use super::storage::Archetypes;

/// The World owns all entities, components, and resources.
///
/// The public API is identical to the pre-M12 naive implementation so that
/// all examples compile without changes. The storage engine behind it is now
/// an archetypal layout: contiguous `Vec<T>` columns per archetype, with O(1)
/// edge-cached archetype transitions on component add/remove (D-036).
pub struct World {
    archetypes: Archetypes,
    resources: ResourceMap,
}

impl World {
    pub fn new() -> Self {
        Self {
            archetypes: Archetypes::new(),
            resources: ResourceMap::new(),
        }
    }

    // ------------------------------------------------------------------
    // Entity lifecycle
    // ------------------------------------------------------------------

    /// Spawn a new entity. Starts in the empty archetype with no components.
    pub fn spawn(&mut self) -> Entity {
        self.archetypes.spawn()
    }

    /// Despawn an entity, removing all of its components.
    ///
    /// No-op if the entity is already dead, so that deferred `CommandBuffer`
    /// despawns remain idempotent when multiple systems target the same
    /// entity in a single frame.
    pub fn despawn(&mut self, entity: Entity) {
        if self.is_alive(entity) {
            self.archetypes.despawn(entity);
        }
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.archetypes.entities.is_alive(entity)
    }

    /// Number of currently live entities in the world. O(1).
    pub fn entity_count(&self) -> u32 {
        self.archetypes.entities.live_count()
    }

    // ------------------------------------------------------------------
    // Component access
    // ------------------------------------------------------------------

    /// Attach a component to an entity. Panics if the entity is not alive
    /// (programmer error per D-022).
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        self.archetypes.insert(entity, component);
    }

    pub fn remove_component<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        self.archetypes.remove::<T>(entity)
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.archetypes.get::<T>(entity)
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.archetypes.get_mut::<T>(entity)
    }

    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        self.archetypes.has::<T>(entity)
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Iterate all entities that have component type `T`.
    /// Returns `(Entity, &T)` pairs.
    ///
    /// Iteration order is archetype-stable: entities within an archetype are
    /// visited in row order (stable until a swap-remove occurs). Across
    /// archetypes the order follows the archetype creation order.
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

    /// Collect entity IDs that have component type `T`.
    ///
    /// Useful when you need to mutate components of multiple types in the
    /// same loop (collect entity IDs first, then call `get_mut` per entity).
    pub fn query_entities<T: 'static>(&self) -> Vec<Entity> {
        self.archetypes
            .archetypes_with::<T>()
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    /// Immutable 2-component query. Iterates all entities that have both `A`
    /// and `B`, visiting each archetype's contiguous column data directly.
    ///
    /// One downcast per archetype per type — not per element. This is the
    /// primary cache-friendly iteration path added in M12 (D-036 Expansion 1).
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

    /// Collect entity IDs that have both `A` and `B`.
    ///
    /// Use this with `get_mut` when mutation of either type is needed.
    pub fn query2_entities<A: 'static, B: 'static>(&self) -> Vec<Entity> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        self.archetypes
            .archetypes_with_two(a_id, b_id)
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    /// Immutable 3-component query. Iterates all entities that have `A`, `B`,
    /// and `C`.
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

    /// Collect entity IDs that have `A`, `B`, and `C`.
    ///
    /// Use this with `get_mut` when mutation is needed.
    pub fn query3_entities<A: 'static, B: 'static, C: 'static>(&self) -> Vec<Entity> {
        let a_id = TypeId::of::<A>();
        let b_id = TypeId::of::<B>();
        let c_id = TypeId::of::<C>();
        self.archetypes
            .archetypes_with_three(a_id, b_id, c_id)
            .flat_map(|arch| arch.entities.iter().copied())
            .collect()
    }

    // ------------------------------------------------------------------
    // Resources
    // ------------------------------------------------------------------

    pub fn insert_resource<T: 'static>(&mut self, resource: T) {
        self.resources.insert(resource);
    }

    pub fn get_resource<T: 'static>(&self) -> Option<&T> {
        self.resources.get::<T>()
    }

    pub fn get_resource_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.resources.get_mut::<T>()
    }

    pub fn has_resource<T: 'static>(&self) -> bool {
        self.resources.contains::<T>()
    }

    pub fn remove_resource<T: 'static>(&mut self) -> Option<T> {
        self.resources.remove::<T>()
    }

    /// Apply all queued commands in `buffer` to the world, then drop it.
    ///
    /// Pass 1 allocates real entities for every queued spawn, building a
    /// `pending_id -> Entity` table. Pass 2 replays all mutations in their
    /// original registration order.
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

// ---------------------------------------------------------------------------
// Tests — the 9 original pre-M12 tests, byte-for-byte unchanged
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::command_buffer::CommandBuffer;
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Velocity {
        dx: f32,
        dy: f32,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Name(String);

    #[test]
    fn spawn_and_check_alive() {
        let mut world = World::new();
        let e = world.spawn();
        assert!(world.is_alive(e));
    }

    #[test]
    fn despawn_removes_entity() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 1.0, y: 2.0 });
        world.despawn(e);
        assert!(!world.is_alive(e));
        assert!(world.get::<Position>(e).is_none());
    }

    #[test]
    fn insert_and_get_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 3.0, y: 4.0 });
        let pos = world.get::<Position>(e).unwrap();
        assert_eq!(pos.x, 3.0);
        assert_eq!(pos.y, 4.0);
    }

    #[test]
    fn get_mut_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 0.0, y: 0.0 });
        world.get_mut::<Position>(e).unwrap().x = 10.0;
        assert_eq!(world.get::<Position>(e).unwrap().x, 10.0);
    }

    #[test]
    fn query_iterates_matching_entities() {
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();
        let e3 = world.spawn();
        world.insert(e1, Position { x: 1.0, y: 0.0 });
        world.insert(e2, Position { x: 2.0, y: 0.0 });
        world.insert(e3, Name("no position".into()));

        let positions: Vec<_> = world.query::<Position>().collect();
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn query_entities_then_mutate() {
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();
        world.insert(e1, Position { x: 0.0, y: 0.0 });
        world.insert(e1, Velocity { dx: 1.0, dy: 2.0 });
        world.insert(e2, Position { x: 5.0, y: 5.0 });
        world.insert(e2, Velocity { dx: -1.0, dy: 0.0 });

        let entities = world.query_entities::<Velocity>();
        for entity in entities {
            let vel = world.get::<Velocity>(entity).unwrap().clone();
            let pos = world.get_mut::<Position>(entity).unwrap();
            pos.x += vel.dx;
            pos.y += vel.dy;
        }

        assert_eq!(world.get::<Position>(e1).unwrap().x, 1.0);
        assert_eq!(world.get::<Position>(e1).unwrap().y, 2.0);
        assert_eq!(world.get::<Position>(e2).unwrap().x, 4.0);
    }

    #[test]
    fn resources() {
        let mut world = World::new();

        #[derive(Debug, PartialEq)]
        struct DeltaTime(f32);

        world.insert_resource(DeltaTime(0.016));
        assert_eq!(world.get_resource::<DeltaTime>().unwrap().0, 0.016);

        world.get_resource_mut::<DeltaTime>().unwrap().0 = 0.033;
        assert_eq!(world.get_resource::<DeltaTime>().unwrap().0, 0.033);

        assert!(world.has_resource::<DeltaTime>());
        let dt = world.remove_resource::<DeltaTime>().unwrap();
        assert_eq!(dt.0, 0.033);
        assert!(!world.has_resource::<DeltaTime>());
    }

    #[test]
    fn remove_component() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 1.0, y: 2.0 });
        let removed = world.remove_component::<Position>(e).unwrap();
        assert_eq!(removed, Position { x: 1.0, y: 2.0 });
        assert!(world.get::<Position>(e).is_none());
    }

    #[test]
    #[should_panic(expected = "insert on dead entity")]
    fn insert_on_dead_entity_panics() {
        let mut world = World::new();
        let e = world.spawn();
        world.despawn(e);
        world.insert(e, Position { x: 0.0, y: 0.0 });
    }

    #[test]
    fn multiple_component_types() {
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 1.0, y: 2.0 });
        world.insert(e, Velocity { dx: 3.0, dy: 4.0 });
        world.insert(e, Name("player".into()));

        assert!(world.has::<Position>(e));
        assert!(world.has::<Velocity>(e));
        assert!(world.has::<Name>(e));
    }

    // ------------------------------------------------------------------
    // New M12 tests — multi-component queries
    // ------------------------------------------------------------------

    #[test]
    fn query2_returns_matching_entities() {
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();
        let e3 = world.spawn();

        world.insert(e1, Position { x: 1.0, y: 0.0 });
        world.insert(e1, Velocity { dx: 1.0, dy: 0.0 });

        world.insert(e2, Position { x: 2.0, y: 0.0 });
        // e2 has no Velocity

        world.insert(e3, Position { x: 3.0, y: 0.0 });
        world.insert(e3, Velocity { dx: 3.0, dy: 0.0 });

        let results: Vec<_> = world.query2::<Position, Velocity>().collect();
        assert_eq!(results.len(), 2);
        // e2 must not appear.
        assert!(results.iter().all(|(e, _, _)| *e != e2));
    }

    #[test]
    fn query2_includes_supersets() {
        // An entity with {Position, Velocity, Name} should appear in
        // query2::<Position, Velocity>().
        let mut world = World::new();
        let e = world.spawn();
        world.insert(e, Position { x: 0.0, y: 0.0 });
        world.insert(e, Velocity { dx: 1.0, dy: 2.0 });
        world.insert(e, Name("full".into()));

        let results: Vec<_> = world.query2::<Position, Velocity>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e);
    }

    #[test]
    fn query2_entities_then_mutate() {
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();
        world.insert(e1, Position { x: 0.0, y: 0.0 });
        world.insert(e1, Velocity { dx: 1.0, dy: 2.0 });
        world.insert(e2, Position { x: 5.0, y: 5.0 });
        world.insert(e2, Velocity { dx: -1.0, dy: 0.0 });

        let entities = world.query2_entities::<Position, Velocity>();
        for entity in entities {
            let vel = world.get::<Velocity>(entity).unwrap().clone();
            let pos = world.get_mut::<Position>(entity).unwrap();
            pos.x += vel.dx;
            pos.y += vel.dy;
        }

        assert_eq!(world.get::<Position>(e1).unwrap().x, 1.0);
        assert_eq!(world.get::<Position>(e2).unwrap().x, 4.0);
    }

    #[test]
    fn query3_returns_three_component_entities() {
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();

        world.insert(e1, Position { x: 1.0, y: 0.0 });
        world.insert(e1, Velocity { dx: 1.0, dy: 0.0 });
        world.insert(e1, Name("full".into()));

        // e2 only has Position + Velocity, no Name.
        world.insert(e2, Position { x: 2.0, y: 0.0 });
        world.insert(e2, Velocity { dx: 2.0, dy: 0.0 });

        let results: Vec<_> = world.query3::<Position, Velocity, Name>().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, e1);
    }

    #[test]
    fn query_across_multiple_archetypes() {
        // Entities spread across 3 different archetypes all contribute to
        // a query for Position.
        let mut world = World::new();
        let e1 = world.spawn();
        let e2 = world.spawn();
        let e3 = world.spawn();

        world.insert(e1, Position { x: 1.0, y: 0.0 }); // archetype {Pos}
        world.insert(e2, Position { x: 2.0, y: 0.0 });
        world.insert(e2, Velocity { dx: 0.0, dy: 0.0 }); // archetype {Pos, Vel}
        world.insert(e3, Position { x: 3.0, y: 0.0 });
        world.insert(e3, Velocity { dx: 0.0, dy: 0.0 });
        world.insert(e3, Name("three".into())); // archetype {Pos, Vel, Name}

        let positions: Vec<_> = world.query::<Position>().collect();
        assert_eq!(positions.len(), 3);
    }

    #[test]
    fn flush_spawn_entity_is_alive() {
        let mut world = World::new();
        let mut buffer = CommandBuffer::new();
        let pending = buffer.spawn();
        buffer.insert_pending(pending, Name("spawned".into()));

        world.flush(buffer);

        let results: Vec<_> = world.query::<Name>().collect();
        assert_eq!(results.len(), 1);
        assert!(world.is_alive(results[0].0));
    }

    #[test]
    fn flush_spawn_insert_pending_components_visible() {
        let mut world = World::new();
        let mut buffer = CommandBuffer::new();
        let pending = buffer.spawn();
        buffer.insert_pending(pending, Position { x: 1.0, y: 2.0 });
        buffer.insert_pending(pending, Velocity { dx: 3.0, dy: 4.0 });

        world.flush(buffer);

        let results: Vec<_> = world.query2::<Position, Velocity>().collect();
        assert_eq!(results.len(), 1);
        let (_, position, velocity) = results[0];
        assert_eq!(*position, Position { x: 1.0, y: 2.0 });
        assert_eq!(*velocity, Velocity { dx: 3.0, dy: 4.0 });
    }

    #[test]
    fn flush_insert_live_entity() {
        let mut world = World::new();
        let entity = world.spawn();
        let mut buffer = CommandBuffer::new();

        buffer.insert(entity, Name("live".into()));
        world.flush(buffer);

        assert_eq!(world.get::<Name>(entity).unwrap(), &Name("live".into()));
    }

    #[test]
    fn flush_despawn_entity_is_dead() {
        let mut world = World::new();
        let entity = world.spawn();
        let mut buffer = CommandBuffer::new();

        buffer.despawn(entity);
        world.flush(buffer);

        assert!(!world.is_alive(entity));
    }

    #[test]
    fn flush_remove_component() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Position { x: 1.0, y: 2.0 });
        let mut buffer = CommandBuffer::new();

        buffer.remove_component::<Position>(entity);
        world.flush(buffer);

        assert!(world.get::<Position>(entity).is_none());
    }

    #[test]
    fn flush_command_order_preserved() {
        let mut world = World::new();
        let entity = world.spawn();
        let mut buffer = CommandBuffer::new();

        buffer.insert(entity, Position { x: 1.0, y: 2.0 });
        buffer.insert(entity, Velocity { dx: 3.0, dy: 4.0 });
        buffer.remove_component::<Position>(entity);
        world.flush(buffer);

        assert!(world.get::<Position>(entity).is_none());
        assert_eq!(
            world.get::<Velocity>(entity),
            Some(&Velocity { dx: 3.0, dy: 4.0 })
        );
    }

    #[test]
    fn flush_despawn_dead_entity_is_silent() {
        let mut world = World::new();
        let entity = world.spawn();
        world.despawn(entity);
        let mut buffer = CommandBuffer::new();

        buffer.despawn(entity);
        world.flush(buffer);

        assert!(!world.is_alive(entity));
    }

    #[test]
    fn flush_insert_skips_entity_despawned_earlier_in_same_buffer() {
        let mut world = World::new();
        let entity = world.spawn();
        let mut buffer = CommandBuffer::new();

        buffer.despawn(entity);
        buffer.insert(entity, Name("late".into()));
        world.flush(buffer);

        assert!(!world.is_alive(entity));
        assert!(world.get::<Name>(entity).is_none());
    }

    #[test]
    fn flush_empty_buffer_is_noop() {
        let mut world = World::new();
        let entity = world.spawn();
        world.insert(entity, Position { x: 1.0, y: 2.0 });

        let before = world.query::<Position>().count();
        world.flush(CommandBuffer::new());
        let after = world.query::<Position>().count();

        assert_eq!(before, after);
    }

    #[test]
    fn entity_count_tracks_spawn_and_despawn() {
        let mut world = World::new();
        assert_eq!(world.entity_count(), 0);
        let a = world.spawn();
        let b = world.spawn();
        let c = world.spawn();
        assert_eq!(world.entity_count(), 3);
        world.despawn(b);
        assert_eq!(world.entity_count(), 2);
        world.despawn(a);
        world.despawn(c);
        assert_eq!(world.entity_count(), 0);
    }

    #[test]
    fn flush_multiple_pending_entities() {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Marker(u32);

        let mut world = World::new();
        let mut buffer = CommandBuffer::new();

        for i in 0..3u32 {
            let pending = buffer.spawn();
            buffer.insert_pending(pending, Marker(i));
        }

        world.flush(buffer);

        let mut markers: Vec<_> = world
            .query::<Marker>()
            .map(|(entity, marker)| {
                assert!(world.is_alive(entity));
                marker.0
            })
            .collect();
        markers.sort_unstable();
        assert_eq!(markers, vec![0, 1, 2]);
    }
}
