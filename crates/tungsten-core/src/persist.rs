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

/// The operations [`write_atomic_with`] performs, injected by tests.
trait Ops {
    /// Whether the platform's sharing errors are retried.
    fn windows(&self) -> bool;
    /// Writes and syncs the temp file.
    fn write(&mut self, file: &mut File, contents: &[u8]) -> std::io::Result<()>;
    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()>;
    fn sleep(&mut self, duration: Duration);
}

struct RealOps;

impl Ops for RealOps {
    fn windows(&self) -> bool {
        cfg!(windows)
    }

    fn write(&mut self, file: &mut File, contents: &[u8]) -> std::io::Result<()> {
        file.write_all(contents)?;
        file.sync_all()
    }

    fn rename(&mut self, from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

fn write_atomic_with(path: &Path, contents: &[u8], ops: &mut impl Ops) -> Result<(), WriteError> {
    let io_error = |source| WriteError::Io {
        path: path.to_path_buf(),
        source,
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(io_error)?;

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let (temp_path, mut file) = create_temp_file(parent, file_name).map_err(io_error)?;
    let written = ops.write(&mut file, contents);
    drop(file);
    if let Err(source) = written {
        let _ = std::fs::remove_file(&temp_path);
        return Err(io_error(source));
    }

    let mut attempt = 1;
    while let Err(source) = ops.rename(&temp_path, path) {
        let sharing = is_sharing_error(ops.windows(), source.raw_os_error());
        if sharing && attempt < ATTEMPTS {
            ops.sleep(backoff(attempt));
            attempt += 1;
            continue;
        }
        let _ = std::fs::remove_file(&temp_path);
        return Err(if sharing {
            WriteError::Busy {
                path: path.to_path_buf(),
                attempts: attempt,
                source,
            }
        } else {
            io_error(source)
        });
    }

    sync_dir(parent);
    Ok(())
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
