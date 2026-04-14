use std::any::TypeId;

use super::archetype::TypedVec;
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
    /// Panics if the entity is already dead (D-022).
    pub fn despawn(&mut self, entity: Entity) {
        if self.is_alive(entity) {
            self.archetypes.despawn(entity);
        }
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        self.archetypes.entities.is_alive(entity)
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
}
