use super::EventQueue;

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
