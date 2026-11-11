//! A room's tileset file (redo 7.12): the tiles every room on it shares,
//! each named by its sprite, with 7.8's tile properties as the Tiled loader
//! takes them, `broken` naming a tile by its sprite, and `whole`.
//!
//! ```json
//! {
//!   "version": 1,
//!   "tiles": [
//!     { "sprite": "rock", "style": "rock" },
//!     { "sprite": "rock_1", "style": "rock", "mask": 1, "weight": 2 },
//!     { "sprite": "rock_45_br", "style": "rock", "slope": "45 br" },
//!     { "sprite": "plank", "one_way": true, "material": "wood", "health": 20 },
//!     { "sprite": "arch", "whole": true }
//!   ]
//! }
//! ```
//!
//! A sprite names one tile: saves record an edited cell's tile by its sprite
//! and restore the first entry holding it, so two tiles of one sprite would
//! restore as one. A styled tile with a `slope` and no `mask` is one of its
//! style's slope tiles, which a polygon's sloped edge takes.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{RoomError, closed, described, from_value, hint, versioned};
use crate::assets::tilemap::{EMPTY_TILE, Slope, TileIndex, TileProps};
use crate::canonical::{self, Arrays};

/// The tileset format's version: a tileset file names it first.
pub const TILESET_VERSION: u32 = 1;

/// A tileset file's model: its tiles in order, each tile's index its place
/// in the list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct Tileset {
    /// The format's version, [`TILESET_VERSION`].
    pub version: u32,
    /// The tiles, in the order a room's map takes them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tiles: Vec<TilesetTile>,
}

/// One tile: its sprite, which names it, and its properties.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct TilesetTile {
    /// The sprite it draws, which names it; at most once in a tileset.
    pub sprite: String,
    /// [`TileProps::material`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<String>,
    /// [`TileProps::health`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<f32>,
    /// The tile a broken cell becomes, by its sprite; empty when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub broken: Option<String>,
    /// [`TileProps::one_way`].
    #[serde(default, skip_serializing_if = "is_false")]
    pub one_way: bool,
    /// [`TileProps::style`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// [`TileProps::mask`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<u8>,
    /// [`TileProps::weight`], 1 when absent.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub weight: u32,
    /// [`TileProps::slope`], written as `"45 br"`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slope: Option<String>,
    /// [`TileProps::whole`].
    #[serde(default, skip_serializing_if = "is_false")]
    pub whole: bool,
}

impl TilesetTile {
    /// A plain tile drawing `sprite`.
    #[must_use]
    pub fn new(sprite: impl Into<String>) -> Self {
        Self {
            sprite: sprite.into(),
            material: None,
            health: None,
            broken: None,
            one_way: false,
            style: None,
            mask: None,
            weight: 1,
            slope: None,
            whole: false,
        }
    }
}

impl Tileset {
    /// Parses tileset JSON: `version` first, so a version other than
    /// [`TILESET_VERSION`] fails before any other field; then the model, an
    /// error naming its field's path; then [`Tileset::validate`].
    ///
    /// # Errors
    ///
    /// A [`RoomError`] naming the field.
    pub fn from_json(json: &str) -> Result<Self, RoomError> {
        let value = versioned(json, "tileset", TILESET_VERSION)?;
        let tileset: Self = from_value(&value)?;
        tileset.validate()?;
        Ok(tileset)
    }

    /// The tileset in its canonical form, as [`Room::to_canonical`]
    /// writes a room.
    ///
    /// [`Room::to_canonical`]: super::Room::to_canonical
    #[must_use]
    pub fn to_canonical(&self) -> String {
        canonical::to_canonical(self, Arrays::InlineNumbers)
    }

    /// The checks [`Tileset::from_json`] runs past the model's own: each
    /// sprite named once and not empty, each `broken` naming a tile, each
    /// `slope` a slope, the rules [`TileProps`]' fields hold, and one brush
    /// a style.
    ///
    /// # Errors
    ///
    /// The first [`RoomError`], naming its field.
    pub fn validate(&self) -> Result<(), RoomError> {
        if self.version != TILESET_VERSION {
            return Err(RoomError::new(
                "version",
                format!(
                    "version {} is not supported; this engine reads version {TILESET_VERSION}",
                    self.version
                ),
            ));
        }
        for (at, tile) in self.tiles.iter().enumerate() {
            if tile.sprite.is_empty() {
                return Err(RoomError::new(
                    format!("tiles[{at}].sprite"),
                    "a tile's sprite is not empty",
                ));
            }
            if let Some(first) = self.tiles[..at]
                .iter()
                .position(|other| other.sprite == tile.sprite)
            {
                return Err(RoomError::new(
                    format!("tiles[{at}].sprite"),
                    format!(
                        "sprite `{}` is also tiles[{first}]'s; a sprite names one tile",
                        tile.sprite
                    ),
                ));
            }
        }
        self.props().map(|_| ())
    }

    /// The tile `sprite` names: its index in the list.
    #[must_use]
    pub fn index_of(&self, sprite: &str) -> Option<TileIndex> {
        self.tiles
            .iter()
            .position(|tile| tile.sprite == sprite)
            .and_then(|index| TileIndex::try_from(index).ok())
    }

    /// Each tile's [`TileProps`], in order, held to their rules.
    pub(crate) fn props(&self) -> Result<Vec<TileProps>, RoomError> {
        let mut all: Vec<TileProps> = Vec::with_capacity(self.tiles.len());
        for (at, tile) in self.tiles.iter().enumerate() {
            let field = |name: &str| format!("tiles[{at}].{name}");
            let broken = match &tile.broken {
                None => EMPTY_TILE,
                Some(sprite) => self.index_of(sprite).ok_or_else(|| {
                    RoomError::new(
                        field("broken"),
                        format!(
                            "no tile `{sprite}`{}",
                            hint(sprite, self.tiles.iter().map(|t| t.sprite.as_str()))
                        ),
                    )
                })?,
            };
            let slope = match &tile.slope {
                None => None,
                Some(text) => Some(
                    Slope::parse(text)
                        .map_err(|message| RoomError::new(field("slope"), message))?,
                ),
            };
            let mut props = TileProps::default();
            props.material.clone_from(&tile.material);
            props.health = tile.health;
            props.broken = broken;
            props.one_way = tile.one_way;
            props.style.clone_from(&tile.style);
            props.mask = tile.mask;
            props.weight = tile.weight;
            props.slope = slope;
            props.whole = tile.whole;
            props
                .check(self.tiles.len())
                .map_err(|message| RoomError::new(format!("tiles[{at}]"), message))?;
            if props.is_brush()
                && let Some(first) = all
                    .iter()
                    .position(|other| other.is_brush() && other.style == props.style)
            {
                return Err(RoomError::new(
                    field("style"),
                    format!(
                        "style `{}` already has a brush, tiles[{first}]",
                        props.style.as_deref().unwrap_or_default()
                    ),
                ));
            }
            all.push(props);
        }
        Ok(all)
    }
}

fn one() -> u32 {
    1
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference.
fn is_one(value: &u32) -> bool {
    *value == 1
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if passes a reference.
fn is_false(value: &bool) -> bool {
    !*value
}

/// The JSON Schema (draft 2020-12) of a tileset file: every object closed,
/// a slope as its text, and a `description` on every property.
#[must_use]
pub fn json_schema() -> Value {
    let text = json!({ "type": "string", "minLength": 1 });
    let tile = closed(
        &[
            (
                "sprite",
                described(
                    text.clone(),
                    "The sprite it draws, a `sprites` ID, and its name in a room; at most once in a tileset.",
                ),
            ),
            (
                "material",
                described(text.clone(), "Its material, a name a game's rules read."),
            ),
            (
                "health",
                described(
                    json!({ "type": "number", "exclusiveMinimum": 0 }),
                    "The damage it takes before it breaks.",
                ),
            ),
            (
                "broken",
                described(
                    text.clone(),
                    "The tile a broken cell becomes, by its sprite; needs `health`.",
                ),
            ),
            (
                "one_way",
                described(
                    json!({ "type": "boolean" }),
                    "In a collision layer, solid through its top face alone, from above.",
                ),
            ),
            (
                "style",
                described(
                    text,
                    "Its auto-tile style; a styled tile with no `mask` and no `slope` is the style's brush, the tile a room paints with.",
                ),
            ),
            (
                "mask",
                described(
                    json!({ "type": "integer", "minimum": 0, "maximum": 15 }),
                    "A variant's open sides: 1 up, 2 right, 4 down, 8 left; needs `style`.",
                ),
            ),
            (
                "weight",
                described(
                    json!({ "type": "integer", "minimum": 1 }),
                    "A variant's draw weight; needs `mask`.",
                ),
            ),
            (
                "slope",
                described(
                    json!({
                        "type": "string",
                        "pattern": "^(45|2:1 low|2:1 high) (br|bl|tr|tl)$",
                    }),
                    "In a collision layer, the slope its cell is: its rise, then the corner its solid part holds, as `45 br`.",
                ),
            ),
            (
                "whole",
                described(
                    json!({ "type": "boolean" }),
                    "Drawn at its sprite's own size from its cell's bottom-left.",
                ),
            ),
        ],
        &["sprite"],
    );
    let mut schema = closed(
        &[
            (
                "version",
                described(json!({ "const": TILESET_VERSION }), "The format's version."),
            ),
            (
                "tiles",
                described(
                    json!({ "type": "array", "items": { "$ref": "#/$defs/tile" } }),
                    "The tiles the rooms on it share, each named by its sprite.",
                ),
            ),
        ],
        &["version"],
    );
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["title"] = json!("Tungsten tileset");
    schema["description"] = json!("A tileset file, listed in a manifest's tilesets.");
    schema["$defs"] = json!({ "tile": tile });
    schema
}
