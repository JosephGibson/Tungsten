//! The sample's components. Each one a prefab file names derives
//! `Deserialize` and is registered in `SamplePlugin::build`.

use serde::Deserialize;

/// The entity the move actions steer; `prefabs/player.json` names it
/// `player`, a marker written as `"player": null`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Player;
