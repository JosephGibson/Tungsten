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
                    requested: requested.as_str().to_string(),
                    available: available_present_mode_labels(supported),
                })
            }
        }
        PresentModeConfig::Mailbox => {
            if supported.contains(&wgpu::PresentMode::Mailbox) {
                Ok(wgpu::PresentMode::Mailbox)
            } else {
                Err(RenderError::UnsupportedPresentMode {
                    requested: requested.as_str().to_string(),
                    available: available_present_mode_labels(supported),
                })
            }
        }
        PresentModeConfig::Fifo => {
            if supported.contains(&wgpu::PresentMode::Fifo) {
                Ok(wgpu::PresentMode::Fifo)
            } else {
                Err(RenderError::UnsupportedPresentMode {
                    requested: requested.as_str().to_string(),
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

/// The features the device validates `format` against, as wgpu-core decides
/// them (`describe_format_features`): the adapter's own when the device has
/// `TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES` or the adapter lacks WebGPU's
/// format guarantees, the WebGPU guarantee otherwise.
pub(crate) fn device_format_flags(
    device_features: wgpu::Features,
    downlevel: wgpu::DownlevelFlags,
    adapter_flags: wgpu::TextureFormatFeatureFlags,
    format: wgpu::TextureFormat,
) -> wgpu::TextureFormatFeatureFlags {
    if device_features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
        || !downlevel.contains(wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT)
    {
        adapter_flags
    } else {
        format.guaranteed_format_features(device_features).flags
    }
}

/// The MSAA sample count the scene renders with: `requested` when the color
/// target's format takes it and resolves, and the depth target's does too
/// when there is one; else 4 under the same test; else 1.
pub(crate) fn msaa_sample_count(
    requested: u32,
    color: wgpu::TextureFormatFeatureFlags,
    depth: Option<wgpu::TextureFormatFeatureFlags>,
) -> u32 {
    let takes = |count: u32| {
        count == 1
            || (color.sample_count_supported(count)
                && color.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)
                && depth.is_none_or(|depth| depth.sample_count_supported(count)))
    };
    [requested, 4]
        .into_iter()
        .find(|&count| takes(count))
        .unwrap_or(1)
}

/// The surface size for a window of `size`: each side at most
/// `max_dimension`, the device's `max_texture_dimension_2d`, since wgpu
/// rejects a larger surface (a validation error, which panics).
pub(crate) fn surface_extent(size: (u32, u32), max_dimension: u32) -> (u32, u32) {
    (size.0.min(max_dimension), size.1.min(max_dimension))
}
