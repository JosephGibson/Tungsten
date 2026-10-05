//! Runs the probe binary in each mode, with `Command::env` and
//! `current_dir`, so no test sets a variable in its own process.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};

const PROBE: &str = env!("CARGO_BIN_EXE_tungsten-crash-probe");
const HOOK_MARKER: &str = "tungsten-crash-probe: previous hook ran";

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn tempdir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("tungsten_probe_{}_{n}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// The probe with its user folder at `user`, run in `cwd`, with the
/// engine's other variables cleared.
fn probe(user: &Path, cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(PROBE);
    command
        .args(args)
        .current_dir(cwd)
        .env("TUNGSTEN_USER_DIR", user)
        .env_remove("RUST_LOG")
        .env_remove("TUNGSTEN_SMOKE_FRAMES")
        .env_remove("TUNGSTEN_TEST_PANIC");
    command
}

fn run(command: &mut Command) -> (Output, String) {
    let output = command.output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output, stderr)
}

/// Files in `dir` whose names end with `suffix`, sorted; none when `dir`
/// does not exist.
fn files_in(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(suffix))
        .collect();
    files.sort();
    files
}

fn field<'a>(report: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}: ");
    report.lines().find_map(|line| line.strip_prefix(&prefix))
}

/// The lines after `header` up to the next section header.
fn section<'a>(report: &'a str, header: &str) -> Vec<&'a str> {
    report
        .lines()
        .skip_while(|line| *line != header)
        .skip(1)
        .take_while(|line| *line != "backtrace:" && *line != "modules:")
        .collect()
}

fn host_triple() -> String {
    let output = Command::new("rustc").arg("-vV").output().unwrap();
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .unwrap()
        .to_string()
}

#[test]
fn a_panic_leaves_one_crash_file_and_calls_the_previous_hook() {
    let dir = tempdir();
    let (output, stderr) = run(&mut probe(&dir, &dir, &[]));
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains(HOOK_MARKER), "{stderr}");

    let logs = dir.join("logs");
    let crashes = files_in(&logs, "-crash.txt");
    assert_eq!(crashes.len(), 1, "{crashes:?}");
    let report = fs::read_to_string(&crashes[0]).unwrap();
    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(field(&report, "game"), Some("tungsten-crash-probe"));
    assert_eq!(field(&report, "game_version"), Some(version));
    assert_eq!(field(&report, "engine_version"), Some(version));
    assert_eq!(field(&report, "target"), Some(host_triple().as_str()));
    let executable = field(&report, "executable").unwrap();
    assert_eq!(
        Path::new(executable).file_stem(),
        Path::new(PROBE).file_stem()
    );
    assert_eq!(field(&report, "thread"), Some("main"));
    assert_eq!(
        field(&report, "message"),
        Some("tungsten-crash-probe: deliberate panic")
    );
    let location = field(&report, "location").unwrap().replace('\\', "/");
    assert!(
        location.starts_with("tools/crash-probe/src/main.rs:"),
        "{location}"
    );
    let anchor = field(&report, "anchor").unwrap();
    assert!(
        anchor.starts_with("0x") && anchor.ends_with(" tungsten::crash::anchor"),
        "{anchor}"
    );
    if cfg!(any(target_os = "linux", windows)) {
        let build_id = field(&report, "build_id").unwrap();
        assert!(!build_id.is_empty() && build_id != "(none)", "{build_id}");
    }
    let backtrace = section(&report, "backtrace:");
    assert!(
        backtrace
            .iter()
            .any(|line| line.contains("tungsten_crash_probe")),
        "{report}"
    );
    if cfg!(target_os = "linux") {
        let probe = fs::canonicalize(PROBE).unwrap();
        let modules = section(&report, "modules:");
        assert!(
            modules
                .iter()
                .any(|line| line.ends_with(&*probe.to_string_lossy())),
            "{report}"
        );
    }

    // The run's log names the report.
    let log_files = files_in(&logs, ".log");
    assert_eq!(log_files.len(), 1, "{log_files:?}");
    let log = fs::read_to_string(&log_files[0]).unwrap();
    let name = crashes[0].file_name().unwrap().to_string_lossy();
    assert!(log.contains(&*name), "{log}");
}

#[test]
fn log_mode_writes_one_file_filtered_by_the_level_or_by_rust_log() {
    let dir = tempdir();
    let (output, stderr) = run(&mut probe(&dir, &dir, &["log", "info"]));
    assert!(output.status.success(), "{stderr}");
    let logs = files_in(&dir.join("logs"), ".log");
    assert_eq!(logs.len(), 1, "{logs:?}");
    let name = logs[0].file_name().unwrap().to_string_lossy();
    assert!(name.starts_with("tungsten-crash-probe-"), "{name}");
    let text = fs::read_to_string(&logs[0]).unwrap();
    assert!(text.contains("probe record at warn"), "{text}");
    assert!(text.contains("probe record at info"), "{text}");
    assert!(!text.contains("probe record at debug"), "{text}");
    assert!(!text.contains("probe record at trace"), "{text}");

    let dir = tempdir();
    let (output, stderr) = run(probe(&dir, &dir, &["log", "info"]).env("RUST_LOG", "warn"));
    assert!(output.status.success(), "{stderr}");
    let logs = files_in(&dir.join("logs"), ".log");
    assert_eq!(logs.len(), 1, "{logs:?}");
    let text = fs::read_to_string(&logs[0]).unwrap();
    let records: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("probe record"))
        .collect();
    assert_eq!(records.len(), 1, "{text}");
    assert!(records[0].contains("probe record at warn"), "{text}");
}

#[test]
fn twelve_runs_leave_ten_log_files() {
    let dir = tempdir();
    for _ in 0..12 {
        let (output, stderr) = run(&mut probe(&dir, &dir, &["log", "info"]));
        assert!(output.status.success(), "{stderr}");
    }
    assert_eq!(files_in(&dir.join("logs"), ".log").len(), 10);
}

#[test]
fn a_game_logger_set_first_stays_and_no_log_file_is_created() {
    let dir = tempdir();
    let (output, stderr) = run(&mut probe(&dir, &dir, &["own-logger"]));
    assert!(output.status.success(), "{stderr}");
    assert!(
        stderr.contains("probe-logger: own-logger marker"),
        "{stderr}"
    );
    assert!(files_in(&dir.join("logs"), ".log").is_empty());
}

#[test]
fn a_malformed_input_json_is_logged_before_app_new_returns_it() {
    let dir = tempdir();
    fs::write(dir.join("input.json"), "{").unwrap();
    let user = dir.join("user");
    let (output, stderr) = run(&mut probe(&user, &dir, &["bad-input"]));
    assert!(!output.status.success(), "{stderr}");
    let logs = files_in(&user.join("logs"), ".log");
    assert_eq!(logs.len(), 1, "{logs:?}");
    let text = fs::read_to_string(&logs[0]).unwrap();
    assert!(
        text.lines()
            .any(|line| line.contains("ERROR")
                && line.contains("invalid action map in 'input.json'")),
        "{text}"
    );
}

#[test]
fn an_invalid_game_id_fails_naming_it_and_creates_no_folder() {
    let dir = tempdir();
    let user = dir.join("user");
    let (output, stderr) = run(&mut probe(&user, &dir, &["bad-id"]));
    assert!(!output.status.success(), "{stderr}");
    assert!(stderr.contains("game.id"), "{stderr}");
    assert!(!user.exists());
}

#[test]
fn smoke_mode_leaves_the_folder_empty() {
    let dir = tempdir();
    let user = dir.join("user");
    fs::create_dir_all(&user).unwrap();
    let (output, stderr) =
        run(probe(&user, &dir, &["log", "info"]).env("TUNGSTEN_SMOKE_FRAMES", "1"));
    assert!(output.status.success(), "{stderr}");
    assert_eq!(fs::read_dir(&user).unwrap().count(), 0);
}
