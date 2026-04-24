//! D-047 per-pixel RGBA diff; non-perceptual by design.

use std::path::Path;

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffReport {
    pub width: u32,
    pub height: u32,
    pub max_delta: u8,
    pub mean_delta: f32,
    pub pixels_above_tolerance: u32,
}

#[derive(Debug, Error)]
pub enum ImageDiffError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("decode error: {0}")]
    Decode(#[from] image::ImageError),
    #[error("dimension mismatch: lhs {lhs_w}x{lhs_h}, rhs {rhs_w}x{rhs_h}")]
    DimensionMismatch {
        lhs_w: u32,
        lhs_h: u32,
        rhs_w: u32,
        rhs_h: u32,
    },
}

pub fn compare_png(lhs: &Path, rhs: &Path, tolerance: u8) -> Result<DiffReport, ImageDiffError> {
    let lhs_img = image::open(lhs)?.to_rgba8();
    let rhs_img = image::open(rhs)?.to_rgba8();

    let (w, h) = (lhs_img.width(), lhs_img.height());
    if (w, h) != (rhs_img.width(), rhs_img.height()) {
        return Err(ImageDiffError::DimensionMismatch {
            lhs_w: w,
            lhs_h: h,
            rhs_w: rhs_img.width(),
            rhs_h: rhs_img.height(),
        });
    }

    let lhs_bytes = lhs_img.as_raw();
    let rhs_bytes = rhs_img.as_raw();
    debug_assert_eq!(lhs_bytes.len(), rhs_bytes.len());
    debug_assert_eq!(lhs_bytes.len() % 4, 0);

    let mut max_delta: u8 = 0;
    let mut sum_delta: u64 = 0;
    let mut pixels_above_tolerance: u32 = 0;
    let pixel_count = u64::from(w) * u64::from(h);

    for i in 0..pixel_count as usize {
        let base = i * 4;
        let d_r = lhs_bytes[base].abs_diff(rhs_bytes[base]);
        let d_g = lhs_bytes[base + 1].abs_diff(rhs_bytes[base + 1]);
        let d_b = lhs_bytes[base + 2].abs_diff(rhs_bytes[base + 2]);
        let d_a = lhs_bytes[base + 3].abs_diff(rhs_bytes[base + 3]);
        let worst = d_r.max(d_g).max(d_b).max(d_a);
        if worst > max_delta {
            max_delta = worst;
        }
        sum_delta += u64::from(worst);
        if worst > tolerance {
            pixels_above_tolerance += 1;
        }
    }

    let mean_delta = if pixel_count == 0 {
        0.0
    } else {
        sum_delta as f32 / pixel_count as f32
    };

    Ok(DiffReport {
        width: w,
        height: h,
        max_delta,
        mean_delta,
        pixels_above_tolerance,
    })
}

#[cfg(test)]
#[path = "tests/image_diff.rs"]
mod tests;
