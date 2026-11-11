//! The sample's script hooks (redo 7.2): `reload_run` for a script to call
//! and `player` for it to read, each with the line `tungsten describe`
//! prints. The shell registers `start_run`, and the kit `place` and
//! `tagged`, which a script calls with `{"tag": "team.player"}` to set or
//! read the player. A game adds its own the same way, from its plugin.

use serde::Deserialize;
use tungsten::core::{Position, World};
use tungsten::kit::Health;

use super::SampleRun;
use crate::shell;

/// `reload_run`'s arguments: none.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

/// Adds the sample's action and probe to the World's script hooks.
pub fn register(world: &mut World) {
    world.add_script_action_typed(
        "reload_run",
        "Saves the run in memory and continues it as Continue does, with no user folder.",
        |world, NoArgs {}| {
            let save = crate::save::capture(world).ok_or("no run to save")?;
            shell::start_run(world, save.into(), None);
            Ok(())
        },
    );
    world.add_script_probe(
        "player",
        "The player: `{position: [x, y], health: [now, max]}`; null with no run.",
        |world, _| {
            let Some(run) = world.get_resource::<SampleRun>() else {
                return Ok(Default::default());
            };
            let position = world
                .get::<Position>(run.player)
                .map(|p| vec![p.0.x, p.0.y]);
            let health = world
                .get::<Health>(run.player)
                .map(|h| vec![h.current(), h.max()]);
            Ok([("position", position), ("health", health)]
                .into_iter()
                .collect())
        },
    );
}
