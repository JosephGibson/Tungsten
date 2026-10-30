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

/// Runs `tungsten check` in `dir`, the game's folder, printing every error
/// and a summary; `true` when there is none.
pub(crate) fn run(dir: &Path) -> anyhow::Result<bool> {
    require_game(dir)?;
    let config = config_stage(dir);
    if !config.is_empty() {
        print_findings(&config);
        println!("{}", config_summary(config.len()));
        return Ok(false);
    }
    let report = report_path();
    let status = content_stage(dir, &report, &mut Command::new("cargo"))?;
    let findings = read_report(&report, status)?;
    print_findings(&findings);
    println!("{}", summary(&findings));
    Ok(findings.is_empty())
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
/// instead) in `dir` with the handshake set, its output the terminal's.
pub(crate) fn content_stage(
    dir: &Path,
    report: &Path,
    command: &mut Command,
) -> anyhow::Result<ExitStatus> {
    run_game(dir, false, &[(CHECK_ENV, report)], command)
}

/// Runs the game in `dir` with the handshake's variables `envs` set: as
/// `cargo run --quiet` (with `--release` when asked) when `command` is
/// `cargo`, else the program given. Its output is the terminal's unless
/// the caller set it.
pub(crate) fn run_game(
    dir: &Path,
    release: bool,
    envs: &[(&str, &Path)],
    command: &mut Command,
) -> anyhow::Result<ExitStatus> {
    if command.get_program() == "cargo" {
        command.args(["run", "--quiet"]);
        if release {
            command.arg("--release");
        }
    }
    for (name, value) in envs {
        command.env(name, value);
    }
    command
        .current_dir(dir)
        .status()
        .with_context(|| format!("cannot run `{}`", command.get_program().to_string_lossy()))
}

/// The findings in the report a check run wrote, the report removed; an
/// error when the run wrote none, which means the game stopped before
/// `App::run`, or when it failed with an empty report.
pub(crate) fn read_report(report: &Path, status: ExitStatus) -> anyhow::Result<Vec<Finding>> {
    let Ok(text) = std::fs::read_to_string(report) else {
        anyhow::bail!(
            "the content stage did not run: the game stopped ({status}) before App::run checked its files; its output above says why"
        );
    };
    let _ = std::fs::remove_file(report);
    let findings = parse_report(&text);
    if findings.is_empty() && !status.success() {
        anyhow::bail!("the game's check run failed ({status}) and reported no error");
    }
    Ok(findings)
}

/// A report's `<file>\t<message>` lines.
pub(crate) fn parse_report(text: &str) -> Vec<Finding> {
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| match line.split_once('\t') {
            Some((file, message)) => Finding {
                file: file.to_owned(),
                message: message.to_owned(),
            },
            None => Finding {
                file: String::new(),
                message: line.to_owned(),
            },
        })
        .collect()
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
