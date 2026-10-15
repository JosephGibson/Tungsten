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

/// A uniform scale then an offset, in UI units: `p ↦ scale · p + offset`.
/// Paint and hit testing compose a node's translation and scale into one
/// down the tree (M56).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UiTransform {
    pub(crate) scale: f32,
    pub(crate) offset: Vec2,
}

impl UiTransform {
    /// No change.
    pub(crate) const IDENTITY: Self = Self {
        scale: 1.0,
        offset: Vec2::ZERO,
    };

    /// The node map `p ↦ centre + scale · (p − centre) + translation`.
    pub(crate) fn about(centre: Vec2, scale: f32, translation: Vec2) -> Self {
        Self {
            scale,
            offset: centre - centre * scale + translation,
        }
    }

    /// `self ∘ inner`: `inner` first, then `self`.
    pub(crate) fn compose(self, inner: Self) -> Self {
        Self {
            scale: self.scale * inner.scale,
            offset: inner.offset * self.scale + self.offset,
        }
    }

    pub(crate) fn point(self, p: Vec2) -> Vec2 {
        p * self.scale + self.offset
    }

    /// `rect` mapped; the scale is positive, so its corners stay ordered.
    pub(crate) fn rect(self, rect: Rect) -> Rect {
        Rect::new(self.point(rect.min), self.point(rect.max))
    }
}
