use super::{EventCursor, EventQueue};
use crate::ecs::World;

#[test]
fn new_queue_is_empty() {
    let queue: EventQueue<i32> = EventQueue::new();
    assert!(queue.is_empty());
    assert_eq!(queue.len(), 0);
    assert_eq!(queue.iter().count(), 0);
}

#[test]
fn send_appears_in_current_and_iter() {
    let mut queue = EventQueue::new();
    queue.send(1);
    queue.send(2);

    assert_eq!(
        queue.iter_current().copied().collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(queue.len(), 2);
}

#[test]
fn flush_moves_current_to_previous() {
    let mut queue = EventQueue::new();
    queue.send(1);

    queue.flush();

    assert_eq!(queue.iter_current().count(), 0);
    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), vec![1]);
    assert_eq!(queue.len(), 1);
}

#[test]
fn flush_twice_drops_previous() {
    let mut queue = EventQueue::new();
    queue.send(1);
    queue.flush();
    queue.send(2);

    queue.flush();

    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), vec![2]);
}

#[test]
fn iter_sees_both_windows() {
    let mut queue = EventQueue::new();
    queue.send(1);
    queue.flush();
    queue.send(2);

    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), vec![1, 2]);
}

#[test]
fn flush_empty_is_idempotent() {
    let mut queue: EventQueue<i32> = EventQueue::new();
    queue.flush();
    queue.flush();

    assert!(queue.is_empty());
}

/// One frame's fixed steps: `steps` times the view on, `step` run with the
/// step's index, the step ended; the view off after the last.
fn run_steps(
    queue: &mut EventQueue<i32>,
    steps: usize,
    mut step: impl FnMut(usize, &mut EventQueue<i32>),
) {
    for k in 0..steps {
        queue.set_fixed_view(true);
        step(k, queue);
        queue.end_fixed_step();
    }
    queue.set_fixed_view(false);
}

#[test]
fn a_step_reads_the_events_sent_since_the_step_before_it() {
    let mut queue = EventQueue::new();
    queue.send(1);
    let mut seen = Vec::new();
    run_steps(&mut queue, 3, |k, q| {
        q.send(10 + i32::try_from(k).unwrap());
        seen.push(q.iter_current().copied().collect::<Vec<_>>());
    });
    assert_eq!(seen, [vec![1, 10], vec![11], vec![12]]);
    assert_eq!(
        queue.iter_current().copied().collect::<Vec<_>>(),
        [1, 10, 11, 12],
        "outside the steps the queue reads the whole frame"
    );
}

#[test]
fn every_reader_follows_the_step_view() {
    let mut queue = EventQueue::new();
    queue.send(1);
    queue.flush();
    queue.send(2);
    queue.set_fixed_view(true);
    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), [1, 2]);
    assert_eq!(queue.iter_current().copied().collect::<Vec<_>>(), [2]);
    assert_eq!(queue.len(), 2);
    assert!(!queue.is_empty());

    queue.end_fixed_step();
    assert_eq!(queue.iter().count(), 0);
    assert_eq!(queue.iter_current().count(), 0);
    assert_eq!(queue.len(), 0);
    assert!(queue.is_empty());

    queue.send(3);
    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), [3]);
    assert_eq!(queue.iter_current().copied().collect::<Vec<_>>(), [3]);
    assert_eq!(queue.len(), 1);
    assert!(!queue.is_empty());

    queue.end_fixed_step();
    queue.set_fixed_view(false);
    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(queue.iter_current().copied().collect::<Vec<_>>(), [2, 3]);
    assert_eq!(queue.len(), 3);
}

#[test]
fn with_the_view_off_every_window_reads_as_before() {
    let mut queue = EventQueue::new();
    queue.send(1);
    queue.flush();
    queue.send(2);
    queue.end_fixed_step();
    queue.send(3);
    assert_eq!(queue.iter().copied().collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(queue.iter_current().copied().collect::<Vec<_>>(), [2, 3]);
    assert_eq!(queue.len(), 3);

    queue.flush();
    queue.set_fixed_view(true);
    assert_eq!(
        queue.iter().copied().collect::<Vec<_>>(),
        [2, 3],
        "a flush starts the next frame's first step over"
    );
    assert_eq!(queue.iter_current().count(), 0);
}

/// Frames that do not hold, as a paused frame does not.
#[test]
fn without_a_hold_an_event_sent_after_the_steps_misses_fixed_readers() {
    let mut queue = EventQueue::new();
    let mut seen = Vec::new();
    run_steps(&mut queue, 1, |_, q| seen.extend(q.iter().copied()));
    queue.send(7);
    queue.flush();
    run_steps(&mut queue, 0, |_, q| seen.extend(q.iter().copied()));
    queue.flush();
    run_steps(&mut queue, 1, |_, q| seen.extend(q.iter().copied()));
    assert!(seen.is_empty(), "seen {seen:?}");
}

#[test]
fn a_first_step_iter_still_includes_the_previous_frame() {
    let mut queue = EventQueue::new();
    queue.send(7);
    queue.flush();
    let mut seen = Vec::new();
    run_steps(&mut queue, 2, |_, q| {
        seen.push(q.iter().copied().collect::<Vec<_>>());
    });
    assert_eq!(seen, [vec![7], vec![]]);
}

fn read(events: impl Iterator<Item = i32>) -> Vec<i32> {
    events.collect()
}

/// A frame that runs no step: `event` sent outside the steps, then the
/// hold before the flush, as the app orders them.
fn frame_without_a_step(queue: &mut EventQueue<i32>, event: i32) {
    queue.send(event);
    queue.hold_for_fixed_step();
    queue.flush();
}

#[test]
fn frames_with_no_step_hold_their_events_for_the_next_first_step() {
    let mut queue = EventQueue::new();
    let mut reads = Vec::new();
    run_steps(&mut queue, 1, |_, q| {
        reads.push(read(q.iter().copied()));
        q.send(1);
    });
    queue.send(2);
    queue.flush();
    frame_without_a_step(&mut queue, 3);
    frame_without_a_step(&mut queue, 4);
    run_steps(&mut queue, 1, |_, q| reads.push(read(q.iter().copied())));
    queue.flush();
    run_steps(&mut queue, 1, |_, q| reads.push(read(q.iter().copied())));
    assert_eq!(
        reads,
        [vec![], vec![1, 2, 3, 4], vec![]],
        "each event once, in order, the step's own after its reader"
    );
}

#[test]
fn held_events_stay_out_of_the_frame_view_and_later_steps() {
    let mut queue = EventQueue::new();
    queue.send(1);
    queue.flush();
    frame_without_a_step(&mut queue, 2);
    assert_eq!(read(queue.iter().copied()), [2], "the frame view");
    assert_eq!(queue.len(), 1);

    queue.set_fixed_view(true);
    assert_eq!(read(queue.iter().copied()), [1, 2]);
    assert_eq!(queue.len(), 2);
    assert!(!queue.is_empty());
    assert_eq!(queue.iter_current().count(), 0);

    queue.end_fixed_step();
    assert_eq!(queue.iter().count(), 0, "a later step reads its own events");
    assert!(queue.is_empty());
}

#[test]
fn a_flush_without_a_hold_drops_the_held_events() {
    let mut queue = EventQueue::new();
    frame_without_a_step(&mut queue, 1);
    frame_without_a_step(&mut queue, 2);
    // A paused frame holds nothing.
    queue.send(3);
    queue.flush();
    queue.set_fixed_view(true);
    assert_eq!(read(queue.iter().copied()), [3]);
}

#[test]
fn a_queue_holds_at_most_256_frames_dropping_the_oldest() {
    let mut queue = EventQueue::new();
    for event in 0..300 {
        frame_without_a_step(&mut queue, event);
    }
    queue.set_fixed_view(true);
    assert_eq!(read(queue.iter().copied()), (43..300).collect::<Vec<_>>());
    assert_eq!(queue.len(), 257);
}

/// The events a cursor reads, in order.
fn read_with(cursor: &mut EventCursor<i32>, queue: &EventQueue<i32>) -> Vec<i32> {
    cursor.read(queue).copied().collect()
}

#[test]
fn a_cursor_reads_each_event_once_across_reads_and_flushes() {
    let mut queue = EventQueue::new();
    let mut cursor = EventCursor::new();
    queue.send(1);
    queue.send(2);
    assert_eq!(read_with(&mut cursor, &queue), [1, 2]);
    assert!(read_with(&mut cursor, &queue).is_empty(), "a second read");
    queue.send(3);
    queue.flush();
    queue.send(4);
    let read = cursor.read(&queue);
    assert_eq!(read.missed(), 0);
    assert_eq!(read.copied().collect::<Vec<_>>(), [3, 4]);
    queue.flush();
    queue.flush();
    let read = cursor.read(&queue);
    assert_eq!(
        (read.len(), read.missed()),
        (0, 0),
        "the flushes dropped only what it had read"
    );
}

#[test]
fn a_cursor_reads_events_sent_after_its_read_at_the_next() {
    let mut queue = EventQueue::new();
    let mut cursor = EventCursor::new();
    let mut reads = Vec::new();
    // The reader runs before its sender in every frame.
    for event in 0..5 {
        reads.push(read_with(&mut cursor, &queue));
        queue.send(event);
        queue.flush();
    }
    reads.push(read_with(&mut cursor, &queue));
    assert_eq!(reads, [vec![], vec![0], vec![1], vec![2], vec![3], vec![4]]);
}

#[test]
fn a_cursor_reads_across_step_views_and_held_frames_once() {
    let mut queue = EventQueue::new();
    let mut cursor = EventCursor::new();
    let (mut by_cursor, mut by_iter) = (Vec::new(), Vec::new());
    // A frame with two steps and an event before and after them; each
    // step's reader runs before its sender.
    queue.send(1);
    run_steps(&mut queue, 2, |k, q| {
        by_cursor.push(read_with(&mut cursor, q));
        by_iter.push(read(q.iter().copied()));
        q.send(10 + i32::try_from(k).unwrap());
    });
    queue.send(2);
    queue.flush();
    // Three frames with no step, then one with a step.
    for event in 3..6 {
        frame_without_a_step(&mut queue, event);
    }
    run_steps(&mut queue, 1, |_, q| {
        by_cursor.push(read_with(&mut cursor, q));
        by_iter.push(read(q.iter().copied()));
    });
    assert_eq!(by_cursor, [vec![1], vec![10], vec![11, 2, 3, 4, 5]]);
    assert_eq!(
        by_iter,
        [vec![1], vec![], vec![1, 10, 11, 2, 3, 4, 5]],
        "`iter` reads 1 twice"
    );
}

#[test]
fn a_cursor_counts_what_a_flush_dropped_before_its_read() {
    let mut queue = EventQueue::new();
    let mut cursor = EventCursor::new();
    queue.send(1);
    assert_eq!(read_with(&mut cursor, &queue), [1]);
    queue.send(2);
    queue.send(3);
    queue.flush();
    queue.send(4);
    queue.flush();
    queue.send(5);
    let read = cursor.read(&queue);
    assert_eq!(read.missed(), 2, "2 and 3 dropped unread; 1 was read");
    assert_eq!(read.copied().collect::<Vec<_>>(), [4, 5]);
    assert!(read_with(&mut cursor, &queue).is_empty());
}

#[test]
fn a_cursor_counts_the_frames_the_hold_cap_dropped() {
    let mut queue = EventQueue::new();
    let mut cursor = EventCursor::new();
    assert_eq!(cursor.read(&queue).len(), 0);
    // The 258th frame's flush holds a 257th frame, past the cap.
    for event in 0..258 {
        frame_without_a_step(&mut queue, event);
    }
    queue.set_fixed_view(true);
    let read = cursor.read(&queue);
    assert_eq!(read.missed(), 1, "the oldest held frame");
    assert_eq!(
        read.copied().collect::<Vec<_>>(),
        (1..258).collect::<Vec<_>>()
    );
}

#[test]
fn a_new_cursor_starts_at_the_oldest_held_event() {
    let mut queue = EventQueue::new();
    for event in 0..300 {
        frame_without_a_step(&mut queue, event);
    }
    let mut cursor = EventCursor::new();
    let read = cursor.read(&queue);
    assert_eq!(read.missed(), 0, "nothing was dropped past this cursor");
    assert_eq!(
        read.copied().collect::<Vec<_>>(),
        (43..300).collect::<Vec<_>>(),
        "the held frames, then the previous window, outside the steps too"
    );
}

#[test]
fn a_cursor_starts_over_on_a_replaced_queue() {
    let mut placed = EventCursor::new();
    {
        let mut old = EventQueue::new();
        for event in 0..3 {
            old.send(event);
        }
        assert_eq!(read_with(&mut placed, &old), [0, 1, 2]);
        // Dropped, as `insert_resource` drops the queue it replaces.
    }
    // The new queue's end below, at and above the old place, 3.
    for count in [1, 3, 5] {
        let mut fresh = EventQueue::new();
        let sent: Vec<i32> = (100..100 + count).collect();
        for &event in &sent {
            fresh.send(event);
        }
        let mut cursor = placed.clone();
        let read = cursor.read(&fresh);
        assert_eq!(read.missed(), 0, "{count} events");
        assert_eq!(read.copied().collect::<Vec<_>>(), sent, "{count} events");
        assert_eq!(cursor.read(&fresh).len(), 0, "{count} events, read once");
    }
}

#[test]
fn a_moved_queue_keeps_its_readers() {
    let mut queue = EventQueue::new();
    let mut cursor = EventCursor::new();
    queue.send(1);
    assert_eq!(read_with(&mut cursor, &queue), [1]);
    let mut moved = queue;
    moved.send(2);
    assert_eq!(read_with(&mut cursor, &moved), [2], "nothing read twice");
    let mut world = World::new();
    world.insert_resource(moved);
    world.resource_mut::<EventQueue<i32>>().send(3);
    assert_eq!(
        read_with(&mut cursor, world.resource::<EventQueue<i32>>()),
        [3]
    );
}

#[test]
fn two_cursors_read_one_queue_apart() {
    let mut queue = EventQueue::new();
    let (mut a, mut b) = (EventCursor::new(), EventCursor::new());
    queue.send(1);
    assert_eq!(read_with(&mut a, &queue), [1]);
    queue.send(2);
    assert_eq!(read_with(&mut b, &queue), [1, 2]);
    assert_eq!(read_with(&mut a, &queue), [2]);
    let mut c = a.clone();
    queue.flush();
    queue.send(3);
    assert_eq!(read_with(&mut a, &queue), [3]);
    assert_eq!(
        read_with(&mut c, &queue),
        [3],
        "a clone reads on from its original's place"
    );
    assert_eq!(read_with(&mut b, &queue), [3]);
}

#[test]
fn a_read_knows_its_length_and_marks_every_event_read() {
    let mut queue = EventQueue::new();
    let mut cursor = EventCursor::new();
    queue.send(1);
    queue.flush();
    queue.send(2);
    queue.send(3);
    let mut read = cursor.read(&queue);
    assert_eq!(read.len(), 3, "across both windows");
    assert_eq!(read.next(), Some(&1));
    assert_eq!(read.len(), 2);
    drop(read);
    assert_eq!(
        cursor.read(&queue).len(),
        0,
        "a read dropped early marks the rest read"
    );
    queue.send(4);
    let mut read = cursor.read(&queue);
    assert_eq!(format!("{read:?}"), "EventRead { len: 1, missed: 0 }");
    assert_eq!(read.next(), Some(&4));
    assert_eq!((read.next(), read.next()), (None, None), "fused");
}
