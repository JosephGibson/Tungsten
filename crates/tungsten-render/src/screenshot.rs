//! Screenshot capture via offscreen render target plus padded readback buffer.
//!
//! Dev-tool path: the capture frame's present blit plus poll-wait. A capture
//! belongs to one frame, and its result comes back in that frame's
//! [`RenderOutcome`](crate::RenderOutcome) (`D-142`).

use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::renderer::Renderer;

/// Why a capture wrote no file.
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
    #[error("capture buffer size {0} does not match width*height*4 = {1}")]
    SizeMismatch(usize, usize),
}

/// One-shot PNG capture armed for the next frame.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PendingCapture {
    pub(crate) path: PathBuf,
    /// Keep the direct present path and read what it draws; see
    /// [`Renderer::capture_frame_direct`].
    pub(crate) direct: bool,
}

impl Renderer {
    /// Arm a one-shot PNG capture for the next `render_frame*` call only;
    /// arming again before it replaces the capture. That frame takes the
    /// blit path (`D-087`): every stage renders offscreen, the screenshot
    /// reads the blit's source and the blit presents it. Its
    /// [`RenderOutcome::Presented`](crate::RenderOutcome::Presented) carries
    /// the result; a skipped or failed frame drops the capture unwritten
    /// (`D-142`).
    pub fn capture_frame(&mut self, path: &Path) {
        self.pending_capture = Some(PendingCapture {
            path: path.to_path_buf(),
            direct: false,
        });
    }

    /// Arm a one-shot PNG capture of the direct present path, the one every
    /// frame without a capture takes, for the next `render_frame*` call only,
    /// as [`Renderer::capture_frame`] does. The frame renders into an
    /// offscreen stand-in for the swapchain, which the screenshot reads and a
    /// blit then presents. A test hook: it lets a pixel test check that both
    /// paths draw the same image.
    pub fn capture_frame_direct(&mut self, path: &Path) {
        self.pending_capture = Some(PendingCapture {
            path: path.to_path_buf(),
            direct: true,
        });
    }
}

/// Takes the capture armed for this frame out of `armed`, whatever the
/// frame's acquire gave, and hands it on only when the acquire gave a texture
/// to present into. A skipped (`Ok(None)`) or failed (`Err`) frame drops it
/// unwritten, so no later frame writes it (`D-142`). GPU-free, as
/// `surface_acquire` keeps the acquire policy.
pub(crate) fn capture_for_frame<T, E>(
    armed: &mut Option<PendingCapture>,
    acquired: &Result<Option<T>, E>,
) -> Option<PendingCapture> {
    let capture = armed.take();
    match acquired {
        Ok(Some(_)) => capture,
        Ok(None) | Err(_) => None,
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
