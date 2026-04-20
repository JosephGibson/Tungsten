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
mod tests {
    use super::*;

    const MINIMAL: &str = r#"{
        "entities": [
            {
                "transform": { "position": [10.0, -4.0], "rotation": 0.5, "scale": [2.0, 3.0] },
                "sprite": { "asset_id": "hero", "color": [10, 20, 30, 255], "z_order": 5 },
                "visible": false,
                "tag": "player"
            }
        ]
    }"#;

    #[test]
    fn load_parses_minimal_fixture() {
        let data: SceneData = serde_json::from_str(MINIMAL).expect("parse minimal");
        assert_eq!(data.entities.len(), 1);
        let entry = &data.entities[0];
        assert_eq!(entry.transform.position, [10.0, -4.0]);
        assert_eq!(entry.transform.rotation, 0.5);
        assert_eq!(entry.transform.scale, [2.0, 3.0]);
        let sprite = entry.sprite.as_ref().expect("sprite present");
        assert_eq!(sprite.asset_id, "hero");
        assert_eq!(sprite.color, [10, 20, 30, 255]);
        assert_eq!(sprite.z_order, 5);
        assert!(!entry.visible);
        assert_eq!(entry.tag.as_deref(), Some("player"));
    }

    #[test]
    fn defaults_fill_missing_fields() {
        let src = r#"{
            "entities": [
                {
                    "transform": { "position": [0.0, 0.0] },
                    "sprite": { "asset_id": "s" }
                }
            ]
        }"#;
        let data: SceneData = serde_json::from_str(src).expect("parse defaults");
        let entry = &data.entities[0];
        assert_eq!(entry.transform.rotation, 0.0);
        assert_eq!(entry.transform.scale, [1.0, 1.0]);
        let sprite = entry.sprite.as_ref().unwrap();
        assert_eq!(sprite.color, [255, 255, 255, 255]);
        assert_eq!(sprite.z_order, 0);
        assert!(entry.visible);
        assert!(entry.tag.is_none());
    }

    #[test]
    fn empty_entities_list_is_valid() {
        let data: SceneData = serde_json::from_str(r#"{ "entities": [] }"#).unwrap();
        assert!(data.entities.is_empty());
    }

    #[test]
    fn round_trip_preserves_fields() {
        let data: SceneData = serde_json::from_str(MINIMAL).unwrap();
        let encoded = serde_json::to_string(&data).unwrap();
        let reparsed: SceneData = serde_json::from_str(&encoded).unwrap();
        assert_eq!(reparsed.entities.len(), data.entities.len());
        assert_eq!(
            reparsed.entities[0].transform.position,
            data.entities[0].transform.position
        );
    }
}
