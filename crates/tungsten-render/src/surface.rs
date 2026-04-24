//! Present-mode resolution + surface pacing; device-free helpers live here.

use tungsten_core::config::PresentModeConfig;

use crate::renderer::RenderError;

pub(crate) fn present_mode_label(mode: wgpu::PresentMode) -> &'static str {
    match mode {
        wgpu::PresentMode::Fifo => "fifo",
        wgpu::PresentMode::FifoRelaxed => "fifo_relaxed",
        wgpu::PresentMode::Immediate => "immediate",
        wgpu::PresentMode::Mailbox => "mailbox",
        wgpu::PresentMode::AutoVsync => "auto_vsync",
        wgpu::PresentMode::AutoNoVsync => "auto_no_vsync",
    }
}

pub(crate) fn requested_present_mode_label(mode: PresentModeConfig) -> &'static str {
    mode.as_str()
}

fn choose_auto_vsync_present_mode(supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if supported.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        wgpu::PresentMode::AutoVsync
    }
}

fn choose_auto_no_vsync_present_mode(supported: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if supported.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else if supported.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else {
        wgpu::PresentMode::AutoNoVsync
    }
}

fn available_present_mode_labels(supported: &[wgpu::PresentMode]) -> Vec<String> {
    [
        wgpu::PresentMode::Fifo,
        wgpu::PresentMode::Immediate,
        wgpu::PresentMode::Mailbox,
    ]
    .into_iter()
    .filter(|mode| supported.contains(mode))
    .map(present_mode_label)
    .map(str::to_string)
    .collect()
}

pub(crate) fn resolve_present_mode(
    supported: &[wgpu::PresentMode],
    requested: Option<PresentModeConfig>,
    vsync: bool,
) -> Result<wgpu::PresentMode, RenderError> {
    let requested = requested.unwrap_or(PresentModeConfig::Auto);
    match requested {
        PresentModeConfig::Auto => {
            if vsync {
                Ok(choose_auto_vsync_present_mode(supported))
            } else {
                Ok(choose_auto_no_vsync_present_mode(supported))
            }
        }
        PresentModeConfig::AutoVsync => Ok(choose_auto_vsync_present_mode(supported)),
        PresentModeConfig::AutoNoVsync => Ok(choose_auto_no_vsync_present_mode(supported)),
        PresentModeConfig::Immediate => {
            if supported.contains(&wgpu::PresentMode::Immediate) {
                Ok(wgpu::PresentMode::Immediate)
            } else {
                Err(RenderError::UnsupportedPresentMode {
                    requested: requested_present_mode_label(requested).to_string(),
                    available: available_present_mode_labels(supported),
                })
            }
        }
        PresentModeConfig::Mailbox => {
            if supported.contains(&wgpu::PresentMode::Mailbox) {
                Ok(wgpu::PresentMode::Mailbox)
            } else {
                Err(RenderError::UnsupportedPresentMode {
                    requested: requested_present_mode_label(requested).to_string(),
                    available: available_present_mode_labels(supported),
                })
            }
        }
        PresentModeConfig::Fifo => {
            if supported.contains(&wgpu::PresentMode::Fifo) {
                Ok(wgpu::PresentMode::Fifo)
            } else {
                Err(RenderError::UnsupportedPresentMode {
                    requested: requested_present_mode_label(requested).to_string(),
                    available: available_present_mode_labels(supported),
                })
            }
        }
    }
}

fn default_max_frame_latency(present_mode: wgpu::PresentMode) -> u32 {
    if matches!(
        present_mode,
        wgpu::PresentMode::Immediate | wgpu::PresentMode::Mailbox | wgpu::PresentMode::AutoNoVsync
    ) {
        1
    } else {
        2
    }
}

pub(crate) fn resolve_max_frame_latency(
    requested: Option<u32>,
    present_mode: wgpu::PresentMode,
) -> Result<u32, RenderError> {
    match requested {
        Some(0) => Err(RenderError::InvalidFrameLatency(0)),
        Some(value) => Ok(value),
        None => Ok(default_max_frame_latency(present_mode)),
    }
}
