//! Opt-in visual regression fixture for `02_sprite_stress`.
//!
//! Gated on `TUNGSTEN_VISUAL_REGRESSION=1` so it does not run under
//! `cargo test --workspace` (per `D-002`, CI is not a release gate and this
//! check needs a real GPU + display). When the env var is unset the test
//! returns early and is reported as passing.
//!
//! Procedure:
//!   1. Shell out to the example binary with `TUNGSTEN_SMOKE_FRAMES=8`,
//!      `TUNGSTEN_CAPTURE_FRAME=5`, `TUNGSTEN_CAPTURE_RESOLUTION=1280x720`,
//!      `TUNGSTEN_CAPTURE_PATH=<tempfile>`.
//!   2. Compare the produced PNG to the committed baseline via
//!      `tungsten_render::compare_png` with `tolerance = 2`.
//!   3. Assert `pixels_above_tolerance == 0`.
//!
//! Baseline regeneration: see `tests/fixtures/README.md`.

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
