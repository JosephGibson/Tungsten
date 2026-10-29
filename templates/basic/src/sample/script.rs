//! The sample's script hooks (redo 7.2): `start_run` and `place_player` for
//! a script to call, `player` for it to read, each with the line `tungsten
//! describe` prints. A game adds its own the same way, from its plugin.

use serde::Deserialize;
use tungsten::core::{Position, PrevPosition, Velocity, World};
use tungsten::kit::Health;

use super::SampleRun;
use crate::shell::{self, Difficulty, RunStart};

/// `start_run`'s arguments.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Start {
    #[serde(default)]
    difficulty: Difficulty,
}

/// `place_player`'s arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Place {
    at: [f32; 2],
}

/// Adds the sample's actions and probe to the World's script hooks.
pub fn register(world: &mut World) {
    world.add_script_action(
        "start_run",
        "Starts a fresh run: `{\"difficulty\"?: \"easy\" | \"normal\" | \"hard\"}`.",
        |world, args| {
            let start = if args.is_null() {
                Start::default()
            } else {
                Start::deserialize(args).map_err(|e| e.to_string())?
            };
            let difficulty = start.difficulty;
            shell::start_run(
                world,
                RunStart {
                    difficulty,
                    save: None,
                },
                None,
            );
            Ok(())
        },
    );
    world.add_script_action(
        "place_player",
        "Sets the player's body at rest at `{\"at\": [x, y]}`; `until` holds it.",
        |world, args| {
            let at = Place::deserialize(args)
                .map_err(|e| e.to_string())?
                .at
                .into();
            let player = world.get_resource::<SampleRun>().ok_or("no run")?.player;
            let body = world.get_mut::<Position>(player).ok_or("no player")?;
            body.0 = at;
            if let Some(prev) = world.get_mut::<PrevPosition>(player) {
                prev.0 = at;
            }
            if let Some(velocity) = world.get_mut::<Velocity>(player) {
                velocity.0 = Default::default();
            }
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
