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
//! onto the kit's `damage_dealt`, and migrates a schema-29 save.
//!
//! A save names another entity by its save ID, never by its entity.
//! Presentation is not saved: particles, lights, glows, flames, overlays
//! and tints, animation and facing, flashes, shake, toasts, the HUD and the
//! music.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::SceneEntity;
use tungsten::core::save::{
    RestoreError, Value, WorldSave, capture_world, check_world, restore_world,
};
use tungsten::core::{CameraController, DataRef, Entity, Position, SaveData, Velocity, World};
use tungsten::kit::{ContactDamage, EffectRequest, Health, Stats};

use crate::features::dummy::{self, components::Dummy};
use crate::features::elements::components::ColdFace;
use crate::features::elements::systems::{coat_of, freeze_for, light, show_douse};
use crate::features::elements::{COAT, burn_left, ice_left};
use crate::features::enemies::components::{Enemy, Held, Holding, Throws};
use crate::features::enemies::{self, troll};
use crate::features::level::components::Hazard;
use crate::features::level::geyser::{self, Geyser};
use crate::features::level::lava::{self, Lava};
use crate::features::props;
use crate::features::run::Run;
use crate::features::tomes::{Tome, Tomes};
use crate::features::toys;
use crate::features::toys::components::{Ball, BallHue, IronScrap, PreviousVelocity, SmallBall};
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
    /// `damage_dealt` line ([`to_30`]).
    const SCHEMA: u32 = 30;

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

/// The dummies the map's `dummies` layer places.
pub const MAP_DUMMIES: u64 = 1;
/// The enemies the map's `enemies` layer places, the health orbs its
/// `pickups` layer places and the geysers its `geysers` layer places.
pub const MAP_ENEMIES: u64 = 36;
pub const MAP_PICKUPS: u64 = 8;
pub const MAP_GEYSERS: u64 = 3;

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
    rebuild(world, &touches);
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
/// plank's look; a crusted basin's body layers, mask, touch and glow, a
/// doused hazard's touch, glow and trail, a geyser's touch on in an
/// eruption and held aside else, from `touches`, the run's, where the save
/// holds none; a troll's hold lifted again; the taken tomes' books off the
/// map; the bodies the player stands in shoved; and the camera on the
/// player where the save left it.
fn rebuild(world: &mut World, touches: &[(Entity, Option<ContactDamage>)]) {
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
        }
    }
    let foes: Vec<Entity> = world
        .query::<(Entity, &Enemy)>()
        .map(|(entity, _)| entity)
        .collect();
    for enemy in foes {
        enemies::restored(world, enemy);
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
