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
fn len_counts_one_per_recorded_command() {
    let mut buffer = CommandBuffer::new();
    let entity = Entity {
        index: 0,
        generation: 0,
    };

    let pending = buffer.spawn();
    buffer.insert_pending(pending, Position { x: 1.0, y: 2.0 });
    buffer.insert_pending(pending, 7u32);
    buffer.insert(entity, Position { x: 3.0, y: 4.0 });
    buffer.remove_component::<Position>(entity);
    buffer.despawn(entity);

    assert_eq!(buffer.len(), 6);
    assert!(!buffer.is_empty());
}

#[test]
fn inserts_share_one_queue_per_component_type() {
    let mut buffer = CommandBuffer::new();
    let entity = Entity {
        index: 0,
        generation: 0,
    };

    buffer.insert(entity, Position { x: 1.0, y: 2.0 });
    buffer.insert(entity, 7u32);
    buffer.insert(entity, Position { x: 3.0, y: 4.0 });

    let queues: Vec<u32> = buffer
        .commands
        .iter()
        .map(|command| match command {
            Command::Insert { queue, .. } => *queue,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(queues, vec![0, 1, 0]);
    assert_eq!(buffer.values.component_type(0), TypeId::of::<Position>());
    assert_eq!(buffer.values.component_type(1), TypeId::of::<u32>());
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
