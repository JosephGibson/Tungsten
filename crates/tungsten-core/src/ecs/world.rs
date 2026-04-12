use super::component::ComponentRegistry;
use super::entity::Entity;
use super::resource::ResourceMap;

/// The World owns all entities, components, and resources.
pub struct World {
    next_entity_id: u32,
    alive: Vec<bool>,
    components: ComponentRegistry,
    resources: ResourceMap,
}

impl World {
    pub fn new() -> Self {
        Self {
            next_entity_id: 0,
            alive: Vec::new(),
            components: ComponentRegistry::new(),
            resources: ResourceMap::new(),
        }
    }

    /// Spawn a new entity and return its handle.
    pub fn spawn(&mut self) -> Entity {
        let id = self.next_entity_id;
        self.next_entity_id = self
            .next_entity_id
            .checked_add(1)
            .expect("entity ID overflow");
        if (id as usize) >= self.alive.len() {
            self.alive.resize(id as usize + 1, false);
        }
        self.alive[id as usize] = true;
        Entity(id)
    }

    /// Despawn an entity, removing all of its components.
    pub fn despawn(&mut self, entity: Entity) {
        if self.is_alive(entity) {
            self.alive[entity.0 as usize] = false;
            self.components.remove_entity(entity);
        }
    }

    pub fn is_alive(&self, entity: Entity) -> bool {
        (entity.0 as usize) < self.alive.len() && self.alive[entity.0 as usize]
    }

    /// Attach a component to an entity. Panics if the entity is not alive
    /// (programmer error per ECS error strategy: panic on wrong-type/bad-state).
    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        assert!(self.is_alive(entity), "insert on dead entity {entity}");
        self.components
            .get_or_create_store::<T>()
            .insert(entity, Box::new(component));
    }

    pub fn remove_component<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        self.components
            .get_store_mut::<T>()?
            .remove(entity)?
            .downcast::<T>()
            .ok()
            .map(|b| *b)
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.components
            .get_store::<T>()?
            .get(entity)?
            .downcast_ref::<T>()
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.components
            .get_store_mut::<T>()?
            .get_mut(entity)?
            .downcast_mut::<T>()
    }

    pub fn has<T: 'static>(&self, entity: Entity) -> bool {
        self.components
            .get_store::<T>()
            .is_some_and(|s| s.contains(entity))
    }

    /// Iterate all entities that have component type `T`.
    /// Returns an iterator of `(Entity, &T)`.
    pub fn query<T: 'static>(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.components
            .get_store::<T>()
            .into_iter()
            .flat_map(|store| {
                store.entities().filter_map(|entity| {
                    store.get(entity)?.downcast_ref::<T>().map(|c| (entity, c))
                })
            })
    }

    /// Collect entity IDs that have component type `T`.
    /// Useful when you need to mutate components of multiple types in the same loop.
    pub fn query_entities<T: 'static>(&self) -> Vec<Entity> {
        self.components
            .get_store::<T>()
            .map(|store| store.entities().collect())
            .unwrap_or_default()
    }

    // --- Resources ---

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
}
