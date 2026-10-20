//! `tungsten package` (`D-182`): a portable x86-64 release build of the
//! game, staged with its files, split into a stripped binary and a debug
//! file as `D-120` splits the examples', checked as staged, with its
//! third-party notices, archived into `dist/` with GNU `tar`. Linux x86-64
//! hosts only for now.

use std::ffi::OsString;
use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, bail};
use serde_json::Value;

use crate::cargo;
use crate::check::{self, Finding};
use crate::elf::{self, SplitError};
use crate::notices::{self, NOTICES, STD_NOTICES};

/// The one host `package` builds on.
pub(crate) const HOST: &str = "x86_64-unknown-linux-gnu";
/// The build's flags: the portable baseline and a GNU build-id whatever the
/// linker's default (`D-072`, `D-120`). Set as `RUSTFLAGS`, they replace
/// every config file's flags.
pub(crate) const RUSTFLAGS: &str = "-C target-cpu=x86-64 -C link-arg=-Wl,--build-id";
/// Variables that would stand in for the flags `package` sets.
const FLAG_VARIABLES: [&str; 2] = ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"];

/// Where everything goes, from the game's `cargo metadata`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Layout {
    /// The game's package name and version.
    pub(crate) name: String,
    pub(crate) version: String,
    /// The binary the archive ships.
    pub(crate) bin: String,
    /// `<name>-<version>-<target>`, each archive's top folder.
    pub(crate) top: String,
    /// The players' stage, `<target-dir>/tungsten-package/<top>`.
    pub(crate) stage: PathBuf,
    /// The debug stage's top folder.
    pub(crate) debug_stage: PathBuf,
    /// `dist/<top>.tar.gz` and `dist/<top>-debug.tar.gz` in the game's
    /// folder.
    pub(crate) archive: PathBuf,
    pub(crate) debug_archive: PathBuf,
}

/// Runs `tungsten package` in `dir`, the game's folder.
pub(crate) fn run(dir: &Path) -> anyhow::Result<bool> {
    check::require_game_folder(dir)?;
    if !dir.join("tungsten.json").is_file() {
        bail!("no tungsten.json here: run this in the game's folder");
    }
    refuse_flags(|name| std::env::var_os(name)).map_err(anyhow::Error::msg)?;
    let host = host()?;
    refuse_host(&host).map_err(anyhow::Error::msg)?;
    let metadata = cargo::metadata(dir)?;
    cargo::check_engine_version(&metadata, cargo::VERSION).map_err(anyhow::Error::msg)?;
    let layout = layout(&metadata, dir, &host).map_err(anyhow::Error::msg)?;

    let config = check::config_stage(dir);
    if !config.is_empty() {
        print_findings(&config);
        println!("{}", check::config_summary(config.len()));
        return Ok(false);
    }

    println!(
        "tungsten package: building {} with RUSTFLAGS=\"{RUSTFLAGS}\"",
        layout.bin
    );
    let binary = build(dir, &layout.bin)?;

    stage(dir, &binary, &layout)?;
    let staged = layout.stage.join(&layout.bin);
    let debug = layout.debug_stage.join(format!("{}.debug", layout.bin));
    split(&staged, &debug)?;

    // What ships is checked as it ships: the stage is the game's folder,
    // so a file outside it fails here as it would fail the player.
    println!("tungsten package: checking the staged game");
    let report = std::env::temp_dir().join(format!("tungsten-package-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&report);
    let status = check::content_stage(&layout.stage, &report, &mut Command::new(&staged))?;
    let findings = check::read_report(&report, status)?;
    if !findings.is_empty() {
        print_findings(&findings);
        println!("{}", check::summary(&findings));
        println!("tungsten package: the staged game fails its check; nothing was archived");
        return Ok(false);
    }

    let collected = notices::collect(dir, &host, &layout.name)?;
    let mut input = collected.input;
    input.game.clone_from(&layout.name);
    input.game_version.clone_from(&layout.version);
    let text = notices::third_party_notices(&input).map_err(anyhow::Error::msg)?;
    write(&layout.stage.join(NOTICES), &text)?;
    write(&layout.stage.join(STD_NOTICES), &collected.std_notices)?;

    std::fs::create_dir_all(dir.join("dist")).context("cannot make dist/")?;
    archive(&layout.archive, &layout.stage)?;
    archive(&layout.debug_archive, &layout.debug_stage)?;
    println!("tungsten package: {}", layout.archive.display());
    println!("tungsten package: {}", layout.debug_archive.display());
    Ok(true)
}

/// Refuses a set `RUSTFLAGS` or `CARGO_ENCODED_RUSTFLAGS`, since the build
/// sets its own; `var` reads the environment.
pub(crate) fn refuse_flags(var: impl Fn(&str) -> Option<OsString>) -> Result<(), String> {
    match FLAG_VARIABLES.iter().find(|name| var(name).is_some()) {
        Some(name) => Err(format!(
            "{name} is set; unset it: package builds with RUSTFLAGS=\"{RUSTFLAGS}\" alone"
        )),
        None => Ok(()),
    }
}

/// Refuses every host but Linux x86-64.
pub(crate) fn refuse_host(host: &str) -> Result<(), String> {
    if host == HOST {
        Ok(())
    } else {
        Err(format!(
            "package builds on {HOST} hosts only, not {host}; packaging on Windows and macOS hosts is a backlog row (`tungsten package` on Windows and macOS hosts)"
        ))
    }
}

/// The `host` line of `rustc -vV`.
fn host() -> anyhow::Result<String> {
    let output = Command::new("rustc")
        .arg("-vV")
        .stdin(Stdio::null())
        .output()
        .context("cannot run `rustc`: is it installed and on PATH?")?;
    host_of(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| anyhow::anyhow!("`rustc -vV` printed no host line"))
}

/// The host triple in `rustc -vV`'s output.
pub(crate) fn host_of(verbose: &str) -> Option<String> {
    verbose
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(|host| host.trim().to_owned())
}

/// The game's package (the one whose `Cargo.toml` is in `dir`), the binary
/// it ships and where the stage and archives go.
pub(crate) fn layout(metadata: &Value, dir: &Path, host: &str) -> Result<Layout, String> {
    let manifest = dir.join("Cargo.toml");
    let manifest = manifest.canonicalize().unwrap_or(manifest);
    let package = cargo::packages(metadata)
        .find(|package| {
            package["manifest_path"]
                .as_str()
                .is_some_and(|path| Path::new(path) == manifest)
        })
        .ok_or("cargo metadata does not list the package in this folder")?;
    let name = package["name"].as_str().unwrap_or_default().to_owned();
    let version = package["version"].as_str().unwrap_or_default().to_owned();
    let bins: Vec<&str> = package["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|target| {
            target["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"))
        })
        .filter_map(|target| target["name"].as_str())
        .collect();
    let bin = match (bins.as_slice(), package["default_run"].as_str()) {
        ([one], _) => (*one).to_owned(),
        (_, Some(default)) if bins.contains(&default) => default.to_owned(),
        ([], _) => return Err(format!("{name} has no binary to package")),
        _ => {
            return Err(format!(
                "{name} has {} binaries and no default-run; set `default-run` in its Cargo.toml",
                bins.len()
            ));
        }
    };
    let target_dir = metadata["target_directory"]
        .as_str()
        .ok_or("cargo metadata names no target directory")?;
    let top = format!("{name}-{version}-{host}");
    let root = Path::new(target_dir).join("tungsten-package");
    Ok(Layout {
        stage: root.join(&top),
        debug_stage: root.join("debug").join(&top),
        archive: dir.join("dist").join(format!("{top}.tar.gz")),
        debug_archive: dir.join("dist").join(format!("{top}-debug.tar.gz")),
        name,
        version,
        bin,
        top,
    })
}

/// The release build's command: the binary alone, cargo's JSON on stdout,
/// `RUSTFLAGS` set.
pub(crate) fn build_command(dir: &Path, bin: &str) -> Command {
    let mut command = Command::new("cargo");
    command
        .args([
            "build",
            "--release",
            "--bin",
            bin,
            "--message-format=json-render-diagnostics",
        ])
        .env("RUSTFLAGS", RUSTFLAGS)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    command
}

/// Builds `bin` and returns the executable cargo reports.
fn build(dir: &Path, bin: &str) -> anyhow::Result<PathBuf> {
    let mut child = build_command(dir, bin)
        .spawn()
        .context("cannot run `cargo`: is it installed and on PATH?")?;
    let mut executable = None;
    let stdout = child.stdout.take().expect("stdout is piped");
    for line in std::io::BufReader::new(stdout).lines() {
        let line = line.context("cannot read cargo's output")?;
        if let Some(path) = artifact_executable(&line, bin) {
            executable = Some(path);
        }
    }
    let status = child.wait().context("cargo did not finish")?;
    if !status.success() {
        bail!("the release build failed ({status})");
    }
    executable.ok_or_else(|| anyhow::anyhow!("cargo reported no executable for `{bin}`"))
}

/// The executable of a `compiler-artifact` message for binary `bin`.
pub(crate) fn artifact_executable(line: &str, bin: &str) -> Option<PathBuf> {
    let message: Value = serde_json::from_str(line).ok()?;
    let is_bin = message["target"]["kind"]
        .as_array()
        .is_some_and(|kinds| kinds.iter().any(|kind| kind == "bin"));
    (message["reason"] == "compiler-artifact" && is_bin && message["target"]["name"] == bin)
        .then(|| message["executable"].as_str().map(PathBuf::from))
        .flatten()
}

/// The files of the game's folder a stage holds besides the binary:
/// `tungsten.json`, `input.json`, `assets/`, and its root `LICENSE*`,
/// `COPYING*` and `NOTICE*` files, sorted.
pub(crate) fn stage_files(dir: &Path) -> anyhow::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir).context("cannot list the game's folder")? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let upper = name.to_uppercase();
        let wanted = matches!(name.as_str(), "tungsten.json" | "input.json" | "assets")
            || (["LICENSE", "COPYING", "NOTICE"]
                .iter()
                .any(|prefix| upper.starts_with(prefix))
                && entry.path().is_file());
        if wanted {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// A fresh stage: the binary and [`stage_files`], links followed.
fn stage(dir: &Path, binary: &Path, layout: &Layout) -> anyhow::Result<()> {
    for folder in [&layout.stage, &layout.debug_stage] {
        if folder.exists() {
            std::fs::remove_dir_all(folder)
                .with_context(|| format!("cannot clear {}", folder.display()))?;
        }
        std::fs::create_dir_all(folder)
            .with_context(|| format!("cannot make {}", folder.display()))?;
    }
    copy(binary, &layout.stage.join(&layout.bin))?;
    for name in stage_files(dir)? {
        copy(&dir.join(&name), &layout.stage.join(&name))?;
    }
    Ok(())
}

/// Copies a file or a folder, following links, so the copy holds itself.
fn copy(from: &Path, to: &Path) -> anyhow::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to).with_context(|| format!("cannot make {}", to.display()))?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            copy(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to)
            .map(|_| ())
            .with_context(|| format!("cannot copy {} to {}", from.display(), to.display()))
    }
}

/// Moves the staged binary's debug information into `debug`, compressed,
/// and links it (`D-120`), then reads both back.
fn split(binary: &Path, debug: &Path) -> anyhow::Result<()> {
    objcopy(&[
        "--only-keep-debug".into(),
        "--compress-debug-sections=zlib".into(),
        binary.into(),
        debug.into(),
    ])?;
    let mut link = OsString::from("--add-gnu-debuglink=");
    link.push(debug);
    objcopy(&[
        "--strip-debug".into(),
        "--remove-section=.debug_gdb_scripts".into(),
        link,
        binary.into(),
    ])?;
    elf::check_split_files(binary, debug).map_err(|e| anyhow::anyhow!(split_failure(&e)))
}

/// What a failed split check tells the game.
pub(crate) fn split_failure(error: &SplitError) -> String {
    match error {
        SplitError::NoLineTables => "the release build has no line tables, so its debug file would be empty: set `debug = 1` under `[profile.release]` in Cargo.toml, as `tungsten new` writes it".to_owned(),
        other => format!("the debug split failed: {other}"),
    }
}

fn objcopy(args: &[OsString]) -> anyhow::Result<()> {
    let output = Command::new("objcopy")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("cannot run `objcopy`: install binutils")?;
    if !output.status.success() {
        bail!(
            "objcopy failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

/// `tar -czf archive -C <folder's parent> <folder's name>`, replacing it.
fn archive(archive: &Path, folder: &Path) -> anyhow::Result<()> {
    let _ = std::fs::remove_file(archive);
    let parent = folder.parent().context("a stage has a parent folder")?;
    let name = folder.file_name().context("a stage has a name")?;
    let output = Command::new("tar")
        .arg("-czf")
        .arg(archive)
        .arg("-C")
        .arg(parent)
        .arg(name)
        .stdin(Stdio::null())
        .output()
        .context("cannot run `tar`: install GNU tar and gzip")?;
    if !output.status.success() {
        bail!(
            "tar failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn write(path: &Path, text: &str) -> anyhow::Result<()> {
    std::fs::write(path, text).with_context(|| format!("cannot write {}", path.display()))
}

fn print_findings(findings: &[Finding]) {
    for line in check::format_findings(findings) {
        println!("{line}");
    }
}

#[cfg(test)]
#[path = "tests/package.rs"]
mod tests;
