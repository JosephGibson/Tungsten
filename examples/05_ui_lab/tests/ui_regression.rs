//! Opt-in visual regression tests of the UI overlay (M46, `D-147`), on the
//! example's `TUNGSTEN_UI_LAB_FIXTURE`.
//!
//! Gate: `TUNGSTEN_VISUAL_REGRESSION=1` (needs GPU/display). Capture: 8 smoke
//! frames, frame 5 at 1280x720.
//!
//! - `ui_visual_matches_fixture` compares the default lab with the fixture
//!   PNG at tolerance 0; the fixture is captured on the reference machine with
//!   the packaged fonts (`D-116`).
//! - `direct_and_capture_paths_draw_the_same_image_with_ui` compares the two
//!   present paths of `D-087` with the UI visible, at tolerance 0, with no AA,
//!   with SMAA and with a vignette.
//! - `hidden_ui_equals_an_empty_ui`: `engine_toggle_ui`'s state draws what an
//!   app with no roots draws.
//! - `the_ui_draws_over_the_scene`: the lab differs from the empty frame.

use std::path::{Path, PathBuf};
use std::process::Command;

use tungsten::render::compare_png;

/// The example loads its manifests relative to the workspace root.
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Runs the lab with `fixture` and the `post_aa` and `post` knobs for 8 smoke
/// frames with a capture of frame 5 into `path`; `direct` captures what the
/// direct present path draws, otherwise the frame takes the blit path.
fn lab_command(fixture: &str, post_aa: &str, post: &str, path: &Path, direct: bool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_example-05-ui-lab"));
    command
        .current_dir(workspace_root())
        .env("TUNGSTEN_UI_LAB_FIXTURE", fixture)
        .env("TUNGSTEN_UI_LAB_POST", post)
        .env("TUNGSTEN_RENDER_POST_AA", post_aa)
        .env_remove("TUNGSTEN_CAPTURE_DIRECT")
        .env("TUNGSTEN_SMOKE_FRAMES", "8")
        .env("TUNGSTEN_CAPTURE_FRAME", "5")
        .env("TUNGSTEN_CAPTURE_RESOLUTION", "1280x720")
        .env("TUNGSTEN_CAPTURE_PATH", path);
    if direct {
        command.env("TUNGSTEN_CAPTURE_DIRECT", "1");
    }
    command
}

/// Captures a run named `name`; see [`lab_command`].
fn capture(name: &str, fixture: &str, post_aa: &str, post: &str, direct: bool) -> PathBuf {
    let actual = std::env::temp_dir().join(format!("tungsten-ui-regression-{name}.png"));
    if actual.exists() {
        let _ = std::fs::remove_file(&actual);
    }
    let status = lab_command(fixture, post_aa, post, &actual, direct)
        .status()
        .expect("run example-05-ui-lab");
    assert!(status.success(), "example-05-ui-lab exited with {status:?}");
    assert!(
        actual.exists(),
        "capture did not produce {}",
        actual.display()
    );
    actual
}

fn gated() -> bool {
    std::env::var("TUNGSTEN_VISUAL_REGRESSION").is_ok()
}

#[test]
fn ui_visual_matches_fixture() {
    if !gated() {
        return;
    }
    let actual = capture("default", "default", "off", "off", false);
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("ui-visual.png");
    let report = compare_png(&fixture, &actual, 0).expect("compare fixture");
    assert_eq!(
        report.pixels_above_tolerance, 0,
        "visual regression: {report:?}"
    );
}

/// With the UI visible, the direct path and the capture path must draw the
/// same pixels whichever stage ends the frame (`D-087`, `D-147`).
#[test]
fn direct_and_capture_paths_draw_the_same_image_with_ui() {
    if !gated() {
        return;
    }
    let cases = [
        ("aa-off", "off", "off"),
        ("smaa-high", "smaa_high", "off"),
        ("vignette", "off", "vignette"),
    ];
    for (name, post_aa, post) in cases {
        let blit = capture(&format!("{name}-blit"), "default", post_aa, post, false);
        let direct = capture(&format!("{name}-direct"), "default", post_aa, post, true);
        let report = compare_png(&blit, &direct, 0).expect("compare the two paths");
        assert_eq!(
            report.pixels_above_tolerance, 0,
            "{name}: the direct path differs from the capture path: {report:?}"
        );
    }
}

#[test]
fn hidden_ui_equals_an_empty_ui() {
    if !gated() {
        return;
    }
    let empty = capture("empty", "empty", "off", "off", false);
    let hidden = capture("hidden", "hidden", "off", "off", false);
    assert_eq!(
        std::fs::read(&empty).expect("read the empty capture"),
        std::fs::read(&hidden).expect("read the hidden capture"),
        "hiding the game layers draws the empty frame, byte for byte"
    );
}

#[test]
fn the_ui_draws_over_the_scene() {
    if !gated() {
        return;
    }
    let empty = capture("empty-vs-default", "empty", "off", "off", false);
    let lab = capture("default-vs-empty", "default", "off", "off", false);
    let report = compare_png(&empty, &lab, 0).expect("compare the two captures");
    assert!(
        report.pixels_above_tolerance > 10_000,
        "the lab changed only {} of {} pixels",
        report.pixels_above_tolerance,
        report.width * report.height
    );
}
