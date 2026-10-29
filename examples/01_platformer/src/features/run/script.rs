//! The run's script hooks (redo 7.2): `start_run` and `place_player` for a
//! script to call, `player` and `area` for it to read, as the template's
//! sample registers its own; `fill_pit`, `wake_foes` and `pit` set up and
//! read the full pit ([`pit`](super::pit), redo 7.3).

use serde::Deserialize;
use tungsten::core::{Position, PrevPosition, Velocity, World};
use tungsten::kit::Health;

use super::{Run, pit};
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

/// `fill_pit`'s arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fill {
    count: usize,
}

/// Adds the run's actions and probes to the World's script hooks.
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
            let player = world.get_resource::<Run>().ok_or("no run")?.player;
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
            let Some(run) = world.get_resource::<Run>() else {
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
    world.add_script_action(
        "fill_pit",
        "Pours small balls into the pit until the world holds `{\"count\": n}`, at most the cap.",
        |world, args| {
            let count = Fill::deserialize(args).map_err(|e| e.to_string())?.count;
            world.get_resource::<Run>().ok_or("no run")?;
            pit::fill(world, count);
            Ok(())
        },
    );
    world.add_script_action(
        "wake_foes",
        "Puts the player on the bridge's east deck and six durable enemies awake about them.",
        |world, _| pit::wake_foes(world).map(drop),
    );
    world.add_script_probe(
        "pit",
        "The full pit: `{balls, foes_alive, foes_awake}`, of the six `wake_foes` spawned.",
        |world, _| {
            let counts = pit::counts(world);
            Ok([
                ("balls", counts.balls),
                ("foes_alive", counts.foes_alive),
                ("foes_awake", counts.foes_awake),
            ]
            .into_iter()
            .collect())
        },
    );
    world.add_script_probe(
        "area",
        "The area the player last entered, by its name; null before one or with no run.",
        |world, _| {
            Ok(world
                .get_resource::<Run>()
                .and_then(|run| run.area.clone())
                .into())
        },
    );
}
