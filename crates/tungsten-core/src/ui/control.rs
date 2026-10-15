//! Checkboxes and sliders (W1 BC, M55, `D-162`): the [`SliderRange`] a
//! slider's value lives in, the rules that keep a value inside it, the
//! [`UiTree`] builders, getters and setters, and the geometry the paint walk
//! draws them with. Values are computed in `f64` from the range's minimum
//! and a whole number of steps, never accumulated, and stored as `f32`
//! clamped to the bounds, so every finite range gives finite values. The
//! setters mark paint and send no event; only the router reports a change,
//! as `UiEvent::ValueChanged`.

use glam::Vec2;

use super::geometry::Rect;
use super::id::WidgetId;
use super::node::WidgetKind;
use super::style::LayoutStyle;
use super::tree::UiTree;

/// The values a slider may hold: `min..=max`, either every value
/// (continuous) or the values `min + k·step` for whole `k` up to `max`, and
/// `max` itself, so `max` stays reachable when the span is not a whole
/// number of steps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderRange {
    min: f32,
    max: f32,
    step: f32,
}

impl SliderRange {
    /// `min..=max`, continuous; `min == max` holds one value.
    ///
    /// # Panics
    ///
    /// When a bound is not finite or `min > max`.
    #[must_use]
    pub fn new(min: f32, max: f32) -> Self {
        assert!(
            min.is_finite() && max.is_finite(),
            "SliderRange: bounds must be finite ({min}, {max})"
        );
        assert!(min <= max, "SliderRange: min {min} is above max {max}");
        Self {
            min,
            max,
            step: 0.0,
        }
    }

    /// This range on the values `min + k·step` and `max`; a step of 0 is
    /// continuous.
    ///
    /// # Panics
    ///
    /// When `step` is negative or not finite.
    #[must_use]
    pub fn with_step(self, step: f32) -> Self {
        assert!(
            step.is_finite() && step >= 0.0,
            "SliderRange: step must be finite and not negative ({step})"
        );
        Self { step, ..self }
    }

    /// The lowest value.
    #[must_use]
    pub fn min(self) -> f32 {
        self.min
    }

    /// The highest value.
    #[must_use]
    pub fn max(self) -> f32 {
        self.max
    }

    /// The distance between allowed values; 0 for a continuous range.
    #[must_use]
    pub fn step(self) -> f32 {
        self.step
    }

    /// `value` clamped to the bounds and moved to the nearest allowed value,
    /// the lower on a tie. `max` is a candidate of its own, so it stays
    /// reachable when the step index loses precision (a span past 2^53
    /// steps). `value` must be finite.
    pub(crate) fn snap(self, value: f32) -> f32 {
        self.snap_wide(f64::from(value))
    }

    /// [`Self::snap`] of an `f64` value.
    fn snap_wide(self, value: f64) -> f32 {
        let (min, max) = (f64::from(self.min), f64::from(self.max));
        let value = value.clamp(min, max);
        if self.step == 0.0 {
            return self.store(value);
        }
        let step = f64::from(self.step);
        let k = ((value - min) / step).floor();
        let lower = min + k * step;
        let upper = (min + (k + 1.0) * step).min(max);
        let mut nearest = lower;
        for candidate in [upper, max] {
            if (candidate - value).abs() < (nearest - value).abs() {
                nearest = candidate;
            }
        }
        self.store(nearest)
    }

    /// Where `value` sits between the bounds, 0 to 1; 0 for a range holding
    /// one value.
    pub(crate) fn fraction(self, value: f32) -> f32 {
        let span = f64::from(self.max) - f64::from(self.min);
        if span <= 0.0 {
            return 0.0;
        }
        ((f64::from(value) - f64::from(self.min)) / span).clamp(0.0, 1.0) as f32
    }

    /// The value a key press moves `value` to, toward `max` when `up`, else
    /// toward `min`: the next allowed value that way, or a tenth of the span
    /// for a continuous range. When that value, stored as `f32`, equals
    /// `value` (a step finer than the `f32` spacing there), the first allowed
    /// value at or past the adjacent `f32` that way, and failing that the
    /// end, so every press short of the end moves the value. `value` at that
    /// end stays there.
    pub(crate) fn key_step(self, value: f32, up: bool) -> f32 {
        let end = if up { self.max } else { self.min };
        if (up && value >= self.max) || (!up && value <= self.min) {
            return end;
        }
        let moved = |next: f32| if up { next > value } else { next < value };
        let (min, max) = (f64::from(self.min), f64::from(self.max));
        let at = f64::from(value);
        let step = f64::from(self.step);
        let next = self.store(if self.step == 0.0 {
            let delta = (max - min) / 10.0;
            if up { at + delta } else { at - delta }
        } else {
            let k = (at - min) / step;
            if up {
                min + (k.floor() + 1.0) * step
            } else {
                min + (k.ceil() - 1.0) * step
            }
        });
        if moved(next) {
            return next;
        }
        let adjacent = if up {
            value.next_up()
        } else {
            value.next_down()
        };
        let past = f64::from(adjacent);
        let next = self.store(if self.step == 0.0 {
            past
        } else {
            let k = (past - min) / step;
            min + if up { k.ceil() } else { k.floor() } * step
        });
        if moved(next) { next } else { end }
    }

    /// `value` as stored: the nearest `f32`, clamped to the bounds.
    fn store(self, value: f64) -> f32 {
        (value as f32).clamp(self.min, self.max)
    }
}

/// A toggle's state or a slider's value and range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Control {
    Toggle(bool),
    Slider { value: f32, range: SliderRange },
}

impl Control {
    /// The size an `Auto` axis takes for a control whose resolved line
    /// height is `line`: a toggle one line square, a slider ten lines wide
    /// and one tall.
    pub(crate) fn natural_size(self, line: f32) -> (f32, f32) {
        match self {
            Self::Toggle(_) => (line, line),
            Self::Slider { .. } => (line * 10.0, line),
        }
    }
}

/// The side of the square thumb of a slider whose box is `rect`: its
/// height, or its width when narrower.
fn thumb_side(rect: Rect) -> f32 {
    rect.height().min(rect.width())
}

/// The value at the pointer's `x` on a slider whose box is `rect`: the
/// fraction `(x − (start + t / 2)) / (width − t)` clamped to 0–1 (0 when
/// `width ≤ t`), between the bounds with exact ends, snapped. In UI units.
pub(crate) fn pointer_value(rect: Rect, range: SliderRange, x: f32) -> f32 {
    let side = thumb_side(rect);
    let travel = f64::from(rect.width() - side);
    let fraction = if travel <= 0.0 {
        0.0
    } else {
        ((f64::from(x) - f64::from(rect.min.x + side / 2.0)) / travel).clamp(0.0, 1.0)
    };
    range.snap_wide(f64::from(range.min) * (1.0 - fraction) + f64::from(range.max) * fraction)
}

/// A toggle's mark inside its box `rect` with corners of `radius`: the box
/// inset on every side by a quarter of its shorter side, with the box's
/// radius less the inset, not below 0. In UI units.
pub(crate) fn toggle_mark(rect: Rect, radius: f32) -> (Rect, f32) {
    let inset = rect.width().min(rect.height()) / 4.0;
    let mark = Rect::new(rect.min + Vec2::splat(inset), rect.max - Vec2::splat(inset));
    (mark, (radius - inset).max(0.0))
}

/// A slider's painted parts, in UI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SliderParts {
    /// The full-width track, a quarter of the height thick on the centre line.
    pub(crate) track: Rect,
    /// The track's and the fill's corner radius.
    pub(crate) track_radius: f32,
    /// The track from its start to the thumb's centre.
    pub(crate) fill: Rect,
    /// The square thumb, as tall as the slider unless it is narrower.
    pub(crate) thumb: Rect,
}

/// The parts of a slider whose box is `rect` and whose value sits at
/// `fraction` (0 to 1), with corners of `radius`: the thumb's side
/// `t = min(height, width)`, its centre at `start + t / 2 + fraction ·
/// (width − t)`.
pub(crate) fn slider_parts(rect: Rect, fraction: f32, radius: f32) -> SliderParts {
    let (width, height) = (rect.width(), rect.height());
    let thumb_side = thumb_side(rect);
    let centre_y = rect.min.y + height / 2.0;
    let thickness = height / 4.0;
    let thumb_x = rect.min.x + thumb_side / 2.0 + fraction * (width - thumb_side);
    let track = Rect::new(
        Vec2::new(rect.min.x, centre_y - thickness / 2.0),
        Vec2::new(rect.max.x, centre_y + thickness / 2.0),
    );
    SliderParts {
        track,
        track_radius: radius.min(height / 8.0),
        fill: Rect::new(track.min, Vec2::new(thumb_x, track.max.y)),
        thumb: Rect::from_pos_size(
            Vec2::new(thumb_x - thumb_side / 2.0, centre_y - thumb_side / 2.0),
            Vec2::splat(thumb_side),
        ),
    }
}

impl UiTree {
    /// A checkbox under `parent`, `on` or off: a focusable, interactive leaf
    /// with the `CheckBox` role. Its caption is a sibling label; give it the
    /// caption as its accessible name. An activation flips it.
    ///
    /// # Panics
    ///
    /// When `parent` is not in the tree, or holds no children.
    pub fn toggle(&mut self, parent: WidgetId, on: bool) -> WidgetId {
        let id = self.add_child(parent, WidgetKind::Toggle, LayoutStyle::default());
        if let Some(node) = self.node_mut(id) {
            node.control = Some(Control::Toggle(on));
        }
        id
    }

    /// A horizontal slider under `parent` at `value`, clamped and snapped
    /// into `range`; a value that is not finite starts it at `range.min()`.
    /// A focusable, interactive leaf with the `Slider` role.
    ///
    /// # Panics
    ///
    /// When `parent` is not in the tree, or holds no children.
    pub fn slider(&mut self, parent: WidgetId, range: SliderRange, value: f32) -> WidgetId {
        let id = self.add_child(parent, WidgetKind::Slider, LayoutStyle::default());
        let value = if value.is_finite() {
            range.snap(value)
        } else {
            range.min()
        };
        if let Some(node) = self.node_mut(id) {
            node.control = Some(Control::Slider { value, range });
        }
        id
    }

    /// A toggle's state; `None` for any other widget.
    #[must_use]
    pub fn toggled(&self, id: WidgetId) -> Option<bool> {
        match self.node(id)?.control? {
            Control::Toggle(on) => Some(on),
            Control::Slider { .. } => None,
        }
    }

    /// Sets a toggle's state and returns whether it changed. Marks paint
    /// when it did; sends no event. Does nothing for any other widget.
    pub fn set_toggled(&mut self, id: WidgetId, on: bool) -> bool {
        let Some(Control::Toggle(current)) =
            self.node_mut(id).and_then(|node| node.control.as_mut())
        else {
            return false;
        };
        if *current == on {
            return false;
        }
        *current = on;
        self.mark_paint_dirty();
        true
    }

    /// A slider's value; `None` for any other widget.
    #[must_use]
    pub fn slider_value(&self, id: WidgetId) -> Option<f32> {
        match self.node(id)?.control? {
            Control::Slider { value, .. } => Some(value),
            Control::Toggle(_) => None,
        }
    }

    /// Sets a slider's value, clamped and snapped, and returns whether it
    /// changed; a value that is not finite changes nothing. Marks paint when
    /// it changed; sends no event. Does nothing for any other widget.
    pub fn set_slider_value(&mut self, id: WidgetId, value: f32) -> bool {
        if !value.is_finite() {
            return false;
        }
        let Some(Control::Slider {
            value: current,
            range,
        }) = self.node_mut(id).and_then(|node| node.control.as_mut())
        else {
            return false;
        };
        let value = range.snap(value);
        if *current == value {
            return false;
        }
        *current = value;
        self.mark_paint_dirty();
        true
    }

    /// A slider's range; `None` for any other widget.
    #[must_use]
    pub fn slider_range(&self, id: WidgetId) -> Option<SliderRange> {
        match self.node(id)?.control? {
            Control::Slider { range, .. } => Some(range),
            Control::Toggle(_) => None,
        }
    }

    /// Replaces a slider's range and snaps its value into it. Marks paint
    /// when the range or the value changed; sends no event. Does nothing for
    /// any other widget.
    pub fn set_slider_range(&mut self, id: WidgetId, range: SliderRange) {
        let Some(Control::Slider {
            value,
            range: current,
        }) = self.node_mut(id).and_then(|node| node.control.as_mut())
        else {
            return;
        };
        let snapped = range.snap(*value);
        if *current == range && *value == snapped {
            return;
        }
        *current = range;
        *value = snapped;
        self.mark_paint_dirty();
    }
}

#[cfg(test)]
#[path = "../tests/ui/control.rs"]
mod tests;
