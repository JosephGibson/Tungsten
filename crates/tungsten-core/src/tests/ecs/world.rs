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

#[test]
fn query2_returns_matching_entities() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    let e3 = world.spawn();

    world.insert(e1, Position { x: 1.0, y: 0.0 });
    world.insert(e1, Velocity { dx: 1.0, dy: 0.0 });

    world.insert(e2, Position { x: 2.0, y: 0.0 });

    world.insert(e3, Position { x: 3.0, y: 0.0 });
    world.insert(e3, Velocity { dx: 3.0, dy: 0.0 });

    let results: Vec<_> = world.query2::<Position, Velocity>().collect();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|(e, _, _)| *e != e2));
}

#[test]
fn query2_includes_supersets() {
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

    world.insert(e2, Position { x: 2.0, y: 0.0 });
    world.insert(e2, Velocity { dx: 2.0, dy: 0.0 });

    let results: Vec<_> = world.query3::<Position, Velocity, Name>().collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, e1);
}

#[test]
fn query_across_multiple_archetypes() {
    let mut world = World::new();
    let e1 = world.spawn();
    let e2 = world.spawn();
    let e3 = world.spawn();

    world.insert(e1, Position { x: 1.0, y: 0.0 });
    world.insert(e2, Position { x: 2.0, y: 0.0 });
    world.insert(e2, Velocity { dx: 0.0, dy: 0.0 });
    world.insert(e3, Position { x: 3.0, y: 0.0 });
    world.insert(e3, Velocity { dx: 0.0, dy: 0.0 });
    world.insert(e3, Name("three".into()));

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
