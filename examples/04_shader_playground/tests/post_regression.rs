//! Opt-in GPU regressions for the stock post stack, on the playground's
//! fixtures.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (needs GPU/display). Each test compares
//! two captures of this machine with each other, so there is no fixture image.
//! Capture: 8 smoke frames, frame 5 at 1280x720.

use std::path::{Path, PathBuf};

use tungsten::render::compare_png;

/// Runs the playground in `root` with `post_stack` preloaded and captures
/// frame 5 into a file named after `name`.
fn capture(name: &str, post_stack: &str, root: &Path) -> PathBuf {
    let actual = std::env::temp_dir().join(format!("tungsten-post-regression-{name}.png"));
    if actual.exists() {
        let _ = std::fs::remove_file(&actual);
    }

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_example-04-shader-playground"))
        .current_dir(root)
        .env("TUNGSTEN_POST_STACK_FIXTURE", post_stack)
        .env("TUNGSTEN_POST_AA_FIXTURE", "off")
        .env_remove("TUNGSTEN_BLOOM_FIXTURE")
        .env_remove("TUNGSTEN_GAME_FEEL_FIXTURE")
        .env_remove("TUNGSTEN_CAPTURE_DIRECT")
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", "5")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", &actual)
        .status()
        .expect("run example-04-shader-playground");
    assert!(
        status.success(),
        "example-04-shader-playground exited with {status:?}"
    );
    assert!(
        actual.exists(),
        "capture did not produce {}",
        actual.display()
    );
    actual
}

/// The playground loads its manifests relative to the workspace root.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Pixels of `lhs` and `rhs` further apart than 8, and the frame's pixel count.
fn pixels_apart(lhs: &Path, rhs: &Path) -> (u32, u32) {
    let report = compare_png(lhs, rhs, 8).expect("compare the two captures");
    (report.pixels_above_tolerance, report.width * report.height)
}

/// Two passes of one stock effect each draw with their own parameters
/// (`D-090`). A red half fade under a blue one must not look like two blue
/// ones, which is what a params buffer shared by the variant drew.
#[test]
fn repeated_effect_keeps_its_own_params() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let pair = capture("fade-pair", "fade_pair", &root);
    let twice = capture("fade-twice", "fade_twice", &root);
    let (apart, total) = pixels_apart(&pair, &twice);
    assert!(
        apart * 2 > total,
        "the first fade's color is lost: {apart} of {total} pixels differ"
    );
}
