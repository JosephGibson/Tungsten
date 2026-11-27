use super::*;

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

/// A fresh folder of its own under the system's temp folder.
fn tempdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tungsten_capture_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn two_captures_of_one_path_keep_the_written_file() {
    let dir = tempdir("one_path");
    let a = dir.join("a.png");
    std::fs::write(&a, b"png bytes").unwrap();
    let mut capture = Capture::empty();
    capture.add(5, a.clone());
    capture.add(5, a.clone());
    capture.frame_rendered(
        5,
        RenderOutcome::Presented {
            capture: Some(Ok(a.clone())),
        },
    );
    let outcomes = capture.take_outcomes();
    assert_eq!(
        std::fs::read(&a).unwrap(),
        b"png bytes",
        "two captures of one path on one frame: the written file keeps its bytes"
    );
    assert_eq!(outcomes.len(), 2, "{outcomes:#?}");
    assert!(
        outcomes.iter().all(|outcome| outcome.error.is_none()),
        "two captures of one path on one frame: both written, {outcomes:#?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_second_capture_of_a_frame_makes_its_folder() {
    let dir = tempdir("folder");
    let first = dir.join("a").join("first.png");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::write(&first, b"png bytes").unwrap();
    let second = dir.join("b").join("second.png");
    let mut capture = Capture::empty();
    capture.add(5, first.clone());
    capture.add(5, second.clone());
    capture.frame_rendered(
        5,
        RenderOutcome::Presented {
            capture: Some(Ok(first.clone())),
        },
    );
    let outcomes = capture.take_outcomes();
    assert!(
        outcomes.iter().all(|outcome| outcome.error.is_none()),
        "a second capture of a frame into a folder not made yet: {outcomes:#?}"
    );
    assert_eq!(
        std::fs::read(&second).ok().as_deref(),
        Some(&b"png bytes"[..]),
        "a second capture of a frame into a folder not made yet: the copy"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
