//! What a save holds, part of the seam the shell calls: [`SaveGame`], how
//! a live run becomes one ([`capture`]) and the slot row's one line
//! ([`summary`]). The run is a `WorldSave`: the entities gameplay names,
//! their changes and the spawns they made, with the resources, captured and
//! restored in one call each. A new game names its entities, registers its
//! own components (`App::register_saved`), bumps the schema and writes
//! `migrate_step`; count on from the sample's schema 4, so a save the
//! sample wrote while you tried it reads as damaged, never as a newer
//! version's.

use serde::{Deserialize, Serialize};
use tungsten::core::save::{WorldSave, capture_world};
use tungsten::core::{SaveData, World};
use tungsten::kit::{Counters, Health};

use crate::sample::{GEM_STARTS, SampleRun};
use crate::shell::{self, RunMeta};

/// A save: the shell's run data, the slot row's numbers, the time each
/// fallen enemy has been down and the run's World.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveGame {
    pub meta: RunMeta,
    pub health: Health,
    /// The game's `gems` count.
    pub gems: i64,
    /// Each fallen enemy's place in `ENEMIES` and its seconds down: a
    /// timer of the run's, which no component holds.
    pub down: Vec<(u8, f32)>,
    pub world: WorldSave,
}

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and add the step from the old
    /// schema to `migrate_step`. The sample's schemas 1 to 3 held no World;
    /// the default step refuses them, so they read as an older version's.
    const SCHEMA: u32 = 4;
}

/// The live run as a save; `None` with no run, no player or a dead one.
#[must_use]
pub fn capture(world: &World) -> Option<SaveGame> {
    let meta = shell::run_meta(world)?;
    let run = world.get_resource::<SampleRun>()?;
    let health = world.get::<Health>(run.player)?.clone();
    if health.is_dead() {
        return None;
    }
    let gems = world
        .get_resource::<Counters>()
        .map_or(0, |counters| counters.get("gems"));
    let down = (0..)
        .zip(&run.enemies)
        .filter(|(_, enemy)| !enemy.alive(world))
        .map(|(slot, enemy)| (slot, enemy.back.elapsed()))
        .collect();
    let world = capture_world(world)
        .inspect_err(|err| log::warn!("The run was not saved: {err}"))
        .ok()?;
    Some(SaveGame {
        meta,
        health,
        gems,
        down,
        world,
    })
}

/// A save's line on its slot row.
#[must_use]
pub fn summary(save: &SaveGame) -> String {
    format!(
        "Gems {} / {} · Health {} / {}",
        save.gems,
        GEM_STARTS.len(),
        save.health.current(),
        save.health.max()
    )
}
