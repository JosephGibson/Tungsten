//! `tungsten new <path>` (`D-182`): a game in a new folder from the
//! template embedded at this command line's build, named after the folder,
//! with the engine pinned to this command line's release tag, the engine
//! workspace's profiles and its lock.

use std::path::Path;

use anyhow::{Context, bail};
use tungsten_core::GameConfig;

use crate::cargo::{REPOSITORY, VERSION};
use crate::embedded;

/// The template's names, which become the game's.
const PACKAGE: &str = "tungsten-template-basic";
const CRATE: &str = "tungsten_template_basic";
const TITLE: &str = "Tungsten template";
/// The template's engine lines, which the pins replace.
const ENGINE_PATH: &str = r#"path = "../../crates/tungsten""#;
const MAX_NAME: usize = 64;
/// Names a game cannot take besides Rust's keywords: Cargo's target
/// folders and the packages its graph already holds.
const TAKEN: [&str; 14] = [
    "test",
    "build",
    "deps",
    "examples",
    "incremental",
    "anyhow",
    "log",
    "serde",
    "tungsten",
    "tungsten-core",
    "tungsten-render",
    "tungsten-kit",
    "tungsten-cli",
    "tungsten-template-basic",
];
const KEYWORDS: [&str; 53] = [
    "abstract",
    "as",
    "async",
    "await",
    "become",
    "box",
    "break",
    "const",
    "continue",
    "crate",
    "do",
    "dyn",
    "else",
    "enum",
    "extern",
    "false",
    "final",
    "fn",
    "for",
    "gen",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "macro",
    "match",
    "mod",
    "move",
    "mut",
    "override",
    "priv",
    "pub",
    "ref",
    "return",
    "self",
    "static",
    "struct",
    "super",
    "trait",
    "true",
    "try",
    "type",
    "typeof",
    "union",
    "unsafe",
    "unsized",
    "use",
    "virtual",
    "where",
    "while",
    "yield",
    "macro_rules",
];

/// Runs `tungsten new <path>`.
pub(crate) fn run(path: &Path) -> anyhow::Result<bool> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("the new game's path names no folder")?;
    check_name(name).map_err(anyhow::Error::msg)?;
    if path.exists() {
        bail!("{} exists; new never overwrites", path.display());
    }
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if !parent.is_dir() {
        bail!("{} is not a folder", parent.display());
    }
    let files = render(name, VERSION).map_err(anyhow::Error::msg)?;
    std::fs::create_dir(path).with_context(|| format!("cannot make {}", path.display()))?;
    for (rel, data) in files {
        let file = path.join(&rel);
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder)
                .with_context(|| format!("cannot make {}", folder.display()))?;
        }
        std::fs::write(&file, data).with_context(|| format!("cannot write {}", file.display()))?;
    }
    println!("tungsten new: made {name} in {}", path.display());
    println!("Next, in that folder:");
    println!("  cargo run        # the game, with hot reload");
    println!("  tungsten check   # every file it reads, every error");
    Ok(true)
}

/// The rule for a game's name: 1–64 lowercase ASCII letters, digits, `-`
/// and `_`, starting with a letter, a valid game ID, neither a Rust keyword
/// nor a name Cargo or the engine's graph holds.
pub(crate) fn check_name(name: &str) -> Result<(), String> {
    let refuse = |why: &str| Err(format!("`{name}` cannot name a game: {why}"));
    if name.is_empty() || name.len() > MAX_NAME {
        return refuse("a name is 1 to 64 characters");
    }
    if !name.starts_with(|c: char| c.is_ascii_lowercase()) {
        return refuse("a name starts with a lowercase letter");
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return refuse("a name holds lowercase letters, digits, `-` and `_` only");
    }
    let mut game = GameConfig::default();
    game.id = Some(name.to_owned());
    if let Err(e) = game.validate() {
        return refuse(&e.to_string());
    }
    if KEYWORDS.contains(&name.replace('-', "_").as_str()) {
        return refuse("it is a Rust keyword");
    }
    if TAKEN.contains(&name) {
        return refuse("Cargo or the engine already uses it");
    }
    Ok(())
}

/// The new game's files: the template's with its names made `name`'s, the
/// engine pinned to `v<version>` with the workspace's profiles, and the
/// workspace's lock, its template entry renamed.
pub(crate) fn render(name: &str, version: &str) -> Result<Vec<(String, Vec<u8>)>, String> {
    let crate_name = name.replace('-', "_");
    let mut files = Vec::new();
    for (rel, data) in embedded::TEMPLATE {
        let data = match std::str::from_utf8(data) {
            Ok(text) => {
                let text = text
                    .replace(PACKAGE, name)
                    .replace(CRATE, &crate_name)
                    .replace(TITLE, name);
                let text = if *rel == "Cargo.toml" {
                    pin(&text, version)?
                } else {
                    text
                };
                text.into_bytes()
            }
            Err(_) => data.to_vec(),
        };
        files.push(((*rel).to_owned(), data));
    }
    let lock = String::from_utf8_lossy(embedded::LOCK);
    let lock = lock.replace(
        &format!("name = \"{PACKAGE}\"\n"),
        &format!("name = \"{name}\"\n"),
    );
    files.push(("Cargo.lock".to_owned(), lock.into_bytes()));
    Ok(files)
}

/// The template's `Cargo.toml` with both engine lines on the release tag
/// and the workspace's profiles after it.
fn pin(manifest: &str, version: &str) -> Result<String, String> {
    let git = format!(r#"git = "{REPOSITORY}", tag = "v{version}""#);
    let pinned = manifest.replace(ENGINE_PATH, &git);
    if pinned.contains("path = ") {
        return Err("the embedded template's Cargo.toml keeps a path dependency".to_owned());
    }
    Ok(format!(
        "{}\n\n# The engine workspace's profiles: the build settings the engine is\n# tested with, and line tables in release builds, which `tungsten package`\n# splits into the debug archive.\n{}",
        pinned.trim_end(),
        embedded::PROFILES
    ))
}

#[cfg(test)]
#[path = "tests/new.rs"]
mod tests;
