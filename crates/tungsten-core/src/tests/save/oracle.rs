//! A seeded random-sequence oracle for world saves: a level of named
//! entities with saved components, a random run over it (writes, gains,
//! removals, despawns, spawns from saved and unsaved sources through the
//! World's command buffer, replacements, references and resources),
//! captured and restored into a fresh World with other allocations. The
//! restored World equals the live one at its capture for every saved
//! entity, component and resource, and its capture equals the first byte
//! for byte. A variant keeps the area `hall` out of the restore, then
//! enters it.
//!
//! The run never steps physics (S-59) and restores only captures, never a
//! hand-edited save (known-issues line 29): [`KNOWN`] in the ECS oracle.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{
    CaptureError, RefCodec, RefReader, RefWriter, SaveId, SaveOptions, SaveState, Saved, WorldSave,
    capture_world, enter_area, name, restore_world,
};
use crate::assets::{PrefabData, PrefabRegistry};
use crate::ecs::oracle::{Fail, Rng, drive, fail, guarded, seed_part, seeds};
use crate::ecs::{CommandBuffer, Entity, World};
use crate::physics::Position;

// ------------------------------------------------------------ the components

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inner {
    a: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    b: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Hp {
    cur: i32,
    max: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shield: Option<i32>,
    #[serde(default)]
    inner: Inner,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Mood {
    name: String,
    #[serde(default)]
    seen: Vec<u32>,
    #[serde(default)]
    heat: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Count(u32);

/// Serializes as `null`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct Marker;

/// Keys from `k0`–`k4`, never `-` (the patch's own key, `DESIGN`'s
/// `patch-dash`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Bag(BTreeMap<String, i32>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Stance {
    Idle,
    Walk { speed: i32 },
    Guard(u8),
}

/// A reference its saved form leaves out.
#[derive(Debug, Clone, PartialEq)]
struct Link {
    target: Entity,
    weight: u8,
    /// Reference changes, counted in the form unless the run is a
    /// `fresh_refs` one.
    moves: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkForm {
    weight: u8,
    #[serde(default)]
    moves: u32,
}

impl From<&Link> for LinkForm {
    fn from(link: &Link) -> Self {
        Self {
            weight: link.weight,
            moves: link.moves,
        }
    }
}

impl From<LinkForm> for Link {
    fn from(form: LinkForm) -> Self {
        Self {
            target: Entity::PLACEHOLDER,
            weight: form.weight,
            moves: form.moves,
        }
    }
}

/// A list of references under its key, which `load` extends, as the
/// area tests' and the kit's hit lists do.
#[derive(Debug, Clone, PartialEq)]
struct Hits {
    key: u32,
    targets: Vec<Entity>,
    moves: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HitsForm {
    key: u32,
    #[serde(default)]
    moves: u32,
}

impl From<&Hits> for HitsForm {
    fn from(hits: &Hits) -> Self {
        Self {
            key: hits.key,
            moves: hits.moves,
        }
    }
}

impl From<HitsForm> for Hits {
    fn from(form: HitsForm) -> Self {
        Self {
            key: form.key,
            targets: Vec::new(),
            moves: form.moves,
        }
    }
}

fn save_hits(hits: &Hits, out: &mut RefWriter<'_>) {
    for &target in &hits.targets {
        out.put(&hits.key.to_string(), target);
    }
}

fn load_hits(hits: &mut Hits, refs: &RefReader<'_>) {
    hits.targets
        .extend_from_slice(refs.get(&hits.key.to_string()));
}

fn hits_keys(hits: &Hits) -> Vec<String> {
    vec![hits.key.to_string()]
}

fn save_link(link: &Link, out: &mut RefWriter<'_>) {
    out.put("target", link.target);
}

fn load_link(link: &mut Link, refs: &RefReader<'_>) {
    if let Some(target) = refs.first("target") {
        link.target = target;
    }
}

fn link_keys(_: &Link) -> Vec<String> {
    vec!["target".into()]
}

/// `Position` as `[x, y]`, the form a restore places a spawn by.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct PosForm([f32; 2]);

impl From<&Position> for PosForm {
    fn from(position: &Position) -> Self {
        Self(position.0.to_array())
    }
}

impl From<PosForm> for Position {
    fn from(form: PosForm) -> Self {
        Self(form.0.into())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Weather {
    raining: bool,
    wind: f32,
    #[serde(default)]
    pressure: f64,
    #[serde(default)]
    log: Vec<String>,
}

/// A resource with a reference.
#[derive(Debug, Clone, PartialEq)]
struct Boss {
    target: Option<Entity>,
    rage: u32,
}

#[derive(Serialize, Deserialize)]
struct BossForm {
    rage: u32,
}

impl From<&Boss> for BossForm {
    fn from(boss: &Boss) -> Self {
        Self { rage: boss.rage }
    }
}

impl From<BossForm> for Boss {
    fn from(form: BossForm) -> Self {
        Self {
            target: None,
            rage: form.rage,
        }
    }
}

fn save_boss(boss: &Boss, out: &mut RefWriter<'_>) {
    if let Some(target) = boss.target {
        out.put("target", target);
    }
}

fn load_boss(boss: &mut Boss, refs: &RefReader<'_>) {
    if let Some(target) = refs.first("target") {
        boss.target = Some(target);
    }
}

fn boss_keys(_: &Boss) -> Vec<String> {
    vec!["target".into()]
}

/// What a restored spawn takes, as a state's marker; unsaved.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Scene;

const PREFABS: [&str; 5] = ["bolt", "orb", "husk", "plain", "brute"];
const AREAS: [Option<&str>; 3] = [None, Some("cave"), Some(HALL)];
const HALL: &str = "hall";

/// A World with the components saved and the prefabs in, as a game's boot
/// leaves it.
fn setup() -> World {
    let mut world = World::new();
    world.register_component::<Hp>("hp");
    world.register_component::<Mood>("mood");
    world.register_component::<Count>("count");
    world.register_component::<Marker>("marker");
    world.register_component::<Bag>("bag");
    world.register_component::<Stance>("stance");
    world.register_saved::<Hp>("hp");
    world.register_saved::<Mood>("mood");
    world.register_saved::<Count>("count");
    world.register_saved::<Marker>("marker");
    world.register_saved::<Bag>("bag");
    world.register_saved::<Stance>("stance");
    world.register_saved_with(
        "link",
        SaveOptions::<Link>::form::<LinkForm>().refs(RefCodec {
            save: save_link,
            load: load_link,
            keys: link_keys,
        }),
    );
    world.register_saved_with(
        "hits",
        SaveOptions::<Hits>::form::<HitsForm>().refs(RefCodec {
            save: save_hits,
            load: load_hits,
            keys: hits_keys,
        }),
    );
    world.register_saved_with("position", SaveOptions::<Position>::form::<PosForm>());
    world.register_saved_resource::<Weather>("weather");
    world.register_saved_resource_with(
        "boss",
        SaveOptions::<Boss>::form::<BossForm>().refs(RefCodec {
            save: save_boss,
            load: load_boss,
            keys: boss_keys,
        }),
    );
    world.inherit_from_source::<Count>();
    let prefab = |value| -> PrefabData { serde_json::from_value(value).unwrap() };
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert(
        "bolt".into(),
        prefab(json!({ "tag": "bolt", "components": {
            "hp": { "cur": 1, "max": 1 }, "count": 2 } })),
    );
    prefabs.insert(
        "orb".into(),
        prefab(json!({ "tag": "orb", "components": {
            "mood": { "name": "orb" }, "marker": null } })),
    );
    prefabs.insert(
        "husk".into(),
        prefab(json!({ "tag": "husk", "components": {
            "bag": { "k1": 1 }, "stance": "Idle" } })),
    );
    prefabs.insert("plain".into(), prefab(json!({ "tag": "plain" })));
    prefabs.insert(
        "brute".into(),
        prefab(json!({ "components": {
            "hp": { "cur": 9, "max": 9, "shield": 2, "inner": { "a": 1, "b": 2 } },
            "stance": { "Walk": { "speed": 3 } },
            "mood": { "name": "brute", "seen": [1, 2], "heat": 0.5 } } })),
    );
    world.insert_resource(prefabs);
    world.insert_resource(CommandBuffer::new());
    world
}

// ---------------------------------------------------------- values and kinds

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SKind {
    Hp,
    Mood,
    Count,
    Marker,
    Bag,
    Stance,
    Link,
    Hits,
    Pos,
}

const SKINDS: [SKind; 9] = [
    SKind::Hp,
    SKind::Mood,
    SKind::Count,
    SKind::Marker,
    SKind::Bag,
    SKind::Stance,
    SKind::Link,
    SKind::Hits,
    SKind::Pos,
];

#[derive(Clone, Debug)]
enum SVal {
    Hp(Hp),
    Mood(Mood),
    Count(u32),
    Marker,
    Bag(BTreeMap<String, i32>),
    Stance(Stance),
    /// To the saved entity `to` (by save ID order), or a dangling one.
    Link {
        to: u32,
        weight: u8,
    },
    /// Under key `key`, to the saved entities `to`.
    Hits {
        key: u8,
        to: Vec<u32>,
    },
    Pos(f32, f32),
}

/// Whether `TUNGSTEN_ORACLE_FOUND` (a comma list, or `all`) lets one of
/// [`FOUND`]'s cases into the run.
fn found_on(id: &str) -> bool {
    std::env::var("TUNGSTEN_ORACLE_FOUND").is_ok_and(|found| {
        found
            .split(',')
            .any(|one| one.trim() == id || one.trim() == "all")
    })
}

fn float(rng: &mut Rng, any: bool) -> f32 {
    let n = rng.below(400) as f32 - 200.0;
    if any && rng.chance(50) {
        n * 0.1
    } else {
        n * 0.25
    }
}

fn float64(rng: &mut Rng, any: bool) -> f64 {
    let n = rng.below(4000) as f64 - 2000.0;
    if any && rng.chance(50) {
        n * 0.1
    } else {
        n * 0.25
    }
}

/// The oracle's own findings, each with its `new-<n>.md` in
/// `perf-runs/20261107-core-hardening/oracle-ecs/` and the case the
/// default run leaves out.
pub(crate) const FOUND: &[(&str, &str, &str)] = &[
    (
        "float-parse",
        "new-1.md",
        "a float whose JSON form takes 17 digits: the default run writes multiples of 0.25",
    ),
    (
        "ref-order",
        "new-3.md",
        "one key's references split between a loaded area and a kept one: the default run's \
         kept runs drop `hall`'s targets from a hit list that also names others",
    ),
];

fn random_val(kind: SKind, rng: &mut Rng, any: bool) -> SVal {
    match kind {
        SKind::Hp => SVal::Hp(Hp {
            cur: rng.below(60) as i32 - 10,
            max: 50,
            shield: rng.chance(40).then(|| rng.below(9) as i32),
            inner: Inner {
                a: rng.below(5) as i32,
                b: rng.chance(50).then(|| rng.below(5) as i32),
            },
        }),
        SKind::Mood => SVal::Mood(Mood {
            name: ["calm", "angry", "wild", "odd"][rng.below(4) as usize].into(),
            seen: (0..rng.below(4)).map(|_| rng.below(9) as u32).collect(),
            heat: float(rng, any),
        }),
        SKind::Count => SVal::Count(rng.below(100) as u32),
        SKind::Marker => SVal::Marker,
        SKind::Bag => SVal::Bag(
            (0..rng.below(4))
                .map(|_| (format!("k{}", rng.below(5)), rng.below(20) as i32))
                .collect(),
        ),
        SKind::Stance => SVal::Stance(match rng.below(3) {
            0 => Stance::Idle,
            1 => Stance::Walk {
                speed: rng.below(9) as i32,
            },
            _ => Stance::Guard(rng.below(4) as u8),
        }),
        SKind::Link => SVal::Link {
            to: rng.small(),
            weight: rng.below(200) as u8,
        },
        SKind::Hits => SVal::Hits {
            key: rng.below(4) as u8,
            to: (0..rng.below(3)).map(|_| rng.small()).collect(),
        },
        SKind::Pos => SVal::Pos(float(rng, any), float(rng, any)),
    }
}

/// Inserts `val`: a link or a hit list counts one move past the one it
/// replaces when `bump`.
fn insert_sval(world: &mut World, entity: Entity, val: &SVal, targets: &[Entity], bump: bool) {
    let link_moves = world
        .get::<Link>(entity)
        .map_or(0, |link| link.moves + u32::from(bump));
    let hits_moves = world
        .get::<Hits>(entity)
        .map_or(0, |hits| hits.moves + u32::from(bump));
    match val {
        SVal::Hp(v) => world.insert(entity, v.clone()),
        SVal::Mood(v) => world.insert(entity, v.clone()),
        SVal::Count(v) => world.insert(entity, Count(*v)),
        SVal::Marker => world.insert(entity, Marker),
        SVal::Bag(v) => world.insert(entity, Bag(v.clone())),
        SVal::Stance(v) => world.insert(entity, v.clone()),
        SVal::Link { to, weight } => world.insert(
            entity,
            Link {
                target: pick_in(targets, *to).unwrap_or(Entity::PLACEHOLDER),
                weight: *weight,
                moves: link_moves,
            },
        ),
        SVal::Hits { key, to } => world.insert(
            entity,
            Hits {
                key: u32::from(*key),
                targets: to.iter().filter_map(|&k| pick_in(targets, k)).collect(),
                moves: hits_moves,
            },
        ),
        SVal::Pos(x, y) => world.insert(entity, Position(glam::Vec2::new(*x, *y))),
    }
}

fn insert_sval_pending(
    buffer: &mut CommandBuffer,
    pending: crate::ecs::PendingEntity,
    val: &SVal,
    targets: &[Entity],
) {
    match val {
        SVal::Hp(v) => buffer.insert_pending(pending, v.clone()),
        SVal::Mood(v) => buffer.insert_pending(pending, v.clone()),
        SVal::Count(v) => buffer.insert_pending(pending, Count(*v)),
        SVal::Marker => buffer.insert_pending(pending, Marker),
        SVal::Bag(v) => buffer.insert_pending(pending, Bag(v.clone())),
        SVal::Stance(v) => buffer.insert_pending(pending, v.clone()),
        SVal::Link { to, weight } => buffer.insert_pending(
            pending,
            Link {
                target: pick_in(targets, *to).unwrap_or(Entity::PLACEHOLDER),
                weight: *weight,
                moves: 0,
            },
        ),
        SVal::Hits { key, to } => buffer.insert_pending(
            pending,
            Hits {
                key: u32::from(*key),
                targets: to.iter().filter_map(|&k| pick_in(targets, k)).collect(),
                moves: 0,
            },
        ),
        SVal::Pos(x, y) => buffer.insert_pending(pending, Position(glam::Vec2::new(*x, *y))),
    }
}

fn remove_kind(world: &mut World, entity: Entity, kind: SKind) {
    match kind {
        SKind::Hp => drop(world.remove_component::<Hp>(entity)),
        SKind::Mood => drop(world.remove_component::<Mood>(entity)),
        SKind::Count => drop(world.remove_component::<Count>(entity)),
        SKind::Marker => drop(world.remove_component::<Marker>(entity)),
        SKind::Bag => drop(world.remove_component::<Bag>(entity)),
        SKind::Stance => drop(world.remove_component::<Stance>(entity)),
        SKind::Link => drop(world.remove_component::<Link>(entity)),
        SKind::Hits => drop(world.remove_component::<Hits>(entity)),
        SKind::Pos => drop(world.remove_component::<Position>(entity)),
    }
}

/// A run's write through `get_mut`; a reference change counts a move when
/// `bump`.
fn write_kind(
    world: &mut World,
    entity: Entity,
    kind: SKind,
    x: u32,
    targets: &[Entity],
    bump: bool,
) {
    match kind {
        SKind::Hp => {
            if let Some(hp) = world.get_mut::<Hp>(entity) {
                match x % 4 {
                    0 => hp.cur -= (x % 7) as i32,
                    1 => hp.shield = if hp.shield.is_some() { None } else { Some(1) },
                    2 => hp.inner.a += 1,
                    _ => {
                        hp.inner.b = if hp.inner.b.is_some() {
                            None
                        } else {
                            Some(x as i32 % 9)
                        }
                    }
                }
            }
        }
        SKind::Mood => {
            if let Some(mood) = world.get_mut::<Mood>(entity) {
                match x % 3 {
                    0 => mood.seen.push(x % 11),
                    1 => mood.heat = (x % 37) as f32 * 0.25,
                    _ => mood.name = format!("m{}", x % 5),
                }
            }
        }
        SKind::Count => {
            if let Some(count) = world.get_mut::<Count>(entity) {
                count.0 = count.0.wrapping_add(x % 13);
            }
        }
        SKind::Marker => {}
        SKind::Bag => {
            if let Some(bag) = world.get_mut::<Bag>(entity) {
                let key = format!("k{}", x % 5);
                if x.is_multiple_of(3) {
                    bag.0.remove(&key);
                } else {
                    bag.0.insert(key, (x % 50) as i32);
                }
            }
        }
        SKind::Stance => {
            if let Some(stance) = world.get_mut::<Stance>(entity) {
                *stance = match x % 3 {
                    0 => Stance::Idle,
                    1 => Stance::Walk {
                        speed: (x % 9) as i32,
                    },
                    _ => Stance::Guard((x % 4) as u8),
                };
            }
        }
        SKind::Link => {
            let target = pick_in(targets, x);
            if let Some(link) = world.get_mut::<Link>(entity) {
                if x.is_multiple_of(4) {
                    link.weight = (x % 200) as u8;
                } else if let Some(target) = target {
                    link.target = target;
                    link.moves += u32::from(bump);
                }
            }
        }
        SKind::Hits => {
            let target = pick_in(targets, x);
            if let Some(hits) = world.get_mut::<Hits>(entity) {
                match x % 3 {
                    0 => {
                        hits.targets.extend(target);
                        hits.moves += u32::from(bump);
                    }
                    1 => {
                        hits.targets.clear();
                        hits.moves += u32::from(bump);
                    }
                    _ => hits.key = x % 4,
                }
            }
        }
        SKind::Pos => {
            if let Some(position) = world.get_mut::<Position>(entity) {
                position.0.x += (x % 9) as f32 * 0.5;
            }
        }
    }
}

// ------------------------------------------------------------------- steps

/// What a run picks a source or a target from.
#[derive(Clone, Copy, Debug)]
enum Src {
    /// A saved entity, by save ID order.
    Saved(u32),
    /// An entity without a save ID.
    Plain(u32),
    /// An entity that has died.
    Dead,
}

#[derive(Clone, Debug)]
enum SOp {
    Write(u32, SKind, u32),
    Gain(u32, SVal),
    Lose(u32, SKind),
    Despawn(u32),
    /// A spawn from `Src` through the World's buffer: its prefab and
    /// place, a `with` count, whether it is prepared unsaved, and what is
    /// inserted on it after.
    Shoot {
        from: Src,
        prefab: u8,
        at: (f32, f32),
        with_count: Option<u32>,
        unsaved: bool,
        extra: Vec<SVal>,
    },
    Replace(u32, u8, bool),
    QueueDespawn(u32),
    /// A plain spawn with saved components, unsaved itself.
    Plain(Vec<SVal>),
    /// A reference held to an entity the save cannot name.
    Dangle(u32, Src),
    Weather(u32, f64),
    Boss(Src, u32),
    Churn(u32),
    Flush,
}

#[derive(Clone, Debug)]
enum Step {
    /// A level entity named `e<n>`: its area, prefab (`None` for a plain
    /// spawn), components, and a link to another level entity.
    Ent {
        area: u8,
        prefab: Option<u8>,
        comps: Vec<SVal>,
        link: Option<(u32, u8)>,
    },
    /// A level entity left unnamed, with saved components.
    Unnamed(Vec<SVal>),
    /// The restoring World: other allocations first, and whether it keeps
    /// `hall` out of the restore and enters it after; and whether the run
    /// tidies its references before the capture, as a game retargets a
    /// link whose target died, so the capture drops none.
    Restore {
        churn: u32,
        keep_hall: bool,
        tidy: bool,
        /// Whether reference changes leave the saved form as it was and
        /// level hit lists start with a target (`fresh-refs`'s case, half
        /// the runs).
        fresh_refs: bool,
        /// Whether a hit list may name `hall` and other areas at once in a
        /// kept run (`ref-order`, [`FOUND`]).
        split_refs: bool,
    },
    Op(SOp),
}

fn comps(rng: &mut Rng, any: bool) -> Vec<SVal> {
    let mut vals = Vec::new();
    for kind in SKINDS {
        if kind != SKind::Link && rng.chance(30) {
            vals.push(random_val(kind, rng, any));
        }
    }
    vals
}

fn src(rng: &mut Rng) -> Src {
    match rng.weighted(&[80, 12, 8]) {
        0 => Src::Saved(rng.small()),
        1 => Src::Plain(rng.small()),
        _ => Src::Dead,
    }
}

fn skind(rng: &mut Rng) -> SKind {
    SKINDS[rng.below(SKINDS.len() as u64) as usize]
}

fn generate(seed: u64, len: usize) -> Vec<Step> {
    let mut rng = Rng::new(seed ^ 0x5a5a_0000);
    let any = found_on("float-parse");
    let mut steps = Vec::new();
    let ents = if rng.chance(20) { 0 } else { 2 + rng.below(9) };
    for _ in 0..ents {
        steps.push(Step::Ent {
            area: rng.below(3) as u8,
            prefab: rng
                .chance(50)
                .then(|| rng.below(PREFABS.len() as u64) as u8),
            comps: comps(&mut rng, any),
            link: rng.chance(50).then(|| (rng.small(), rng.below(200) as u8)),
        });
    }
    for _ in 0..rng.below(3) {
        steps.push(Step::Unnamed(comps(&mut rng, any)));
    }
    steps.push(Step::Restore {
        churn: rng.below(40) as u32,
        keep_hall: rng.chance(35),
        tidy: rng.chance(75),
        fresh_refs: rng.chance(50),
        split_refs: found_on("ref-order"),
    });
    let dangle_weight = if rng.chance(30) { 3 } else { 0 };
    for _ in 0..len {
        let op = match rng.weighted(&[16, 8, 5, 4, 12, 4, 3, 2, dangle_weight, 2, 3, 2, 6]) {
            0 => SOp::Write(rng.small(), skind(&mut rng), rng.small()),
            1 => {
                let kind = skind(&mut rng);
                SOp::Gain(rng.small(), random_val(kind, &mut rng, any))
            }
            2 => SOp::Lose(rng.small(), skind(&mut rng)),
            3 => SOp::Despawn(rng.small()),
            4 => SOp::Shoot {
                from: src(&mut rng),
                prefab: rng.below(PREFABS.len() as u64) as u8,
                at: (float(&mut rng, any), float(&mut rng, any)),
                with_count: rng.chance(20).then(|| rng.below(9) as u32),
                unsaved: rng.chance(10),
                extra: (0..rng.below(3))
                    .map(|_| {
                        let kind = skind(&mut rng);
                        random_val(kind, &mut rng, any)
                    })
                    .collect(),
            },
            5 => SOp::Replace(
                rng.small(),
                rng.below(PREFABS.len() as u64) as u8,
                rng.chance(15),
            ),
            6 => SOp::QueueDespawn(rng.small()),
            7 => SOp::Plain(comps(&mut rng, any)),
            8 => SOp::Dangle(rng.small(), src(&mut rng)),
            9 => SOp::Weather(rng.small(), float64(&mut rng, any)),
            10 => SOp::Boss(src(&mut rng), rng.small()),
            11 => SOp::Churn(rng.below(6) as u32),
            _ => SOp::Flush,
        };
        steps.push(Step::Op(op));
    }
    steps
}

// --------------------------------------------------------------- the worlds

fn pick_in(list: &[Entity], k: u32) -> Option<Entity> {
    (!list.is_empty()).then(|| list[k as usize % list.len()])
}

/// The live saved entities, by save ID.
fn saved_live(world: &World) -> Vec<Entity> {
    let mut found: Vec<(SaveId, Entity)> = world
        .query::<(Entity, &Saved)>()
        .map(|(entity, saved)| (saved.id(), entity))
        .collect();
    found.sort_by_key(|&(id, _)| id);
    found.into_iter().map(|(_, entity)| entity).collect()
}

/// The live entities without a save ID, by index.
fn plain_live(world: &World) -> Vec<Entity> {
    let mut found: Vec<Entity> = world
        .query::<Entity>()
        .filter(|&entity| !world.has::<Saved>(entity))
        .collect();
    found.sort_by_key(|entity| entity.id());
    found
}

fn source(world: &mut World, from: Src) -> Option<Entity> {
    match from {
        Src::Saved(k) => pick_in(&saved_live(world), k),
        Src::Plain(k) => pick_in(&plain_live(world), k),
        Src::Dead => {
            let gone = world.spawn();
            world.despawn(gone);
            Some(gone)
        }
    }
}

fn flush(world: &mut World) {
    let mut buffer = std::mem::take(world.resource_mut::<CommandBuffer>());
    world.flush_reusing(&mut buffer);
    *world.resource_mut::<CommandBuffer>() = buffer;
}

fn churn(world: &mut World, count: u32) {
    let spawned: Vec<Entity> = (0..count).map(|_| world.spawn()).collect();
    for (i, entity) in spawned.into_iter().enumerate() {
        if i % 3 != 1 {
            world.despawn(entity);
        }
    }
}

/// Which of the level's areas a build makes.
#[derive(Clone, Copy, PartialEq)]
enum Build {
    All,
    Without(&'static str),
    Only(&'static str),
}

impl Build {
    fn makes(self, area: Option<&str>) -> bool {
        match self {
            Build::All => true,
            Build::Without(skip) => area != Some(skip),
            Build::Only(only) => area == Some(only),
        }
    }
}

/// A level entity's area, prefab, components and link, from its step.
type LevelEnt<'a> = (u8, Option<u8>, &'a Vec<SVal>, Option<(u32, u8)>);

/// Builds the level's entities that `build` makes, links them (to an
/// entity `built` holds, or one this build makes), names them, then the
/// unnamed ones and the resources (in `All` and `Without` builds).
fn build_level(
    world: &mut World,
    steps: &[Step],
    build: Build,
    built: &mut BTreeMap<usize, Entity>,
) -> Result<(), Fail> {
    let fresh_hits = steps.iter().any(|step| {
        matches!(
            step,
            Step::Restore {
                fresh_refs: true,
                ..
            }
        )
    });
    let ents: Vec<LevelEnt<'_>> = steps
        .iter()
        .filter_map(|step| match step {
            Step::Ent {
                area,
                prefab,
                comps,
                link,
            } => Some((*area, *prefab, comps, *link)),
            _ => None,
        })
        .collect();
    let mut made = Vec::new();
    for (index, &(area, prefab, comps, _)) in ents.iter().enumerate() {
        if !build.makes(AREAS[area as usize]) {
            continue;
        }
        let entity = match prefab {
            Some(p) => world
                .spawn_prefab(PREFABS[p as usize], [index as f32 * 16.0, 8.0])
                .map_err(|err| Fail {
                    tag: "level-prefab".into(),
                    detail: err.to_string(),
                    step: 0,
                })?,
            None => world.spawn(),
        };
        for val in comps {
            insert_sval(world, entity, val, &[], false);
        }
        built.insert(index, entity);
        made.push(index);
    }
    for &index in &made {
        if let Some((to, weight)) = ents[index].3 {
            let target = built
                .get(&(to as usize % ents.len()))
                .copied()
                .unwrap_or(Entity::PLACEHOLDER);
            let holder = built[&index];
            world.insert(
                holder,
                Link {
                    target,
                    weight,
                    moves: 0,
                },
            );
            // A level hit list starts with the same target (`fresh-refs`).
            if fresh_hits && let Some(hits) = world.get_mut::<Hits>(holder) {
                hits.targets.push(target);
            }
        }
    }
    for &index in &made {
        let area = AREAS[ents[index].0 as usize];
        name(world, built[&index], area, &format!("e{index}")).map_err(|err| Fail {
            tag: "level-name".into(),
            detail: err.to_string(),
            step: 0,
        })?;
    }
    if !matches!(build, Build::Only(_)) {
        for step in steps {
            if let Step::Unnamed(comps) = step {
                let entity = world.spawn();
                for val in comps {
                    insert_sval(world, entity, val, &[], false);
                }
            }
        }
        world.insert_resource(Weather {
            raining: false,
            wind: 1.5,
            pressure: 1.0,
            log: Vec::new(),
        });
        let target = built.values().next().copied();
        world.insert_resource(Boss { target, rage: 1 });
    }
    Ok(())
}

/// Retargets each saved link and the boss whose target a save cannot name
/// to the first live saved entity, or removes the link when there is none,
/// and drops such targets from each hit list, each a move when `bump`.
fn tidy(world: &mut World, bump: bool) {
    let saved = saved_live(world);
    let nameable =
        |world: &World, target: Entity| world.is_alive(target) && world.has::<Saved>(target);
    for &holder in &saved {
        if let Some(target) = world.get::<Link>(holder).map(|link| link.target)
            && !nameable(world, target)
        {
            match saved.first() {
                Some(&first) => {
                    let link = world.get_mut::<Link>(holder).unwrap();
                    link.target = first;
                    link.moves += u32::from(bump);
                }
                None => drop(world.remove_component::<Link>(holder)),
            }
        }
        if let Some(targets) = world.get::<Hits>(holder).map(|hits| hits.targets.clone())
            && targets.iter().any(|&target| !nameable(world, target))
        {
            let kept: Vec<Entity> = targets
                .into_iter()
                .filter(|&target| nameable(world, target))
                .collect();
            let hits = world.get_mut::<Hits>(holder).unwrap();
            hits.targets = kept;
            hits.moves += u32::from(bump);
        }
    }
    if let Some(target) = world.resource::<Boss>().target
        && !nameable(world, target)
    {
        world.resource_mut::<Boss>().target = saved.first().copied();
    }
}

/// Drops `hall`'s targets from each hit list that also names an entity
/// elsewhere (`ref-order`), each a move when `bump`.
fn unsplit(world: &mut World, bump: bool) {
    let in_hall = |world: &World, target: Entity| {
        world
            .get::<Saved>(target)
            .is_some_and(|saved| saved.area() == Some(HALL))
    };
    for holder in saved_live(world) {
        let Some(targets) = world.get::<Hits>(holder).map(|hits| hits.targets.clone()) else {
            continue;
        };
        let hall = targets
            .iter()
            .filter(|&&target| in_hall(world, target))
            .count();
        if hall == 0 || hall == targets.len() {
            continue;
        }
        let kept: Vec<Entity> = targets
            .into_iter()
            .filter(|&target| !in_hall(world, target))
            .collect();
        let hits = world.get_mut::<Hits>(holder).unwrap();
        hits.targets = kept;
        hits.moves += u32::from(bump);
    }
}

/// Applies `op` to the live World; each reference change counts a move in
/// its value's saved form when `bump`.
fn apply(world: &mut World, op: &SOp, bump: bool) {
    match op {
        SOp::Write(k, kind, x) => {
            let saved = saved_live(world);
            if let Some(entity) = pick_in(&saved, *k) {
                write_kind(world, entity, *kind, *x, &saved, bump);
            }
        }
        SOp::Gain(k, val) => {
            let saved = saved_live(world);
            if let Some(entity) = pick_in(&saved, *k) {
                insert_sval(world, entity, val, &saved, bump);
            }
        }
        SOp::Lose(k, kind) => {
            if let Some(entity) = pick_in(&saved_live(world), *k) {
                remove_kind(world, entity, *kind);
            }
        }
        SOp::Despawn(k) => {
            if let Some(entity) = pick_in(&saved_live(world), *k) {
                world.despawn(entity);
            }
        }
        SOp::Shoot {
            from,
            prefab,
            at,
            with_count,
            unsaved,
            extra,
        } => {
            let Some(from) = source(world, *from) else {
                return;
            };
            let mut prepared = world
                .prepare_prefab(PREFABS[*prefab as usize], [at.0, at.1])
                .expect("the oracle's prefabs prepare");
            if let Some(count) = with_count {
                prepared = prepared.with((Count(*count),));
            }
            if *unsaved {
                prepared = prepared.unsaved();
            }
            let saved = saved_live(world);
            let buffer = world.resource_mut::<CommandBuffer>();
            let pending = buffer.spawn_prefab_from(from, prepared);
            for val in extra {
                insert_sval_pending(buffer, pending, val, &saved);
            }
        }
        SOp::Replace(k, prefab, unsaved) => {
            let Some(entity) = pick_in(&saved_live(world), *k) else {
                return;
            };
            let mut prepared = world
                .prepare_prefab(PREFABS[*prefab as usize], [0.0, 0.0])
                .expect("the oracle's prefabs prepare");
            if *unsaved {
                prepared = prepared.unsaved();
            }
            world
                .resource_mut::<CommandBuffer>()
                .replace_prefab(entity, prepared);
        }
        SOp::QueueDespawn(k) => {
            if let Some(entity) = pick_in(&saved_live(world), *k) {
                world.resource_mut::<CommandBuffer>().despawn(entity);
            }
        }
        SOp::Plain(comps) => {
            let entity = world.spawn();
            let saved = saved_live(world);
            for val in comps {
                insert_sval(world, entity, val, &saved, bump);
            }
        }
        SOp::Dangle(k, to) => {
            let Some(holder) = pick_in(&saved_live(world), *k) else {
                return;
            };
            let target = match to {
                Src::Saved(_) | Src::Dead => source(world, Src::Dead),
                Src::Plain(_) => source(world, *to),
            };
            if let (Some(target), Some(link)) = (target, world.get_mut::<Link>(holder)) {
                link.target = target;
                link.moves += u32::from(bump);
            }
        }
        SOp::Weather(x, pressure) => {
            let weather = world.resource_mut::<Weather>();
            match x % 4 {
                0 => weather.raining = !weather.raining,
                1 => weather.wind = (x % 17) as f32 * 0.5,
                2 => weather.pressure = *pressure,
                _ => weather.log.push(format!("w{}", x % 7)),
            }
        }
        SOp::Boss(to, rage) => {
            let target = source(world, *to);
            let boss = world.resource_mut::<Boss>();
            boss.rage = *rage % 1000;
            if target.is_some() {
                boss.target = target;
            }
        }
        SOp::Churn(n) => churn(world, *n),
        SOp::Flush => flush(world),
    }
}

// --------------------------------------------------------------- comparing

/// The live saved entities by save ID; fails on an ID two hold.
fn by_id(world: &World, side: &str, skip: Option<&str>) -> Result<BTreeMap<SaveId, Entity>, Fail> {
    let mut map = BTreeMap::new();
    for (entity, saved) in world.query::<(Entity, &Saved)>() {
        if skip.is_some() && saved.area() == skip {
            continue;
        }
        if let Some(other) = map.insert(saved.id(), entity) {
            return fail(
                "duplicate-save-id",
                format!(
                    "{side}: {entity:?} and {other:?} both hold {:?}",
                    saved.id()
                ),
            );
        }
    }
    Ok(map)
}

/// A reference's target as a save names it: its ID, or `None` for an
/// entity the save cannot name (dead, or without an ID); a target in
/// `skip`, an area the restore kept, is `Err` (not compared).
fn ref_of(world: &World, target: Entity, skip: Option<&str>) -> Result<Option<SaveId>, ()> {
    if !world.is_alive(target) {
        return Ok(None);
    }
    match world.get::<Saved>(target) {
        Some(saved) if skip.is_some() && saved.area() == skip => Err(()),
        Some(saved) => Ok(Some(saved.id())),
        None => Ok(None),
    }
}

fn same<T: PartialEq + std::fmt::Debug>(
    what: &str,
    id: SaveId,
    a: Option<&T>,
    b: Option<&T>,
) -> Result<(), Fail> {
    if a == b {
        Ok(())
    } else {
        fail(
            &format!("restored-{what}"),
            format!("save ID {}: live {a:?}, restored {b:?}", id.0),
        )
    }
}

/// The restored World `b` equals the live `a` for every saved entity,
/// component and resource; an area in `skip` is left out of both.
/// Returns how many references it could not compare (dangling in `a`).
fn compare(a: &World, b: &World, skip: Option<&str>) -> Result<u32, Fail> {
    let live = by_id(a, "live", skip)?;
    let restored = by_id(b, "restored", None)?;
    let ids = |map: &BTreeMap<SaveId, Entity>| map.keys().map(|id| id.0).collect::<Vec<_>>();
    if ids(&live) != ids(&restored) {
        return fail(
            "restored-ids",
            format!("live IDs {:?}, restored {:?}", ids(&live), ids(&restored)),
        );
    }
    let mut loose = 0;
    for (&id, &ea) in &live {
        let eb = restored[&id];
        same("saved", id, a.get::<Saved>(ea), b.get::<Saved>(eb))?;
        same("hp", id, a.get::<Hp>(ea), b.get::<Hp>(eb))?;
        same("mood", id, a.get::<Mood>(ea), b.get::<Mood>(eb))?;
        same("count", id, a.get::<Count>(ea), b.get::<Count>(eb))?;
        same("marker", id, a.get::<Marker>(ea), b.get::<Marker>(eb))?;
        same("bag", id, a.get::<Bag>(ea), b.get::<Bag>(eb))?;
        same("stance", id, a.get::<Stance>(ea), b.get::<Stance>(eb))?;
        same("position", id, a.get::<Position>(ea), b.get::<Position>(eb))?;
        let la = a.get::<Link>(ea);
        let lb = b.get::<Link>(eb);
        same(
            "link",
            id,
            la.map(|link| link.weight).as_ref(),
            lb.map(|link| link.weight).as_ref(),
        )?;
        same(
            "link-moves",
            id,
            la.map(|link| link.moves).as_ref(),
            lb.map(|link| link.moves).as_ref(),
        )?;
        let ha = a.get::<Hits>(ea);
        let hb = b.get::<Hits>(eb);
        same(
            "hits",
            id,
            ha.map(|hits| (hits.key, hits.moves)).as_ref(),
            hb.map(|hits| (hits.key, hits.moves)).as_ref(),
        )?;
        if let (Some(ha), Some(hb)) = (ha, hb) {
            let live: Result<Vec<Option<SaveId>>, ()> = ha
                .targets
                .iter()
                .map(|&target| ref_of(a, target, skip))
                .collect();
            match live {
                Ok(live) if live.iter().all(Option::is_some) => {
                    let got: Vec<Option<SaveId>> = hb
                        .targets
                        .iter()
                        .map(|&target| ref_of(b, target, None).unwrap_or(None))
                        .collect();
                    if got != live {
                        return fail(
                            "restored-hits-targets",
                            format!("save ID {}: live {live:?}, restored {got:?}", id.0),
                        );
                    }
                }
                Ok(_) => loose += 1,
                Err(()) => {}
            }
        }
        if let (Some(la), Some(lb)) = (la, lb) {
            match ref_of(a, la.target, skip) {
                Ok(Some(target)) => {
                    let got = ref_of(b, lb.target, None).unwrap_or(None);
                    if got != Some(target) {
                        return fail(
                            "restored-link-target",
                            format!(
                                "save ID {}: live target {}, restored {got:?}",
                                id.0, target.0
                            ),
                        );
                    }
                }
                Ok(None) => loose += 1,
                Err(()) => {}
            }
        }
    }
    same(
        "weather",
        SaveId(0),
        a.get_resource::<Weather>(),
        b.get_resource::<Weather>(),
    )?;
    let ba = a.get_resource::<Boss>();
    let bb = b.get_resource::<Boss>();
    same(
        "boss",
        SaveId(0),
        ba.map(|boss| boss.rage).as_ref(),
        bb.map(|boss| boss.rage).as_ref(),
    )?;
    if let (Some(ba), Some(bb)) = (ba, bb) {
        let got = bb
            .target
            .and_then(|target| ref_of(b, target, None).unwrap_or(None));
        match ba.target.map(|target| ref_of(a, target, skip)) {
            None if bb.target.is_some() => {
                return fail(
                    "restored-boss-target",
                    format!("live target none, restored {got:?}"),
                );
            }
            Some(Ok(Some(target))) if got != Some(target) => {
                return fail(
                    "restored-boss-target",
                    format!("live target {}, restored {got:?}", target.0),
                );
            }
            Some(Ok(None)) => loose += 1,
            _ => {}
        }
    }
    if super::saving(a) != super::saving(b) {
        return fail(
            "restored-saving",
            format!(
                "the live World saves: {}, the restored one: {}",
                super::saving(a),
                super::saving(b)
            ),
        );
    }
    let next = |world: &World| world.get_resource::<SaveState>().map(SaveState::next_id);
    if next(a) != next(b) {
        return fail(
            "restored-next-id",
            format!("live {:?}, restored {:?}", next(a), next(b)),
        );
    }
    Ok(loose)
}

fn text(save: &WorldSave) -> String {
    serde_json::to_string(save).expect("a capture serializes")
}

fn capture(world: &World, tag: &str) -> Result<WorldSave, Fail> {
    capture_world(world).map_err(|err| Fail {
        tag: tag.to_owned(),
        detail: err.to_string(),
        step: 0,
    })
}

/// `b`'s capture equals `first` byte for byte.
fn recapture(b: &World, first: &str, tag: &str) -> Result<(), Fail> {
    let second = text(&capture(b, tag)?);
    if second != first {
        let at = first
            .bytes()
            .zip(second.bytes())
            .position(|(x, y)| x != y)
            .unwrap_or(first.len().min(second.len()));
        let from = at.saturating_sub(120);
        return fail(
            tag,
            format!(
                "captures differ at byte {at}:\n  first  …{}\n  second …{}",
                &first[from..(at + 120).min(first.len())],
                &second[from..(at + 120).min(second.len())]
            ),
        );
    }
    Ok(())
}

/// What a run did, for `TUNGSTEN_ORACLE_STATS=1`.
#[derive(Default, Debug)]
struct Stats {
    runs: u64,
    saved_entities: u64,
    spawns_saved: u64,
    gone: u64,
    kept_runs: u64,
    dropped_ref_runs: u64,
    dropped_after_tidy: u64,
    loose_refs: u64,
    save_bytes: u64,
}

fn run(steps: &[Step]) -> Result<(), Fail> {
    run_with(steps, &mut Stats::default())
}

fn run_with(steps: &[Step], stats: &mut Stats) -> Result<(), Fail> {
    guarded(|| {
        let split_refs = steps.iter().any(|step| {
            matches!(
                step,
                Step::Restore {
                    split_refs: true,
                    ..
                }
            )
        });
        let (churn_b, keep_hall, tidy_refs, bump) = steps
            .iter()
            .rev()
            .find_map(|step| match step {
                Step::Restore {
                    churn,
                    keep_hall,
                    tidy,
                    fresh_refs,
                    ..
                } => Some((*churn, *keep_hall, *tidy, !*fresh_refs)),
                _ => None,
            })
            .unwrap_or((0, false, false, true));
        let mut a = setup();
        build_level(&mut a, steps, Build::All, &mut BTreeMap::new())?;
        for (index, step) in steps.iter().enumerate() {
            if let Step::Op(op) = step {
                guarded(|| {
                    apply(&mut a, op, bump);
                    Ok(())
                })
                .map_err(|mut err| {
                    err.step = index;
                    err.tag = format!("run {}", err.tag);
                    err
                })?;
            }
        }
        let queued = a.resource::<CommandBuffer>().len();
        if queued > 0 {
            match capture_world(&a) {
                Err(CaptureError::Queued { commands }) if commands == queued => {}
                other => {
                    return fail(
                        "capture-queued",
                        format!("{queued} commands queued, the capture gave {other:?}"),
                    );
                }
            }
        }
        flush(&mut a);
        if tidy_refs {
            tidy(&mut a, bump);
        }
        if keep_hall && !split_refs {
            unsplit(&mut a, bump);
        }

        let save = capture(&a, "capture")?;
        let first = text(&save);
        recapture(&a, &first, "capture-twice")?;
        let parsed: WorldSave = serde_json::from_str(&first).map_err(|err| Fail {
            tag: "parse".into(),
            detail: err.to_string(),
            step: 0,
        })?;
        if text(&parsed) != first {
            return fail("parse-round-trip", "the parsed save writes other bytes");
        }

        let mut b = setup();
        churn(&mut b, churn_b);
        let mut built = BTreeMap::new();
        let skip = keep_hall.then_some(HALL);
        let build = skip.map_or(Build::All, Build::Without);
        build_level(&mut b, steps, build, &mut built)?;
        let report = guarded(|| {
            let report = restore_world(&mut b, &parsed, &(Scene,)).map_err(|err| Fail {
                tag: "restore-error".into(),
                detail: err.to_string(),
                step: 0,
            })?;
            if report.is_clean() {
                Ok(())
            } else {
                fail("restore-report", format!("{:?}", report.dropped))
            }
        })
        .map_err(|mut err| {
            if err.tag.starts_with("panic") {
                err.detail = format!("restore: {}", err.detail);
            }
            err
        });
        report?;
        let clean = save.dropped_refs() == 0;
        let mut loose = compare(&a, &b, skip)?;
        if clean {
            recapture(&b, &first, "recapture")?;
        }
        if keep_hall {
            build_level(&mut b, steps, Build::Only(HALL), &mut built)?;
            let report = enter_area(&mut b, HALL, &(Scene,)).map_err(|err| Fail {
                tag: "enter-error".into(),
                detail: err.to_string(),
                step: 0,
            })?;
            if !report.is_clean() {
                return fail("enter-report", format!("{:?}", report.dropped));
            }
            loose += compare(&a, &b, None)?;
            if clean {
                recapture(&b, &first, "recapture-entered")?;
            }
        }

        stats.runs += 1;
        stats.saved_entities += saved_live(&a).len() as u64;
        stats.spawns_saved += a
            .query::<&Saved>()
            .filter(|saved| saved.prefab().is_some())
            .count() as u64;
        stats.gone += first.matches("\"gone\":[\"").count() as u64;
        stats.kept_runs += u64::from(keep_hall);
        stats.dropped_ref_runs += u64::from(!clean);
        stats.dropped_after_tidy += u64::from(!clean && tidy_refs);
        if !clean && tidy_refs && std::env::var_os("TUNGSTEN_ORACLE_WHY").is_some() {
            println!("dropped after tidy: {first}");
        }
        stats.loose_refs += u64::from(loose);
        stats.save_bytes += first.len() as u64;
        Ok(())
    })
}

/// The default run's seeds and run length (`notes.md` has the debug and
/// release times).
const SEEDS: u64 = 4_000;
const LEN: usize = 120;

#[test]
fn oracle_stats() {
    if std::env::var_os("TUNGSTEN_ORACLE_STATS").is_none() {
        return;
    }
    let mut stats = Stats::default();
    let seeds = seeds(0, SEEDS);
    let mut failed = 0;
    for &seed in &seeds {
        failed += u32::from(run_with(&generate(seed, LEN), &mut stats).is_err());
    }
    println!(
        "save oracle, {} seeds of {LEN} ops, {failed} failed: {stats:?}",
        seeds.len()
    );
}

/// The default seeds' quarter `part` (0 to 3).
fn restores_on_default_quarter(part: usize) {
    drive(
        "save oracle",
        &seed_part(0, SEEDS, part, 4),
        |seed| generate(seed, LEN),
        run,
    );
}

#[test]
fn a_restored_world_equals_the_live_one_and_captures_the_same_1() {
    restores_on_default_quarter(0);
}

#[test]
fn a_restored_world_equals_the_live_one_and_captures_the_same_2() {
    restores_on_default_quarter(1);
}

#[test]
fn a_restored_world_equals_the_live_one_and_captures_the_same_3() {
    restores_on_default_quarter(2);
}

#[test]
fn a_restored_world_equals_the_live_one_and_captures_the_same_4() {
    restores_on_default_quarter(3);
}

#[test]
#[ignore = "long mode: run by hand, about ten minutes in debug"]
fn a_restored_world_equals_the_live_one_and_captures_the_same_long() {
    drive(
        "save oracle (long)",
        &seeds(1_000_000, SEEDS * 20),
        |seed| generate(seed, LEN * 3),
        run,
    );
}

#[test]
fn the_save_oracle_catches_a_changed_value() {
    // A restored World that differs from the live one must fail.
    let steps = generate(7, LEN);
    let mut a = setup();
    build_level(&mut a, &steps, Build::All, &mut BTreeMap::new()).unwrap();
    let save = capture_world(&a).unwrap();
    let mut b = setup();
    build_level(&mut b, &steps, Build::All, &mut BTreeMap::new()).unwrap();
    restore_world(&mut b, &save, &(Scene,)).unwrap();
    assert!(compare(&a, &b, None).is_ok());
    let entity = saved_live(&b)[0];
    b.insert(entity, Count(12_345));
    assert_eq!(compare(&a, &b, None).unwrap_err().tag, "restored-count");
}
