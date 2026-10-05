//! Screen geometry in UI units (`D-125`): window pixels until M0b's scale
//! factor.

use glam::Vec2;

/// A box in UI units, top-left origin, viewport space. Not physics'
/// centre-and-half-extents `Aabb`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Top-left corner.
    pub min: Vec2,
    /// Bottom-right corner.
    pub max: Vec2,
}

impl Rect {
    /// The rect from `min` to `max`.
    #[must_use]
    pub const fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// The rect with its top-left at `pos` and the given `size`.
    #[must_use]
    pub fn from_pos_size(pos: Vec2, size: Vec2) -> Self {
        Self {
            min: pos,
            max: pos + size,
        }
    }

    /// Width and height.
    #[must_use]
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// The width.
    #[must_use]
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// The height.
    #[must_use]
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Whether `point` is inside, edges included.
    #[must_use]
    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}
