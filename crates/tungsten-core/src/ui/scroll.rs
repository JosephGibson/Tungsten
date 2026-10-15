//! Scroll panels (M57): vertical panels that clip their children and move
//! them under a paint-only offset. Layout rects never move: paint, hit
//! testing and [`UiTree::visual_rect`] map a panel's children through a
//! translation of minus its offset, rounded to whole physical pixels as
//! translations are, while directional focus keeps the layout rects. The
//! router's wheel moves the innermost panel that can move that way, every
//! focus change scrolls the new focus into view, and a thumb shows where the
//! view sits in the content.

use glam::Vec2;

use super::geometry::{Rect, UiTransform};
use super::id::WidgetId;
use super::node::{Node, WidgetKind};
use super::style::LayoutStyle;
use super::tree::UiTree;

/// The thumb's width in UI units.
pub(crate) const THUMB_WIDTH: f32 = 4.0;

/// The thumb's shortest length in UI units.
const THUMB_MIN_LENGTH: f32 = 16.0;

/// A scroll panel's state.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ScrollState {
    /// The offset as set: 0 or more, `+∞` to follow the end. Clamped to
    /// `0..=max` where it is used.
    pub(crate) offset: f32,
    /// The largest offset the last layout left; 0 before it.
    pub(crate) max: f32,
    /// The content box's top and bottom insets at the last layout, padding
    /// and border, which bound the view [`UiTree::scroll_into_view`] fits.
    pub(crate) insets: (f32, f32),
    /// Whether a layout has measured the panel.
    pub(crate) laid_out: bool,
}

impl ScrollState {
    /// The offset in use: the one set, clamped to `0..=max`.
    pub(crate) fn current(self) -> f32 {
        self.offset.clamp(0.0, self.max)
    }

    /// `offset` against a known `max`: at or past the end, it follows the
    /// end; a panel whose content fits keeps 0 as it is.
    fn settle(offset: f32, max: f32) -> f32 {
        if offset > max || (max > 0.0 && offset >= max) {
            f32::INFINITY
        } else {
            offset
        }
    }

    /// A layout's measurement: the new maximum and insets. An offset at or
    /// past the new end follows it through later layouts.
    pub(crate) fn measured(&mut self, max: f32, insets: (f32, f32)) {
        self.max = max;
        self.insets = insets;
        self.laid_out = true;
        self.offset = Self::settle(self.offset, max);
    }
}

/// The map a node's children paint and hit through on top of the node's
/// own: for a scroll panel, a translation of minus its offset, rounded to
/// whole physical pixels at `paint_scale`; the identity for anything else.
pub(crate) fn children_transform(node: &Node, paint_scale: f32) -> UiTransform {
    let Some(state) = node.scroll else {
        return UiTransform::IDENTITY;
    };
    let offset = state.current();
    if offset == 0.0 {
        return UiTransform::IDENTITY;
    }
    let offset = if paint_scale > 0.0 && paint_scale.is_finite() {
        (offset * paint_scale).round() / paint_scale
    } else {
        offset
    };
    UiTransform {
        scale: 1.0,
        offset: Vec2::new(0.0, -offset),
    }
}

/// A scroll panel's thumb in UI units, for its layout box `rect` inside a
/// border of `border` units: [`THUMB_WIDTH`] wide at the inner right edge,
/// as long as the visible share of the content and at least 16 units, placed
/// along the inner height as far as the offset is along the maximum. `None`
/// when the content fits or the box has no inner height.
pub(crate) fn thumb(rect: Rect, state: ScrollState, border: f32) -> Option<Rect> {
    if state.max <= 0.0 {
        return None;
    }
    let border = border.max(0.0);
    let top = rect.min.y + border;
    let track = rect.max.y - border - top;
    if track <= 0.0 {
        return None;
    }
    let visible = rect.height() / (rect.height() + state.max);
    let length = (track * visible).max(THUMB_MIN_LENGTH).min(track);
    let start = top + (track - length) * (state.current() / state.max);
    let right = rect.max.x - border;
    Some(Rect::new(
        Vec2::new(right - THUMB_WIDTH, start),
        Vec2::new(right, start + length),
    ))
}

impl UiTree {
    /// A scroll panel under `parent` (M57): a panel that clips its children
    /// and scrolls them vertically. Its main size, when `Auto`, takes its
    /// content's height up to what its parent's limits leave, so a
    /// `max_height`, a fixed size or `Fill` makes it scroll. While the
    /// content does not fit, a thumb paints at its inner right edge, in its
    /// text colour at half its alpha, after its children. The offset starts
    /// at 0. Panics when `parent` is not in the tree.
    pub fn scroll_panel(&mut self, parent: WidgetId, style: LayoutStyle) -> WidgetId {
        let id = self.add_child(parent, WidgetKind::Panel, style);
        if let Some(node) = self.node_mut(id) {
            node.clip_children = true;
            node.scroll = Some(ScrollState::default());
        }
        id
    }

    /// Sets a scroll panel's offset in UI units from the top of its
    /// content. It is clamped to `0..=`[`Self::scroll_max`] where it is
    /// used, so it can be set before a layout; an offset at or past the end
    /// follows the end through later layouts, and `f32::INFINITY` follows it
    /// from the start. NaN, `-∞` and negative offsets are 0. Returns whether
    /// the stored offset changed, and then marks paint, never layout; does
    /// nothing for a widget that is not a scroll panel.
    pub fn set_scroll(&mut self, id: WidgetId, offset: f32) -> bool {
        let Some(state) = self.node_mut(id).and_then(|node| node.scroll.as_mut()) else {
            return false;
        };
        let offset = if offset.is_nan() {
            0.0
        } else {
            offset.max(0.0)
        };
        let offset = if state.laid_out {
            ScrollState::settle(offset, state.max)
        } else {
            offset
        };
        if state.offset == offset {
            return false;
        }
        state.offset = offset;
        self.mark_paint_dirty();
        true
    }

    /// A scroll panel's offset in use, the one set clamped to
    /// `0..=`[`Self::scroll_max`]; `None` for a widget that is not a scroll
    /// panel.
    #[must_use]
    pub fn scroll(&self, id: WidgetId) -> Option<f32> {
        self.node(id)?.scroll.map(ScrollState::current)
    }

    /// A scroll panel's largest offset from the last layout: how far its
    /// lowest child's box, margin included, and its bottom padding reach
    /// past its box; 0 before the first layout or while the content fits.
    /// `None` for a widget that is not a scroll panel.
    #[must_use]
    pub fn scroll_max(&self, id: WidgetId) -> Option<f32> {
        self.node(id)?.scroll.map(|state| state.max)
    }

    /// Scrolls each scroll panel above `id`, innermost first, the least
    /// that shows `id`'s box inside the panel's padding, from the last
    /// layout's rects; a box taller than the view shows its top. The panels
    /// further out fit the box where the inner ones leave it. Returns
    /// whether any panel moved, and then marks paint; before a first layout
    /// nothing moves. Every focus change, the router's or the game's, calls
    /// it for the new focus.
    pub fn scroll_into_view(&mut self, id: WidgetId) -> bool {
        let Some(mut target) = self.rect(id) else {
            return false;
        };
        let mut moved = false;
        let mut current = self.parent(id);
        while let Some(panel) = current {
            let Some(node) = self.node_mut(panel) else {
                break;
            };
            current = node.parent;
            let (Some(rect), Some(state)) = (node.rect, node.scroll.as_mut()) else {
                continue;
            };
            if !state.laid_out {
                continue;
            }
            let top = rect.min.y + state.insets.0;
            let bottom = rect.max.y - state.insets.1;
            let offset = state.current();
            let mut next = offset;
            if target.max.y > bottom + next {
                next = target.max.y - bottom;
            }
            if target.min.y < top + next {
                next = target.min.y - top;
            }
            let next = next.clamp(0.0, state.max);
            if next != offset {
                state.offset = ScrollState::settle(next, state.max);
                moved = true;
            }
            let shift = Vec2::new(0.0, next);
            target = Rect::new(target.min - shift, target.max - shift);
        }
        if moved {
            self.mark_paint_dirty();
        }
        moved
    }

    /// Moves a scroll panel's offset by `change` UI units, positive towards
    /// the end, clamped; reaching the end follows it. Returns the offset it
    /// left, or `None` when `id` is not a scroll panel or cannot move that
    /// way.
    pub(crate) fn scroll_by(&mut self, id: WidgetId, change: f32) -> Option<f32> {
        let state = self.node_mut(id)?.scroll.as_mut()?;
        let offset = state.current();
        let next = (offset + change).clamp(0.0, state.max);
        if next == offset {
            return None;
        }
        state.offset = ScrollState::settle(next, state.max);
        self.mark_paint_dirty();
        Some(next)
    }
}

#[cfg(test)]
#[path = "../tests/ui/scroll.rs"]
mod tests;
