//! The one-frame PNG capture the environment asks for (`D-047`, `D-142`).
//! `TUNGSTEN_CAPTURE_FRAME=N` names the N-th frame that reaches render,
//! presented or skipped; `TUNGSTEN_CAPTURE_PATH` the file (`actual.png` by
//! default); `TUNGSTEN_CAPTURE_DIRECT=1` the direct present path (`D-087`).
//! A capture that writes no file is the run's error: `App::run` returns it
//! when no fatal error came first.

use std::path::PathBuf;

use tungsten_render::RenderOutcome;

/// The capture `TUNGSTEN_CAPTURE_*` asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CaptureConfig {
    /// The frame to capture, counting from 1 every frame that reaches
    /// render, presented or skipped.
    pub(crate) target_frame: u64,
    pub(crate) path: PathBuf,
    /// `TUNGSTEN_CAPTURE_DIRECT=1`: capture what the direct present path
    /// draws, not the blit path's source (`D-087`).
    pub(crate) direct: bool,
}

/// Parse one-shot capture env vars.
pub(crate) fn parse_capture_config() -> Option<CaptureConfig> {
    let target_frame: u64 = std::env::var("TUNGSTEN_CAPTURE_FRAME").ok()?.parse().ok()?;
    if target_frame == 0 {
        return None;
    }
    let path = std::env::var("TUNGSTEN_CAPTURE_PATH")
        .map_or_else(|_| PathBuf::from("actual.png"), PathBuf::from);
    let direct = std::env::var("TUNGSTEN_CAPTURE_DIRECT").is_ok_and(|value| value == "1");
    Some(CaptureConfig {
        target_frame,
        path,
        direct,
    })
}

/// One capture's course through a run: armed for its frame only, then
/// written or failed once.
#[derive(Debug)]
pub(crate) struct Capture {
    config: CaptureConfig,
    /// The target frame has rendered, or the run has ended: nothing more is
    /// armed or logged.
    settled: bool,
    /// Why no file was written, until the run takes it.
    error: Option<anyhow::Error>,
}

impl Capture {
    pub(crate) fn new(config: CaptureConfig) -> Self {
        Self {
            config,
            settled: false,
            error: None,
        }
    }

    /// What to arm for `frame`, the frame about to render (counting from 1):
    /// the capture, for its target frame only.
    pub(crate) fn armed_for(&self, frame: u64) -> Option<&CaptureConfig> {
        (!self.settled && frame == self.config.target_frame).then_some(&self.config)
    }

    /// Settles the capture with its target frame's outcome; another frame's
    /// changes nothing. A written file logs at info, any other outcome is a
    /// capture error, logged once.
    pub(crate) fn frame_rendered(&mut self, frame: u64, outcome: RenderOutcome) {
        if self.settled || frame != self.config.target_frame {
            return;
        }
        self.settled = true;
        let path = self.config.path.display();
        let error = match outcome {
            RenderOutcome::Presented {
                capture: Some(Ok(written)),
            } => {
                log::info!("captured frame {frame} -> {}", written.display());
                return;
            }
            RenderOutcome::Presented {
                capture: Some(Err(error)),
            } => {
                anyhow::Error::new(error).context(format!("capture frame {frame} -> {path} failed"))
            }
            RenderOutcome::Presented { capture: None } => anyhow::anyhow!(
                "capture frame {frame} -> {path} not written: the frame presented without it"
            ),
            // `Skipped`, and whatever a later renderer adds that presents nothing.
            _ => anyhow::anyhow!(
                "capture frame {frame} -> {path} not written: the frame was skipped (no surface texture)"
            ),
        };
        self.fail(error);
    }

    /// Ends the capture with the run, after `frames` frames reached render:
    /// a target frame that never came is a capture error. Returns the
    /// capture's error, once.
    pub(crate) fn finish(&mut self, frames: u64) -> Option<anyhow::Error> {
        if !self.settled {
            self.settled = true;
            let target = self.config.target_frame;
            self.fail(anyhow::anyhow!(
                "capture frame {target} -> {} not written: the run ended after {frames} frames",
                self.config.path.display()
            ));
        }
        self.error.take()
    }

    fn fail(&mut self, error: anyhow::Error) {
        log::error!("{error:#}");
        self.error = Some(error);
    }
}

#[cfg(test)]
#[path = "tests/capture.rs"]
mod tests;
