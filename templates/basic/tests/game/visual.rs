//! The title's pixels against a fixture, in the UI lab's shape. Gate:
//! `TUNGSTEN_VISUAL_REGRESSION=1` (needs a GPU and a display). The template
//! runs from its folder for 24 smoke frames, capturing frame 20, after the
//! title's slide, at 1280 × 720; smoke mode has no user folder, so the
//! title shows "Saving is off". The fixture is captured on the reference
//! machine, and the comparison's tolerance is 0.

use std::path::Path;
use std::process::Command;

use tungsten::render::compare_png;

#[test]
fn the_title_matches_its_fixture() {
    if std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_err() {
        eprintln!("skipped: set TUNGSTEN_VISUAL_REGRESSION=1 to compare the title's pixels");
        return;
    }
    let actual = std::env::temp_dir().join("tungsten-template-title.png");
    let _ = std::fs::remove_file(&actual);
    let status = Command::new(env!("CARGO_BIN_EXE_tungsten-template-basic"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env_remove("TUNGSTEN_CAPTURE_DIRECT")
        .env("TUNGSTEN_SMOKE_FRAMES", "24")
        .env("TUNGSTEN_CAPTURE_FRAME", "20")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", &actual)
        .status()
        .expect("run the template");
    assert!(status.success(), "the template exited with {status:?}");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/title.png");
    let report = compare_png(&fixture, &actual, 0).expect("compare with the fixture");
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "visual regression: {report:?}"
    );
}
