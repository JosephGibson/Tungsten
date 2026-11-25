//! Chemistry's work a frame, which the perf log prints as its `chemistry:`
//! line: [`ChemistryStats`].

use std::fmt::Write as _;
use std::time::Duration;

use tungsten_core::{Time, World};

/// One handler's reactions in a frame and their milliseconds, under its
/// name; kept across frames, so a warm frame allocates nothing.
#[derive(Debug, Clone)]
struct HandlerWork {
    name: String,
    reactions: u32,
    ms: f64,
}

/// A run's own tallies, kept by its system between runs and added to the
/// frame's [`ChemistryStats`] at the run's end: the milliseconds in its rule
/// matches and its outputs, and each handler's reactions.
#[derive(Debug, Default)]
pub(crate) struct RunWork {
    pub(crate) match_ms: f64,
    pub(crate) outputs_ms: f64,
    handlers: Vec<HandlerWork>,
}

impl RunWork {
    /// Counts one reaction of handler `name` that took `took`.
    pub(crate) fn handler(&mut self, name: &str, took: Duration) {
        let ms = took.as_secs_f64() * 1e3;
        match self.handlers.iter_mut().find(|work| work.name == name) {
            Some(work) => {
                work.reactions += 1;
                work.ms += ms;
            }
            None => self.handlers.push(HandlerWork {
                name: name.to_owned(),
                reactions: 1,
                ms,
            }),
        }
    }

    /// Names `names`' handlers ahead of their first reaction, so a run that
    /// first meets one allocates nothing.
    pub(crate) fn know(&mut self, names: impl IntoIterator<Item = impl AsRef<str>>) {
        for name in names {
            let name = name.as_ref();
            if !self.handlers.iter().any(|work| work.name == name) {
                self.handlers.push(HandlerWork {
                    name: name.to_owned(),
                    reactions: 0,
                    ms: 0.0,
                });
            }
        }
    }
}

/// Chemistry's work in one frame, for the perf log: its runs; the entries
/// they made, took, dropped past the bound and cut past the depth limit,
/// and the entries waiting after the last; how many runs the oldest entry
/// taken waited and the deepest one taken; the blasts set off and the
/// receivers their gathers found; each handler's reactions; and the
/// milliseconds in the takes, the rule matches, the outputs and the blasts.
/// A World resource [`ChemistryPlugin`](crate::ChemistryPlugin) inserts,
/// which a frame's first run or blast starts afresh; the umbrella's perf
/// log prints it after `physics:` ([`line`](Self::line)).
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, World};
/// use tungsten_kit::{ChemistryPlugin, ChemistryStats};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// ChemistryPlugin::new().build(&mut schedule, &mut world);
/// let line = world.resource::<ChemistryStats>().line(1);
/// assert!(line.starts_with("chemistry: runs=0 queued=0 taken=0 dropped=0"));
/// ```
#[derive(Debug, Default)]
pub struct ChemistryStats {
    /// The frame counted, as `Time::frame` names it; `None` before the first.
    frame: Option<u64>,
    runs: u32,
    queued: u64,
    taken: u64,
    dropped: u64,
    cut: u64,
    pending: u64,
    behind: u64,
    depth: u8,
    blasts: u32,
    gathered: u64,
    take_ms: f64,
    match_ms: f64,
    outputs_ms: f64,
    blast_ms: f64,
    handlers: Vec<HandlerWork>,
}

/// The frame `world`'s clock is in, 0 without one.
pub(crate) fn frame_of(world: &World) -> u64 {
    world.get_resource::<Time>().map_or(0, Time::frame)
}

/// One run's counts, as its system adds them.
pub(crate) struct RunCounts {
    pub(crate) queued: u64,
    pub(crate) taken: u64,
    pub(crate) dropped: u64,
    pub(crate) cut: u64,
    pub(crate) pending: u64,
    pub(crate) behind: u64,
    pub(crate) depth: u8,
    pub(crate) take: Duration,
}

impl ChemistryStats {
    /// Starts counting `frame` unless it is the frame held: every count 0,
    /// the handlers' names kept.
    fn start(&mut self, frame: u64) {
        if self.frame == Some(frame) {
            return;
        }
        let handlers = std::mem::take(&mut self.handlers);
        *self = Self {
            frame: Some(frame),
            handlers,
            ..Self::default()
        };
        for work in &mut self.handlers {
            work.reactions = 0;
            work.ms = 0.0;
        }
    }

    /// Adds a run of `frame`: its counts, and `work`'s tallies, which it
    /// zeroes for the next run.
    pub(crate) fn add_run(&mut self, frame: u64, counts: &RunCounts, work: &mut RunWork) {
        self.start(frame);
        self.runs += 1;
        self.queued += counts.queued;
        self.taken += counts.taken;
        self.dropped += counts.dropped;
        self.cut += counts.cut;
        self.pending = counts.pending;
        self.behind = self.behind.max(counts.behind);
        self.depth = self.depth.max(counts.depth);
        self.take_ms += counts.take.as_secs_f64() * 1e3;
        self.match_ms += std::mem::take(&mut work.match_ms);
        self.outputs_ms += std::mem::take(&mut work.outputs_ms);
        for run in &mut work.handlers {
            match self.handlers.iter_mut().find(|held| held.name == run.name) {
                Some(held) => {
                    held.reactions += run.reactions;
                    held.ms += run.ms;
                }
                None => self.handlers.push(run.clone()),
            }
            run.reactions = 0;
            run.ms = 0.0;
        }
    }

    /// Adds a blast set off in `frame` whose gather found `gathered`
    /// receivers and which took `took`.
    pub(crate) fn add_blast(&mut self, frame: u64, gathered: usize, took: Duration) {
        self.start(frame);
        self.blasts += 1;
        self.gathered += gathered as u64;
        self.blast_ms += took.as_secs_f64() * 1e3;
    }

    /// The perf log's `chemistry:` line for `frame`: its counts, or zeros
    /// when nothing counted in it, then each handler's reactions and their
    /// time as `handler.<name>=n handler.<name>.time=Xms`, whitespace and
    /// `=` in a name written `_`.
    #[must_use]
    pub fn line(&self, frame: u64) -> String {
        let empty = Self::default();
        let held = if self.frame == Some(frame) {
            self
        } else {
            &empty
        };
        let mut line = format!(
            "chemistry: runs={} queued={} taken={} dropped={} cut={} pending={} behind={} depth={} blasts={} gathered={} take={:.3}ms match={:.3}ms outputs={:.3}ms blast={:.3}ms",
            held.runs,
            held.queued,
            held.taken,
            held.dropped,
            held.cut,
            held.pending,
            held.behind,
            held.depth,
            held.blasts,
            held.gathered,
            held.take_ms,
            held.match_ms,
            held.outputs_ms,
            held.blast_ms,
        );
        for work in &self.handlers {
            let (reactions, ms) = if self.frame == Some(frame) {
                (work.reactions, work.ms)
            } else {
                (0, 0.0)
            };
            let name: String = work
                .name
                .chars()
                .map(|c| {
                    if c.is_whitespace() || c == '=' {
                        '_'
                    } else {
                        c
                    }
                })
                .collect();
            let _ = write!(
                line,
                " handler.{name}={reactions} handler.{name}.time={ms:.3}ms"
            );
        }
        line
    }
}
