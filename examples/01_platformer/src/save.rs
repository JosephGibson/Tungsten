//! What a save holds, part of the seam the shell calls: [`SaveGame`], how
//! a live run becomes one ([`capture`]), the slot row's line ([`summary`])
//! and how a loaded run takes it back ([`restore`]). The schema counts on
//! from the template sample's 3, so a sample save reads as an older
//! version's, never as a newer one's.
//!
//! The run is the engine's `WorldSave` (`D-230`) beside 01's table of its
//! balls. A run's start names the player, global, and the map's saved
//! objects (its lava basins, decks, the elevator, the dummy, enemies, placed
//! orbs, tomes and geysers) in the map's area, once its setup is done, so a
//! capture writes each one's changes over its fresh spawn: their state is
//! the components and resources 01 and the kit register, chemistry's queue
//! among them. The run's spawns are the World save's records, whole: the
//! kit's from a saved source (the gun's bullets, fire missiles and rolled
//! orbs) and 01's own (summoned dummies and enemies, iron and its scraps,
//! black holes, its own missiles, flasks and bullets, the props, planks,
//! stones and snowballs), each taking the run's next save ID at its spawn.
//! The small and bowling balls carry a kept identity, `Saved::kept` of the
//! run's counter, which the engine's references name, and 01's table holds
//! them, since a full pit's 12,000 as records would pass the save's bound;
//! 7.14's prefab table takes them over. Schema 29 makes the run's spawns
//! records, so `migrate_step` refuses every older save, which reads as an
//! older version's (`D-204`); schema 30 moves arcane power's `spell_power`
//! onto the kit's `damage_dealt`, and schema 31 an enemy's `heading` onto
//! the kit's `facing`, each migrating the schema before.
//!
//! A save names another entity by its save ID, never by its entity.
//! Presentation is not saved: particles, lights, glows, flames, overlays
//! and tints, animation, flashes, shake, toasts, the HUD and the music.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::save::{
    RestoreError, Value, WorldSave, capture_world, check_world, restore_world,
};
use tungsten::core::{CameraController, DataRef, Entity, Position, SaveData, Velocity, World};
use tungsten::kit::{EffectRequest, Health, ImpactDamage, Stats, carrier_of};

use crate::features::dummy::{self, components::Dummy};
use crate::features::elements::components::ColdFace;
use crate::features::elements::systems::{
    arm_throw, coat_of, freeze_for, light, prefab_of, show_douse,
};
use crate::features::elements::{COAT, burn_left, ice_left};
use crate::features::enemies::components::{Enemy, Holding, Throws};
use crate::features::enemies::drops::{self, Orb};
use crate::features::enemies::{self, troll};
use crate::features::level::components::Hazard;
use crate::features::level::lava::{self, Lava};
use crate::features::props;
use crate::features::run::Run;
use crate::features::tomes::{Tome, Tomes};
use crate::features::toys;
use crate::features::toys::components::{
    Ball, BallHue, IronScrap, PreviousVelocity, SmallBall, ToxicFlask,
};
use crate::game::GAMEPLAY;
use crate::shared::{Joined, SaveId, Saved, despawn_tree};
use crate::shell::{self, RunMeta};

/// A save: the shell's run data, the last area the player entered, the
/// slot row's numbers, the engine's save of the run's World and the table
/// of the balls, in save-ID order.
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
    /// The named objects' changes over their fresh spawn, the run's spawns
    /// whole and the saved resources.
    pub world: WorldSave,
    pub balls: Vec<BallSave>,
}

impl SaveData for SaveGame {
    /// Bump it when `SaveGame` changes shape, and add its step to
    /// `migrate_step`. Schema 28 moved the run onto the engine's saves and
    /// 29 its spawns onto the World save's records: `migrate_step` refuses
    /// every older schema, whose slot reads as an older version's
    /// (`D-204`). Schema 30 moved arcane power's `spell_power` onto a
    /// `damage_dealt` line ([`to_30`]), 31 an enemy's `heading` onto the
    /// kit's `facing` ([`to_31`]), 32 a foe's rests and "!" onto the
    /// kit's cooldowns and effects ([`to_32`]), 33 the crawler's
    /// snowball onto the kit's projectile ([`to_33`]), 34 the lightning's
    /// chain onto the kit's conduction ([`to_34`]), 35 the missile's and
    /// the tracers' turns onto the kit's facing ([`to_35`]), 36 the
    /// black hole's life onto a kit cooldown ([`to_36`]), 37 the props'
    /// caps onto the kit's, marking their records ([`to_37`]), 38 iron's
    /// and the black holes' ([`to_38`]), 39 the summoned dummies' and
    /// enemies' ([`to_39`]), 40 the geysers' cycles onto the kit's timed
    /// machine, their touch kept behind its switch ([`to_40`]), and 41 the
    /// elevator onto the kit's two-way lift ([`to_41`]).
    const SCHEMA: u32 = 41;

    fn migrate_step(
        from: u32,
        mut data: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        match from {
            29 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 29: the save has no world")?;
                to_30(world);
                Ok(data)
            }
            30 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 30: the save has no world")?;
                to_31(world);
                Ok(data)
            }
            31 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 31: the save has no world")?;
                to_32(world);
                Ok(data)
            }
            32 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 32: the save has no world")?;
                to_33(world);
                Ok(data)
            }
            33 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 33: the save has no world")?;
                to_34(world);
                Ok(data)
            }
            34 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 34: the save has no world")?;
                to_35(world);
                Ok(data)
            }
            35 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 35: the save has no world")?;
                to_36(world);
                Ok(data)
            }
            36 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 36: the save has no world")?;
                to_37(world);
                Ok(data)
            }
            37 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 37: the save has no world")?;
                to_38(world);
                Ok(data)
            }
            38 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 38: the save has no world")?;
                to_39(world);
                Ok(data)
            }
            39 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 39: the save has no world")?;
                to_40(world);
                Ok(data)
            }
            40 => {
                let world = data
                    .get_mut("world")
                    .ok_or("schema 40: the save has no world")?;
                to_41(world);
                Ok(data)
            }
            _ => Err(format!("schema {from} has no migration to schema {}", from + 1).into()),
        }
    }
}

/// The arcane tome's source on the player's stats, frozen for the
/// migration.
const S29_ARCANE: &str = "tome.arcane";

/// Schema 29 to 30 over a save's `world`: every saved `stats`, a record's
/// or a patch's, loses its `spell_power`, and one whose `spell_power`
/// carried the arcane tome's modifier takes the `damage_dealt` stat at base
/// 1 with the tome's line of 2 on the `hit.spell` context, as the tome
/// gives it now.
fn to_30(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(stats)) = object.get_mut("stats")
                && let Some(power) = stats.remove("spell_power")
            {
                let arcane = power
                    .get("modifiers")
                    .and_then(Value::as_array)
                    .is_some_and(|modifiers| {
                        modifiers.iter().any(|modifier| {
                            modifier.get(0).and_then(Value::as_str) == Some(S29_ARCANE)
                        })
                    });
                if arcane {
                    let line = Value::from(vec![
                        Value::from(S29_ARCANE),
                        Value::from_iter([
                            (
                                "modifier".to_owned(),
                                Value::from_iter([("mul".to_owned(), Value::from(2.0))]),
                            ),
                            (
                                "with".to_owned(),
                                Value::from_iter([(
                                    "all".to_owned(),
                                    Value::from(vec![Value::from("hit.spell")]),
                                )]),
                            ),
                        ]),
                    ]);
                    let dealt = Value::from_iter([
                        ("base".to_owned(), Value::from(1.0)),
                        ("min".to_owned(), Value::from(0.0)),
                        ("max".to_owned(), Value::Null),
                        ("lines".to_owned(), Value::from(vec![line])),
                    ]);
                    stats.insert("damage_dealt".to_owned(), dealt);
                }
            }
            for inner in object.values_mut() {
                to_30(inner);
            }
        }
        Value::Array(items) => {
            for item in items {
                to_30(item);
            }
        }
        _ => {}
    }
}

/// Schema 30 to 31 over a save's `world`: every saved `enemy`, a record's
/// or a patch's, loses its `heading`, 1 right or -1 left, which becomes the
/// kit's `facing` beside it, from an intent and snapped left or right, as
/// the foes' prefabs give it at schema 31.
fn to_31(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let heading = object
                .get_mut("enemy")
                .and_then(Value::as_object_mut)
                .and_then(|enemy| enemy.remove("heading"));
            if let Some(heading) = heading {
                let left = heading.as_f64().is_some_and(|x| x < 0.0);
                object.entry("facing").or_insert_with(|| s30_facing(left));
            }
            for inner in object.values_mut() {
                to_31(inner);
            }
        }
        Value::Array(items) => {
            for item in items {
                to_31(item);
            }
        }
        _ => {}
    }
}

/// A foe's `facing` as schema 31 saves it, facing left or right: frozen
/// for the migration.
fn s30_facing(left: bool) -> Value {
    let x = if left { -1.0 } else { 1.0 };
    Value::from_iter([
        ("from".to_owned(), Value::from("intent")),
        ("snap".to_owned(), Value::from("horizontal")),
        ("turn".to_owned(), Value::from(false)),
        ("offset".to_owned(), Value::from(0.0)),
        ("min_speed".to_owned(), Value::from(1.0)),
        (
            "direction".to_owned(),
            Value::from(vec![Value::from(x), Value::from(0.0)]),
        ),
    ])
}

/// The source of the bat's dive's speed on its `move_speed` at schema 31,
/// frozen for the migration.
const S31_DIVE: &str = "dive";

/// Schema 31 to 32 over a save's `world`: every saved `brain`, a record's
/// or a patch's, loses its `alerted`, `slam_cooldown` and `cooldown`. A
/// rest left becomes the foe's kit `cooldown` beside it, of the rest's own
/// seconds and spent by its one elapse, so it needs no kind and serves no
/// later rest; the "!" is dropped. Every saved `stats` line from the bat's
/// dive goes, its speed now an effect's.
fn to_32(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let rest = object
                .get_mut("brain")
                .and_then(Value::as_object_mut)
                .map(|brain| {
                    brain.remove("alerted");
                    ["slam_cooldown", "cooldown"]
                        .map(|key| brain.remove(key).and_then(|left| left.as_f64()))
                        .into_iter()
                        .flatten()
                        .fold(0.0_f64, f64::max)
                });
            if let Some(rest) = rest.filter(|rest| *rest > 0.0) {
                object.insert("cooldown".to_owned(), s31_rest(rest));
            }
            if let Some(Value::Object(stats)) = object.get_mut("stats") {
                for stat in stats.values_mut().filter_map(Value::as_object_mut) {
                    for key in ["modifiers", "lines"] {
                        if let Some(Value::Array(pairs)) = stat.get_mut(key) {
                            pairs.retain(|pair| {
                                pair.get(0).and_then(Value::as_str) != Some(S31_DIVE)
                            });
                        }
                    }
                }
            }
            for inner in object.values_mut() {
                to_32(inner);
            }
        }
        Value::Array(items) => {
            for item in items {
                to_32(item);
            }
        }
        _ => {}
    }
}

/// Schema 32 to 33 over a save's `world`: each saved snowball, a dynamic
/// body's record whose `snowball` held its thrower, factor and seconds,
/// goes, a body a load could not fly on the kit's projectile; at most
/// eight were in flight, for at most 3 s.
fn to_33(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for inner in object.values_mut() {
                to_33(inner);
            }
        }
        Value::Array(items) => {
            items.retain(|item| {
                item.get("components")
                    .and_then(|components| components.get(S32_SNOWBALL))
                    .is_none()
            });
            for item in items {
                to_33(item);
            }
        }
        _ => {}
    }
}

/// The name schema 32 saved a snowball's flight under, frozen for the
/// migration.
const S32_SNOWBALL: &str = "snowball";

/// Schema 33 to 34 over a save's `world`: the saved `cascades` loses its
/// `entries`, the lightning's jumps in flight, at most six a strike for at
/// most 0.1 s each, which the kit's conduction cannot take up, as schema 33
/// dropped the snowballs; and each record of iron, a scrap or a keg takes
/// `conductive` into its `tags`, as their prefabs give it at schema 34, so
/// the conduction finds it.
fn to_34(world: &mut Value) {
    if let Some(cascades) = world
        .get_mut("resources")
        .and_then(|resources| resources.get_mut(S33_CASCADES))
        .and_then(Value::as_object_mut)
    {
        cascades.remove("entries");
    }
    metal_conducts(world);
}

/// Each record of [`S33_METAL`]'s prefabs within `value` takes
/// [`S33_CONDUCTIVE`] into its `tags`.
fn metal_conducts(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let metal = object
                .get("prefab")
                .and_then(Value::as_str)
                .is_some_and(|prefab| S33_METAL.contains(&prefab));
            if metal && let Some(Value::Object(components)) = object.get_mut("components") {
                let mut tags: Vec<String> = components
                    .get("tags")
                    .and_then(Value::as_array)
                    .map(|tags| {
                        tags.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                tags.push(S33_CONDUCTIVE.to_owned());
                tags.sort();
                tags.dedup();
                components.insert(
                    "tags".to_owned(),
                    Value::from(tags.into_iter().map(Value::from).collect::<Vec<_>>()),
                );
            }
            for inner in object.values_mut() {
                metal_conducts(inner);
            }
        }
        Value::Array(items) => {
            for item in items {
                metal_conducts(item);
            }
        }
        _ => {}
    }
}

/// The name schema 33 saved the cascades under, the prefabs of its metal
/// and the tag the conduction finds metal by at schema 34, frozen for the
/// migration.
const S33_CASCADES: &str = "cascades";
const S33_METAL: [&str; 3] = ["iron_block", "iron_scrap", "keg"];
const S33_CONDUCTIVE: &str = "conductive";

/// Schema 34 to 35 over a save's `world`: each record of a fire missile or
/// a bullet in flight without a `facing` takes its prefab's, as schema 35
/// saves it, so the kit turns its rocket or its tracer along its flight.
fn to_35(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let facing = match object.get("prefab").and_then(Value::as_str) {
                Some(S34_MISSILE) => Some(s34_facing(true)),
                Some(S34_BULLET) => Some(s34_facing(false)),
                _ => None,
            };
            if let (Some(facing), Some(Value::Object(components))) =
                (facing, object.get_mut("components"))
            {
                components.entry("facing").or_insert(facing);
            }
            for inner in object.values_mut() {
                to_35(inner);
            }
        }
        Value::Array(items) => {
            for item in items {
                to_35(item);
            }
        }
        _ => {}
    }
}

/// Schema 35 to 36 over a save's `world`: each saved `black_hole`, a
/// record's or a patch's, loses its `left`, which becomes the kit's
/// `cooldown` beside it, the hole's life of [`S35_HOLE_LIFE`] with that
/// left, at least 0, and its one elapse to go, as the prefab gives it at
/// schema 36.
fn to_36(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let left = object
                .get_mut(S35_BLACK_HOLE)
                .and_then(Value::as_object_mut)
                .and_then(|hole| hole.remove("left"))
                .and_then(|left| left.as_f64());
            if let Some(left) = left {
                object.insert("cooldown".to_owned(), s35_life(left.max(0.0)));
            }
            for inner in object.values_mut() {
                to_36(inner);
            }
        }
        Value::Array(items) => {
            for item in items {
                to_36(item);
            }
        }
        _ => {}
    }
}

/// The name schema 35 saved a black hole under and its life's seconds,
/// frozen for the migration.
const S35_BLACK_HOLE: &str = "black_hole";
const S35_HOLE_LIFE: f64 = 6.0;

/// A black hole's life at schema 35 as the kit's `cooldown`, `left`
/// seconds of its six to go and one elapse: frozen for the migration.
fn s35_life(left: f64) -> Value {
    Value::from_iter([
        ("seconds".to_owned(), Value::from(S35_HOLE_LIFE)),
        ("left".to_owned(), Value::from(left)),
        ("remaining".to_owned(), Value::from(1)),
    ])
}

/// Schema 36 to 37 over a save's `world`: each record of a placed crate
/// or keg (one without [`S36_MAP_PROP`]), of a goblin's rock and of a
/// plank takes the kit's admission mark, [`S36_ADMITTED`], as schema 37
/// gives each one placed, so a loaded older save's props count toward
/// their prefabs' caps as they counted toward 01's; the map's own stay
/// aside, and a record already marked keeps its one mark.
fn to_37(value: &mut Value) {
    mark_admitted(value, &S36_PROPS);
}

/// Schema 37 to 38 over a save's `world`: each record of an iron block,
/// the map's among them, and of a black hole takes the kit's admission
/// mark, [`S36_ADMITTED`], as schema 38 gives each one placed, so a
/// loaded older save's iron and holes count toward their prefabs' caps
/// as they counted toward 01's.
fn to_38(value: &mut Value) {
    mark_admitted(value, &S37_IRON_HOLES);
}

/// The prefabs of schema 37's iron and black holes, frozen for the
/// migration.
const S37_IRON_HOLES: [&str; 2] = ["iron_block", "black_hole"];

/// Schema 38 to 39 over a save's `world`: each record of a dummy or an
/// enemy, every one a summon's, the map's being named objects, takes the
/// kit's admission mark, [`S36_ADMITTED`], as schema 39 gives each one
/// summoned, so a loaded older save's summons count toward their
/// prefabs' caps as they counted toward 01's.
fn to_39(value: &mut Value) {
    mark_admitted(value, &S38_SUMMONS);
}

/// The prefabs of schema 38's summons, frozen for the migration.
const S38_SUMMONS: [&str; 8] = [
    "dummy",
    "wisp",
    "crawler",
    "beetle",
    "bat",
    "troll",
    "goblin",
    "troll_armoured",
];

/// Schema 39 to 40 over a save's `world`: each geyser's cycle moves onto
/// the kit's timed machine, saved as `geyser_phase`, and its touch stays on
/// it behind its switch. The global record and each area are walked for the
/// map's geysers that hold a record ([`S39_GEYSERS`], in `named`) and for
/// the spawns of [`S39_GEYSER`] ([`s39_geyser`]).
fn to_40(world: &mut Value) {
    if let Some(global) = world.get_mut("global") {
        s39_area(global);
    }
    if let Some(Value::Object(areas)) = world.get_mut("areas") {
        for area in areas.values_mut() {
            s39_area(area);
        }
    }
}

/// One area's geysers at schema 39: its named records by the map's names,
/// each a patch over a fresh geyser at its offset, and its spawns of the
/// geyser's prefab, each whole.
fn s39_area(area: &mut Value) {
    if let Some(Value::Object(named)) = area.get_mut("named") {
        for (name, offset) in S39_GEYSERS {
            if let Some(record) = named.get_mut(name) {
                s39_geyser(record, Some(offset));
            }
        }
    }
    if let Some(Value::Array(spawns)) = area.get_mut("spawns") {
        for spawn in spawns {
            if spawn.get("prefab").and_then(Value::as_str) == Some(S39_GEYSER) {
                s39_geyser(spawn, None);
            }
        }
    }
}

/// One geyser's record from schema 39 to 40; `named` is a map geyser's
/// offset, whose record is a patch, and none for a spawn's, which is whole.
///
/// The record's `geyser` is laid over what a fresh geyser held at schema
/// 39: the phase and seconds left its offset gave, no chill, no choke,
/// started. `geyser` keeps its offset, chill and `started`, written whole,
/// since the form refuses the fields it lost. `geyser_phase` is the machine
/// in that phase, entered when the cycle had started, the seconds spent in
/// it the phase's length less the seconds left, or, choked, the choke's
/// length less what was left of it; at least 0, since the old cycle's carry
/// left up to 1e-5 s over and the machine's save refuses a negative time.
/// Its touch is the save's own where it held one, erupting, else for a
/// spawn the prefab's with its clock at 0, and takes its switch, on only in
/// an eruption: a named record lies over a fresh geyser whose touch is off.
fn s39_geyser(record: &mut Value, named: Option<f64>) {
    let Value::Object(record) = record else {
        return;
    };
    if let Some(Value::Array(removed)) = record.get_mut("removed") {
        removed.retain(|name| name.as_str() != Some(S39_TOUCH));
    }
    let components = record
        .entry("components")
        .or_insert_with(|| Value::Object(Default::default()));
    let Value::Object(components) = components else {
        return;
    };
    let old = components.remove(S39_GEYSER);
    let field = |key: &str| old.as_ref().and_then(|geyser| geyser.get(key));
    let offset = field("offset")
        .and_then(Value::as_f64)
        .or(named)
        .unwrap_or(0.0);
    let (fresh, fresh_left) = s39_at_offset(offset);
    let phase = field("phase").and_then(Value::as_str).unwrap_or(fresh);
    let left = field("left").and_then(Value::as_f64).unwrap_or(fresh_left);
    let cold = field("cold").and_then(Value::as_f64).unwrap_or(0.0);
    let choked = field("choked").and_then(Value::as_f64).unwrap_or(0.0);
    let started = field("started").and_then(Value::as_bool).unwrap_or(true);
    let (state, elapsed) = if choked > 0.0 {
        (S39_CHOKED, S39_CHOKE - choked)
    } else {
        let seconds = match phase {
            "sputter" => S39_SPUTTER,
            "erupt" => S39_ERUPT,
            _ => S39_IDLE,
        };
        (phase, seconds - left)
    };
    let machine = Value::from_iter([
        ("state".to_owned(), Value::from(state)),
        ("previous".to_owned(), Value::Null),
        ("elapsed".to_owned(), Value::from(elapsed.max(0.0))),
        ("next".to_owned(), Value::Null),
        ("entered".to_owned(), Value::from(started)),
        ("held".to_owned(), Value::from(false)),
    ]);
    let erupting = state == "erupt";
    let geyser = Value::from_iter([
        ("offset".to_owned(), Value::from(offset)),
        ("cold".to_owned(), Value::from(cold)),
        ("started".to_owned(), Value::from(started)),
    ]);
    components.insert(S39_GEYSER.to_owned(), geyser);
    components.insert(S39_PHASE.to_owned(), machine);
    let touch = components.entry(S39_TOUCH).or_insert_with(|| {
        if named.is_some() {
            Value::Object(Default::default())
        } else {
            s39_touch()
        }
    });
    if let Value::Object(touch) = touch
        && let Value::Object(contact) = touch
            .entry("contact")
            .or_insert_with(|| Value::Object(Default::default()))
    {
        contact.insert("on".to_owned(), Value::from(erupting));
    }
}

/// The phase and its seconds left `offset` seconds into a cycle, as schema
/// 39's geysers began: frozen for the migration.
fn s39_at_offset(offset: f64) -> (&'static str, f64) {
    let cycle = S39_IDLE + S39_SPUTTER + S39_ERUPT;
    let t = offset.rem_euclid(cycle);
    if t < S39_IDLE {
        ("idle", S39_IDLE - t)
    } else if t < S39_IDLE + S39_SPUTTER {
        ("sputter", S39_IDLE + S39_SPUTTER - t)
    } else {
        ("erupt", cycle - t)
    }
}

/// A geyser's touch as its prefab gave it at schema 39, its clock at 0:
/// 12 fire every 0.5 s on the enemies and the player, thrown at 1,000 px/s
/// past the window, over its plume's box. Frozen for the migration.
fn s39_touch() -> Value {
    let names = |names: [&str; 2]| Value::from(names.map(Value::from).to_vec());
    let pair = |x: f64, y: f64| Value::from(vec![Value::from(x), Value::from(y)]);
    let part = Value::from_iter([
        ("amount".to_owned(), Value::from(12.0)),
        (
            "tags".to_owned(),
            Value::from(vec![Value::from("damage.fire")]),
        ),
    ]);
    let contact = Value::from_iter([
        ("damage".to_owned(), Value::from(12.0)),
        (
            "filter".to_owned(),
            Value::from_iter([("any".to_owned(), names(["team.enemy", "team.player"]))]),
        ),
        ("interval".to_owned(), Value::from(0.5)),
        ("knockback".to_owned(), Value::from(1000.0)),
        ("offset".to_owned(), pair(32.0, 32.0)),
        ("parts".to_owned(), Value::from(vec![part])),
        ("push_always".to_owned(), Value::from(true)),
        (
            "shape".to_owned(),
            Value::from_iter([("aabb".to_owned(), pair(20.0, 176.0))]),
        ),
        ("tags".to_owned(), Value::from(Vec::<Value>::new())),
    ]);
    Value::from_iter([
        ("clock".to_owned(), Value::from(0.0)),
        ("contact".to_owned(), contact),
    ])
}

/// Schema 39's geysers, frozen for the migration: the map's three by name,
/// each with the seconds into its cycle its run began at; their prefab,
/// which is also the name their state was saved under; the names of their
/// touch and, at schema 40, of their machine and its choke; and the seconds
/// of their three phases and of a choke.
const S39_GEYSERS: [(&str, f64); 3] = [
    ("geyser_floor_west", 0.0),
    ("geyser_floor_east", 1.3),
    ("geyser_shelf", 2.6),
];
const S39_GEYSER: &str = "geyser";
const S39_TOUCH: &str = "contact_damage";
const S39_PHASE: &str = "geyser_phase";
const S39_CHOKED: &str = "choked";
const S39_IDLE: f64 = 2.5;
const S39_SPUTTER: f64 = 0.6;
const S39_ERUPT: f64 = 0.8;
const S39_CHOKE: f64 = 6.0;

/// Schema 40 to 41 over a save's `world`: the mine's elevator moves onto
/// the kit's two-way lift. Its saved component, [`S40_ELEVATOR`], leaves
/// the components' versions, and its named record ([`S40_LIFT`]), where
/// the global record or an area holds one, takes the lift's
/// `path_follower` in its place ([`s40_lift`]). An untouched lift has no
/// record and spawns fresh.
fn to_41(world: &mut Value) {
    if let Some(Value::Object(versions)) = world
        .get_mut("versions")
        .and_then(|versions| versions.get_mut("components"))
    {
        versions.remove(S40_ELEVATOR);
    }
    if let Some(global) = world.get_mut("global") {
        s40_area(global);
    }
    if let Some(Value::Object(areas)) = world.get_mut("areas") {
        for area in areas.values_mut() {
            s40_area(area);
        }
    }
}

/// One area's lift at schema 40: its named record, a patch over a fresh
/// lift.
fn s40_area(area: &mut Value) {
    if let Some(record) = area
        .get_mut("named")
        .and_then(|named| named.get_mut(S40_LIFT))
    {
        s40_lift(record);
    }
}

/// The lift's record from schema 40 to 41.
///
/// Its `elevator` is laid over what a fresh one held at schema 40: at the
/// bottom, nobody boarding, no seconds alone, not ridden, not held. Its
/// time on the kit's path, the bottom to the top and back, each way
/// [`s40_way`] seconds, is 0 at the bottom and the way out at the top; on
/// a trip, the seconds along its way ([`s40_along`]), the way out added
/// when falling. A trip that has reached its end, the step before the old
/// tick named the stop, stands at that end, since a ping-pong refuses a
/// time of its whole length. `path_follower` is written whole in the kit's
/// saved form, the rider's rule as the prefab has it, with the hold's
/// state: `boarding` and the seconds a boarding has counted; `carried`
/// where the player rode it to the stop it stands at; else its seconds
/// alone, which count toward the return at the top; and `paused` where a
/// trip was held. `elevator` leaves the record and its `removed`; its
/// `position`, `transform` and `velocity` stay.
fn s40_lift(record: &mut Value) {
    let Value::Object(record) = record else {
        return;
    };
    if let Some(Value::Array(removed)) = record.get_mut("removed") {
        removed.retain(|name| name.as_str() != Some(S40_ELEVATOR));
    }
    let components = record
        .entry("components")
        .or_insert_with(|| Value::Object(Default::default()));
    let Value::Object(components) = components else {
        return;
    };
    let old = components.remove(S40_ELEVATOR);
    let field = |key: &str| old.as_ref().and_then(|lift| lift.get(key));
    let stop = field("stop").and_then(Value::as_str).unwrap_or("bottom");
    let board = field("board").and_then(Value::as_f64);
    let idle = field("idle").and_then(Value::as_f64).unwrap_or(0.0);
    let ridden = field("ridden").and_then(Value::as_bool).unwrap_or(false);
    let held = field("held").and_then(Value::as_bool).unwrap_or(false);
    let way = s40_way();
    let falling = stop == "falling";
    let (time, standing, trip) = if falling || stop == "rising" {
        let along = s40_along(
            components.get(S40_FOLLOWER),
            components.get("position"),
            falling,
            way,
        );
        // The time the kit reads, an `f32`: at the way out or past it, the
        // trip has reached its end.
        if along as f32 >= way as f32 {
            (if falling { 0.0 } else { way }, true, true)
        } else {
            (if falling { way + along } else { along }, false, true)
        }
    } else {
        (if stop == "top" { way } else { 0.0 }, true, false)
    };
    let pair = |[x, y]: [f64; 2]| Value::from(vec![Value::from(x), Value::from(y)]);
    let rider = Value::from_iter([
        ("board".to_owned(), Value::from(S40_BOARD)),
        ("back".to_owned(), Value::from(S40_BACK)),
        (
            "filter".to_owned(),
            Value::from_iter([("any".to_owned(), Value::from(vec![Value::from(S40_RIDER)]))]),
        ),
        ("two_way".to_owned(), Value::from(true)),
    ]);
    let mut follower = Value::from_iter([
        (
            "points".to_owned(),
            Value::from(vec![pair(S40_BOTTOM), pair(S40_TOP)]),
        ),
        ("speed".to_owned(), Value::from(f64::from(S40_SPEED))),
        ("mode".to_owned(), Value::from("ping_pong")),
        ("easing".to_owned(), Value::from("sine_in_out")),
        ("time".to_owned(), Value::from(time)),
        ("rider".to_owned(), rider),
    ]);
    if let Value::Object(follower) = &mut follower {
        if let Some(left) = board.filter(|_| !trip) {
            follower.insert("boarding".to_owned(), Value::from(true));
            follower.insert("hold".to_owned(), Value::from((S40_BOARD - left).max(0.0)));
        } else if standing && ridden {
            follower.insert("carried".to_owned(), Value::from(true));
        } else if stop == "top" {
            follower.insert("hold".to_owned(), Value::from(idle.max(0.0)));
        }
        if held && !standing {
            follower.insert("paused".to_owned(), Value::from(true));
        }
    }
    components.insert(S40_FOLLOWER.to_owned(), follower);
}

/// The seconds a schema 40 trip is along its way from the stop it left,
/// the top when `falling`, `way` seconds from the other: its saved
/// follower's time where that follower began at that stop; else, for a
/// trip resumed or turned back mid-shaft, whose follower began there, and
/// a held one, with none, the time whose eased point is its saved
/// `position`, the sine's inverse of the share of the shaft it has
/// covered.
fn s40_along(follower: Option<&Value>, position: Option<&Value>, falling: bool, way: f64) -> f64 {
    let (left, toward) = if falling {
        (S40_TOP, S40_BOTTOM)
    } else {
        (S40_BOTTOM, S40_TOP)
    };
    let began = follower
        .and_then(|follower| follower.get("points"))
        .and_then(|points| points.get(0))
        .and_then(|point| point.get(1))
        .and_then(Value::as_f64);
    let time = follower
        .and_then(|follower| follower.get("time"))
        .and_then(Value::as_f64);
    if let (Some(began), Some(time)) = (began, time)
        && (began - left[1]).abs() < 0.01
    {
        return time.max(0.0);
    }
    // A record without a `position` stands where a fresh lift does.
    let y = position
        .and_then(|position| position.get(1))
        .and_then(Value::as_f64)
        .unwrap_or(S40_BOTTOM[1]);
    let share = ((y - left[1]) / (toward[1] - left[1])).clamp(0.0, 1.0);
    (1.0 - 2.0 * share).acos() / std::f64::consts::PI * way
}

/// The seconds the lift's path takes each way, in the kit's own sum: its
/// rise over its speed in `f32`, widened, so a lift at its top reads as
/// standing there.
fn s40_way() -> f64 {
    f64::from((S40_BOTTOM[1] - S40_TOP[1]) as f32 / S40_SPEED)
}

/// Schema 40's lift, frozen for the migration: its record's name in its
/// area, its saved component and the kit's follower, its points at rest and
/// at the top, its speed, and its rider's rule as schema 41's prefab has
/// it: the seconds aboard before it goes, the seconds alone at the top
/// before it comes back, and the tag its riders carry.
const S40_LIFT: &str = "deck_elevator";
const S40_ELEVATOR: &str = "elevator";
const S40_FOLLOWER: &str = "path_follower";
const S40_BOTTOM: [f64; 2] = [20160.0, 3351.0];
const S40_TOP: [f64; 2] = [20160.0, 2327.0];
const S40_SPEED: f32 = 320.0;
const S40_BOARD: f64 = 0.3;
const S40_BACK: f64 = 4.0;
const S40_RIDER: &str = "team.player";

/// Each record of `prefabs` within `value` the map did not set down
/// takes [`S36_ADMITTED`].
fn mark_admitted(value: &mut Value, prefabs: &[&str]) {
    match value {
        Value::Object(object) => {
            let placed = object
                .get("prefab")
                .and_then(Value::as_str)
                .is_some_and(|prefab| prefabs.contains(&prefab));
            if placed
                && let Some(Value::Object(components)) = object.get_mut("components")
                && !components.contains_key(S36_MAP_PROP)
            {
                components
                    .entry(S36_ADMITTED)
                    .or_insert_with(|| Value::Object(Default::default()));
            }
            for inner in object.values_mut() {
                mark_admitted(inner, prefabs);
            }
        }
        Value::Array(items) => {
            for item in items {
                mark_admitted(item, prefabs);
            }
        }
        _ => {}
    }
}

/// The kit's admission mark as schema 37 saves it, the name schema 36
/// saved a map's prop's mark under and its placed props' prefabs, frozen
/// for the migration.
const S36_ADMITTED: &str = "cap_admitted";
const S36_MAP_PROP: &str = "map_prop";
const S36_PROPS: [&str; 4] = ["crate", "keg", "goblin_rock", "plank"];

/// The prefabs of schema 34's shots, frozen for the migration.
const S34_MISSILE: &str = "fire_missile";
const S34_BULLET: &str = "bullet";

/// A shot's `facing` as schema 35 saves it: from its motion, turning, the
/// missile's mirrored past the vertical; facing right until its first
/// frame turns it. Frozen for the migration.
fn s34_facing(mirror: bool) -> Value {
    let mut facing = Value::from_iter([
        ("from".to_owned(), Value::from("motion")),
        ("snap".to_owned(), Value::from("free")),
        ("turn".to_owned(), Value::from(true)),
        ("offset".to_owned(), Value::from(0.0)),
        ("min_speed".to_owned(), Value::from(1.0)),
        (
            "direction".to_owned(),
            Value::from(vec![Value::from(1.0), Value::from(0.0)]),
        ),
    ]);
    if mirror && let Value::Object(fields) = &mut facing {
        fields.insert("mirror".to_owned(), Value::from(true));
    }
    facing
}

/// A foe's rest left at schema 31 as the kit's `cooldown` of its own
/// seconds, one elapse to go: frozen for the migration.
fn s31_rest(seconds: f64) -> Value {
    Value::from_iter([
        ("seconds".to_owned(), Value::from(seconds)),
        ("left".to_owned(), Value::from(seconds)),
        ("remaining".to_owned(), Value::from(1)),
    ])
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
    Some(SaveGame {
        meta,
        area: run.area.clone(),
        health: health.clone(),
        mana,
        world: saved,
        balls,
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
/// three passes, once the engine's checks pass (`check_world`), so a save
/// that cannot load stages nothing: no ball, and none of the queue entries
/// its statuses would add. First the balls come back under their kept
/// identities, at their saved points, their burn and ice put back through
/// 01's setters: before the engine's restore replaces chemistry's queue and
/// the request queues with the saved ones, so what a status put back queues
/// gives way to the saved entries. Then the engine lays the named objects'
/// changes over their fresh spawn, spawns its records and puts the saved
/// resources and every reference back (`restore_world`), its report's lines
/// logged. Last it gives each restored entity what its spawn gives beyond
/// its saved state ([`rebuild`]).
///
/// # Errors
///
/// The engine's [`RestoreError`] for a save whose World does not restore,
/// found by the check before the first pass, with the World as it was.
pub fn restore(world: &mut World, save: &SaveGame) -> Result<(), RestoreError> {
    check_world(world, &save.world)?;
    let staged = spawn_balls(world, save);
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
    rebuild(world);
    Ok(())
}

/// The first pass: the balls back under their kept identities, with their
/// hue, burn and ice. The entities it spawned.
fn spawn_balls(world: &mut World, save: &SaveGame) -> Vec<Entity> {
    let mut staged = Vec::with_capacity(save.balls.len());
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
    staged
}

/// The last pass: what each restored entity's spawn gives beyond its saved
/// state, read off the restored state and never by applying it again: a
/// record's `PreviousVelocity`, a dummy's and every enemy's body, an
/// enemy's hold on rock, a scrap's, a crate's, a rock's, a stone's and a
/// plank's look; what an older save's records lack of their prefabs, an
/// enemy's disarm, a dropped orb's fall, a flask's end, shape and body
/// walls and a body's or an enemy's impact rule, a body's credited to the
/// player; each live throw's rule set as the throw set it; a crusted basin's body layers, mask, touch and glow, a
/// doused hazard's touch, glow and trail; a troll's hold lifted again; the taken tomes' books off the
/// map; the bodies the player stands in shoved; and the camera on the
/// player where the save left it.
fn rebuild(world: &mut World) {
    let recorded: Vec<Entity> = world
        .query::<(Entity, &Saved)>()
        .filter(|(_, saved)| saved.prefab().is_some())
        .map(|(entity, _)| entity)
        .collect();
    for &entity in &recorded {
        if !world.has::<PreviousVelocity>(entity) {
            world.insert(entity, PreviousVelocity::default());
        }
        // The face the cold last met, where a freeze's frost bursts: a
        // chilled or frozen record's, at its centre.
        if (coat_of(world, entity) > 0.0 || ice_left(world, entity).is_some())
            && !world.has::<ColdFace>(entity)
            && let Some(centre) = crate::shared::body_centre(world, entity)
        {
            world.insert(entity, ColdFace(centre));
        }
        if world.has::<IronScrap>(entity) {
            toys::restored_scrap(world, entity);
        } else if world.has::<Dummy>(entity) {
            dummy::restored(world, entity);
        } else if props::is_prop(world, entity) {
            props::restored(world, entity);
        } else if world.has::<Orb>(entity) {
            drops::restored(world, entity);
        } else if world.has::<ToxicFlask>(entity) {
            toys::flask::restored(world, entity);
        }
    }
    let foes: Vec<Entity> = world
        .query::<(Entity, &Enemy)>()
        .map(|(entity, _)| entity)
        .collect();
    for &enemy in &foes {
        enemies::restored(world, enemy);
    }
    impact_rules(world, recorded.iter().chain(&foes).copied());
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
    let holds: Vec<(Entity, Entity)> = world
        .query::<(Entity, &Holding)>()
        .map(|(troll, holding)| (troll, holding.0))
        .collect();
    for (troll, object) in holds {
        world.remove_component::<Holding>(troll);
        if !world.is_alive(object) {
            continue;
        }
        // The kit's `carried` brings an engine record back held with no
        // body until its first step: let it go at rest and lift it again at
        // once, so a paused load shows it held, as a kept ball, whose hold
        // no save captures, is lifted.
        if carrier_of(world, object) == Some(troll) {
            tungsten::kit::release(world, troll, Vec2::ZERO);
        }
        if carrier_of(world, object).is_none() {
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
    if let Some(centre) = crate::shared::body_centre(world, player)
        && let Some(camera) = world.get_resource_mut::<CameraController>()
    {
        camera.cut_to(centre);
    }
}

/// What an older save's bodies and enemies lack of their impact rules: a
/// record or an enemy without one takes its prefab's, and a body's rule
/// without a credit the run's player's, as its spawn gives them; then each
/// live throw's body takes its throw's rule ([`arm_throw`]), as the throw
/// set it.
fn impact_rules(world: &mut World, restored: impl Iterator<Item = Entity>) {
    let player = world.get_resource::<Run>().map(|run| run.player);
    for entity in restored.collect::<Vec<_>>() {
        let enemy = world.has::<Enemy>(entity) || world.has::<Dummy>(entity);
        if !world.has::<ImpactDamage>(entity) {
            let Some(rule) =
                prefab_of(world, entity).and_then(|prefab| toys::prefab_rule(world, &prefab))
            else {
                continue;
            };
            world.insert(entity, rule);
        }
        if !enemy
            && let Some(rule) = world.get_mut::<ImpactDamage>(entity)
            && rule.credit().is_none()
        {
            rule.set_credit(player);
        }
    }
    let throws = world
        .get_resource::<Throws>()
        .map(|throws| throws.0.clone())
        .unwrap_or_default();
    for throw in &throws {
        arm_throw(world, throw);
    }
}
