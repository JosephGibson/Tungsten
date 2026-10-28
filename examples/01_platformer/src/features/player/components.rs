//! The player's components and resources.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::Deserialize;
use tungsten::core::AnimationState;

use super::Gesture;
use crate::features::level::components::Glow;

/// The player's marker, which its prefab names `player`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Player;

/// The player's staff light, the orb on the wizard's staff, which the code
/// names the lantern (the courier carried one before 01 (h)): off at a
/// run's start until the `lantern` action turns it on, and a loaded run's
/// as its save had it. While on it burns mana through the `staff_light`
/// ability each step; refused, it gutters out until the mana reaches
/// [`RELIGHT_MANA`]. Its glow follows the orb in each frame's art.
///
/// [`RELIGHT_MANA`]: super::RELIGHT_MANA
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

/// Where the staff's orb sits in each wizard frame, from the sprite's
/// top-left, by sprite ID: the `lantern_anchors` data kind, which the
/// generator takes from the wizard's rig.
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
    /// The last cast's gesture, and the game seconds it still holds: its
    /// one-shot clip facing the aim, or standing, its pose along it.
    pub gesture: Option<Gesture>,
    pub cast: f32,
    /// Game seconds the staff still flares from a spell.
    pub glow: f32,
    /// Game seconds a blow's red tint still shows, fading
    /// (`systems::HURT_SECS`).
    pub hurt: f32,
    /// Game seconds a blow's hurt clip still holds
    /// (`systems::HURT_CLIP_SECS`).
    pub hurt_clip: f32,
    /// The hurt edge's share over the view, 0 to 1: a blow's full, a
    /// tick's at least `systems::TICK_EDGE`, falling to 0 over
    /// `systems::EDGE_SECS`.
    pub edge: f32,
    /// The last cast's aim, which its gesture faces.
    pub aim: Vec2,
    /// The one-shot clip shown over the animator's.
    pub shown: Option<Shown>,
    /// It died: its controller drops it, its input locked for good.
    pub dead: bool,
    /// It faced left when it died, as its death clip keeps it.
    pub dead_left: bool,
}

/// The staff light's glow as the player's prefab sets it, kept at spawn:
/// the staff's flare scales the glow from it and puts it back. Not saved.
#[derive(Debug, Clone, Copy)]
pub struct StaffGlow(pub Glow);

/// A one-shot clip shown over the character animator's: its state as last
/// shown, and the animator's clip it covers, which comes back after it.
#[derive(Debug, Clone)]
pub struct Shown {
    pub state: AnimationState,
    pub base: String,
}
