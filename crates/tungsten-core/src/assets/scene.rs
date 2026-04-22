//! Scene data model (M20).
//!
//! A `scene.json` file lists entities to spawn when a `GameState` becomes
//! active. Each entry reuses the M15 render components (`Transform`, `Sprite`,
//! `Visibility`, `Tag`) via a minimal JSON schema; spawn-time wiring lives in
//! the umbrella crate's `asset_loader::spawn_scene` (see `D-046`).
//!
//! This module owns only the plain data model and its JSON serde. Sprite ID
//! validation is intentionally deferred to the extract path — missing IDs
//! fall through to the sprite-extract default warning, matching
//! `TilemapInstance`.

use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
        serde_json::from_str(&contents).map_err(|source| SceneError::Parse {
            path: path.display().to_string(),
            source,
        })
    }
}

#[cfg(test)]
#[path = "../tests/assets/scene.rs"]
mod tests;
