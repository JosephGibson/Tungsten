use super::*;
use tungsten_render::ScreenshotError;

fn capture_at(target_frame: u64) -> Capture {
    Capture::new(CaptureConfig {
        target_frame,
        path: PathBuf::from("shots/frame.png"),
        direct: false,
    })
}

fn presented_without_capture() -> RenderOutcome {
    RenderOutcome::Presented { capture: None }
}

fn written() -> RenderOutcome {
    RenderOutcome::Presented {
        capture: Some(Ok(PathBuf::from("shots/frame.png"))),
    }
}

#[test]
fn only_the_target_frame_is_armed() {
    let capture = capture_at(2);
    assert!(capture.armed_for(1).is_none());
    let armed = capture.armed_for(2).expect("frame 2 is the target");
    assert_eq!(armed.path, PathBuf::from("shots/frame.png"));
    assert!(capture.armed_for(3).is_none());
}

#[test]
fn a_written_target_frame_leaves_no_error() {
    let mut capture = capture_at(2);
    capture.frame_rendered(1, presented_without_capture());
    capture.frame_rendered(2, written());
    capture.frame_rendered(3, presented_without_capture());
    assert!(capture.finish(3).is_none());
}

#[test]
fn a_write_error_is_the_capture_error_naming_its_frame() {
    let mut capture = capture_at(2);
    capture.frame_rendered(
        2,
        RenderOutcome::Presented {
            capture: Some(Err(ScreenshotError::MapFailed)),
        },
    );
    let error = capture.finish(4).expect("the failed write is an error");
    let message = format!("{error:#}");
    assert!(message.contains("capture frame 2"), "{message}");
    assert!(message.contains("buffer map failed"), "{message}");
}

#[test]
fn a_skipped_target_frame_fails_without_sliding_to_the_next_frame() {
    let mut capture = capture_at(2);
    capture.frame_rendered(2, RenderOutcome::Skipped);
    // Frame 3 presents, but it is not frame 2: nothing is armed for it.
    assert!(capture.armed_for(3).is_none());
    capture.frame_rendered(3, written());
    let error = capture.finish(3).expect("the skipped target is an error");
    let message = format!("{error:#}");
    assert!(message.contains("capture frame 2"), "{message}");
    assert!(message.contains("skipped"), "{message}");
}

#[test]
fn a_target_frame_the_run_never_reaches_is_an_error() {
    let mut capture = capture_at(9);
    for frame in 1..=8 {
        capture.frame_rendered(frame, presented_without_capture());
    }
    let error = capture.finish(8).expect("frame 9 never rendered");
    let message = format!("{error:#}");
    assert!(message.contains("capture frame 9"), "{message}");
    assert!(message.contains("ended after 8 frames"), "{message}");
}

#[test]
fn nothing_is_armed_after_the_target_frame() {
    let mut capture = capture_at(2);
    capture.frame_rendered(2, written());
    assert!(capture.armed_for(2).is_none());
    assert!(capture.armed_for(3).is_none());

    let mut capture = capture_at(2);
    capture.frame_rendered(2, RenderOutcome::Skipped);
    assert!(capture.armed_for(2).is_none());
    assert!(capture.armed_for(3).is_none());
}

#[test]
fn the_capture_error_is_returned_once() {
    let mut capture = capture_at(5);
    assert!(capture.finish(1).is_some());
    assert!(capture.finish(1).is_none());
}
