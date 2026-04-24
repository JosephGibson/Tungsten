use super::*;

#[derive(Debug, Clone, PartialEq)]
struct Position {
    x: f32,
    y: f32,
}

#[test]
fn new_buffer_is_empty() {
    let buffer = CommandBuffer::new();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
}

#[test]
fn spawn_increments_len() {
    let mut buffer = CommandBuffer::new();
    assert_eq!(buffer.len(), 0);

    buffer.spawn();
    assert_eq!(buffer.len(), 1);

    buffer.spawn();
    assert_eq!(buffer.len(), 2);
}

#[test]
fn spawn_returns_distinct_pending_ids() {
    let mut buffer = CommandBuffer::new();
    let a = buffer.spawn();
    let b = buffer.spawn();
    assert_ne!(a, b);
}

#[test]
fn despawn_queued() {
    let mut buffer = CommandBuffer::new();
    let entity = Entity {
        index: 0,
        generation: 0,
    };

    buffer.despawn(entity);

    assert_eq!(buffer.len(), 1);
}

#[test]
fn insert_live_queued() {
    let mut buffer = CommandBuffer::new();
    let entity = Entity {
        index: 0,
        generation: 0,
    };

    buffer.insert(entity, Position { x: 1.0, y: 2.0 });

    assert_eq!(buffer.len(), 1);
}

#[test]
fn insert_pending_queued() {
    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn();

    buffer.insert_pending(pending, Position { x: 1.0, y: 2.0 });

    assert_eq!(buffer.len(), 2);
}

#[test]
fn remove_component_queued() {
    let mut buffer = CommandBuffer::new();
    let entity = Entity {
        index: 0,
        generation: 0,
    };

    buffer.remove_component::<Position>(entity);

    assert_eq!(buffer.len(), 1);
}
