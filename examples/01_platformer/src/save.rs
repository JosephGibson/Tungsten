//! What a save holds, part of the seam the shell calls: [`SaveGame`], how
//! a live run becomes one ([`capture`]), the slot row's line ([`summary`])
//! and how a loaded run takes it back ([`restore`]). Schema 5 counts on
//! from the template sample's 3, so a sample save reads as damaged, never
//! as a newer version's; schema 4, 01 (a)'s, migrates.
//!
//! Every saved entity carries a [`SaveId`]: the map's objects take theirs
//! in the order the map spawns them, which a load repeats, and whatever
//! spawns later takes the run's next, which the save keeps. A save names
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
    Abilities, AilmentBuckets, Effects, Health, KitRng, LinearMotion, Oscillator, PathFollower,
    PlatformerController, Projectile, Resist, Source, Stats, Tags,
};

use crate::features::combat::components::{Hints, Reaction};
use crate::features::dummy::{self, components::Dummy};
use crate::features::elements::cascade::Cascades;
use crate::features::elements::components::{BallBurn, Chill, Frozen};
use crate::features::gun::components::{Bullet, Reload};
use crate::features::gun::{self, BULLET_CENTRE, BulletShot};
use crate::features::level::components::Doused;
use crate::features::player::components::Lantern;
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
    pub player: PlayerSave,
    pub platforms: Vec<PlatformSave>,
    pub hazards: Vec<HazardSave>,
    pub balls: Vec<BallSave>,
    pub blocks: Vec<BlockSave>,
    pub black_hole: Option<HoleSave>,
    pub missiles: Vec<MissileSave>,
    pub bullets: Vec<BulletSave>,
    pub dummies: Vec<DummySave>,
    /// The map's objects the run removed, by save ID: its placed dummies no
    /// longer alive, which a load takes off the map.
    pub gone: Vec<u64>,
    /// The element endings waiting, whole.
    pub cascades: Cascades,
    /// The reactions whose hints the save has shown, in order.
    pub hints: Vec<Reaction>,
    /// The summon slot's type, by its place in `SUMMONS`.
    pub summon: usize,
}

/// The dummies the map's `dummies` layer places: schema 5 added the layer,
/// the map's last, whose objects take the save IDs schema 4's player and
/// later spawns had.
pub const MAP_DUMMIES: u64 = 1;
/// The player prefab's `move_speed`, its run speed, which schema 5 added.
pub const MOVE_SPEED: f32 = 560.0;
/// A freeze's seconds, which schema 4's frozen block takes whole.
const FROZEN_SECS: f32 = crate::features::elements::FROZEN_SECS;

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and extend `migrate`.
    const SCHEMA: u32 = 5;

    /// Schema 4, 01 (a)'s, migrates ([`migrate_four`]); schemas 1 to 3 are
    /// the template sample's, and none migrates.
    fn migrate(from: u32, data: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        match from {
            4 => migrate_four(data),
            _ => Err(format!("no migration from schema {from}: not this game's save").into()),
        }
    }
}

/// Moves `id` up by `by` when it is a save ID at or above `from`.
fn shift_id(id: &mut Value, from: u64, by: u64) {
    if let Some(value) = id.as_u64().filter(|&value| value >= from) {
        *id = Value::from(value + by);
    }
}

/// Schema 4 into schema 5. The map's new `dummies` layer takes the save IDs
/// from schema 4's player's on, so every save ID at or above it moves up by
/// [`MAP_DUMMIES`]: the next ID, the player, the balls, the blocks, the hole
/// and the missiles with their sources; the map's earlier objects keep
/// theirs. A frozen block, frozen for good in schema 4, has a whole freeze
/// left; each ball gains no ice and each hazard no douse. The new lists
/// start empty, so the placed dummy loads as the map spawns it, and the
/// cascades wait on nothing. A restore puts the saved stats and abilities
/// over the prefab's, so the player gains what the prefab gained, every
/// saved value kept: its `move_speed` stat and the `lightning`, `gun` and
/// `summon` abilities, ready. No hint has shown, and the summon slot holds
/// its first type.
fn migrate_four(mut data: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let player = data["player"]["id"]
        .as_u64()
        .ok_or("schema 4: the player has no save ID")?;
    shift_id(&mut data["next_id"], player, MAP_DUMMIES);
    shift_id(&mut data["player"]["id"], player, MAP_DUMMIES);
    for ball in data["balls"].as_array_mut().into_iter().flatten() {
        shift_id(&mut ball[0], player, MAP_DUMMIES);
        if let Some(record) = ball.as_array_mut() {
            record.push(Value::Null);
        }
    }
    for block in data["blocks"].as_array_mut().into_iter().flatten() {
        shift_id(&mut block["id"], player, MAP_DUMMIES);
        let frozen = block["frozen"].as_bool().unwrap_or(false);
        block["frozen"] = if frozen {
            Value::from(FROZEN_SECS)
        } else {
            Value::Null
        };
    }
    for hazard in data["hazards"].as_array_mut().into_iter().flatten() {
        if let Some(hazard) = hazard.as_object_mut() {
            hazard.insert("doused".into(), Value::Null);
        }
    }
    if data["black_hole"].is_object() {
        shift_id(&mut data["black_hole"]["id"], player, MAP_DUMMIES);
    }
    for missile in data["missiles"].as_array_mut().into_iter().flatten() {
        shift_id(&mut missile["id"], player, MAP_DUMMIES);
        shift_id(&mut missile["source"], player, MAP_DUMMIES);
    }
    let save = data.as_object_mut().ok_or("schema 4: not an object")?;
    save.insert("dummies".into(), Value::Array(Vec::new()));
    save.insert("bullets".into(), Value::Array(Vec::new()));
    save.insert("gone".into(), Value::Array(Vec::new()));
    let cascades = [
        ("step", Value::from(0)),
        ("strike", Value::from(0)),
        ("entries", Value::Array(Vec::new())),
    ];
    save.insert("cascades".into(), Value::from_iter(cascades));
    save.insert("hints".into(), Value::Array(Vec::new()));
    save.insert("summon".into(), Value::from(0));
    let saved = data["player"]
        .as_object_mut()
        .ok_or("schema 4: no player")?;
    saved.insert(
        "buckets".into(),
        Value::from_iter(Vec::<(String, Value)>::new()),
    );
    saved.insert("sources".into(), Value::Array(Vec::new()));
    let stats = data["player"]["stats"]
        .as_object_mut()
        .ok_or("schema 4: the player has no stats")?;
    if !stats.contains_key(Stats::MOVE_SPEED) {
        let speed = [
            ("base", Value::from(MOVE_SPEED)),
            ("min", Value::from(0.0)),
            ("max", Value::Null),
        ];
        stats.insert(Stats::MOVE_SPEED.into(), Value::from_iter(speed));
    }
    grant_ability(&mut data, "lightning", None)?;
    grant_ability(&mut data, "gun", None)?;
    grant_ability(&mut data, "summon", None)?;
    Ok(data)
}

/// Gives the saved player the ability `name` when it has none of that
/// name: ready, with no cooldown and `charges` full.
fn grant_ability(
    data: &mut Value,
    name: &str,
    charges: Option<u32>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let entries = data["player"]["abilities"]["entries"]
        .as_array_mut()
        .ok_or("schema 4: the player has no abilities")?;
    if entries.iter().any(|entry| entry["name"] == name) {
        return Ok(());
    }
    let entry = [
        ("name", Value::from(name)),
        ("ability", Value::from(name)),
        ("cooldown_left", Value::from(0.0)),
        ("charges", charges.map_or(Value::Null, Value::from)),
        ("refill_left", Value::from(0.0)),
        ("interval_left", Value::from(0.0)),
    ];
    entries.push(Value::from_iter(entry));
    Ok(())
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
    /// The seconds the gun's reload has left; none while the magazine holds
    /// a round.
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

/// A moving platform: where it is and where its path has it going.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformSave {
    pub id: u64,
    pub position: [f32; 2],
    pub follower: PathFollower,
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
/// it has none. Its ice is a chill in `(0, 1)`, or 1 plus the seconds a
/// freeze has left.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "BallRecord", into = "BallRecord")]
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

/// The black hole: where it is open and for how long yet.
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

/// The live run as a save; `None` with no run or a dead player.
#[must_use]
pub fn capture(world: &World) -> Option<SaveGame> {
    let meta = shell::run_meta(world)?;
    let run = world.get_resource::<Run>()?;
    let player = capture_player(world, run.player)?;
    let id_of = |entity: Entity| world.get::<SaveId>(entity).map(|id| id.0);
    let mut platforms: Vec<PlatformSave> = world
        .query::<(&SaveId, &Position, &PathFollower)>()
        .map(|(id, at, follower)| PlatformSave {
            id: id.0,
            position: at.0.to_array(),
            follower: follower.clone(),
        })
        .collect();
    platforms.sort_by_key(|platform| platform.id);
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
    let black_hole = world
        .query::<(&SaveId, &Transform, &BlackHole)>()
        .map(|(id, at, hole)| HoleSave {
            id: id.0,
            position: at.position.to_array(),
            left: hole.left,
        })
        .min_by_key(|hole| hole.id);
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
    let dummies = dummy::live(world)
        .into_iter()
        .filter_map(|(id, entity)| capture_dummy(world, id, entity))
        .collect();
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
        player,
        platforms,
        hazards,
        balls,
        blocks,
        black_hole,
        missiles,
        bullets,
        dummies,
        gone: dummy::gone(world),
        cascades: world
            .get_resource::<Cascades>()
            .cloned()
            .unwrap_or_default(),
        hints: world
            .get_resource::<Hints>()
            .map(|hints| hints.seen.clone())
            .unwrap_or_default(),
        summon: world
            .get_resource::<Summon>()
            .map_or(0, |summon| summon.selected),
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
        health.current(),
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
    world.insert_bundle(
        player,
        (Spellbook { slot: save.spell }, Lantern { on: save.lantern }),
    );
    if let Some(left) = p.reload {
        world.insert(player, Reload { left });
    }
    for platform in &save.platforms {
        if let Some(&entity) = by_id.get(&platform.id) {
            place(world, entity, platform.position);
            world.insert(entity, platform.follower.clone());
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
    if let Some(hole) = &save.black_hole {
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
    let named = std::iter::once((player, &p.sources)).chain(
        save.dummies
            .iter()
            .filter_map(|saved| Some((*by_id.get(&saved.id)?, &saved.sources))),
    );
    let named: Vec<(Entity, Vec<(u64, u64)>)> = named
        .map(|(entity, sources)| (entity, sources.clone()))
        .collect();
    for (entity, sources) in named {
        name_sources(world, entity, &sources, &by_id);
    }
    world.insert_resource(save.cascades.clone());
    world.insert_resource(Hints {
        seen: save.hints.clone(),
    });
    world.insert_resource(Summon {
        selected: save.summon,
    });
    world.insert_resource(save.rng.clone());
    world.insert_resource(SaveIds::starting_at(save.next_id));
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
