//! The element rules' components.

use glam::Vec2;

/// A lit small ball's flame: whether nothing rests on it, so it burns on
/// the pile's surface. Presentation, which `flame_exposure` refreshes each
/// step; the fire itself is the ball's `ball_burn` and `kindled` statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flame {
    pub exposed: bool,
}

/// Where the cold last met a body, so its freeze's crystals show on that
/// face: presentation, which `chill` writes and the `freeze` handler reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColdFace(pub Vec2);

/// One emitter of the burning surface's bounded pool.
#[derive(Debug, Clone, Copy)]
pub struct FireEmitter;

/// A brief icy ring where iron froze or shattered: presentation.
#[derive(Debug, Clone, Copy)]
pub struct IcePulse {
    pub at: Vec2,
    pub size: f32,
    pub age: f32,
    pub shatter: bool,
}

impl IcePulse {
    /// How long it shows.
    #[must_use]
    pub fn duration(self) -> f32 {
        if self.shatter { 0.32 } else { 0.24 }
    }
}
