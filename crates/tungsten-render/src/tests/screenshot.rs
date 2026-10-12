use super::*;

#[test]
fn aligned_row_rounds_up_to_256() {
    assert_eq!(aligned_bytes_per_row(64), 256);
    assert_eq!(aligned_bytes_per_row(65), 512);
    assert_eq!(aligned_bytes_per_row(1280), 5120);
}

#[test]
fn strip_row_padding_removes_tail() {
    let width = 3;
    let height = 2;
    let padded_bpr = 16;
    let mut padded = Vec::new();
    for row in 0..height {
        for px in 0..width {
            let base = (row * 10 + px) as u8;
            padded.extend_from_slice(&[base, base + 1, base + 2, 255]);
        }
        padded.extend_from_slice(&[0, 0, 0, 0]);
    }
    let stripped = strip_row_padding(&padded, width, height, padded_bpr);
    assert_eq!(stripped.len(), (width * height * 4) as usize);
    assert_eq!(stripped[0..4], [0, 1, 2, 255]);
    assert_eq!(stripped[12..16], [10, 11, 12, 255]);
}

fn pending(path: &str) -> PendingCapture {
    PendingCapture {
        path: PathBuf::from(path),
        direct: false,
    }
}

/// A presented frame: the acquire gave a texture.
const PRESENTED: Result<Option<()>, ()> = Ok(Some(()));
/// A skipped frame: no texture (timeout, occluded, still outdated).
const SKIPPED: Result<Option<()>, ()> = Ok(None);
/// A failed frame: the acquire returned an error.
const FAILED: Result<Option<()>, ()> = Err(());

#[test]
fn a_presented_frame_takes_its_armed_capture() {
    let mut slot = Some(pending("frame.png"));
    assert_eq!(
        capture_for_frame(&mut slot, &PRESENTED),
        Some(pending("frame.png"))
    );
    assert_eq!(slot, None);
}

#[test]
fn a_capture_that_meets_a_skipped_frame_is_gone_unwritten() {
    let mut slot = Some(pending("frame.png"));
    assert_eq!(capture_for_frame(&mut slot, &SKIPPED), None);
    assert_eq!(slot, None);
    // The next presented frame gets none: no later frame writes it.
    assert_eq!(capture_for_frame(&mut slot, &PRESENTED), None);
}

#[test]
fn a_capture_that_meets_a_failed_frame_is_gone_unwritten() {
    let mut slot = Some(pending("frame.png"));
    assert_eq!(capture_for_frame(&mut slot, &FAILED), None);
    assert_eq!(slot, None);
    assert_eq!(capture_for_frame(&mut slot, &PRESENTED), None);
}

#[test]
fn a_frame_with_nothing_armed_gets_no_capture() {
    let mut slot = None;
    assert_eq!(capture_for_frame(&mut slot, &PRESENTED), None);
}
