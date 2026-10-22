//! The level's components and resources.

use glam::Vec2;
use serde::Deserialize;
use tungsten::core::Entity;
use tungsten::kit::ContactDamage;

/// A level emitter of torch embers, spray or motes: the map's `emitters`
/// objects spawn it from the `ex10_ambient_emitter` prefab, which names it
/// `ambient_emitter`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct AmbientEmitter;

/// A hazard the map places: spikes, a fireball on its oscillator, or a
/// lava basin's surface, whose touch the kit's contact damage deals. Its
/// prefab names it `hazard`. Cold douses a fire; lava, never.
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
/// ID; a hidden glow draws nothing and its light goes dark.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Glow {
    pub offset: [f32; 2],
    /// The halo's radius, in world pixels, 0 for none.
    pub radius: f32,
    pub color: [u8; 4],
    /// The light's intensity before its flicker.
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
    pub light: Entity,
    /// The pulse's phase, from the owner's save ID.
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
