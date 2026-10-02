//! Opt-in visual regression tests on the `gpu` benchmark's `visual` preset.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (D-002, needs GPU/display).
//! Capture: 8 smoke frames, frame 5 at 1280x720.
//!
//! - `gpu_visual_matches_fixture` compares that frame with the fixture PNG,
//!   tolerance 2.
//! - `direct_and_capture_paths_draw_the_same_image` compares the two present
//!   paths of `D-087` with each other, tolerance 0.

use std::path::{Path, PathBuf};

use tungsten_render::compare_png;

/// Runs the `visual` preset with `sets` on top and captures frame 5 into a
/// file named after `name`. `direct` captures what the direct present path
/// draws; otherwise the frame takes the capture (blit) path.
fn capture(name: &str, sets: Option<&str>, direct: bool) -> PathBuf {
    let actual = std::env::temp_dir().join(format!("tungsten-visual-regression-{name}.png"));
    if actual.exists() {
        let _ = std::fs::remove_file(&actual);
    }

    // The benchmark loads its manifests relative to the workspace root.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_example-02-bench"));
    command
        .current_dir(&root)
        .env("TUNGSTEN_BENCH", "gpu")
        .env("TUNGSTEN_BENCH_PRESET", "visual")
        .env_remove("TUNGSTEN_BENCH_SCALE")
        .env_remove("TUNGSTEN_BENCH_SET")
        .env_remove("TUNGSTEN_BENCH_DESCRIBE")
        .env_remove("TUNGSTEN_CAPTURE_DIRECT")
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", "5")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", &actual);
    if let Some(sets) = sets {
        command.env("TUNGSTEN_BENCH_SET", sets);
    }
    if direct {
        command.env("TUNGSTEN_CAPTURE_DIRECT", "1");
    }
    let status = command.status().expect("run example-02-bench");
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
