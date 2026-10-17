//! The game's components. Each one a prefab or level file names derives
//! `Deserialize` and is registered in `GamePlugin::build`.

use serde::Deserialize;

/// The entity the move actions steer; `prefabs/player.json` names it
/// `player`, a marker written as `"player": null`.
#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Player;

/// What hurts the player on touch; `prefabs/hazard.json` names it `hazard`:
/// `{ "radius": 28, "damage": 1, "push": 420 }`. Within `radius` world
/// pixels of the player's point it deals `damage`, and a hit that lands
/// pushes the player away at `push` world pixels per second
/// (`game::hazard_touch`). Each number is finite and at least 0, checked
/// as the manifest loads, so a bad value fails the load rather than the
/// first touch.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "HazardFields")]
pub struct Hazard {
    pub radius: f32,
    pub damage: f32,
    pub push: f32,
}

/// A hazard as a prefab writes it, before its numbers are checked.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HazardFields {
    radius: f32,
    damage: f32,
    push: f32,
}

impl TryFrom<HazardFields> for Hazard {
    type Error = String;

    fn try_from(fields: HazardFields) -> Result<Self, Self::Error> {
        for (field, value) in [
            ("radius", fields.radius),
            ("damage", fields.damage),
            ("push", fields.push),
        ] {
            if !(value.is_finite() && value >= 0.0) {
                return Err(format!("{field}: must be finite and >= 0 (got {value})"));
            }
        }
        Ok(Self {
            radius: fields.radius,
            damage: fields.damage,
            push: fields.push,
        })
    }
}
