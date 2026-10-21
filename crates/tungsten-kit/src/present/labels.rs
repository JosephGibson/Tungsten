//! Timed world labels: a cue's `label` action floats an event's amount at
//! its position, rising, popping and fading over seconds of game time.

use std::collections::VecDeque;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::{Time, World, WorldLabel, WorldLabels};

use crate::foundation::{finite_color, optional_positive, positive};

/// [`CuesPlugin`](crate::CuesPlugin)'s system that ages and draws the
/// labels, in the present phase.
pub const LABEL_TICK: &str = "label_tick";

/// How a cue's `label` action draws its amount: a manifest font face, a
/// size in UI units, a linear colour, how many seconds of game time it
/// lasts, how far it rises over them in UI units, and the scale it pops
/// from to 1 over its first fifth; it fades over its second half. The
/// label's text is the event's amount, whole as `3`, else to one decimal
/// as `1.5`.
///
/// `font` names a manifest face: world labels resolve face IDs only, and
/// the load does not check it, since the fonts load with the window.
///
/// Data form, inside a `label` action:
/// `{ "font": "sans", "size": 16, "color": [1, 1, 1, 1], "seconds": 0.8, "rise": 32, "pop": 1.5 }`,
/// all but `font` optional with those defaults; a label that names any
/// field names `font`. A `damaged_part` cue's `{ "label": {} }` names none
/// and takes its part's type's style, a `damage_types` entry's
/// `label_style` ([`CueAction::type_label`](crate::CueAction::type_label)).
///
/// # Examples
///
/// ```
/// use tungsten_kit::{CueAction, LabelStyle};
///
/// let gold = LabelStyle::new("sans")
///     .with_size(20.0)
///     .with_color([1.0, 0.8, 0.2, 1.0])
///     .with_seconds(1.2);
/// let action = CueAction::label(gold);
/// assert_eq!(action.name(), "label");
/// let data = serde_json::to_value(&action).unwrap();
/// assert_eq!(data["label"]["font"], "sans");
/// assert_eq!(data["label"]["rise"], 32.0);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LabelStyleDef", into = "LabelStyleDef")]
pub struct LabelStyle {
    font: String,
    size: f32,
    color: [f32; 4],
    seconds: f32,
    rise: f32,
    pop: f32,
}

/// A label style as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LabelStyleDef {
    font: String,
    #[serde(default = "default_size", deserialize_with = "positive")]
    size: f32,
    #[serde(default = "default_color", deserialize_with = "finite_color")]
    color: [f32; 4],
    #[serde(default = "default_seconds", deserialize_with = "positive")]
    seconds: f32,
    #[serde(default = "default_rise", deserialize_with = "finite")]
    rise: f32,
    #[serde(default = "default_pop", deserialize_with = "positive")]
    pop: f32,
}

fn default_size() -> f32 {
    16.0
}

fn default_color() -> [f32; 4] {
    [1.0; 4]
}

fn default_seconds() -> f32 {
    0.8
}

fn default_rise() -> f32 {
    32.0
}

fn default_pop() -> f32 {
    1.5
}

/// A finite number, for a field's `deserialize_with`.
fn finite<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    use serde::de::Error as _;
    let value = f32::deserialize(deserializer)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(D::Error::custom(format!("must be finite (got {value})")))
    }
}

impl TryFrom<LabelStyleDef> for LabelStyle {
    type Error = String;

    fn try_from(def: LabelStyleDef) -> Result<Self, Self::Error> {
        if def.font.is_empty() {
            return Err("font: must name a manifest font face".to_string());
        }
        Ok(Self {
            font: def.font,
            size: def.size,
            color: def.color,
            seconds: def.seconds,
            rise: def.rise,
            pop: def.pop,
        })
    }
}

impl From<LabelStyle> for LabelStyleDef {
    fn from(style: LabelStyle) -> Self {
        Self {
            font: style.font,
            size: style.size,
            color: style.color,
            seconds: style.seconds,
            rise: style.rise,
            pop: style.pop,
        }
    }
}

/// A `label` action as data writes it: `{}`, which floats a `damaged_part`
/// occurrence's amount in its type's style, or a style that names its
/// font, every other field defaulted.
#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "LabelFields", into = "LabelFields")]
pub(crate) struct LabelDef(pub(crate) Option<LabelStyle>);

/// A `label` action's fields before the font rule, each optional and
/// checked.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LabelFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font: Option<String>,
    #[serde(
        default,
        deserialize_with = "optional_positive",
        skip_serializing_if = "Option::is_none"
    )]
    size: Option<f32>,
    #[serde(
        default,
        deserialize_with = "optional_color",
        skip_serializing_if = "Option::is_none"
    )]
    color: Option<[f32; 4]>,
    #[serde(
        default,
        deserialize_with = "optional_positive",
        skip_serializing_if = "Option::is_none"
    )]
    seconds: Option<f32>,
    #[serde(
        default,
        deserialize_with = "optional_finite",
        skip_serializing_if = "Option::is_none"
    )]
    rise: Option<f32>,
    #[serde(
        default,
        deserialize_with = "optional_positive",
        skip_serializing_if = "Option::is_none"
    )]
    pop: Option<f32>,
}

/// An optional finite number, for a field's `deserialize_with`.
fn optional_finite<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f32>, D::Error> {
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct Checked(#[serde(deserialize_with = "finite")] f32);
    Ok(Option::<Checked>::deserialize(deserializer)?.map(|checked| checked.0))
}

/// An optional finite colour, for a field's `deserialize_with`.
fn optional_color<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<[f32; 4]>, D::Error> {
    #[derive(Deserialize)]
    #[serde(transparent)]
    struct Checked(#[serde(deserialize_with = "finite_color")] [f32; 4]);
    Ok(Option::<Checked>::deserialize(deserializer)?.map(|checked| checked.0))
}

impl TryFrom<LabelFields> for LabelDef {
    type Error = String;

    fn try_from(fields: LabelFields) -> Result<Self, Self::Error> {
        let LabelFields {
            font,
            size,
            color,
            seconds,
            rise,
            pop,
        } = fields;
        let Some(font) = font else {
            let named = size.is_some()
                || color.is_some()
                || seconds.is_some()
                || rise.is_some()
                || pop.is_some();
            return if named {
                Err("font: a label that names any field names its font".to_string())
            } else {
                Ok(Self(None))
            };
        };
        let style = LabelStyle::try_from(LabelStyleDef {
            font,
            size: size.unwrap_or_else(default_size),
            color: color.unwrap_or_else(default_color),
            seconds: seconds.unwrap_or_else(default_seconds),
            rise: rise.unwrap_or_else(default_rise),
            pop: pop.unwrap_or_else(default_pop),
        })?;
        Ok(Self(Some(style)))
    }
}

impl From<LabelDef> for LabelFields {
    fn from(def: LabelDef) -> Self {
        match def.0 {
            None => Self::default(),
            Some(style) => Self {
                font: Some(style.font),
                size: Some(style.size),
                color: Some(style.color),
                seconds: Some(style.seconds),
                rise: Some(style.rise),
                pop: Some(style.pop),
            },
        }
    }
}

impl LabelStyle {
    /// A white label in the manifest face `font`, 16 UI units, lasting 0.8
    /// seconds, rising 32 UI units and popping from 1.5.
    ///
    /// # Panics
    ///
    /// When `font` is empty.
    #[must_use]
    #[track_caller]
    pub fn new(font: impl Into<String>) -> Self {
        let font = font.into();
        assert!(
            !font.is_empty(),
            "LabelStyle::new: font must name a manifest font face"
        );
        Self {
            font,
            size: default_size(),
            color: default_color(),
            seconds: default_seconds(),
            rise: default_rise(),
            pop: default_pop(),
        }
    }

    /// The style at `ui_units` tall.
    ///
    /// # Panics
    ///
    /// When `ui_units` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_size(mut self, ui_units: f32) -> Self {
        assert!(
            ui_units.is_finite() && ui_units > 0.0,
            "LabelStyle::with_size: must be finite and > 0 (got {ui_units})"
        );
        self.size = ui_units;
        self
    }

    /// The style in `linear` RGBA, encoded to sRGB when drawn.
    ///
    /// # Panics
    ///
    /// When a channel is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_color(mut self, linear: [f32; 4]) -> Self {
        assert!(
            linear.iter().all(|channel| channel.is_finite()),
            "LabelStyle::with_color: must be finite (got {linear:?})"
        );
        self.color = linear;
        self
    }

    /// The style lasting `s` seconds of game time.
    ///
    /// # Panics
    ///
    /// When `s` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_seconds(mut self, s: f32) -> Self {
        assert!(
            s.is_finite() && s > 0.0,
            "LabelStyle::with_seconds: must be finite and > 0 (got {s})"
        );
        self.seconds = s;
        self
    }

    /// The style rising `ui_units` over its seconds; a negative rise sinks.
    ///
    /// # Panics
    ///
    /// When `ui_units` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with_rise(mut self, ui_units: f32) -> Self {
        assert!(
            ui_units.is_finite(),
            "LabelStyle::with_rise: must be finite (got {ui_units})"
        );
        self.rise = ui_units;
        self
    }

    /// The style popping from `scale` to 1 over its first fifth.
    ///
    /// # Panics
    ///
    /// When `scale` is not finite and above 0.
    #[must_use]
    #[track_caller]
    pub fn with_pop(mut self, scale: f32) -> Self {
        assert!(
            scale.is_finite() && scale > 0.0,
            "LabelStyle::with_pop: must be finite and > 0 (got {scale})"
        );
        self.pop = scale;
        self
    }
}

/// The most labels alive at once; a new one past it drops the oldest.
pub(crate) const MAX_LABELS: usize = 512;

/// The share of a label's seconds its pop takes.
const POP_SHARE: f32 = 0.2;
/// The share of a label's seconds after which it fades.
const FADE_FROM: f32 = 0.5;

/// The labels the cues and [`float_label`] floated, a World resource
/// [`CuesPlugin`](crate::CuesPlugin) inserts: at most [`MAX_LABELS`],
/// oldest first.
#[derive(Debug, Default)]
pub(crate) struct LiveLabels {
    labels: VecDeque<Live>,
}

/// One live label.
#[derive(Debug)]
struct Live {
    text: String,
    anchor: Vec2,
    style: LabelStyle,
    /// Game seconds it has lived.
    age: f32,
    /// Its first frame draws it at age 0.
    fresh: bool,
}

impl LiveLabels {
    /// How many labels are alive.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.labels.len()
    }
}

/// `amount` as a label shows it: whole as `3`, else to one decimal.
pub(crate) fn amount_text(amount: f32) -> String {
    if amount.fract() == 0.0 && amount.abs() < 1e15 {
        format!("{amount:.0}")
    } else {
        format!("{amount:.1}")
    }
}

/// A linear channel in `[0, 1]`, encoded to an sRGB byte.
pub(crate) fn srgb_byte(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// Floats `amount` at `anchor`, a world point, in `style`, as a cue's
/// `label` action does: for a number a game's own code shapes (a crit's
/// size, ticks summed over a window). The text is the amount, whole as
/// `3`, else to one decimal. Past 512 live labels the oldest goes; a World
/// without [`CuesPlugin`](crate::CuesPlugin)'s labels takes none, and a
/// non-finite amount or anchor floats nothing.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World, WorldLabels};
/// use tungsten_kit::{CuesPlugin, LabelStyle, float_label};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// world.insert_resource(WorldLabels::default());
/// let mut schedule = Schedule::new();
/// CuesPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// // Three burn ticks a game summed, floated once in gold.
/// let gold = LabelStyle::new("sans").with_size(24.0).with_color([1.0, 0.8, 0.1, 1.0]);
/// float_label(&mut world, &gold, Vec2::new(40.0, 12.0), 3.0 + 3.0 + 1.5);
/// float_label(&mut world, &gold, Vec2::new(40.0, 12.0), f32::NAN);
/// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
/// schedule.run_stage(Stage::PostUpdate, &mut world, |_, _| {});
/// let drawn: Vec<_> = world.resource_mut::<WorldLabels>().drain().collect();
/// assert_eq!(drawn.len(), 1, "a NaN floats nothing");
/// assert_eq!((drawn[0].text.as_str(), drawn[0].anchor), ("7.5", Vec2::new(40.0, 12.0)));
/// ```
pub fn float_label(world: &mut World, style: &LabelStyle, anchor: Vec2, amount: f32) {
    if !(amount.is_finite() && anchor.is_finite()) {
        return;
    }
    let Some(live) = world.get_resource_mut::<LiveLabels>() else {
        return;
    };
    if live.labels.len() == MAX_LABELS {
        live.labels.pop_front();
    }
    live.labels.push_back(Live {
        text: amount_text(amount),
        anchor,
        style: style.clone(),
        age: 0.0,
        fresh: true,
    });
}

/// What a label of `style` draws at `t`, its age over its seconds: its
/// offset in UI units (rising is up, so negative y), its scale and its
/// alpha.
pub(crate) fn shape_at(style: &LabelStyle, t: f32) -> (Vec2, f32, f32) {
    let t = t.clamp(0.0, 1.0);
    let offset = Vec2::new(0.0, -style.rise * t);
    let scale = if t < POP_SHARE {
        style.pop + (1.0 - style.pop) * (t / POP_SHARE)
    } else {
        1.0
    };
    let alpha = if t <= FADE_FROM {
        1.0
    } else {
        1.0 - (t - FADE_FROM) / (1.0 - FADE_FROM)
    };
    (offset, scale, alpha)
}

/// Ages each live label by the frame's game time, held while paused,
/// drops those past their seconds, and pushes the rest to the frame's
/// `WorldLabels`; a fresh label draws at age 0.
pub(crate) fn label_tick(world: &mut World) {
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let Some(mut live) = world.remove_resource::<LiveLabels>() else {
        return;
    };
    for label in &mut live.labels {
        if !std::mem::take(&mut label.fresh) {
            label.age += dt;
        }
    }
    live.labels.retain(|label| label.age < label.style.seconds);
    if let Some(labels) = world.get_resource_mut::<WorldLabels>() {
        for label in &live.labels {
            let style = &label.style;
            let (offset, scale, alpha) = shape_at(style, label.age / style.seconds);
            let [r, g, b, a] = style.color;
            let color = [
                srgb_byte(r),
                srgb_byte(g),
                srgb_byte(b),
                ((a * alpha).clamp(0.0, 1.0) * 255.0).round() as u8,
            ];
            labels.push(
                WorldLabel::new(
                    label.text.clone(),
                    label.anchor,
                    style.font.clone(),
                    style.size,
                )
                .with_offset(offset)
                .with_color(color)
                .with_scale(scale),
            );
        }
    }
    world.insert_resource(live);
}

#[cfg(test)]
#[path = "../tests/labels.rs"]
mod tests;
