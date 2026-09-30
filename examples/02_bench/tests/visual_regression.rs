//! Opt-in visual regression fixture: the `gpu` benchmark's `visual` preset.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (D-002, needs GPU/display).
//! Capture: 8 smoke frames, compare frame 5 at 1280x720, tolerance 2.

use std::path::Path;

use tungsten_render::compare_png;

#[test]
fn gpu_visual_matches_fixture() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let actual = std::env::temp_dir().join("tungsten-visual-regression-gpu-actual.png");
    if actual.exists() {
        let _ = std::fs::remove_file(&actual);
    }

    // The benchmark loads its manifests relative to the workspace root.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_example-02-bench"))
        .current_dir(&root)
        .env("TUNGSTEN_BENCH", "gpu")
        .env("TUNGSTEN_BENCH_PRESET", "visual")
        .env_remove("TUNGSTEN_BENCH_SCALE")
        .env_remove("TUNGSTEN_BENCH_SET")
        .env_remove("TUNGSTEN_BENCH_DESCRIBE")
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", "5")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", &actual)
        .status()
        .expect("run example-02-bench");
    assert!(status.success(), "example-02-bench exited with {status:?}");
    assert!(
        actual.exists(),
        "capture did not produce {}",
        actual.display()
    );

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
