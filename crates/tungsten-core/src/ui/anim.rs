//! UI animation tracks (M56): a [`UiTrack`] moves one paint-only property
//! of one widget (opacity, translation, scale, or a background, border or
//! text colour) from a start value to an end value over a duration, eased
//! and repeated with core's tween enums (`D-054`). [`ui_animate_system`]
//! ticks them on the real clock (`D-129`), so a paused game animates its
//! pause menu, and a finished track sends [`UiEvent::AnimationFinished`].
//! None of the six properties relayouts.

use glam::Vec2;

use crate::ecs::{EventQueue, World};
use crate::time::Time;
use crate::tween::{Easing, TweenDirection, TweenRepeat, lerp_f32, lerp_u8};

use super::event::UiEvent;
use super::id::WidgetId;
use super::theme::StyleOverrides;
use super::tree::UiTree;

/// The property a [`UiTrack`] moves.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiProperty {
    /// [`UiTree::set_opacity`].
    Opacity,
    /// [`UiTree::set_translation`].
    Translation,
    /// [`UiTree::set_scale`].
    Scale,
    /// The background colour, through the widget's own overrides.
    Background,
    /// The border colour, through the widget's own overrides.
    Border,
    /// The text colour, through the widget's own overrides; a button's
    /// label inherits it.
    TextColor,
}

/// A track's start and end values.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TrackValues {
    Opacity(f32, f32),
    Translation(Vec2, Vec2),
    Scale(f32, f32),
    Background([u8; 4], [u8; 4]),
    Border([u8; 4], [u8; 4]),
    TextColor([u8; 4], [u8; 4]),
}

/// One animation of one property (M56): built with a constructor per
/// property, then the `with_*` methods, and started with
/// [`UiTree::animate`]. Linear, played once, forward, tag 0 by default.
/// `Loop` and `PingPong` never finish; `Times(n)` finishes after `n`
/// cycles; `Backward` plays the timeline from its end to its start.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiTrack {
    values: TrackValues,
    duration: f32,
    easing: Easing,
    repeat: TweenRepeat,
    direction: TweenDirection,
    tag: u32,
}

impl UiTrack {
    fn new(values: TrackValues, secs: f32) -> Self {
        Self {
            values,
            duration: secs,
            easing: Easing::Linear,
            repeat: TweenRepeat::Once,
            direction: TweenDirection::Forward,
            tag: 0,
        }
    }

    /// Opacity from `from` to `to` over `secs` real seconds.
    #[must_use]
    pub fn opacity(from: f32, to: f32, secs: f32) -> Self {
        Self::new(TrackValues::Opacity(from, to), secs)
    }

    /// Translation in UI units from `from` to `to` over `secs`.
    #[must_use]
    pub fn translation(from: Vec2, to: Vec2, secs: f32) -> Self {
        Self::new(TrackValues::Translation(from, to), secs)
    }

    /// Scale from `from` to `to` over `secs`.
    #[must_use]
    pub fn scale(from: f32, to: f32, secs: f32) -> Self {
        Self::new(TrackValues::Scale(from, to), secs)
    }

    /// Background colour, sRGB, from `from` to `to` over `secs`.
    #[must_use]
    pub fn background(from: [u8; 4], to: [u8; 4], secs: f32) -> Self {
        Self::new(TrackValues::Background(from, to), secs)
    }

    /// Border colour, sRGB, from `from` to `to` over `secs`.
    #[must_use]
    pub fn border(from: [u8; 4], to: [u8; 4], secs: f32) -> Self {
        Self::new(TrackValues::Border(from, to), secs)
    }

    /// Text colour, sRGB, from `from` to `to` over `secs`.
    #[must_use]
    pub fn text_color(from: [u8; 4], to: [u8; 4], secs: f32) -> Self {
        Self::new(TrackValues::TextColor(from, to), secs)
    }

    /// This track with `easing` applied to its timeline.
    #[must_use]
    pub fn with_easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// This track repeated by `repeat`.
    #[must_use]
    pub fn with_repeat(mut self, repeat: TweenRepeat) -> Self {
        self.repeat = repeat;
        self
    }

    /// This track played `direction`.
    #[must_use]
    pub fn with_direction(mut self, direction: TweenDirection) -> Self {
        self.direction = direction;
        self
    }

    /// This track with `tag`, which its [`UiEvent::AnimationFinished`]
    /// carries (Q2: a number, so the event stays `Copy`).
    #[must_use]
    pub fn with_tag(mut self, tag: u32) -> Self {
        self.tag = tag;
        self
    }

    /// The property the track moves.
    #[must_use]
    pub fn property(&self) -> UiProperty {
        match self.values {
            TrackValues::Opacity(..) => UiProperty::Opacity,
            TrackValues::Translation(..) => UiProperty::Translation,
            TrackValues::Scale(..) => UiProperty::Scale,
            TrackValues::Background(..) => UiProperty::Background,
            TrackValues::Border(..) => UiProperty::Border,
            TrackValues::TextColor(..) => UiProperty::TextColor,
        }
    }

    /// The duration in real seconds, as given.
    #[must_use]
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// The tag.
    #[must_use]
    pub fn tag(&self) -> u32 {
        self.tag
    }

    /// Whether every value can be set: finite, and a scale positive.
    fn is_valid(&self) -> bool {
        match self.values {
            TrackValues::Opacity(from, to) => from.is_finite() && to.is_finite(),
            TrackValues::Translation(from, to) => from.is_finite() && to.is_finite(),
            TrackValues::Scale(from, to) => {
                from.is_finite() && to.is_finite() && from > 0.0 && to > 0.0
            }
            TrackValues::Background(..) | TrackValues::Border(..) | TrackValues::TextColor(..) => {
                true
            }
        }
    }

    /// Whether the duration can run: finite and positive.
    fn runs(&self) -> bool {
        self.duration.is_finite() && self.duration > 0.0
    }

    /// The timeline position `u` (0..=1) after the direction, as the eased
    /// fraction from start to end.
    fn eased(&self, u: f32) -> f32 {
        let u = match self.direction {
            TweenDirection::Forward => u,
            TweenDirection::Backward => 1.0 - u,
        };
        self.easing.apply(u.clamp(0.0, 1.0))
    }

    /// The start of the timeline: the start value forward, the end
    /// backward.
    fn start_fraction(&self) -> f32 {
        match self.direction {
            TweenDirection::Forward => 0.0,
            TweenDirection::Backward => 1.0,
        }
    }

    /// The end of the timeline: the end value forward, the start backward.
    fn end_fraction(&self) -> f32 {
        1.0 - self.start_fraction()
    }
}

/// A running track and where it is on its timeline.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunningTrack {
    pub(crate) widget: WidgetId,
    track: UiTrack,
    /// Seconds into the current cycle, or into the two-cycle span of a
    /// `PingPong`; the remainder carries across cycles.
    elapsed: f32,
    /// Cycles completed, for `Times`.
    cycles: u32,
}

/// What one tick did to a track.
enum Step {
    /// Write the value at this fraction.
    At(f32),
    /// Write the exact end value and drop the track.
    Finished,
    /// Nothing moved.
    Still,
}

impl RunningTrack {
    fn step(&mut self, dt: f32) -> Step {
        let track = self.track;
        if !track.runs() {
            return Step::Finished;
        }
        if dt <= 0.0 {
            return Step::Still;
        }
        let duration = track.duration;
        self.elapsed += dt;
        let u = match track.repeat {
            TweenRepeat::Once => {
                if self.elapsed >= duration {
                    return Step::Finished;
                }
                self.elapsed / duration
            }
            TweenRepeat::Times(n) => {
                let whole = (self.elapsed / duration).floor();
                if whole >= 1.0 {
                    self.cycles = self.cycles.saturating_add(whole as u32);
                    self.elapsed -= whole * duration;
                }
                if self.cycles >= n.max(1) {
                    return Step::Finished;
                }
                self.elapsed / duration
            }
            TweenRepeat::Loop => {
                self.elapsed = self.elapsed.rem_euclid(duration);
                self.elapsed / duration
            }
            TweenRepeat::PingPong => {
                self.elapsed = self.elapsed.rem_euclid(2.0 * duration);
                let u = self.elapsed / duration;
                if u <= 1.0 { u } else { 2.0 - u }
            }
        };
        Step::At(track.eased(u))
    }
}

/// Writes `track`'s value at `fraction` (0 the start, 1 the end) to
/// `widget`, through the paint-only setters or the widget's overrides.
fn write(tree: &mut UiTree, widget: WidgetId, track: &UiTrack, fraction: f32) {
    let color = |from: [u8; 4], to: [u8; 4]| -> [u8; 4] {
        std::array::from_fn(|i| lerp_u8(from[i], to[i], fraction))
    };
    match track.values {
        TrackValues::Opacity(from, to) => tree.set_opacity(widget, lerp_f32(from, to, fraction)),
        TrackValues::Translation(from, to) => tree.set_translation(widget, from.lerp(to, fraction)),
        TrackValues::Scale(from, to) => tree.set_scale(widget, lerp_f32(from, to, fraction)),
        TrackValues::Background(from, to) => {
            let value = color(from, to);
            override_with(tree, widget, |overrides| overrides.background = Some(value));
        }
        TrackValues::Border(from, to) => {
            let value = color(from, to);
            override_with(tree, widget, |overrides| overrides.border = Some(value));
        }
        TrackValues::TextColor(from, to) => {
            let value = color(from, to);
            override_with(tree, widget, |overrides| overrides.text.color = Some(value));
        }
    }
}

/// Sets `widget`'s overrides to its current ones changed by `change`.
fn override_with(tree: &mut UiTree, widget: WidgetId, change: impl FnOnce(&mut StyleOverrides)) {
    let Some(mut overrides) = tree.overrides(widget).cloned() else {
        return;
    };
    change(&mut overrides);
    tree.set_overrides(widget, overrides);
}

impl UiTree {
    /// Starts `track` on `id` (M56): writes its start value at once and
    /// replaces a track running on the same property, without an event
    /// (Q14). A track whose value is not finite, or whose scale is not
    /// positive, is refused with one warning and changes nothing; one whose
    /// duration is not finite or not positive finishes at the next tick on
    /// its end value. Returns whether the track started.
    pub fn animate(&mut self, id: WidgetId, track: UiTrack) -> bool {
        if !self.contains(id) {
            return false;
        }
        if !track.is_valid() {
            log::warn!(
                "UiTree: refused a {:?} track on {id:?} with a value that is not finite or a scale that is not positive",
                track.property()
            );
            return false;
        }
        let property = track.property();
        self.tracks
            .retain(|running| !(running.widget == id && running.track.property() == property));
        write(self, id, &track, track.start_fraction());
        self.tracks.push(RunningTrack {
            widget: id,
            track,
            elapsed: 0.0,
            cycles: 0,
        });
        true
    }

    /// Stops the track moving `property` on `id`, leaving the property
    /// where it is; no event. Does nothing when none runs.
    pub fn stop_animation(&mut self, id: WidgetId, property: UiProperty) {
        self.tracks
            .retain(|running| !(running.widget == id && running.track.property() == property));
    }

    /// Whether any track runs on `id`.
    #[must_use]
    pub fn is_animating(&self, id: WidgetId) -> bool {
        self.tracks.iter().any(|running| running.widget == id)
    }

    /// Advances every track by `dt` real seconds and writes their values;
    /// pushes an [`UiEvent::AnimationFinished`] per track that finished, in
    /// start order. A `dt` that is not positive moves nothing.
    pub(crate) fn tick_animations(&mut self, dt: f32, finished: &mut Vec<UiEvent>) {
        if self.tracks.is_empty() {
            return;
        }
        let dt = if dt.is_finite() { dt } else { 0.0 };
        let mut tracks = std::mem::take(&mut self.tracks);
        tracks.retain_mut(|running| {
            if !self.contains(running.widget) {
                return false;
            }
            let track = running.track;
            match running.step(dt) {
                Step::Still => true,
                Step::At(fraction) => {
                    write(self, running.widget, &track, fraction);
                    true
                }
                Step::Finished => {
                    write(self, running.widget, &track, track.end_fraction());
                    finished.push(UiEvent::AnimationFinished {
                        widget: running.widget,
                        property: track.property(),
                        tag: track.tag,
                    });
                    false
                }
            }
        });
        self.tracks = tracks;
    }
}

/// Ticks every [`UiTree`] animation track on [`Time::real_delta`] (`D-129`),
/// so tracks run while the game clock is paused, and sends
/// [`UiEvent::AnimationFinished`] for each that finished when
/// `EventQueue<UiEvent>` is registered. The umbrella's `UiPlugin` registers
/// it as `ui_animate`, after the routing. A world without a tree does
/// nothing.
pub fn ui_animate_system(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::real_delta);
    let Some(tree) = world.get_resource_mut::<UiTree>() else {
        return;
    };
    if tree.tracks.is_empty() {
        return;
    }
    let mut finished = Vec::new();
    tree.tick_animations(dt, &mut finished);
    if !finished.is_empty()
        && world.has_event::<UiEvent>()
        && let Some(queue) = world.get_resource_mut::<EventQueue<UiEvent>>()
    {
        for event in finished {
            queue.send(event);
        }
    }
}

#[cfg(test)]
#[path = "../tests/ui/anim.rs"]
mod tests;
