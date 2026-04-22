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
    let padded_bpr = 16; // 12 bytes real, 4 bytes padding
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
