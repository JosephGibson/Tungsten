//! The sample's components. Each one a prefab file names derives
//! `Deserialize` and is registered in `SamplePlugin::build`.

use serde::Deserialize;

/// The entity the move actions steer; `prefabs/player.json` names it
/// `player`, a marker written as `"player": null`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Player;

/// A gem's place in `GEM_STARTS`, which a save lists for the gems left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GemSlot(pub u8);
