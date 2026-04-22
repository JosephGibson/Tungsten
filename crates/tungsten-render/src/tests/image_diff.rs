use super::*;
use image::{ImageBuffer, Rgba};
use std::path::PathBuf;

struct TempPng(PathBuf);
impl Drop for TempPng {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn write_png(name: &str, buf: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> TempPng {
    let path = std::env::temp_dir().join(format!(
        "tungsten-image-diff-test-{name}-{}.png",
        std::process::id()
    ));
    buf.save(&path).expect("write png");
    TempPng(path)
}

#[test]
fn identical_images_yield_zero_delta() {
    let mut img = ImageBuffer::new(4, 4);
    for pixel in img.pixels_mut() {
        *pixel = Rgba([10, 20, 30, 255]);
    }
    let lhs = write_png("ident_lhs", &img);
    let rhs = write_png("ident_rhs", &img);
    let report = compare_png(&lhs.0, &rhs.0, 0).expect("compare");
    assert_eq!(report.max_delta, 0);
    assert_eq!(report.mean_delta, 0.0);
    assert_eq!(report.pixels_above_tolerance, 0);
    assert_eq!(report.width, 4);
    assert_eq!(report.height, 4);
}

#[test]
fn single_flipped_channel_counts_one_pixel_above() {
    let mut a = ImageBuffer::new(2, 2);
    for pixel in a.pixels_mut() {
        *pixel = Rgba([0, 0, 0, 255]);
    }
    let mut b = a.clone();
    *b.get_pixel_mut(1, 1) = Rgba([255, 0, 0, 255]);

    let lhs = write_png("flip_lhs", &a);
    let rhs = write_png("flip_rhs", &b);
    let report = compare_png(&lhs.0, &rhs.0, 2).expect("compare");
    assert_eq!(report.max_delta, 255);
    assert_eq!(report.pixels_above_tolerance, 1);
}

#[test]
fn mismatched_dimensions_return_error() {
    let a = ImageBuffer::from_pixel(2, 2, Rgba([0u8, 0, 0, 255]));
    let b = ImageBuffer::from_pixel(3, 2, Rgba([0u8, 0, 0, 255]));
    let lhs = write_png("dim_lhs", &a);
    let rhs = write_png("dim_rhs", &b);
    let err = compare_png(&lhs.0, &rhs.0, 0).unwrap_err();
    assert!(matches!(err, ImageDiffError::DimensionMismatch { .. }));
}
