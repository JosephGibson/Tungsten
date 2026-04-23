//! D-032 tilemaps: Tiled `.tmj`, sprite IDs in `"sprite_id"` properties.
//!
//! D-007: tilemaps reference `AssetRegistry` sprite IDs, never own atlases.

use glam::Vec2;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct TiledMap {
    tilewidth: u32,
    tileheight: u32,
    width: u32,
    height: u32,
    tilesets: Vec<TiledTileset>,
    layers: Vec<TiledLayer>,
}

#[derive(Deserialize)]
struct TiledTileset {
    firstgid: u32,
    #[serde(default)]
    tiles: Vec<TiledTile>,
}

#[derive(Deserialize, Clone)]
struct TiledTile {
    id: u32,
    #[serde(default)]
    properties: Vec<TiledProperty>,
}

#[derive(Deserialize)]
struct TiledLayer {
    #[serde(rename = "type")]
    layer_type: String,
    name: String,
    #[serde(default)]
    data: Option<Vec<u32>>,
    #[serde(default)]
    properties: Vec<TiledProperty>,
}

#[derive(Deserialize, Clone)]
struct TiledProperty {
    name: String,
    value: serde_json::Value,
}

/// Index into `TilemapData::tileset`; negative means empty.
pub type TileIndex = i32;

/// Empty tile sentinel.
pub const EMPTY_TILE: TileIndex = -1;

/// Tile layer purpose.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerKind {
    #[default]
    Render,
    Collision,
}

/// Row-major tilemap layer.
#[derive(Debug, Clone, Deserialize)]
pub struct TilemapLayer {
    pub name: String,
    #[serde(default)]
    pub kind: LayerKind,
    pub tiles: Vec<TileIndex>,
}

/// Parsed tilemap data.
#[derive(Debug, Clone, Deserialize)]
pub struct TilemapData {
    pub tile_width: u32,
    pub tile_height: u32,
    pub width: u32,
    pub height: u32,
    /// Sprite IDs registered in `AssetRegistry`.
    pub tileset: Vec<String>,
    pub layers: Vec<TilemapLayer>,
}

impl TilemapData {
    /// Load and validate Tiled `.tmj`; sprite existence checked by asset loader.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read tilemap '{}': {}", path.display(), e))?;

        let tiled: TiledMap = serde_json::from_str(&contents)
            .map_err(|e| anyhow::anyhow!("Invalid Tiled .tmj '{}': {}", path.display(), e))?;

        // Single embedded tileset; sprite IDs come from tile properties.
        let ts = tiled
            .tilesets
            .first()
            .ok_or_else(|| anyhow::anyhow!("Tilemap '{}': no tilesets defined", path.display()))?;
        let firstgid = ts.firstgid;

        let mut sorted_tiles = ts.tiles.clone();
        sorted_tiles.sort_by_key(|t| t.id);

        let mut tileset: Vec<String> = Vec::with_capacity(sorted_tiles.len());
        for tile in &sorted_tiles {
            let sprite_id = tile
                .properties
                .iter()
                .find(|p| p.name == "sprite_id")
                .and_then(|p| p.value.as_str())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Tilemap '{}': tile id={} has no 'sprite_id' property",
                        path.display(),
                        tile.id
                    )
                })?;
            tileset.push(sprite_id.to_owned());
        }

        let mut layers: Vec<TilemapLayer> = Vec::new();
        for tl in &tiled.layers {
            if tl.layer_type != "tilelayer" {
                continue;
            }
            let data = tl.data.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Tilemap '{}': tilelayer '{}' has no data array",
                    path.display(),
                    tl.name
                )
            })?;

            let kind = tl
                .properties
                .iter()
                .find(|p| p.name == "kind")
                .and_then(|p| p.value.as_str())
                .map_or(LayerKind::Render, |s| match s {
                    "collision" => LayerKind::Collision,
                    _ => LayerKind::Render,
                });

            let tiles: Vec<TileIndex> = data
                .iter()
                .map(|&gid| {
                    if gid == 0 {
                        EMPTY_TILE
                    } else {
                        (gid - firstgid) as TileIndex
                    }
                })
                .collect();

            layers.push(TilemapLayer {
                name: tl.name.clone(),
                kind,
                tiles,
            });
        }

        let data = TilemapData {
            tile_width: tiled.tilewidth,
            tile_height: tiled.tileheight,
            width: tiled.width,
            height: tiled.height,
            tileset,
            layers,
        };

        data.validate()
            .map_err(|e| anyhow::anyhow!("Tilemap '{}': {}", path.display(), e))?;
        Ok(data)
    }

    fn validate(&self) -> Result<(), String> {
        if self.tile_width == 0 || self.tile_height == 0 {
            return Err(format!(
                "tile dimensions must be non-zero (got {}x{})",
                self.tile_width, self.tile_height
            ));
        }
        if self.width == 0 || self.height == 0 {
            return Err(format!(
                "map dimensions must be non-zero (got {}x{})",
                self.width, self.height
            ));
        }
        let cells = (self.width as usize) * (self.height as usize);
        let tileset_len = self.tileset.len() as i32;
        for (i, layer) in self.layers.iter().enumerate() {
            if layer.tiles.len() != cells {
                return Err(format!(
                    "layer {} ('{}') has {} tiles but map is {}x{} ({} cells)",
                    i,
                    layer.name,
                    layer.tiles.len(),
                    self.width,
                    self.height,
                    cells,
                ));
            }
            for (k, &t) in layer.tiles.iter().enumerate() {
                if t >= tileset_len {
                    return Err(format!(
                        "layer {} ('{}') tile[{}] = {} is out of range (tileset has {} entries)",
                        i, layer.name, k, t, tileset_len,
                    ));
                }
            }
        }
        Ok(())
    }

    /// Tile index at `(col, row)`.
    #[must_use]
    pub fn tile_at(&self, layer: usize, col: u32, row: u32) -> Option<TileIndex> {
        let layer = self.layers.get(layer)?;
        if col >= self.width || row >= self.height {
            return None;
        }
        let idx = (row as usize) * (self.width as usize) + (col as usize);
        layer.tiles.get(idx).copied()
    }

    /// Map pixel size excluding instance origin.
    #[must_use]
    pub fn pixel_size(&self) -> Vec2 {
        Vec2::new(
            (self.width * self.tile_width) as f32,
            (self.height * self.tile_height) as f32,
        )
    }
}

/// Loaded tilemap registry resource.
#[derive(Debug, Default, Clone)]
pub struct TilemapRegistry {
    maps: HashMap<String, TilemapData>,
    path_to_id: HashMap<PathBuf, String>,
}

impl TilemapRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: String, data: TilemapData) {
        self.maps.insert(id, data);
    }

    /// Insert with source-path reverse lookup.
    pub fn insert_with_path(&mut self, id: String, data: TilemapData, path: PathBuf) {
        self.path_to_id.insert(path, id.clone());
        self.maps.insert(id, data);
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&TilemapData> {
        self.maps.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &TilemapData)> {
        self.maps.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.maps.keys().map(String::as_str)
    }

    #[must_use]
    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(String::as_str)
    }
}

/// Placed tilemap instance component.
#[derive(Debug, Clone)]
pub struct TilemapInstance {
    pub id: String,
    /// Top-left world pixel position.
    pub origin: Vec2,
}

impl TilemapInstance {
    pub fn new(id: impl Into<String>, origin: Vec2) -> Self {
        Self {
            id: id.into(),
            origin,
        }
    }
}

#[cfg(test)]
#[path = "../tests/assets/tilemap.rs"]
mod tests;
