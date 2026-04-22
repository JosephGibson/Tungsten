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
