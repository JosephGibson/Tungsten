//! Screenshot capture (M21): renders the next frame additionally into an
//! offscreen `RENDER_ATTACHMENT | COPY_SRC` texture, reads it back through a
//! row-padded `MAP_READ` buffer, and encodes the result as PNG via
//! `image::save_buffer`.
//!
//! The swapchain surface is never `COPY_SRC`: the offscreen target is the
//! canonical readback source. Arming `capture_frame` incurs one extra render
//! pass and one device poll-wait on the next frame; it is a dev-tool path
//! and not safe for production frame-critical flows.

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::renderer::Renderer;

#[derive(Debug, Error)]
pub enum ScreenshotError {
    #[error("io error writing screenshot {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("image encode error: {0}")]
    Encode(#[from] image::ImageError),
    #[error("buffer map failed")]
    MapFailed,
    #[error("device lost while capturing screenshot")]
    DeviceLost,
    #[error("capture buffer size {0} does not match width*height*4 = {1}")]
    SizeMismatch(usize, usize),
}

impl Renderer {
    /// Arm a screenshot capture for the next call to `render_frame_full` /
    /// `render_frame_full_timed`. The path is captured-by-copy; the next
    /// frame writes a PNG there and then disarms the capture.
    pub fn capture_frame(&mut self, path: &Path) -> Result<(), ScreenshotError> {
        self.pending_capture = Some(path.to_path_buf());
        Ok(())
    }

    /// Whether a capture is currently armed.
    pub fn capture_armed(&self) -> bool {
        self.pending_capture.is_some()
    }
}

/// Row-padded readback helper. `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT` requires
/// each row of a `copy_texture_to_buffer` destination buffer to be aligned
/// to 256 bytes. `strip_row_padding` removes the tail padding after mapping.
pub(crate) fn aligned_bytes_per_row(width: u32) -> u32 {
    let unpadded = width * 4;
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    unpadded.div_ceil(alignment) * alignment
}

pub(crate) fn strip_row_padding(
    padded: &[u8],
    width: u32,
    height: u32,
    padded_bytes_per_row: u32,
) -> Vec<u8> {
    let unpadded_bpr = (width * 4) as usize;
    let padded_bpr = padded_bytes_per_row as usize;
    let mut out = Vec::with_capacity(unpadded_bpr * height as usize);
    for row in 0..height as usize {
        let start = row * padded_bpr;
        out.extend_from_slice(&padded[start..start + unpadded_bpr]);
    }
    out
}

#[cfg(test)]
#[path = "tests/screenshot.rs"]
mod tests;
