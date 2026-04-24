//! D-046 scene data model; sprite ID validation deferred to extract path.
//! D-054/D-055 scene tweens map one `Tween` per entry.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::tween::{Easing, IntSlot, ScalarSlot, Tween, TweenChannel, TweenRepeat, Vec4Slot};

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
pub struct SceneData {
    #[serde(default)]
    pub entities: Vec<SceneEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SceneTransform {
    pub position: [f32; 2],
    #[serde(default)]
    pub rotation: f32,
    #[serde(default = "one_scale")]
    pub scale: [f32; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneSprite {
    pub asset_id: String,
    #[serde(default = "white")]
    pub color: [u8; 4],
    #[serde(default)]
    pub z_order: i32,
}

fn default_visible() -> bool {
    true
}

fn one_scale() -> [f32; 2] {
    [1.0, 1.0]
}

fn white() -> [u8; 4] {
    [255; 4]
}

impl SceneData {
    pub fn load(path: &Path) -> Result<Self, SceneError> {
        let contents = std::fs::read_to_string(path).map_err(|source| SceneError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let data: SceneData =
            serde_json::from_str(&contents).map_err(|source| SceneError::Parse {
                path: path.display().to_string(),
                source,
            })?;
        for (i, entry) in data.entities.iter().enumerate() {
            for (j, tween) in entry.tweens.iter().enumerate() {
                tween.validate().map_err(|msg| SceneError::Validation {
                    path: path.display().to_string(),
                    message: format!("entities[{i}].tweens[{j}]: {msg}"),
                })?;
            }
        }
        Ok(data)
    }
}

#[cfg(test)]
#[path = "../tests/assets/scene.rs"]
mod tests;
