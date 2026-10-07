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

#[test]
fn an_event_sent_after_the_steps_misses_fixed_readers_when_no_step_follows() {
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
