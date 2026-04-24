//! Declarative pass description: labels, attachments, clears.

pub use crate::targets::TargetId;

/// Clear-or-load + store for a single color attachment.
#[derive(Debug, Clone, Copy)]
pub struct PassDesc {
    pub label: &'static str,
    pub color: TargetId,
    pub color_resolve: Option<TargetId>,
    pub depth: Option<TargetId>,
    pub clear: Option<wgpu::Color>,
    pub depth_clear: Option<f32>,
}

impl PassDesc {
    #[must_use]
    pub const fn new(label: &'static str, color: TargetId) -> Self {
        Self {
            label,
            color,
            color_resolve: None,
            depth: None,
            clear: None,
            depth_clear: None,
        }
    }

    #[must_use]
    pub const fn with_clear(mut self, clear: wgpu::Color) -> Self {
        self.clear = Some(clear);
        self
    }

    #[must_use]
    pub const fn with_depth(mut self, depth: TargetId, clear: f32) -> Self {
        self.depth = Some(depth);
        self.depth_clear = Some(clear);
        self
    }

    #[must_use]
    pub const fn with_resolve(mut self, resolve: TargetId) -> Self {
        self.color_resolve = Some(resolve);
        self
    }
}
