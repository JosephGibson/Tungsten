//! Two-window event queue: `previous <- current`, then `current <- empty`.
//!
//! App flushes once per frame after systems and command flush, before hot reload/extract/render.
//! Inside the fixed steps a later step reads only its own window, and a
//! frame with no step holds its events for the next step ([`EventQueue`]).
//! An [`EventCursor`] reads each event once from any stage.

use std::fmt;
use std::iter::FusedIterator;
use std::marker::PhantomData;
use std::sync::{Arc, Weak};

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
///
/// A reader that keeps an [`EventCursor`] reads each event once in the order
/// it was sent, whatever its stage, the fixed step or its place among the
/// senders: the cursor ignores the step views, reads held events like any
/// other, and counts what the queue dropped before it got to them.
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
    /// The sequence of the oldest event the queue keeps: how many it has
    /// dropped. The kept events are a contiguous tail of all it was sent:
    /// `held`, then `previous`, then `current`.
    first: u64,
    /// How many events `held` keeps.
    held_len: usize,
    /// The queue's identity, which its cursors keep as a `Weak`, so that a
    /// read tells a replaced queue from this one.
    id: Arc<()>,
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
            first: 0,
            held_len: 0,
            id: Arc::new(()),
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

    /// How many events were sent since the last [`flush`](Self::flush),
    /// whatever the step view.
    pub(super) fn sent_since_flush(&self) -> usize {
        self.current.len()
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
                    self.drop_held(oldest.len());
                    self.recycle(oldest);
                }
                self.held_len += self.previous.len();
                let empty = self.spare.pop().unwrap_or_default();
                self.held.push(std::mem::replace(&mut self.previous, empty));
            }
        } else {
            self.drop_held(self.held_len);
            while let Some(frame) = self.held.pop() {
                self.recycle(frame);
            }
        }
        self.first += self.previous.len() as u64;
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

    /// Counts `count` held events as dropped.
    fn drop_held(&mut self, count: usize) {
        self.held_len -= count;
        self.first += count as u64;
    }

    /// The sequence one past the newest event.
    fn end(&self) -> u64 {
        self.first + (self.held_len + self.previous.len() + self.current.len()) as u64
    }

    /// The events from sequence `start`, which lies between `first` and the
    /// end: the held frames' tail, found from the back, then both windows.
    fn read_from(&self, start: u64, missed: u64) -> EventRead<'_, T> {
        let mut read = EventRead {
            front: std::slice::Iter::default(),
            held: std::slice::Iter::default(),
            previous: &self.previous,
            current: &self.current,
            len: usize::try_from(self.end() - start).expect("unread events fit in memory"),
            missed,
        };
        let previous_start = self.first + self.held_len as u64;
        if start >= previous_start {
            let skip = usize::try_from(start - previous_start).expect("a window index");
            if let Some(rest) = self.previous.get(skip..).filter(|rest| !rest.is_empty()) {
                read.front = rest.iter();
            } else {
                read.front = self.current[skip - self.previous.len()..].iter();
                read.current = &[];
            }
            read.previous = &[];
        } else {
            // The held frame that holds `start`, counted from the back.
            let mut frame_start = previous_start;
            let mut frame = self.held.len();
            while frame_start > start {
                frame -= 1;
                frame_start -= self.held[frame].len() as u64;
            }
            let skip = usize::try_from(start - frame_start).expect("a frame index");
            read.front = self.held[frame][skip..].iter();
            read.held = self.held[frame + 1..].iter();
        }
        read
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

/// A reader's place in an [`EventQueue<T>`], by sequence: each
/// [`read`](Self::read) yields the events sent since the last read that the
/// queue still holds, once each, in the order they were sent, whatever the
/// stage, the fixed step or the reader's place among the senders. A cursor
/// follows its queue: when the World's queue is replaced, its next read
/// starts at the new queue's oldest event.
///
/// A reader keeps its cursor in its system's state and reads once a run:
///
/// ```
/// use tungsten_core::{EventCursor, EventQueue};
///
/// let mut queue = EventQueue::new();
/// let mut cursor = EventCursor::new();
/// queue.send("hit");
/// assert_eq!(cursor.read(&queue).copied().collect::<Vec<_>>(), ["hit"]);
///
/// queue.flush();
/// queue.send("miss");
/// let read = cursor.read(&queue);
/// assert_eq!(read.missed(), 0);
/// assert_eq!(read.copied().collect::<Vec<_>>(), ["miss"], "`hit` is read once");
///
/// queue.send("lost");
/// queue.flush();
/// queue.flush();
/// let read = cursor.read(&queue);
/// assert_eq!(read.missed(), 1, "two flushes dropped `lost` unread");
/// assert_eq!(read.len(), 0);
/// ```
pub struct EventCursor<T> {
    /// The sequence of the next event this cursor has not read.
    next: u64,
    /// The identity of the queue this cursor reads, from its first read.
    queue: Option<Weak<()>>,
    marker: PhantomData<fn() -> T>,
}

impl<T> EventCursor<T> {
    /// A cursor whose first read yields every event the queue then holds.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next: 0,
            queue: None,
            marker: PhantomData,
        }
    }

    /// The events past this cursor, oldest first. The cursor moves past all
    /// of them when `read` returns, whether or not the iterator is driven to
    /// its end.
    pub fn read<'q>(&mut self, queue: &'q EventQueue<T>) -> EventRead<'q, T> {
        let same = self
            .queue
            .as_ref()
            .is_some_and(|id| std::ptr::eq(id.as_ptr(), Arc::as_ptr(&queue.id)));
        let (start, missed) = if same {
            let start = self.next.max(queue.first).min(queue.end());
            (start, queue.first.saturating_sub(self.next))
        } else {
            self.queue = Some(Arc::downgrade(&queue.id));
            (queue.first, 0)
        };
        self.next = queue.end();
        queue.read_from(start, missed)
    }
}

impl<T> Default for EventCursor<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Clone for EventCursor<T> {
    fn clone(&self) -> Self {
        Self {
            next: self.next,
            queue: self.queue.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> fmt::Debug for EventCursor<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventCursor")
            .field("next", &self.next)
            .finish_non_exhaustive()
    }
}

/// One [`EventCursor::read`]: its events, oldest first, and how many the
/// queue dropped before the cursor reached them.
#[must_use = "a read moves its cursor past every event it holds"]
pub struct EventRead<'q, T> {
    /// The events being read.
    front: std::slice::Iter<'q, T>,
    /// The held frames after the front one.
    held: std::slice::Iter<'q, Vec<T>>,
    /// The windows after the held frames, each emptied once it is the front.
    previous: &'q [T],
    current: &'q [T],
    /// The events not yet yielded.
    len: usize,
    missed: u64,
}

impl<T> EventRead<'_, T> {
    /// The events past the cursor that the queue dropped before this read:
    /// rotated out by flushes since the last read, or past the held-frame
    /// cap. Always 0 for a reader that reads every frame. A reader in
    /// `fixed_update` misses what frames with a stopped clock dropped, and
    /// what the 256-frame hold cap dropped when its steps came further apart
    /// than that, as in deep slow motion.
    #[must_use]
    pub fn missed(&self) -> u64 {
        self.missed
    }
}

impl<'q, T> Iterator for EventRead<'q, T> {
    type Item = &'q T;

    fn next(&mut self) -> Option<&'q T> {
        loop {
            if let Some(event) = self.front.next() {
                self.len -= 1;
                return Some(event);
            }
            self.front = if let Some(frame) = self.held.next() {
                frame.iter()
            } else if !self.previous.is_empty() {
                std::mem::take(&mut self.previous).iter()
            } else if !self.current.is_empty() {
                std::mem::take(&mut self.current).iter()
            } else {
                return None;
            };
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T> ExactSizeIterator for EventRead<'_, T> {}

impl<T> FusedIterator for EventRead<'_, T> {}

impl<T> fmt::Debug for EventRead<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventRead")
            .field("len", &self.len)
            .field("missed", &self.missed)
            .finish()
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
