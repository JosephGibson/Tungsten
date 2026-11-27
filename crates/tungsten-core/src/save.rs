//! Save slots (`D-160`): the game's own serde types, each in a versioned
//! envelope in the user folder's `saves` folder, with the game's own
//! migrations.
//!
//! ```json
//! { "envelope": 1, "slot": "autosave", "schema": 2, "engine_version": "0.73.0", "saved_at_ms": 1760400000000, "data": { "…": "the game's type" } }
//! ```
//!
//! Saves of the World itself: the entities a game names ([`name`]), a run's
//! spawns from them and the entities a game keeps itself ([`Saved::kept`])
//! carry a [`SaveId`], and the components and resources a [`SaveRegistry`]
//! holds are what a save writes.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::{DeserializeOwned, IgnoredAny, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

use crate::config::GameConfig;
use crate::persist::{self, Ops, RealOps, Renamed, Staged, WriteError, write_atomic};

pub(crate) mod area;
pub(crate) mod id;
pub(crate) mod patch;
pub(crate) mod registry;
pub(crate) mod table;
pub(crate) mod world;

pub use area::{AreaError, enter_area};
pub use id::{AUTHORED, NameError, SaveId, SaveState, Saved, name, order_key, saving, take_id};
pub use registry::{
    MigrationStep, RefCodec, RefReader, RefWriter, Refs, SaveOptions, SaveRegistry,
};
pub use table::expand_tables;
pub use world::{
    CaptureError, FORMAT, RestoreError, RestoreReport, WorldSave, capture_world, check_world,
    on_restore, regenerate, restore_world,
};

/// The JSON value [`SaveData::migrate`] rewrites, re-exported so a game
/// needs no `serde_json` dependency of its own to write a migration.
pub use serde_json::Value;

/// The envelope format this build reads and writes.
const ENVELOPE: u32 = 1;
const SLOT_NAME_MAX_LEN: usize = 64;

/// A game's saved data: its serde type, its schema number and its
/// migrations.
///
/// ```
/// use serde::{Deserialize, Serialize};
/// use tungsten_core::SaveData;
/// use tungsten_core::save::Value;
///
/// #[derive(Serialize, Deserialize)]
/// struct Progress {
///     level: u32,
///     gems: u32,
/// }
///
/// impl SaveData for Progress {
///     const SCHEMA: u32 = 3;
///
///     fn migrate_step(
///         from: u32,
///         mut data: Value,
///     ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
///         match from {
///             // Schema 1 had no gems.
///             1 => data["gems"] = 0.into(),
///             // Schema 2 counted levels from 0.
///             2 => data["level"] = (data["level"].as_u64().ok_or("no level")? + 1).into(),
///             _ => return Err(format!("no migration from schema {from}").into()),
///         }
///         Ok(data)
///     }
/// }
/// ```
pub trait SaveData: Serialize + DeserializeOwned {
    /// This type's schema; bump it when the type changes shape, and add the
    /// step from the old one to [`migrate_step`](Self::migrate_step).
    const SCHEMA: u32;

    /// Rewrites `data`, saved under schema `from`, into schema `from + 1`'s
    /// shape: one schema's change. The default refuses every schema. `data`
    /// is `serde_json`'s [`Value`], which this module re-exports, so a game
    /// writes its steps over `tungsten::core::save::Value` with no `serde_json`
    /// of its own.
    ///
    /// # Errors
    ///
    /// Whatever the game's step reports; [`SaveSlots::load`] returns it as
    /// [`SaveError::Migration`].
    fn migrate_step(
        from: u32,
        data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let _ = data;
        Err(format!("schema {from} has no migration to schema {}", from + 1).into())
    }

    /// Rewrites `data`, saved under the older schema `from`, into this
    /// schema's shape: by default [`migrate_step`](Self::migrate_step) once
    /// per schema from `from` up to [`SCHEMA`](Self::SCHEMA). A type that
    /// overrides it keeps its own.
    ///
    /// # Errors
    ///
    /// The first step's error; [`SaveSlots::load`] returns it as
    /// [`SaveError::Migration`].
    fn migrate(
        from: u32,
        mut data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        for schema in from..Self::SCHEMA {
            data = Self::migrate_step(schema, data)?;
        }
        Ok(data)
    }
}

/// What [`SaveSlots::save_stamped`] writes in place of this build's: a
/// schema, an engine version and a time, for a test's older save.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SaveStamp {
    /// The schema the data is under.
    pub schema: u32,
    /// The engine version that saved it.
    pub engine_version: String,
    /// When: Unix milliseconds.
    pub saved_at_ms: u64,
}

impl SaveStamp {
    /// A stamp of `schema`, `engine_version` and `saved_at_ms`.
    #[must_use]
    pub fn new(schema: u32, engine_version: impl Into<String>, saved_at_ms: u64) -> Self {
        Self {
            schema,
            engine_version: engine_version.into(),
            saved_at_ms,
        }
    }
}

/// The game's save slots in its user folder: a `World` resource `App`
/// inserts. Without a folder (smoke mode, `UserFolder::Off`, or no
/// `game.id`) every call returns [`SaveError::NoFolder`].
///
/// A window's run writes behind ([`Self::write_behind`], `D-303`): a save
/// is serialized and written to its temp file before it returns, and synced
/// and renamed over the slot on a writer thread, so the disk's wait leaves
/// the frame. `App::new`, the test harness and the checks write each save
/// whole before it returns.
#[derive(Debug, Clone, Default)]
pub struct SaveSlots {
    dir: Option<PathBuf>,
    /// The saves staged and not yet in place, and their writer; `None`
    /// while every save is written before it returns. Clones share it.
    behind: Option<Arc<Behind>>,
}

/// The slots' saves that are staged and not yet in place, and the writer
/// thread that commits them (`D-303`). The writer starts at the first save.
struct Behind {
    shared: Arc<Shared>,
    writer: Mutex<Option<JoinHandle<()>>>,
    /// Tests: the operations the writer commits with, in place of the file
    /// system's own.
    #[cfg(test)]
    ops: Mutex<Option<Box<dyn Ops>>>,
    /// Tests: the writer thread does not start.
    #[cfg(test)]
    no_thread: bool,
}

#[derive(Default)]
struct Shared {
    state: Mutex<Queue>,
    /// The writer waits on it for a save, and a flush for the queue to
    /// empty.
    changed: Condvar,
}

#[derive(Default)]
struct Queue {
    /// The staged saves, in the order they were made.
    staged: VecDeque<Job>,
    /// Each slot's newest save not yet in place: its number and the temp
    /// file that holds it, which `load` and `list` read.
    pending: HashMap<String, Pending>,
    /// The writer holds a save.
    busy: bool,
    /// The commits that failed since the last flush.
    failed: Vec<SaveError>,
    /// The slots are gone: the writer commits what is staged and ends.
    closed: bool,
    /// The writer thread ended on a panic: saves are committed where they
    /// are made from then on.
    dead: bool,
    /// The last save's number.
    last: u64,
}

struct Job {
    slot: String,
    number: u64,
    staged: Staged,
}

struct Pending {
    number: u64,
    temp: PathBuf,
}

impl std::fmt::Debug for Behind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let queue = lock(&self.shared.state);
        f.debug_struct("Behind")
            .field("staged", &queue.staged.len())
            .field("pending", &queue.pending.len())
            .field("failed", &queue.failed.len())
            .finish_non_exhaustive()
    }
}

/// A poisoned lock's state is still whole: every write under it is one
/// insert, removal or flag.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Behind {
    fn new() -> Self {
        Self {
            shared: Arc::new(Shared::default()),
            writer: Mutex::new(None),
            #[cfg(test)]
            ops: Mutex::new(None),
            #[cfg(test)]
            no_thread: false,
        }
    }

    /// Hands `staged`, slot `slot`'s new save, to the writer, starting it
    /// at the first; where no writer runs, commits it here.
    fn commit_behind(&self, slot: &str, staged: Staged) -> Result<(), SaveError> {
        if !self.start_writer() {
            return Ok(staged.commit(&mut RealOps)?);
        }
        let mut queue = lock(&self.shared.state);
        if queue.dead {
            // The slot's older staged save, if any, is no longer its
            // newest: the flush drops it and reads go to the slot's file.
            queue.pending.remove(slot);
            drop(queue);
            return Ok(staged.commit(&mut RealOps)?);
        }
        queue.last += 1;
        let number = queue.last;
        queue.pending.insert(
            slot.to_string(),
            Pending {
                number,
                temp: staged.temp().to_path_buf(),
            },
        );
        queue.staged.push_back(Job {
            slot: slot.to_string(),
            number,
            staged,
        });
        drop(queue);
        self.shared.changed.notify_all();
        Ok(())
    }

    /// Starts the writer thread unless it runs; `false` when the system
    /// refuses one.
    fn start_writer(&self) -> bool {
        let mut writer = lock(&self.writer);
        if writer.is_some() {
            return true;
        }
        #[cfg(test)]
        if self.no_thread {
            return false;
        }
        #[cfg(test)]
        let ops: Box<dyn Ops> = lock(&self.ops).take().unwrap_or_else(|| Box::new(RealOps));
        #[cfg(not(test))]
        let ops: Box<dyn Ops> = Box::new(RealOps);
        let shared = Arc::clone(&self.shared);
        let spawned = std::thread::Builder::new()
            .name("tungsten-saves".into())
            .spawn(move || writer_loop(&shared, ops));
        match spawned {
            Ok(handle) => {
                *writer = Some(handle);
                true
            }
            Err(error) => {
                log::warn!(
                    "the save slots' writer thread did not start ({error}); a save is written before it returns"
                );
                false
            }
        }
    }

    /// Waits until every staged save is in place or has failed, and takes
    /// the failures.
    fn flush(&self) -> Vec<SaveError> {
        let mut queue = lock(&self.shared.state);
        while (!queue.staged.is_empty() && !queue.dead) || queue.busy {
            queue = self
                .shared
                .changed
                .wait(queue)
                .unwrap_or_else(PoisonError::into_inner);
        }
        // A writer that died left its queue: commit it here, in order.
        while let Some(job) = queue.staged.pop_front() {
            if queue
                .pending
                .get(&job.slot)
                .is_some_and(|pending| pending.number == job.number)
            {
                queue.pending.remove(&job.slot);
                if let Err(error) = job.staged.commit(&mut RealOps) {
                    queue.failed.push(error.into());
                }
            }
        }
        std::mem::take(&mut queue.failed)
    }
}

impl Drop for Behind {
    fn drop(&mut self) {
        lock(&self.shared.state).closed = true;
        self.shared.changed.notify_all();
        if let Some(writer) = lock(&self.writer).take() {
            // A writer that panicked left its saves to the flush below.
            let _ = writer.join();
        }
        for error in self.flush() {
            log::error!("a save was not written: {error}");
        }
    }
}

/// Marks the writer dead if its thread unwinds, so that nothing waits for
/// it again.
struct Alive<'a>(&'a Shared);

impl Drop for Alive<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            let mut queue = lock(&self.0.state);
            queue.dead = true;
            queue.busy = false;
            // The save the writer held is lost with its temp file: its
            // slot's old file stands.
            let Queue {
                staged, pending, ..
            } = &mut *queue;
            pending.retain(|_, pending| staged.iter().any(|job| job.number == pending.number));
            drop(queue);
            self.0.changed.notify_all();
        }
    }
}

/// The writer thread: commits the staged saves in order until the slots
/// are gone and the queue is empty.
fn writer_loop(shared: &Shared, mut ops: Box<dyn Ops>) {
    let _alive = Alive(shared);
    loop {
        let job = {
            let mut queue = lock(&shared.state);
            loop {
                if let Some(job) = queue.staged.pop_front() {
                    queue.busy = true;
                    break job;
                }
                if queue.closed {
                    return;
                }
                queue = shared
                    .changed
                    .wait(queue)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let slot = job.slot.clone();
        let outcome = commit_staged(shared, job, &mut *ops);
        let mut queue = lock(&shared.state);
        queue.busy = false;
        if let Err(error) = outcome {
            log::error!("save slot '{slot}' was not written; its old file stands: {error}");
            queue.failed.push(error.into());
        }
        drop(queue);
        shared.changed.notify_all();
    }
}

/// Syncs `job`'s temp file and renames it over its slot, unless a newer
/// save of the slot or a delete took its place. The sync, the slow half,
/// runs outside the lock a read takes; each rename attempt takes the lock,
/// checks the save is still the slot's newest and tries once, and a retry's
/// sleep is outside the lock again, so a read never waits on the disk.
fn commit_staged(shared: &Shared, mut job: Job, ops: &mut dyn Ops) -> Result<(), WriteError> {
    let newest = |queue: &Queue| {
        queue
            .pending
            .get(&job.slot)
            .is_some_and(|pending| pending.number == job.number)
    };
    if !newest(&lock(&shared.state)) {
        return Ok(());
    }
    if let Err(error) = job.staged.sync(ops) {
        let mut queue = lock(&shared.state);
        if newest(&queue) {
            queue.pending.remove(&job.slot);
        }
        return Err(error);
    }
    let mut attempt = 1;
    loop {
        let mut queue = lock(&shared.state);
        if !newest(&queue) {
            return Ok(());
        }
        match job.staged.rename(ops, attempt) {
            Renamed::Done => {
                queue.pending.remove(&job.slot);
                drop(queue);
                job.staged.sync_folder();
                return Ok(());
            }
            Renamed::Retry(wait) => {
                drop(queue);
                ops.sleep(wait);
                attempt += 1;
            }
            Renamed::Failed(error) => {
                queue.pending.remove(&job.slot);
                return Err(error);
            }
        }
    }
}

/// One slot as [`SaveSlots::list`] reads it: the envelope's header.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SlotInfo {
    /// The slot's name, from its file name.
    pub name: String,
    /// The schema its data was saved under.
    pub schema: u32,
    /// The engine version that saved it.
    pub engine_version: String,
    /// When it was saved: Unix milliseconds, 0 for a clock before 1970.
    pub saved_at_ms: u64,
}

/// What [`SaveSlots::list`] found.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct SlotList {
    /// Every readable slot, by name.
    pub slots: Vec<SlotInfo>,
    /// One line per `.json` file skipped as unreadable.
    pub warnings: Vec<String>,
}

/// A failed save-slot call; a runtime condition, never a panic (`D-022`).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SaveError {
    /// Smoke mode, `UserFolder::Off`, or no `game.id` and no override.
    #[error("the game has no user folder, so it has no save slots")]
    NoFolder,
    /// Not 1–64 of `a`–`z`, `0`–`9`, `.`, `_` and `-` starting with a letter
    /// or digit, or a Windows device name.
    #[error(
        "invalid slot name '{name}': expected 1-64 of a-z, 0-9, '.', '_' or '-', starting with a letter or digit, not ending in '.', and not a Windows device name"
    )]
    InvalidName {
        /// The name given.
        name: String,
    },
    /// No such slot.
    #[error("no save slot '{slot}'")]
    NotFound {
        /// The slot.
        slot: String,
    },
    /// The file is not an envelope, or its data does not fit the type.
    #[error("save slot '{slot}' is corrupt: {source}")]
    Corrupt {
        /// The slot.
        slot: String,
        /// Why it failed to parse.
        source: serde_json::Error,
    },
    /// The game's type failed to serialize.
    #[error("save slot '{slot}' could not be serialized: {source}")]
    Serialize {
        /// The slot.
        slot: String,
        /// Why.
        source: serde_json::Error,
    },
    /// An envelope format this build does not know, from a newer engine.
    #[error("save slot '{slot}' has envelope {envelope}; this build reads envelope 1")]
    UnknownEnvelope {
        /// The slot.
        slot: String,
        /// The envelope it names.
        envelope: u32,
    },
    /// Saved under a schema newer than the type's, by a newer build.
    #[error("save slot '{slot}' has schema {found}, newer than this build's {supported}")]
    NewerSchema {
        /// The slot.
        slot: String,
        /// Its schema.
        found: u32,
        /// The type's [`SaveData::SCHEMA`].
        supported: u32,
    },
    /// The game's migration failed, or its result did not fit the type.
    #[error("save slot '{slot}' failed to migrate from schema {from}: {source}")]
    Migration {
        /// The slot.
        slot: String,
        /// The schema it was saved under.
        from: u32,
        /// Why.
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Reading, listing or deleting failed.
    #[error("save slot I/O on '{}' failed: {source}", path.display())]
    Io {
        /// The file or folder.
        path: PathBuf,
        /// The failure.
        source: std::io::Error,
    },
    /// The write failed; the slot's file is unchanged.
    #[error(transparent)]
    Write(#[from] WriteError),
}

#[derive(Serialize)]
struct EnvelopeOut<'a, T> {
    envelope: u32,
    slot: &'a str,
    schema: u32,
    engine_version: &'a str,
    saved_at_ms: u64,
    data: &'a T,
}

#[derive(Deserialize)]
struct EnvelopeVersion {
    envelope: u32,
}

#[derive(Deserialize)]
struct EnvelopeIn {
    schema: u32,
    data: Value,
}

#[derive(Deserialize)]
struct Header {
    schema: u32,
    engine_version: String,
    saved_at_ms: u64,
}

impl SaveSlots {
    /// Slots in `dir`, the user folder's `saves`; `None` for none.
    #[must_use]
    pub fn new(dir: Option<PathBuf>) -> Self {
        Self { dir, behind: None }
    }

    /// From here on a save returns once its bytes are written to a temp
    /// file beside its slot, and a writer thread syncs that file and
    /// renames it over the slot (`D-303`): the wait for the disk leaves the
    /// caller, and with it the frame. `App::run` calls this for a window's
    /// run. Slots without a folder, or already writing behind, are left as
    /// they are; clones made after the call share the writer.
    ///
    /// A save still fails at once on what fails before the sync: no folder,
    /// a bad name, a type that does not serialize, a folder or a temp file
    /// that cannot be made, a full disk. [`load`](Self::load),
    /// [`list`](Self::list) and [`delete`](Self::delete) see a save the
    /// moment it returns. A sync or a rename that fails later is logged,
    /// leaves the slot's old file whole, and is [`flush`](Self::flush)'s
    /// error.
    pub fn write_behind(&mut self) {
        if self.dir.is_some() && self.behind.is_none() {
            self.behind = Some(Arc::new(Behind::new()));
        }
    }

    /// Whether saves are committed on the writer thread
    /// ([`write_behind`](Self::write_behind)).
    #[must_use]
    pub fn writes_behind(&self) -> bool {
        self.behind.is_some()
    }

    /// Waits until every save made so far is in its slot's file or has
    /// failed. Slots that do not write behind have nothing to wait for.
    /// `App::run` flushes as it exits, and dropping the last clone does.
    ///
    /// # Errors
    ///
    /// [`SaveError::Write`]: the first commit that failed since the last
    /// flush; each was logged when it failed.
    pub fn flush(&self) -> Result<(), SaveError> {
        match &self.behind {
            Some(behind) => behind.flush().into_iter().next().map_or(Ok(()), Err),
            None => Ok(()),
        }
    }

    /// The folder slots live in; created on the first save.
    #[must_use]
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// Saves `data` in `slot` through `write_atomic`, so a crash mid-save
    /// leaves the old slot.
    ///
    /// # Errors
    ///
    /// [`SaveError::NoFolder`], [`SaveError::InvalidName`],
    /// [`SaveError::Serialize`] or [`SaveError::Write`].
    pub fn save<T: SaveData>(&self, slot: &str, data: &T) -> Result<(), SaveError> {
        let stamp = SaveStamp::new(
            T::SCHEMA,
            env!("CARGO_PKG_VERSION"),
            now_ms(SystemTime::now()),
        );
        self.save_stamped(slot, data, &stamp)
    }

    /// Saves `data` in `slot` as [`save`](Self::save) does, under `stamp`'s
    /// schema, engine version and time: a test's save from an older build,
    /// written as one is, so it lists and loads as one does.
    ///
    /// # Errors
    ///
    /// As [`save`](Self::save).
    pub fn save_stamped<T: Serialize>(
        &self,
        slot: &str,
        data: &T,
        stamp: &SaveStamp,
    ) -> Result<(), SaveError> {
        let path = self.slot_path(slot)?;
        let envelope = EnvelopeOut {
            envelope: ENVELOPE,
            slot,
            schema: stamp.schema,
            engine_version: &stamp.engine_version,
            saved_at_ms: stamp.saved_at_ms,
            data,
        };
        let mut json =
            serde_json::to_vec_pretty(&envelope).map_err(|source| SaveError::Serialize {
                slot: slot.to_string(),
                source,
            })?;
        json.push(b'\n');
        match &self.behind {
            Some(behind) => behind.commit_behind(slot, persist::stage(&path, &json)?),
            None => Ok(write_atomic(&path, &json)?),
        }
    }

    /// Loads `slot` as `T`: data saved under `T::SCHEMA` directly, under an
    /// older schema through [`SaveData::migrate`], by default a step a
    /// schema. A failed load never
    /// panics and never changes the file.
    ///
    /// # Errors
    ///
    /// [`SaveError::NoFolder`], [`SaveError::InvalidName`],
    /// [`SaveError::NotFound`], [`SaveError::Io`], [`SaveError::Corrupt`],
    /// [`SaveError::UnknownEnvelope`], [`SaveError::NewerSchema`] or
    /// [`SaveError::Migration`].
    pub fn load<T: SaveData>(&self, slot: &str) -> Result<T, SaveError> {
        let path = self.slot_path(slot)?;
        let bytes = self.read_slot(slot, &path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                SaveError::NotFound {
                    slot: slot.to_string(),
                }
            } else {
                SaveError::Io {
                    path: path.clone(),
                    source,
                }
            }
        })?;
        let corrupt = |source| SaveError::Corrupt {
            slot: slot.to_string(),
            source,
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(corrupt)?;
        let version = EnvelopeVersion::deserialize(&value).map_err(corrupt)?;
        if version.envelope != ENVELOPE {
            return Err(SaveError::UnknownEnvelope {
                slot: slot.to_string(),
                envelope: version.envelope,
            });
        }
        let envelope = EnvelopeIn::deserialize(value).map_err(corrupt)?;
        if envelope.schema == T::SCHEMA {
            return T::deserialize(envelope.data).map_err(corrupt);
        }
        if envelope.schema > T::SCHEMA {
            return Err(SaveError::NewerSchema {
                slot: slot.to_string(),
                found: envelope.schema,
                supported: T::SCHEMA,
            });
        }
        let from = envelope.schema;
        let migration = |source| SaveError::Migration {
            slot: slot.to_string(),
            from,
            source,
        };
        let migrated = T::migrate(from, envelope.data).map_err(migration)?;
        T::deserialize(migrated).map_err(|error| migration(Box::new(error)))
    }

    /// Every readable slot, sorted by name, from each envelope's header. A
    /// `.json` file that is not a readable slot is skipped with a line in
    /// [`SlotList::warnings`], and [`load`](Self::load) reports it; the
    /// write's temp files are dotfiles and are not listed.
    ///
    /// A slot this engine wrote is read up to its `data` key and no further,
    /// whatever its size: the envelope writes its header first. So a slot
    /// whose header reads lists though its data may not parse, which
    /// [`load`](Self::load) then reports, and a header key repeated after
    /// `data` is not seen. A file whose keys come in another order, or whose
    /// header is not whole before `data`, is parsed whole.
    ///
    /// # Errors
    ///
    /// [`SaveError::NoFolder`], or [`SaveError::Io`] when the folder cannot
    /// be read. A folder not yet created lists no slots.
    pub fn list(&self) -> Result<SlotList, SaveError> {
        self.list_by(read_header)
    }

    /// [`list`](Self::list), each slot's header read by `read`.
    /// `read` takes a slot's file, or the temp file a pending save of it
    /// is staged in, and its name.
    fn list_by(
        &self,
        read: fn(&Path, &str) -> Result<Header, String>,
    ) -> Result<SlotList, SaveError> {
        let dir = self.dir.as_deref().ok_or(SaveError::NoFolder)?;
        let io_error = |source| SaveError::Io {
            path: dir.to_path_buf(),
            source,
        };
        // Held through the listing, so that no save moves from its temp
        // file to its slot between the names and the headers.
        let queue = self
            .behind
            .as_ref()
            .map(|behind| lock(&behind.shared.state));
        let pending = |name: &str| queue.as_ref().and_then(|queue| queue.pending.get(name));
        let mut names = Vec::new();
        match std::fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries {
                    let file_name = entry.map_err(io_error)?.file_name();
                    let Some(file_name) = file_name.to_str() else {
                        continue;
                    };
                    if let Some(stem) = file_name.strip_suffix(".json")
                        && !file_name.starts_with('.')
                    {
                        names.push(stem.to_string());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
        // A slot's first save is listed before its file is in place.
        if let Some(queue) = &queue {
            names.extend(queue.pending.keys().cloned());
        }
        names.sort_unstable();
        names.dedup();

        let mut list = SlotList::default();
        for name in names {
            let path = match pending(&name) {
                Some(pending) => pending.temp.clone(),
                None => dir.join(format!("{name}.json")),
            };
            match read(&path, &name) {
                Ok(header) => list.slots.push(SlotInfo {
                    name,
                    schema: header.schema,
                    engine_version: header.engine_version,
                    saved_at_ms: header.saved_at_ms,
                }),
                Err(reason) => list
                    .warnings
                    .push(format!("save slot '{name}' skipped: {reason}")),
            }
        }
        Ok(list)
    }

    /// Deletes `slot`.
    ///
    /// # Errors
    ///
    /// [`SaveError::NoFolder`], [`SaveError::InvalidName`],
    /// [`SaveError::NotFound`] or [`SaveError::Io`].
    pub fn delete(&self, slot: &str) -> Result<(), SaveError> {
        let path = self.slot_path(slot)?;
        // A save of the slot not yet in place is cancelled: the writer
        // finds it is no longer the slot's newest and drops it.
        let mut queue = self
            .behind
            .as_ref()
            .map(|behind| lock(&behind.shared.state));
        let cancelled = queue
            .as_mut()
            .is_some_and(|queue| queue.pending.remove(slot).is_some());
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                if cancelled {
                    Ok(())
                } else {
                    Err(SaveError::NotFound {
                        slot: slot.to_string(),
                    })
                }
            }
            Err(source) => Err(SaveError::Io { path, source }),
        }
    }

    /// `slot`'s bytes: its newest save's, from the temp file while that
    /// save is not yet in place, else the slot's file. Read under the
    /// writer's lock, so the file is not renamed between the two.
    fn read_slot(&self, slot: &str, path: &Path) -> std::io::Result<Vec<u8>> {
        let Some(behind) = &self.behind else {
            return std::fs::read(path);
        };
        let queue = lock(&behind.shared.state);
        if let Some(pending) = queue.pending.get(slot) {
            match std::fs::read(&pending.temp) {
                // A sync that just failed removed it: the old file stands.
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                read => return read,
            }
        }
        std::fs::read(path)
    }

    fn slot_path(&self, slot: &str) -> Result<PathBuf, SaveError> {
        if !is_valid_slot_name(slot) {
            return Err(SaveError::InvalidName {
                name: slot.to_string(),
            });
        }
        let dir = self.dir.as_deref().ok_or(SaveError::NoFolder)?;
        Ok(dir.join(format!("{slot}.json")))
    }
}

/// `game.id`'s rule, lowercase only, so no two names collide on a
/// case-insensitive file system.
fn is_valid_slot_name(name: &str) -> bool {
    let game = GameConfig {
        id: Some(name.to_string()),
        ..GameConfig::default()
    };
    !name.is_empty()
        && name.len() <= SLOT_NAME_MAX_LEN
        && !name.bytes().any(|b| b.is_ascii_uppercase())
        && game.validate().is_ok()
}

/// A slot's header: read up to the `data` key where the envelope writes its
/// header first ([`header_before_data`]), else from the file parsed whole,
/// whose answer and whose reason for a file it skips are what every file got
/// before.
fn read_header(path: &Path, name: &str) -> Result<Header, String> {
    if is_valid_slot_name(name)
        && let Ok(file) = std::fs::File::open(path)
        && let Some(header) = header_before_data(file)
    {
        return Ok(header);
    }
    read_header_whole(path, name)
}

/// A slot's header from its file read and parsed whole.
fn read_header_whole(path: &Path, name: &str) -> Result<Header, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if !is_valid_slot_name(name) {
        return Err("not a valid slot name".into());
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let version = EnvelopeVersion::deserialize(&value).map_err(|e| e.to_string())?;
    if version.envelope != ENVELOPE {
        return Err(format!("envelope {}", version.envelope));
    }
    Header::deserialize(&value).map_err(|e| e.to_string())
}

/// The header of an envelope of this build's format whose `schema`,
/// `engine_version` and `saved_at_ms` all come before its `data`, as
/// [`SaveSlots::save`] writes them: read through a buffer up to the `data`
/// key and no further, so a slot costs its header whatever its data weighs.
/// `None` for anything else: another envelope, a key missing or of another
/// type, no `data`, or no JSON object at all.
fn header_before_data(file: impl std::io::Read) -> Option<Header> {
    /// The envelope's keys this read tells apart.
    enum Key {
        Envelope,
        Schema,
        EngineVersion,
        SavedAtMs,
        Data,
        Other,
    }

    impl<'de> Deserialize<'de> for Key {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Name;
            impl Visitor<'_> for Name {
                type Value = Key;

                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("an envelope key")
                }

                fn visit_str<E>(self, name: &str) -> Result<Key, E> {
                    Ok(match name {
                        "envelope" => Key::Envelope,
                        "schema" => Key::Schema,
                        "engine_version" => Key::EngineVersion,
                        "saved_at_ms" => Key::SavedAtMs,
                        "data" => Key::Data,
                        _ => Key::Other,
                    })
                }
            }
            deserializer.deserialize_identifier(Name)
        }
    }

    /// Fills its header at the `data` key, then stops the read with an
    /// error: the only way out of a map that leaves its rest unread.
    struct UpToData<'a>(&'a mut Option<Header>);

    impl<'de> Visitor<'de> for UpToData<'_> {
        type Value = ();

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a save envelope")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
            let (mut envelope, mut schema, mut engine_version, mut saved_at_ms) =
                (None, None, None, None);
            // A key given twice keeps its last value, as the whole parse's
            // map does.
            while let Some(key) = map.next_key::<Key>()? {
                match key {
                    Key::Envelope => envelope = Some(map.next_value::<u32>()?),
                    Key::Schema => schema = Some(map.next_value::<u32>()?),
                    Key::EngineVersion => engine_version = Some(map.next_value::<String>()?),
                    Key::SavedAtMs => saved_at_ms = Some(map.next_value::<u64>()?),
                    Key::Other => {
                        map.next_value::<IgnoredAny>()?;
                    }
                    Key::Data => {
                        if let (
                            Some(ENVELOPE),
                            Some(schema),
                            Some(engine_version),
                            Some(saved_at_ms),
                        ) = (envelope, schema, engine_version, saved_at_ms)
                        {
                            *self.0 = Some(Header {
                                schema,
                                engine_version,
                                saved_at_ms,
                            });
                        }
                        return Err(serde::de::Error::custom("the header ends at the data"));
                    }
                }
            }
            Ok(())
        }
    }

    let mut header = None;
    let mut json = serde_json::Deserializer::from_reader(std::io::BufReader::new(file));
    // The result is the stop itself or the reason there is no header.
    let _ = json.deserialize_map(UpToData(&mut header));
    header
}

/// Unix milliseconds; 0 for a clock before 1970.
fn now_ms(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH).map_or(0, |since| {
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    })
}

#[cfg(test)]
#[path = "tests/save.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/save/oracle.rs"]
pub(crate) mod oracle;
