//! Tilemap data: layered, sprite-ID-referenced, JSON-driven.
//!
//! Parses Tiled's standard `.tmj` JSON map format (D-032). The on-disk
//! schema is Tiled-compatible (tilewidth/tileheight, tilesets[], layers[]
//! with GID data arrays) so maps can be authored in the Tiled editor
//! directly. Tungsten sprite IDs are stored in per-tile custom properties
//! (`"sprite_id"`) rather than deriving from image paths, keeping the
//! manifest-driven invariant intact.
//!
//! Tilemaps do **not** own their own atlas. Each entry in the `tileset`
//! array is a sprite ID that already lives in `AssetRegistry`, and the
//! renderer batches tiles per-texture the same way it batches sprites.
//! This keeps the core/render seam clean (D-007) and means hot reload
//! of a tile sprite's PNG is automatically reflected on the next frame.
//!
//! Only embedded image-collection tilesets are supported. External `.tsx`
//! tileset files are not parsed; reference the tileset inline in the `.tmj`.

use glam::Vec2;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Private Tiled .tmj deserialization types
// These reflect the subset of Tiled's JSON map format that Tungsten cares
// about. Unknown fields are ignored via `#[serde(deny_unknown_fields)]`
// being intentionally absent — Tiled adds many optional fields we don't need.
// ---------------------------------------------------------------------------

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
    id: u32, // 0-based local ID within this tileset
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

/// Index into a tilemap's `tileset` array. `-1` (= `EMPTY_TILE`) marks
/// an empty cell; values `>= 0` index into `TilemapData::tileset`.
pub type TileIndex = i32;

/// Sentinel value meaning "no tile here". Any negative value is treated
/// as empty on the render path, but authors should emit `-1`.
pub const EMPTY_TILE: TileIndex = -1;

/// What a layer is used for. Only `Render` is consumed by M10; `Collision`
/// is accepted and round-trips so the M10→M11 seam works without a
/// breaking JSON change when collision response lands.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerKind {
    #[default]
    Render,
    Collision,
}

/// A single layer of a tilemap. `tiles` is a flat row-major array of
/// length `width * height` where `width`/`height` come from the parent
/// `TilemapData`.
#[derive(Debug, Clone, Deserialize)]
pub struct TilemapLayer {
    pub name: String,
    #[serde(default)]
    pub kind: LayerKind,
    pub tiles: Vec<TileIndex>,
}

/// Fully parsed tilemap. Produced by `TilemapData::load`.
#[derive(Debug, Clone, Deserialize)]
pub struct TilemapData {
    pub tile_width: u32,
    pub tile_height: u32,
    pub width: u32,
    pub height: u32,
    /// Sprite IDs (must already be registered in `AssetRegistry`).
    pub tileset: Vec<String>,
    pub layers: Vec<TilemapLayer>,
}

impl TilemapData {
    /// Load and validate a Tiled `.tmj` map file. Validation failures are
    /// fatal (returned as `anyhow::Error`): bad layer length, out-of-range
    /// tile GID, zero dimensions.
    ///
    /// Expects Tiled's standard JSON map format with an embedded
    /// image-collection tileset. Sprite IDs are read from per-tile custom
    /// properties named `"sprite_id"`. Sprite-ID existence is *not* checked
    /// here — that lives in the asset loader (no `AssetRegistry` access here).
    ///
    /// Only layers with `"type": "tilelayer"` are processed. The layer kind
    /// (Render vs Collision) is read from a custom property `"kind"` on the
    /// layer; the default is `Render`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, anyhow::Error> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read tilemap '{}': {}", path.display(), e))?;

        let tiled: TiledMap = serde_json::from_str(&contents)
            .map_err(|e| anyhow::anyhow!("Invalid Tiled .tmj '{}': {}", path.display(), e))?;

        // --- Build the tileset sprite-ID array ---
        // We only support a single tileset per map for now.
        let ts = tiled
            .tilesets
            .first()
            .ok_or_else(|| anyhow::anyhow!("Tilemap '{}': no tilesets defined", path.display()))?;
        let firstgid = ts.firstgid;

        // Sort tiles by local id to build a contiguous 0-based array.
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

        // --- Build layers ---
        let mut layers: Vec<TilemapLayer> = Vec::new();
        for tl in &tiled.layers {
            if tl.layer_type != "tilelayer" {
                continue; // objectgroup, imagelayer, etc. are ignored
            }
            let data = tl.data.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Tilemap '{}': tilelayer '{}' has no data array",
                    path.display(),
                    tl.name
                )
            })?;

            // Read layer kind from custom properties.
            let kind = tl
                .properties
                .iter()
                .find(|p| p.name == "kind")
                .and_then(|p| p.value.as_str())
                .map(|s| match s {
                    "collision" => LayerKind::Collision,
                    _ => LayerKind::Render,
                })
                .unwrap_or(LayerKind::Render);

            // Convert Tiled GIDs → internal tile indices.
            // GID 0 = empty → -1; GID N = (N - firstgid) for tiles in our tileset.
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

    /// Tile index at (col, row) in the given layer, or `None` if any
    /// index is out of bounds.
    pub fn tile_at(&self, layer: usize, col: u32, row: u32) -> Option<TileIndex> {
        let layer = self.layers.get(layer)?;
        if col >= self.width || row >= self.height {
            return None;
        }
        let idx = (row as usize) * (self.width as usize) + (col as usize);
        layer.tiles.get(idx).copied()
    }

    /// World-space pixel size of the whole map (one instance of it,
    /// ignoring the `TilemapInstance::origin` offset).
    pub fn pixel_size(&self) -> Vec2 {
        Vec2::new(
            (self.width * self.tile_width) as f32,
            (self.height * self.tile_height) as f32,
        )
    }
}

/// Runtime registry of loaded tilemaps, stored as a Resource in the
/// World. Mirrors `AnimationRegistry` so the hot-reload dispatch code
/// can extend uniformly.
#[derive(Debug, Default, Clone)]
pub struct TilemapRegistry {
    maps: HashMap<String, TilemapData>,
    path_to_id: HashMap<PathBuf, String>,
}

impl TilemapRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, id: String, data: TilemapData) {
        self.maps.insert(id, data);
    }

    /// Insert with a source-path registration for hot-reload reverse lookup.
    pub fn insert_with_path(&mut self, id: String, data: TilemapData, path: PathBuf) {
        self.path_to_id.insert(path, id.clone());
        self.maps.insert(id, data);
    }

    pub fn get(&self, id: &str) -> Option<&TilemapData> {
        self.maps.get(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &TilemapData)> {
        self.maps.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.maps.keys().map(|s| s.as_str())
    }

    pub fn id_for_path(&self, path: &Path) -> Option<&str> {
        self.path_to_id.get(path).map(|s| s.as_str())
    }
}

/// ECS component: a placed instance of a tilemap at a given world origin.
/// One entity per tilemap instance. Game code spawns an entity, attaches
/// `TilemapInstance { id, origin }`, and the extract helper pulls it in.
#[derive(Debug, Clone)]
pub struct TilemapInstance {
    pub id: String,
    /// World-space pixel position of the tilemap's top-left corner.
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
mod tests {
    use super::*;
    use std::io::Write;

    fn tiny_map() -> TilemapData {
        TilemapData {
            tile_width: 16,
            tile_height: 16,
            width: 3,
            height: 2,
            tileset: vec!["grass".into(), "dirt".into()],
            layers: vec![
                TilemapLayer {
                    name: "bg".into(),
                    kind: LayerKind::Render,
                    tiles: vec![0, 0, 0, 1, 1, 1],
                },
                TilemapLayer {
                    name: "fg".into(),
                    kind: LayerKind::Render,
                    tiles: vec![-1, 0, -1, -1, -1, -1],
                },
            ],
        }
    }

    #[test]
    fn tile_at_reads_row_major() {
        let m = tiny_map();
        assert_eq!(m.tile_at(0, 0, 0), Some(0));
        assert_eq!(m.tile_at(0, 2, 0), Some(0));
        assert_eq!(m.tile_at(0, 0, 1), Some(1));
        assert_eq!(m.tile_at(1, 1, 0), Some(0));
        assert_eq!(m.tile_at(1, 0, 0), Some(-1));
    }

    #[test]
    fn tile_at_out_of_bounds_is_none() {
        let m = tiny_map();
        assert_eq!(m.tile_at(0, 3, 0), None);
        assert_eq!(m.tile_at(0, 0, 2), None);
        assert_eq!(m.tile_at(2, 0, 0), None);
    }

    #[test]
    fn pixel_size_multiplies_dimensions() {
        let m = tiny_map();
        assert_eq!(m.pixel_size(), Vec2::new(48.0, 32.0));
    }

    #[test]
    fn validate_rejects_wrong_layer_length() {
        let mut m = tiny_map();
        m.layers[0].tiles.pop();
        assert!(m.validate().is_err());
    }

    #[test]
    fn validate_rejects_out_of_range_index() {
        let mut m = tiny_map();
        m.layers[0].tiles[0] = 99;
        let err = m.validate().unwrap_err();
        assert!(err.contains("out of range"));
    }

    #[test]
    fn validate_allows_negative_indices_as_empty() {
        let mut m = tiny_map();
        m.layers[0].tiles[0] = -1;
        m.layers[0].tiles[1] = -42;
        assert!(m.validate().is_ok());
    }

    #[test]
    fn validate_rejects_zero_dims() {
        let mut m = tiny_map();
        m.width = 0;
        assert!(m.validate().is_err());
        let mut m = tiny_map();
        m.tile_width = 0;
        assert!(m.validate().is_err());
    }

    #[test]
    fn load_parses_a_real_file() {
        // Tests Tiled .tmj format parsing (D-032).
        let dir = std::env::temp_dir().join(format!(
            "tungsten_tilemap_test_{}_{}",
            std::process::id(),
            std::time::UNIX_EPOCH.elapsed().unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("demo.tmj");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(
            br#"{
          "type": "map", "version": "1.10",
          "orientation": "orthogonal", "renderorder": "right-down",
          "tilewidth": 8, "tileheight": 8,
          "width": 2, "height": 2,
          "infinite": false,
          "tilesets": [{
            "firstgid": 1, "columns": 0, "name": "test",
            "spacing": 0, "margin": 0,
            "tilewidth": 8, "tileheight": 8, "tilecount": 2,
            "tiles": [
              { "id": 0, "image": "a.png",
                "properties": [{"name": "sprite_id", "type": "string", "value": "a"}] },
              { "id": 1, "image": "b.png",
                "properties": [{"name": "sprite_id", "type": "string", "value": "b"}] }
            ]
          }],
          "layers": [
            { "id": 1, "type": "tilelayer", "name": "ground",
              "x": 0, "y": 0, "width": 2, "height": 2,
              "data": [1, 2, 2, 1] },
            { "id": 2, "type": "tilelayer", "name": "solid",
              "x": 0, "y": 0, "width": 2, "height": 2,
              "properties": [{"name": "kind", "type": "string", "value": "collision"}],
              "data": [0, 1, 0, 0] }
          ]
        }"#,
        )
        .unwrap();
        drop(f);

        let m = TilemapData::load(&path).unwrap();
        assert_eq!(m.width, 2);
        assert_eq!(m.tileset, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(m.layers.len(), 2);
        // GID 1 → index 0, GID 2 → index 1
        assert_eq!(m.layers[0].tiles, vec![0, 1, 1, 0]);
        assert_eq!(m.layers[1].kind, LayerKind::Collision);
        // GID 0 → EMPTY_TILE (-1), GID 1 → index 0
        assert_eq!(m.layers[1].tiles, vec![-1, 0, -1, -1]);
    }

    #[test]
    fn layer_kind_defaults_to_render() {
        let json = r#"{"name": "x", "tiles": []}"#;
        let layer: TilemapLayer = serde_json::from_str(json).unwrap();
        assert_eq!(layer.kind, LayerKind::Render);
    }

    #[test]
    fn registry_insert_and_lookup() {
        let mut reg = TilemapRegistry::new();
        reg.insert_with_path("demo".into(), tiny_map(), PathBuf::from("/tmp/demo.tmj"));
        assert!(reg.get("demo").is_some());
        assert_eq!(reg.id_for_path(Path::new("/tmp/demo.tmj")), Some("demo"),);
        let ids: Vec<&str> = reg.ids().collect();
        assert_eq!(ids, vec!["demo"]);
    }
}
