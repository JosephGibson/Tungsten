//! `tungsten check` (`D-182`): the config stage here, with the engine's own
//! loaders, then the content stage in the game's own build, which
//! `App::run` runs in place of the window when `TUNGSTEN_CHECK` names a
//! report file.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use anyhow::Context;
use tungsten_core::{ActionMap, Config};

use crate::cargo;

/// One error: the file to fix, relative to the game's folder, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) file: String,
    pub(crate) message: String,
}

/// The engine's handshake: the report file a check run writes.
pub(crate) const CHECK_ENV: &str = "TUNGSTEN_CHECK";

/// The handshake's seeds for the room generators, `<first>+<count>`
/// (redo 7.12); unset, the engine sweeps `0+16`.
pub(crate) const SEEDS_ENV: &str = "TUNGSTEN_CHECK_SEEDS";

/// `tungsten level generate`'s handshake (redo 7.12): the report file a
/// generate run writes, and the generation it runs,
/// `<generator>:<seed>:<prefix>`.
pub(crate) const GENERATE_ENV: &str = "TUNGSTEN_GENERATE";
pub(crate) const GENERATE_RUN_ENV: &str = "TUNGSTEN_GENERATE_RUN";

/// The seeds `--seeds <count>` or `--seed <seed>` asks a check to sweep
/// each room generator over: `count` of them from `first`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Seeds {
    pub(crate) first: u64,
    pub(crate) count: u64,
}

impl std::fmt::Display for Seeds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}+{}", self.first, self.count)
    }
}

/// What a check run reported: its findings, and its notes (`#\t` lines),
/// one per room generator it swept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Report {
    pub(crate) findings: Vec<Finding>,
    pub(crate) notes: Vec<String>,
}

/// Runs `tungsten check` in `dir`, the game's folder, with the room
/// generators swept over `seeds` when given, printing the notes, every
/// error and a summary; `true` when there is no error.
pub(crate) fn run(dir: &Path, seeds: Option<Seeds>) -> anyhow::Result<bool> {
    require_game(dir)?;
    let config = config_stage(dir);
    if !config.is_empty() {
        print_findings(&config);
        println!("{}", config_summary(config.len()));
        return Ok(false);
    }
    let report = report_path();
    let status = content_stage(dir, &report, seeds, &mut Command::new("cargo"))?;
    let report = read_report(&report, status)?;
    for line in report_lines(&report, seeds) {
        println!("{line}");
    }
    println!("{}", summary(&report.findings));
    Ok(report.findings.is_empty())
}

/// What a check prints before its summary: [`note_lines`], then each
/// finding.
pub(crate) fn report_lines(report: &Report, seeds: Option<Seeds>) -> Vec<String> {
    let mut lines = note_lines(report, seeds);
    lines.extend(format_findings(&report.findings));
    lines
}

/// A report's notes; with `seeds` asked for and no note back, `no room
/// generator registered`, so a sweep that did not run never reads as a
/// clean one.
pub(crate) fn note_lines(report: &Report, seeds: Option<Seeds>) -> Vec<String> {
    if seeds.is_some() && report.notes.is_empty() {
        return vec!["no room generator registered".to_owned()];
    }
    report.notes.clone()
}

/// The game in `dir` that `check`, `run` and `describe` work on: its
/// folder, and its engine at this command line's version.
pub(crate) fn require_game(dir: &Path) -> anyhow::Result<()> {
    require_game_folder(dir)?;
    let metadata = cargo::metadata(dir)?;
    cargo::check_engine_version(&metadata, cargo::VERSION).map_err(anyhow::Error::msg)
}

/// A `Cargo.toml` in `dir`, or the error that says where to run.
pub(crate) fn require_game_folder(dir: &Path) -> anyhow::Result<()> {
    if dir.join("Cargo.toml").is_file() {
        Ok(())
    } else {
        anyhow::bail!("no Cargo.toml here: run this in the game's folder")
    }
}

/// `tungsten.json` and `input.json` through the engine's loaders, each at
/// most one error, its first; a missing file is none, as at startup.
pub(crate) fn config_stage(dir: &Path) -> Vec<Finding> {
    let mut findings = Vec::new();
    if let Err(e) = Config::load(dir.join("tungsten.json")) {
        findings.push(Finding {
            file: "tungsten.json".to_owned(),
            message: relative(&e.to_string(), dir),
        });
    }
    match ActionMap::load(&dir.join("input.json")) {
        Err(e) if !e.is_not_found() => findings.push(Finding {
            file: "input.json".to_owned(),
            message: relative(&e.to_string(), dir),
        }),
        _ => {}
    }
    findings
}

/// The summary after a failed config stage.
pub(crate) fn config_summary(errors: usize) -> String {
    format!(
        "tungsten check: {}; the game's assets are checked once tungsten.json and input.json load",
        count(errors, "error")
    )
}

/// `<temp>/tungsten-check-<pid>.txt`, removed if a run left it.
fn report_path() -> PathBuf {
    let path = std::env::temp_dir().join(format!("tungsten-check-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

/// Runs `cargo` (`command`, so `package` can point it at the staged binary
/// instead) in `dir` with the handshake set, its output the terminal's:
/// the report file, and `seeds` for the room generators when given, else
/// none, so the engine sweeps its default.
pub(crate) fn content_stage(
    dir: &Path,
    report: &Path,
    seeds: Option<Seeds>,
    command: &mut Command,
) -> anyhow::Result<ExitStatus> {
    match seeds {
        Some(seeds) => command.env(SEEDS_ENV, seeds.to_string()),
        None => command.env_remove(SEEDS_ENV),
    };
    prepare_game(dir, false, &[(CHECK_ENV, report)], command);
    // The game's stderr passes through but for its closing line, which
    // names the report: a temp file this command line reads and removes,
    // its findings printed after.
    let quiet = quiet_log();
    pass_stderr(command, |line| {
        !(names_the_report(line) || quiet && is_info_record(line))
    })
}

/// Whether the game's `INFO` records are dropped from its output: unless
/// the caller set `RUST_LOG`, which asks for a log and is the game's whole
/// filter. A run's records are its load's, about sixty lines that say
/// nothing went wrong; its warnings, errors and panics pass.
pub(crate) fn quiet_log() -> bool {
    std::env::var_os("RUST_LOG").is_none()
}

/// Whether `line` is one of the engine logger's `INFO` records,
/// `[<time> INFO  <target>] <message>`.
pub(crate) fn is_info_record(line: &str) -> bool {
    line.strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .is_some_and(|(head, _)| head.split_whitespace().nth(1) == Some("INFO"))
}

/// Runs `command` with its stderr passed through line by line, those
/// `keep` refuses dropped.
fn pass_stderr(command: &mut Command, keep: impl Fn(&str) -> bool) -> anyhow::Result<ExitStatus> {
    let mut child = command
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot run `{}`", command.get_program().to_string_lossy()))?;
    if let Some(stderr) = child.stderr.take() {
        use std::io::BufRead as _;
        for line in std::io::BufReader::new(stderr).lines() {
            let Ok(line) = line else { break };
            if keep(&line) {
                eprintln!("{line}");
            }
        }
    }
    child
        .wait()
        .with_context(|| format!("cannot run `{}`", command.get_program().to_string_lossy()))
}

/// Whether `line` is the game's closing line of a check that found errors,
/// `Error: the check found <n> error(s), written to '<report>'`.
pub(crate) fn names_the_report(line: &str) -> bool {
    line.strip_prefix("Error: the check found ")
        .is_some_and(|rest| rest.contains(", written to '"))
}

/// Sets `command` up to run the game in `dir` as [`run_game`] runs it: the
/// `cargo run` arguments for `cargo`, the variables and the folder.
fn prepare_game(dir: &Path, release: bool, envs: &[(&str, &Path)], command: &mut Command) {
    if command.get_program() == "cargo" {
        command.args(["run", "--quiet"]);
        if release {
            command.arg("--release");
        }
    }
    for (name, value) in envs {
        command.env(name, value);
    }
    command.current_dir(dir);
}

/// Runs the game in `dir` with the handshake's variables `envs` set: as
/// `cargo run --quiet` (with `--release` when asked) when `command` is
/// `cargo`, else the program given. Its output is the terminal's, less its
/// `INFO` log records unless `RUST_LOG` is set ([`quiet_log`]).
pub(crate) fn run_game(
    dir: &Path,
    release: bool,
    envs: &[(&str, &Path)],
    command: &mut Command,
) -> anyhow::Result<ExitStatus> {
    prepare_game(dir, release, envs, command);
    if quiet_log() {
        return pass_stderr(command, |line| !is_info_record(line));
    }
    command
        .status()
        .with_context(|| format!("cannot run `{}`", command.get_program().to_string_lossy()))
}

/// The report a check run wrote, removed once read; an error when the run
/// wrote none, which means the game stopped before `App::run`, or when it
/// failed with no finding.
pub(crate) fn read_report(report: &Path, status: ExitStatus) -> anyhow::Result<Report> {
    let Ok(text) = std::fs::read_to_string(report) else {
        anyhow::bail!(
            "the content stage did not run: the game stopped ({status}) before App::run checked its files; its output above says why"
        );
    };
    let _ = std::fs::remove_file(report);
    let report = parse_report(&text);
    if report.findings.is_empty() && !status.success() {
        anyhow::bail!("the game's check run failed ({status}) and reported no error");
    }
    Ok(report)
}

/// A report's `<file>\t<message>` lines, and its `#\t<note>` lines apart.
pub(crate) fn parse_report(text: &str) -> Report {
    let mut report = Report::default();
    for line in text.lines().filter(|line| !line.is_empty()) {
        if let Some(note) = line.strip_prefix("#\t") {
            report.notes.push(note.to_owned());
            continue;
        }
        report.findings.push(match line.split_once('\t') {
            Some((file, message)) => Finding {
                file: file.to_owned(),
                message: message.to_owned(),
            },
            None => Finding {
                file: String::new(),
                message: line.to_owned(),
            },
        });
    }
    report
}

/// Each finding as `<file>: <message>`, sorted by file, then in the order
/// found.
pub(crate) fn format_findings(findings: &[Finding]) -> Vec<String> {
    let mut sorted: Vec<&Finding> = findings.iter().collect();
    sorted.sort_by(|a, b| a.file.cmp(&b.file));
    sorted
        .into_iter()
        .map(|finding| format!("{}: {}", finding.file, finding.message))
        .collect()
}

fn print_findings(findings: &[Finding]) {
    for line in format_findings(findings) {
        println!("{line}");
    }
}

/// `tungsten check: no errors`, or the count of errors and their files.
pub(crate) fn summary(findings: &[Finding]) -> String {
    if findings.is_empty() {
        return "tungsten check: no errors".to_owned();
    }
    let files: BTreeSet<&str> = findings.iter().map(|f| f.file.as_str()).collect();
    format!(
        "tungsten check: {} in {}",
        count(findings.len(), "error"),
        count(files.len(), "file")
    )
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// `text` with the absolute folder `dir`'s prefix, canonical or as given,
/// taken off its paths.
pub(crate) fn relative(text: &str, dir: &Path) -> String {
    let mut text = text.to_owned();
    let canonical = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    for dir in [canonical.as_path(), dir] {
        if dir.is_absolute() && dir.parent().is_some() {
            let prefix = format!("{}{}", dir.display(), std::path::MAIN_SEPARATOR);
            text = text.replace(&prefix, "");
        }
    }
    text
}

#[cfg(test)]
#[path = "tests/check.rs"]
mod tests;
