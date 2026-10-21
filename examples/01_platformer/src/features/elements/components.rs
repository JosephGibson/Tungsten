//! The element rules' components.

use glam::Vec2;
use serde::{Deserialize, Serialize};

/// A small ball's fire: the game seconds it has left, kept at 0 once spent,
/// so a spent ball never lights again, and whether nothing rests on it, so
/// it burns on the pile's surface.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BallBurn {
    pub remaining: f32,
    /// Presentation, which `flame_exposure` refreshes each step.
    #[serde(skip, default = "exposed")]
    pub exposed: bool,
}

fn exposed() -> bool {
    true
}

/// One emitter of the burning surface's bounded pool.
#[derive(Debug, Clone, Copy)]
pub struct FireEmitter;

/// A body's partial frost, iron's or a ball's, 0 to 1; a full coating
/// turns it [`Frozen`]. A body a cold spray did not reach this step thaws.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Chill {
    pub amount: f32,
    /// A spray reached it this step: presentation of the step, never saved.
    #[serde(skip)]
    pub held: bool,
}

/// A frozen body, iron, a scrap or a ball, for `left` game seconds, when it
/// thaws: frozen iron slides on less friction; fire makes any frozen body
/// explode and a hard impact shatters it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frozen {
    pub left: f32,
}

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
