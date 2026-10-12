//! Opt-in visual regression tests on the `gpu` benchmark's `visual` preset.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (D-002, needs GPU/display).
//! Capture: 8 smoke frames, frame 5 at 1280x720.
//!
//! - `gpu_visual_matches_fixture` compares that frame with the fixture PNG,
//!   tolerance 2.
//! - `direct_and_capture_paths_draw_the_same_image` compares the two present
//!   paths of `D-087` with each other, tolerance 0.
//! - `capture_past_the_last_frame_fails_the_run` and
//!   `unwritable_capture_path_fails_the_run` check that a capture that writes
//!   no file fails the run (`D-142`).

use std::path::{Path, PathBuf};
use std::process::Command;

use tungsten::render::compare_png;

/// The `visual` preset with `sets` on top, run for 8 smoke frames with a
/// capture of `frame` into `path`. `direct` captures what the direct present
/// path draws; otherwise the frame takes the capture (blit) path.
fn visual_command(frame: u32, path: &Path, sets: Option<&str>, direct: bool) -> Command {
    // The benchmark loads its manifests relative to the workspace root.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new(env!("CARGO_BIN_EXE_example-02-bench"));
    command
        .current_dir(&root)
        .env("TUNGSTEN_BENCH", "gpu")
        .env("TUNGSTEN_BENCH_PRESET", "visual")
        .env_remove("TUNGSTEN_BENCH_SCALE")
        .env_remove("TUNGSTEN_BENCH_SET")
        .env_remove("TUNGSTEN_BENCH_DESCRIBE")
        .env_remove("TUNGSTEN_CAPTURE_DIRECT")
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", frame.to_string())
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", path);
    if let Some(sets) = sets {
        command.env("TUNGSTEN_BENCH_SET", sets);
    }
    if direct {
        command.env("TUNGSTEN_CAPTURE_DIRECT", "1");
    }
    command
}

/// A temp-folder path named after `name`, with no file left at it.
fn capture_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("tungsten-visual-regression-{name}.png"));
    if path.exists() {
        let _ = std::fs::remove_file(&path);
    }
    path
}

/// Runs the `visual` preset with `sets` on top and captures frame 5 into a
/// file named after `name`; see [`visual_command`] for `direct`.
fn capture(name: &str, sets: Option<&str>, direct: bool) -> PathBuf {
    let actual = capture_path(name);
    let status = visual_command(5, &actual, sets, direct)
        .status()
        .expect("run example-02-bench");
    assert!(status.success(), "example-02-bench exited with {status:?}");
    assert!(
        actual.exists(),
        "capture did not produce {}",
        actual.display()
    );
    actual
}

#[test]
fn gpu_visual_matches_fixture() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let actual = capture("gpu-actual", None, false);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("gpu-visual.png");
    let report = compare_png(&fixture, &actual, 2).expect("compare fixture");
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "visual regression: {report:?}"
    );
}

/// A frame without a capture renders its last full-screen stage into the
/// swapchain; a capture frame renders offscreen and blits (`D-087`). Both
/// must draw the same pixels, whichever stage ends the frame.
#[test]
fn direct_and_capture_paths_draw_the_same_image() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    // What writes the swapchain on the direct path, and the knobs that make
    // it the last stage. The preset itself is bloom, vignette and no AA.
    let cases = [
        ("scene", "post=none,aa=off"),
        ("msaa-resolve", "post=none,aa=msaa4"),
        ("last-post-pass", "aa=off"),
        ("smaa-after-post", "aa=smaa_high"),
        ("smaa-after-scene", "post=none,aa=smaa_high"),
    ];
    for (name, sets) in cases {
        let blit = capture(&format!("{name}-blit"), Some(sets), false);
        let direct = capture(&format!("{name}-direct"), Some(sets), true);
        let report = compare_png(&blit, &direct, 0).expect("compare the two paths");
        assert_eq!(
            report.pixels_above_tolerance, 0,
            "{name} ({sets}): the direct path differs from the capture path: {report:?}"
        );
    }
}

/// Runs `command`, whose capture must fail the run: a non-zero exit, no file
/// at `path`, and an error naming each of `expected` on stderr.
fn assert_capture_fails_the_run(mut command: Command, path: &Path, expected: &[&str]) {
    let output = command.output().expect("run example-02-bench");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "example-02-bench exited 0 with a failed capture:\n{stderr}"
    );
    assert!(
        !path.exists(),
        "the failed capture wrote {}",
        path.display()
    );
    for text in expected {
        assert!(
            stderr.contains(text),
            "stderr does not name `{text}`:\n{stderr}"
        );
    }
}

/// Eight smoke frames never reach frame 9, so the run fails (`D-142`).
#[test]
fn capture_past_the_last_frame_fails_the_run() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let path = capture_path("past-the-last-frame");
    let command = visual_command(9, &path, None, false);
    assert_capture_fails_the_run(
        command,
        &path,
        &["capture frame 9", "the run ended after 8 frames"],
    );
}

/// Frame 5 renders, but its folder is a regular file, so the write fails and
/// so does the run (`D-142`).
#[test]
fn unwritable_capture_path_fails_the_run() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let blocker = std::env::temp_dir().join("tungsten-visual-regression-not-a-folder");
    std::fs::write(&blocker, b"a file where the capture's folder would be")
        .expect("write the blocking file");
    let path = blocker.join("capture.png");
    let command = visual_command(5, &path, None, false);
    assert_capture_fails_the_run(
        command,
        &path,
        &["capture frame 5", "io error writing screenshot"],
    );
}
