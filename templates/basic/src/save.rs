//! What a save holds, part of the seam the shell calls: [`SaveGame`], how
//! a live run becomes one ([`capture`]) and the slot row's one line
//! ([`summary`]). A new game edits this file for its own fields, bumps the
//! schema and writes `migrate`.

use serde::{Deserialize, Serialize};
use tungsten::core::save::Value;
use tungsten::core::{Position, SaveData, World};
use tungsten::kit::{Counters, Health};

use crate::sample::{GEM_STARTS, SampleRun};
use crate::shell::{self, RunMeta};

/// A save: the shell's run data and the sample's fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveGame {
    pub meta: RunMeta,
    /// The player body's `Position`, where the mover left it.
    pub player: [f32; 2],
    pub health: Health,
    /// The gems not yet taken, by their place in `GEM_STARTS`.
    pub gems_left: Vec<u8>,
    /// The game's `gems` count.
    pub gems: i64,
    /// The pickups not yet taken, by their place in `PICKUPS`. The dummy
    /// keeps no state, and an active speed boost is not saved.
    pub pickups_left: Vec<u8>,
}

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and extend `migrate`.
    const SCHEMA: u32 = 3;

    /// Schema 1 held the player's position alone: a Normal run with no
    /// playtime, full health, every gem and none counted. Schema 2 had no
    /// pickups: every one is still there.
    fn migrate(
        from: u32,
        mut data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        if !(1..=2).contains(&from) || !data.is_object() {
            return Err(format!("no migration from schema {from}: {data}").into());
        }
        if from == 2 {
            data["pickups_left"] = vec![0, 1].into();
            return Ok(data);
        }
        let meta: [(&str, Value); 3] = [
            ("game_version", "unknown".into()),
            ("difficulty", "normal".into()),
            ("playtime_secs", 0.0.into()),
        ];
        data["meta"] = meta.into_iter().collect();
        // The player prefab's health at Normal, its window included.
        let health: [(&str, Value); 3] = [
            ("current", 5.0.into()),
            ("max", 5.0.into()),
            ("invulnerability", 0.5.into()),
        ];
        data["health"] = health.into_iter().collect();
        data["gems_left"] = vec![0, 1, 2].into();
        data["gems"] = 0.into();
        Self::migrate(2, data)
    }
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
    let at = world.get::<Position>(run.player)?.0;
    let gems_left = run
        .gems
        .iter()
        .filter(|(_, gem)| world.is_alive(*gem))
        .map(|(slot, _)| *slot)
        .collect();
    let gems = world
        .get_resource::<Counters>()
        .map_or(0, |counters| counters.get("gems"));
    let pickups_left = run
        .pickups
        .iter()
        .filter(|(_, pickup)| world.is_alive(*pickup))
        .map(|(slot, _)| *slot)
        .collect();
    Some(SaveGame {
        meta,
        player: [at.x, at.y],
        health,
        gems_left,
        gems,
        pickups_left,
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
