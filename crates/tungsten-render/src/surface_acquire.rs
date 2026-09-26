//! Frame-loop policy for `Surface::get_current_texture` results (wgpu 30
//! `CurrentSurfaceTexture`). Kept free of GPU objects so every transition is
//! unit-testable; `Renderer::acquire_texture` carries out the actions.

/// Acquire outcome without its texture payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AcquireStatus {
    Success,
    Suboptimal,
    Timeout,
    Occluded,
    Outdated,
    Lost,
    Validation,
}

impl AcquireStatus {
    pub(crate) fn of(result: &wgpu::CurrentSurfaceTexture) -> Self {
        match result {
            wgpu::CurrentSurfaceTexture::Success(_) => Self::Success,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) => Self::Suboptimal,
            wgpu::CurrentSurfaceTexture::Timeout => Self::Timeout,
            wgpu::CurrentSurfaceTexture::Occluded => Self::Occluded,
            wgpu::CurrentSurfaceTexture::Outdated => Self::Outdated,
            wgpu::CurrentSurfaceTexture::Lost => Self::Lost,
            wgpu::CurrentSurfaceTexture::Validation => Self::Validation,
        }
    }
}

/// What the frame loop does next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AcquireAction {
    /// Draw into the acquired texture. With `reconfigure_after`, reconfigure the
    /// surface once the texture has been presented and released.
    Render { reconfigure_after: bool },
    /// Skip this frame; try again next frame.
    Skip,
    /// Reconfigure the surface, then acquire again.
    ReconfigureAndRetry,
    /// Recreate the surface from the window, configure it, then acquire again.
    RecreateAndRetry,
    /// Unrecoverable here: report an error.
    Fail,
}

/// Acquire attempts per frame: the first try plus one retry after a
/// reconfigure or recreation.
pub(crate) const MAX_ACQUIRE_ATTEMPTS: u32 = 2;

/// Policy for one acquire result; `attempt` counts from 0.
pub(crate) fn acquire_action(status: AcquireStatus, attempt: u32) -> AcquireAction {
    let last_attempt = attempt + 1 >= MAX_ACQUIRE_ATTEMPTS;
    match status {
        AcquireStatus::Success => AcquireAction::Render {
            reconfigure_after: false,
        },
        AcquireStatus::Suboptimal => AcquireAction::Render {
            reconfigure_after: true,
        },
        // Transient: minimized, hidden, or the compositor is slow.
        AcquireStatus::Timeout | AcquireStatus::Occluded => AcquireAction::Skip,
        // Still outdated after reconfiguring (e.g. mid-resize): wait a frame.
        AcquireStatus::Outdated if last_attempt => AcquireAction::Skip,
        AcquireStatus::Outdated => AcquireAction::ReconfigureAndRetry,
        // Lost again right after recreation points at device loss, which
        // this renderer does not recover from.
        AcquireStatus::Lost if last_attempt => AcquireAction::Fail,
        AcquireStatus::Lost => AcquireAction::RecreateAndRetry,
        AcquireStatus::Validation => AcquireAction::Fail,
    }
}

/// Whether to reconfigure after presenting a suboptimal frame. `window` is
/// the window's current size, `configured` the surface's, and `last` the size
/// already reconfigured for a suboptimal frame since the last clean acquire.
/// A size change always reconfigures. At an unchanged size, reconfiguring
/// can't clear a surface the platform keeps reporting as suboptimal, so it
/// happens once instead of recreating the swapchain every frame.
pub(crate) fn reconfigure_for_suboptimal(
    window: (u32, u32),
    configured: (u32, u32),
    last: Option<(u32, u32)>,
) -> bool {
    let resized = window.0 > 0 && window.1 > 0 && window != configured;
    resized || last != Some(configured)
}

#[cfg(test)]
#[path = "tests/surface_acquire.rs"]
mod tests;
