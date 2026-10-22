//! The player's components and resources.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::Deserialize;
use tungsten::core::AnimationState;

/// The player's marker, which its prefab names `player`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Player;

/// The player's lantern, the staff light's flame until the wizard's staff
/// (01 (h)): on until the `lantern` action turns it off. While on it burns
/// mana through the `staff_light` ability each step; refused, it gutters
/// out until the mana reaches [`RELIGHT_MANA`]. Its glow follows the flame
/// in each frame's art.
///
/// [`RELIGHT_MANA`]: super::RELIGHT_MANA
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lantern {
    pub on: bool,
    /// It guttered out for want of mana and waits for the relight.
    pub guttered: bool,
}

impl Lantern {
    /// Whether its flame burns: on and not guttered.
    #[must_use]
    pub fn lit(&self) -> bool {
        self.on && !self.guttered
    }
}

impl Default for Lantern {
    fn default() -> Self {
        Self {
            on: true,
            guttered: false,
        }
    }
}

/// Where the lantern's flame sits in each courier frame, from the sprite's
/// top-left, by sprite ID: the `lantern_anchors` data kind, which the
/// generator measures from the art.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct LanternAnchors(pub BTreeMap<String, [f32; 2]>);

/// What the player's last steps did, for its one-shot clips and bursts:
/// presentation, beside the kit's controller and animator.
#[derive(Debug, Clone, Default)]
pub struct Moves {
    /// It ended its last step on the ground.
    pub grounded: bool,
    /// It has stood once: a spawn settles onto the ground silently.
    pub settled: bool,
    /// Its air jump's tuck plays until the rise ends.
    pub tuck: bool,
    /// Game seconds the land clip still holds.
    pub landing: f32,
    /// Game seconds the toss clip still holds.
    pub toss: f32,
    /// Game seconds a cast's pose still holds: standing, the courier faces
    /// the aim with its arm and gun along it.
    pub pose: f32,
    /// Game seconds the lantern still burns hot from a cast.
    pub glow: f32,
    /// The last cast's aim, which its pose and toss face.
    pub aim: Vec2,
    /// The one-shot clip shown over the animator's.
    pub shown: Option<Shown>,
    /// It died: its controller drops it, its input locked for good.
    pub dead: bool,
}

/// A one-shot clip shown over the character animator's: its state as last
/// shown, and the animator's clip it covers, which comes back after it.
#[derive(Debug, Clone)]
pub struct Shown {
    pub state: AnimationState,
    pub base: String,
}
