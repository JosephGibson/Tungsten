//! D-032 tilemaps: Tiled `.tmj`, sprite IDs in `"sprite_id"` properties.
//!
//! D-007: tilemaps reference `AssetRegistry` sprite IDs, never own atlases.
//!
//! M51: object layers load as [`ObjectLayer`]s of [`MapObject`]s, whose
//! classes name prefabs (`World::spawn_map_objects`); a map built in code
//! starts from [`TilemapData::new`].
//!
//! A tileset tile's custom properties beside `sprite_id` load as its
//! [`TileProps`], and a collision layer's `layers` and `mask` as its
//! [`LayerPhysics`]; unknown properties are ignored.

use glam::Vec2;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::assets::scene::{MAX_POSITION, within_position_bound};

/// Error loading a Tiled `.tmj` tilemap.
#[derive(Debug, Error)]
pub enum TilemapError {
    #[error("Failed to read tilemap '{path}': {error}")]
    Read { path: String, error: std::io::Error },
    #[error("Invalid Tiled .tmj '{path}': {error}")]
    Parse {
        path: String,
        error: serde_json::Error,
    },
    #[error("Tilemap '{path}': no tilesets defined")]
    NoTileset { path: String },
    #[error("Tilemap '{path}': tile id={tile} has no 'sprite_id' property")]
    MissingSpriteId { path: String, tile: u32 },
    #[error("Tilemap '{path}': too many tiles")]
    TooManyTiles { path: String },
    #[error("Tilemap '{path}': duplicate tile id={tile}")]
    DuplicateTile { path: String, tile: u32 },
    #[error("Tilemap '{path}': tilelayer '{layer}' has no data array")]
    MissingLayerData { path: String, layer: String },
    #[error("Tilemap '{path}': layer '{layer}' references unsupported or unknown gid={gid}")]
    UnknownGid {
        path: String,
        layer: String,
        gid: u32,
    },
    #[error("Tilemap '{path}': {message}")]
    Invalid { path: String, message: String },
}

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
    /// Where a tile object's `x` and `y` sit on its tile; absent is
    /// `unspecified`, bottom-left on an orthogonal map.
    #[serde(default)]
    objectalignment: Option<String>,
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
    /// An object layer's objects.
    #[serde(default)]
    objects: Vec<TiledObject>,
    #[serde(default)]
    offsetx: Option<f32>,
    #[serde(default)]
    offsety: Option<f32>,
}

/// One Tiled object as written; the load checks its fields
/// (`object_layer`), so each is optional here.
#[derive(Deserialize)]
struct TiledObject {
    #[serde(default)]
    id: Option<u32>,
    #[serde(default)]
    name: Option<String>,
    /// The class in Tiled 1.8 and earlier and 1.10 and later.
    #[serde(default, rename = "type")]
    object_type: Option<String>,
    /// The class in Tiled 1.9.
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    x: Option<f32>,
    #[serde(default)]
    y: Option<f32>,
    #[serde(default)]
    width: Option<f32>,
    #[serde(default)]
    height: Option<f32>,
    /// Degrees clockwise about the object's origin.
    #[serde(default)]
    rotation: Option<f32>,
    /// Present on a tile object.
    #[serde(default)]
    gid: Option<u32>,
    #[serde(default)]
    template: Option<String>,
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

/// A tileset entry's properties beside its sprite, from its Tiled tile's
/// custom properties of the same names or
/// [`TilemapData::set_tile_props`]. The default is a plain tile: no
/// material, unbreakable, solid on every side and unstyled.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct TileProps {
    /// A name game code reads, such as `"stone"`; never empty.
    pub material: Option<String>,
    /// The damage that breaks a cell of the tile: finite and above 0, or
    /// `None` for an unbreakable tile.
    pub health: Option<f32>,
    /// The tile a broken cell becomes, [`EMPTY_TILE`] to clear it; a tile
    /// other than empty needs `health`. Tiled names it by its sprite ID, the
    /// first tile of the tileset with that sprite.
    pub broken: TileIndex,
    /// In a collision layer, solid through its top face only, from above.
    pub one_way: bool,
    /// Its auto-tile style.
    pub style: Option<String>,
    /// A style variant's open sides, 0 to 15 (1 up, 2 right, 4 down, 8
    /// left); it needs `style`, and a styled tile without one is the
    /// style's brush.
    pub mask: Option<u8>,
    /// A variant's draw weight, at least 1; a weight other than 1 needs
    /// `mask`.
    pub weight: u32,
}

/// The default [`TileProps`], which a tile without its own reads.
static PLAIN_TILE: TileProps = TileProps {
    material: None,
    health: None,
    broken: EMPTY_TILE,
    one_way: false,
    style: None,
    mask: None,
    weight: 1,
};

impl Default for TileProps {
    fn default() -> Self {
        PLAIN_TILE.clone()
    }
}

impl TileProps {
    /// The rules [`TilemapData::set_tile_props`] and the loader hold every
    /// tile's properties to, for a tileset of `tiles` entries; the message
    /// names the property.
    fn check(&self, tiles: usize) -> Result<(), String> {
        if self.material.as_deref() == Some("") {
            return Err("`material` must not be empty".to_owned());
        }
        if let Some(health) = self.health
            && !(health.is_finite() && health > 0.0)
        {
            return Err(format!(
                "`health` must be finite and above 0 (got {health})"
            ));
        }
        if self.broken >= 0 {
            if self.health.is_none() {
                return Err("`broken` needs `health`".to_owned());
            }
            if self.broken as usize >= tiles {
                return Err(format!(
                    "`broken` tile {} is past the tileset's {tiles} entries",
                    self.broken
                ));
            }
        }
        if let Some(mask) = self.mask {
            if mask > 15 {
                return Err(format!("`mask` must be 0 to 15 (got {mask})"));
            }
            if self.style.is_none() {
                return Err("`mask` needs `style`".to_owned());
            }
        }
        if self.weight == 0 {
            return Err("`weight` must be at least 1 (got 0)".to_owned());
        }
        if self.weight != 1 && self.mask.is_none() {
            return Err("`weight` needs `mask`".to_owned());
        }
        Ok(())
    }
}

/// The physics layers a collision layer's tiles are in and meet, one bit
/// per layer 0 to 31, as a `RigidBody`'s `layers` and `mask` are (`D-178`).
/// The default, as without the properties, is layer 0 meeting every layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerPhysics {
    /// The layers its tiles are in.
    pub layers: u32,
    /// The layers its tiles meet; 0 meets none.
    pub mask: u32,
}

impl LayerPhysics {
    /// Layer 0 meeting every layer: a layer without the properties.
    pub const DEFAULT: Self = Self {
        layers: 1,
        mask: u32::MAX,
    };
}

impl Default for LayerPhysics {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// What a map holds beside its public fields: each tileset entry's
/// properties and each layer's physics, an entry past either list reading
/// the defaults, so a map built in code or written in bulk has none.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TerrainProps {
    /// By tileset entry, trailing defaults left off.
    tiles: Vec<TileProps>,
    /// By layer, trailing defaults left off.
    layers: Vec<LayerPhysics>,
}

/// Parsed tilemap data: a Tiled file's ([`load`](Self::load)) or one built
/// in code ([`new`](Self::new) and its setters). The fields stay public for
/// reading and bulk writes; tile properties and layer physics are read and
/// set through their methods ([`tile_props`](Self::tile_props),
/// [`layer_physics`](Self::layer_physics)).
#[derive(Debug, Clone, Deserialize)]
#[non_exhaustive]
pub struct TilemapData {
    pub tile_width: u32,
    pub tile_height: u32,
    pub width: u32,
    pub height: u32,
    /// Sprite IDs registered in `AssetRegistry`.
    pub tileset: Vec<String>,
    pub layers: Vec<TilemapLayer>,
    /// The object layers, in file order; `Deserialize` skips them.
    #[serde(skip)]
    pub object_layers: Vec<ObjectLayer>,
    /// Tile properties and layer physics; `Deserialize` skips them.
    #[serde(skip)]
    pub(crate) terrain: TerrainProps,
}

/// A Tiled object layer: its name and its objects, in file order.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ObjectLayer {
    /// The layer's name in Tiled.
    pub name: String,
    /// The layer's objects, in file order.
    pub objects: Vec<MapObject>,
}

/// One Tiled object. `class` names the prefab it spawns
/// (`World::spawn_map_objects`); empty, the object is a marker the game
/// reads by name.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MapObject {
    /// The object's ID in Tiled, unique in its map.
    pub id: u32,
    /// The object's name, empty when Tiled gave none.
    pub name: String,
    /// Its class: Tiled's `type`, or `class` from a Tiled 1.9 file.
    pub class: String,
    /// Its top-left in map pixels, the layer's offset added; a tile
    /// object's is moved up from the bottom-left Tiled writes.
    pub position: Vec2,
    /// Its width and height in pixels, 0 for a point.
    pub size: Vec2,
    /// Its custom properties by name, each Tiled `value` as JSON: a
    /// string, int, float, bool, color or file as a scalar, an object
    /// reference as its ID, a class as an object of the members Tiled
    /// wrote.
    pub properties: BTreeMap<String, Value>,
}

impl TilemapData {
    /// An empty map of `width` × `height` cells of `tile_width` ×
    /// `tile_height` pixels: no tileset entries, layers or objects. With
    /// [`add_tile`](Self::add_tile), [`add_layer`](Self::add_layer),
    /// [`set_tile`](Self::set_tile) and [`fill`](Self::fill), a game builds
    /// its map in code and registers it in the `TilemapRegistry`:
    ///
    /// ```
    /// use tungsten_core::{LayerKind, TilemapData, TilemapRegistry, World};
    ///
    /// let mut arena = TilemapData::new(16, 16, 64, 64);
    /// let floor = arena.add_tile("arena_floor");
    /// let wall = arena.add_tile("arena_wall");
    /// let ground = arena.add_layer("ground", LayerKind::Render);
    /// let walls = arena.add_layer("walls", LayerKind::Collision);
    /// arena.fill(ground, floor);
    /// for col in 0..64 {
    ///     arena.set_tile(walls, col, 0, wall);
    ///     arena.set_tile(walls, col, 63, wall);
    /// }
    /// assert_eq!(arena.tile_at(walls, 10, 63), Some(wall));
    /// assert_eq!(arena.tile_at(walls, 10, 1), Some(tungsten_core::EMPTY_TILE));
    ///
    /// let mut world = World::new();
    /// world.insert_resource(TilemapRegistry::new());
    /// world.resource_mut::<TilemapRegistry>().insert("arena".to_string(), arena);
    /// ```
    ///
    /// # Panics
    ///
    /// When a dimension is 0 (`D-022`).
    #[must_use]
    #[track_caller]
    pub fn new(tile_width: u32, tile_height: u32, width: u32, height: u32) -> Self {
        assert!(
            tile_width > 0 && tile_height > 0 && width > 0 && height > 0,
            "TilemapData::new: dimensions must be non-zero (got {tile_width}x{tile_height} pixel tiles, {width}x{height} cells)"
        );
        Self {
            tile_width,
            tile_height,
            width,
            height,
            tileset: Vec::new(),
            layers: Vec::new(),
            object_layers: Vec::new(),
            terrain: TerrainProps::default(),
        }
    }

    /// Appends `sprite`, a sprite ID, to the tileset and returns its tile
    /// index.
    ///
    /// # Panics
    ///
    /// When the tileset already holds `TileIndex::MAX` + 1 entries, so the
    /// next index does not fit.
    #[track_caller]
    pub fn add_tile(&mut self, sprite: impl Into<String>) -> TileIndex {
        let Ok(index) = TileIndex::try_from(self.tileset.len()) else {
            panic!(
                "TilemapData::add_tile: the tileset holds {} entries, past TileIndex::MAX",
                self.tileset.len()
            );
        };
        self.tileset.push(sprite.into());
        index
    }

    /// Appends a layer named `name` with every cell [`EMPTY_TILE`] and
    /// returns its index.
    pub fn add_layer(&mut self, name: impl Into<String>, kind: LayerKind) -> usize {
        let cells = (self.width as usize) * (self.height as usize);
        self.layers.push(TilemapLayer {
            name: name.into(),
            kind,
            tiles: vec![EMPTY_TILE; cells],
        });
        self.layers.len() - 1
    }

    /// Sets the cell at `(col, row)` of `layer` to `tile`; a negative tile
    /// clears it, stored as [`EMPTY_TILE`].
    ///
    /// # Panics
    ///
    /// When `layer`, `col` or `row` is out of range, or `tile` is at or past
    /// the tileset's length.
    #[track_caller]
    pub fn set_tile(&mut self, layer: usize, col: u32, row: u32, tile: TileIndex) {
        assert!(
            col < self.width && row < self.height,
            "TilemapData::set_tile: cell ({col}, {row}) is outside the {}x{} map",
            self.width,
            self.height
        );
        let tile = self.checked_tile("set_tile", tile);
        let width = self.width as usize;
        let cells = self.layer_tiles("set_tile", layer);
        cells[(row as usize) * width + (col as usize)] = tile;
    }

    /// Sets every cell of `layer` to `tile`; a negative tile clears them.
    ///
    /// # Panics
    ///
    /// As [`set_tile`](Self::set_tile).
    #[track_caller]
    pub fn fill(&mut self, layer: usize, tile: TileIndex) {
        let tile = self.checked_tile("fill", tile);
        self.layer_tiles("fill", layer).fill(tile);
    }

    /// `tile` as a cell stores it, or a panic naming `method` when it is at
    /// or past the tileset's length.
    #[track_caller]
    fn checked_tile(&self, method: &str, tile: TileIndex) -> TileIndex {
        if tile < 0 {
            return EMPTY_TILE;
        }
        assert!(
            (tile as usize) < self.tileset.len(),
            "TilemapData::{method}: tile {tile} is past the tileset's {} entries",
            self.tileset.len()
        );
        tile
    }

    /// The cells of `layer`, or a panic naming `method` when there is no
    /// such layer.
    #[track_caller]
    fn layer_tiles(&mut self, method: &str, layer: usize) -> &mut [TileIndex] {
        let count = self.layers.len();
        match self.layers.get_mut(layer) {
            Some(layer) => &mut layer.tiles,
            None => panic!("TilemapData::{method}: layer {layer} is past the map's {count} layers"),
        }
    }

    /// The properties of tileset entry `tile`; the defaults for an entry
    /// that has none set, and for an empty or unknown tile.
    #[must_use]
    pub fn tile_props(&self, tile: TileIndex) -> &TileProps {
        usize::try_from(tile)
            .ok()
            .and_then(|tile| self.terrain.tiles.get(tile))
            .unwrap_or(&PLAIN_TILE)
    }

    /// Sets the properties of tileset entry `tile`.
    ///
    /// # Panics
    ///
    /// When `tile` is not an entry of the tileset, or `props` breaks a rule
    /// of [`TileProps`]'s fields: an empty `material`, a `health` not finite
    /// and above 0, a `broken` tile without `health` or past the tileset, a
    /// `mask` past 15 or without `style`, a `weight` of 0, or one other than
    /// 1 without `mask`.
    #[track_caller]
    pub fn set_tile_props(&mut self, tile: TileIndex, props: TileProps) {
        let count = self.tileset.len();
        assert!(
            tile >= 0,
            "TilemapData::set_tile_props: tile {tile} is empty, not a tileset entry"
        );
        let index = tile as usize;
        assert!(
            index < count,
            "TilemapData::set_tile_props: tile {tile} is past the tileset's {count} entries"
        );
        if let Err(message) = props.check(count) {
            panic!("TilemapData::set_tile_props: tile {tile}: {message}");
        }
        let tiles = &mut self.terrain.tiles;
        if index >= tiles.len() {
            tiles.resize(index + 1, TileProps::default());
        }
        tiles[index] = props;
        while tiles.last() == Some(&PLAIN_TILE) {
            tiles.pop();
        }
    }

    /// The physics layers of `layer`'s tiles; the defaults for a layer that
    /// has none set or does not exist.
    #[must_use]
    pub fn layer_physics(&self, layer: usize) -> LayerPhysics {
        self.terrain
            .layers
            .get(layer)
            .copied()
            .unwrap_or(LayerPhysics::DEFAULT)
    }

    /// Sets the physics layers of collision layer `layer`'s tiles.
    ///
    /// # Panics
    ///
    /// When `layer` is past the map's layers or is not a collision layer.
    #[track_caller]
    pub fn set_layer_physics(&mut self, layer: usize, physics: LayerPhysics) {
        let count = self.layers.len();
        match self.layers.get(layer) {
            Some(found) if found.kind == LayerKind::Collision => {}
            Some(found) => panic!(
                "TilemapData::set_layer_physics: layer {layer} ('{}') is not a collision layer",
                found.name
            ),
            None => panic!(
                "TilemapData::set_layer_physics: layer {layer} is past the map's {count} layers"
            ),
        }
        let layers = &mut self.terrain.layers;
        if layer >= layers.len() {
            layers.resize(layer + 1, LayerPhysics::DEFAULT);
        }
        layers[layer] = physics;
        while layers.last() == Some(&LayerPhysics::DEFAULT) {
            layers.pop();
        }
    }

    /// Load and validate Tiled `.tmj`; sprite existence checked by asset loader.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, TilemapError> {
        let file = path.as_ref();
        let path = file.display().to_string();
        let contents = std::fs::read_to_string(file).map_err(|error| TilemapError::Read {
            path: path.clone(),
            error,
        })?;

        let tiled: TiledMap =
            serde_json::from_str(&contents).map_err(|error| TilemapError::Parse {
                path: path.clone(),
                error,
            })?;

        // Single embedded tileset; sprite IDs come from tile properties.
        let ts = tiled
            .tilesets
            .first()
            .ok_or_else(|| TilemapError::NoTileset { path: path.clone() })?;
        let firstgid = ts.firstgid;

        let mut sorted_tiles = ts.tiles.clone();
        sorted_tiles.sort_by_key(|t| t.id);

        let mut tileset: Vec<String> = Vec::with_capacity(sorted_tiles.len());
        let mut tile_indices = HashMap::new();
        for tile in &sorted_tiles {
            let sprite_id = tile
                .properties
                .iter()
                .find(|p| p.name == "sprite_id")
                .and_then(|p| p.value.as_str())
                .ok_or_else(|| TilemapError::MissingSpriteId {
                    path: path.clone(),
                    tile: tile.id,
                })?;
            let index = TileIndex::try_from(tileset.len())
                .map_err(|_| TilemapError::TooManyTiles { path: path.clone() })?;
            if tile_indices.insert(tile.id, index).is_some() {
                return Err(TilemapError::DuplicateTile {
                    path,
                    tile: tile.id,
                });
            }
            tileset.push(sprite_id.to_owned());
        }
        let mut tile_props = Vec::with_capacity(sorted_tiles.len());
        for tile in &sorted_tiles {
            let props = tile_props_of(tile, &tileset).map_err(|message| TilemapError::Invalid {
                path: path.clone(),
                message: format!("tile id={}: {message}", tile.id),
            })?;
            tile_props.push(props);
        }
        while tile_props.last() == Some(&PLAIN_TILE) {
            tile_props.pop();
        }

        let alignment = ts.objectalignment.as_deref().unwrap_or("unspecified");
        let mut layers: Vec<TilemapLayer> = Vec::new();
        let mut layer_physics: Vec<LayerPhysics> = Vec::new();
        let mut object_layers: Vec<ObjectLayer> = Vec::new();
        for tl in &tiled.layers {
            // Skipped with its group, a collision layer would go unnoticed.
            if tl.layer_type == "group" {
                return Err(TilemapError::Invalid {
                    path,
                    message: format!(
                        "group layer '{}' is not supported; move its layers to the top level",
                        tl.name
                    ),
                });
            }
            if tl.layer_type == "objectgroup" {
                let objects =
                    object_layer(tl, alignment).map_err(|message| TilemapError::Invalid {
                        path: path.clone(),
                        message,
                    })?;
                object_layers.push(objects);
                continue;
            }
            // Image layers carry nothing a tilemap draws.
            if tl.layer_type != "tilelayer" {
                continue;
            }
            let data = tl
                .data
                .as_ref()
                .ok_or_else(|| TilemapError::MissingLayerData {
                    path: path.clone(),
                    layer: tl.name.clone(),
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
            let physics = layer_physics_of(tl, kind).map_err(|message| TilemapError::Invalid {
                path: path.clone(),
                message: format!("layer '{}': {message}", tl.name),
            })?;
            layer_physics.push(physics);

            let tiles: Vec<TileIndex> = data
                .iter()
                .map(|&gid| {
                    if gid == 0 {
                        Ok(EMPTY_TILE)
                    } else {
                        gid.checked_sub(firstgid)
                            .and_then(|id| tile_indices.get(&id).copied())
                            .ok_or_else(|| TilemapError::UnknownGid {
                                path: path.clone(),
                                layer: tl.name.clone(),
                                gid,
                            })
                    }
                })
                .collect::<Result<_, TilemapError>>()?;

            layers.push(TilemapLayer {
                name: tl.name.clone(),
                kind,
                tiles,
            });
        }

        while layer_physics.last() == Some(&LayerPhysics::DEFAULT) {
            layer_physics.pop();
        }
        let data = TilemapData {
            tile_width: tiled.tilewidth,
            tile_height: tiled.tileheight,
            width: tiled.width,
            height: tiled.height,
            tileset,
            layers,
            object_layers,
            terrain: TerrainProps {
                tiles: tile_props,
                layers: layer_physics,
            },
        };

        data.validate()
            .map_err(|message| TilemapError::Invalid { path, message })?;
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
            self.width as f32 * self.tile_width as f32,
            self.height as f32 * self.tile_height as f32,
        )
    }
}

/// A tileset tile's properties beside `sprite_id`, each given once, of its
/// type and held to [`TileProps`]'s rules; `broken` names a sprite of
/// `tileset`. The message names the property.
fn tile_props_of(tile: &TiledTile, tileset: &[String]) -> Result<TileProps, String> {
    const NAMES: [&str; 7] = [
        "material", "health", "broken", "one_way", "style", "mask", "weight",
    ];
    let mut props = TileProps::default();
    let mut seen: Vec<&str> = Vec::new();
    for property in &tile.properties {
        let (name, value) = (property.name.as_str(), &property.value);
        if !NAMES.contains(&name) {
            continue;
        }
        if seen.contains(&name) {
            return Err(format!("`{name}` is given twice"));
        }
        seen.push(name);
        let wrong = |expected: &str| format!("`{name}` must be {expected} (got {value})");
        match name {
            "material" => {
                props.material = Some(value.as_str().ok_or_else(|| wrong("a string"))?.to_owned());
            }
            "health" => {
                props.health = Some(value.as_f64().ok_or_else(|| wrong("a number"))? as f32);
            }
            "broken" => {
                let sprite = value.as_str().ok_or_else(|| wrong("a string"))?;
                let Some(index) = tileset.iter().position(|entry| entry == sprite) else {
                    return Err(format!(
                        "`broken` names sprite `{sprite}`, which no tile of the tileset has"
                    ));
                };
                props.broken = index as TileIndex;
            }
            "one_way" => props.one_way = value.as_bool().ok_or_else(|| wrong("a bool"))?,
            "style" => {
                props.style = Some(value.as_str().ok_or_else(|| wrong("a string"))?.to_owned());
            }
            "mask" => {
                let mask = value.as_i64().ok_or_else(|| wrong("an integer"))?;
                props.mask = Some(
                    u8::try_from(mask)
                        .ok()
                        .filter(|&mask| mask <= 15)
                        .ok_or_else(|| format!("`mask` must be 0 to 15 (got {mask})"))?,
                );
            }
            "weight" => {
                let weight = value.as_i64().ok_or_else(|| wrong("an integer"))?;
                props.weight = u32::try_from(weight)
                    .ok()
                    .filter(|&weight| weight >= 1)
                    .ok_or_else(|| format!("`weight` must be 1 to {} (got {weight})", u32::MAX))?;
            }
            _ => {}
        }
    }
    if seen.contains(&"weight") && props.mask.is_none() {
        return Err("`weight` needs `mask`".to_owned());
    }
    props.check(tileset.len())?;
    Ok(props)
}

/// A tile layer's `layers` and `mask` properties, the physics layers its
/// tiles are in and meet: comma-separated layer numbers 0 to 31, on a
/// collision layer only. The message names the property.
fn layer_physics_of(layer: &TiledLayer, kind: LayerKind) -> Result<LayerPhysics, String> {
    let mut physics = LayerPhysics::DEFAULT;
    for property in &layer.properties {
        let name = property.name.as_str();
        let bits = match name {
            "layers" => &mut physics.layers,
            "mask" => &mut physics.mask,
            _ => continue,
        };
        if kind != LayerKind::Collision {
            return Err(format!(
                "`{name}` is a collision layer's property, and this layer's `kind` is not `collision`"
            ));
        }
        let Some(list) = property.value.as_str() else {
            return Err(format!(
                "`{name}` must be a string of layer numbers separated by commas (got {})",
                property.value
            ));
        };
        *bits = layer_bits(list).map_err(|message| format!("`{name}` {message}"))?;
    }
    Ok(physics)
}

/// The bits of `list`, layer numbers 0 to 31 separated by commas, spaces
/// around them allowed; an empty list holds none.
fn layer_bits(list: &str) -> Result<u32, String> {
    if list.trim().is_empty() {
        return Ok(0);
    }
    let mut bits = 0;
    for item in list.split(',').map(str::trim) {
        if item.is_empty() || !item.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!(
                "must be layer numbers 0 to 31 separated by commas (got \"{list}\")"
            ));
        }
        match item.parse::<u32>() {
            Ok(layer) if layer <= 31 => bits |= 1 << layer,
            _ => return Err(format!("holds layer {item}; a layer must be 0 to 31")),
        }
    }
    Ok(bits)
}

/// One `objectgroup` layer, checked: every number finite and each size at
/// least 0, every position within ±2^24, a class given once, and no
/// templated object or classed rotated one.
fn object_layer(layer: &TiledLayer, alignment: &str) -> Result<ObjectLayer, String> {
    let name = &layer.name;
    let (offset_x, offset_y) = (layer.offsetx.unwrap_or(0.0), layer.offsety.unwrap_or(0.0));
    if !(offset_x.is_finite() && offset_y.is_finite()) {
        return Err(format!(
            "object layer '{name}': offset must be finite (got [{offset_x}, {offset_y}])"
        ));
    }
    let mut objects = Vec::with_capacity(layer.objects.len());
    for (index, object) in layer.objects.iter().enumerate() {
        let Some(id) = object.id else {
            return Err(format!(
                "object layer '{name}': objects[{index}] has no `id`"
            ));
        };
        let map_object = map_object(id, object, alignment, offset_x, offset_y)
            .map_err(|message| format!("object layer '{name}', object {id}: {message}"))?;
        objects.push(map_object);
    }
    Ok(ObjectLayer {
        name: name.clone(),
        objects,
    })
}

fn map_object(
    id: u32,
    object: &TiledObject,
    alignment: &str,
    offset_x: f32,
    offset_y: f32,
) -> Result<MapObject, String> {
    let x = object.x.ok_or("has no `x`")?;
    let y = object.y.ok_or("has no `y`")?;
    let width = object.width.unwrap_or(0.0);
    let height = object.height.unwrap_or(0.0);
    for (field, value) in [("x", x), ("y", y), ("width", width), ("height", height)] {
        if !value.is_finite() {
            return Err(format!("`{field}` must be finite (got {value})"));
        }
    }
    for (field, value) in [("width", width), ("height", height)] {
        if value < 0.0 {
            return Err(format!("`{field}` must be at least 0 (got {value})"));
        }
    }
    let object_type = object.object_type.as_deref().unwrap_or_default();
    let class = object.class.as_deref().unwrap_or_default();
    if !object_type.is_empty() && !class.is_empty() && object_type != class {
        return Err(format!(
            "`type` `{object_type}` and `class` `{class}` differ; give the class once"
        ));
    }
    let class = if object_type.is_empty() {
        class
    } else {
        object_type
    };
    if let Some(template) = &object.template {
        return Err(format!(
            "is an instance of template `{template}`, whose fields the loader does not read; detach it in Tiled"
        ));
    }
    let rotation = object.rotation.unwrap_or(0.0);
    if !class.is_empty() && rotation != 0.0 {
        return Err(format!(
            "has class `{class}` and a rotation of {rotation} degrees; a prefab spawns unrotated"
        ));
    }
    // Tiled writes a tile object's `y` at its tileset's alignment point.
    let top = match (object.gid, alignment) {
        (None, _) | (Some(_), "topleft") => y,
        (Some(_), "unspecified" | "bottomleft") => y - height,
        (Some(_), other) => {
            return Err(format!(
                "is a tile object, and its tileset's object alignment `{other}` is not supported; use `bottomleft` or `topleft`"
            ));
        }
    };
    let position = Vec2::new(x + offset_x, top + offset_y);
    if !within_position_bound(position.x, position.y) {
        return Err(format!(
            "position must be finite and within ±{MAX_POSITION} (got {position})"
        ));
    }
    let mut properties = BTreeMap::new();
    for property in &object.properties {
        if properties
            .insert(property.name.clone(), property.value.clone())
            .is_some()
        {
            return Err(format!("property `{}` is given twice", property.name));
        }
    }
    Ok(MapObject {
        id,
        name: object.name.clone().unwrap_or_default(),
        class: class.to_owned(),
        position,
        size: Vec2::new(width, height),
        properties,
    })
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
