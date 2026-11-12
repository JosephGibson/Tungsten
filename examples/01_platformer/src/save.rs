//! What a save holds, part of the seam the shell calls: [`SaveGame`], how
//! a live run becomes one ([`capture`]), the slot row's line ([`summary`])
//! and how a loaded run takes it back ([`restore`]). The schema counts on
//! from the template sample's 3, so a sample save reads as an older
//! version's, never as a newer one's.
//!
//! The run is the engine's `WorldSave` (`D-230`) beside 01's records of its
//! own spawns. A run's start names the player, global, and the map's saved
//! objects (its lava basins, decks, the elevator, the dummy, enemies, placed
//! orbs, fire hazards and geysers) in the map's area, once its setup is
//! done, so a capture writes each one's changes over its fresh spawn: their
//! state is the components and resources 01 and the kit register, chemistry's
//! queue among them. The kit's spawns from a saved source (the gun's bullets,
//! fire missiles and rolled orbs) are the engine's records, whole. 01's own
//! spawns (summoned dummies and enemies, the pit's, the balls, iron and its
//! scraps, black holes, its own missiles, flasks and bullets, the props,
//! planks, stones and snowballs) carry a kept identity, `Saved::kept` of the
//! run's counter, which the engine's references name, and 01's records hold
//! them. Schema 28 moves the run onto the engine, so `migrate_step` refuses
//! every older save, which reads as an older version's (`D-204`).
//!
//! A save names another entity by its save ID, never by its entity: an
//! effect's source too, which a load names again once every entity is
//! back. Presentation is not saved: particles, lights, glows, flames,
//! overlays and tints, animation and facing, flashes, shake, toasts, the
//! HUD and the music.

use std::collections::HashMap;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::save::{RestoreError, WorldSave, capture_world, check_world, restore_world};
use tungsten::core::{
    CameraController, CommandBuffer, DataRef, Entity, Position, PrevPosition, SaveData, Transform,
    Velocity, Visibility, World,
};
use tungsten::kit::{
    Abilities, ContactDamage, ContactTouch, EffectRequest, Effects, Health, Hitbox, Knockback,
    LinearMotion, PlatformerController, Projectile, Resist, Source, StateMachine, Stats, Tags,
    TopDownMover,
};

use crate::features::dummy;
use crate::features::elements::systems::{coat_of, freeze_for, light, show_douse};
use crate::features::elements::{COAT, burn_left, ice_left};
use crate::features::enemies::components::{
    Brain, Displaced, Enemy, EnemyKind, EnemyState, Held, Holding, Sight, Snowball, Throws,
};
use crate::features::enemies::{self, snowball, troll};
use crate::features::gun::components::Bullet;
use crate::features::gun::{self, BULLET_CENTRE, BulletShot};
use crate::features::level::components::Hazard;
use crate::features::level::geyser::{self, Geyser};
use crate::features::level::lava::{self, Lava};
use crate::features::props::{
    self,
    components::{Bomb, Crate, Keg, MapProp, Plank, PropFire, PropKind, Rock, Stone, StoneSize},
};
use crate::features::run::Run;
use crate::features::tomes::{Tome, Tomes};
use crate::features::toys::components::{
    Ball, BallHue, BlackHole, FireMissile, IronBlock, IronScrap, PreviousVelocity, SmallBall,
    ToxicFlask,
};
use crate::features::toys::{self, MISSILE_SIZE};
use crate::game::GAMEPLAY;
use crate::shared::{Joined, SaveId, Saved, despawn_tree, save_id};
use crate::shell::{self, RunMeta};

/// A save: the shell's run data, the last area the player entered, the
/// slot row's numbers, the engine's save of the run's World and the records
/// of 01's own spawns, each list in save-ID order.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaveGame {
    pub meta: RunMeta,
    /// The last area the player entered, which a load does not announce.
    pub area: Option<String>,
    /// The slot row's numbers: the player's health, and its mana and its
    /// most.
    pub health: Health,
    pub mana: [f32; 2],
    /// The named objects' changes over their fresh spawn, the kit's spawns
    /// whole and the saved resources.
    pub world: WorldSave,
    pub dummies: Vec<DummySave>,
    pub enemies: Vec<EnemySave>,
    pub balls: Vec<BallSave>,
    pub blocks: Vec<BlockSave>,
    /// The open black holes.
    pub black_holes: Vec<HoleSave>,
    /// 01's own fire missiles in flight; the spell's are the engine's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missiles: Vec<MissileSave>,
    /// 01's own bullets in flight; the gun's are the engine's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bullets: Vec<BulletSave>,
    /// The crates, kegs, bombs and rocks.
    pub props: Vec<PropSave>,
    /// The crawlers' snowballs in flight, none written while none fly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snowballs: Vec<SnowballSave>,
    /// The broken crates' planks, none written while none lie about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub planks: Vec<PlankSave>,
    /// The hollow's rubble stones, none written while none lie about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stones: Vec<StoneSave>,
    /// The toxic flasks in flight, none written while none fly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flasks: Vec<FlaskSave>,
}

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and add its step to
    /// `migrate_step`. Schema 28 moves the run onto the engine's saves:
    /// the default `migrate_step` refuses every older schema, whose slot
    /// reads as an older version's (`D-204`).
    const SCHEMA: u32 = 28;
}

/// The dummies the map's `dummies` layer places.
pub const MAP_DUMMIES: u64 = 1;
/// The enemies the map's `enemies` layer places, the health orbs its
/// `pickups` layer places and the geysers its `geysers` layer places.
pub const MAP_ENEMIES: u64 = 36;
pub const MAP_PICKUPS: u64 = 8;
pub const MAP_GEYSERS: u64 = 3;

/// A summoned training dummy, spawned: its body, health, resistances,
/// effects with their sources (its statuses' builds among them) and tags,
/// which its statuses grant. The map's is named, the engine's to save.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DummySave {
    pub id: u64,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub health: Health,
    pub resist: Resist,
    pub effects: Effects,
    pub sources: Vec<(u64, u64)>,
    pub tags: Tags,
}

/// A summoned enemy, or one the pit spawns, spawned: its body, its driver
/// (a walker's controller or a flyer's mover, and a frozen flyer's mover
/// set aside beside the controller it falls on), its machine, what it
/// knows and sees, its health, stats, resistances, effects with their
/// sources (its statuses' builds among them), tags, abilities, knockback,
/// its hitbox with the targets its activation has hit and its touches'
/// times, by save ID, and what a troll holds. The map's are named, the
/// engine's to save.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemySave {
    pub id: u64,
    pub kind: EnemyKind,
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
    /// Its burn's seconds left, 0 once spent; none for one never lit.
    pub burn: Option<f32>,
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
            burn,
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
            ball.burn,
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

/// A body's coat as a save writes it, its `chill`'s intensity; none
/// without one.
fn chill_of(world: &World, entity: Entity) -> Option<f32> {
    let coat = coat_of(world, entity);
    (coat > 0.0).then_some(coat)
}

/// A body's ice as a save writes it: 1 plus the seconds its freeze (its
/// `ice`) has left, else its coat.
fn ice_of(world: &World, entity: Entity) -> Option<f32> {
    ice_left(world, entity)
        .map(|left| 1.0 + left.max(0.0))
        .or_else(|| chill_of(world, entity))
}

/// Puts a saved coat back on `entity`, as its `chill`'s intensity.
fn set_chill(world: &mut World, entity: Entity, amount: Option<f32>) {
    if let Some(amount) = amount.filter(|&amount| amount > 0.0) {
        EffectRequest::new(entity, DataRef::by_id(COAT))
            .with_amount(amount)
            .apply(world);
    }
}

/// Puts a saved freeze back on `entity`: its `ice` for the seconds `left`
/// had.
fn set_frozen(world: &mut World, entity: Entity, left: Option<f32>) {
    if let Some(left) = left {
        freeze_for(world, entity, left.max(0.0));
    }
}

/// Puts a saved ice back on `entity`.
fn set_ice(world: &mut World, entity: Entity, ice: Option<f32>) {
    match ice {
        Some(ice) if ice >= 1.0 => set_frozen(world, entity, Some(ice - 1.0)),
        amount => set_chill(world, entity, amount),
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

/// A toxic flask in flight (e01flask), as a missile is: its point, its
/// flight and its range so far, and the save ID of what threw it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlaskSave {
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

/// A crate, a keg, or a goblin's bomb or rock: its kind, whether the map
/// set it down ([`MapProp`]), a crate's or a rock's art, its body, its
/// frost and the seconds its freeze has left, a crate's fire, a keg's or a
/// bomb's lit fuse and the save ID its blast credits, and the seconds a
/// douse keeps a keg from lighting: the fire, the fuse and the douse their
/// statuses' (7.13a). A thrown rock's credit is its throw's (`throws`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropSave {
    pub id: u64,
    pub kind: PropKind,
    pub map: bool,
    pub variant: usize,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub chill: Option<f32>,
    pub frozen: Option<f32>,
    pub fuel: Option<PropFire>,
    pub fuse: Option<f32>,
    pub lighter: Option<u64>,
    pub doused: f32,
}

/// A crawler's snowball in flight: its body and its [`Snowball`]: its
/// thrower's save ID, the thrower's damage factor and its seconds left.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnowballSave {
    pub id: u64,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub thrower: Option<u64>,
    pub factor: f32,
    pub left: f32,
}

/// A broken crate's plank: its body, its art's variant, its frost and its
/// fire, whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlankSave {
    pub id: u64,
    pub variant: usize,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
    pub chill: Option<f32>,
    pub frozen: Option<f32>,
    pub fuel: PropFire,
}

/// A rubble stone: its size, its art's variant and its body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoneSave {
    pub id: u64,
    pub size: StoneSize,
    pub variant: usize,
    pub position: [f32; 2],
    pub velocity: [f32; 2],
}

/// Whether `entity` carries a kept identity: one of 01's own spawns, which
/// its records save, not a name's or an engine record's.
fn kept(world: &World, entity: Entity) -> bool {
    world
        .get::<Saved>(entity)
        .is_some_and(|saved| saved.prefab().is_none() && !saved.id().is_authored())
}

/// Each live kept entity `T` marks, by save ID.
fn kept_of<T: 'static>(world: &World) -> Vec<(u64, Entity)> {
    let mut found: Vec<(u64, Entity)> = world
        .query::<(Entity, &T, &Saved)>()
        .filter(|&(entity, ..)| kept(world, entity))
        .map(|(entity, _, saved)| (saved.id().0, entity))
        .collect();
    found.sort_unstable_by_key(|&(id, _)| id);
    found
}

/// The live run as a save; `None` with no run, a dead player, or a frame
/// whose commands are still queued, which the engine's capture refuses
/// with a warning (`D-197`).
#[must_use]
pub fn capture(world: &World) -> Option<SaveGame> {
    let meta = shell::run_meta(world)?;
    let run = world.get_resource::<Run>()?;
    let health = world.get::<Health>(run.player)?;
    if health.is_dead() {
        return None;
    }
    let stats = world.get::<Stats>(run.player)?;
    let mana = [
        stats.base("mana").unwrap_or(0.0),
        stats.get("max_mana").unwrap_or(0.0),
    ];
    let saved = capture_world(world)
        .inspect_err(|err| log::warn!("The run was not saved: {err}"))
        .ok()?;
    let id_of = |entity: Entity| save_id(world, entity).map(|id| id.0);
    let balls = kept_of::<Ball>(world)
        .into_iter()
        .filter_map(|(id, entity)| {
            Some(BallSave {
                id,
                small: world.has::<SmallBall>(entity),
                position: world.get::<Position>(entity)?.0.to_array(),
                velocity: world.get::<Velocity>(entity)?.0.to_array(),
                hue: world.get::<BallHue>(entity).copied(),
                burn: burn_left(world, entity),
                ice: ice_of(world, entity),
            })
        })
        .collect();
    let mut blocks: Vec<BlockSave> = kept_of::<IronBlock>(world)
        .into_iter()
        .chain(kept_of::<IronScrap>(world))
        .filter_map(|(id, entity)| {
            let frozen = ice_left(world, entity);
            Some(BlockSave {
                id,
                scrap: world.get::<IronScrap>(entity).map(|scrap| scrap.variant),
                position: world.get::<Position>(entity)?.0.to_array(),
                velocity: world.get::<Velocity>(entity)?.0.to_array(),
                chill: chill_of(world, entity).filter(|_| frozen.is_none()),
                frozen,
            })
        })
        .collect();
    blocks.sort_by_key(|block| block.id);
    let black_holes = kept_of::<BlackHole>(world)
        .into_iter()
        .filter_map(|(id, entity)| {
            Some(HoleSave {
                id,
                position: world.get::<Transform>(entity)?.position.to_array(),
                left: world.get::<BlackHole>(entity)?.left,
            })
        })
        .collect();
    let flight = |entity: Entity| {
        Some((
            world.get::<Position>(entity)?.0.to_array(),
            world.get::<Projectile>(entity)?.clone(),
            world.get::<LinearMotion>(entity)?.clone(),
            world
                .get::<Source>(entity)
                .and_then(|source| id_of(source.entity())),
        ))
    };
    let missiles = kept_of::<FireMissile>(world)
        .into_iter()
        .filter_map(|(id, entity)| {
            let (position, projectile, motion, source) = flight(entity)?;
            Some(MissileSave {
                id,
                position,
                projectile,
                motion,
                source,
            })
        })
        .collect();
    let flasks = kept_of::<ToxicFlask>(world)
        .into_iter()
        .filter_map(|(id, entity)| {
            let (position, projectile, motion, source) = flight(entity)?;
            Some(FlaskSave {
                id,
                position,
                projectile,
                motion,
                source,
            })
        })
        .collect();
    let bullets = kept_of::<Bullet>(world)
        .into_iter()
        .filter_map(|(id, entity)| {
            let (position, projectile, motion, source) = flight(entity)?;
            (!projectile.is_spent()).then_some(BulletSave {
                id,
                position,
                projectile,
                motion,
                ricochets: world.get::<Bullet>(entity)?.ricochets,
                source,
            })
        })
        .collect();
    let mut props: Vec<PropSave> = props::all(world)
        .into_iter()
        .filter(|&entity| kept(world, entity))
        .filter(|&entity| !world.has::<Plank>(entity) && !world.has::<Stone>(entity))
        .filter_map(|entity| capture_prop(world, entity))
        .collect();
    props.sort_by_key(|prop| prop.id);
    let planks = kept_of::<Plank>(world)
        .into_iter()
        .filter_map(|(id, entity)| {
            Some(PlankSave {
                id,
                variant: world.get::<Plank>(entity)?.variant,
                position: world.get::<Position>(entity)?.0.to_array(),
                velocity: world.get::<Velocity>(entity)?.0.to_array(),
                chill: chill_of(world, entity).filter(|_| ice_left(world, entity).is_none()),
                frozen: ice_left(world, entity),
                fuel: props::fire(world, entity)?,
            })
        })
        .collect();
    let stones = kept_of::<Stone>(world)
        .into_iter()
        .filter_map(|(id, entity)| {
            let stone = world.get::<Stone>(entity)?;
            Some(StoneSave {
                id,
                size: stone.size,
                variant: stone.variant,
                position: world.get::<Position>(entity)?.0.to_array(),
                velocity: world.get::<Velocity>(entity)?.0.to_array(),
            })
        })
        .collect();
    let snowballs = snowball::flying(world)
        .into_iter()
        .filter(|&(_, entity)| kept(world, entity))
        .filter_map(|(id, entity)| {
            let ball = world.get::<Snowball>(entity)?;
            Some(SnowballSave {
                id,
                position: world.get::<Position>(entity)?.0.to_array(),
                velocity: world.get::<Velocity>(entity)?.0.to_array(),
                thrower: ball.thrower,
                factor: ball.factor,
                left: ball.left,
            })
        })
        .collect();
    let dummies = dummy::live(world)
        .into_iter()
        .filter(|&(_, entity)| kept(world, entity))
        .filter_map(|(id, entity)| capture_dummy(world, id, entity))
        .collect();
    let enemies = enemies::live(world)
        .into_iter()
        .filter(|&(_, entity)| kept(world, entity))
        .filter_map(|(id, entity)| capture_enemy(world, id, entity))
        .collect();
    Some(SaveGame {
        meta,
        area: run.area.clone(),
        health: health.clone(),
        mana,
        world: saved,
        dummies,
        enemies,
        balls,
        blocks,
        black_holes,
        missiles,
        bullets,
        props,
        snowballs,
        planks,
        stones,
        flasks,
    })
}

/// A crate's, a keg's, a bomb's or a rock's part.
fn capture_prop(world: &World, entity: Entity) -> Option<PropSave> {
    let fused = world.has::<Keg>(entity) || world.has::<Bomb>(entity);
    let variant = world
        .get::<Crate>(entity)
        .map(|own| own.variant)
        .or_else(|| world.get::<Rock>(entity).map(|own| own.variant));
    Some(PropSave {
        id: save_id(world, entity)?.0,
        kind: props::kind(world, entity)?,
        map: world.has::<MapProp>(entity),
        variant: variant.unwrap_or(0),
        position: world.get::<Position>(entity)?.0.to_array(),
        velocity: world.get::<Velocity>(entity)?.0.to_array(),
        chill: chill_of(world, entity).filter(|_| ice_left(world, entity).is_none()),
        frozen: ice_left(world, entity),
        fuel: props::fire(world, entity),
        fuse: props::fuse_left(world, entity),
        lighter: props::lighter(world, entity).filter(|_| fused),
        doused: if world.has::<Keg>(entity) {
            props::doused_left(world, entity)
        } else {
            0.0
        },
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
                    let source = save_id(world, instance.source()?)?;
                    Some((instance.serial(), source.0))
                })
                .collect()
        })
}

/// A summoned dummy's part.
fn capture_dummy(world: &World, id: u64, entity: Entity) -> Option<DummySave> {
    Some(DummySave {
        id,
        position: world.get::<Position>(entity)?.0.to_array(),
        velocity: world.get::<Velocity>(entity)?.0.to_array(),
        health: world.get::<Health>(entity)?.clone(),
        resist: world.get::<Resist>(entity).cloned().unwrap_or_default(),
        effects: world.get::<Effects>(entity).cloned().unwrap_or_default(),
        sources: sources(world, entity),
        tags: world.get::<Tags>(entity).cloned().unwrap_or_default(),
    })
}

/// A summoned enemy's part.
fn capture_enemy(world: &World, id: u64, entity: Entity) -> Option<EnemySave> {
    let id_of = |entity: Entity| save_id(world, entity).map(|id| id.0);
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
        tags: world.get::<Tags>(entity).cloned().unwrap_or_default(),
        abilities: world.get::<Abilities>(entity).cloned(),
        knockback: world.get::<Knockback>(entity).cloned(),
        hitbox,
        hit,
        touches,
        held: troll::held(world, entity).and_then(id_of),
    })
}

/// A save's line on its slot row: "Area · HP current / max · Mana current
/// / max", the area left out before the player names one.
#[must_use]
pub fn summary(save: &SaveGame) -> String {
    let health = &save.health;
    let [mana, most] = save.mana;
    let meters = format!(
        "HP {} / {} · Mana {} / {most}",
        crate::features::spells::hud::whole_health(Some(health)),
        health.max(),
        mana.floor()
    );
    match &save.area {
        Some(area) => format!("{area} · {meters}"),
        None => meters,
    }
}

/// Takes `save` back into the run gameplay has just spawned and named, in
/// four passes, once the engine's checks pass (`check_world`), so a save
/// that cannot load stages nothing: none of 01's spawns, and none of the
/// queue entries their statuses would add. First 01's own spawns come back under their kept
/// identities, at their saved points with their saved state over their
/// prefabs', their statuses put back through 01's setters, sourceless where
/// the source is not back yet: before the engine's restore replaces
/// chemistry's queue and the request queues with the saved ones, so what a
/// status put back queues gives way to the saved entries. Then the engine
/// lays the named objects' changes over their fresh spawn, spawns its
/// records and puts the saved resources and every reference back
/// (`restore_world`), its report's lines logged. Then 01 names its own
/// spawns' references by save ID, applying nothing: their statuses'
/// sources, an enemy's targets, a troll's hold, and spawns its missiles,
/// flasks and bullets in flight from what fired them at the frame's flush.
/// Last it gives each restored entity what its spawn gives beyond its
/// saved state ([`rebuild`]). Every restored point writes `Position` and
/// `PrevPosition` together, so nothing draws sliding there.
///
/// # Errors
///
/// The engine's [`RestoreError`] for a save whose World does not restore,
/// found by the check before the first pass, with the World as it was.
pub fn restore(world: &mut World, save: &SaveGame) -> Result<(), RestoreError> {
    check_world(world, &save.world)?;
    // The run's own touches, which a geyser holds aside where no save
    // keeps them.
    let touches: Vec<(Entity, Option<ContactDamage>)> = world
        .query::<(Entity, &Geyser)>()
        .map(|(entity, state)| {
            let touch = world
                .get::<ContactDamage>(entity)
                .cloned()
                .or_else(|| state.touch.clone());
            (entity, touch)
        })
        .collect();
    let staged = spawn_own(world, save);
    let scene = (SceneEntity { state_id: GAMEPLAY }, Joined);
    match restore_world(world, &save.world, &scene) {
        Ok(report) => {
            for line in &report.dropped {
                log::warn!("The save's run: {line}");
            }
        }
        // The check passed, so this is a bug: what the first pass staged
        // goes, and the run starts fresh.
        Err(err) => {
            log::error!("A checked save failed to restore: {err}");
            for entity in staged {
                despawn_tree(world, entity);
            }
            return Err(err);
        }
    }
    name_own(world, save);
    rebuild(world, &touches);
    Ok(())
}

/// Each live saved entity by its save ID.
fn by_id(world: &World) -> HashMap<u64, Entity> {
    world
        .query::<(Entity, &Saved)>()
        .map(|(entity, saved)| (saved.id().0, entity))
        .collect()
}

/// The first pass: 01's own spawns back under their kept identities, their
/// statuses through 01's setters, a prop's fire and fuse credited to its
/// lighter where it is back. The entities it spawned.
fn spawn_own(world: &mut World, save: &SaveGame) -> Vec<Entity> {
    let mut staged = Vec::new();
    for saved in &save.dummies {
        let entity = dummy::spawn(world, Vec2::from(saved.position));
        place(world, entity, saved.position);
        world.insert_bundle(
            entity,
            (
                Saved::kept(SaveId(saved.id)),
                Velocity(Vec2::from(saved.velocity)),
                saved.health.clone(),
                saved.resist.clone(),
                saved.effects.clone(),
                saved.tags.clone(),
            ),
        );
        staged.push(entity);
    }
    for saved in &save.enemies {
        let corner = Vec2::from(saved.position);
        let (kind, variant, id) = (saved.kind, saved.enemy.variant, SaveId(saved.id));
        let Some(entity) = enemies::spawn_as(world, kind, variant, corner, id) else {
            continue;
        };
        restore_enemy(world, entity, saved);
        staged.push(entity);
    }
    for ball in &save.balls {
        let entity = toys::restore_ball(world, ball.small, Vec2::from(ball.position));
        world.insert_bundle(
            entity,
            (
                Saved::kept(SaveId(ball.id)),
                Velocity(Vec2::from(ball.velocity)),
            ),
        );
        if let Some(hue) = ball.hue {
            world.insert(entity, hue);
        }
        // A burn with seconds left lights on for them; a spent one keeps
        // its `kindled` alone.
        if let Some(left) = ball.burn {
            light(world, entity, left.max(0.0));
        }
        set_ice(world, entity, ball.ice);
        staged.push(entity);
    }
    for block in &save.blocks {
        let at = Vec2::from(block.position);
        let entity = match block.scrap {
            Some(variant) => toys::restore_scrap(world, at, variant),
            None => toys::restore_iron(world, at),
        };
        world.insert_bundle(
            entity,
            (
                Saved::kept(SaveId(block.id)),
                Velocity(Vec2::from(block.velocity)),
            ),
        );
        set_chill(world, entity, block.chill);
        set_frozen(world, entity, block.frozen);
        staged.push(entity);
    }
    for prop in &save.props {
        staged.push(restore_prop(world, prop));
    }
    for plank in &save.planks {
        staged.push(restore_plank(world, plank));
    }
    for stone in &save.stones {
        let at = Vec2::from(stone.position);
        let kind = PropKind::Stone(stone.size);
        let entity = props::restore(world, kind, at, SaveId(stone.id), stone.variant);
        world.insert(entity, Velocity(Vec2::from(stone.velocity)));
        staged.push(entity);
    }
    for saved in &save.snowballs {
        let ball = snowball::spawn(world, Vec2::from(saved.position));
        let flying = Snowball {
            thrower: saved.thrower,
            factor: saved.factor,
            left: saved.left,
        };
        world.insert_bundle(
            ball,
            (
                Saved::kept(SaveId(saved.id)),
                Velocity(Vec2::from(saved.velocity)),
                flying,
            ),
        );
        staged.push(ball);
    }
    for hole in &save.black_holes {
        let entity = toys::restore_hole(world, Vec2::from(hole.position));
        world.insert_bundle(
            entity,
            (Saved::kept(SaveId(hole.id)), BlackHole { left: hole.left }),
        );
        staged.push(entity);
    }
    relight(world, save, &by_id(world));
    staged
}

/// The third pass: 01's own spawns' references by save ID, against every
/// entity now back, applying nothing: each dummy's and enemy's effects'
/// sources, each enemy's targets, a summoned troll's hold and a prop's
/// lighter the first pass missed; then its missiles, flasks and bullets in
/// flight, spawned at the frame's flush from what fired them.
fn name_own(world: &mut World, save: &SaveGame) {
    let by_id = by_id(world);
    let named = save
        .dummies
        .iter()
        .map(|saved| (saved.id, &saved.sources))
        .chain(save.enemies.iter().map(|saved| (saved.id, &saved.sources)));
    let named: Vec<(Entity, Vec<(u64, u64)>)> = named
        .filter_map(|(id, sources)| Some((*by_id.get(&id)?, sources.clone())))
        .collect();
    for (entity, sources) in named {
        name_sources(world, entity, &sources, &by_id);
    }
    for saved in &save.enemies {
        if let Some(&entity) = by_id.get(&saved.id) {
            name_targets(world, entity, saved, &by_id);
            if let Some(&held) = saved.held.and_then(|id| by_id.get(&id)) {
                world.insert(entity, Holding(held));
            }
        }
    }
    let lighter = |id: Option<u64>| id.and_then(|id| by_id.get(&id).copied());
    for saved in &save.props {
        if let Some(&entity) = by_id.get(&saved.id) {
            let fire = saved.fuel.and_then(|fuel| lighter(fuel.lighter));
            props::credit(world, entity, fire, lighter(saved.lighter));
        }
    }
    for saved in &save.planks {
        if let Some(&entity) = by_id.get(&saved.id) {
            props::credit(world, entity, lighter(saved.fuel.lighter), None);
        }
    }
    for missile in &save.missiles {
        restore_missile(world, missile, lighter(missile.source));
    }
    for flask in &save.flasks {
        restore_flask(world, flask, lighter(flask.source));
    }
    for bullet in &save.bullets {
        let shot = BulletShot {
            id: SaveId(bullet.id),
            centre: Vec2::from(bullet.position) + BULLET_CENTRE,
            motion: bullet.motion.clone(),
            projectile: Some(bullet.projectile.clone()),
            ricochets: bullet.ricochets,
        };
        gun::spawn(world, shot, lighter(bullet.source));
    }
}

/// The last pass: what each restored entity's spawn gives beyond its saved
/// state, read off the restored state and never by applying it again: a
/// recorded spawn's `PreviousVelocity`; a crusted basin's body layers,
/// mask, touch and glow, a doused hazard's touch, glow and trail, a
/// geyser's touch on in an eruption and held aside else, from `touches`,
/// the run's, where the save holds none; a troll's hold lifted again; the
/// taken tomes' books off the map; the bodies the player stands in shoved;
/// and the camera on the player where the save left it.
fn rebuild(world: &mut World, touches: &[(Entity, Option<ContactDamage>)]) {
    let recorded: Vec<Entity> = world
        .query::<(Entity, &Saved)>()
        .filter(|(entity, saved)| {
            saved.prefab().is_some() && !world.has::<PreviousVelocity>(*entity)
        })
        .map(|(entity, _)| entity)
        .collect();
    for entity in recorded {
        world.insert(entity, PreviousVelocity::default());
    }
    let basins: Vec<Entity> = world
        .query::<(Entity, &Lava)>()
        .map(|(entity, _)| entity)
        .collect();
    for basin in basins {
        if lava::crusted(world, basin) {
            lava::show_crust(world, basin);
        }
    }
    let hazards: Vec<Entity> = world
        .query::<(Entity, &Hazard)>()
        .filter(|(_, hazard)| hazard.fire)
        .map(|(entity, _)| entity)
        .collect();
    for hazard in hazards {
        show_douse(world, hazard);
    }
    for (geyser, touch) in touches {
        geyser::restored(world, *geyser, touch.clone());
    }
    let holds: Vec<(Entity, Entity)> = world
        .query::<(Entity, &Holding)>()
        .map(|(troll, holding)| (troll, holding.0))
        .collect();
    for (troll, object) in holds {
        world.remove_component::<Holding>(troll);
        if world.is_alive(object) && !world.has::<Held>(object) {
            troll::pick_up(world, troll, object);
        }
    }
    if let Some(throws) = world.get_resource_mut::<Throws>() {
        throws.0.retain(|throw| throw.body != Entity::PLACEHOLDER);
    }
    let Some(player) = world.get_resource::<Run>().map(|run| run.player) else {
        return;
    };
    // The books the player holds leave the map, which spawned all three.
    if let Some(tomes) = world.get::<Tomes>(player).cloned() {
        let taken: Vec<Entity> = world
            .query::<(Entity, &Tome)>()
            .filter(|(_, tome)| tomes.0.contains(&tome.kind))
            .map(|(book, _)| book)
            .collect();
        for book in taken {
            despawn_tree(world, book);
        }
    }
    crate::features::player::shove_bodies(world, player);
    if let Some(centre) = crate::shared::body_centre(world, player)
        && let Some(camera) = world.get_resource_mut::<CameraController>()
    {
        camera.cut_to(centre);
    }
}

/// A saved crate, keg, bomb or rock, spawned at once at its saved point
/// with its state over its prefab's: before the load names its held and
/// thrown bodies by save ID. Its fire, fuse and douse wait for every
/// entity to be back ([`relight`]).
fn restore_prop(world: &mut World, saved: &PropSave) -> Entity {
    let at = Vec2::from(saved.position);
    let entity = props::restore(world, saved.kind, at, SaveId(saved.id), saved.variant);
    world.insert(entity, Velocity(Vec2::from(saved.velocity)));
    if saved.map {
        world.insert(entity, MapProp);
    }
    set_chill(world, entity, saved.chill);
    set_frozen(world, entity, saved.frozen);
    entity
}

/// A saved plank, spawned at once at its saved point with its state over
/// its prefab's, at rest from its tumble; its fire waits as a prop's does.
fn restore_plank(world: &mut World, saved: &PlankSave) -> Entity {
    let at = Vec2::from(saved.position);
    let entity = props::restore(world, PropKind::Plank, at, SaveId(saved.id), saved.variant);
    world.insert(entity, Velocity(Vec2::from(saved.velocity)));
    set_chill(world, entity, saved.chill);
    set_frozen(world, entity, saved.frozen);
    entity
}

/// The saved props' and planks' fires, fuses and douses put back quietly,
/// once 01's own spawns are: each status sourced to its lighter by save ID
/// where it is back, else credited at the third pass ([`props::credit`]).
fn relight(world: &mut World, save: &SaveGame, by_id: &HashMap<u64, Entity>) {
    let lighter = |id: Option<u64>| id.and_then(|id| by_id.get(&id).copied());
    for saved in &save.props {
        let Some(&entity) = by_id.get(&saved.id) else {
            continue;
        };
        if let Some(fire) = &saved.fuel {
            props::set_fire(world, entity, fire, lighter(fire.lighter));
        }
        if matches!(saved.kind, PropKind::Keg | PropKind::Bomb) {
            props::set_fuse(
                world,
                entity,
                saved.fuse,
                lighter(saved.lighter),
                saved.doused,
            );
        }
    }
    for saved in &save.planks {
        if let Some(&entity) = by_id.get(&saved.id) {
            props::set_fire(world, entity, &saved.fuel, lighter(saved.fuel.lighter));
        }
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
            Saved::kept(SaveId(saved.id)),
            Velocity(Vec2::from(saved.velocity)),
            saved.machine.clone(),
            saved.enemy,
            saved.brain,
            saved.sight,
            saved.health.clone(),
            saved.stats.clone(),
            saved.resist.clone(),
            saved.effects.clone(),
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
    by_id: &HashMap<u64, Entity>,
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
    by_id: &HashMap<u64, Entity>,
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
/// A saved flask set back in flight, from `source` when it lives, at the
/// load's flush, as a missile is.
fn restore_flask(world: &mut World, flask: &FlaskSave, source: Option<Entity>) {
    let at = Vec2::from(flask.position);
    let Ok(prepared) = world.prepare_prefab(toys::flask::FLASK_PREFAB, at.to_array()) else {
        return;
    };
    let prepared = prepared.with((
        Transform::from_position(at),
        Position(at),
        PrevPosition(at),
        Saved::kept(SaveId(flask.id)),
        flask.projectile.clone(),
        flask.motion.clone(),
        toys::components::PreviousVelocity::default(),
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
        Saved::kept(SaveId(missile.id)),
        Joined,
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
