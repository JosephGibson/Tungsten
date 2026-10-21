//! What a save holds, part of the seam the shell calls: [`SaveGame`], how
//! a live run becomes one ([`capture`]), the slot row's line ([`summary`])
//! and how a loaded run takes it back ([`restore`]). Schema 4 counts on
//! from the template sample's 3, so a sample save reads as damaged, never
//! as a newer version's.
//!
//! Every saved entity carries a [`SaveId`]: the map's objects take theirs
//! in the order the map spawns them, which a load repeats, and whatever
//! spawns later takes the run's next, which the save keeps. A save names
//! another entity by its save ID, never by its entity. Presentation is not
//! saved: particles, lights, glows, flames, animation and facing, flashes,
//! shake, toasts, the HUD and the music.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::save::Value;
use tungsten::core::{
    CommandBuffer, Entity, Position, PrevPosition, SaveData, Transform, Velocity, Visibility, World,
};
use tungsten::kit::{
    Abilities, Effects, Health, KitRng, LinearMotion, Oscillator, PathFollower,
    PlatformerController, Projectile, Source, Stats, Tags,
};

use crate::features::elements::components::{BallBurn, Chill, Frozen};
use crate::features::player::components::Lantern;
use crate::features::run::Run;
use crate::features::spells::components::Spellbook;
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
}

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and extend `migrate`.
    const SCHEMA: u32 = 4;

    /// Schemas 1 to 3 are the template sample's: none migrates.
    fn migrate(from: u32, data: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let _ = data;
        Err(format!("no migration from schema {from}: not this game's save").into())
    }
}

/// The player: its body's point and velocity, its controller, health,
/// stats, effects, abilities and tags. The tags hold what an effect grants,
/// so a save during `casting` loads with the mana still paused.
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
    pub abilities: Abilities,
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
/// oscillator adds its change to the point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HazardSave {
    pub id: u64,
    pub position: [f32; 2],
    pub oscillator: Oscillator,
}

/// A small ball or a bowling ball, its hue and its fire. A full pit holds
/// 12,000, so each is one flat array in the save: `[id, small, x, y, vx,
/// vy, hue, hue speed, burn left]`, the last three `null` when it has none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "BallRecord", into = "BallRecord")]
pub struct BallSave {
    pub id: u64,
    pub small: bool,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub hue: Option<BallHue>,
    pub burn: Option<BallBurn>,
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
);

impl From<BallRecord> for BallSave {
    fn from((id, small, x, y, vx, vy, hue, speed, burn): BallRecord) -> Self {
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
        )
    }
}

/// An iron block, or a scrap of one by its variant, and its frost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockSave {
    pub id: u64,
    pub scrap: Option<usize>,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub chill: Option<f32>,
    pub frozen: bool,
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
        .query::<(&SaveId, &Position, &Oscillator)>()
        .map(|(id, at, oscillator)| HazardSave {
            id: id.0,
            position: at.0.to_array(),
            oscillator: oscillator.clone(),
        })
        .collect();
    hazards.sort_by_key(|hazard| hazard.id);
    let mut balls: Vec<BallSave> = world
        .query::<(
            &SaveId,
            &Ball,
            &Position,
            &Velocity,
            Option<&SmallBall>,
            Option<&BallHue>,
            Option<&BallBurn>,
        )>()
        .map(|(id, _, at, velocity, small, hue, burn)| BallSave {
            id: id.0,
            small: small.is_some(),
            position: at.0.to_array(),
            velocity: velocity.0.to_array(),
            hue: hue.copied(),
            burn: burn.copied(),
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
            frozen: frozen.is_some(),
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
        abilities: world.get::<Abilities>(player)?.clone(),
        tags: world.get::<Tags>(player)?.clone(),
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
/// `player` at the save's point: each saved state over the map object with
/// its save ID, the player's over the player, and each dynamic entity
/// spawned from its prefab at its saved point with its saved components
/// over the prefab's. Every restored point writes `Position` and
/// `PrevPosition` together, so nothing draws sliding there.
pub fn restore(world: &mut World, player: Entity, save: &SaveGame) {
    let by_id: std::collections::HashMap<u64, Entity> = world
        .query::<(Entity, &SaveId)>()
        .map(|(entity, id)| (id.0, entity))
        .collect();
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
            p.tags.clone(),
        ),
    );
    world.insert_bundle(
        player,
        (Spellbook { slot: save.spell }, Lantern { on: save.lantern }),
    );
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
        }
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
        if block.frozen {
            world.insert(entity, Frozen);
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
    world.insert_resource(save.rng.clone());
    world.insert_resource(SaveIds::starting_at(save.next_id));
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
