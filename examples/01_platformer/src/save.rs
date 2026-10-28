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
//! walk and the cold ladder's halved gates. Schema 15 walls the start
//! (01 (k)) and sets a saved body from the wall, or from left of the map,
//! on the clearing's floor. Schema 16 keeps the tomes the player holds
//! (01 (l)) and sets a saved body from inside one of the books' three new
//! supports on its top. Schema 19 (e01hollow) keeps the hollow's geysers
//! and stones, moves the save IDs past its three new enemies and its
//! geysers, and sets what the mouth's lowered lintel holds under it.
//! Schema 22 moves the save IDs past the map's seven new enemies and
//! lowers each wisp's health.
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
    Projectile, Resist, Shield, Source, StateMachine, Stats, Tags, TopDownMover,
};

use crate::features::dummy::{self, components::Dummy};
use crate::features::elements::cascade::Cascades;
use crate::features::elements::components::{BallBurn, Chill, Frozen};
use crate::features::enemies::components::{
    Brain, Displaced, Enemy, EnemyKind, EnemyState, HeldTouch, Sight, Snowball, Throw, Throws,
};
use crate::features::enemies::drops::{self, Orb, OrbKind};
use crate::features::enemies::{self, crawler, snowball, troll, wisp};
use crate::features::gun::components::{Bullet, Reload};
use crate::features::gun::{self, BULLET_CENTRE, BulletShot};
use crate::features::level::components::Doused;
use crate::features::level::elevator::{Elevator, Stop};
use crate::features::level::geyser::{self, Geyser, GeyserPhase};
use crate::features::level::lava::{self, Lava};
use crate::features::player::STAFF_LIGHT;
use crate::features::player::components::Lantern;
use crate::features::props::{
    self,
    components::{Bomb, Crate, Fuel, Keg, Plank, PropKind, Rock, Stone, StoneSize},
};
use crate::features::run::Run;
use crate::features::spells::components::{Spell, Spellbook, Summon};
use crate::features::tomes::{Tome, TomeKind, Tomes};
use crate::features::toys::components::{
    Ball, BallHue, BlackHole, FireMissile, IronBlock, IronScrap, SmallBall, ToxicFlask,
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
    /// The hollow's geysers (e01hollow); none before schema 19, whose map
    /// starts them at their offsets.
    #[serde(default)]
    pub geysers: Vec<GeyserSave>,
    /// The crates, kegs, bombs and rocks; none before schema 12, whose map
    /// adds none to an older save's run.
    #[serde(default)]
    pub props: Vec<PropSave>,
    /// The crawlers' snowballs in flight, apart from `props`, whose bodies
    /// a migration reads by kind; none in an older save, and none written
    /// while none fly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snowballs: Vec<SnowballSave>,
    /// The broken crates' planks (e01sfx), apart from `props` as the
    /// snowballs are; none before schema 18, and none written while none
    /// lie about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub planks: Vec<PlankSave>,
    /// The hollow's rubble stones (e01hollow), apart from `props` as the
    /// planks are; none written while none lie about.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stones: Vec<StoneSave>,
    /// The toxic flasks in flight (e01flask), as the missiles are; none
    /// before schema 21, and none written while none fly.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flasks: Vec<FlaskSave>,
}

/// The dummies the map's `dummies` layer places.
pub const MAP_DUMMIES: u64 = 1;
/// The enemies the map's `enemies` layer places, and the health orbs its
/// `pickups` layer places after them; and the geysers its `geysers` layer
/// places, its last saved, after the tomes, which take none.
pub const MAP_ENEMIES: u64 = 36;
pub const MAP_PICKUPS: u64 = 8;
pub const MAP_GEYSERS: u64 = 3;
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
/// Schema 14's start, which schema 15 walls (01 (k)): the wall's box over
/// columns 0 and 1 from row 21 to the clearing's floor, min and max; the
/// world left of the map, where the boundary stands, lies past its min x
/// too. Frozen for the migration, as the boxes below are.
pub const S15_WALL: [Vec2; 2] = [Vec2::new(0.0, 21.0 * 64.0), Vec2::new(128.0, 36.0 * 64.0)];
/// The clearing's floor's top; the stretch of it from the wall's face to
/// the terrace's, where a body out of the wall is set; and the moving
/// deck's columns, which it skips, the deck coming down to 27 px over the
/// floor.
pub const S15_FLOOR: f32 = 36.0 * 64.0;
pub const S15_SPAN: [f32; 2] = [128.0, 1024.0];
pub const S15_DECK: [f32; 2] = [556.8, 723.2];
/// A fresh run's spawn, the feet's point, where a player with no clear
/// spot on the floor stands.
pub const S15_SPAWN: Vec2 = Vec2::new(416.0, 36.0 * 64.0);
/// Each saved body's box as schema 14 has it: its centre from its point
/// and its half extents. A ball's and a goblin's bomb's and rock's are
/// their circles' squares.
const S15_PLAYER: [Vec2; 2] = [Vec2::new(48.0, 65.0), Vec2::new(20.0, 28.0)];
const S15_DUMMY: [Vec2; 2] = [Vec2::new(32.0, 33.0), Vec2::new(14.0, 28.0)];
const S15_ENEMIES: [(&str, [Vec2; 2]); 6] = [
    ("wisp", [Vec2::new(32.0, 34.0), Vec2::new(12.0, 12.0)]),
    ("crawler", [Vec2::new(32.0, 51.0), Vec2::new(24.0, 10.0)]),
    ("beetle", [Vec2::new(32.0, 48.0), Vec2::new(24.0, 13.0)]),
    ("bat", [Vec2::new(32.0, 28.0), Vec2::new(10.0, 9.0)]),
    ("troll", [Vec2::new(74.0, 70.0), Vec2::new(28.0, 55.0)]),
    ("goblin", [Vec2::new(34.0, 41.0), Vec2::new(9.0, 20.0)]),
];
const S15_BALL: [Vec2; 2] = [Vec2::splat(16.0), Vec2::splat(15.0)];
const S15_SMALL_BALL: [Vec2; 2] = [Vec2::splat(8.0), Vec2::splat(7.5)];
const S15_IRON: [Vec2; 2] = [Vec2::splat(64.0), Vec2::splat(60.0)];
const S15_SCRAP: [Vec2; 2] = [Vec2::splat(16.0), Vec2::splat(15.0)];
const S15_PROPS: [(&str, [Vec2; 2]); 4] = [
    ("crate", [Vec2::splat(32.0), Vec2::splat(28.0)]),
    ("keg", [Vec2::new(32.0, 35.0), Vec2::new(19.0, 25.0)]),
    ("bomb", [Vec2::splat(8.0), Vec2::splat(6.0)]),
    ("rock", [Vec2::splat(8.0), Vec2::splat(5.0)]),
];
const S15_ORB: [Vec2; 2] = [Vec2::splat(12.0), Vec2::splat(12.0)];
/// A plank's box as schema 18 has it, which schema 19 reads with the rest.
const S19_PLANK: [Vec2; 2] = [Vec2::new(16.0, 8.0), Vec2::new(14.0, 4.0)];
/// Schema 18's first placed orb's save ID and its player's, its layout
/// kept; frozen for the migration, as schema 11's are.
pub const S18_PICKUPS: u64 = 35;
pub const S18_PLAYER: u64 = 43;
/// How far schema 19 moves the save IDs: every one at or above schema
/// 18's player's by the three enemies and three geysers it places before
/// the player, and every one from the first placed orb's below it by the
/// three enemies, which take theirs before the orbs.
pub const SHIFT_19: u64 = 6;
pub const SHIFT_19_PICKUPS: u64 = 3;
/// The row schema 19's lintel lowers into the hollow's mouth (e01hollow):
/// `east_wall`'s new bottom row, columns 203 to 215, its box's min and max.
/// Frozen for the migration, as schema 15's wall is.
pub const S19_LINTEL: [Vec2; 2] = [
    Vec2::new(203.0 * 64.0, 57.0 * 64.0),
    Vec2::new(216.0 * 64.0, 58.0 * 64.0),
];
/// The ravine's bat c, schema 18's save ID 14, which roosted under the
/// lintel's old row, and its new roost under row 58, its body's centre.
pub const S18_BAT_C: u64 = 14;
pub const S19_BAT_C_HOME: Vec2 = Vec2::new(13440.0, 3740.0);
/// The tomes' supports, which schema 16 adds (01 (l)): `pact_shelf`,
/// `bound_shelf` and `arcane_ledge`, each its cells' box, min and max.
/// Frozen for the migration, as schema 15's wall is.
pub const S16_SUPPORTS: [[Vec2; 2]; 3] = [
    [
        Vec2::new(231.0 * 64.0, 57.0 * 64.0),
        Vec2::new(234.0 * 64.0, 58.0 * 64.0),
    ],
    [
        Vec2::new(194.0 * 64.0, 40.0 * 64.0),
        Vec2::new(196.0 * 64.0, 41.0 * 64.0),
    ],
    [
        Vec2::new(117.0 * 64.0, 34.0 * 64.0),
        Vec2::new(120.0 * 64.0, 35.0 * 64.0),
    ],
];

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
    /// tuned that a save keeps (`to_14`). Schema 15 sets the bodies in the
    /// start's new wall or past its boundary on the clearing's floor
    /// (`to_15`). Schema 16 keeps the tomes held and sets the bodies inside
    /// their supports on top (`to_16`). Schema 17 moves each crawler's run
    /// to e01foes' (`to_17`). Schema 18 keeps a broken crate's planks
    /// (`planks`), which an older save has none of. Schema 19 keeps the
    /// hollow's geysers and stones, moves the save IDs past its new enemies
    /// and geysers and sets what its lowered lintel holds under it
    /// (`to_19`). Schema 20 puts the toxic flask fifth on the bar and gives
    /// the player its ability (`to_20`). Schema 21 keeps the flasks in
    /// flight and gives the living, the armoured and the bridge's armoured
    /// troll their tags and table (`to_21`). Schema 22 moves the save IDs
    /// past the map's seven new enemies and each wisp's health to its new
    /// maximum (`to_22`).
    const SCHEMA: u32 = 22;

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
                to_15(&mut data)?;
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            8 => {
                shift_ids(&mut data)?;
                to_10(&mut data)?;
                to_11(&mut data)?;
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                to_15(&mut data)?;
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            9 => {
                to_10(&mut data)?;
                to_11(&mut data)?;
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                to_15(&mut data)?;
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            10 => {
                to_11(&mut data)?;
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                to_15(&mut data)?;
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            11 => {
                to_12(&mut data);
                to_13(&mut data)?;
                to_14(&mut data)?;
                to_15(&mut data)?;
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            12 => {
                to_13(&mut data)?;
                to_14(&mut data)?;
                to_15(&mut data)?;
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            13 => {
                to_14(&mut data)?;
                to_15(&mut data)?;
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            14 => {
                to_15(&mut data)?;
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            15 => {
                to_16(&mut data)?;
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            16 => {
                to_17(&mut data);
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            17 | 18 => {
                to_19(&mut data)?;
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            19 => {
                to_20(&mut data)?;
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            20 => {
                to_21(&mut data);
                to_22(&mut data);
                Ok(data)
            }
            21 => {
                to_22(&mut data);
                Ok(data)
            }
            _ => Err(format!("schema {from} has no migration to schema {}", Self::SCHEMA).into()),
        }
    }
}

/// The toxic flask's ability, which schema 20 gives the player.
const TOXIC_FLASK: &str = Spell::ToxicFlask.ability();
/// The bar's slot schema 20 puts the toxic flask in, the fifth (e01flask):
/// a schema 19 save's from it on moved up one. Frozen for the migration.
pub const S20_FLASK_SLOT: u64 = 4;

/// Schema 19 to 20 (e01flask): the toxic flask takes the bar's fifth slot
/// ([`S20_FLASK_SLOT`]), so a saved slot from it on moves up one, and the
/// player's abilities take the flask's entry, ready, as a fresh run's, where
/// it is absent; a duplicate would fail the load.
fn to_20(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(spell) = data.pointer_mut("/spell")
        && let Some(slot) = spell.as_u64()
        && slot >= S20_FLASK_SLOT
    {
        *spell = Value::from(slot + 1);
    }
    let entries = data
        .pointer_mut("/player/abilities/entries")
        .and_then(Value::as_array_mut)
        .ok_or("schema 19: the player has no abilities")?;
    if !entries.iter().any(|entry| entry["name"] == TOXIC_FLASK) {
        let entry = format!(
            r#"{{"name": "{TOXIC_FLASK}", "ability": "{TOXIC_FLASK}", "cooldown_left": 0.0, "charges": null, "refill_left": 0.0, "interval_left": 0.0}}"#
        );
        entries.push(entry.parse()?);
    }
    Ok(())
}

/// The bridge troll's save ID, which schema 21 makes the armoured troll
/// (e01flask, Q2), and armour's table, its bases. Frozen for the migration.
pub const S21_BRIDGE_TROLL: u64 = 21;
pub const S21_ARMOUR: [(&str, f64); 5] = [
    ("damage.physical", 0.25),
    ("damage.fire", 0.75),
    ("damage.cold", 0.75),
    ("damage.shock", 0.75),
    ("damage.poison", 0.75),
];

/// Schema 20 to 21 (e01flask): every saved enemy but a wisp, and every
/// dummy, takes `living`, which poison needs; every beetle `armoured`; and
/// the map's troll of [`S21_BRIDGE_TROLL`] becomes the armoured troll: its
/// variant, `armoured` and armour's table ([`S21_ARMOUR`]) as its bases,
/// its saved modifiers kept. Each part holds when it already has, since a
/// schema 20 save from e01flask's own steps may carry some of it. A schema
/// 20 save holds no flask.
fn to_21(data: &mut Value) {
    let tag = |entry: &mut Value, tag: &str| {
        if let Some(tags) = entry["tags"].as_array_mut()
            && !tags.iter().any(|held| held == tag)
        {
            tags.push(Value::from(tag));
        }
    };
    for dummy in each(data, "/dummies") {
        tag(dummy, "living");
    }
    for enemy in each(data, "/enemies") {
        if enemy["kind"] != "wisp" {
            tag(enemy, "living");
        }
        if enemy["kind"] == "beetle" {
            tag(enemy, "armoured");
        }
        let bridge = enemy["id"].as_u64() == Some(S21_BRIDGE_TROLL)
            && enemy["kind"] == "troll"
            && enemy["map"].as_bool() == Some(true);
        if !bridge {
            continue;
        }
        enemy["enemy"]["variant"] = Value::from("armoured");
        tag(enemy, "armoured");
        if let Some(resist) = enemy["resist"].as_object_mut() {
            for (name, base) in S21_ARMOUR {
                match resist.get_mut(name) {
                    Some(factor) if factor.is_object() => factor["base"] = Value::from(base),
                    _ => {
                        resist.insert(name.to_string(), Value::from(base));
                    }
                }
            }
        }
    }
}

/// Schema 21's first placed orb's save ID, its layout kept, and how far
/// schema 22 moves every save ID from it on: past the map's seven new
/// enemies (four crawlers, two wisps and a beetle), which take theirs
/// before the orbs. Frozen for the migration, as schema 11's are.
pub const S21_PICKUPS: u64 = 38;
pub const SHIFT_22: u64 = 7;

/// Schema 21 to 22: the map's seven new enemies take the save IDs from
/// schema 21's first placed orb's on, so every one from it moves up by
/// [`SHIFT_22`]; and each wisp's `max_health` base moves to
/// [`wisp::MAX_HEALTH`], its maximum with it and its health keeping its
/// share, its modifiers kept. A schema 21 save names no new enemy: the
/// map's stand at home.
fn to_22(data: &mut Value) {
    map_ids(data, &|id| {
        if id >= S21_PICKUPS { id + SHIFT_22 } else { id }
    });
    let full = f64::from(wisp::MAX_HEALTH);
    for enemy in each(data, "/enemies") {
        if enemy["kind"] != "wisp" {
            continue;
        }
        let Some(base) = enemy["stats"]["max_health"]["base"].as_f64() else {
            continue;
        };
        if base > 0.0 {
            for key in ["current", "max"] {
                if let Some(value) = enemy["health"][key].as_f64() {
                    enemy["health"][key] = Value::from(value * full / base);
                }
            }
        }
        enemy["stats"]["max_health"]["base"] = Value::from(full);
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

/// A saved body: its list and place in it (`player` for the player), its
/// save ID, its centre from its point and its half extents, its point, and
/// whether the map placed it.
struct Body {
    list: &'static str,
    index: usize,
    id: u64,
    offset: Vec2,
    half: Vec2,
    point: Vec2,
    map: bool,
}

impl Body {
    fn min(&self) -> Vec2 {
        self.point + self.offset - self.half
    }

    fn max(&self) -> Vec2 {
        self.point + self.offset + self.half
    }
}

/// Every body a schema 14 or 15 save keeps, the player first: the dummies,
/// the enemies, the balls, the blocks and scraps, the props and the orbs.
fn bodies(data: &Value) -> Result<Vec<Body>, Box<dyn std::error::Error + Send + Sync>> {
    let pair = |xy: Option<&Value>| -> Option<Vec2> {
        let xy = xy?.as_array()?;
        Some(Vec2::new(
            xy.first()?.as_f64()? as f32,
            xy.get(1)?.as_f64()? as f32,
        ))
    };
    let player =
        pair(data.pointer("/player/position")).ok_or("schema 14: the player has no position")?;
    let mut out = vec![Body {
        list: "player",
        index: 0,
        id: data
            .pointer("/player/id")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        offset: S15_PLAYER[0],
        half: S15_PLAYER[1],
        point: player,
        map: false,
    }];
    for list in [
        "dummies", "enemies", "balls", "blocks", "props", "orbs", "planks",
    ] {
        for (index, entry) in data[list].as_array().into_iter().flatten().enumerate() {
            let named = |table: &[(&str, [Vec2; 2])], key: &str| {
                table
                    .iter()
                    .find(|(name, _)| entry[key].as_str() == Some(name))
                    .map(|(_, sizes)| *sizes)
            };
            let (sizes, point) = match list {
                "balls" => {
                    let small = entry.get(1).and_then(Value::as_bool) == Some(true);
                    let x = entry.get(2).and_then(Value::as_f64);
                    let y = entry.get(3).and_then(Value::as_f64);
                    let point = x.zip(y).map(|(x, y)| Vec2::new(x as f32, y as f32));
                    (Some(if small { S15_SMALL_BALL } else { S15_BALL }), point)
                }
                "dummies" => (Some(S15_DUMMY), pair(entry.get("position"))),
                "enemies" => (named(&S15_ENEMIES, "kind"), pair(entry.get("position"))),
                "blocks" => (
                    Some(if entry["scrap"].is_null() {
                        S15_IRON
                    } else {
                        S15_SCRAP
                    }),
                    pair(entry.get("position")),
                ),
                "props" => (named(&S15_PROPS, "kind"), pair(entry.get("position"))),
                "planks" => (Some(S19_PLANK), pair(entry.get("position"))),
                _ => (Some(S15_ORB), pair(entry.get("position"))),
            };
            let (Some([offset, half]), Some(point)) = (sizes, point) else {
                return Err(format!("schema 14: a body of /{list}/{index} has no box").into());
            };
            let id = if list == "balls" {
                entry.get(0)
            } else {
                entry.get("id")
            };
            out.push(Body {
                list,
                index,
                id: id.and_then(Value::as_u64).unwrap_or(0),
                offset,
                half,
                point,
                map: entry["map"].as_bool() == Some(true),
            });
        }
    }
    Ok(out)
}

/// Schema 14 to 15 (01 (k)): the start's wall stands over columns 0 and 1
/// from row 21, and the boundary left of the map. Each saved body whose box
/// meets the wall's ([`S15_WALL`]) or lies past the map's left edge is set
/// at rest on the clearing's floor, at the first x from the wall's face to
/// the terrace's ([`S15_SPAN`]) where its box meets no other saved body's
/// and skips the moving deck's columns ([`S15_DECK`]), the player first.
/// A player with no such spot stands at the run's spawn, where the solver
/// parts it from any body; any other body with no room is dropped, a map
/// dummy or enemy taking its save ID into `gone` so the load takes the
/// map's off too, and the log names each dropped save ID.
fn to_15(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut bodies = bodies(data)?;
    let [wall, face] = S15_WALL;
    let inside = |body: &Body| {
        let (min, max) = (body.min(), body.max());
        min.x < 0.0 || (min.x < face.x && max.x > wall.x && min.y < face.y && max.y > wall.y)
    };
    let moving: Vec<usize> = (0..bodies.len()).filter(|&i| inside(&bodies[i])).collect();
    if moving.is_empty() {
        return Ok(());
    }
    let mut placed = vec![true; bodies.len()];
    for &i in &moving {
        placed[i] = false;
    }
    let mut dropped = Vec::new();
    for &i in &moving {
        let (offset, half) = (bodies[i].offset, bodies[i].half);
        let y = S15_FLOOR - 0.5 - half.y;
        let others: Vec<(Vec2, Vec2)> = (0..bodies.len())
            .filter(|&j| j != i && placed[j])
            .map(|j| (bodies[j].min(), bodies[j].max()))
            .collect();
        let fits = |left: f32| {
            let (min, max) = (
                Vec2::new(left, y - half.y),
                Vec2::new(left + 2.0 * half.x, y + half.y),
            );
            max.x <= S15_SPAN[1]
                && !(min.x < S15_DECK[1] && max.x > S15_DECK[0])
                && others
                    .iter()
                    .all(|(a, b)| !(min.x < b.x && max.x > a.x && min.y < b.y && max.y > a.y))
        };
        let mut lefts: Vec<f32> = [S15_SPAN[0], S15_DECK[1]]
            .into_iter()
            .chain(others.iter().map(|(_, max)| max.x))
            .filter(|&left| left >= S15_SPAN[0])
            .collect();
        lefts.sort_by(f32::total_cmp);
        let centre = match lefts.into_iter().find(|&left| fits(left)) {
            Some(left) => Vec2::new(left + half.x, y),
            None if bodies[i].list == "player" => S15_SPAWN - Vec2::new(0.0, half.y + 0.5),
            None => {
                dropped.push(i);
                continue;
            }
        };
        bodies[i].point = centre - offset;
        placed[i] = true;
    }
    write_bodies(data, &bodies, &moving, &dropped);
    let ids: Vec<u64> = dropped.iter().map(|&i| bodies[i].id).collect();
    log::info!(
        "schema 15: saved bodies set out of the start's wall on the clearing's floor: {}; dropped with no room: {ids:?}",
        moving.len() - dropped.len()
    );
    Ok(())
}

/// Schema 15 to 16 (01 (l)): the tomes' three supports stand where a
/// schema 15 run had open air. Each saved body whose box meets one
/// ([`S16_SUPPORTS`]) is set at rest on that support's top, its x kept on
/// it, where its box meets no other saved body's, the player first; a
/// player with no room there stands at the run's spawn, and any other body
/// is dropped, as [`to_15`] drops one.
fn to_16(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut bodies = bodies(data)?;
    let meets = |(min, max): (Vec2, Vec2), [low, high]: [Vec2; 2]| {
        min.x < high.x && max.x > low.x && min.y < high.y && max.y > low.y
    };
    let moving: Vec<(usize, [Vec2; 2])> = (0..bodies.len())
        .filter_map(|i| {
            let boxed = (bodies[i].min(), bodies[i].max());
            let support = S16_SUPPORTS.into_iter().find(|&s| meets(boxed, s))?;
            Some((i, support))
        })
        .collect();
    if moving.is_empty() {
        return Ok(());
    }
    let mut placed = vec![true; bodies.len()];
    for &(i, _) in &moving {
        placed[i] = false;
    }
    let mut dropped = Vec::new();
    for &(i, [low, high]) in &moving {
        let (offset, half) = (bodies[i].offset, bodies[i].half);
        // Its x on the support, which a body wider than it cannot keep.
        let fits = 2.0 * half.x <= high.x - low.x;
        let x = (bodies[i].point.x + offset.x)
            .min(high.x - half.x)
            .max(low.x + half.x);
        let centre = Vec2::new(x, low.y - 0.5 - half.y);
        let clear = (0..bodies.len()).filter(|&j| j != i && placed[j]).all(|j| {
            !meets(
                (centre - half, centre + half),
                [bodies[j].min(), bodies[j].max()],
            )
        });
        let centre = match (fits && clear, bodies[i].list) {
            (true, _) => centre,
            (false, "player") => S15_SPAWN - Vec2::new(0.0, half.y + 0.5),
            (false, _) => {
                dropped.push(i);
                continue;
            }
        };
        bodies[i].point = centre - offset;
        placed[i] = true;
    }
    let moving: Vec<usize> = moving.into_iter().map(|(i, _)| i).collect();
    write_bodies(data, &bodies, &moving, &dropped);
    let ids: Vec<u64> = dropped.iter().map(|&i| bodies[i].id).collect();
    log::info!(
        "schema 16: saved bodies set out of the tomes' supports on top: {}; dropped with no room: {ids:?}",
        moving.len() - dropped.len()
    );
    Ok(())
}

/// Schema 16 to 17 (e01foes): each saved crawler's `move_speed` base moves
/// to [`crawler::MOVE_SPEED`], its modifiers kept, as [`to_14`] moved the
/// troll's.
fn to_17(data: &mut Value) {
    for enemy in data["enemies"].as_array_mut().into_iter().flatten() {
        if enemy["kind"] == "crawler" {
            enemy["stats"]["move_speed"]["base"] = Value::from(f64::from(crawler::MOVE_SPEED));
        }
    }
}

/// Schema 18 to 19 (e01hollow): the map's three new enemies take the save
/// IDs from schema 18's first placed orb's on and its three geysers those
/// after the orbs, so every save ID at or above schema 18's player's moves
/// up by [`SHIFT_19`] and every one from its first orb's below it by
/// [`SHIFT_19_PICKUPS`]. The mouth's lintel comes down a row
/// ([`S19_LINTEL`]): the ravine's bat c, which roosted under its old row,
/// takes its new roost as its home ([`S19_BAT_C_HOME`]), and its body too
/// when it meets the row, since a load puts both over the map's; every
/// other saved body that meets the row is set at rest with its top 0.5 px
/// under it, its x kept, where it meets no other saved body, a player with
/// no room standing at the run's spawn and any other body dropped, as
/// [`to_16`] drops one. A schema 18 save names no new enemy or geyser and
/// holds no stone: the map's stand at home, the geysers start at their
/// offsets and the hollow has no stones.
fn to_19(data: &mut Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    map_ids(data, &|id| {
        if id >= S18_PLAYER {
            id + SHIFT_19
        } else if id >= S18_PICKUPS {
            id + SHIFT_19_PICKUPS
        } else {
            id
        }
    });
    let [low, high] = S19_LINTEL;
    let meets = |(min, max): (Vec2, Vec2), others: &[(Vec2, Vec2)]| {
        let inside = |a: Vec2, b: Vec2| min.x < b.x && max.x > a.x && min.y < b.y && max.y > a.y;
        others.iter().any(|&(a, b)| inside(a, b))
    };
    let lintel = [(low, high)];
    let [offset, half] = S15_ENEMIES
        .iter()
        .find(|(kind, _)| *kind == "bat")
        .map(|(_, sizes)| *sizes)
        .ok_or("schema 18: no bat box")?;
    for enemy in each(data, "/enemies") {
        let bat_c = enemy["id"].as_u64() == Some(S18_BAT_C)
            && enemy["kind"] == "bat"
            && enemy["map"].as_bool() == Some(true);
        if !bat_c {
            continue;
        }
        enemy["enemy"]["home"] = Value::from(vec![S19_BAT_C_HOME.x, S19_BAT_C_HOME.y]);
        let point = enemy["position"].as_array().and_then(|xy| {
            Some(Vec2::new(
                xy.first()?.as_f64()? as f32,
                xy.get(1)?.as_f64()? as f32,
            ))
        });
        if let Some(point) = point
            && meets((point + offset - half, point + offset + half), &lintel)
        {
            let corner = S19_BAT_C_HOME - offset;
            enemy["position"] = Value::from(vec![corner.x, corner.y]);
            enemy["velocity"] = Value::from(vec![0.0_f32, 0.0]);
        }
    }
    let mut bodies = bodies(data)?;
    let bat_c = |body: &Body| body.list == "enemies" && body.id == S18_BAT_C && body.map;
    let moving: Vec<usize> = (0..bodies.len())
        .filter(|&i| !bat_c(&bodies[i]) && meets((bodies[i].min(), bodies[i].max()), &lintel))
        .collect();
    if moving.is_empty() {
        return Ok(());
    }
    let mut placed = vec![true; bodies.len()];
    for &i in &moving {
        placed[i] = false;
    }
    let mut dropped = Vec::new();
    for &i in &moving {
        let (offset, half) = (bodies[i].offset, bodies[i].half);
        let centre = Vec2::new(bodies[i].point.x + offset.x, high.y + 0.5 + half.y);
        let others: Vec<(Vec2, Vec2)> = (0..bodies.len())
            .filter(|&j| j != i && placed[j])
            .map(|j| (bodies[j].min(), bodies[j].max()))
            .collect();
        let clear = !meets((centre - half, centre + half), &others);
        let centre = match (clear, bodies[i].list) {
            (true, _) => centre,
            (false, "player") => S15_SPAWN - Vec2::new(0.0, half.y + 0.5),
            (false, _) => {
                dropped.push(i);
                continue;
            }
        };
        bodies[i].point = centre - offset;
        placed[i] = true;
    }
    write_bodies(data, &bodies, &moving, &dropped);
    let ids: Vec<u64> = dropped.iter().map(|&i| bodies[i].id).collect();
    log::info!(
        "schema 19: saved bodies set under the hollow's lowered lintel: {}; dropped with no room: {ids:?}",
        moving.len() - dropped.len()
    );
    Ok(())
}

/// Writes the points [`to_15`] or [`to_16`] gave the `moving` bodies into
/// `data`, at rest, and takes the `dropped` off their lists, a map dummy
/// or enemy taking its save ID into `gone` so the load takes the map's off
/// too.
fn write_bodies(data: &mut Value, bodies: &[Body], moving: &[usize], dropped: &[usize]) {
    for &i in moving {
        let body = &bodies[i];
        if dropped.contains(&i) {
            continue;
        }
        let [x, y] = body.point.to_array();
        if body.list == "balls" {
            let ball = &mut data["balls"][body.index];
            for (k, value) in [x, y, 0.0, 0.0].into_iter().enumerate() {
                ball[2 + k] = Value::from(value);
            }
            continue;
        }
        let entry = if body.list == "player" {
            &mut data["player"]
        } else {
            &mut data[body.list][body.index]
        };
        entry["position"] = Value::from(vec![x, y]);
        if entry.get("velocity").is_some() {
            entry["velocity"] = Value::from(vec![0.0_f32, 0.0]);
        }
    }
    // The dropped, from each list's last, so the places hold.
    let mut gone: Vec<u64> = Vec::new();
    let mut order = dropped.to_vec();
    order.sort_by_key(|&i| std::cmp::Reverse(bodies[i].index));
    for i in order {
        let body = &bodies[i];
        if let Some(list) = data[body.list].as_array_mut() {
            list.remove(body.index);
        }
        if body.map && matches!(body.list, "dummies" | "enemies") {
            gone.push(body.id);
        }
    }
    if !gone.is_empty()
        && let Some(list) = data["gone"].as_array_mut()
    {
        list.extend(gone.into_iter().map(Value::from));
        list.sort_by_key(|id| id.as_u64());
    }
}

/// Halves the `damage.cold` fill in a saved `{damage type: fill}` map.
fn halve_cold(buckets: &mut Value) {
    if let Some(fill) = buckets.get_mut("damage.cold")
        && let Some(value) = fill.as_f64()
    {
        *fill = Value::from(value * 0.5);
    }
}

/// Every save ID field of `data`, a save's JSON, through `to`; a field it
/// lacks stays lacking. The migrations' walk, which the tests also take
/// back to an older numbering.
pub fn map_ids(data: &mut Value, to: &impl Fn(u64) -> u64) {
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
        "/geysers",
        "/stones",
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
    for list in ["/missiles", "/bullets", "/flasks"] {
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
    for snowball in each(data, "/snowballs") {
        map_id(snowball.get_mut("id"), to);
        map_id(snowball.get_mut("thrower"), to);
    }
    for plank in each(data, "/planks") {
        map_id(plank.get_mut("id"), to);
        map_id(plank.pointer_mut("/fuel/lighter"), to);
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
/// stats, effects with their sources, abilities, ailment buckets, tags,
/// the gun's reload while one runs, the tomes held and the pact's shield.
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
    /// The tomes taken, in order; none before schema 16.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tomes: Vec<TomeKind>,
    /// The pact's shield, with its recharge's wait; none without the pact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield: Option<Shield>,
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

/// A geyser: its phase, the phase's seconds left, the cold spray's seconds
/// on it and its choke's seconds left (e01hollow).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeyserSave {
    pub id: u64,
    pub phase: GeyserPhase,
    pub left: f32,
    pub cold: f32,
    pub choked: f32,
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
    pub fuel: Fuel,
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
    let mut geysers: Vec<GeyserSave> = world
        .query::<(&SaveId, &Geyser)>()
        .map(|(id, state)| GeyserSave {
            id: id.0,
            phase: state.phase,
            left: state.left,
            cold: state.cold,
            choked: state.choked,
        })
        .collect();
    geysers.sort_by_key(|geyser| geyser.id);
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
    let mut flasks: Vec<FlaskSave> = world
        .query::<(
            &SaveId,
            &ToxicFlask,
            &Position,
            &Projectile,
            &LinearMotion,
            Option<&Source>,
        )>()
        .map(|(id, _, at, projectile, motion, source)| FlaskSave {
            id: id.0,
            position: at.0.to_array(),
            projectile: projectile.clone(),
            motion: motion.clone(),
            source: source.and_then(|source| id_of(source.entity())),
        })
        .collect();
    flasks.sort_by_key(|flask| flask.id);
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
        .filter(|&entity| !world.has::<Plank>(entity) && !world.has::<Stone>(entity))
        .filter_map(|entity| capture_prop(world, entity))
        .collect();
    props.sort_by_key(|prop| prop.id);
    let mut planks: Vec<PlankSave> = world
        .query::<(Entity, &Plank, &SaveId, &Fuel)>()
        .filter_map(|(entity, plank, id, fuel)| {
            Some(PlankSave {
                id: id.0,
                variant: plank.variant,
                position: world.get::<Position>(entity)?.0.to_array(),
                velocity: world.get::<Velocity>(entity)?.0.to_array(),
                chill: world.get::<Chill>(entity).map(|chill| chill.amount),
                frozen: world.get::<Frozen>(entity).map(|frozen| frozen.left),
                fuel: *fuel,
            })
        })
        .collect();
    planks.sort_by_key(|plank| plank.id);
    let mut stones: Vec<StoneSave> = world
        .query::<(Entity, &Stone, &SaveId, &Position, &Velocity)>()
        .map(|(_, stone, id, at, velocity)| StoneSave {
            id: id.0,
            size: stone.size,
            variant: stone.variant,
            position: at.0.to_array(),
            velocity: velocity.0.to_array(),
        })
        .collect();
    stones.sort_by_key(|stone| stone.id);
    let snowballs = snowball::flying(world)
        .into_iter()
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
        geysers,
        props,
        snowballs,
        planks,
        stones,
        flasks,
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
        tomes: world
            .get::<Tomes>(player)
            .map(|tomes| tomes.0.clone())
            .unwrap_or_default(),
        shield: world.get::<Shield>(player).cloned(),
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
    crate::features::player::shove_bodies(world, player);
    let lantern = Lantern {
        on: save.lantern,
        guttered: save.lantern_guttered,
    };
    world.insert_bundle(player, (Spellbook { slot: save.spell }, lantern));
    if let Some(left) = p.reload {
        world.insert(player, Reload { left });
    }
    if let Some(shield) = &p.shield {
        world.insert(player, shield.clone());
    }
    // The books the player holds leave the map, which spawned all three.
    if !p.tomes.is_empty() {
        world.insert(player, Tomes(p.tomes.clone()));
        let taken: Vec<Entity> = world
            .query::<(Entity, &Tome)>()
            .filter(|(_, tome)| p.tomes.contains(&tome.kind))
            .map(|(book, _)| book)
            .collect();
        for book in taken {
            crate::shared::despawn_tree(world, book);
        }
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
    for saved in &save.geysers {
        if let Some(&entity) = by_id.get(&saved.id) {
            geyser::set(world, entity, saved.phase, saved.left, saved.choked);
            if let Some(state) = world.get_mut::<Geyser>(entity) {
                state.cold = saved.cold;
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
                let (kind, variant, id) = (saved.kind, saved.enemy.variant, SaveId(saved.id));
                let Some(entity) = enemies::spawn_as(world, kind, variant, corner, id) else {
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
    for plank in &save.planks {
        restore_plank(world, plank);
    }
    for stone in &save.stones {
        let at = Vec2::from(stone.position);
        let kind = PropKind::Stone(stone.size);
        let entity = props::restore(world, kind, at, SaveId(stone.id), stone.variant);
        world.insert(entity, Velocity(Vec2::from(stone.velocity)));
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
                SaveId(saved.id),
                Velocity(Vec2::from(saved.velocity)),
                flying,
            ),
        );
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
    for flask in &save.flasks {
        let source = flask.source.and_then(|id| by_id.get(&id).copied());
        restore_flask(world, flask, source);
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
        PropKind::Crate | PropKind::Rock | PropKind::Plank | PropKind::Stone(_) => {}
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

/// A saved plank, spawned at once at its saved point with its state over
/// its prefab's, at rest from its tumble.
fn restore_plank(world: &mut World, saved: &PlankSave) {
    let at = Vec2::from(saved.position);
    let entity = props::restore(world, PropKind::Plank, at, SaveId(saved.id), saved.variant);
    world.insert_bundle(entity, (Velocity(Vec2::from(saved.velocity)), saved.fuel));
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
        SaveId(flask.id),
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
