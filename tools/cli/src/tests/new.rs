use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_new_{}_{n}", std::process::id()));
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

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn text(files: &BTreeMap<String, Vec<u8>>, rel: &str) -> String {
    String::from_utf8(files[rel].clone()).unwrap()
}

#[test]
fn a_new_game_holds_every_template_file_under_its_own_names() {
    let files: BTreeMap<String, Vec<u8>> =
        render("my-game", "0.90.0").unwrap().into_iter().collect();
    for (rel, _) in embedded::TEMPLATE {
        assert!(files.contains_key(*rel), "{rel}");
    }
    assert!(files.contains_key(".gitignore") && files.contains_key("Cargo.lock"));
    assert_eq!(files.len(), embedded::TEMPLATE.len() + 1);
    for (rel, data) in &files {
        if let Ok(text) = std::str::from_utf8(data) {
            for token in [PACKAGE, CRATE, TITLE] {
                assert!(!text.contains(token), "{rel} keeps {token}");
            }
        }
    }
    let manifest = text(&files, "Cargo.toml");
    assert!(manifest.contains("name = \"my-game\""));
    assert_eq!(
        manifest
            .matches(r#"git = "https://github.com/JosephGibson/Tungsten", tag = "v0.90.0""#)
            .count(),
        2,
        "{manifest}"
    );
    assert!(!manifest.contains("path = "));
    assert!(manifest.contains("[profile.release]\n") && manifest.contains("debug = 1\n"));
    assert!(manifest.contains("[profile.dev.package.\"*\"]\n"));
    assert!(text(&files, "src/main.rs").contains("use my_game::game;"));
    assert!(text(&files, "tungsten.json").contains(r#""title": "my-game""#));
    assert!(text(&files, "tungsten.json").contains(r#""id": "my-game""#));
    assert!(text(&files, "tests/game/visual.rs").contains("CARGO_BIN_EXE_my-game"));
    assert_eq!(text(&files, ".gitignore"), "/target\n/dist\n");
    // Binary files are copied byte for byte.
    let png = std::fs::read(repo().join("templates/basic/assets/sprites/player.png")).unwrap();
    assert_eq!(files["assets/sprites/player.png"], png);
}

#[test]
fn the_lock_is_the_workspaces_with_its_template_entry_renamed() {
    let files: BTreeMap<String, Vec<u8>> =
        render("my-game", "0.90.0").unwrap().into_iter().collect();
    let workspace = std::fs::read_to_string(repo().join("Cargo.lock")).unwrap();
    assert_eq!(
        workspace
            .matches("name = \"tungsten-template-basic\"\n")
            .count(),
        1
    );
    assert_eq!(
        text(&files, "Cargo.lock"),
        workspace.replace(
            "name = \"tungsten-template-basic\"\n",
            "name = \"my-game\"\n"
        )
    );
}

#[test]
fn an_existing_folder_is_refused_with_nothing_written() {
    let dir = Folder::new();
    let game = dir.0.join("my-game");
    std::fs::create_dir(&game).unwrap();
    std::fs::write(game.join("keep.txt"), "mine").unwrap();
    let err = run(&game).unwrap_err();
    assert!(err.to_string().contains("exists"), "{err}");
    let left: Vec<_> = std::fs::read_dir(&game).unwrap().collect();
    assert_eq!(left.len(), 1);
    let err = run(&dir.0.join("no/such/parent/game")).unwrap_err();
    assert!(err.to_string().contains("not a folder"), "{err}");
}

#[test]
fn names_follow_the_rule() {
    let long = "a".repeat(65);
    for refused in [
        "My-game",
        "1game",
        "my.game",
        long.as_str(),
        "con",
        "fn",
        "test",
        "tungsten",
        "serde",
        "",
        "-game",
        "my game",
    ] {
        assert!(check_name(refused).is_err(), "{refused:?} was accepted");
    }
    for accepted in ["my-game", "my_game", "g", &"a".repeat(64)] {
        assert_eq!(check_name(accepted), Ok(()), "{accepted}");
    }
}

#[test]
fn the_embedded_template_is_the_tracked_one() {
    let Ok(output) = std::process::Command::new("git")
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "templates/basic",
        ])
        .current_dir(repo())
        .output()
    else {
        eprintln!("skipped: no git");
        return;
    };
    if !output.status.success() {
        eprintln!("skipped: not a git checkout");
        return;
    }
    let tracked: Vec<String> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| line.trim_start_matches("templates/basic/").to_owned())
        .collect();
    let mut embedded: Vec<String> = embedded::TEMPLATE
        .iter()
        .map(|(rel, _)| (*rel).to_owned())
        .collect();
    let mut tracked = tracked;
    tracked.sort();
    embedded.sort();
    assert_eq!(embedded, tracked);
}
