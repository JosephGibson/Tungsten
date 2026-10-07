//! Two-window event queue: `previous <- current`, then `current <- empty`.
//!
//! App flushes once per frame after systems and command flush, before hot reload/extract/render.
//! Inside the fixed steps a later step reads only its own window ([`EventQueue`]).

use crate::components::SquashTrigger;
use crate::ecs::Entity;

/// Typed two-window event buffer resource.
///
/// The app rotates the windows once a frame (`D-040`). While the frame's
/// fixed steps run it turns on a step view
/// ([`World::set_fixed_event_view`](crate::ecs::World::set_fixed_event_view)):
/// the first step reads the queue as any stage does, and each later step
/// reads only the events sent since the step before it ended, so an event
/// sent in a step reaches that step's later readers once and no later
/// step's. Outside the steps a queue reads the whole frame, every step's
/// events included.
///
/// An event sent outside the steps reaches a `fixed_update` reader only in a
/// step that runs after it in the same frame ([`iter_current`](Self::iter_current))
/// or in the next frame's first step ([`iter`](Self::iter)); when neither
/// comes, it rotates out unseen. So a `fixed_update` system reads the events
/// `fixed_update` sends, and one that needs another stage's events reads
/// them in `update` or carries them in a resource.
pub struct EventQueue<T> {
    current: Vec<T>,
    previous: Vec<T>,
    /// Where the current step's window starts in `current`, once a step of
    /// this frame has ended; `None` before that.
    step_start: Option<usize>,
    fixed_view: bool,
}

impl<T> EventQueue<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
            step_start: None,
            fixed_view: false,
        }
    }

    /// Append event to current window.
    pub fn send(&mut self, event: T) {
        self.current.push(event);
    }

    /// Iterate previous frame first, then current. In a fixed step after the
    /// frame's first, only the events sent since the step before it ended.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let (previous, current) = self.windows();
        previous.iter().chain(current.iter())
    }

    /// Iterate current window only. In a fixed step after the frame's first,
    /// only the events sent since the step before it ended.
    pub fn iter_current(&self) -> impl Iterator<Item = &T> {
        self.windows().1.iter()
    }

    /// Both windows empty, as [`iter`](Self::iter) reads them.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let (previous, current) = self.windows();
        current.is_empty() && previous.is_empty()
    }

    /// Total event count across both windows, as [`iter`](Self::iter) reads
    /// them.
    #[must_use]
    pub fn len(&self) -> usize {
        let (previous, current) = self.windows();
        previous.len() + current.len()
    }

    /// Rotate windows; App-owned frame boundary.
    pub fn flush(&mut self) {
        self.previous.clear();
        std::mem::swap(&mut self.current, &mut self.previous);
        self.step_start = None;
    }

    /// The windows the readers see: both, or in a later fixed step only the
    /// events since the last step ended.
    fn windows(&self) -> (&[T], &[T]) {
        match self.step_start {
            Some(start) if self.fixed_view => (&[], &self.current[start..]),
            _ => (&self.previous, &self.current),
        }
    }

    /// Turns the step view on or off.
    pub(crate) fn set_fixed_view(&mut self, on: bool) {
        self.fixed_view = on;
    }

    /// Ends a fixed step: the next step's readers start after its events.
    pub(crate) fn end_fixed_step(&mut self) {
        self.step_start = Some(self.current.len());
    }
}

impl<T> Default for EventQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Additive camera trauma request (M30). Drained by `shake_tick_system`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShakeEvent {
    pub trauma_add: f32,
}

/// Arms an entity's squash envelope (M30). Sent by gameplay, never by core:
/// `SquashTrigger::OnLand` is not detectable without gameplay knowledge, so the
/// sender names the trigger and `SpriteSquashStretch.on` filters it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SquashEvent {
    pub entity: Entity,
    pub trigger: SquashTrigger,
}

#[cfg(test)]
#[path = "../tests/ecs/event_queue.rs"]
mod tests;
