//! Opt-in GPU regression for the M31 screen transitions (`D-093`), on the
//! example's `TUNGSTEN_TRANSITION_FIXTURE`.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (needs GPU/display). The test compares
//! captures of this machine with each other, so there is no fixture image.
//! Capture: 8 smoke frames, frame 5 at 1280x720. Smoke runs step 1/60 s on
//! every frame, so frame 5 is in `Out` at cover 0.83 and frame 6 is the fully
//! covered boundary frame.

use std::path::{Path, PathBuf};

use tungsten::render::compare_png;

/// Runs the example with `fixture` and captures frame 5 into a file named
/// after it.
fn capture(fixture: &str, root: &Path) -> PathBuf {
    let actual = std::env::temp_dir().join(format!("tungsten-transition-regression-{fixture}.png"));
    if actual.exists() {
        let _ = std::fs::remove_file(&actual);
    }

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_example-03-scene-state"))
        .current_dir(root)
        .env("TUNGSTEN_TRANSITION_FIXTURE", fixture)
        .env_remove("TUNGSTEN_CAPTURE_DIRECT")
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", "5")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", &actual)
        .status()
        .expect("run example-03-scene-state");
    assert!(
        status.success(),
        "example-03-scene-state exited with {status:?}"
    );
    assert!(
        actual.exists(),
        "capture did not produce {}",
        actual.display()
    );
    actual
}

/// The example loads its manifests relative to the workspace root.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Each effect's pass reaches the frame: mid-`Out`, the menu under a
/// transition must not look like the menu without one.
#[test]
fn each_transition_effect_changes_the_frame() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let root = workspace_root();
    let still = capture("none", &root);
    for effect in ["fade", "wipe_radial", "dissolve", "pixelate"] {
        let covered = capture(effect, &root);
        let report = compare_png(&still, &covered, 8).expect("compare the two captures");
        assert!(
            report.pixels_above_tolerance > 1_000,
            "{effect} did not change the frame: {} of {} pixels differ",
            report.pixels_above_tolerance,
            report.width * report.height
        );
    }
}
