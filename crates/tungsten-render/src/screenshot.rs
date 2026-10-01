//! Screenshot capture via offscreen render target plus padded readback buffer.
//!
//! Dev-tool path: the capture frame's present blit plus poll-wait.

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

/// One-shot PNG capture armed for the next frame.
#[derive(Debug)]
pub(crate) struct PendingCapture {
    pub(crate) path: PathBuf,
    /// Keep the direct present path and read what it draws; see
    /// [`Renderer::capture_frame_direct`].
    pub(crate) direct: bool,
}

impl Renderer {
    /// Arm one-shot PNG capture for next full frame. That frame takes the
    /// blit path (`D-087`): every stage renders offscreen, the screenshot
    /// reads the blit's source and the blit presents it.
    pub fn capture_frame(&mut self, path: &Path) -> Result<(), ScreenshotError> {
        self.pending_capture = Some(PendingCapture {
            path: path.to_path_buf(),
            direct: false,
        });
        Ok(())
    }

    /// Arm a one-shot PNG capture of the direct present path, the one every
    /// frame without a capture takes. The frame renders into an offscreen
    /// stand-in for the swapchain, which the screenshot reads and a blit then
    /// presents. A test hook: it lets a pixel test check that both paths draw
    /// the same image.
    pub fn capture_frame_direct(&mut self, path: &Path) -> Result<(), ScreenshotError> {
        self.pending_capture = Some(PendingCapture {
            path: path.to_path_buf(),
            direct: true,
        });
        Ok(())
    }

    /// Capture armed.
    pub fn capture_armed(&self) -> bool {
        self.pending_capture.is_some()
    }
}

/// Row-padded readback stride.
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
