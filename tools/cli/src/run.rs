//! `tungsten run --script` (redo 7.2): the script parsed here with core's
//! parser, so a bad one stops before any build, then the game run in its
//! window in its own build with the script, a report file and a user folder
//! named by the handshake's variables, and the report's failures printed.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use tungsten_core::script::{AssertReport, Report, Script};

use crate::args::RunArgs;
use crate::check;

/// The script the game follows.
pub(crate) const SCRIPT_ENV: &str = "TUNGSTEN_SCRIPT";
/// The report the game writes.
pub(crate) const REPORT_ENV: &str = "TUNGSTEN_REPORT";
/// The game's user folder.
pub(crate) const USER_DIR_ENV: &str = "TUNGSTEN_USER_DIR";

/// Unreached steps listed before the rest are counted.
const UNREACHED_LISTED: usize = 10;

/// Runs `args`' script on the game in `dir`, printing each failure and a
/// summary; `true` when the script passed.
pub(crate) fn run(dir: &Path, args: &RunArgs) -> anyhow::Result<bool> {
    let script = absolute(dir, &args.script);
    load(&script)?;
    check::require_game(dir)?;
    let (report, temp_report) = match &args.report {
        Some(path) => (absolute(dir, path), false),
        None => (temp("tungsten-run", "json"), true),
    };
    let _ = std::fs::remove_file(&report);
    let (user_dir, temp_user) = match &args.user_dir {
        Some(path) => (absolute(dir, path), false),
        None => (temp("tungsten-run-user", ""), true),
    };
    if temp_user {
        let _ = std::fs::remove_dir_all(&user_dir);
    }
    std::fs::create_dir_all(&user_dir)
        .with_context(|| format!("cannot make the user folder '{}'", user_dir.display()))?;
    let status = check::run_game(
        dir,
        args.release,
        &[
            (SCRIPT_ENV, &script),
            (REPORT_ENV, &report),
            (USER_DIR_ENV, &user_dir),
        ],
        &mut Command::new("cargo"),
    );
    if temp_user {
        let _ = std::fs::remove_dir_all(&user_dir);
    }
    let status = status?;
    let Ok(text) = std::fs::read_to_string(&report) else {
        anyhow::bail!(
            "the game stopped ({status}) before writing its report; its output above says why"
        );
    };
    if temp_report {
        let _ = std::fs::remove_file(&report);
    }
    let parsed: Report = serde_json::from_str(&text)
        .with_context(|| format!("the report '{}' is not a script report", report.display()))?;
    for line in failures(&parsed) {
        println!("{line}");
    }
    if !temp_report {
        println!("report: {}", report.display());
    }
    println!("{}", summary(&parsed));
    Ok(parsed.passed)
}

/// The script at `path`, read and parsed; the error names the file and the
/// field that failed.
pub(crate) fn load(path: &Path) -> anyhow::Result<Script> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the script '{}'", path.display()))?;
    Script::parse(&text)
        .map_err(|err| anyhow::anyhow!("invalid script '{}': {err}", path.display()))
}

/// `path` against `dir` when relative: the game runs in `dir`.
fn absolute(dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        dir.join(path)
    }
}

/// `<temp>/<stem>-<pid>[.<extension>]`.
fn temp(stem: &str, extension: &str) -> PathBuf {
    let mut name = format!("{stem}-{}", std::process::id());
    if !extension.is_empty() {
        name.push('.');
        name.push_str(extension);
    }
    std::env::temp_dir().join(name)
}

/// One line per failure: the fatal error, each failed assert with its value
/// and the actual, each capture not written, each failed step, then the
/// unreached steps, the first ten listed.
pub(crate) fn failures(report: &Report) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(error) = &report.error {
        lines.push(format!("error: {error}"));
    }
    for assert in report.asserts.iter().filter(|a| !a.passed) {
        lines.push(assert_line(assert));
    }
    for capture in report.captures.iter().filter(|c| !c.written) {
        lines.push(format!(
            "frame {}: capture {} not written: {}",
            capture.frame,
            capture.path,
            capture.error.as_deref().unwrap_or("no reason given")
        ));
    }
    for step in &report.failed_steps {
        lines.push(format!(
            "frame {}: step {} ({}) failed: {}",
            step.frame, step.index, step.verb, step.error
        ));
    }
    for step in report.unreached.iter().take(UNREACHED_LISTED) {
        lines.push(format!(
            "frame {}: step {} ({}) unreached",
            step.frame, step.index, step.verb
        ));
    }
    if report.unreached.len() > UNREACHED_LISTED {
        lines.push(format!(
            "and {} more unreached",
            report.unreached.len() - UNREACHED_LISTED
        ));
    }
    lines
}

/// `frame <n>: assert <probe>[<path>] <op> <value> failed: actual <actual>`,
/// or with the reason it could not compare.
fn assert_line(assert: &AssertReport) -> String {
    let path = assert.path.as_deref().unwrap_or("");
    let head = format!(
        "frame {}: assert {}{path} {} {}",
        assert.frame, assert.probe, assert.op, assert.value
    );
    match &assert.error {
        Some(error) => format!("{head} failed: {error}"),
        None => format!("{head} failed: actual {}", assert.actual),
    }
}

/// `tungsten run: <frames> frames, <n> captures, <n> asserts: passed`, or
/// `failed`.
pub(crate) fn summary(report: &Report) -> String {
    format!(
        "tungsten run: {}, {}, {}: {}",
        count(report.frames as usize, "frame"),
        count(report.captures.len(), "capture"),
        count(report.asserts.len(), "assert"),
        if report.passed { "passed" } else { "failed" }
    )
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

#[cfg(test)]
#[path = "tests/run.rs"]
mod tests;
