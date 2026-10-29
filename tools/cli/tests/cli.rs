//! The `tungsten` binary end to end.

use std::process::Command;

fn tungsten() -> Command {
    Command::new(env!("CARGO_BIN_EXE_tungsten"))
}

#[test]
fn help_prints_the_usage_and_exits_0() {
    let output = tungsten().arg("--help").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.starts_with("Usage: tungsten <command>\n"),
        "{stdout}"
    );
    assert!(stdout.ends_with("this command line.\n"), "{stdout}");
    assert!(output.stderr.is_empty());
}

#[test]
fn version_prints_the_workspace_version() {
    let output = tungsten().arg("-V").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("tungsten {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn an_unknown_command_prints_the_usage_to_stderr_and_exits_2() {
    let output = tungsten().arg("frobnicate").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("tungsten: unknown command `frobnicate`\n\nUsage: tungsten <command>"),
        "{stderr}"
    );
}

#[test]
fn new_makes_a_game_once() {
    let root = std::env::temp_dir().join(format!("tungsten_cli_new_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let game = root.join("my-game");
    let output = tungsten().arg("new").arg(&game).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    for file in [
        "Cargo.toml",
        "Cargo.lock",
        "tungsten.json",
        "input.json",
        "assets/manifest.json",
        "src/main.rs",
        ".gitignore",
    ] {
        assert!(game.join(file).is_file(), "{file}");
    }
    let again = tungsten().arg("new").arg(&game).output().unwrap();
    assert_eq!(again.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&again.stderr).contains("exists"));
    let refused = tungsten()
        .arg("new")
        .arg(root.join("Bad"))
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(1));
    assert!(!root.join("Bad").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn run_without_its_script_and_a_misplaced_option_exit_2() {
    let output = tungsten().arg("run").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("tungsten: `run` needs --script <file>\n\nUsage:"),
        "{stderr}"
    );
    let output = tungsten().args(["check", "--json"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("tungsten: `--json` is an option of `describe`, not of `check`"),
        "{stderr}"
    );
}

#[test]
fn a_bad_script_exits_1_with_its_field_path_before_any_build() {
    let dir = std::env::temp_dir().join(format!("tungsten_cli_bad_script_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // A game folder, so only the script can stop the run before cargo.
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"never-built\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("bad.json"),
        r#"{ "version": 1, "frames": 5, "steps": [{ "frame": 1, "captur": "a.png" }] }"#,
    )
    .unwrap();
    let output = tungsten()
        .args(["run", "--script", "bad.json"])
        .current_dir(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("tungsten run: invalid script '"),
        "{stderr}"
    );
    assert!(stderr.contains("steps[0].captur"), "{stderr}");
    assert!(!dir.join("target").exists(), "nothing was built");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn render_without_its_output_exits_2() {
    let output = tungsten()
        .args(["render", "--sheet", "goblin"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with("tungsten: `render` needs -o <path>\n\nUsage:"),
        "{stderr}"
    );
}
