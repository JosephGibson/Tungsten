//! The PNG captures a run asks for (`D-047`, `D-142`): the environment's one
//! frame, or a script's (redo 7.2). `TUNGSTEN_CAPTURE_FRAME=N` names the
//! N-th frame that reaches render, presented or skipped;
//! `TUNGSTEN_CAPTURE_PATH` the file (`actual.png` by default);
//! `TUNGSTEN_CAPTURE_DIRECT=1` the direct present path (`D-087`).
//! A capture that writes no file is the run's error: `App::run` returns it
//! when no fatal error came first. Under a script the variables are
//! ignored and each `capture` step's result goes to the script's report.

use std::path::PathBuf;

use tungsten_render::RenderOutcome;

/// One capture: the one `TUNGSTEN_CAPTURE_*` asks for, or a script's.
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

/// The captures' course through a run: each armed for its frame only, then
/// written or failed once. `TUNGSTEN_CAPTURE_*` gives one; a script gives
/// one per `capture` step's frame (redo 7.2), several paths on one frame
/// written once and copied.
#[derive(Debug)]
pub(crate) struct Capture {
    /// The captures not yet settled, in frame order.
    targets: Vec<CaptureConfig>,
    /// The run has ended: nothing more is armed or logged.
    ended: bool,
    /// Each settled capture, until taken.
    outcomes: Vec<CaptureOutcome>,
    /// Why the first capture that failed wrote no file, until the run takes
    /// it.
    error: Option<anyhow::Error>,
}

/// One settled capture: where it was to go, and why it was not written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CaptureOutcome {
    pub(crate) frame: u64,
    pub(crate) path: PathBuf,
    pub(crate) error: Option<String>,
}

impl Capture {
    pub(crate) fn new(config: CaptureConfig) -> Self {
        Self {
            targets: vec![config],
            ended: false,
            outcomes: Vec::new(),
            error: None,
        }
    }

    /// No capture yet; a script [`add`](Self::add)s its own.
    pub(crate) fn empty() -> Self {
        Self {
            targets: Vec::new(),
            ended: false,
            outcomes: Vec::new(),
            error: None,
        }
    }

    /// Asks for `frame`'s render at `path`, through the blit path.
    pub(crate) fn add(&mut self, frame: u64, path: PathBuf) {
        let at = self.targets.partition_point(|t| t.target_frame <= frame);
        self.targets.insert(
            at,
            CaptureConfig {
                target_frame: frame,
                path,
                direct: false,
            },
        );
    }

    /// What to arm for `frame`, the frame about to render (counting from 1):
    /// a capture of that frame, the first asked for.
    pub(crate) fn armed_for(&self, frame: u64) -> Option<&CaptureConfig> {
        if self.ended {
            return None;
        }
        self.targets.iter().find(|t| t.target_frame == frame)
    }

    /// Settles `frame`'s captures with its outcome; another frame's changes
    /// nothing. A written file logs at info and is copied to the frame's
    /// other paths; any other outcome is a capture error, logged once per
    /// path.
    pub(crate) fn frame_rendered(&mut self, frame: u64, outcome: RenderOutcome) {
        if self.ended || !self.targets.iter().any(|t| t.target_frame == frame) {
            return;
        }
        let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.targets)
            .into_iter()
            .partition(|t| t.target_frame == frame);
        self.targets = later;
        let first = &now[0];
        let path = first.path.display();
        let result = match outcome {
            RenderOutcome::Presented {
                capture: Some(Ok(written)),
            } => {
                log::info!("captured frame {frame} -> {}", written.display());
                Ok(written)
            }
            RenderOutcome::Presented {
                capture: Some(Err(error)),
            } => Err(anyhow::Error::new(error)
                .context(format!("capture frame {frame} -> {path} failed"))),
            RenderOutcome::Presented { capture: None } => Err(anyhow::anyhow!(
                "capture frame {frame} -> {path} not written: the frame presented without it"
            )),
            // `Skipped`, and whatever a later renderer adds that presents nothing.
            _ => Err(anyhow::anyhow!(
                "capture frame {frame} -> {path} not written: the frame was skipped (no surface texture)"
            )),
        };
        for (index, target) in now.iter().enumerate() {
            let error = match &result {
                Ok(_) if index == 0 => None,
                Ok(written) => std::fs::copy(written, &target.path).err().map(|err| {
                    anyhow::anyhow!(
                        "capture frame {frame} -> {} not written: copying {} failed: {err}",
                        target.path.display(),
                        written.display()
                    )
                }),
                Err(error) if index == 0 => Some(anyhow::anyhow!("{error:#}")),
                Err(_) => Some(anyhow::anyhow!(
                    "capture frame {frame} -> {} not written: the frame's capture failed",
                    target.path.display()
                )),
            };
            self.settle(frame, target.path.clone(), error);
        }
    }

    /// Ends the captures with the run, after `frames` frames reached render:
    /// a frame that never came is a capture error. Returns the first
    /// capture error, once.
    pub(crate) fn finish(&mut self, frames: u64) -> Option<anyhow::Error> {
        if !self.ended {
            self.ended = true;
            for target in std::mem::take(&mut self.targets) {
                let error = anyhow::anyhow!(
                    "capture frame {} -> {} not written: the run ended after {frames} frames",
                    target.target_frame,
                    target.path.display()
                );
                self.settle(target.target_frame, target.path, Some(error));
            }
        }
        self.error.take()
    }

    /// The captures settled since the last call, in the order they settled.
    pub(crate) fn take_outcomes(&mut self) -> Vec<CaptureOutcome> {
        std::mem::take(&mut self.outcomes)
    }

    fn settle(&mut self, frame: u64, path: PathBuf, error: Option<anyhow::Error>) {
        let text = error.as_ref().map(|error| format!("{error:#}"));
        if let Some(error) = error {
            log::error!("{error:#}");
            if self.error.is_none() {
                self.error = Some(error);
            }
        }
        self.outcomes.push(CaptureOutcome {
            frame,
            path,
            error: text,
        });
    }
}

#[cfg(test)]
#[path = "tests/capture.rs"]
mod tests;
