use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

use super::*;
use crate::cargo::{check_engine_version, install_line};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A temp folder of its own, removed on drop.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_cli_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_broken_config_and_a_broken_input_map_give_a_line_each() {
    let dir = Folder::new();
    std::fs::write(dir.0.join("tungsten.json"), r#"{"render": {"msaa": 3}}"#).unwrap();
    std::fs::write(dir.0.join("input.json"), "not json").unwrap();
    let findings = config_stage(&dir.0);
    assert_eq!(findings.len(), 2, "{findings:?}");
    assert_eq!(findings[0].file, "tungsten.json");
    assert!(
        findings[0].message.contains("msaa"),
        "{}",
        findings[0].message
    );
    assert_eq!(findings[1].file, "input.json");
    // Paths inside the messages are the game's own.
    let dir_text = dir.0.display().to_string();
    assert!(
        findings.iter().all(|f| !f.message.contains(&dir_text)),
        "{findings:?}"
    );
    assert_eq!(
        config_summary(2),
        "tungsten check: 2 errors; the game's assets are checked once tungsten.json and input.json load"
    );
}

#[test]
fn missing_config_files_are_no_error() {
    let dir = Folder::new();
    assert_eq!(config_stage(&dir.0), []);
    std::fs::write(dir.0.join("tungsten.json"), "{}").unwrap();
    std::fs::write(dir.0.join("input.json"), r#"{"actions": {}}"#).unwrap();
    assert_eq!(config_stage(&dir.0), []);
}

fn graph(versions: &[&str]) -> serde_json::Value {
    let mut packages: Vec<serde_json::Value> = versions
        .iter()
        .map(|version| json!({ "name": "tungsten", "version": version }))
        .collect();
    packages.push(json!({ "name": "tungsten-core", "version": "9.9.9" }));
    packages.push(json!({ "name": "my-game", "version": "0.1.0" }));
    json!({ "packages": packages })
}

#[test]
fn the_engine_must_be_this_command_lines_version() {
    assert_eq!(check_engine_version(&graph(&["0.90.0"]), "0.90.0"), Ok(()));
    let older = check_engine_version(&graph(&["0.89.0"]), "0.90.0").unwrap_err();
    assert!(
        older.contains("0.89.0") && older.contains("0.90.0"),
        "{older}"
    );
    assert!(older.ends_with(&install_line("0.89.0")), "{older}");
    assert_eq!(
        install_line("0.89.0"),
        "cargo install --locked --git https://github.com/JosephGibson/Tungsten --tag v0.89.0 tungsten-cli"
    );
    let none = check_engine_version(&graph(&[]), "0.90.0").unwrap_err();
    assert!(none.contains("does not depend on the engine"), "{none}");
    let two = check_engine_version(&graph(&["0.89.0", "0.90.0"]), "0.90.0").unwrap_err();
    assert!(two.contains("0.89.0 and 0.90.0"), "{two}");
}

#[test]
fn a_report_prints_sorted_by_file_with_the_count() {
    let findings = parse_report(
        "assets/prefabs/gem.json\tunknown component `x`\n\
         assets/manifest.json\tsprite 'a' references missing file: assets/a.png\n\
         assets/prefabs/gem.json\tsecond\n",
    );
    assert_eq!(
        format_findings(&findings),
        [
            "assets/manifest.json: sprite 'a' references missing file: assets/a.png",
            "assets/prefabs/gem.json: unknown component `x`",
            "assets/prefabs/gem.json: second",
        ]
    );
    assert_eq!(summary(&findings), "tungsten check: 3 errors in 2 files");
    assert_eq!(summary(&findings[..1]), "tungsten check: 1 error in 1 file");
    assert_eq!(summary(&[]), "tungsten check: no errors");
}

#[test]
fn no_report_means_the_content_stage_did_not_run() {
    let dir = Folder::new();
    let report = dir.0.join("report.txt");
    let status = std::process::Command::new("true").status().unwrap();
    let err = read_report(&report, status).unwrap_err();
    assert!(err.to_string().contains("did not run"), "{err}");
    std::fs::write(&report, "a.json\tbad\n").unwrap();
    let findings = read_report(&report, status).unwrap();
    assert_eq!(findings.len(), 1);
    assert!(!report.exists(), "the report is removed once read");
}
