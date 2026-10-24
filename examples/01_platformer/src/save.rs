//! What a save holds, part of the seam the shell calls: [`SaveGame`], how
//! a live run becomes one ([`capture`]), the slot row's line ([`summary`])
//! and how a loaded run takes it back ([`restore`]). The schema counts on
//! from the template sample's 3, so a sample save reads as an older
//! version's, never as a newer one's. Schema 7 marks 01 (d)'s map, whose
//! objects take every save ID anew, so nothing older migrates (`D-202`);
//! schema 8 gives the player the staff light, which [`SaveData::migrate`]
//! adds to a schema 7 save. Schema 10 gives save IDs only to the map
//! objects a save names, so an art change never moves one, and keeps the
//! basins' crusts and the mine's elevator. Schema 11 keeps up to three open
//! black holes, a list where schema 10 kept one. Schema 12 places the map's
//! seven goblins before its placed orbs, which moves every later save ID,
//! and keeps the crates, kegs, bombs and rocks. Schema 13 draws the player
//! as the wizard on a 96 px frame, which moves the body's centre from the
//! sprite's corner, the point a save keeps. Schema 14 drops the hints and
//! carries 01 (i)'s tuning into an older run: the player's one maximum
//! health and its wider reach, a crawler's harder touch, a troll's faster
//! walk and the cold ladder's halved gates.
//!
//! Every saved entity carries a [`SaveId`]: the map's objects a save names
//! (its lava basins, decks, dummies, enemies and placed orbs) take theirs
//! in the order the map spawns them, which a load repeats, and whatever
//! spawns later takes the run's next, which the save keeps: a fresh run's
//! iron, crates and kegs, set at the map's markers, the first. A save names
//! another entity by its save ID, never by its entity: an effect's source
//! too, which a load names again once every entity is back. Presentation
//! is not saved: particles, lights, glows, flames, overlays and tints,
//! animation and facing, flashes, shake, toasts, the HUD and the music.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::save::Value;
use tungsten::core::{
    CommandBuffer, Entity, Position, PrevPosition, SaveData, Transform, Velocity, Visibility, World,
};
use tungsten::kit::{
    Abilities, AilmentBuckets, ContactDamage, ContactTouch, Effects, Health, Hitbox, KitRng,
    Knockback, Lifetime, LinearMotion, Oscillator, PathFollower, Pickup, PlatformerController,
    Projectile, Resist, Source, StateMachine, Stats, Tags, TopDownMover,
};

use crate::features::dummy::{self, components::Dummy};
use crate::features::elements::cascade::Cascades;
use crate::features::elements::components::{BallBurn, Chill, Frozen};
use crate::features::enemies::components::{
    Brain, Displaced, Enemy, EnemyKind, EnemyState, HeldTouch, Sight, Throw, Throws,
};
use crate::features::enemies::drops::{self, Orb, OrbKind};
use crate::features::enemies::{self, crawler, troll};
use crate::features::gun::components::{Bullet, Reload};
use crate::features::gun::{self, BULLET_CENTRE, BulletShot};
use crate::features::level::components::Doused;
use crate::features::level::elevator::{Elevator, Stop};
use crate::features::level::lava::{self, Lava};
use crate::features::player::STAFF_LIGHT;
use crate::features::player::components::Lantern;
use crate::features::props::{
    self,
    components::{Bomb, Crate, Fuel, Keg, PropKind, Rock},
};
use crate::features::run::Run;
use crate::features::spells::components::{Spellbook, Summon};
use crate::features::toys::components::{
    Ball, BallHue, BlackHole, FireMissile, IronBlock, IronScrap, SmallBall,
};
use crate::features::toys::{self, MISSILE_SIZE};
use crate::game::GAMEPLAY;
use crate::shared::{SaveId, SaveIds};
use crate::shell::{self, RunMeta};

/// A save: the shell's run data and the game's, each list in save-ID order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveGame {
    pub meta: RunMeta,
    /// The save ID the run's next spawn takes.
    pub next_id: u64,
    /// The kit's random stream, its generator's words whole.
    pub rng: KitRng,
    /// The last area the player entered, which a load does not announce.
    pub area: Option<String>,
    /// The spell bar's selected slot.
    pub spell: usize,
    pub lantern: bool,
    /// The staff light guttered out and waits for the relight; a schema 7
    /// save, from before the light, has none.
    #[serde(default)]
    pub lantern_guttered: bool,
    pub player: PlayerSave,
    pub platforms: Vec<PlatformSave>,
    pub hazards: Vec<HazardSave>,
    pub balls: Vec<BallSave>,
    pub blocks: Vec<BlockSave>,
    /// The open black holes; schema 10 kept at most one, which its
    /// migration moves into the list.
    pub black_holes: Vec<HoleSave>,
    pub missiles: Vec<MissileSave>,
    pub bullets: Vec<BulletSave>,
    pub dummies: Vec<DummySave>,
    pub enemies: Vec<EnemySave>,
    /// The throws in flight, by their bodies' save IDs.
    pub throws: Vec<ThrowSave>,
    pub orbs: Vec<OrbSave>,
    /// The map's objects the run removed, by save ID: its placed dummies and
    /// enemies no longer alive and its health orbs taken, which a load takes
    /// off the map.
    pub gone: Vec<u64>,
    /// The element endings waiting, whole.
    pub cascades: Cascades,
    /// The summon slot's type, by its place in `SUMMONS`.
    pub summon: usize,
    /// Each lava basin's chill and crust; none before schema 10.
    #[serde(default)]
    pub basins: Vec<BasinSave>,
    /// The mine's elevator; none before schema 10, which leaves it at rest.
    #[serde(default)]
    pub elevators: Vec<ElevatorSave>,
    /// The crates, kegs, bombs and rocks; none before schema 12, whose map
    /// adds none to an older save's run.
    #[serde(default)]
    pub props: Vec<PropSave>,
}

/// The dummies the map's `dummies` layer places.
pub const MAP_DUMMIES: u64 = 1;
/// The enemies the map's `enemies` layer places, and the health orbs its
/// `pickups` layer places after them, its last.
pub const MAP_ENEMIES: u64 = 26;
pub const MAP_PICKUPS: u64 = 8;
/// How far schema 9 moves every save ID at or above schema 8's player's:
/// the thirteen map objects it added (five enemies and eight orbs), which
/// take theirs before the player's.
pub const SHIFT_9: u64 = 13;
/// Schema 9's save IDs of the map objects a save names, every map object
/// taking one then, its props first: the lake's and the shelf's lava, which
/// an effect names as its source, the decks (the elevator's the last), the
/// dummy, the enemies and the placed orbs, then the player's.
const S9_LAKE: u64 = 839;
const S9_SHELF: u64 = 840;
const S9_DECKS: u64 = 841;
const S9_ELEVATOR: u64 = 845;
const S9_DUMMY: u64 = 859;
const S9_ENEMIES: u64 = 860;
const S9_PICKUPS: u64 = 878;
const S9_PLAYER: u64 = 886;
/// Schema 10's: only the map objects a save names take save IDs, in layer
/// order, the three basins first. Frozen here, as schema 9's are, for the
/// migration; `tests/game/saves.rs` holds them to the map. The lake's lava
/// is now two pools, its west one first, and the shelf's the third.
pub const S10_LAKE: u64 = 0;
pub const S10_SHELF: u64 = 2;
pub const S10_DECKS: u64 = 3;
pub const S10_DUMMY: u64 = 8;
pub const S10_ENEMIES: u64 = 9;
pub const S10_PICKUPS: u64 = 28;
pub const S10_PLAYER: u64 = 36;
/// Schema 10's lava, where schema 9's player may stand in it or under it:
/// on an old bed, in the lake's moved stone or on the old ground of a
/// widened pool. Each lake's columns, its banks' row, and the columns where
/// a player moved out of it stands on its banks and its stone, in tiles.
type Lake = (f32, f32, f32, &'static [f32]);
const S10_LAKES: [Lake; 2] = [
    (230.0, 240.0, 63.0, &[229.5, 235.0, 240.5]),
    (260.0, 263.0, 57.0, &[259.5, 263.5]),
];
/// How far schema 12 moves every save ID at or above schema 11's first
/// placed orb's: the map's seven goblins, which take theirs after its other
/// enemies, before the orbs.
pub const SHIFT_12: u64 = 7;
/// Schema 11's first placed orb's save ID, schema 10's layout kept; frozen
/// for the migration, as schema 10's are.
pub const S11_PICKUPS: u64 = 28;
/// Where schema 12's player's body centre sat from its sprite's corner, the
/// point a save keeps: the courier's 64 px frame's. Frozen for the
/// migrations, which place a player by its body before schema 13 moves the
/// point.
pub const S12_BODY_OFFSET: Vec2 = Vec2::new(32.0, 33.0);

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and extend `migrate`. Schema 7
    /// is 01 (d)'s new map: its objects take every save ID anew and every
    /// place moved, so `migrate` refuses each older schema and the slot
    /// reads as an older version's (`D-204`). Schema 8 adds the staff light
    /// to the player's abilities. Schema 9 places new map objects before
    /// the player, which take the save IDs from schema 8's player's on.
    /// Schema 10 gives save IDs only to what a save names, keeps the
    /// basins' crusts and the elevator, and widens the basins. Schema 11
    /// keeps the open black holes as a list (`to_11`). Schema 12 places the
    /// map's goblins before its orbs and keeps the props
    /// (`to_12`). Schema 13 moves the player's point for the wizard's frame
    /// (`to_13`). Schema 14 drops the hints and moves the values 01 (i)
    /// tuned that a save keeps (`to_14`).
    const SCHEMA: u32 = 14;

    fn migrate(
        from: u32,
        mut data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        match from {
            7 => {
                // The staff light's entry, last, ready, as a fresh run's.
                let entries = data
                    .pointer_mut("/player/abilities/entries")
                    .and_then(Value::as_array_mut)
                    .ok_or("schema 7: the player has no abilities")?;
                let entry = format!(
                    r#"{{"name": "{STAFF_LIGHT}", "ability": "{STAFF_LIGHT}", "cooldown_left": 0.0, "charges": null, "refill_left": 0.0, "interval_left": 0.0}}"#
                );
                entries.push(entry.parse()?);
                shift_ids(&mut data)?;
                to_10(&mut data)?;
                to_11(&mut data)?;
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                Ok(data)
            }
            8 => {
                shift_ids(&mut data)?;
                to_10(&mut data)?;
                to_11(&mut data)?;
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                Ok(data)
            }
            9 => {
                to_10(&mut data)?;
                to_11(&mut data)?;
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                Ok(data)
            }
            10 => {
                to_11(&mut data)?;
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                Ok(data)
            }
            11 => {
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                Ok(data)
            }
            12 => {
                to_13(&mut data)?;
                to_14(&mut data)?;
                Ok(data)
            }
            13 => {
                to_14(&mut data)?;
                Ok(data)
            }
            _ => Err(format!("schema {from} has no migration to schema {}", Self::SCHEMA).into()),
        }
    }
}

/// Maps `id` through `to`, when it is a save ID.
fn map_id(id: Option<&mut Value>, to: &impl Fn(u64) -> u64) {
    if let Some(id) = id
        && let Some(value) = id.as_u64()
    {
        *id = Value::from(to(value));
    }
}

/// The elements of the array at `pointer` in `data`, none when it is
/// missing or no array.
fn each<'a>(data: &'a mut Value, pointer: &str) -> impl Iterator<Item = &'a mut Value> {
    data.pointer_mut(pointer)
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
}

/// Schema 8 to 9: every save ID at or above schema 8's player's, which the
/// new map objects now take, moves up by [`SHIFT_9`].
fn shift_ids(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let from = data
        .pointer("/player/id")
        .and_then(Value::as_u64)
        .ok_or("schema 8: the player has no save ID")?;
    map_ids(data, &|id| if id >= from { id + SHIFT_9 } else { id });
    Ok(())
}

/// A schema 9 save ID as schema 10 numbers it: the lava, the decks, the
/// dummy, the enemies and the placed orbs at their new places, each later
/// ID down by the props and statics that no longer take one; any other
/// names nothing a save holds.
fn remap_9(id: u64) -> u64 {
    if id == S9_LAKE {
        S10_LAKE
    } else if id == S9_SHELF {
        S10_SHELF
    } else if id >= S9_PLAYER {
        id - S9_PLAYER + S10_PLAYER
    } else if id >= S9_PICKUPS {
        id - S9_PICKUPS + S10_PICKUPS
    } else if id >= S9_ENEMIES {
        id - S9_ENEMIES + S10_ENEMIES
    } else if id == S9_DUMMY {
        S10_DUMMY
    } else if (S9_DECKS..S9_DUMMY).contains(&id) {
        id - S9_DECKS + S10_DECKS
    } else {
        id
    }
}

/// Schema 9 to 10: the elevator's old ping-pong deck dropped, so it starts
/// at rest; every save ID through [`remap_9`]; and a player over a lake no
/// higher than its banks, in its lava or under it, set at rest on the
/// nearest of its banks and its stone.
fn to_10(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(platforms) = data.pointer_mut("/platforms").and_then(Value::as_array_mut) {
        platforms
            .retain(|platform| platform.get("id").and_then(Value::as_u64) != Some(S9_ELEVATOR));
    }
    map_ids(data, &remap_9);
    let Some(at) = data
        .pointer("/player/position")
        .and_then(Value::as_array)
        .and_then(|xy| Some([xy.first()?.as_f64()? as f32, xy.get(1)?.as_f64()? as f32]))
    else {
        return Err("schema 9: the player has no position".into());
    };
    let centre = Vec2::from(at) + S12_BODY_OFFSET;
    let half = crate::features::player::PLAYER_HALF;
    let tile = crate::features::level::TILE;
    for (left, right, bank, places) in S10_LAKES {
        let over = centre.x + half.x > left * tile && centre.x - half.x < right * tile;
        if !over || centre.y + half.y <= bank * tile - 1.0 {
            continue;
        }
        let to = |column: f32| (column * tile - centre.x).abs();
        let nearest = places
            .iter()
            .copied()
            .min_by(|a, b| to(*a).total_cmp(&to(*b)));
        if let Some(column) = nearest {
            let middle = Vec2::new(column * tile, bank * tile - half.y - 0.5);
            let corner = middle - S12_BODY_OFFSET;
            data["player"]["position"] = Value::from(vec![corner.x, corner.y]);
            data["player"]["velocity"] = Value::from(vec![0.0_f32, 0.0]);
        }
    }
    Ok(())
}

/// Schema 10 to 11: the one black hole a save kept becomes the list's
/// only one, none an empty list, and the old key goes, since [`SaveGame`]
/// denies unknown fields; a schema 9 save's hole took its new save ID in
/// [`to_10`] first. The gun's magazine holds [`gun::MAGAZINE`] rounds now,
/// so the player's `gun` keeps at most that many of a save's twelve, a
/// reload left running running on.
fn to_11(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let save = data
        .as_object_mut()
        .ok_or("schema 10: the save is no object")?;
    let holes = match save.remove("black_hole") {
        None | Some(Value::Null) => Vec::new(),
        Some(hole) => vec![hole],
    };
    save.insert("black_holes".into(), Value::Array(holes));
    for entry in each(data, "/player/abilities/entries") {
        if entry.get("name").and_then(Value::as_str) == Some(gun::GUN)
            && let Some(charges) = entry.get("charges").and_then(Value::as_u64)
        {
            entry["charges"] = Value::from(charges.min(u64::from(gun::MAGAZINE)));
        }
    }
    Ok(())
}

/// Schema 11 to 12: the map's seven goblins take the save IDs from schema
/// 11's first placed orb's on, so every save ID at or above it moves up by
/// [`SHIFT_12`]. A schema 11 save names no goblin, in its lists or in
/// `gone`, so the map's stand at their homes after the load, and it has no
/// props, so its run has none of the map's crates and kegs.
fn to_12(data: &mut Value) {
    map_ids(data, &|id| {
        if id >= S11_PICKUPS { id + SHIFT_12 } else { id }
    });
}

/// Schema 12 to 13: the player is drawn on the wizard's 96 px frame, whose
/// body centre sits at [`BODY_OFFSET`](crate::features::player::BODY_OFFSET)
/// from its corner, so the saved corner moves by the two offsets'
/// difference and the body stays where it stood. Nothing else saved names
/// the frame.
fn to_13(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(at) = data
        .pointer("/player/position")
        .and_then(Value::as_array)
        .and_then(|xy| Some([xy.first()?.as_f64()? as f32, xy.get(1)?.as_f64()? as f32]))
    else {
        return Err("schema 12: the player has no position".into());
    };
    let corner = Vec2::from(at) + S12_BODY_OFFSET - crate::features::player::BODY_OFFSET;
    data["player"]["position"] = Value::from(vec![corner.x, corner.y]);
    Ok(())
}

/// Schema 13 to 14 (01 (i)): the hints go; the player's maximum health is
/// [`MAX_HEALTH`](crate::features::player::MAX_HEALTH) at every difficulty,
/// its health keeping its share of the old maximum, as the kit's
/// `Health::set_max` keeps it; its `pickup_radius` base, each crawler's
/// `damage` base and each troll's `move_speed` base move to this stop's
/// values, their modifiers kept; and every saved cold build-up, the
/// player's, a dummy's or an enemy's, halves with the cold ladder's gates,
/// keeping its share of them.
fn to_14(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(save) = data.as_object_mut() {
        save.remove("hints");
    }
    let health = |key: &str| {
        data.pointer(&format!("/player/health/{key}"))
            .and_then(Value::as_f64)
    };
    let (Some(current), Some(max)) = (health("current"), health("max")) else {
        return Err("schema 13: the player has no health".into());
    };
    let full = f64::from(crate::features::player::MAX_HEALTH);
    let player = &mut data["player"];
    let share = if max > 0.0 { current / max } else { 1.0 };
    player["health"]["current"] = Value::from((share * full).clamp(0.0, full));
    player["health"]["max"] = Value::from(full);
    player["stats"]["max_health"]["base"] = Value::from(full);
    let reach = f64::from(crate::features::player::PICKUP_RADIUS);
    player["stats"]["pickup_radius"]["base"] = Value::from(reach);
    halve_cold(&mut player["buckets"]);
    for dummy in data["dummies"].as_array_mut().into_iter().flatten() {
        halve_cold(&mut dummy["buckets"]);
    }
    for enemy in data["enemies"].as_array_mut().into_iter().flatten() {
        if enemy["kind"] == "crawler" {
            enemy["stats"]["damage"]["base"] = Value::from(f64::from(crawler::DAMAGE));
        }
        if enemy["kind"] == "troll" {
            enemy["stats"]["move_speed"]["base"] = Value::from(f64::from(troll::MOVE_SPEED));
        }
        halve_cold(&mut enemy["buckets"]);
    }
    Ok(())
}

/// Halves the `damage.cold` fill in a saved `{damage type: fill}` map.
fn halve_cold(buckets: &mut Value) {
    if let Some(fill) = buckets.get_mut("damage.cold")
        && let Some(value) = fill.as_f64()
    {
        *fill = Value::from(value * 0.5);
    }
}

/// Every save ID field of `data` through `to`; a field it lacks stays
/// lacking.
fn map_ids(data: &mut Value, to: &impl Fn(u64) -> u64) {
    map_id(data.get_mut("next_id"), to);
    map_id(data.pointer_mut("/player/id"), to);
    map_id(data.pointer_mut("/black_hole/id"), to);
    for source in each(data, "/player/sources") {
        map_id(source.get_mut(1), to);
    }
    for list in [
        "/platforms",
        "/hazards",
        "/blocks",
        "/black_holes",
        "/orbs",
        "/basins",
        "/elevators",
    ] {
        for entry in each(data, list) {
            map_id(entry.get_mut("id"), to);
        }
    }
    for prop in each(data, "/props") {
        map_id(prop.get_mut("id"), to);
        map_id(prop.get_mut("lighter"), to);
        map_id(prop.pointer_mut("/fuel/lighter"), to);
    }
    for list in ["/missiles", "/bullets"] {
        for entry in each(data, list) {
            map_id(entry.get_mut("id"), to);
            map_id(entry.get_mut("source"), to);
        }
    }
    for list in ["/dummies", "/enemies"] {
        for entry in each(data, list) {
            map_id(entry.get_mut("id"), to);
            for source in each(entry, "/sources") {
                map_id(source.get_mut(1), to);
            }
        }
    }
    for enemy in each(data, "/enemies") {
        for target in each(enemy, "/hit") {
            map_id(Some(target), to);
        }
        for touch in each(enemy, "/touches") {
            map_id(touch.get_mut(0), to);
        }
        map_id(enemy.get_mut("held"), to);
    }
    for throw in each(data, "/throws") {
        map_id(throw.get_mut("body"), to);
        map_id(throw.get_mut("thrower"), to);
    }
    for ball in each(data, "/balls") {
        map_id(ball.get_mut(0), to);
    }
    for id in each(data, "/gone") {
        map_id(Some(id), to);
    }
    for entry in each(data, "/cascades/entries") {
        map_id(entry.get_mut("target"), to);
        map_id(entry.get_mut("source"), to);
        for member in each(entry, "/chain") {
            map_id(Some(member), to);
        }
    }
}

/// The player: its body's point and velocity, its controller, health,
/// stats, effects with their sources, abilities, ailment buckets, tags and
/// the gun's reload while one runs.
/// The tags hold what an effect grants, so a save during `casting` loads
/// with the mana still paused.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerSave {
    pub id: u64,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub controller: PlatformerController,
    pub health: Health,
    pub stats: Stats,
    pub effects: Effects,
    /// Each effect instance's source: its serial and the source's save ID.
    pub sources: Vec<(u64, u64)>,
    pub abilities: Abilities,
    pub buckets: AilmentBuckets,
    pub tags: Tags,
    /// The seconds the gun's reload has left; none while no reload runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reload: Option<f32>,
}

/// A training dummy: one the map placed (`map`), found by its save ID, or
/// a summoned one, spawned; its body, health, resistances, effects with
/// their sources, ailment buckets and tags, which its statuses grant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DummySave {
    pub id: u64,
    pub map: bool,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub health: Health,
    pub resist: Resist,
    pub effects: Effects,
    pub sources: Vec<(u64, u64)>,
    pub buckets: AilmentBuckets,
    pub tags: Tags,
}

/// An enemy: one the map placed (`map`), found by its save ID, or a
/// summoned one, spawned; its body, its driver (a walker's controller or a
/// flyer's mover, and a frozen flyer's mover set aside beside the
/// controller it falls on), its machine, what it knows and sees, its
/// health, stats, resistances, effects with their sources, ailment buckets,
/// tags, abilities, knockback, its hitbox with the targets its activation
/// has hit and its touches' times, by save ID, and what a troll holds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemySave {
    pub id: u64,
    pub kind: EnemyKind,
    pub map: bool,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub controller: Option<PlatformerController>,
    pub mover: Option<TopDownMover>,
    pub displaced: Option<TopDownMover>,
    pub machine: StateMachine<EnemyState>,
    pub enemy: Enemy,
    pub brain: Brain,
    pub sight: Sight,
    pub health: Health,
    pub stats: Stats,
    pub resist: Resist,
    pub effects: Effects,
    pub sources: Vec<(u64, u64)>,
    pub buckets: AilmentBuckets,
    pub tags: Tags,
    pub abilities: Option<Abilities>,
    pub knockback: Option<Knockback>,
    pub hitbox: Option<Hitbox>,
    /// Its hitbox's activation's targets.
    pub hit: Vec<u64>,
    /// Each target its touch times: (save ID, seconds since its hit, since
    /// they met).
    pub touches: Vec<(u64, f32, f32)>,
    /// What a troll holds over its head, by save ID.
    pub held: Option<u64>,
}

/// An orb: its save ID, kind, point and the seconds before it fades.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrbSave {
    pub id: u64,
    pub kind: OrbKind,
    pub position: [f32; 2],
    pub left: f32,
}

/// A throw in flight: the body's and the thrower's save IDs, the
/// thrower's damage factor at the throw and the seconds it still credits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThrowSave {
    pub body: u64,
    pub thrower: u64,
    pub factor: f32,
    pub left: f32,
}

/// A moving platform: where it is and where its path has it going.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformSave {
    pub id: u64,
    pub position: [f32; 2],
    pub follower: PathFollower,
}

/// A lava basin: the cold spray's seconds on it and its crust's seconds
/// left, while set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BasinSave {
    pub id: u64,
    pub cold: f32,
    pub crust: Option<f32>,
}

/// The mine's elevator: its deck's point, its stop, a boarding's seconds
/// left, its seconds unridden at the top, its rider last step, a held trip
/// and the trip's follower, while one runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElevatorSave {
    pub id: u64,
    pub position: [f32; 2],
    pub stop: Stop,
    pub board: Option<f32>,
    pub idle: f32,
    pub ridden: bool,
    pub held: bool,
    pub follower: Option<PathFollower>,
}

/// A fire hazard on its oscillator: where it is and its clock, since the
/// oscillator adds its change to the point, and the seconds cold has put it
/// out for yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HazardSave {
    pub id: u64,
    pub position: [f32; 2],
    pub oscillator: Oscillator,
    pub doused: Option<f32>,
}

/// A small ball or a bowling ball, its hue, its fire and its ice. A full
/// pit holds 12,000, so each is one flat array in the save: `[id, small, x,
/// y, vx, vy, hue, hue speed, burn left, ice]`, the last four `null` when
/// it has none. Since schema 12 the array leaves its trailing `null`s off,
/// so a hued ball with no fire or ice writes eight values; a load reads six
/// to ten. Its ice is a chill in `(0, 1)`, or 1 plus the seconds a freeze
/// has left.
#[derive(Debug, Clone, PartialEq)]
pub struct BallSave {
    pub id: u64,
    pub small: bool,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub hue: Option<BallHue>,
    pub burn: Option<BallBurn>,
    pub ice: Option<f32>,
}

/// A [`BallSave`] as the save writes it.
type BallRecord = (
    u64,
    bool,
    f32,
    f32,
    f32,
    f32,
    Option<f32>,
    Option<f32>,
    Option<f32>,
    Option<f32>,
);

impl From<BallRecord> for BallSave {
    fn from((id, small, x, y, vx, vy, hue, speed, burn, ice): BallRecord) -> Self {
        Self {
            id,
            small,
            position: [x, y],
            velocity: [vx, vy],
            hue: hue.zip(speed).map(|(hue, speed)| BallHue { hue, speed }),
            burn: burn.map(|remaining| BallBurn {
                remaining,
                exposed: true,
            }),
            ice,
        }
    }
}

impl From<BallSave> for BallRecord {
    fn from(ball: BallSave) -> Self {
        let [x, y] = ball.position;
        let [vx, vy] = ball.velocity;
        let hue = ball.hue.map(|hue| (hue.hue, hue.speed));
        (
            ball.id,
            ball.small,
            x,
            y,
            vx,
            vy,
            hue.map(|(hue, _)| hue),
            hue.map(|(_, speed)| speed),
            ball.burn.map(|burn| burn.remaining),
            ball.ice,
        )
    }
}

impl Serialize for BallSave {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let (id, small, x, y, vx, vy, hue, speed, burn, ice) = BallRecord::from(self.clone());
        let tail = [hue, speed, burn, ice];
        let kept = tail
            .iter()
            .rposition(Option::is_some)
            .map_or(0, |last| last + 1);
        let mut seq = serializer.serialize_seq(Some(6 + kept))?;
        seq.serialize_element(&id)?;
        seq.serialize_element(&small)?;
        for value in [x, y, vx, vy] {
            seq.serialize_element(&value)?;
        }
        for value in &tail[..kept] {
            seq.serialize_element(value)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for BallSave {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(BallVisitor)
    }
}

/// Reads a [`BallSave`]'s array: its six leading values, then as many of its
/// last four as it keeps, an older save's ten whole.
struct BallVisitor;

impl<'de> serde::de::Visitor<'de> for BallVisitor {
    type Value = BallSave;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a ball's array of six to ten values")
    }

    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<BallSave, A::Error> {
        let short = |read: usize| serde::de::Error::invalid_length(read, &self);
        let id = seq.next_element()?.ok_or_else(|| short(0))?;
        let small = seq.next_element()?.ok_or_else(|| short(1))?;
        let mut body = [0.0_f32; 4];
        for (i, value) in body.iter_mut().enumerate() {
            *value = seq.next_element()?.ok_or_else(|| short(2 + i))?;
        }
        let mut tail = [None; 4];
        for value in &mut tail {
            match seq.next_element::<Option<f32>>()? {
                Some(read) => *value = read,
                None => break,
            }
        }
        if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
            return Err(serde::de::Error::invalid_length(11, &self));
        }
        let [x, y, vx, vy] = body;
        let [hue, speed, burn, ice] = tail;
        Ok(BallSave::from((
            id, small, x, y, vx, vy, hue, speed, burn, ice,
        )))
    }
}

/// A body's ice as a save writes it: 1 plus the seconds its freeze has
/// left, else its chill.
fn ice_of(chill: Option<&Chill>, frozen: Option<&Frozen>) -> Option<f32> {
    frozen
        .map(|frozen| 1.0 + frozen.left.max(0.0))
        .or_else(|| chill.map(|chill| chill.amount))
}

/// Puts a saved ice back on `entity`.
fn set_ice(world: &mut World, entity: Entity, ice: Option<f32>) {
    match ice {
        Some(ice) if ice >= 1.0 => world.insert(entity, Frozen { left: ice - 1.0 }),
        Some(amount) => world.insert(
            entity,
            Chill {
                amount,
                held: false,
            },
        ),
        None => {}
    }
}

/// An iron block, or a scrap of one by its variant, its frost and the
/// seconds its freeze has left.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockSave {
    pub id: u64,
    pub scrap: Option<usize>,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub chill: Option<f32>,
    pub frozen: Option<f32>,
}

/// An open black hole: where it is open and for how long yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HoleSave {
    pub id: u64,
    pub position: [f32; 2],
    pub left: f32,
}

/// A fire missile in flight, and the save ID of what fired it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissileSave {
    pub id: u64,
    pub position: [f32; 2],
    pub projectile: Projectile,
    pub motion: LinearMotion,
    pub source: Option<u64>,
}

/// A bullet in flight: its point, its flight and its range so far, the
/// times it turned off iron, and the save ID of what fired it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BulletSave {
    pub id: u64,
    pub position: [f32; 2],
    pub projectile: Projectile,
    pub motion: LinearMotion,
    pub ricochets: u32,
    pub source: Option<u64>,
}

/// A crate, a keg, or a goblin's bomb or rock: its kind, a crate's or a
/// rock's art, its body, its frost and the seconds its freeze has left, a
/// crate's fire, a keg's or a bomb's lit fuse and the save ID its fire or
/// blast credits, and the seconds a douse keeps a keg from lighting. A
/// thrown rock's credit is its throw's (`throws`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropSave {
    pub id: u64,
    pub kind: PropKind,
    pub variant: usize,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub chill: Option<f32>,
    pub frozen: Option<f32>,
    pub fuel: Option<Fuel>,
    pub fuse: Option<f32>,
    pub lighter: Option<u64>,
    pub doused: f32,
}

/// The live run as a save; `None` with no run or a dead player.
#[must_use]
pub fn capture(world: &World) -> Option<SaveGame> {
    let meta = shell::run_meta(world)?;
    let run = world.get_resource::<Run>()?;
    let player = capture_player(world, run.player)?;
    let id_of = |entity: Entity| world.get::<SaveId>(entity).map(|id| id.0);
    let mut platforms: Vec<PlatformSave> = world
        .query_filtered::<(&SaveId, &Position, &PathFollower), tungsten::core::Without<Elevator>>()
        .map(|(id, at, follower)| PlatformSave {
            id: id.0,
            position: at.0.to_array(),
            follower: follower.clone(),
        })
        .collect();
    platforms.sort_by_key(|platform| platform.id);
    let mut basins: Vec<BasinSave> = world
        .query::<(&SaveId, &Lava)>()
        .map(|(id, lava)| BasinSave {
            id: id.0,
            cold: lava.cold,
            crust: lava.crust.as_ref().map(|crust| crust.left),
        })
        .collect();
    basins.sort_by_key(|basin| basin.id);
    let mut elevators: Vec<ElevatorSave> = world
        .query::<(&SaveId, &Position, &Elevator, Option<&PathFollower>)>()
        .map(|(id, at, lift, follower)| ElevatorSave {
            id: id.0,
            position: at.0.to_array(),
            stop: lift.stop,
            board: lift.board,
            idle: lift.idle,
            ridden: lift.ridden,
            held: lift.held,
            follower: follower.cloned(),
        })
        .collect();
    elevators.sort_by_key(|lift| lift.id);
    let mut hazards: Vec<HazardSave> = world
        .query::<(&SaveId, &Position, &Oscillator, Option<&Doused>)>()
        .map(|(id, at, oscillator, doused)| HazardSave {
            id: id.0,
            position: at.0.to_array(),
            oscillator: oscillator.clone(),
            doused: doused.map(|doused| doused.left),
        })
        .collect();
    hazards.sort_by_key(|hazard| hazard.id);
    let mut balls: Vec<BallSave> = world
        .query::<(
            Entity,
            &SaveId,
            &Ball,
            &Position,
            &Velocity,
            Option<&SmallBall>,
            Option<&BallHue>,
            Option<&BallBurn>,
        )>()
        .map(|(entity, id, _, at, velocity, small, hue, burn)| BallSave {
            id: id.0,
            small: small.is_some(),
            position: at.0.to_array(),
            velocity: velocity.0.to_array(),
            hue: hue.copied(),
            burn: burn.copied(),
            ice: ice_of(world.get::<Chill>(entity), world.get::<Frozen>(entity)),
        })
        .collect();
    balls.sort_by_key(|ball| ball.id);
    let mut blocks: Vec<BlockSave> = world
        .query::<(
            &SaveId,
            &Position,
            &Velocity,
            Option<&IronBlock>,
            Option<&IronScrap>,
            Option<&Chill>,
            Option<&Frozen>,
        )>()
        .filter(|(_, _, _, block, scrap, ..)| block.is_some() || scrap.is_some())
        .map(|(id, at, velocity, _, scrap, chill, frozen)| BlockSave {
            id: id.0,
            scrap: scrap.map(|scrap| scrap.variant),
            position: at.0.to_array(),
            velocity: velocity.0.to_array(),
            chill: chill.map(|chill| chill.amount),
            frozen: frozen.map(|frozen| frozen.left),
        })
        .collect();
    blocks.sort_by_key(|block| block.id);
    let mut black_holes: Vec<HoleSave> = world
        .query::<(&SaveId, &Transform, &BlackHole)>()
        .map(|(id, at, hole)| HoleSave {
            id: id.0,
            position: at.position.to_array(),
            left: hole.left,
        })
        .collect();
    black_holes.sort_by_key(|hole| hole.id);
    let mut missiles: Vec<MissileSave> = world
        .query::<(
            &SaveId,
            &FireMissile,
            &Position,
            &Projectile,
            &LinearMotion,
            Option<&Source>,
        )>()
        .map(|(id, _, at, projectile, motion, source)| MissileSave {
            id: id.0,
            position: at.0.to_array(),
            projectile: projectile.clone(),
            motion: motion.clone(),
            source: source.and_then(|source| id_of(source.entity())),
        })
        .collect();
    missiles.sort_by_key(|missile| missile.id);
    let mut bullets: Vec<BulletSave> = world
        .query::<(
            &SaveId,
            &Bullet,
            &Position,
            &Projectile,
            &LinearMotion,
            Option<&Source>,
        )>()
        .filter(|(.., projectile, _, _)| !projectile.is_spent())
        .map(|(id, bullet, at, projectile, motion, source)| BulletSave {
            id: id.0,
            position: at.0.to_array(),
            projectile: projectile.clone(),
            motion: motion.clone(),
            ricochets: bullet.ricochets,
            source: source.and_then(|source| id_of(source.entity())),
        })
        .collect();
    bullets.sort_by_key(|bullet| bullet.id);
    let mut props: Vec<PropSave> = props::all(world)
        .into_iter()
        .filter_map(|entity| capture_prop(world, entity))
        .collect();
    props.sort_by_key(|prop| prop.id);
    let dummies = dummy::live(world)
        .into_iter()
        .filter_map(|(id, entity)| capture_dummy(world, id, entity))
        .collect();
    let enemies = enemies::live(world)
        .into_iter()
        .filter_map(|(id, entity)| capture_enemy(world, id, entity))
        .collect();
    let mut throws: Vec<ThrowSave> = world
        .get_resource::<Throws>()
        .map(|throws| {
            throws
                .0
                .iter()
                .filter_map(|throw| {
                    Some(ThrowSave {
                        body: world.get::<SaveId>(throw.body)?.0,
                        thrower: throw.thrower,
                        factor: throw.factor,
                        left: throw.left,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    throws.sort_by_key(|throw| throw.body);
    let mut orbs: Vec<OrbSave> = world
        .query::<(&Orb, &SaveId, &Position, &Lifetime, &Pickup)>()
        .filter(|(.., pickup)| !pickup.is_collected())
        .map(|(orb, id, at, lifetime, _)| OrbSave {
            id: id.0,
            kind: orb.kind,
            position: at.0.to_array(),
            left: lifetime.timer().remaining(),
        })
        .collect();
    orbs.sort_by_key(|orb| orb.id);
    let mut gone = dummy::gone(world);
    gone.extend(enemies::gone(world));
    gone.extend(enemies::drops::gone(world));
    gone.sort_unstable();
    Some(SaveGame {
        meta,
        next_id: world.resource::<SaveIds>().peek(),
        rng: world.get_resource::<KitRng>().cloned().unwrap_or_default(),
        area: run.area.clone(),
        spell: world
            .get::<Spellbook>(run.player)
            .map_or(0, |book| book.slot),
        lantern: world
            .get::<Lantern>(run.player)
            .is_none_or(|lantern| lantern.on),
        lantern_guttered: world
            .get::<Lantern>(run.player)
            .is_some_and(|lantern| lantern.guttered),
        player,
        platforms,
        hazards,
        balls,
        blocks,
        black_holes,
        missiles,
        bullets,
        dummies,
        enemies,
        throws,
        orbs,
        gone,
        cascades: world
            .get_resource::<Cascades>()
            .cloned()
            .unwrap_or_default(),
        summon: world
            .get_resource::<Summon>()
            .map_or(0, |summon| summon.selected),
        basins,
        elevators,
        props,
    })
}

/// A crate's, a keg's, a bomb's or a rock's part.
fn capture_prop(world: &World, entity: Entity) -> Option<PropSave> {
    let keg = world.get::<Keg>(entity);
    let bomb = world.get::<Bomb>(entity);
    let variant = world
        .get::<Crate>(entity)
        .map(|own| own.variant)
        .or_else(|| world.get::<Rock>(entity).map(|own| own.variant));
    Some(PropSave {
        id: world.get::<SaveId>(entity)?.0,
        kind: props::kind(world, entity)?,
        variant: variant.unwrap_or(0),
        position: world.get::<Position>(entity)?.0.to_array(),
        velocity: world.get::<Velocity>(entity)?.0.to_array(),
        chill: world.get::<Chill>(entity).map(|chill| chill.amount),
        frozen: world.get::<Frozen>(entity).map(|frozen| frozen.left),
        fuel: world.get::<Fuel>(entity).copied(),
        fuse: keg
            .and_then(|keg| keg.fuse)
            .or_else(|| bomb.and_then(|bomb| bomb.fuse)),
        lighter: keg
            .and_then(|keg| keg.lighter)
            .or_else(|| bomb.and_then(|bomb| bomb.lighter)),
        doused: keg.map_or(0.0, |keg| keg.doused),
    })
}

/// Each of `entity`'s effect instances that names a source with a save ID:
/// its serial and the source's save ID.
fn sources(world: &World, entity: Entity) -> Vec<(u64, u64)> {
    world
        .get::<Effects>(entity)
        .map_or_else(Vec::new, |effects| {
            effects
                .instances()
                .filter_map(|instance| {
                    let source = world.get::<SaveId>(instance.source()?)?;
                    Some((instance.serial(), source.0))
                })
                .collect()
        })
}

/// A live dummy's part.
fn capture_dummy(world: &World, id: u64, entity: Entity) -> Option<DummySave> {
    Some(DummySave {
        id,
        map: dummy::placed(world, id),
        position: world.get::<Position>(entity)?.0.to_array(),
        velocity: world.get::<Velocity>(entity)?.0.to_array(),
        health: world.get::<Health>(entity)?.clone(),
        resist: world.get::<Resist>(entity).cloned().unwrap_or_default(),
        effects: world.get::<Effects>(entity).cloned().unwrap_or_default(),
        sources: sources(world, entity),
        buckets: world
            .get::<AilmentBuckets>(entity)
            .cloned()
            .unwrap_or_default(),
        tags: world.get::<Tags>(entity).cloned().unwrap_or_default(),
    })
}

/// A live enemy's part.
fn capture_enemy(world: &World, id: u64, entity: Entity) -> Option<EnemySave> {
    let id_of = |entity: Entity| world.get::<SaveId>(entity).map(|id| id.0);
    let hitbox = world.get::<Hitbox>(entity).cloned();
    let hit = hitbox.as_ref().map_or_else(Vec::new, |hitbox| {
        hitbox
            .hit_targets()
            .iter()
            .filter_map(|&target| id_of(target))
            .collect()
    });
    let touches = world
        .get::<ContactDamage>(entity)
        .or_else(|| world.get::<HeldTouch>(entity).map(|held| &held.0))
        .map_or_else(Vec::new, |contact| {
            contact
                .touches()
                .filter_map(|touch| {
                    Some((id_of(touch.target())?, touch.since_hit(), touch.since_met()))
                })
                .collect()
        });
    let enemy = *world.get::<Enemy>(entity)?;
    Some(EnemySave {
        id,
        kind: enemy.kind,
        map: enemies::placed(world, id),
        position: world.get::<Position>(entity)?.0.to_array(),
        velocity: world.get::<Velocity>(entity)?.0.to_array(),
        controller: world.get::<PlatformerController>(entity).cloned(),
        mover: world.get::<TopDownMover>(entity).cloned(),
        displaced: world
            .get::<Displaced>(entity)
            .map(|displaced| displaced.0.clone()),
        machine: world.get::<StateMachine<EnemyState>>(entity)?.clone(),
        enemy,
        brain: *world.get::<Brain>(entity)?,
        sight: *world.get::<Sight>(entity)?,
        health: world.get::<Health>(entity)?.clone(),
        stats: world.get::<Stats>(entity)?.clone(),
        resist: world.get::<Resist>(entity).cloned().unwrap_or_default(),
        effects: world.get::<Effects>(entity).cloned().unwrap_or_default(),
        sources: sources(world, entity),
        buckets: world
            .get::<AilmentBuckets>(entity)
            .cloned()
            .unwrap_or_default(),
        tags: world.get::<Tags>(entity).cloned().unwrap_or_default(),
        abilities: world.get::<Abilities>(entity).cloned(),
        knockback: world.get::<Knockback>(entity).cloned(),
        hitbox,
        hit,
        touches,
        held: troll::held(world, entity).and_then(id_of),
    })
}

/// The player's part, or `None` while it is dead.
fn capture_player(world: &World, player: Entity) -> Option<PlayerSave> {
    let health = world.get::<Health>(player)?;
    if health.is_dead() {
        return None;
    }
    Some(PlayerSave {
        id: world.get::<SaveId>(player)?.0,
        position: world.get::<Position>(player)?.0.to_array(),
        velocity: world.get::<Velocity>(player)?.0.to_array(),
        controller: world.get::<PlatformerController>(player)?.clone(),
        health: health.clone(),
        stats: world.get::<Stats>(player)?.clone(),
        effects: world.get::<Effects>(player)?.clone(),
        sources: sources(world, player),
        abilities: world.get::<Abilities>(player)?.clone(),
        buckets: world
            .get::<AilmentBuckets>(player)
            .cloned()
            .unwrap_or_default(),
        tags: world.get::<Tags>(player)?.clone(),
        reload: world.get::<Reload>(player).map(|reload| reload.left),
    })
}

/// A save's line on its slot row: "Area · HP current / max · Mana current
/// / max", the area left out before the player names one.
#[must_use]
pub fn summary(save: &SaveGame) -> String {
    let health = &save.player.health;
    let stats = &save.player.stats;
    let mana = stats.base("mana").unwrap_or(0.0).floor();
    let most = stats.get("max_mana").unwrap_or(0.0);
    let meters = format!(
        "HP {} / {} · Mana {mana} / {most}",
        crate::features::spells::hud::whole_health(Some(health)),
        health.max()
    );
    match &save.area {
        Some(area) => format!("{area} · {meters}"),
        None => meters,
    }
}

/// Takes `save` back into the run gameplay has just spawned, its map and
/// `player` at the save's point: the map's objects it names gone taken off,
/// each saved state over the map object with its save ID, the player's
/// over the player, and each dynamic entity spawned from its prefab at its
/// saved point with its saved components over the prefab's; then every
/// effect's source named again, once every entity is back. Every restored
/// point writes `Position` and `PrevPosition` together, so nothing draws
/// sliding there.
pub fn restore(world: &mut World, player: Entity, save: &SaveGame) {
    let mut by_id: std::collections::HashMap<u64, Entity> = world
        .query::<(Entity, &SaveId)>()
        .map(|(entity, id)| (id.0, entity))
        .collect();
    for id in &save.gone {
        if let Some(entity) = by_id.remove(id) {
            crate::shared::despawn_tree(world, entity);
        }
    }
    let p = &save.player;
    place(world, player, p.position);
    world.insert_bundle(
        player,
        (
            SaveId(p.id),
            Velocity(Vec2::from(p.velocity)),
            p.controller.clone(),
            p.health.clone(),
            p.stats.clone(),
            p.effects.clone(),
            p.abilities.clone(),
            p.buckets.clone(),
            p.tags.clone(),
        ),
    );
    let lantern = Lantern {
        on: save.lantern,
        guttered: save.lantern_guttered,
    };
    world.insert_bundle(player, (Spellbook { slot: save.spell }, lantern));
    if let Some(left) = p.reload {
        world.insert(player, Reload { left });
    }
    for platform in &save.platforms {
        if let Some(&entity) = by_id.get(&platform.id) {
            place(world, entity, platform.position);
            world.insert(entity, platform.follower.clone());
        }
    }
    for basin in &save.basins {
        if let Some(&entity) = by_id.get(&basin.id) {
            if let Some(left) = basin.crust {
                lava::harden(world, entity, left);
            }
            if let Some(lava) = world.get_mut::<Lava>(entity) {
                lava.cold = basin.cold;
            }
        }
    }
    for saved in &save.elevators {
        let Some(&entity) = by_id.get(&saved.id) else {
            continue;
        };
        place(world, entity, saved.position);
        if let Some(lift) = world.get_mut::<Elevator>(entity) {
            (lift.stop, lift.board, lift.idle) = (saved.stop, saved.board, saved.idle);
            (lift.ridden, lift.held) = (saved.ridden, saved.held);
        }
        match &saved.follower {
            Some(follower) => world.insert(entity, follower.clone()),
            None => {
                world.remove_component::<PathFollower>(entity);
            }
        }
    }
    for hazard in &save.hazards {
        if let Some(&entity) = by_id.get(&hazard.id) {
            place(world, entity, hazard.position);
            world.insert(entity, hazard.oscillator.clone());
            if let Some(left) = hazard.doused {
                crate::features::elements::douse_hazard(world, entity);
                if let Some(doused) = world.get_mut::<Doused>(entity) {
                    doused.left = left;
                }
            }
        }
    }
    for saved in &save.dummies {
        let entity = match by_id.get(&saved.id) {
            Some(&entity) if saved.map && world.has::<Dummy>(entity) => entity,
            _ => {
                let entity = dummy::spawn(world, Vec2::from(saved.position));
                by_id.insert(saved.id, entity);
                entity
            }
        };
        place(world, entity, saved.position);
        world.insert_bundle(
            entity,
            (
                SaveId(saved.id),
                Velocity(Vec2::from(saved.velocity)),
                saved.health.clone(),
                saved.resist.clone(),
                saved.effects.clone(),
                saved.buckets.clone(),
                saved.tags.clone(),
            ),
        );
    }
    for saved in &save.enemies {
        let entity = match by_id.get(&saved.id) {
            Some(&entity) if saved.map && world.has::<Enemy>(entity) => entity,
            _ => {
                let corner = Vec2::from(saved.position);
                let Some(entity) = enemies::spawn(world, saved.kind, corner, SaveId(saved.id))
                else {
                    continue;
                };
                by_id.insert(saved.id, entity);
                entity
            }
        };
        restore_enemy(world, entity, saved);
    }
    for ball in &save.balls {
        let entity = toys::restore_ball(world, ball.small, Vec2::from(ball.position));
        world.insert_bundle(
            entity,
            (SaveId(ball.id), Velocity(Vec2::from(ball.velocity))),
        );
        if let Some(hue) = ball.hue {
            world.insert(entity, hue);
        }
        if let Some(burn) = ball.burn {
            world.insert(entity, burn);
        }
        set_ice(world, entity, ball.ice);
    }
    for block in &save.blocks {
        let at = Vec2::from(block.position);
        let entity = match block.scrap {
            Some(variant) => toys::restore_scrap(world, at, variant),
            None => toys::restore_iron(world, at),
        };
        world.insert_bundle(
            entity,
            (SaveId(block.id), Velocity(Vec2::from(block.velocity))),
        );
        if let Some(amount) = block.chill {
            world.insert(
                entity,
                Chill {
                    amount,
                    held: false,
                },
            );
        }
        if let Some(left) = block.frozen {
            world.insert(entity, Frozen { left });
        }
    }
    for prop in &save.props {
        restore_prop(world, prop);
    }
    for orb in &save.orbs {
        drops::spawn(
            world,
            orb.kind,
            Vec2::from(orb.position),
            SaveId(orb.id),
            orb.left,
        );
    }
    for hole in &save.black_holes {
        let entity = toys::restore_hole(world, Vec2::from(hole.position));
        world.insert_bundle(entity, (SaveId(hole.id), BlackHole { left: hole.left }));
    }
    for missile in &save.missiles {
        restore_missile(
            world,
            missile,
            missile.source.and_then(|id| by_id.get(&id).copied()),
        );
    }
    for bullet in &save.bullets {
        let shot = BulletShot {
            id: SaveId(bullet.id),
            centre: Vec2::from(bullet.position) + BULLET_CENTRE,
            motion: bullet.motion.clone(),
            projectile: Some(bullet.projectile.clone()),
            ricochets: bullet.ricochets,
        };
        let source = bullet.source.and_then(|id| by_id.get(&id).copied());
        gun::spawn(world, shot, source);
    }
    let by_id: std::collections::HashMap<u64, Entity> = world
        .query::<(Entity, &SaveId)>()
        .map(|(entity, id)| (id.0, entity))
        .collect();
    let named = std::iter::once((player, &p.sources))
        .chain(
            save.dummies
                .iter()
                .filter_map(|saved| Some((*by_id.get(&saved.id)?, &saved.sources))),
        )
        .chain(
            save.enemies
                .iter()
                .filter_map(|saved| Some((*by_id.get(&saved.id)?, &saved.sources))),
        );
    let named: Vec<(Entity, Vec<(u64, u64)>)> = named
        .map(|(entity, sources)| (entity, sources.clone()))
        .collect();
    for (entity, sources) in named {
        name_sources(world, entity, &sources, &by_id);
    }
    for saved in &save.enemies {
        if let Some(&entity) = by_id.get(&saved.id) {
            name_targets(world, entity, saved, &by_id);
        }
    }
    // Each troll's hold again, the object put back at its hands.
    for saved in &save.enemies {
        let held = saved.held.and_then(|id| by_id.get(&id).copied());
        if let (Some(&entity), Some(held)) = (by_id.get(&saved.id), held) {
            troll::pick_up(world, entity, held);
        }
    }
    let throws = save
        .throws
        .iter()
        .filter_map(|throw| {
            Some(Throw {
                body: *by_id.get(&throw.body)?,
                thrower: throw.thrower,
                factor: throw.factor,
                left: throw.left,
            })
        })
        .collect();
    world.insert_resource(Throws(throws));
    world.insert_resource(save.cascades.clone());
    world.insert_resource(Summon {
        selected: save.summon,
    });
    world.insert_resource(save.rng.clone());
    world.insert_resource(SaveIds::starting_at(save.next_id));
}

/// A saved crate, keg, bomb or rock, spawned at once at its saved point
/// with its state over its prefab's: before the load names its held and
/// thrown bodies by save ID.
fn restore_prop(world: &mut World, saved: &PropSave) {
    let at = Vec2::from(saved.position);
    let entity = props::restore(world, saved.kind, at, SaveId(saved.id), saved.variant);
    world.insert(entity, Velocity(Vec2::from(saved.velocity)));
    if let Some(amount) = saved.chill {
        world.insert(
            entity,
            Chill {
                amount,
                held: false,
            },
        );
    }
    if let Some(left) = saved.frozen {
        world.insert(entity, Frozen { left });
    }
    if let Some(fuel) = saved.fuel {
        world.insert(entity, fuel);
    }
    match saved.kind {
        PropKind::Crate | PropKind::Rock => {}
        PropKind::Keg => world.insert(
            entity,
            Keg {
                fuse: saved.fuse,
                lighter: saved.lighter,
                doused: saved.doused,
            },
        ),
        PropKind::Bomb => world.insert(
            entity,
            Bomb {
                fuse: saved.fuse,
                lighter: saved.lighter,
            },
        ),
    }
}

/// Puts a saved enemy's state over `entity`: its point, its driver or
/// drivers, its machine and the rest; its targets wait for every entity to
/// be back ([`name_targets`]).
fn restore_enemy(world: &mut World, entity: Entity, saved: &EnemySave) {
    place(world, entity, saved.position);
    world.insert_bundle(
        entity,
        (
            SaveId(saved.id),
            Velocity(Vec2::from(saved.velocity)),
            saved.machine.clone(),
            saved.enemy,
            saved.brain,
            saved.sight,
            saved.health.clone(),
            saved.stats.clone(),
            saved.resist.clone(),
            saved.effects.clone(),
            saved.buckets.clone(),
            saved.tags.clone(),
        ),
    );
    match &saved.controller {
        Some(controller) => {
            world.insert(entity, controller.clone());
            enemies::walk_on_rock(world, entity);
        }
        None => {
            world.remove_component::<PlatformerController>(entity);
        }
    }
    match &saved.mover {
        Some(mover) => world.insert(entity, mover.clone()),
        None => {
            world.remove_component::<TopDownMover>(entity);
        }
    }
    if let Some(mover) = &saved.displaced {
        world.insert(entity, Displaced(mover.clone()));
    }
    if let Some(abilities) = &saved.abilities {
        world.insert(entity, abilities.clone());
    }
    if let Some(knockback) = &saved.knockback {
        world.insert(entity, knockback.clone());
    }
    if let Some(hitbox) = &saved.hitbox {
        world.insert(entity, hitbox.clone());
    }
}

/// Names a loaded enemy's targets again from their save IDs: its hitbox's
/// activation's, so it hits none twice, and its touches', so each keeps its
/// schedule.
fn name_targets(
    world: &mut World,
    entity: Entity,
    saved: &EnemySave,
    by_id: &std::collections::HashMap<u64, Entity>,
) {
    if let Some(hitbox) = world.get_mut::<Hitbox>(entity) {
        hitbox.set_hit_targets(saved.hit.iter().filter_map(|id| by_id.get(id).copied()));
    }
    if let Some(contact) = world.get_mut::<ContactDamage>(entity) {
        contact.set_touches(
            saved
                .touches
                .iter()
                .filter_map(|&(id, hit, met)| Some(ContactTouch::new(*by_id.get(&id)?, hit, met))),
        );
    }
}

/// Names each of `entity`'s effect instances' source again, from its saved
/// serial and the source's save ID.
fn name_sources(
    world: &mut World,
    entity: Entity,
    sources: &[(u64, u64)],
    by_id: &std::collections::HashMap<u64, Entity>,
) {
    let Some(effects) = world.get_mut::<Effects>(entity) else {
        return;
    };
    for &(serial, source) in sources {
        if let Some(&source) = by_id.get(&source) {
            effects.set_source(serial, Some(source));
        }
    }
}

/// Puts `entity`'s point at `position`, its last step's with it.
fn place(world: &mut World, entity: Entity, position: [f32; 2]) {
    let at = Vec2::from(position);
    if let Some(point) = world.get_mut::<Position>(entity) {
        point.0 = at;
    }
    if let Some(prev) = world.get_mut::<PrevPosition>(entity) {
        prev.0 = at;
    }
    if let Some(transform) = world.get_mut::<Transform>(entity) {
        transform.position = at;
    }
}

/// A saved missile, spawned at the frame's flush from what fired it, as
/// the spell's payload spawned it, so its source and its emitter return:
/// its save ID, flight and look are its own, so the spell's
/// `missile_adopt` leaves it be.
fn restore_missile(world: &mut World, missile: &MissileSave, source: Option<Entity>) {
    let at = Vec2::from(missile.position);
    let Ok(prepared) = world.prepare_prefab(toys::FIRE_MISSILE_PREFAB, at.to_array()) else {
        return;
    };
    let transform = Transform {
        position: at,
        rotation: 0.0,
        scale: Vec2::splat(MISSILE_SIZE / 64.0),
    };
    let prepared = prepared.with((
        transform,
        Position(at),
        PrevPosition(at),
        SaveId(missile.id),
        missile.projectile.clone(),
        missile.motion.clone(),
        toys::components::PreviousVelocity::default(),
        Visibility { visible: true },
        SceneEntity { state_id: GAMEPLAY },
    ));
    let commands = world.resource_mut::<CommandBuffer>();
    match source {
        Some(source) => {
            commands.spawn_prefab_from(source, prepared);
        }
        None => {
            commands.spawn_prefab(prepared);
        }
    }
}
