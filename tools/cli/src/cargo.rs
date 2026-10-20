//! The game's Cargo graph: `cargo metadata` in the game's folder, the engine
//! version it builds, and the gate that holds it to this command line's
//! (`D-182`).

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, bail};
use serde_json::Value;

/// This command line's version, the workspace's.
pub(crate) const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The repository `new` pins and `cargo install` names.
pub(crate) const REPOSITORY: &str = "https://github.com/JosephGibson/Tungsten";

/// `cargo metadata --format-version 1` in `dir`, parsed.
pub(crate) fn metadata(dir: &Path) -> anyhow::Result<Value> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .context("cannot run `cargo metadata`: is cargo installed and on PATH?")?;
    if !output.status.success() {
        bail!(
            "`cargo metadata` failed ({}) in the game's folder",
            output.status
        );
    }
    serde_json::from_slice(&output.stdout).context("`cargo metadata` printed no JSON")
}

/// Holds the game to this command line's engine: exactly one package named
/// `tungsten` in the graph, at `cli_version`. The error names both versions
/// and the `cargo install` line for the game's.
pub(crate) fn check_engine_version(metadata: &Value, cli_version: &str) -> Result<(), String> {
    let mut versions: Vec<&str> = packages(metadata)
        .filter(|package| package["name"] == "tungsten")
        .filter_map(|package| package["version"].as_str())
        .collect();
    versions.sort_unstable();
    versions.dedup();
    match versions.as_slice() {
        [version] if *version == cli_version => Ok(()),
        [version] => Err(format!(
            "the game builds the engine at {version} and this command line is {cli_version}; \
             install the game's: {}",
            install_line(version)
        )),
        [] => Err(format!(
            "the game does not depend on the engine (no `tungsten` package in its Cargo graph); \
             this command line is {cli_version}"
        )),
        several => Err(format!(
            "the game's Cargo graph holds the engine at {}, and this command line is {cli_version}",
            several.join(" and ")
        )),
    }
}

/// `cargo install` for the command line of engine `version`.
pub(crate) fn install_line(version: &str) -> String {
    format!("cargo install --locked --git {REPOSITORY} --tag v{version} tungsten-cli")
}

/// Every package in the graph.
pub(crate) fn packages(metadata: &Value) -> impl Iterator<Item = &Value> {
    metadata["packages"].as_array().into_iter().flatten()
}
