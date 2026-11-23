//! The step's worker pool (`D-298`): threads kept between forks, each parked
//! on its own channel until the step hands it a stage's share.
//!
//! A fork moves the buffers its stage reads into one [`ForkData`], shares it
//! with every participant, the calling thread among them, and takes it back
//! when all have answered. A stage cuts its work into fixed chunks which the
//! participants claim through [`ForkData::next`]; each writes into its own
//! [`WorkerScratch`] with the chunk's index, and the caller lays the chunks
//! out in index order. What the step reads after a fork is therefore what
//! its serial loop writes, whichever thread took a chunk and however many
//! there are.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use glam::Vec4;

use super::ProxyLayers;
use super::pairs::{PairGrid, WorkerScratch};
use crate::physics::collision::Aabb;

/// How long the calling thread spins for a worker's answer before it blocks:
/// the workers finish within microseconds of its own share.
const ANSWER_SPIN: Duration = Duration::from_micros(50);

/// What a fork runs.
#[derive(Debug, Clone, Copy, Default)]
pub(super) enum Stage {
    /// Nothing: a `ForkData` at rest.
    #[default]
    None,
    /// The pair build's candidate walks over `bodies` entity proxies, in
    /// `chunks` chunks of `chunk`.
    Build {
        bodies: usize,
        chunk: usize,
        chunks: usize,
    },
    /// A share that panics in one participant, for the pool's own test.
    #[cfg(test)]
    Fail { participant: usize },
}

/// The step's buffers a fork's participants read, moved in for the fork and
/// back after it, with the stage and its chunk counter.
#[derive(Debug, Default)]
pub(super) struct ForkData {
    pub(super) stage: Stage,
    /// The next chunk no participant has taken.
    pub(super) next: AtomicUsize,
    pub(super) grid: PairGrid,
    pub(super) pair_flags: Vec<u8>,
    pub(super) pair_bounds: Vec<Vec4>,
    pub(super) pair_aabbs: Vec<Aabb>,
    pub(super) layers: ProxyLayers,
}

impl ForkData {
    /// The next chunk of `chunks` free, taken; `None` once all are.
    pub(super) fn take_chunk(&self, chunks: usize) -> Option<usize> {
        let chunk = self.next.fetch_add(1, Ordering::Relaxed);
        (chunk < chunks).then_some(chunk)
    }
}

/// The share of a fork's stage that `participant` takes: 0 is the calling
/// thread, the rest the workers. A stage's chunks are claimed, so only the
/// pool's own test reads who runs.
#[cfg_attr(not(test), allow(unused_variables))]
fn work(data: &ForkData, scratch: &mut WorkerScratch, participant: usize) {
    match data.stage {
        Stage::None => {}
        Stage::Build {
            bodies,
            chunk,
            chunks,
        } => super::pairs::build_work(data, scratch, bodies, chunk, chunks),
        #[cfg(test)]
        Stage::Fail {
            participant: failing,
        } => {
            assert!(failing != participant, "participant {participant} fails");
        }
    }
}

/// A worker's share of a fork: the data, and the worker's scratch, which
/// travels to the thread and back.
struct Job {
    data: Arc<ForkData>,
    scratch: WorkerScratch,
}

/// A worker's answer: its scratch, or `None` when its share panicked.
type Answer = (usize, Option<WorkerScratch>);

/// Sends a worker's answer when it drops, so a share that panics still
/// answers and the step does not wait for it forever.
struct Answering<'a> {
    answers: &'a mpsc::Sender<Answer>,
    worker: usize,
    scratch: Option<WorkerScratch>,
}

impl Drop for Answering<'_> {
    fn drop(&mut self) {
        // The step went away with its receiver: nothing waits.
        let _ = self.answers.send((self.worker, self.scratch.take()));
    }
}

fn worker_loop(worker: usize, jobs: &mpsc::Receiver<Job>, answers: &mpsc::Sender<Answer>) {
    // Parked in `recv` between forks; a closed channel ends the thread.
    while let Ok(Job { data, mut scratch }) = jobs.recv() {
        let mut answering = Answering {
            answers,
            worker,
            scratch: None,
        };
        work(&data, &mut scratch, worker);
        // The caller takes the data back once every worker has answered.
        drop(data);
        answering.scratch = Some(scratch);
    }
}

/// The pool's threads: each worker's channel, the channel they answer on and
/// their handles.
#[derive(Debug)]
struct Threads {
    senders: Vec<mpsc::Sender<Job>>,
    answers: mpsc::Receiver<Answer>,
    handles: Vec<JoinHandle<()>>,
}

/// The step's participants: the worker threads, each participant's scratch
/// (the caller's first) and the merge's chunk table. Made at the step's first
/// fork, so a world that never crosses a stage's threshold starts no thread.
#[derive(Debug)]
pub(super) struct Workers {
    /// The participants asked for, which a failed spawn can leave unmet.
    wanted: usize,
    /// Behind a `Mutex` only for what the lock lends the step's buffers:
    /// `Sync` and unwind safety, which a channel's receiver lacks. The step
    /// reaches the threads through `get_mut` and never locks.
    threads: Mutex<Threads>,
    /// A scratch a participant: the caller's, then each worker's.
    pub(super) scratches: Vec<WorkerScratch>,
    /// Per chunk of the last fork: the participant that took it and where
    /// its output starts and how long it is, as the stage's merge reads it.
    pub(super) chunk_at: Vec<(u32, u32, u32)>,
    /// The forks run, for the tests that must see a stage fork.
    #[cfg(test)]
    pub(super) forks: usize,
}

impl Workers {
    /// `participants - 1` worker threads, fewer where the system refuses a
    /// thread: the step then forks among those it has.
    pub(super) fn new(participants: usize) -> Self {
        let (answer_tx, answers) = mpsc::channel();
        let mut senders = Vec::new();
        let mut handles = Vec::new();
        for worker in 1..participants.max(1) {
            let (tx, rx) = mpsc::channel::<Job>();
            let answer_tx = answer_tx.clone();
            let spawned = std::thread::Builder::new()
                .name(format!("tungsten-physics-{worker}"))
                .spawn(move || worker_loop(worker, &rx, &answer_tx));
            match spawned {
                Ok(handle) => {
                    senders.push(tx);
                    handles.push(handle);
                }
                Err(error) => {
                    log::warn!(
                        "physics worker {worker} of {} did not start ({error}); the step forks among {}",
                        participants - 1,
                        handles.len() + 1
                    );
                    break;
                }
            }
        }
        let mut scratches = Vec::new();
        scratches.resize_with(senders.len() + 1, WorkerScratch::default);
        Self {
            wanted: participants,
            threads: Mutex::new(Threads {
                senders,
                answers,
                handles,
            }),
            scratches,
            chunk_at: Vec::new(),
            #[cfg(test)]
            forks: 0,
        }
    }

    /// The step's pool for `participants`, made at the first fork and made
    /// again when the count changed.
    pub(super) fn of(slot: &mut Option<Box<Self>>, participants: usize) -> &mut Self {
        if slot
            .as_ref()
            .is_some_and(|workers| workers.wanted != participants)
        {
            *slot = None;
        }
        slot.get_or_insert_with(|| Box::new(Self::new(participants)))
    }

    /// Runs `data`'s stage on every participant and gives the data back,
    /// each participant's output in its scratch.
    ///
    /// # Panics
    ///
    /// Panics when a worker's share panicked, as the serial loop would have.
    pub(super) fn fork(&mut self, data: ForkData) -> ForkData {
        #[cfg(test)]
        {
            self.forks += 1;
        }
        let data = Arc::new(data);
        let threads = self
            .threads
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        for (index, sender) in threads.senders.iter().enumerate() {
            let job = Job {
                data: Arc::clone(&data),
                scratch: std::mem::take(&mut self.scratches[index + 1]),
            };
            sender
                .send(job)
                .expect("a physics worker thread ended before its pool");
        }
        work(&data, &mut self.scratches[0], 0);
        let waited = Instant::now();
        let mut panicked = false;
        for _ in 0..threads.senders.len() {
            let (worker, scratch) = loop {
                match threads.answers.try_recv() {
                    Ok(answer) => break answer,
                    Err(mpsc::TryRecvError::Empty) if waited.elapsed() < ANSWER_SPIN => {
                        std::hint::spin_loop();
                    }
                    Err(_) => {
                        break threads
                            .answers
                            .recv()
                            .expect("a physics worker thread ended before its pool");
                    }
                }
            };
            match scratch {
                Some(scratch) => self.scratches[worker] = scratch,
                None => panicked = true,
            }
        }
        assert!(!panicked, "a physics worker's share of the step panicked");
        Arc::try_unwrap(data).expect("every physics worker drops the step's data before it answers")
    }

    /// Lays the last fork's chunks out in index order: `chunk_at[chunk]`
    /// names the participant whose scratch holds it, from the spans each
    /// participant recorded.
    pub(super) fn index_chunks(
        &mut self,
        chunks: usize,
        spans: impl Fn(&WorkerScratch) -> &[(u32, u32, u32)],
    ) {
        self.chunk_at.clear();
        self.chunk_at.resize(chunks, (0, 0, 0));
        for (participant, scratch) in self.scratches.iter().enumerate() {
            for &(chunk, start, count) in spans(scratch) {
                self.chunk_at[chunk as usize] = (participant as u32, start, count);
            }
        }
    }
}

impl Drop for Workers {
    fn drop(&mut self) {
        let threads = self
            .threads
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        // Closing a worker's channel ends its `recv`, and with it the thread.
        threads.senders.clear();
        for thread in threads.handles.drain(..) {
            // A worker that panicked has already failed its step.
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
#[path = "../../tests/physics/workers.rs"]
mod tests;
