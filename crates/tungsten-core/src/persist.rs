//! Atomic file writes shared by the action map, the settings and the save
//! slots (`D-159`), public for a game's own files.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use thiserror::Error;

/// Rename attempts on Windows before a sharing failure is [`WriteError::Busy`].
const ATTEMPTS: u32 = 5;

/// A failed [`write_atomic`]; the file at the path is unchanged.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WriteError {
    /// Creating the folder, writing the temp file or renaming it failed.
    #[error("failed to write '{}': {source}", path.display())]
    Io {
        /// The file that was being written.
        path: PathBuf,
        /// The failure.
        source: std::io::Error,
    },
    /// Another process held the file open through every attempt (Windows).
    #[error("'{}' stayed in use after {attempts} attempts: {source}", path.display())]
    Busy {
        /// The file that was being written.
        path: PathBuf,
        /// How many renames were tried.
        attempts: u32,
        /// The last attempt's failure.
        source: std::io::Error,
    },
}

impl WriteError {
    /// The underlying error, for callers whose own error keeps an `io::Error`.
    pub(crate) fn into_source(self) -> std::io::Error {
        match self {
            Self::Io { source, .. } | Self::Busy { source, .. } => source,
        }
    }
}

/// Writes `contents` to `path` so a reader sees the old file or the new one,
/// never a torn one: a temp file in the same folder, `sync_all`, `rename`,
/// then on Unix an `fsync` of the folder. Creates the folder.
///
/// On Windows a rename refused because another process holds the file (an
/// antivirus scan, a backup tool) is retried with a short backoff before it
/// becomes [`WriteError::Busy`]. Power-loss durability on Windows is not
/// claimed.
pub fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), WriteError> {
    write_atomic_with(path, contents, &mut RealOps)
}

/// The operations an atomic write performs, injected by tests.
pub(crate) trait Ops: Send {
    /// Whether the platform's sharing errors are retried.
    fn windows(&self) -> bool;
    /// Writes the temp file.
    fn write(&mut self, file: &mut File, contents: &[u8]) -> std::io::Result<()>;
    /// Syncs the temp file's bytes to the disk.
    fn sync(&mut self, file: &mut File) -> std::io::Result<()>;
    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()>;
    fn sleep(&mut self, duration: Duration);
}

/// The file system's own operations.
#[derive(Debug)]
pub(crate) struct RealOps;

impl Ops for RealOps {
    fn windows(&self) -> bool {
        cfg!(windows)
    }

    fn write(&mut self, file: &mut File, contents: &[u8]) -> std::io::Result<()> {
        file.write_all(contents)
    }

    fn sync(&mut self, file: &mut File) -> std::io::Result<()> {
        file.sync_all()
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// A file staged for an atomic write ([`stage`]): its temp file written
/// beside the destination, not yet synced or renamed over it. The save
/// slots stage on the frame and commit on their writer thread (`D-303`);
/// [`write_atomic`] does both in a row. Dropped before its rename, it
/// removes its temp file and leaves the destination as it was.
#[derive(Debug)]
pub(crate) struct Staged {
    path: PathBuf,
    temp: PathBuf,
    /// Open until it is synced.
    file: Option<File>,
    /// The temp file is still to be renamed or removed.
    pending: bool,
}

/// How one rename attempt of a [`Staged`] file ended.
#[derive(Debug)]
pub(crate) enum Renamed {
    /// The file is in place.
    Done,
    /// A Windows sharing failure: try again after this sleep.
    Retry(Duration),
    /// The rename failed: the temp file is gone and the destination is
    /// unchanged.
    Failed(WriteError),
}

/// Makes `path`'s folder, creates a temp file in it and writes `contents`
/// there: every failure a full disk or a refused folder gives, before
/// anything is synced.
pub(crate) fn stage(path: &Path, contents: &[u8]) -> Result<Staged, WriteError> {
    stage_with(path, contents, &mut RealOps)
}

pub(crate) fn stage_with(
    path: &Path,
    contents: &[u8],
    ops: &mut dyn Ops,
) -> Result<Staged, WriteError> {
    let io_error = |source| WriteError::Io {
        path: path.to_path_buf(),
        source,
    };
    let parent = parent_of(path);
    std::fs::create_dir_all(parent).map_err(io_error)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let (temp, mut file) = create_temp_file(parent, file_name).map_err(io_error)?;
    if let Err(source) = ops.write(&mut file, contents) {
        drop(file);
        let _ = std::fs::remove_file(&temp);
        return Err(io_error(source));
    }
    Ok(Staged {
        path: path.to_path_buf(),
        temp,
        file: Some(file),
        pending: true,
    })
}

/// `path`'s folder, the working directory for a bare file name.
fn parent_of(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

impl Staged {
    /// The temp file, which holds the new bytes until the rename.
    pub(crate) fn temp(&self) -> &Path {
        &self.temp
    }

    /// Syncs the temp file to the disk, the slow half of the write. A
    /// failure removes the temp file.
    pub(crate) fn sync(&mut self, ops: &mut dyn Ops) -> Result<(), WriteError> {
        let Some(mut file) = self.file.take() else {
            return Ok(());
        };
        let synced = ops.sync(&mut file);
        drop(file);
        synced.map_err(|source| {
            self.remove_temp();
            WriteError::Io {
                path: self.path.clone(),
                source,
            }
        })
    }

    /// One attempt, the `attempt`th from 1, to rename the temp file over
    /// the destination. A Windows sharing failure asks for another after a
    /// backoff, up to [`ATTEMPTS`]; the caller sleeps, so that it can let go
    /// of a lock first.
    pub(crate) fn rename(&mut self, ops: &mut dyn Ops, attempt: u32) -> Renamed {
        // A file never synced is closed before it is renamed.
        self.file = None;
        let Err(source) = ops.rename(&self.temp, &self.path) else {
            self.pending = false;
            return Renamed::Done;
        };
        let sharing = is_sharing_error(ops.windows(), source.raw_os_error());
        if sharing && attempt < ATTEMPTS {
            return Renamed::Retry(backoff(attempt));
        }
        self.remove_temp();
        Renamed::Failed(if sharing {
            WriteError::Busy {
                path: self.path.clone(),
                attempts: attempt,
                source,
            }
        } else {
            WriteError::Io {
                path: self.path.clone(),
                source,
            }
        })
    }

    /// Syncs the destination's folder once the rename is in.
    pub(crate) fn sync_folder(&self) {
        sync_dir(parent_of(&self.path));
    }

    /// Syncs, renames with the Windows retries and syncs the folder: the
    /// rest of [`write_atomic`] after [`stage`].
    pub(crate) fn commit(mut self, ops: &mut dyn Ops) -> Result<(), WriteError> {
        self.sync(ops)?;
        let mut attempt = 1;
        loop {
            match self.rename(ops, attempt) {
                Renamed::Done => break,
                Renamed::Retry(wait) => {
                    ops.sleep(wait);
                    attempt += 1;
                }
                Renamed::Failed(error) => return Err(error),
            }
        }
        self.sync_folder();
        Ok(())
    }

    fn remove_temp(&mut self) {
        self.file = None;
        if std::mem::take(&mut self.pending) {
            let _ = std::fs::remove_file(&self.temp);
        }
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        self.remove_temp();
    }
}

fn write_atomic_with(path: &Path, contents: &[u8], ops: &mut dyn Ops) -> Result<(), WriteError> {
    stage_with(path, contents, ops)?.commit(ops)
}

/// Windows' access denied (5), sharing violation (32) and lock violation
/// (33): another process has the file open, which passes.
fn is_sharing_error(windows: bool, raw: Option<i32>) -> bool {
    windows && matches!(raw, Some(5 | 32 | 33))
}

/// The sleep after failed attempt `attempt` (from 1): 20, 40, 80, 160 ms.
fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(20 << (attempt - 1))
}

/// Best effort: the rename has already swapped the file, and some file
/// systems refuse a folder sync.
fn sync_dir(dir: &Path) {
    #[cfg(unix)]
    if let Ok(folder) = File::open(dir) {
        let _ = folder.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

fn create_temp_file(parent: &Path, file_name: &str) -> std::io::Result<(PathBuf, File)> {
    // Exclusive creation handles stale files and concurrent saves without a
    // global nonce or truncating another writer's temporary file.
    for nonce in 0..1024 {
        let path = parent.join(format!(".{file_name}.{}.{nonce}.tmp", std::process::id()));
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "no unused temporary file name",
    ))
}

/// Renames a file the game could not read to `<name>.bad` beside it,
/// replacing an older backup, and returns the backup's path.
pub(crate) fn quarantine(path: &Path) -> std::io::Result<PathBuf> {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bad");
    let backup = path.with_file_name(name);
    std::fs::rename(path, &backup)?;
    Ok(backup)
}

#[cfg(test)]
#[path = "tests/persist.rs"]
mod tests;
