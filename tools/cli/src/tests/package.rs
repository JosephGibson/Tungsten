use std::sync::atomic::{AtomicU32, Ordering};

use serde_json::json;

use super::*;

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("tungsten_package_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.canonicalize().unwrap())
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn metadata(
    dir: &Path,
    targets: &serde_json::Value,
    default_run: Option<&str>,
) -> serde_json::Value {
    json!({
        "packages": [
            { "name": "tungsten", "version": "0.90.0", "manifest_path": "/engine/crates/tungsten/Cargo.toml", "targets": [] },
            {
                "name": "my-game", "version": "0.3.1",
                "manifest_path": dir.join("Cargo.toml").display().to_string(),
                "targets": targets,
                "default_run": default_run,
            }
        ],
        "target_directory": "/work/target",
    })
}

fn bin(name: &str) -> serde_json::Value {
    json!({ "name": name, "kind": ["bin"] })
}

#[test]
fn the_layout_names_the_stage_and_both_archives() {
    let dir = Folder::new();
    std::fs::write(dir.0.join("Cargo.toml"), "").unwrap();
    let meta = metadata(
        &dir.0,
        &json!([bin("my-game"), { "name": "my_game", "kind": ["lib"] }]),
        None,
    );
    let layout = layout(&meta, &dir.0, HOST).unwrap();
    assert_eq!(layout.bin, "my-game");
    assert_eq!(layout.top, "my-game-0.3.1-x86_64-unknown-linux-gnu");
    assert_eq!(
        layout.stage,
        Path::new("/work/target/tungsten-package/my-game-0.3.1-x86_64-unknown-linux-gnu")
    );
    assert_eq!(
        layout.debug_stage,
        Path::new("/work/target/tungsten-package/debug/my-game-0.3.1-x86_64-unknown-linux-gnu")
    );
    assert_eq!(
        layout.archive,
        dir.0
            .join("dist/my-game-0.3.1-x86_64-unknown-linux-gnu.tar.gz")
    );
    assert_eq!(
        layout.debug_archive,
        dir.0
            .join("dist/my-game-0.3.1-x86_64-unknown-linux-gnu-debug.tar.gz")
    );
}

#[test]
fn several_binaries_need_a_default_run() {
    let dir = Folder::new();
    std::fs::write(dir.0.join("Cargo.toml"), "").unwrap();
    let both = json!([bin("my-game"), bin("editor")]);
    let err = layout(&metadata(&dir.0, &both, None), &dir.0, HOST).unwrap_err();
    assert!(err.contains("default-run"), "{err}");
    let chosen = layout(&metadata(&dir.0, &both, Some("editor")), &dir.0, HOST).unwrap();
    assert_eq!(chosen.bin, "editor");
    let err = layout(&metadata(&dir.0, &json!([]), None), &dir.0, HOST).unwrap_err();
    assert!(err.contains("no binary"), "{err}");
}

#[test]
fn the_build_sets_exactly_the_portable_flags() {
    let command = build_command(Path::new("/game"), "my-game");
    let args: Vec<&std::ffi::OsStr> = command.get_args().collect();
    assert_eq!(
        args,
        [
            "build",
            "--release",
            "--bin",
            "my-game",
            "--message-format=json-render-diagnostics"
        ]
    );
    let envs: Vec<_> = command.get_envs().collect();
    assert_eq!(
        envs,
        [(
            std::ffi::OsStr::new("RUSTFLAGS"),
            Some(std::ffi::OsStr::new(
                "-C target-cpu=x86-64 -C link-arg=-Wl,--build-id"
            ))
        )]
    );
}

#[test]
fn a_set_rustflags_is_refused() {
    assert_eq!(refuse_flags(|_| None), Ok(()));
    for name in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"] {
        let err = refuse_flags(|asked| (asked == name).then(|| "-C target-cpu=native".into()))
            .unwrap_err();
        assert!(err.starts_with(&format!("{name} is set")), "{err}");
    }
}

#[test]
fn only_linux_x86_64_hosts_package() {
    assert_eq!(refuse_host("x86_64-unknown-linux-gnu"), Ok(()));
    for host in [
        "x86_64-pc-windows-msvc",
        "aarch64-apple-darwin",
        "aarch64-unknown-linux-gnu",
    ] {
        let err = refuse_host(host).unwrap_err();
        assert!(err.contains(host) && err.contains("backlog"), "{err}");
    }
    assert_eq!(
        host_of("rustc 1.98.1\nbinary: rustc\nhost: x86_64-unknown-linux-gnu\nrelease: 1.98.1\n")
            .as_deref(),
        Some("x86_64-unknown-linux-gnu")
    );
    assert_eq!(host_of("rustc 1.98.1\n"), None);
}

#[test]
fn missing_line_tables_tell_the_game_to_set_debug_1() {
    let message = split_failure(&SplitError::NoLineTables);
    assert!(
        message.contains("debug = 1") && message.contains("[profile.release]"),
        "{message}"
    );
    assert!(split_failure(&SplitError::NoBuildId).contains("build-id"));
}

#[test]
fn the_stage_holds_the_game_files_and_its_licenses() {
    let dir = Folder::new();
    for name in [
        "tungsten.json",
        "input.json",
        "LICENSE-MIT",
        "COPYING",
        "NOTICE.md",
        "README.md",
        "Cargo.toml",
        "build.rs",
    ] {
        std::fs::write(dir.0.join(name), "").unwrap();
    }
    for folder in ["assets", "src", "target", "dist", "licenses"] {
        std::fs::create_dir_all(dir.0.join(folder)).unwrap();
    }
    assert_eq!(
        stage_files(&dir.0).unwrap(),
        [
            "COPYING",
            "LICENSE-MIT",
            "NOTICE.md",
            "assets",
            "input.json",
            "tungsten.json"
        ]
    );
}

#[test]
fn the_executable_comes_from_the_bins_artifact_message() {
    let artifact = |name: &str, kind: &str, executable: serde_json::Value| {
        json!({
            "reason": "compiler-artifact",
            "target": { "name": name, "kind": [kind] },
            "executable": executable,
        })
        .to_string()
    };
    assert_eq!(
        artifact_executable(
            &artifact("my-game", "bin", json!("/t/release/my-game")),
            "my-game"
        ),
        Some(PathBuf::from("/t/release/my-game"))
    );
    assert_eq!(
        artifact_executable(&artifact("my_game", "lib", json!(null)), "my-game"),
        None
    );
    assert_eq!(
        artifact_executable(&artifact("editor", "bin", json!("/t/editor")), "my-game"),
        None
    );
    assert_eq!(artifact_executable("Compiling foo", "my-game"), None);
    assert_eq!(
        artifact_executable(r#"{"reason":"build-finished","success":true}"#, "my-game"),
        None
    );
}
