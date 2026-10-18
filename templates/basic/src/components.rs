//! The game's components. Each one a prefab or level file names derives
//! `Deserialize` and is registered in `GamePlugin::build`.

use serde::Deserialize;

/// The entity the move actions steer; `prefabs/player.json` names it
/// `player`, a marker written as `"player": null`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Player;
