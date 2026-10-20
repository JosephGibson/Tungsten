//! Embeds the files `tungsten package` writes into a game's notices (the
//! engine's `licenses/` folder, its root `LICENSE` and the licence files of
//! the works it compiles in) and what `tungsten new` writes (`templates/basic`
//! without `target/` and `dist/`, the workspace's `Cargo.lock` and its
//! `[profile.*]` tables), `D-182`. Standard library only.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The licence files of the works the engine compiles in, as
/// `scripts/release.py`'s `EMBEDDED_WORKS` names them.
const WORKS: [&str; 2] = [
    "crates/tungsten-render/src/assets/smaa/ATTRIBUTION.md",
    "crates/tungsten/assets/fonts/OFL.txt",
];

fn main() {
    let manifest_dir =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let repo = manifest_dir.join("../..");
    let mut files: Vec<String> = vec!["LICENSE".to_owned()];
    files.extend(WORKS.iter().map(|work| (*work).to_owned()));
    let licenses = repo.join("licenses");
    let mut found = Vec::new();
    walk(&licenses, &mut found);
    found.sort();
    for path in found {
        let rel = path.strip_prefix(&repo).expect("under the repository");
        files.push(slashes(rel));
    }
    println!("cargo:rerun-if-changed={}", licenses.display());
    let mut out = String::from(
        "/// Files the command line carries, by their path in the engine's repository.\n\
         pub(crate) static FILES: &[(&str, &[u8])] = &[\n",
    );
    for rel in &files {
        let path = repo.join(rel);
        assert!(path.is_file(), "{} is missing", path.display());
        println!("cargo:rerun-if-changed={}", path.display());
        let absolute = path.canonicalize().expect("an existing file");
        writeln!(
            out,
            "    ({rel:?}, include_bytes!({:?})),",
            absolute.display().to_string()
        )
        .expect("a String");
    }
    out.push_str("];\n");
    embed_template(&repo, &mut out);
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets it"));
    std::fs::write(out_dir.join("embedded.rs"), out).expect("OUT_DIR is writable");
}

/// The template's files, the lock and the profiles, for `tungsten new`.
fn embed_template(repo: &Path, out: &mut String) {
    let template = repo.join("templates/basic");
    println!("cargo:rerun-if-changed={}", template.display());
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&template).expect("the template folder") {
        let path = entry.expect("a folder entry").path();
        let name = path
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned();
        if path.is_dir() {
            if name != "target" && name != "dist" {
                walk(&path, &mut found);
            }
        } else {
            found.push(path);
        }
    }
    found.sort();
    out.push_str(
        "/// `templates/basic`'s files, by their path in the template.\n\
         pub(crate) static TEMPLATE: &[(&str, &[u8])] = &[\n",
    );
    for path in found {
        println!("cargo:rerun-if-changed={}", path.display());
        let rel = slashes(path.strip_prefix(&template).expect("under the template"));
        let absolute = path.canonicalize().expect("an existing file");
        writeln!(
            out,
            "    ({rel:?}, include_bytes!({:?})),",
            absolute.display().to_string()
        )
        .expect("a String");
    }
    out.push_str("];\n");
    let lock = repo.join("Cargo.lock");
    println!("cargo:rerun-if-changed={}", lock.display());
    writeln!(
        out,
        "/// The workspace's lock.\npub(crate) static LOCK: &[u8] = include_bytes!({:?});",
        lock.canonicalize()
            .expect("the workspace's lock")
            .display()
            .to_string()
    )
    .expect("a String");
    let manifest = repo.join("Cargo.toml");
    println!("cargo:rerun-if-changed={}", manifest.display());
    let text = std::fs::read_to_string(&manifest).expect("the workspace's Cargo.toml");
    writeln!(
        out,
        "/// The workspace's `[profile.*]` tables, comments left out.\npub(crate) static PROFILES: &str = {:?};",
        profiles(&text)
    )
    .expect("a String");
}

/// The `[profile.*]` tables of a Cargo.toml, their comment and blank lines
/// left out, a blank line between tables.
fn profiles(manifest: &str) -> String {
    let mut out = String::new();
    let mut in_profile = false;
    for line in manifest.lines() {
        if line.starts_with('[') {
            in_profile = line.starts_with("[profile.");
            if in_profile && !out.is_empty() {
                out.push('\n');
            }
        }
        if in_profile && !line.trim().is_empty() && !line.starts_with('#') {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Every file under `dir`, recursively.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("a readable folder") {
        let path = entry.expect("a folder entry").path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// `path` with `/` between its components.
fn slashes(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
