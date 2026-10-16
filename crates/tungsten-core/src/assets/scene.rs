//! D-046 scene data model; sprite ID validation deferred to extract path.
//! D-054/D-055 scene tweens map one `Tween` per entry.
//! M48: an entry's `components` name registered components, checked at load
//! against the World's `ComponentRegistry`; unknown keys fail the parse.
//! M51: prefab files and Tiled objects share the entry's readers and its
//! position bound.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use thiserror::Error;

use crate::component_registry::ComponentRegistry;
use crate::ecs::World;
use crate::tween::{Easing, IntSlot, ScalarSlot, Tween, TweenChannel, TweenRepeat, Vec4Slot};

/// The largest position a scene transform may name on either axis: 2^24,
/// where an `f32` still holds whole pixels. Prefab spawns and Tiled objects
/// keep the same bound.
pub(crate) const MAX_POSITION: f32 = 16_777_216.0;

/// Whether both coordinates are finite and within ±[`MAX_POSITION`].
pub(crate) fn within_position_bound(x: f32, y: f32) -> bool {
    x.abs() <= MAX_POSITION && y.abs() <= MAX_POSITION
}

#[derive(Debug, Error)]
pub enum SceneError {
    #[error("failed to read scene '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("invalid scene '{path}': {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
    #[error("invalid scene '{path}': {message}")]
    Validation { path: String, message: String },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneData {
    #[serde(default)]
    pub entities: Vec<SceneEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneEntry {
    pub transform: SceneTransform,
    #[serde(default)]
    pub sprite: Option<SceneSprite>,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tweens: Vec<SceneTween>,
    /// Registered components by name: each value deserializes as the type its
    /// name was registered with and spawns after the fields above, in name
    /// order. A name given twice fails the parse.
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "strict_components"
    )]
    pub components: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneTween {
    pub duration: f32,
    #[serde(default)]
    pub easing: Easing,
    #[serde(default)]
    pub repeat: SceneTweenRepeat,
    #[serde(default)]
    pub tag: Option<String>,
    pub channels: Vec<SceneTweenChannel>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneTweenChannel {
    PositionX {
        from: f32,
        to: f32,
    },
    PositionY {
        from: f32,
        to: f32,
    },
    Rotation {
        from: f32,
        to: f32,
    },
    ScaleX {
        from: f32,
        to: f32,
    },
    ScaleY {
        from: f32,
        to: f32,
    },
    ColorR {
        from: u8,
        to: u8,
    },
    ColorG {
        from: u8,
        to: u8,
    },
    ColorB {
        from: u8,
        to: u8,
    },
    ColorA {
        from: u8,
        to: u8,
    },
    /// Drives one lane of a `UniformOverrideBlock.vec4[slot]` slot.
    UniformVec4Lane {
        slot: Vec4Slot,
        lane: u8,
        from: f32,
        to: f32,
    },
    UniformScalar {
        slot: ScalarSlot,
        from: f32,
        to: f32,
    },
    UniformInt {
        slot: IntSlot,
        from: i32,
        to: i32,
    },
}

impl From<SceneTweenChannel> for TweenChannel {
    fn from(c: SceneTweenChannel) -> Self {
        match c {
            SceneTweenChannel::PositionX { from, to } => TweenChannel::PositionX { from, to },
            SceneTweenChannel::PositionY { from, to } => TweenChannel::PositionY { from, to },
            SceneTweenChannel::Rotation { from, to } => TweenChannel::Rotation { from, to },
            SceneTweenChannel::ScaleX { from, to } => TweenChannel::ScaleX { from, to },
            SceneTweenChannel::ScaleY { from, to } => TweenChannel::ScaleY { from, to },
            SceneTweenChannel::ColorR { from, to } => TweenChannel::ColorR { from, to },
            SceneTweenChannel::ColorG { from, to } => TweenChannel::ColorG { from, to },
            SceneTweenChannel::ColorB { from, to } => TweenChannel::ColorB { from, to },
            SceneTweenChannel::ColorA { from, to } => TweenChannel::ColorA { from, to },
            SceneTweenChannel::UniformVec4Lane {
                slot,
                lane,
                from,
                to,
            } => TweenChannel::UniformVec4Lane {
                slot,
                lane,
                from,
                to,
            },
            SceneTweenChannel::UniformScalar { slot, from, to } => {
                TweenChannel::UniformScalar { slot, from, to }
            }
            SceneTweenChannel::UniformInt { slot, from, to } => {
                TweenChannel::UniformInt { slot, from, to }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneTweenRepeat {
    #[default]
    Once,
    Loop,
    PingPong,
    Times(u32),
}

impl From<SceneTweenRepeat> for TweenRepeat {
    fn from(r: SceneTweenRepeat) -> Self {
        match r {
            SceneTweenRepeat::Once => TweenRepeat::Once,
            SceneTweenRepeat::Loop => TweenRepeat::Loop,
            SceneTweenRepeat::PingPong => TweenRepeat::PingPong,
            SceneTweenRepeat::Times(n) => TweenRepeat::Times(n),
        }
    }
}

impl SceneTween {
    #[must_use]
    pub fn into_tween(&self) -> Tween {
        let mut t = Tween::new(self.duration, self.easing).with_repeat(self.repeat.into());
        for ch in &self.channels {
            t = t.with_channel((*ch).into());
        }
        if let Some(tag) = &self.tag {
            t = t.with_tag(tag.clone());
        }
        t
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.duration.is_finite() || self.duration <= 0.0 {
            return Err(format!(
                "tween duration must be finite and > 0 (got {})",
                self.duration
            ));
        }
        if self.channels.is_empty() {
            return Err("tween requires at least one channel".to_string());
        }
        for channel in &self.channels {
            if let SceneTweenChannel::UniformVec4Lane { lane, .. } = channel
                && *lane > 3
            {
                return Err(format!("uniform_vec4_lane lane must be 0..=3 (got {lane})"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneTransform {
    pub position: [f32; 2],
    #[serde(default)]
    pub rotation: f32,
    #[serde(default = "one_scale")]
    pub scale: [f32; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneSprite {
    pub asset_id: String,
    #[serde(default = "white")]
    pub color: [u8; 4],
    #[serde(default)]
    pub z_order: i32,
}

pub(crate) fn default_visible() -> bool {
    true
}

fn one_scale() -> [f32; 2] {
    [1.0, 1.0]
}

fn white() -> [u8; 4] {
    [255; 4]
}

impl SceneTransform {
    /// Every number finite, and the position within ±2^24 on both axes.
    fn validate(&self) -> Result<(), String> {
        let [x, y] = self.position;
        if !within_position_bound(x, y) {
            return Err(format!(
                "position must be finite and within ±{MAX_POSITION} (got [{x}, {y}])"
            ));
        }
        if !self.rotation.is_finite() {
            return Err(format!("rotation must be finite (got {})", self.rotation));
        }
        let [sx, sy] = self.scale;
        if !(sx.is_finite() && sy.is_finite()) {
            return Err(format!("scale must be finite (got [{sx}, {sy}])"));
        }
        Ok(())
    }
}

impl SceneData {
    /// Reads and parses `path`, then checks every entry's transform and
    /// tweens, and its components with `ComponentRegistry::check_in` against
    /// `world` (an empty registry when the World has none, so any component
    /// is unknown), so an asset ID a component names must be in `world`.
    /// The first error stops the load.
    pub fn load(path: &Path, world: &World) -> Result<Self, SceneError> {
        let contents = std::fs::read_to_string(path).map_err(|source| SceneError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let data: SceneData =
            serde_json::from_str(&contents).map_err(|source| SceneError::Parse {
                path: path.display().to_string(),
                source,
            })?;
        let invalid = |message: String| SceneError::Validation {
            path: path.display().to_string(),
            message,
        };
        let empty = ComponentRegistry::new();
        let registry = world.get_resource::<ComponentRegistry>().unwrap_or(&empty);
        for (i, entry) in data.entities.iter().enumerate() {
            entry
                .transform
                .validate()
                .map_err(|msg| invalid(format!("entities[{i}].transform: {msg}")))?;
            for (j, tween) in entry.tweens.iter().enumerate() {
                tween
                    .validate()
                    .map_err(|msg| invalid(format!("entities[{i}].tweens[{j}]: {msg}")))?;
            }
            for (name, value) in &entry.components {
                registry
                    .check_in(name, value, world)
                    .map_err(|err| invalid(format!("entities[{i}]: {err}")))?;
            }
        }
        Ok(data)
    }
}

/// `components`' reader: fails on a component named twice, and on a key
/// repeated at any depth inside a value, where a plain `Value` keeps the
/// last one and a valid repeat would mask an invalid first.
pub(crate) fn strict_components<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Value>, D::Error> {
    struct Components;

    impl<'de> Visitor<'de> for Components {
        type Value = BTreeMap<String, Value>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an object of components by name")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut components = BTreeMap::new();
            while let Some(name) = map.next_key::<String>()? {
                if components.contains_key(&name) {
                    return Err(de::Error::custom(format_args!(
                        "duplicate component `{name}`"
                    )));
                }
                let StrictValue(value) = map.next_value()?;
                components.insert(name, value);
            }
            Ok(components)
        }
    }

    deserializer.deserialize_map(Components)
}

/// A JSON value whose objects refuse a repeated key: scene and prefab
/// components' values and whole data files read through it.
pub(crate) struct StrictValue(pub(crate) Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor).map(StrictValue)
    }
}

struct StrictVisitor;

impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::from(value))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Ok(Number::from_f64(value).map_or(Value::Null, Value::Number))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        StrictValue::deserialize(deserializer).map(|StrictValue(value)| value)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut items = Vec::new();
        while let Some(StrictValue(item)) = seq.next_element()? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            if object.contains_key(&key) {
                return Err(de::Error::custom(format_args!("duplicate field `{key}`")));
            }
            let StrictValue(value) = map.next_value()?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

#[cfg(test)]
#[path = "../tests/assets/scene.rs"]
mod tests;
