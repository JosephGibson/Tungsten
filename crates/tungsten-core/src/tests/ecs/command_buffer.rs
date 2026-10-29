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

#[test]
fn bundle_forms_record_one_insert_per_component() {
    let mut buffer = CommandBuffer::new();
    let entity = Entity {
        index: 0,
        generation: 0,
    };

    let pending = buffer.spawn_with((Position { x: 1.0, y: 2.0 }, 7u32));
    assert_eq!(buffer.len(), 3);
    buffer.insert_bundle(entity, (8u32,));
    buffer.insert_bundle_pending(pending, (Position { x: 3.0, y: 4.0 }, 9u32));
    assert_eq!(buffer.len(), 6);

    // A spawn, then each component as an insert on its target, in bundle
    // order, through the one queue per component type.
    assert!(matches!(
        buffer.commands[0],
        Command::Spawn { pending_id: 0 }
    ));
    let inserts: Vec<(bool, u32)> = buffer.commands[1..]
        .iter()
        .map(|command| match command {
            Command::Insert { target, queue } => {
                (*target == CommandTarget::Pending(pending.index), *queue)
            }
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(
        inserts,
        vec![(true, 0), (true, 1), (false, 1), (true, 0), (true, 1)]
    );
}

#[test]
fn a_command_stays_24_bytes() {
    assert_eq!(std::mem::size_of::<Command>(), 24);
}

#[test]
fn a_derive_after_a_run_adds_one_archetype_per_derived_type() {
    #[derive(Clone)]
    struct A;
    #[derive(Clone)]
    struct B;

    let mut world = World::new();
    world.inherit_from_source::<A>();
    world.inherit_from_source::<B>();
    let source = world.spawn_with((A, B));
    // The spawn without a source makes the run's archetype, P.
    let mut buffer = CommandBuffer::new();
    buffer.spawn_with((Position { x: 0.0, y: 0.0 }, 7u32));
    world.flush(buffer);
    let before = world.archetypes.archetypes.len();

    let mut buffer = CommandBuffer::new();
    let pending = buffer.spawn_with((Position { x: 1.0, y: 2.0 }, 8u32));
    buffer.derive_pending(source, pending);
    world.flush(buffer);
    let added: Vec<Vec<TypeId>> = world.archetypes.archetypes[before..]
        .iter()
        .map(|arch| arch.component_types.to_vec())
        .collect();
    let key = |types: &[TypeId]| {
        let mut types = types.to_vec();
        types.sort_unstable();
        types
    };
    let p = [TypeId::of::<Position>(), TypeId::of::<u32>()];
    let a = TypeId::of::<A>();
    let b = TypeId::of::<B>();
    assert_eq!(
        added,
        [key(&[p[0], p[1], a]), key(&[p[0], p[1], a, b])],
        "P+A, then P+A+B"
    );
    let spawned: Vec<u32> = world
        .query::<(&u32, &A, &B)>()
        .map(|(n, _, _)| *n)
        .collect();
    assert_eq!(spawned, [8]);
}

#[test]
#[should_panic(expected = "valid only for its own buffer before its flush")]
fn a_pending_handle_kept_across_a_flush_panics_when_recorded() {
    let mut world = World::new();
    let mut buf = CommandBuffer::new();
    let kept = buf.spawn();
    world.flush_reusing(&mut buf);
    // The next frame's first spawn has the same index.
    let _ = buf.spawn();
    buf.insert_pending(kept, 1_u32);
}

#[test]
#[should_panic(expected = "valid only for its own buffer before its flush")]
fn a_pending_handle_past_another_buffers_spawns_panics_when_recorded() {
    let mut first = CommandBuffer::new();
    let _ = first.spawn();
    let foreign = first.spawn();
    let mut second = CommandBuffer::new();
    let _ = second.spawn();
    second.insert_pending(foreign, 1_u32);
}

#[test]
fn a_pending_handle_records_until_its_flush() {
    let mut world = World::new();
    let mut buf = CommandBuffer::new();
    world.flush_reusing(&mut buf);
    let pending = buf.spawn();
    buf.insert_pending(pending, 7_u32);
    world.flush_reusing(&mut buf);
    let values: Vec<u32> = world.query::<&u32>().copied().collect();
    assert_eq!(values, vec![7]);
}
