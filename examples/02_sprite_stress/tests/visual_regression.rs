//! Opt-in visual regression fixture.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (D-002, needs GPU/display).
//! Capture: 8 smoke frames, compare frame 5 at 1280x720, tolerance 2.

use std::path::Path;

use tungsten_render::compare_png;

#[test]
fn sprite_stress_matches_baseline() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        return;
    }

    let actual = std::env::temp_dir().join("tungsten-visual-regression-actual.png");
    if actual.exists() {
        let _ = std::fs::remove_file(&actual);
    }

    let status = std::process::Command::new(env!("CARGO_BIN_EXE_example-02-sprite-stress"))
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", "5")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", &actual)
        .status()
        .expect("run example-02-sprite-stress");
    assert!(status.success(), "sprite-stress exited with {status:?}");
    assert!(
        actual.exists(),
        "capture did not produce {}",
        actual.display()
    );

    let baseline = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("baseline-sprite-stress.png");
    let report = compare_png(&baseline, &actual, 2).expect("compare baseline");
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "visual regression: {report:?}"
    );
}
