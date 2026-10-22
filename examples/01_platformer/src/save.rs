//! What a save holds, part of the seam the shell calls: [`SaveGame`], how
//! a live run becomes one ([`capture`]), the slot row's line ([`summary`])
//! and how a loaded run takes it back ([`restore`]). The schema counts on
//! from the template sample's 3, so a sample save reads as an older
//! version's, never as a newer one's. Schema 7 marks 01 (d)'s map, whose
//! objects take every save ID anew, so nothing older migrates (`D-202`);
//! schema 8 gives the player the staff light, which [`SaveData::migrate`]
//! adds to a schema 7 save.
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
    Abilities, AilmentBuckets, ContactDamage, ContactTouch, Effects, Health, Hitbox, KitRng,
    Knockback, Lifetime, LinearMotion, Oscillator, PathFollower, Pickup, PlatformerController,
    Projectile, Resist, Source, StateMachine, Stats, Tags, TopDownMover,
};

use crate::features::combat::components::{Hints, Reaction};
use crate::features::dummy::{self, components::Dummy};
use crate::features::elements::cascade::Cascades;
use crate::features::elements::components::{BallBurn, Chill, Frozen};
use crate::features::enemies::components::{
    Brain, Displaced, Enemy, EnemyKind, EnemyState, HeldTouch, Sight, Throw, Throws,
};
use crate::features::enemies::drops::{self, Orb, OrbKind};
use crate::features::enemies::{self, troll};
use crate::features::gun::components::{Bullet, Reload};
use crate::features::gun::{self, BULLET_CENTRE, BulletShot};
use crate::features::level::components::Doused;
use crate::features::player::STAFF_LIGHT;
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
    /// The staff light guttered out and waits for the relight; a schema 7
    /// save, from before the light, has none.
    #[serde(default)]
    pub lantern_guttered: bool,
    pub player: PlayerSave,
    pub platforms: Vec<PlatformSave>,
    pub hazards: Vec<HazardSave>,
    pub balls: Vec<BallSave>,
    pub blocks: Vec<BlockSave>,
    pub black_hole: Option<HoleSave>,
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
    /// The reactions whose hints the save has shown, in order.
    pub hints: Vec<Reaction>,
    /// The summon slot's type, by its place in `SUMMONS`.
    pub summon: usize,
}

/// The dummies the map's `dummies` layer places.
pub const MAP_DUMMIES: u64 = 1;
/// The enemies the map's `enemies` layer places, and the health orbs its
/// `pickups` layer places after them, its last.
pub const MAP_ENEMIES: u64 = 18;
pub const MAP_PICKUPS: u64 = 8;
/// Schema 8's map's enemies, before 01's quick fixes placed five more.
const SCHEMA_8_ENEMIES: u64 = 13;
/// How far schema 9 moves every save ID at or above schema 8's player's:
/// the map objects it added, which take theirs before the player's.
pub const SHIFT_9: u64 = MAP_ENEMIES - SCHEMA_8_ENEMIES + MAP_PICKUPS;

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and extend `migrate`. Schema 7
    /// is 01 (d)'s new map: its objects take every save ID anew and every
    /// place moved, so `migrate` refuses each older schema and the slot
    /// reads as an older version's (`D-204`). Schema 8 adds the staff light
    /// to the player's abilities. Schema 9 places new map objects before
    /// the player, which take the save IDs from schema 8's player's on.
    const SCHEMA: u32 = 9;

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
                Ok(data)
            }
            8 => {
                shift_ids(&mut data)?;
                Ok(data)
            }
            _ => Err(format!("schema {from} has no migration to schema {}", Self::SCHEMA).into()),
        }
    }
}

/// Moves `id` up by [`SHIFT_9`] when it is a save ID at or above `from`.
fn shift_id(id: Option<&mut Value>, from: u64) {
    if let Some(id) = id
        && let Some(value) = id.as_u64().filter(|&value| value >= from)
    {
        *id = Value::from(value + SHIFT_9);
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
/// new map objects now take, moves up by [`SHIFT_9`], in every field that
/// names one; a field it lacks stays lacking.
fn shift_ids(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let from = data
        .pointer("/player/id")
        .and_then(Value::as_u64)
        .ok_or("schema 8: the player has no save ID")?;
    shift_id(data.get_mut("next_id"), from);
    shift_id(data.pointer_mut("/player/id"), from);
    shift_id(data.pointer_mut("/black_hole/id"), from);
    for source in each(data, "/player/sources") {
        shift_id(source.get_mut(1), from);
    }
    for list in ["/platforms", "/hazards", "/blocks", "/orbs"] {
        for entry in each(data, list) {
            shift_id(entry.get_mut("id"), from);
        }
    }
    for list in ["/missiles", "/bullets"] {
        for entry in each(data, list) {
            shift_id(entry.get_mut("id"), from);
            shift_id(entry.get_mut("source"), from);
        }
    }
    for list in ["/dummies", "/enemies"] {
        for entry in each(data, list) {
            shift_id(entry.get_mut("id"), from);
            for source in each(entry, "/sources") {
                shift_id(source.get_mut(1), from);
            }
        }
    }
    for enemy in each(data, "/enemies") {
        for target in each(enemy, "/hit") {
            shift_id(Some(target), from);
        }
        for touch in each(enemy, "/touches") {
            shift_id(touch.get_mut(0), from);
        }
        shift_id(enemy.get_mut("held"), from);
    }
    for throw in each(data, "/throws") {
        shift_id(throw.get_mut("body"), from);
        shift_id(throw.get_mut("thrower"), from);
    }
    for ball in each(data, "/balls") {
        shift_id(ball.get_mut(0), from);
    }
    for id in each(data, "/gone") {
        shift_id(Some(id), from);
    }
    for entry in each(data, "/cascades/entries") {
        shift_id(entry.get_mut("target"), from);
        shift_id(entry.get_mut("source"), from);
        for member in each(entry, "/chain") {
            shift_id(Some(member), from);
        }
    }
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
        black_hole,
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
    for orb in &save.orbs {
        drops::spawn(
            world,
            orb.kind,
            Vec2::from(orb.position),
            SaveId(orb.id),
            orb.left,
        );
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
    world.insert_resource(Hints {
        seen: save.hints.clone(),
    });
    world.insert_resource(Summon {
        selected: save.summon,
    });
    world.insert_resource(save.rng.clone());
    world.insert_resource(SaveIds::starting_at(save.next_id));
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
        Some(controller) => world.insert(entity, controller.clone()),
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
