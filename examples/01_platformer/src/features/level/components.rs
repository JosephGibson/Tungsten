//! The level's components and resources.

use std::collections::BTreeSet;

use glam::Vec2;
use serde::Deserialize;
use tungsten::core::{Entity, LayerKind, TilemapData};
use tungsten::kit::ContactDamage;

/// A level emitter of torch embers, spray or motes: the map's `emitters`
/// objects spawn it from the `ex10_ambient_emitter` prefab, which names it
/// `ambient_emitter`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AmbientEmitter;

/// A hazard the map places: spikes, a fireball on its oscillator, or a
/// lava basin, whose touch the kit's contact damage deals. Its prefab names
/// it `hazard`. Cold douses a fire and crusts a basin over for a while
/// (`lava`). A basin holds the player on its surface and its touch burns and
/// throws it up and out of the pool, in its invulnerability window too
/// (`push_always`); its child touches what is frozen with fire, which blows
/// it apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Hazard {
    pub fire: bool,
    /// A lava basin's surface.
    #[serde(default)]
    pub lava: bool,
}

/// A fire hazard cold put out, for `left` game seconds: its touch held
/// aside until it relights, its glow and trail hidden meanwhile.
#[derive(Debug, Clone, PartialEq)]
pub struct Doused {
    pub left: f32,
    pub touch: Option<ContactDamage>,
}

/// One of the level's areas: a zone across its columns, named after the
/// landmark it holds, with how it is lit and sounds. Its prefab names it
/// `area`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Area {
    pub name: String,
    /// A cave: the ambient dims and the moon goes out inside it.
    #[serde(default)]
    pub indoor: bool,
    /// Its ambience loop, a sound ID.
    #[serde(default)]
    pub ambience: Option<String>,
    /// Its far strip, a sprite ID drawn between the ridges and the woodland;
    /// none indoors.
    #[serde(default)]
    pub backdrop: Option<String>,
    /// Under the black night sky rather than the midnight one.
    #[serde(default)]
    pub night: bool,
    /// How many rows its backdrop's strips stand lower than the start's,
    /// each times its scroll.
    #[serde(default)]
    pub horizon: i32,
    /// Whether the distant ridges and the near woodland draw behind it; an
    /// area's far strip may stand alone, as the gorge's and the cut's do.
    #[serde(default = "shown")]
    pub ridges: bool,
    #[serde(default = "shown")]
    pub woodland: bool,
}

fn shown() -> bool {
    true
}

/// The player's zoom over the window's base zoom: 1 until the zoom actions
/// change it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom(pub f32);

impl Default for Zoom {
    fn default() -> Self {
        Self(1.0)
    }
}

/// A soft glow and the point light it casts, centred `offset` from the
/// entity's point, its sprite's top-left: a lantern's, a crystal's, a
/// fireball's, the player's lantern. Its prefab names it `glow`. The halo
/// and the light pulse by the level's clock, phased by the entity's save
/// ID, or its column when a save names it not; a hidden glow draws nothing
/// and its light goes dark.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Glow {
    pub offset: [f32; 2],
    /// The halo's radius, in world pixels, 0 for none.
    pub radius: f32,
    pub color: [u8; 4],
    /// The light's intensity before its flicker; 0 for no light, a glow
    /// drawn alone, which takes none of the frame's lights (an orb's).
    #[serde(default = "default_light")]
    pub light: f32,
    /// A hot core's radius drawn over the halo through the flame material,
    /// 0 for none.
    #[serde(default)]
    pub core: f32,
    /// How far the light reaches; 1.4 times the halo's radius when left out.
    #[serde(default)]
    pub reach: Option<f32>,
    /// Hidden while the player's lantern is off, or the player is dead.
    #[serde(skip)]
    pub hidden: bool,
}

fn default_light() -> f32 {
    0.8
}

impl Glow {
    /// The glow's centre for an entity whose point is drawn at `at`.
    #[must_use]
    pub fn centre(&self, at: Vec2) -> Vec2 {
        at + Vec2::from(self.offset)
    }

    /// How far its light reaches.
    #[must_use]
    pub fn reach(&self) -> f32 {
        self.reach.unwrap_or(self.radius * 1.4)
    }
}

/// What a glow draws with, on its owner: the halo's and the core's sprite
/// entities and its light, which leave with the owner.
#[derive(Debug, Clone, Copy)]
pub struct GlowParts {
    pub halo: Option<Entity>,
    pub core: Option<Entity>,
    pub light: Option<Entity>,
    /// The pulse's phase, from the owner's save ID or its column.
    pub phase: f32,
}

/// One of a glow's parts, naming its owner.
#[derive(Debug, Clone, Copy)]
pub struct GlowPart {
    pub owner: Entity,
}

/// Whether `level_light` has lit the run yet: its first frame snaps to the
/// area's light, so a load in a cave starts dark; later frames ease.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AreaLight {
    pub lit: bool,
}

/// The level's clock: game seconds since the run began, which the fixed
/// step advances, so a pause holds it. The clouds drift and the glows and
/// lights pulse by it, drawn between its last two steps.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SceneTime(pub f32);

/// The sprites every pixel of which is opaque, as the generator measured
/// them (`data/opaque_tiles/level.json`).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OpaqueTiles {
    pub sprites: BTreeSet<String>,
}

/// Which of the map's cells a drawn tile of an opaque sprite fills, as a
/// summed-area table of the others, built at a run's spawn: a backdrop quad
/// whose visible part lies wholly on filled cells is never seen.
#[derive(Debug, Clone, Default)]
pub struct Cover {
    cols: usize,
    rows: usize,
    origin: Vec2,
    tile: Vec2,
    /// `(cols + 1) × (rows + 1)`: the open cells above and left of each.
    open: Vec<u32>,
}

impl Cover {
    /// The cells `data`'s drawn layers fill with a sprite of `opaque`,
    /// placed at `origin`.
    #[must_use]
    pub fn of(data: &TilemapData, origin: Vec2, opaque: &OpaqueTiles) -> Self {
        let (cols, rows) = (data.width as usize, data.height as usize);
        let solid: Vec<bool> = data
            .tileset
            .iter()
            .map(|sprite| opaque.sprites.contains(sprite))
            .collect();
        let mut filled = vec![false; cols * rows];
        for layer in data.layers.iter().filter(|l| l.kind == LayerKind::Render) {
            for (cell, &tile) in layer.tiles.iter().take(cols * rows).enumerate() {
                if usize::try_from(tile).is_ok_and(|tile| solid.get(tile) == Some(&true)) {
                    filled[cell] = true;
                }
            }
        }
        let width = cols + 1;
        let mut open = vec![0; width * (rows + 1)];
        for row in 0..rows {
            let mut run = 0;
            for col in 0..cols {
                run += u32::from(!filled[row * cols + col]);
                open[(row + 1) * width + col + 1] = open[row * width + col + 1] + run;
            }
        }
        Self {
            cols,
            rows,
            origin,
            tile: Vec2::new(data.tile_width as f32, data.tile_height as f32),
            open,
        }
    }

    /// Whether every cell the world box `min..max` touches is filled; a box
    /// reaching past the map never is.
    #[must_use]
    pub fn hides(&self, min: Vec2, max: Vec2) -> bool {
        let lo = ((min - self.origin) / self.tile).floor();
        let hi = ((max - self.origin) / self.tile).ceil();
        if !(lo.cmpge(Vec2::ZERO).all() && hi.x <= self.cols as f32 && hi.y <= self.rows as f32)
            || lo.x >= hi.x
            || lo.y >= hi.y
        {
            return false;
        }
        let (c0, r0, c1, r1) = (lo.x as usize, lo.y as usize, hi.x as usize, hi.y as usize);
        let width = self.cols + 1;
        let at = |row: usize, col: usize| self.open[row * width + col];
        at(r1, c1) + at(r0, c0) == at(r0, c1) + at(r1, c0)
    }
}

impl Cover {
    /// The bottom edge, in world y, of the lowest row of cells within the
    /// world box `min..max` that holds an unfilled cell, or `min.y` when
    /// every row is filled: what of the box lies below it is hidden. Rows
    /// past the map are unfilled.
    #[must_use]
    pub fn open_bottom(&self, min: Vec2, max: Vec2) -> f32 {
        let lo = ((min - self.origin) / self.tile).floor();
        let hi = ((max - self.origin) / self.tile).ceil();
        if !(lo.cmpge(Vec2::ZERO).all() && hi.x <= self.cols as f32 && hi.y <= self.rows as f32)
            || lo.x >= hi.x
            || lo.y >= hi.y
        {
            return max.y;
        }
        let (c0, c1) = (lo.x as usize, hi.x as usize);
        let (r0, r1) = (lo.y as usize, hi.y as usize);
        let width = self.cols + 1;
        let at = |row: usize, col: usize| self.open[row * width + col];
        let filled = |row: usize| at(row + 1, c1) + at(row, c0) == at(row, c1) + at(row + 1, c0);
        let mut bottom = r1;
        while bottom > r0 && filled(bottom - 1) {
            bottom -= 1;
        }
        if bottom == r1 {
            max.y
        } else {
            (self.origin.y + bottom as f32 * self.tile.y).max(min.y)
        }
    }
}

/// Marks a game whose assets hot-reload (`main.rs` in a debug build): a
/// sprite reload can change a tile's pixels under its ID with the map as it
/// was, so the backdrop culls nothing then.
#[derive(Debug, Clone, Copy, Default)]
pub struct AssetsReload;
