//! Screen transitions (M31, `D-093`): a full-screen effect that covers the
//! frame, lets a state change happen underneath and uncovers the result.
//!
//! This module holds the plain types and the timing; `StateStack` queues a
//! transition with its state command and the app appends
//! [`TransitionEffect::pass_at`] to the frame's post stack. Screen-space text
//! draws after the post stack and is not covered.

use tungsten_core::Easing;
use tungsten_core::post::{DissolveParams, FadeParams, PostPass, WipeRadialParams};

/// Full-screen effect of one transition phase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransitionEffect {
    /// Mix the frame toward `color`.
    Fade { color: [f32; 4] },
    /// Close a circle on `center` (UV); `softness` is the edge width in UV.
    WipeRadial { center: [f32; 2], softness: f32 },
    /// Burn the frame away through noise, with `edge_color` at the front.
    Dissolve {
        noise_scale: f32,
        edge_color: [f32; 4],
    },
    /// Coarsen the frame up to `max_block_px` blocks; it never hides the frame.
    Pixelate { max_block_px: f32 },
}

impl TransitionEffect {
    /// The post pass for `cover` in [0, 1]: 0 leaves the frame untouched, 1
    /// hides it as far as the effect can.
    #[must_use]
    pub fn pass_at(self, cover: f32) -> PostPass {
        let cover = if cover.is_finite() {
            cover.clamp(0.0, 1.0)
        } else {
            0.0
        };
        match self {
            Self::Fade { color } => PostPass::Fade(FadeParams {
                progress: cover,
                color,
            }),
            Self::WipeRadial { center, softness } => {
                // `wipe_radial.wgsl` is inverted: `radius = progress * 1.5`
                // shows the frame inside the circle. Scaling by the farthest
                // corner starts the wipe there instead of off screen, and the
                // shrinking softness closes the soft dot `progress = 0` would
                // leave at the center.
                let softness = if softness.is_finite() {
                    softness.max(0.0)
                } else {
                    0.0
                };
                let far = farthest_corner_distance(center);
                let open = 1.0 - cover;
                PostPass::WipeRadial(WipeRadialParams {
                    progress: open * ((far + softness) / 1.5).min(1.0),
                    center,
                    softness: softness * open,
                })
            }
            Self::Dissolve {
                noise_scale,
                edge_color,
            } => PostPass::Dissolve(DissolveParams {
                progress: cover,
                noise_scale,
                edge_color,
            }),
            Self::Pixelate { max_block_px } => {
                PostPass::Pixelate(1.0 + (max_block_px.max(1.0) - 1.0) * cover)
            }
        }
    }
}

/// Largest distance from `center` to a corner of the unit UV square.
fn farthest_corner_distance(center: [f32; 2]) -> f32 {
    let dx = center[0].abs().max((1.0 - center[0]).abs());
    let dy = center[1].abs().max((1.0 - center[1]).abs());
    dx.hypot(dy)
}

/// A screen transition: cover the frame with `out_effect`, apply the state
/// command on the boundary frame, uncover with `in_effect`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    pub out_effect: TransitionEffect,
    pub in_effect: TransitionEffect,
    pub out_secs: f32,
    pub in_secs: f32,
    pub easing: Easing,
}

impl Transition {
    /// One effect for both phases, `secs` each, linear.
    #[must_use]
    pub fn new(effect: TransitionEffect, secs: f32) -> Self {
        Self {
            out_effect: effect,
            in_effect: effect,
            out_secs: secs,
            in_secs: secs,
            easing: Easing::Linear,
        }
    }

    #[must_use]
    pub fn with_in_effect(mut self, effect: TransitionEffect) -> Self {
        self.in_effect = effect;
        self
    }

    #[must_use]
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// How far the frame is covered at `state`, in [0, 1].
    #[must_use]
    pub fn cover(&self, state: TransitionState) -> f32 {
        let (secs, rising) = match state.phase {
            TransitionPhase::Out => (duration(self.out_secs), true),
            TransitionPhase::In => (duration(self.in_secs), false),
        };
        let t = if secs > 0.0 {
            (state.elapsed / secs).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let eased = self.easing.apply(t).clamp(0.0, 1.0);
        if rising { eased } else { 1.0 - eased }
    }

    /// The post pass to draw at `state`.
    #[must_use]
    pub fn pass(&self, state: TransitionState) -> PostPass {
        let effect = match state.phase {
            TransitionPhase::Out => self.out_effect,
            TransitionPhase::In => self.in_effect,
        };
        effect.pass_at(self.cover(state))
    }

    /// Advance `state` by `dt` seconds.
    ///
    /// `Boundary` ends `Out`: the caller applies the state command, then calls
    /// [`Self::begin_in`]. Leftover time is dropped there, so the boundary
    /// frame always draws fully covered.
    pub(crate) fn advance(&self, state: &mut TransitionState, dt: f32) -> TransitionStep {
        state.elapsed += duration(dt);
        match state.phase {
            TransitionPhase::Out if state.elapsed >= duration(self.out_secs) => {
                TransitionStep::Boundary
            }
            TransitionPhase::In if state.elapsed >= duration(self.in_secs) => {
                TransitionStep::Finished
            }
            _ => TransitionStep::Running,
        }
    }

    /// Enter `In` after the boundary. `Finished` when there is no `In` phase to
    /// run.
    pub(crate) fn begin_in(&self, state: &mut TransitionState) -> TransitionStep {
        state.phase = TransitionPhase::In;
        state.elapsed = 0.0;
        if duration(self.in_secs) > 0.0 {
            TransitionStep::Running
        } else {
            TransitionStep::Finished
        }
    }
}

/// A duration or time step in seconds; non-finite and negative values count
/// as 0.
fn duration(secs: f32) -> f32 {
    if secs.is_finite() && secs > 0.0 {
        secs
    } else {
        0.0
    }
}

/// Which half of a transition is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionPhase {
    /// Covering the old state.
    Out,
    /// Uncovering the new state.
    In,
}

/// Progress of the active transition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionState {
    pub phase: TransitionPhase,
    pub elapsed: f32,
}

impl TransitionState {
    /// The start of `Out`.
    pub(crate) const START: Self = Self {
        phase: TransitionPhase::Out,
        elapsed: 0.0,
    };
}

/// Result of advancing a transition by one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransitionStep {
    Running,
    /// `Out` completed: apply the state command now.
    Boundary,
    Finished,
}

#[cfg(test)]
#[path = "tests/transition.rs"]
mod tests;
