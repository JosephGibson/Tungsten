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
#[path = "../tests/ecs/event_queue.rs"]
mod tests;
