//! The event trace: a ring of what the kit's items did, for a failing
//! recipe or a game's debug view to tell why an entity died.

use std::collections::VecDeque;
use std::fmt;

use tungsten_core::{Entity, Time, World};

/// [`KitDebugPlugin`](crate::KitDebugPlugin)'s system that counts ended
/// steps, last in the react phase.
pub const KIT_TRACE_STEP: &str = "kit_trace_step";

/// What the kit's items did, a World resource that
/// [`KitDebugPlugin`](crate::KitDebugPlugin) inserts: a ring of
/// [`TraceEntry`]s, the oldest dropped and counted once it holds
/// [`capacity`](Self::capacity). The kit pushes each event its items send
/// (`damaged`, `killed`, `zone_entered`, `cooldown_elapsed`), each cue
/// action run or skipped, each spawn queued or failed and each despawn
/// queued, with the entity it is about. It records whether or not the
/// debug view is on; while [`KitDebug`](crate::KitDebug) is on, the debug
/// plugin logs each new entry once at `info` under target
/// `tungsten_kit::trace`, which the log console shows.
///
/// An entry's [`step`](TraceEntry::step) is the count of fixed steps ended
/// before it: the step it was pushed in, for a push from a fixed-step
/// system, and the next step for a push outside the steps.
///
/// # Examples
///
/// ```
/// use tungsten_core::World;
/// use tungsten_kit::KitTrace;
///
/// let trace = KitTrace::new(KitTrace::DEFAULT_CAPACITY);
/// assert_eq!(trace.capacity(), 256);
/// assert_eq!((trace.iter().count(), trace.dropped(), trace.steps()), (0, 0, 0));
/// let nobody = World::new().spawn();
/// assert_eq!(trace.about(nobody).count(), 0);
/// ```
#[derive(Debug)]
pub struct KitTrace {
    ring: VecDeque<TraceEntry>,
    capacity: usize,
    steps: u64,
    pushed: u64,
}

impl Default for KitTrace {
    /// [`new`](Self::new) at [`DEFAULT_CAPACITY`](Self::DEFAULT_CAPACITY).
    fn default() -> Self {
        Self::new(Self::DEFAULT_CAPACITY)
    }
}

impl KitTrace {
    /// The entries a trace keeps unless the game asks for another size.
    pub const DEFAULT_CAPACITY: usize = 256;

    /// An empty trace keeping the last `capacity` entries; 0 records
    /// nothing.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            ring: VecDeque::with_capacity(capacity.min(Self::DEFAULT_CAPACITY)),
            capacity,
            steps: 0,
            pushed: 0,
        }
    }

    /// How many entries it keeps.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// The kept entries, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &TraceEntry> {
        self.ring.iter()
    }

    /// The kept entries about `entity`: it is the entry's entity or its
    /// other one, oldest first.
    pub fn about(&self, entity: Entity) -> impl Iterator<Item = &TraceEntry> {
        self.ring
            .iter()
            .filter(move |entry| entry.entity == entity || entry.kind.other() == Some(entity))
    }

    /// How many entries it dropped, oldest first, to keep its capacity.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.pushed - self.ring.len() as u64
    }

    /// How many fixed steps have ended since it was inserted.
    #[must_use]
    pub fn steps(&self) -> u64 {
        self.steps
    }

    /// How many entries were ever pushed, kept or dropped: the debug view's
    /// place.
    pub(crate) fn pushed(&self) -> u64 {
        self.pushed
    }

    /// The entries pushed after the first `logged`, those still kept, oldest
    /// first.
    pub(crate) fn since(&self, logged: u64) -> impl Iterator<Item = &TraceEntry> {
        let new = self.pushed.saturating_sub(logged);
        let kept = new.min(self.ring.len() as u64) as usize;
        self.ring.iter().skip(self.ring.len() - kept)
    }

    /// Drops every entry and the dropped count; the step count stays.
    pub fn clear(&mut self) {
        self.ring.clear();
        self.pushed = 0;
    }

    fn push(&mut self, entry: TraceEntry) {
        self.pushed += 1;
        if self.capacity == 0 {
            return;
        }
        if self.ring.len() == self.capacity {
            self.ring.pop_front();
        }
        self.ring.push_back(entry);
    }

    /// Counts a step ended: [`KIT_TRACE_STEP`]'s work.
    pub(crate) fn end_step(&mut self) {
        self.steps += 1;
    }
}

/// One thing the kit did, in a [`KitTrace`]. Built by the kit only;
/// `Display` writes it on one line.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TraceEntry {
    /// `Time::frame()` when it was pushed.
    pub frame: u64,
    /// The fixed steps ended before it.
    pub step: u64,
    /// The entity it is about: an event's subject, a cue's subject, a
    /// spawn's source or a despawn's target.
    pub entity: Entity,
    /// What happened.
    pub kind: TraceKind,
}

/// What a [`TraceEntry`] records. A match outside this crate needs a
/// wildcard arm, since later items add kinds.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TraceKind {
    /// An item sent event `name` about the entry's entity.
    Event {
        /// The event's cue name: `damaged`, `zone_entered`.
        name: &'static str,
        /// The event's other entity: a zone's entrant.
        other: Option<Entity>,
        /// The event's amount: a hit's damage.
        amount: Option<f32>,
    },
    /// A cue action ran for an event on the entry's entity.
    Cue {
        /// The event's cue name.
        event: &'static str,
        /// The action's name: `prefab`, `counter`, `label`.
        action: &'static str,
        /// The `cue_rules` entry the action came from, as `<id>.<index>`;
        /// `None` for the entity's own cues.
        rule: Option<String>,
    },
    /// A cue action, an occurrence or an item's work did not run.
    Skipped {
        /// The event's cue name.
        event: &'static str,
        /// The action's name, or `all` for a whole occurrence.
        action: &'static str,
        /// Why.
        why: &'static str,
    },
    /// A spawn from the entry's entity was queued.
    Spawn {
        /// The prefab's ID.
        prefab: String,
        /// The item that spawned it: `spawner`, `spawn_children`, `cues`.
        by: &'static str,
    },
    /// A spawn from the entry's entity did not prepare.
    SpawnFailed {
        /// The prefab's ID.
        prefab: String,
        /// The item that spawned it.
        by: &'static str,
        /// The error.
        error: String,
    },
    /// The entry's entity was queued for despawn.
    Despawn {
        /// The item that despawned it: `lifetime`, `despawn_outside`,
        /// `cues`.
        by: &'static str,
    },
}

impl TraceKind {
    fn other(&self) -> Option<Entity> {
        match self {
            TraceKind::Event { other, .. } => *other,
            _ => None,
        }
    }
}

impl fmt::Display for TraceEntry {
    /// `frame 12 step 40: Entity(7) damaged 3 (killed)`, in the kind's
    /// words.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "frame {} step {}: {} ",
            self.frame, self.step, self.entity
        )?;
        match &self.kind {
            TraceKind::Event {
                name,
                other,
                amount,
            } => {
                write!(f, "{name}")?;
                if let Some(amount) = amount {
                    write!(f, " {amount}")?;
                }
                if let Some(other) = other {
                    write!(f, " with {other}")?;
                }
                Ok(())
            }
            TraceKind::Cue {
                event,
                action,
                rule,
            } => {
                write!(f, "{event}: {action}")?;
                match rule {
                    Some(rule) => write!(f, " (rule {rule})"),
                    None => write!(f, " (own cue)"),
                }
            }
            TraceKind::Skipped { event, action, why } => {
                write!(f, "{event}: {action} skipped, {why}")
            }
            TraceKind::Spawn { prefab, by } => write!(f, "spawned `{prefab}` by {by}"),
            TraceKind::SpawnFailed { prefab, by, error } => {
                write!(f, "`{prefab}` by {by} spawned nothing: {error}")
            }
            TraceKind::Despawn { by } => write!(f, "despawned by {by}"),
        }
    }
}

/// Pushes `kind()` about `entity` when the World has a [`KitTrace`], at the
/// frame `Time` gives; `kind` runs only then, so a World without a trace
/// builds no entry.
pub(crate) fn trace(world: &mut World, entity: Entity, kind: impl FnOnce() -> TraceKind) {
    let frame = world.get_resource::<Time>().map_or(0, Time::frame);
    let Some(trace) = world.get_resource_mut::<KitTrace>() else {
        return;
    };
    let entry = TraceEntry {
        frame,
        step: trace.steps,
        entity,
        kind: kind(),
    };
    trace.push(entry);
}

#[cfg(test)]
#[path = "../tests/trace.rs"]
mod tests;
