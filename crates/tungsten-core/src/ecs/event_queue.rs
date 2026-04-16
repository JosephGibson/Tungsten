//! Typed two-window event queue.
//!
//! Each [`EventQueue<T>`] resource holds two `Vec<T>` windows: `previous` and
//! `current`. Senders call [`EventQueue::send`] during the update stage.
//! Readers call [`EventQueue::iter`] for the canonical two-window view or
//! [`EventQueue::iter_current`] when they are guaranteed to run after all
//! senders in the same frame.
//!
//! The App frame loop calls [`EventQueue::flush`] once per frame at the same
//! boundary as `CommandBuffer` flush: after systems and before hot reload,
//! extract, and render. Flush rotates the windows so `previous <- current` and
//! `current <- empty`.

/// A typed two-window event buffer stored as a World resource.
pub struct EventQueue<T> {
    current: Vec<T>,
    previous: Vec<T>,
}

impl<T> EventQueue<T> {
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
        }
    }

    /// Append an event to the current frame's window.
    pub fn send(&mut self, event: T) {
        self.current.push(event);
    }

    /// Iterate events across both windows, previous frame first.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.previous.iter().chain(self.current.iter())
    }

    /// Iterate only the current frame's events.
    pub fn iter_current(&self) -> impl Iterator<Item = &T> {
        self.current.iter()
    }

    /// Returns `true` when both windows are empty.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty() && self.previous.is_empty()
    }

    /// Total event count across both windows.
    pub fn len(&self) -> usize {
        self.previous.len() + self.current.len()
    }

    /// Rotate the windows: `previous <- current`, `current <- empty`.
    ///
    /// Called once per frame by the App event-flush stage. Game systems should
    /// not call this directly.
    pub fn flush(&mut self) {
        self.previous.clear();
        std::mem::swap(&mut self.current, &mut self.previous);
    }
}

impl<T> Default for EventQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
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
}
