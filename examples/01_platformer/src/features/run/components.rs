//! The run's resources.

use tungsten::core::Entity;

/// The entities the run spawned and the area it last entered: a `World`
/// resource while gameplay runs, which the death check, the HUD and
/// `save::capture` read, so a replaced run's entities, despawned at the
/// frame's end, never count.
#[derive(Debug, Clone)]
pub struct Run {
    pub player: Entity,
    /// The last area the player entered, by its name.
    pub area: Option<String>,
}
