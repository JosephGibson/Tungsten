//! The run's script hooks (redo 7.2): `place_player` and `reload_run` for a
//! script to call, `player` and `area` for it to read, as the template's
//! sample registers its own, the shell `start_run`; `fill_pit`, `wake_foes`
//! and `pit` set up and read the full pit ([`pit`](super::pit), redo 7.3),
//! and `cast_at` and `pit_fire` land spells on it and read its fire for the
//! `e01-stress` row.

use serde::Deserialize;
use tungsten::core::{Position, PrevPosition, Velocity, World};
use tungsten::kit::Health;

use super::{Run, pit};
use crate::shell;

/// `reload_run`'s arguments: none.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

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

/// `cast_at`'s arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CastAt {
    spell: String,
    at: [f32; 2],
}

/// Adds the run's actions and probes to the World's script hooks.
pub fn register(world: &mut World) {
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
    world.add_script_action(
        "cast_at",
        "Lands a spell's toy at `{\"spell\": name, \"at\": [x, y]}`, free of mana and cooldown: `fire_missile`'s blast, a `lightning` strike, `force_push`'s wave from the wizard, a `black_hole` or an `iron_block`.",
        |world, args| {
            let cast = CastAt::deserialize(args).map_err(|e| e.to_string())?;
            pit::cast_at(world, &cast.spell, cast.at.into())
        },
    );
    world.add_script_probe(
        "pit_fire",
        "The small balls' fire: `{burning, burnt}`, those alight now and those burned out.",
        |world, _| {
            let (burning, burnt) = pit::fire(world);
            Ok([("burning", burning), ("burnt", burnt)]
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
