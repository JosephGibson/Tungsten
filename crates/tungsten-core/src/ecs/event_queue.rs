//! Two-window event queue: `previous <- current`, then `current <- empty`.
//!
//! App flushes once per frame after systems and command flush, before hot reload/extract/render.
//! Inside the fixed steps a later step reads only its own window, and a
//! frame with no step holds its events for the next step ([`EventQueue`]).

use crate::components::SquashTrigger;
use crate::ecs::Entity;

/// The most frames a queue holds for the next fixed step: frames with
/// events, the oldest dropped first. A running clock steps well within it;
/// it bounds slow motion.
const MAX_HELD_FRAMES: usize = 256;

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
/// A frame that runs no step while the game clock advances holds its
/// queues ([`World::hold_events_for_fixed_step`](crate::ecs::World::hold_events_for_fixed_step),
/// `D-139`): its flush keeps the previous window instead of dropping it,
/// and the first step of the next frame that runs one reads every held
/// event before the previous window. So a `fixed_update` reader that runs
/// before a sender in the frame's order, earlier in the step or ahead of
/// `update` and `post_update`, reads each of its events once through
/// [`iter`](Self::iter), at any frame rate, in the first step of a later
/// frame. [`iter_current`](Self::iter_current) still reads the frame's own
/// events only, and readers outside the steps never see held events. A
/// flush that does not hold, in a frame with a step or a paused one, drops
/// what was held, so a pause starts the next step's readers over; a queue
/// holds at most 256 frames' events.
pub struct EventQueue<T> {
    current: Vec<T>,
    previous: Vec<T>,
    /// Earlier frames' events that no fixed step has read yet, one buffer
    /// per held frame with events, oldest first.
    held: Vec<Vec<T>>,
    /// Emptied held buffers, reused so that holding a frame allocates
    /// nothing once the queue has warmed up.
    spare: Vec<Vec<T>>,
    /// Where the current step's window starts in `current`, once a step of
    /// this frame has ended; `None` before that.
    step_start: Option<usize>,
    fixed_view: bool,
    /// Whether this frame's flush holds `previous` for the next step.
    hold: bool,
}

impl<T> EventQueue<T> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: Vec::new(),
            previous: Vec::new(),
            held: Vec::new(),
            spare: Vec::new(),
            step_start: None,
            fixed_view: false,
            hold: false,
        }
    }

    /// Append event to current window.
    pub fn send(&mut self, event: T) {
        self.current.push(event);
    }

    /// Iterate previous frame first, then current. In a frame's first fixed
    /// step, the held frames' events before both; in a later step, only the
    /// events sent since the step before it ended.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let (held, previous, current) = self.windows();
        held.iter().flatten().chain(previous).chain(current)
    }

    /// Iterate current window only. In a fixed step after the frame's first,
    /// only the events sent since the step before it ended.
    pub fn iter_current(&self) -> impl Iterator<Item = &T> {
        self.windows().2.iter()
    }

    /// Every window empty, as [`iter`](Self::iter) reads them.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let (held, previous, current) = self.windows();
        current.is_empty() && previous.is_empty() && held.iter().all(Vec::is_empty)
    }

    /// Total event count across the windows, as [`iter`](Self::iter) reads
    /// them.
    #[must_use]
    pub fn len(&self) -> usize {
        let (held, previous, current) = self.windows();
        held.iter().map(Vec::len).sum::<usize>() + previous.len() + current.len()
    }

    /// Rotate windows; App-owned frame boundary. In a frame that holds
    /// ([`World::hold_events_for_fixed_step`](crate::ecs::World::hold_events_for_fixed_step))
    /// the previous window joins the held events; otherwise both are
    /// dropped.
    pub fn flush(&mut self) {
        if self.hold {
            if !self.previous.is_empty() {
                if self.held.len() == MAX_HELD_FRAMES {
                    let oldest = self.held.remove(0);
                    self.recycle(oldest);
                }
                let empty = self.spare.pop().unwrap_or_default();
                self.held.push(std::mem::replace(&mut self.previous, empty));
            }
        } else {
            while let Some(frame) = self.held.pop() {
                self.recycle(frame);
            }
        }
        self.previous.clear();
        std::mem::swap(&mut self.current, &mut self.previous);
        self.step_start = None;
        self.hold = false;
    }

    /// The windows the readers see: the frame's two, with the held events
    /// before them in a frame's first fixed step, or in a later step only
    /// the events since the last step ended.
    fn windows(&self) -> (&[Vec<T>], &[T], &[T]) {
        match self.step_start {
            Some(start) if self.fixed_view => (&[], &[], &self.current[start..]),
            None if self.fixed_view => (&self.held, &self.previous, &self.current),
            _ => (&[], &self.previous, &self.current),
        }
    }

    fn recycle(&mut self, mut frame: Vec<T>) {
        frame.clear();
        self.spare.push(frame);
    }

    /// Turns the step view on or off.
    pub(crate) fn set_fixed_view(&mut self, on: bool) {
        self.fixed_view = on;
    }

    /// Ends a fixed step: the next step's readers start after its events.
    pub(crate) fn end_fixed_step(&mut self) {
        self.step_start = Some(self.current.len());
    }

    /// This frame runs no fixed step: its flush holds the previous window
    /// for the next step instead of dropping it.
    pub(crate) fn hold_for_fixed_step(&mut self) {
        self.hold = true;
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
