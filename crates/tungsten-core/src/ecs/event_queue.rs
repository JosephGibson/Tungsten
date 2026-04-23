//! Two-window event queue: `previous <- current`, then `current <- empty`.
//!
//! App flushes once per frame after systems and command flush, before hot reload/extract/render.

/// Typed two-window event buffer resource.
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

    /// Append event to current window.
    pub fn send(&mut self, event: T) {
        self.current.push(event);
    }

    /// Iterate previous frame first, then current.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.previous.iter().chain(self.current.iter())
    }

    /// Iterate current window only.
    pub fn iter_current(&self) -> impl Iterator<Item = &T> {
        self.current.iter()
    }

    /// Both windows empty.
    pub fn is_empty(&self) -> bool {
        self.current.is_empty() && self.previous.is_empty()
    }

    /// Total event count across both windows.
    pub fn len(&self) -> usize {
        self.previous.len() + self.current.len()
    }

    /// Rotate windows; App-owned frame boundary.
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
