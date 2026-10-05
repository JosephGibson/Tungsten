//! The engine logger (`D-119`): one `env_logger` per process, installed by
//! `App::new` unless the game set its own, writing to stderr and to a log
//! file per run in the game's user folder.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use env_logger::{Builder, Target};
use log::LevelFilter;

/// The run's log file, which the logger's [`Tee`] and the crash hook share;
/// empty until the file is opened, and for good when it is not.
pub(crate) type LogFile = Arc<OnceLock<File>>;

/// Log files kept per executable stem.
const KEPT_LOGS: usize = 10;

/// Where records go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sinks {
    pub(crate) stderr: bool,
    pub(crate) file: bool,
}

impl Sinks {
    /// Stderr in debug builds, and in release builds only while `RUST_LOG`
    /// is set, so the perf runner still reads a release build's records;
    /// the file whenever the game has a user folder.
    pub(crate) const fn new(debug_build: bool, rust_log_set: bool, folder: bool) -> Self {
        Self {
            stderr: debug_build || rust_log_set,
            file: folder,
        }
    }
}

/// Installs the engine logger unless a logger is already set, the game's
/// or an earlier `App`'s, which then stays: no file is opened and none
/// deleted. `logs` is the user folder's logs directory. Returns the run's
/// log file, empty when none was opened.
pub(crate) fn install(level: LevelFilter, logs: Option<&Path>) -> LogFile {
    let rust_log = std::env::var("RUST_LOG").ok();
    let sinks = Sinks::new(cfg!(debug_assertions), rust_log.is_some(), logs.is_some());
    let file = LogFile::default();
    if !sinks.stderr && !sinks.file {
        return file;
    }
    let mut builder = builder(rust_log.as_deref(), level);
    if sinks.file {
        builder.target(Target::Pipe(Box::new(Tee {
            file: Arc::clone(&file),
            stderr: sinks.stderr,
        })));
    } else {
        builder
            .target(Target::Stderr)
            .is_test(cfg!(debug_assertions));
    }
    if builder.try_init().is_err() {
        return file;
    }
    if let Some(logs) = logs {
        open(
            &file,
            logs,
            &exe_stem(),
            SystemTime::now(),
            std::process::id(),
        );
    }
    file
}

/// `RUST_LOG`, when set, is the whole filter, parsed as `env_logger::init`
/// parses it; otherwise `logging.level` is.
pub(crate) fn builder(rust_log: Option<&str>, level: LevelFilter) -> Builder {
    let mut builder = Builder::new();
    match rust_log {
        Some(filters) => builder.parse_filters(filters),
        None => builder.filter_level(level),
    };
    builder
}

/// Creates `logs`, opens the run's file in it and prunes the stem's older
/// files. Failing costs one stderr line and no file, never an error.
pub(crate) fn open(file: &LogFile, logs: &Path, stem: &str, now: SystemTime, pid: u32) {
    let path = logs.join(log_file_name(stem, now, pid));
    let opened = std::fs::create_dir_all(logs)
        .and_then(|()| OpenOptions::new().create(true).append(true).open(&path));
    match opened {
        Ok(opened) => {
            let _ = file.set(opened);
            prune(logs, stem, KEPT_LOGS);
        }
        Err(err) => eprintln!("tungsten: no log file at '{}': {err}", path.display()),
    }
}

/// `<stem>-<YYYYMMDDTHHMMSS.mmmZ>-<pid>.log`.
pub(crate) fn log_file_name(stem: &str, now: SystemTime, pid: u32) -> String {
    format!("{stem}-{}-{pid}.log", utc_stamp(now))
}

/// Deletes all but the `keep` newest log files of `stem`, newest by name.
pub(crate) fn prune(logs: &Path, stem: &str, keep: usize) {
    let Ok(entries) = std::fs::read_dir(logs) else {
        return;
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| is_log_of(name, stem))
        .collect();
    names.sort_unstable();
    let excess = names.len().saturating_sub(keep);
    for name in &names[..excess] {
        let _ = std::fs::remove_file(logs.join(name));
    }
}

/// Exactly `<stem>-<stamp>-<pid>.log`: `game-helper-…` is not `game`'s, and
/// crash files never count.
fn is_log_of(name: &str, stem: &str) -> bool {
    let Some(rest) = name
        .strip_prefix(stem)
        .and_then(|rest| rest.strip_prefix('-'))
        .and_then(|rest| rest.strip_suffix(".log"))
    else {
        return false;
    };
    let Some((stamp, pid)) = rest.split_once('-') else {
        return false;
    };
    is_stamp(stamp) && !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit())
}

/// `YYYYMMDDTHHMMSS.mmmZ`.
fn is_stamp(stamp: &str) -> bool {
    let bytes = stamp.as_bytes();
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    bytes.len() == 20
        && bytes[8] == b'T'
        && bytes[15] == b'.'
        && bytes[19] == b'Z'
        && digits(0..8)
        && digits(9..15)
        && digits(16..19)
}

/// `time` in UTC as `YYYYMMDDTHHMMSS.mmmZ`; a time before 1970 reads as
/// 1970.
pub(crate) fn utc_stamp(time: SystemTime) -> String {
    let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs();
    let (year, month, day) = civil_from_days(secs / 86_400);
    let day_secs = secs % 86_400;
    format!(
        "{year:04}{month:02}{day:02}T{:02}{:02}{:02}.{:03}Z",
        day_secs / 3_600,
        day_secs / 60 % 60,
        day_secs % 60,
        since.subsec_millis()
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian (year, month, day): Howard
/// Hinnant's `civil_from_days`, for days on or after the epoch.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

/// The running executable's file stem, which names its log and crash files.
pub(crate) fn exe_stem() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "tungsten".to_string())
}

/// The one fan-out point for records (`D-119`). `env_logger` hands it each
/// whole formatted record in one `write_all`, under its lock, and it writes
/// the record to the file and, when enabled, to stderr. W1 M5's console
/// adds its bounded channel sink here, not a second logger.
pub(crate) struct Tee {
    pub(crate) file: LogFile,
    pub(crate) stderr: bool,
}

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(mut file) = self.file.get() {
            let _ = file.write_all(buf);
        }
        if self.stderr {
            let _ = io::stderr().write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/logging.rs"]
mod tests;
