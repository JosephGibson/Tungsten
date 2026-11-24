//! The step's worker pool (`D-298`): threads kept between forks, each parked
//! on its own channel until the step hands it a stage's share. While its step
//! runs a worker may spin for the next fork before it parks (`D-300`).
//!
//! A fork moves the buffers its stage reads into one [`ForkData`], shares it
//! with every participant, the calling thread among them, and takes it back
//! when all have answered. A stage cuts its work into fixed chunks which the
//! participants claim through [`ForkData::next`]; each writes into its own
//! [`WorkerScratch`] with the chunk's index, and the caller lays the chunks
//! out in index order. The narrow phase writes each chunk into its own
//! [`NarrowSlot`] instead, which the substep's pair loop consumes in index
//! order while the fork runs ([`Workers::fork_with`]). What the step reads
//! after a fork is therefore what its serial loop writes, whichever thread
//! took a chunk and however many there are.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use glam::Vec4;

use super::arrival::Round;
use super::pairs::{PairGrid, WorkerScratch};
use super::{Proxy, ProxyLayers};
use crate::physics::broadphase::SpatialGrid;
use crate::physics::collision::{Aabb, Contact};

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
    /// An arrival round's search for the pairs its listed bodies reach.
    Arrival(Round),
    /// A repair's removal scan, in `chunks` chunks of `chunk` pairs.
    Remove {
        chunk: usize,
        chunks: usize,
        friction_on: bool,
    },
    /// A repair's walks for its members' pairs, in `chunks` chunks of
    /// `chunk` members.
    Repair {
        chunk: usize,
        chunks: usize,
        friction_on: bool,
        carried: bool,
        carried_tangents: bool,
    },
    /// The pair loop's narrow phase, in `chunks` chunks of `chunk` pairs, a
    /// chunk's contacts written to its [`NarrowSlot`].
    Narrow {
        sub_dt: f32,
        slack: f32,
        chunk: usize,
        chunks: usize,
    },
    /// A share that panics in one participant, for the pool's own test.
    #[cfg(test)]
    Fail { participant: usize },
}

/// The step's buffers a fork's participants read, moved in for the fork and
/// back after it, with the stage and its chunk counter, and the narrow
/// phase's slots, which its participants write.
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
    pub(super) proxies: Vec<Proxy>,
    pub(super) repair_grid: SpatialGrid,
    pub(super) repaired: Vec<u32>,
    pub(super) oversize: Vec<u32>,
    /// The arrival pass's listed bodies, or a repair's members.
    pub(super) list: Vec<u32>,
    /// The substep's pair list, and for a repair's removal its impulses,
    /// tangents and sent flags.
    pub(super) pairs: Vec<(u32, u32)>,
    pub(super) pair_impulses: Vec<f32>,
    pub(super) pair_tangents: Vec<f32>,
    pub(super) sent_pairs: Vec<bool>,
    /// For a repair's walks: the step's sent pairs that a producer finds by
    /// their proxies (`D-270`, `SentRecord::found`), and the removed pairs'
    /// carried impulses and tangents by key.
    pub(super) found: super::pairs::PairSet,
    pub(super) repair_impulses: super::pairs::ImpulseMap,
    pub(super) repair_tangents: super::pairs::ImpulseMap,
    /// The narrow phase's chunks, a slot a chunk, each filled by the
    /// participant that takes it and marked whole: the pair loop consumes a
    /// chunk as soon as it is.
    pub(super) narrow_slots: Vec<NarrowSlot>,
    /// Tests: the narrow chunk whose worker panics after taking it.
    #[cfg(test)]
    pub(super) fail_narrow: Option<usize>,
    /// Tests: how long a worker waits before each narrow claim, and between a
    /// claim and its chunk.
    #[cfg(test)]
    pub(super) claim_delay: Duration,
    #[cfg(test)]
    pub(super) publish_delay: Duration,
}

/// A narrow chunk's contacts, a contact a pair, and whether they are whole.
#[derive(Debug, Default)]
pub(super) struct NarrowSlot {
    pub(super) done: AtomicBool,
    pub(super) contacts: Mutex<Vec<Option<Contact>>>,
}

/// Waits for a narrow chunk another participant took. A worker whose share
/// unwound fails the step here, instead of leaving its chunk unmarked and
/// this wait unending.
pub(super) fn wait_slot(slot: &NarrowSlot, shared: &Shared) {
    let waited = Instant::now();
    while !slot.done.load(Ordering::Acquire) {
        assert!(
            !shared.failed.load(Ordering::Acquire),
            "a physics worker's share of the step panicked"
        );
        if waited.elapsed() < ANSWER_SPIN {
            std::hint::spin_loop();
        } else {
            std::thread::yield_now();
        }
    }
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
        Stage::Arrival(round) => super::arrival::search_work(data, scratch, &round),
        Stage::Remove {
            chunk,
            chunks,
            friction_on,
        } => super::pairs::remove_work(data, scratch, chunk, chunks, friction_on),
        Stage::Repair {
            chunk,
            chunks,
            friction_on,
            carried,
            carried_tangents,
        } => super::pairs::repair_work(
            data,
            scratch,
            chunk,
            chunks,
            friction_on,
            carried,
            carried_tangents,
        ),
        Stage::Narrow {
            sub_dt,
            slack,
            chunk,
            chunks,
        } => super::solver::narrow_work(data, sub_dt, slack, chunk, chunks),
        #[cfg(test)]
        Stage::Fail {
            participant: failing,
        } => {
            assert!(failing != participant, "participant {participant} fails");
        }
    }
}

/// How the step forks (`D-298`): the World's worker count as this step read
/// it, and the pool, made at the first stage that forks.
#[derive(Debug, Default)]
pub(super) struct Fork {
    workers: Option<Box<Workers>>,
    /// The threads a forked stage runs on, the calling thread among them; 0
    /// and 1 are the serial step.
    pub(super) participants: u32,
    /// Whether a stage forks whatever its size, as the lockstep tests ask.
    pub(super) any: bool,
    /// How long a worker spins for the step's next fork before it parks, in
    /// microseconds (`D-300`); 0 parks at once. It sits in the padding the
    /// two fields above leave.
    pub(super) spin_us: u16,
}

impl Fork {
    /// Whether a stage of `size` forks: with two participants or more, at
    /// `min` and above.
    #[inline]
    pub(super) fn splits(&self, size: usize, min: usize) -> bool {
        self.participants > 1 && (size >= min || self.any)
    }

    /// A stage's chunk: `usual`, or `forced` where the lockstep tests fork a
    /// small world, so that it has chunks enough for every participant.
    pub(super) fn chunk(&self, usual: usize, forced: usize) -> usize {
        if self.any { forced } else { usual }
    }

    /// The pool, made at the first fork and made again when the count or
    /// the spin time changed.
    pub(super) fn workers(&mut self) -> &mut Workers {
        let spin = Duration::from_micros(u64::from(self.spin_us));
        Workers::of(&mut self.workers, self.participants as usize, spin)
    }

    /// The step has run its last fork: a worker spinning for another parks
    /// (`D-300`), so the pool is busy for no longer than its step.
    pub(super) fn rest(&mut self) {
        if let Some(workers) = &self.workers {
            workers.shared.stepping.store(false, Ordering::Release);
        }
    }

    /// The pool, once a stage has forked.
    #[cfg(test)]
    pub(super) fn pool(&self) -> Option<&Workers> {
        self.workers.as_deref()
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
    /// Set as the share unwinds, for a caller waiting on one of its chunks.
    failed: &'a AtomicBool,
}

impl Drop for Answering<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.failed.store(true, Ordering::Release);
        }
        // The step went away with its receiver: nothing waits.
        let _ = self.answers.send((self.worker, self.scratch.take()));
    }
}

/// What the step shares with its workers beside their channels.
#[derive(Debug, Default)]
pub(super) struct Shared {
    /// Set by a fork and cleared at the step's end ([`Fork::rest`]): a
    /// worker spins for the next fork only while its step runs.
    stepping: AtomicBool,
    /// Set by a worker whose share unwound, for a caller that waits on a
    /// chunk of its ([`wait_slot`]); cleared at each fork.
    pub(super) failed: AtomicBool,
    /// Tests: the forks a worker took out of its spin.
    #[cfg(test)]
    pub(super) spun: AtomicUsize,
    /// Tests: the times a worker left its spin without a fork and parked.
    #[cfg(test)]
    pub(super) parked: AtomicUsize,
}

/// The step's next fork, when it comes while `shared`'s step still runs and
/// within `spin` of the worker's last answer (`D-300`): the worker polls its
/// channel under `spin_loop` and so takes the fork without a wake-up.
/// `Ok(None)` once it should park, at once when `spin` is zero; `Err` when
/// the channel has closed.
fn spin_for(
    jobs: &mpsc::Receiver<Job>,
    spin: Duration,
    shared: &Shared,
) -> Result<Option<Job>, ()> {
    if spin.is_zero() {
        return Ok(None);
    }
    let answered = Instant::now();
    loop {
        match jobs.try_recv() {
            Ok(job) => {
                #[cfg(test)]
                shared.spun.fetch_add(1, Ordering::Relaxed);
                return Ok(Some(job));
            }
            Err(mpsc::TryRecvError::Disconnected) => return Err(()),
            Err(mpsc::TryRecvError::Empty) => {
                if !shared.stepping.load(Ordering::Acquire) || answered.elapsed() >= spin {
                    #[cfg(test)]
                    shared.parked.fetch_add(1, Ordering::Relaxed);
                    return Ok(None);
                }
                std::hint::spin_loop();
            }
        }
    }
}

fn worker_loop(
    worker: usize,
    jobs: &mpsc::Receiver<Job>,
    answers: &mpsc::Sender<Answer>,
    spin: Duration,
    shared: &Shared,
) {
    // Parked in `recv` between steps; a closed channel ends the thread.
    while let Ok(mut job) = jobs.recv() {
        loop {
            let Job { data, mut scratch } = job;
            let mut answering = Answering {
                answers,
                worker,
                scratch: None,
                failed: &shared.failed,
            };
            work(&data, &mut scratch, worker);
            // The caller takes the data back once every worker has answered.
            drop(data);
            answering.scratch = Some(scratch);
            drop(answering);
            match spin_for(jobs, spin, shared) {
                Ok(Some(next)) => job = next,
                Ok(None) => break,
                Err(()) => return,
            }
        }
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
    /// How long a worker spins for the step's next fork (`D-300`).
    pub(super) spin: Duration,
    /// The step's flag and the tests' counts, shared with every worker.
    pub(super) shared: Arc<Shared>,
    /// Behind a `Mutex` only for what the lock lends the step's buffers:
    /// `Sync` and unwind safety, which a channel's receiver lacks. The step
    /// reaches the threads through `get_mut` and never locks.
    threads: Mutex<Threads>,
    /// A scratch a participant: the caller's, then each worker's.
    pub(super) scratches: Vec<WorkerScratch>,
    /// Per chunk of the last fork: the participant that took it and the two
    /// numbers its span holds, where its output starts and how long it is
    /// for most stages, as the stage's merge reads them.
    pub(super) chunk_at: Vec<(u32, u32, u32)>,
    /// Per body of a forked arrival round, in list order: the participant
    /// that searched it and where its pairs start and how many they are.
    pub(super) body_spans: Vec<(u32, u32, u32)>,
    /// The arrival rounds forked, for the tests that must see one.
    #[cfg(test)]
    pub(super) arrival_forks: usize,
    /// The narrow phases forked, likewise.
    #[cfg(test)]
    pub(super) narrow_forks: usize,
    /// Tests: the narrow chunks the pair loop's own thread computed, having
    /// found them untaken.
    #[cfg(test)]
    pub(super) caller_claims: usize,
    /// Tests: the narrow chunks the pair loop's own thread waited for, and
    /// the wakes and keeps its byte decided.
    #[cfg(test)]
    pub(super) caller_waits: usize,
    #[cfg(test)]
    pub(super) woken_by_byte: usize,
    #[cfg(test)]
    pub(super) kept_by_byte: usize,
    /// Tests: the builds whose carry remap ran while the workers walked.
    #[cfg(test)]
    pub(super) remaps_overlapped: usize,
    /// Tests: the repairs whose walks forked, and those whose removal did;
    /// the forked removals that carried tangents, and the forked walks that
    /// read carried tangents or admitted an oversize proxy's pair.
    #[cfg(test)]
    pub(super) repair_forks: usize,
    #[cfg(test)]
    pub(super) removal_forks: usize,
    #[cfg(test)]
    pub(super) tangent_removals: usize,
    #[cfg(test)]
    pub(super) tangent_repairs: usize,
    #[cfg(test)]
    pub(super) oversize_repairs: usize,
    /// The forks run, for the tests that must see a stage fork.
    #[cfg(test)]
    pub(super) forks: usize,
}

impl Workers {
    /// `participants - 1` worker threads, fewer where the system refuses a
    /// thread: the step then forks among those it has. Cold and out of
    /// line: it runs once a pool, and inlined it would sit in every caller.
    #[cold]
    #[inline(never)]
    pub(super) fn new(participants: usize, spin: Duration) -> Self {
        let (answer_tx, answers) = mpsc::channel();
        let shared = Arc::new(Shared::default());
        let mut senders = Vec::new();
        let mut handles = Vec::new();
        for worker in 1..participants.max(1) {
            let (tx, rx) = mpsc::channel::<Job>();
            let answer_tx = answer_tx.clone();
            let shared = Arc::clone(&shared);
            let spawned = std::thread::Builder::new()
                .name(format!("tungsten-physics-{worker}"))
                .spawn(move || worker_loop(worker, &rx, &answer_tx, spin, &shared));
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
            spin,
            shared,
            threads: Mutex::new(Threads {
                senders,
                answers,
                handles,
            }),
            scratches,
            chunk_at: Vec::new(),
            body_spans: Vec::new(),
            #[cfg(test)]
            arrival_forks: 0,
            #[cfg(test)]
            narrow_forks: 0,
            #[cfg(test)]
            caller_claims: 0,
            #[cfg(test)]
            caller_waits: 0,
            #[cfg(test)]
            woken_by_byte: 0,
            #[cfg(test)]
            kept_by_byte: 0,
            #[cfg(test)]
            remaps_overlapped: 0,
            #[cfg(test)]
            repair_forks: 0,
            #[cfg(test)]
            removal_forks: 0,
            #[cfg(test)]
            tangent_removals: 0,
            #[cfg(test)]
            tangent_repairs: 0,
            #[cfg(test)]
            oversize_repairs: 0,
            #[cfg(test)]
            forks: 0,
        }
    }

    /// The step's pool for `participants` spinning `spin`, made at the first
    /// fork and made again when either changed.
    fn of(slot: &mut Option<Box<Self>>, participants: usize, spin: Duration) -> &mut Self {
        if slot
            .as_ref()
            .is_some_and(|workers| workers.wanted != participants || workers.spin != spin)
        {
            *slot = None;
        }
        slot.get_or_insert_with(|| Box::new(Self::new(participants, spin)))
    }

    /// Runs `data`'s stage on every participant and gives the data back,
    /// each participant's output in its scratch.
    ///
    /// # Panics
    ///
    /// Panics when a worker's share panicked, as the serial loop would have.
    pub(super) fn fork(&mut self, data: ForkData) -> ForkData {
        self.fork_with(data, |data, scratch, _| work(data, scratch, 0))
            .0
    }

    /// [`Self::fork`] with `caller` in place of the calling thread's share:
    /// it runs while the workers run theirs, with the fork's data, the
    /// caller's scratch and what the step shares with its workers, and its
    /// result comes back beside the data.
    ///
    /// # Panics
    ///
    /// As [`Self::fork`].
    pub(super) fn fork_with<R>(
        &mut self,
        data: ForkData,
        caller: impl FnOnce(&ForkData, &mut WorkerScratch, &Shared) -> R,
    ) -> (ForkData, R) {
        #[cfg(test)]
        {
            self.forks += 1;
        }
        let data = Arc::new(data);
        self.shared.failed.store(false, Ordering::Relaxed);
        // The step runs: a worker may spin for its next fork.
        self.shared.stepping.store(true, Ordering::Release);
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
        let out = caller(&data, &mut self.scratches[0], &self.shared);
        let waited = Instant::now();
        // Workers that spin answer within their spin time or not at all.
        let patience = self.spin.max(ANSWER_SPIN);
        let mut panicked = false;
        for _ in 0..threads.senders.len() {
            let (worker, scratch) = loop {
                match threads.answers.try_recv() {
                    Ok(answer) => break answer,
                    Err(mpsc::TryRecvError::Empty) if waited.elapsed() < patience => {
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
        let data = Arc::try_unwrap(data)
            .expect("every physics worker drops the step's data before it answers");
        (data, out)
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
