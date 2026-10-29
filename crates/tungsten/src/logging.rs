//! The engine logger (`D-119`): one `env_logger` per process, installed by
//! `App::new` unless the game set its own, writing to stderr and to a log
//! file per run in the game's user folder. From M57 its [`Tee`] also copies
//! each record into the log console's bounded channel; from M58 the console
//! reads each record's fields with [`record_fields`] as it shows it.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use env_logger::{Builder, Target};
use log::{Level, LevelFilter};

/// The run's log file, which the logger's [`Tee`] and the crash hook share;
/// empty until the file is opened, and for good when it is not.
pub(crate) type LogFile = Arc<OnceLock<File>>;

/// Log files kept per executable stem.
const KEPT_LOGS: usize = 10;

/// Records the console's channel holds before it drops them (M57, Q8).
pub(crate) const CONSOLE_CHANNEL: usize = 1024;

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

/// How the [`Tee`] writes stderr (M57).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StderrMode {
    /// Not at all.
    Off,
    /// Through `eprint!`, which the test harness captures as `env_logger`'s
    /// `is_test` did: a debug build without a log file.
    Captured,
    /// With `write_all` on the stream.
    Direct,
}

impl StderrMode {
    /// The mode for `sinks` in a debug build or not.
    pub(crate) const fn new(debug_build: bool, sinks: Sinks) -> Self {
        if !sinks.stderr {
            Self::Off
        } else if debug_build && !sinks.file {
            Self::Captured
        } else {
            Self::Direct
        }
    }
}

/// The log console's end of the engine logger (M57): each record the
/// [`Tee`] sent, and how many it dropped on a full or closed channel; and
/// the records' tally by level, which a script's report reads (redo 7.2).
#[derive(Debug)]
pub(crate) struct ConsoleFeed {
    pub(crate) records: Receiver<String>,
    pub(crate) dropped: Arc<AtomicU64>,
    pub(crate) tally: Arc<LogTally>,
}

/// The [`Tee`]'s end of the console's channel.
#[derive(Debug)]
pub(crate) struct ConsoleSink {
    sender: SyncSender<String>,
    dropped: Arc<AtomicU64>,
    tally: Arc<LogTally>,
}

impl ConsoleSink {
    /// Copies one record, lossy UTF-8 with one trailing newline dropped,
    /// into the channel; a full or closed channel counts it dropped. The
    /// tally counts it first, whatever the channel does.
    pub(crate) fn send(&self, record: &[u8]) {
        let mut text = String::from_utf8_lossy(record).into_owned();
        if text.ends_with('\n') {
            text.pop();
        }
        self.tally.record(&text);
        if self.sender.try_send(text).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// A console channel of `capacity` records: the sink the [`Tee`] writes,
/// the feed the console drains.
pub(crate) fn console_channel(capacity: usize) -> (ConsoleSink, ConsoleFeed) {
    let (sender, records) = sync_channel(capacity);
    let dropped = Arc::new(AtomicU64::new(0));
    let tally = Arc::new(LogTally::default());
    (
        ConsoleSink {
            sender,
            dropped: Arc::clone(&dropped),
            tally: Arc::clone(&tally),
        },
        ConsoleFeed {
            records,
            dropped,
            tally,
        },
    )
}

/// How many error and warning records the logger wrote, and the first
/// error lines since the last [`mark`](Self::mark) (redo 7.2).
#[derive(Debug, Default)]
pub(crate) struct LogTally {
    errors: AtomicU32,
    warnings: AtomicU32,
    first_errors: Mutex<Vec<String>>,
}

/// The error lines a [`LogTally`] keeps after a mark.
pub(crate) const FIRST_ERRORS: usize = 5;

impl LogTally {
    /// Counts one record by its level; an error's first line is kept while
    /// fewer than [`FIRST_ERRORS`] are.
    fn record(&self, record: &str) {
        let Some(fields) = record_fields(record) else {
            return;
        };
        match fields.level {
            Level::Error => {
                self.errors.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut first) = self.first_errors.lock()
                    && first.len() < FIRST_ERRORS
                {
                    first.push(format!("{}: {}", fields.target, fields.message));
                }
            }
            Level::Warn => {
                self.warnings.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }

    /// The errors and warnings counted so far.
    pub(crate) fn counts(&self) -> (u32, u32) {
        (
            self.errors.load(Ordering::Relaxed),
            self.warnings.load(Ordering::Relaxed),
        )
    }

    /// Starts keeping first error lines afresh and returns the counts so
    /// far, from which a run counts its own.
    pub(crate) fn mark(&self) -> (u32, u32) {
        if let Ok(mut first) = self.first_errors.lock() {
            first.clear();
        }
        self.counts()
    }

    /// The error lines kept since the last mark.
    #[cfg_attr(not(any(test, feature = "testing")), allow(dead_code))]
    pub(crate) fn first_errors(&self) -> Vec<String> {
        self.first_errors
            .lock()
            .map(|first| first.clone())
            .unwrap_or_default()
    }
}

/// A record's fields (M58), as [`record_fields`] reads them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordFields<'a> {
    /// `HH:MM:SS` from the record's timestamp.
    pub(crate) time: &'a str,
    pub(crate) level: Level,
    /// The target's last `::` segment.
    pub(crate) target: &'a str,
    /// The message's first line.
    pub(crate) message: &'a str,
    /// Whether more lines follow it.
    pub(crate) more: bool,
}

/// The fields of one record in `env_logger`'s default format, `[<time>
/// <LEVEL> <target>] <message>`, as [`builder`]'s logger writes it (M58):
/// the time of day of its RFC 3339 stamp, the level, the target's last `::`
/// segment and the message's first line. `None` for text in another shape.
pub(crate) fn record_fields(record: &str) -> Option<RecordFields<'_>> {
    let (head, message) = record.strip_prefix('[')?.split_once(']')?;
    let message = message.strip_prefix(' ').unwrap_or(message);
    let mut tokens = head.split_whitespace();
    let stamp = tokens.next()?;
    let level: Level = tokens.next()?.parse().ok()?;
    let target = tokens.next().unwrap_or("");
    if tokens.next().is_some() {
        return None;
    }
    let time = stamp.split_once('T')?.1.get(..8)?;
    let digits =
        |range: std::ops::Range<usize>| time.as_bytes()[range].iter().all(u8::is_ascii_digit);
    if !(digits(0..2) && digits(3..5) && digits(6..8) && &time[2..3] == ":" && &time[5..6] == ":") {
        return None;
    }
    let target = target.rsplit("::").next().unwrap_or(target);
    let (message, more) = match message.split_once('\n') {
        Some((first, _)) => (first, true),
        None => (message, false),
    };
    Some(RecordFields {
        time,
        level,
        target,
        message,
        more,
    })
}

/// Installs the engine logger unless a logger is already set, the game's
/// or an earlier `App`'s, which then stays: no file is opened and none
/// deleted. `logs` is the user folder's logs directory. Returns the run's
/// log file, empty when none was opened, and the log console's feed, none
/// when this call installed no logger (M57).
pub(crate) fn install(level: LevelFilter, logs: Option<&Path>) -> (LogFile, Option<ConsoleFeed>) {
    let rust_log = std::env::var("RUST_LOG").ok();
    let sinks = Sinks::new(cfg!(debug_assertions), rust_log.is_some(), logs.is_some());
    let file = LogFile::default();
    if !sinks.stderr && !sinks.file {
        return (file, None);
    }
    let (console, feed) = console_channel(CONSOLE_CHANNEL);
    let mut builder = builder(rust_log.as_deref(), level);
    builder.target(Target::Pipe(Box::new(Tee {
        file: Arc::clone(&file),
        stderr: StderrMode::new(cfg!(debug_assertions), sinks),
        console: Some(console),
    })));
    if builder.try_init().is_err() {
        return (file, None);
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
    (file, Some(feed))
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

/// The one fan-out point for records (`D-119`), the logger's target
/// whenever stderr or the file is on. `env_logger` hands it each whole
/// formatted record in one `write_all`, under its lock, and it writes the
/// record to the file, to stderr as its mode says, and to the log
/// console's bounded channel (M57), not through a second logger.
pub(crate) struct Tee {
    pub(crate) file: LogFile,
    pub(crate) stderr: StderrMode,
    pub(crate) console: Option<ConsoleSink>,
}

impl Write for Tee {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Some(mut file) = self.file.get() {
            let _ = file.write_all(buf);
        }
        match self.stderr {
            StderrMode::Off => {}
            StderrMode::Captured => eprint!("{}", String::from_utf8_lossy(buf)),
            StderrMode::Direct => {
                let _ = io::stderr().write_all(buf);
            }
        }
        if let Some(console) = &self.console {
            console.send(buf);
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
