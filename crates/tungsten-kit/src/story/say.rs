//! Barks (redo 8.1, P8-1): a line said over an entity's head, the `say` cue
//! action's, which a talk need not open a box for.

use glam::Vec2;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};
use tungsten_core::text::TextOutline;
use tungsten_core::{Entity, Plugin, Schedule, Time, World, WorldLabel, WorldLabels, save, system};

use crate::foundation::phases::{Phase, ensure_phases};
use crate::motion::drive::centre_of;
use crate::present::labels::srgb_byte;

/// [`SayPlugin`]'s system, in the present phase.
pub const SAY_TICK: &str = "say_tick";

/// The most live barks; past it the oldest goes.
pub(crate) const MAX_BARKS: usize = 64;
/// The seconds a bark fades over at its end.
const FADE: f32 = 0.2;
/// A bark's outline: near black, 1 UI unit, as an interactor's prompt's.
const OUTLINE: [u8; 4] = [12, 12, 16, 255];
/// The restore hook's name.
const RESTORE_HOOK: &str = "kit.say";

/// How a bark shows: its text, its seconds and its face, size, linear
/// colour and shift in UI units off its entity's centre.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SayStyle {
    pub(crate) text: String,
    pub(crate) seconds: f32,
    pub(crate) font: String,
    pub(crate) size: f32,
    pub(crate) color: [f32; 4],
    pub(crate) offset: Vec2,
}

impl SayStyle {
    /// `text` in the defaults: 2.5 s, white `engine_mono` at 14 UI units, 16
    /// units above the centre.
    pub(crate) fn new(text: String) -> Self {
        Self {
            text,
            seconds: 2.5,
            font: "engine_mono".to_owned(),
            size: 14.0,
            color: [1.0, 1.0, 1.0, 1.0],
            offset: Vec2::new(0.0, -16.0),
        }
    }
}

/// The most characters a bark's text holds.
const MAX_TEXT: usize = 2000;

/// A `say` action as data writes it: its text alone, or the object of
/// `text`, `seconds`, `font`, `size`, `color`, `offset` and `target`;
/// written back as the text while every other field is its default.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SayDef {
    pub(crate) style: SayStyle,
    /// Whether the event's other entity says it, not the subject.
    pub(crate) other: bool,
}

/// A `say` action's object form.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SayFields {
    text: String,
    #[serde(default = "default_seconds")]
    seconds: f32,
    #[serde(default = "default_font")]
    font: String,
    #[serde(default = "default_size")]
    size: f32,
    #[serde(default = "default_color")]
    color: [f32; 4],
    #[serde(default = "default_offset")]
    offset: [f32; 2],
    #[serde(default)]
    target: SayTarget,
}

/// Who says a `say` action's text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SayTarget {
    #[default]
    #[serde(rename = "self")]
    Subject,
    Other,
}

fn default_seconds() -> f32 {
    2.5
}

fn default_font() -> String {
    "engine_mono".to_owned()
}

fn default_size() -> f32 {
    14.0
}

fn default_color() -> [f32; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn default_offset() -> [f32; 2] {
    [0.0, -16.0]
}

impl TryFrom<SayFields> for SayDef {
    type Error = String;

    fn try_from(fields: SayFields) -> Result<Self, Self::Error> {
        let count = fields.text.chars().count();
        if count == 0 || count > MAX_TEXT {
            return Err(format!("text: 1 to {MAX_TEXT} characters, not {count}"));
        }
        if !(fields.seconds.is_finite() && fields.seconds > 0.0) {
            return Err(format!(
                "seconds: {} is not finite and above 0",
                fields.seconds
            ));
        }
        if !(fields.size.is_finite() && fields.size > 0.0) {
            return Err(format!("size: {} is not finite and above 0", fields.size));
        }
        if fields
            .color
            .iter()
            .any(|channel| !(0.0..=1.0).contains(channel))
        {
            return Err(format!(
                "color: {:?} has a channel outside 0 to 1",
                fields.color
            ));
        }
        if fields.offset.iter().any(|v| !v.is_finite()) {
            return Err("offset: not finite".into());
        }
        Ok(Self {
            style: SayStyle {
                text: fields.text,
                seconds: fields.seconds,
                font: fields.font,
                size: fields.size,
                color: fields.color,
                offset: Vec2::from(fields.offset),
            },
            other: fields.target == SayTarget::Other,
        })
    }
}

impl From<&SayDef> for SayFields {
    fn from(def: &SayDef) -> Self {
        Self {
            text: def.style.text.clone(),
            seconds: def.style.seconds,
            font: def.style.font.clone(),
            size: def.style.size,
            color: def.style.color,
            offset: def.style.offset.into(),
            target: if def.other {
                SayTarget::Other
            } else {
                SayTarget::Subject
            },
        }
    }
}

impl Serialize for SayDef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let plain = SayStyle::new(self.style.text.clone());
        if self.style == plain && !self.other {
            self.style.text.serialize(serializer)
        } else {
            SayFields::from(self).serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for SayDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        let fields = match value.as_str() {
            Some(text) => SayFields {
                text: text.to_owned(),
                seconds: default_seconds(),
                font: default_font(),
                size: default_size(),
                color: default_color(),
                offset: default_offset(),
                target: SayTarget::Subject,
            },
            None => SayFields::deserialize(value).map_err(de::Error::custom)?,
        };
        SayDef::try_from(fields).map_err(de::Error::custom)
    }
}

/// The `say_action` round trip, through the object form so every described
/// field reads back.
pub(crate) fn say_round_trip(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let def: SayDef = serde_json::from_value(value).map_err(|e| e.to_string())?;
    serde_json::to_value(SayFields::from(&def)).map_err(|e| e.to_string())
}

/// One live bark.
#[derive(Debug, Clone)]
struct Bark {
    entity: Entity,
    style: SayStyle,
    age: f32,
    /// Drawn at age 0 on its first frame.
    fresh: bool,
}

/// The live barks, a World resource the first `say` inserts: at most one
/// an entity, a new one replacing its last, and at most 64, the oldest
/// dropped. [`SayPlugin`]'s system draws each as an outlined world label
/// over its entity's centre, fading over its last 0.2 s, aged by the game
/// clock so a bark holds under a pause; one whose entity is gone goes. A
/// restore clears them.
#[derive(Debug, Default)]
pub struct Barks {
    live: Vec<Bark>,
}

impl Barks {
    /// The text `entity` says now.
    #[must_use]
    pub fn text_of(&self, entity: Entity) -> Option<&str> {
        self.live
            .iter()
            .find(|bark| bark.entity == entity)
            .map(|bark| bark.style.text.as_str())
    }

    /// How many barks live.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Whether none lives.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }
}

/// `entity` says `style`, replacing its last bark.
pub(crate) fn bark(world: &mut World, entity: Entity, style: &SayStyle) {
    if !world.has_resource::<Barks>() {
        world.insert_resource(Barks::default());
    }
    let barks = world.resource_mut::<Barks>();
    barks.live.retain(|bark| bark.entity != entity);
    if barks.live.len() == MAX_BARKS {
        barks.live.remove(0);
    }
    barks.live.push(Bark {
        entity,
        style: style.clone(),
        age: 0.0,
        fresh: true,
    });
}

/// Registers [`SAY_TICK`] in the present phase, which does nothing until a
/// `say` has inserted [`Barks`], and a restore hook that clears them.
#[derive(Debug, Default, Clone, Copy)]
pub struct SayPlugin;

impl Plugin for SayPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        save::on_restore(world, RESTORE_HOOK, clear_barks);
        schedule.add(
            Phase::Present.stage(),
            Phase::Present.join(system(SAY_TICK, say_tick)),
        );
    }
}

/// The restore hook: a restore clears the barks.
fn clear_barks(world: &mut World) {
    if let Some(barks) = world.get_resource_mut::<Barks>() {
        barks.live.clear();
    }
}

/// Ages each bark by the frame's game time, drops those past their seconds
/// or whose entity is gone, and pushes the rest to the frame's
/// `WorldLabels`.
fn say_tick(world: &mut World) {
    if !world.has_resource::<Barks>() {
        return;
    }
    let dt = world.get_resource::<Time>().map_or(0.0, Time::delta);
    let barks = world.resource_mut::<Barks>();
    if barks.live.is_empty() {
        return;
    }
    for bark in &mut barks.live {
        if !std::mem::take(&mut bark.fresh) {
            bark.age += dt;
        }
    }
    barks.live.retain(|bark| bark.age < bark.style.seconds);
    let mut live = std::mem::take(&mut world.resource_mut::<Barks>().live);
    live.retain(|bark| world.is_alive(bark.entity));
    if world.has_resource::<WorldLabels>() {
        for bark in &live {
            let Some(anchor) = centre_of(world, bark.entity) else {
                continue;
            };
            let style = &bark.style;
            let alpha = ((style.seconds - bark.age) / FADE).clamp(0.0, 1.0);
            let [r, g, b, a] = style.color;
            let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            let [or, og, ob, oa] = OUTLINE;
            let label = WorldLabel::new(style.text.clone(), anchor, style.font.clone(), style.size)
                .with_offset(style.offset)
                .with_pivot(Vec2::new(0.5, 1.0))
                .with_color([srgb_byte(r), srgb_byte(g), srgb_byte(b), byte(a * alpha)])
                .with_outline(TextOutline::new(
                    [or, og, ob, byte(f32::from(oa) / 255.0 * alpha)],
                    1.0,
                ));
            world.resource_mut::<WorldLabels>().push(label);
        }
    }
    world.resource_mut::<Barks>().live = live;
}

#[cfg(test)]
#[path = "../tests/say.rs"]
mod tests;
