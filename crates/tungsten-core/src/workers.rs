//! A system's parallel map (`D-311`): a fork over owned inputs on worker
//! threads of the map's own, cut into fixed chunks and laid out in index
//! order, so its output is one serial call's bit for bit at every worker
//! count.
//!
//! A system moves what its work reads into one value, `shared`, and calls
//! [`map`] over a length: each participant, the calling thread among them,
//! claims chunks of the range through one atomic counter and appends its
//! outputs, and the caller appends each chunk's outputs to `out` in index
//! order and takes `shared` back. Threads cannot borrow from the World, so a
//! component the work reads is gathered or moved out first; `Send` and
//! `Sync` are bounds of the call alone (`D-131`), never of a component or a
//! resource.
//!
//! [`SystemWorkers`] holds the participants. `App::run` inserts it at the
//! physics step's count; `App::new`, the harness, the tests and the counter
//! guards insert none, so their maps run serially.

use std::fmt;
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::ecs::World;

/// How long the calling thread spins for a worker's answer before it blocks:
/// a map's shares end within microseconds of its own.
const ANSWER_SPIN: Duration = Duration::from_micros(50);

/// The threads a system's [`map`] forks on, the calling thread among them: a
/// World resource. The threads start at the first map that forks, so a game
/// whose maps stay under their minimum starts none.
pub struct SystemWorkers {
    participants: u32,
    /// Whether every map of more than one chunk forks, in chunks of
    /// `forced_chunk`, whatever its minimum, as the tests ask.
    fork_any: bool,
    forced_chunk: u32,
    pool: Option<Box<Pool>>,
    /// Tests: the worker threads that have ended, across every pool made.
    #[cfg(test)]
    exits: Arc<AtomicUsize>,
}

impl fmt::Debug for SystemWorkers {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemWorkers")
            .field("participants", &self.participants)
            .field("fork_any", &self.fork_any)
            .field("forced_chunk", &self.forced_chunk)
            .field("started", &self.pool.is_some())
            .finish_non_exhaustive()
    }
}

impl SystemWorkers {
    /// The most participants a map takes.
    pub const MAX: usize = 16;

    /// `participants` threads, the calling thread among them, clamped to 1
    /// through [`Self::MAX`]; 1 maps serially.
    #[must_use]
    pub fn new(participants: usize) -> Self {
        Self {
            participants: participants.clamp(1, Self::MAX) as u32,
            fork_any: false,
            forced_chunk: 0,
            pool: None,
            #[cfg(test)]
            exits: Arc::default(),
        }
    }

    /// For tests: `participants` threads that fork every map of more than one
    /// chunk, whatever its minimum, in chunks of `chunk` (at least 1) in place
    /// of the caller's, so a small test world runs the forked code.
    #[must_use]
    pub fn forced(participants: usize, chunk: usize) -> Self {
        Self {
            fork_any: true,
            forced_chunk: chunk.clamp(1, u32::MAX as usize) as u32,
            ..Self::new(participants)
        }
    }

    /// The participants, the calling thread among them.
    #[must_use]
    pub fn participants(&self) -> usize {
        self.participants as usize
    }

    /// Whether the threads have started.
    #[must_use]
    pub fn started(&self) -> bool {
        self.pool.is_some()
    }

    /// Whether [`Self::map`] forks a range of `len` in chunks of `chunk`
    /// from `min`: with two participants or more and more than one chunk,
    /// and `len` at `min` or above unless the workers are
    /// [forced](Self::forced). A system that keeps its serial code calls the
    /// map only when this holds; that code must then give what `f` gives over
    /// `0..len`, which the system's own tests hold at forced counts.
    #[must_use]
    pub fn forks(&self, len: usize, chunk: usize, min: usize) -> bool {
        self.participants > 1 && len > self.chunk(chunk) && (self.fork_any || len >= min)
    }

    /// The chunk a map takes: the caller's, at least 1, or the forced one.
    fn chunk(&self, chunk: usize) -> usize {
        if self.fork_any {
            self.forced_chunk as usize
        } else {
            chunk.max(1)
        }
    }

    /// [`map`] on these threads.
    pub fn map<S, O>(
        &mut self,
        shared: S,
        len: usize,
        chunk: usize,
        min: usize,
        out: &mut Vec<O>,
        f: fn(&S, Range<usize>, &mut Vec<O>),
    ) -> S
    where
        S: Send + Sync + 'static,
        O: Send + 'static,
    {
        if !self.forks(len, chunk, min) {
            f(&shared, 0..len, out);
            return shared;
        }
        let chunk = self.chunk(chunk);
        // The pool is out of `self` for the fork: when a share panics, the
        // unwinding drops it, joining its threads, and the next fork starts
        // fresh ones rather than reading this fork's late answers.
        let mut pool = self.pool.take().unwrap_or_else(|| Box::new(self.start()));
        let shared = fork(&mut pool, shared, len, chunk, out, f);
        self.pool = Some(pool);
        shared
    }

    /// The pool, made at the first fork. Cold and out of line: it runs once.
    #[cold]
    #[inline(never)]
    fn start(&self) -> Pool {
        #[cfg(test)]
        let pool = Pool::new(self.participants as usize, &self.exits);
        #[cfg(not(test))]
        let pool = Pool::new(self.participants as usize);
        pool
    }

    /// Tests: the worker threads that have ended so far.
    #[cfg(test)]
    pub(crate) fn exits(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.exits)
    }
}

/// The forked map on `pool`; out of line, so a map that stays serial holds a
/// call.
#[inline(never)]
fn fork<S, O>(
    pool: &mut Pool,
    shared: S,
    len: usize,
    chunk: usize,
    out: &mut Vec<O>,
    f: fn(&S, Range<usize>, &mut Vec<O>),
) -> S
where
    S: Send + Sync + 'static,
    O: Send + 'static,
{
    let threads = pool
        .threads
        .get_mut()
        .unwrap_or_else(PoisonError::into_inner);
    if threads.senders.is_empty() {
        // Every spawn was refused: the calling thread maps alone.
        f(&shared, 0..len, out);
        return shared;
    }
    let chunks = len.div_ceil(chunk);
    let mut results = Vec::new();
    results.resize_with(threads.senders.len() + 1, || Mutex::new(None));
    let job = Arc::new(MapJob {
        shared,
        len,
        chunk,
        chunks,
        next: AtomicUsize::new(0),
        f,
        results,
    });
    for sender in &threads.senders {
        let share: Arc<dyn Share> = job.clone();
        sender
            .send(share)
            .expect("a map worker ended before its pool");
    }
    job.run(0);
    let waited = Instant::now();
    for _ in 0..threads.senders.len() {
        loop {
            match threads.answers.try_recv() {
                Ok(_) => break,
                Err(mpsc::TryRecvError::Empty) if waited.elapsed() < ANSWER_SPIN => {
                    std::hint::spin_loop();
                }
                Err(_) => {
                    threads
                        .answers
                        .recv()
                        .expect("a map worker ended before its pool");
                    break;
                }
            }
        }
    }
    // A worker that answered without its outputs panicked in its share.
    let complete = job.results.iter().all(|slot| {
        slot.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    });
    assert!(complete, "a worker's share of a system's map panicked");
    let job = Arc::try_unwrap(job)
        .ok()
        .expect("every map worker drops its share before it answers");
    // A participant claims its chunks in rising order, so its outputs are in
    // chunk order: the chunks are laid out by index from the counts.
    let mut at = vec![(0usize, 0usize); chunks];
    let mut total = 0;
    let mut outputs = Vec::with_capacity(job.results.len());
    for (participant, slot) in job.results.into_iter().enumerate() {
        let taken = slot
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
            .expect("checked above");
        for &(chunk, count) in &taken.spans {
            at[chunk] = (participant, count);
            total += count;
        }
        outputs.push(taken.out.into_iter());
    }
    out.reserve(total);
    for &(participant, count) in &at {
        out.extend(outputs[participant].by_ref().take(count));
    }
    job.shared
}

/// `f` over `0..len`, its outputs appended to `out`, forked on the World's
/// [`SystemWorkers`] when there are two participants or more, `len` is at
/// least `min` and more than one `chunk`; one call `f(&shared, 0..len, out)`
/// otherwise. Returns `shared`.
///
/// Forked, the range is cut into chunks of `chunk` that the participants
/// claim; each chunk's outputs go to `out` in index order. The output equals
/// the serial call's whenever `f`'s output for a range is the concatenation
/// of its outputs for any split of that range, which holds when each item's
/// outputs depend only on the item and on `shared`. An item may give any
/// number of outputs. `f` only appends to the vector it is handed and never
/// reads it: forked, it is a participant's own, holding that participant's
/// earlier chunks; serially, it is `out` with whatever `out` held.
///
/// # Panics
///
/// When a worker's share panicked, as the serial call would have.
pub fn map<S, O>(
    world: &mut World,
    shared: S,
    len: usize,
    chunk: usize,
    min: usize,
    out: &mut Vec<O>,
    f: fn(&S, Range<usize>, &mut Vec<O>),
) -> S
where
    S: Send + Sync + 'static,
    O: Send + 'static,
{
    if let Some(workers) = world.get_resource_mut::<SystemWorkers>() {
        workers.map(shared, len, chunk, min, out, f)
    } else {
        f(&shared, 0..len, out);
        shared
    }
}

/// Whether [`map`] forks on the World's [`SystemWorkers`]: never without
/// them; see [`SystemWorkers::forks`].
#[must_use]
pub fn forks(world: &World, len: usize, chunk: usize, min: usize) -> bool {
    world
        .get_resource::<SystemWorkers>()
        .is_some_and(|workers| workers.forks(len, chunk, min))
}

/// A fork's share as a participant runs it.
trait Share: Send + Sync {
    /// Claims chunks until none is left, keeping the outputs for the caller.
    fn run(&self, participant: usize);
}

struct MapJob<S, O> {
    shared: S,
    len: usize,
    chunk: usize,
    chunks: usize,
    next: AtomicUsize,
    f: fn(&S, Range<usize>, &mut Vec<O>),
    /// A participant's outputs, the caller's first.
    results: Vec<Mutex<Option<Taken<O>>>>,
}

/// A participant's outputs and, per chunk it took in rising order, the
/// chunk's index and its output count.
struct Taken<O> {
    out: Vec<O>,
    spans: Vec<(usize, usize)>,
}

impl<S, O> Share for MapJob<S, O>
where
    S: Send + Sync,
    O: Send,
{
    fn run(&self, participant: usize) {
        let mut taken = Taken {
            out: Vec::new(),
            spans: Vec::new(),
        };
        loop {
            let chunk = self.next.fetch_add(1, Ordering::Relaxed);
            if chunk >= self.chunks {
                break;
            }
            let start = chunk * self.chunk;
            let end = (start + self.chunk).min(self.len);
            let before = taken.out.len();
            (self.f)(&self.shared, start..end, &mut taken.out);
            taken.spans.push((chunk, taken.out.len() - before));
        }
        *self.results[participant]
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(taken);
    }
}

/// Sends a worker's answer when it drops, so a share that panics still
/// answers and the map does not wait for it forever.
struct Answering<'a> {
    answers: &'a mpsc::Sender<usize>,
    worker: usize,
}

impl Drop for Answering<'_> {
    fn drop(&mut self) {
        // The map went away with its receiver: nothing waits.
        let _ = self.answers.send(self.worker);
    }
}

fn worker_loop(
    worker: usize,
    jobs: &mpsc::Receiver<Arc<dyn Share>>,
    answers: &mpsc::Sender<usize>,
) {
    // Parked in `recv` between maps; a closed channel ends the thread.
    while let Ok(share) = jobs.recv() {
        let answering = Answering { answers, worker };
        // Bound after the guard, so a share that panics drops its job
        // before the guard answers, as a normal return does: the caller
        // takes the job back once every worker has answered.
        let job = share;
        job.run(worker);
        drop(job);
        drop(answering);
    }
}

/// Tests: counts a worker thread's end as it ends, by a return or a panic.
#[cfg(test)]
struct Exit(Arc<AtomicUsize>);

#[cfg(test)]
impl Drop for Exit {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

/// The threads: each worker's channel, the channel they answer on and their
/// handles.
struct Threads {
    senders: Vec<mpsc::Sender<Arc<dyn Share>>>,
    answers: mpsc::Receiver<usize>,
    handles: Vec<JoinHandle<()>>,
}

/// The map's threads, behind a `Mutex` only for what the lock lends the
/// resource: `Sync` and unwind safety, which a channel's receiver lacks. The
/// map reaches them through `get_mut` and never locks.
struct Pool {
    threads: Mutex<Threads>,
}

impl Pool {
    /// `participants - 1` worker threads, fewer where the system refuses a
    /// thread: the map then forks among those it has.
    fn new(participants: usize, #[cfg(test)] exits: &Arc<AtomicUsize>) -> Self {
        let (answer_tx, answers) = mpsc::channel();
        let mut senders = Vec::new();
        let mut handles = Vec::new();
        for worker in 1..participants.max(1) {
            let (tx, rx) = mpsc::channel::<Arc<dyn Share>>();
            let answer_tx = answer_tx.clone();
            #[cfg(test)]
            let exits = Arc::clone(exits);
            let spawned = std::thread::Builder::new()
                .name(format!("tungsten-map-{worker}"))
                .spawn(move || {
                    #[cfg(test)]
                    let _exit = Exit(exits);
                    worker_loop(worker, &rx, &answer_tx);
                });
            match spawned {
                Ok(handle) => {
                    senders.push(tx);
                    handles.push(handle);
                }
                Err(error) => {
                    log::warn!(
                        "map worker {worker} of {} did not start ({error}); maps fork among {}",
                        participants - 1,
                        handles.len() + 1
                    );
                    break;
                }
            }
        }
        Self {
            threads: Mutex::new(Threads {
                senders,
                answers,
                handles,
            }),
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        let threads = self
            .threads
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        // Closing a worker's channel ends its `recv`, and with it the thread.
        threads.senders.clear();
        for thread in threads.handles.drain(..) {
            // A worker that panicked has already failed its map.
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
#[path = "tests/workers.rs"]
mod tests;
