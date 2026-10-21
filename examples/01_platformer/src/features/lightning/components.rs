//! The lightning's components: its presentation, never saved.

use glam::Vec2;

/// A strike's bolt from the sky: where it landed, the segments stacked
/// above it from the bottom up, its age and the frames that showed it, so
/// its veil shows on the first alone.
#[derive(Debug, Clone, PartialEq)]
pub struct Bolt {
    pub landing: Vec2,
    pub pieces: Vec<usize>,
    pub age: f32,
    pub frames: u32,
}

/// A conduction jump's arc from one conductive thing to the next: its
/// zigzag's seed, its age and the frames that showed it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Arc {
    pub from: Vec2,
    pub to: Vec2,
    pub seed: u64,
    pub age: f32,
    pub frames: u32,
}

/// A strike's point light, fading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrikeLight {
    pub age: f32,
}
