//! The cascades: a saved queue of the element endings and the lightning's
//! conduction, which `cascade_run` works through in the overlap phase,
//! after 01's contact systems and before the kit's resolve, so every hit an
//! entry deals takes the step's resolve. An explosion is thermal shock,
//! fire on a frozen thing; a shatter, a hard hit on one; a steam burst,
//! cold on a burning thing; a jump, a strike's conduction from one
//! conductive thing to the next. Each waits for its run: one queued while a
//! run executes is due on the next, so a chain rolls one ring a step
//! whatever the frame rate.
//!
//! A run takes at most [`CASCADE_CAP`] due entries, by their due run, then
//! their distance from their chain's origin, then their target's save ID.
//! A kind waits once a target, a jump once a strike's target, so the queue
//! holds at most one entry a kind per live saved entity and six jumps a
//! strike; past [`CASCADE_BOUND`] an add is dropped, counted and warned of
//! once a step.

use std::collections::{HashMap, HashSet};

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::core::{
    BodyKind, Collider, DataRef, Entity, Position, RigidBody, Shape, Velocity, World,
};
use tungsten::kit::{EffectRequest, Health, end_effect};
use tungsten::physics::wake;

use super::components::{BallBurn, Frozen};
use super::systems::{ice_pulse, shatter_iron};
use super::{
    BALL_BLAST, BLAST_PUSH, CASCADE_BOUND, CASCADE_CAP, IRON_BLAST, SCRAP_BLAST, SMALL_STEAM_REACH,
    STEAM_PART, STEAM_PUSH, TARGET_BLAST,
};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::components::Reaction;
use crate::features::combat::{self, BURNING, Kind, has_status};
use crate::features::run::Run;
use crate::features::toys::components::{Ball, IronBlock, IronScrap, SmallBall};
use crate::shared::{SaveId, despawn_tree, spawn_burst};

/// The cells a run bins its bodies and targets into, in world pixels.
const CELL: f32 = 64.0;
/// The largest body's half side, an iron block's, which a bin query adds.
const LARGEST_HALF: f32 = 60.0;

/// What a cascade does when it runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CascadeKind {
    /// Thermal shock: fire and physical parts to the targets in reach, its
    /// small balls lit, its frozen bodies queued to explode, its bodies
    /// pushed, the frozen thing ended.
    Explosion,
    /// A hard hit's shards: the frozen thing ended, no explosion.
    Shatter,
    /// Cold on fire: the targets in reach put out and doused, its burning
    /// small balls queued to burst, then its fire and physical parts and its
    /// push; a small ball that bursts is gone.
    Steam,
    /// A strike's conduction: its strength in shock to its receiver, then,
    /// unless the receiver is frozen or the chain has made its last jump,
    /// the next jump queued to the nearest conductive thing not yet in the
    /// chain.
    Jump,
}

/// One waiting cascade: its kind, the thing it happens to by save ID (a
/// gone one's burst still runs at its point), where it was queued, where
/// its chain began, the save ID it credits, its reach (a jump's shock) and
/// the run from which it may run; a jump's also its strike's number, the
/// jumps its chain has made and the chain's members by save ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub kind: CascadeKind,
    pub target: Option<u64>,
    pub at: [f32; 2],
    pub origin: [f32; 2],
    pub source: Option<u64>,
    pub strength: f32,
    pub due: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<u64>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub jumps: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain: Vec<u64>,
}

fn is_zero(jumps: &u32) -> bool {
    *jumps == 0
}

/// The queue, a World resource a run's start inserts and a save holds
/// whole: the run counter, the lightning strikes' counter and the entries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cascades {
    pub step: u64,
    pub strike: u64,
    pub entries: Vec<Entry>,
    /// Adds dropped past the bound: diagnostics, never saved.
    #[serde(skip)]
    pub dropped: u64,
    /// The targets this step's steam bursts hit, which a dummy's end reads:
    /// the step's, never saved.
    #[serde(skip)]
    pub steamed: Vec<Entity>,
    /// The kinds waiting on each target, kept beside the entries.
    #[serde(skip)]
    keys: HashSet<(CascadeKind, u64)>,
    /// The run of the last warning about the bound.
    #[serde(skip)]
    warned: u64,
}

impl Cascades {
    /// The keys of the waiting entries but the jumps, rebuilt after a load.
    fn index(&mut self) {
        let keyed = self
            .entries
            .iter()
            .filter(|entry| entry.target.is_some() && entry.kind != CascadeKind::Jump)
            .count();
        if self.keys.len() != keyed {
            self.keys = self
                .entries
                .iter()
                .filter(|entry| entry.kind != CascadeKind::Jump)
                .filter_map(|entry| Some((entry.kind, entry.target?)))
                .collect();
        }
    }

    /// Whether `kind` waits on the save ID `target`.
    #[must_use]
    pub fn waits(&self, kind: CascadeKind, target: u64) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.kind == kind && entry.target == Some(target))
    }
}

/// Queues a cascade of `kind` at `at`, on `target`, from the chain begun at
/// `origin`, its reach `strength`, crediting the run's player; due on the
/// next run. False when `kind` waits on the target already, or past
/// [`CASCADE_BOUND`].
pub fn queue(
    world: &mut World,
    kind: CascadeKind,
    target: Option<Entity>,
    at: Vec2,
    origin: Vec2,
    strength: f32,
) -> bool {
    let target = target
        .and_then(|entity| world.get::<SaveId>(entity))
        .map(|id| id.0);
    let source = world
        .get_resource::<Run>()
        .and_then(|run| world.get::<SaveId>(run.player))
        .map(|id| id.0);
    let Some(cascades) = world.get_resource_mut::<Cascades>() else {
        return false;
    };
    cascades.index();
    if let Some(target) = target
        && cascades.keys.contains(&(kind, target))
    {
        return false;
    }
    if cascades.entries.len() >= CASCADE_BOUND {
        cascades.dropped += 1;
        let run = cascades.step + 1;
        if cascades.warned != run {
            cascades.warned = run;
            log::warn!("cascades: past {CASCADE_BOUND} waiting, an add is dropped");
        }
        return false;
    }
    if let Some(target) = target {
        cascades.keys.insert((kind, target));
    }
    let due = cascades.step + 1;
    cascades.entries.push(Entry {
        kind,
        target,
        at: at.to_array(),
        origin: origin.to_array(),
        source,
        strength,
        due,
        strike: None,
        jumps: 0,
        chain: Vec::new(),
    });
    true
}

/// One jump of a strike's conduction to queue: its receiver, where it
/// leaves from, its shock, the strike's number, the jumps made with it and
/// the chain's members by save ID.
#[derive(Debug, Clone, PartialEq)]
pub struct Jump {
    pub receiver: Entity,
    pub from: Vec2,
    pub strength: f32,
    pub strike: u64,
    pub jumps: u32,
    pub chain: Vec<u64>,
}

/// Queues `jump`, due `runs` runs after the last counted, crediting the run's
/// player: 1 from a run or after it, the next step's; 2 from before the
/// step's run, so a strike's first jump waits for the next step. False when
/// its strike's jump to the receiver waits already, or past the bound.
pub fn queue_jump(world: &mut World, jump: Jump, runs: u64) -> bool {
    let Some(target) = world.get::<SaveId>(jump.receiver).map(|id| id.0) else {
        return false;
    };
    let at = centre(world, jump.receiver).unwrap_or(jump.from);
    let source = world
        .get_resource::<Run>()
        .and_then(|run| world.get::<SaveId>(run.player))
        .map(|id| id.0);
    let Some(cascades) = world.get_resource_mut::<Cascades>() else {
        return false;
    };
    let waiting = cascades.entries.iter().any(|entry| {
        entry.kind == CascadeKind::Jump
            && entry.strike == Some(jump.strike)
            && entry.target == Some(target)
    });
    if waiting {
        return false;
    }
    if cascades.entries.len() >= CASCADE_BOUND {
        cascades.dropped += 1;
        return false;
    }
    let due = cascades.step + runs.max(1);
    cascades.entries.push(Entry {
        kind: CascadeKind::Jump,
        target: Some(target),
        at: at.to_array(),
        origin: jump.from.to_array(),
        source,
        strength: jump.strength,
        due,
        strike: Some(jump.strike),
        jumps: jump.jumps,
        chain: jump.chain,
    });
    true
}

/// A frozen body's explosion's reach: an ice ball's, iron's or a scrap's.
#[must_use]
pub fn blast_reach(world: &World, body: Entity) -> f32 {
    if world.has::<IronBlock>(body) {
        IRON_BLAST
    } else if world.has::<IronScrap>(body) {
        SCRAP_BLAST
    } else if world.has::<Ball>(body) {
        BALL_BLAST
    } else {
        TARGET_BLAST
    }
}

/// Queues thermal shock on the frozen `thing` at `at`, its chain begun at
/// `origin`.
pub fn thermal_shock(world: &mut World, thing: Entity, at: Vec2, origin: Vec2) -> bool {
    let reach = blast_reach(world, thing);
    queue(
        world,
        CascadeKind::Explosion,
        Some(thing),
        at,
        origin,
        reach,
    )
}

/// The order entries run in: their due run, their distance from their
/// chain's origin, their target's save ID.
fn order(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    let distance = |entry: &Entry| Vec2::from(entry.at).distance(Vec2::from(entry.origin));
    a.due
        .cmp(&b.due)
        .then(distance(a).total_cmp(&distance(b)))
        .then(
            a.target
                .unwrap_or(u64::MAX)
                .cmp(&b.target.unwrap_or(u64::MAX)),
        )
}

/// `cascade_run`: counts the run, then runs at most [`CASCADE_CAP`] of the
/// due entries in order; with none due it returns after the count.
pub fn run(world: &mut World) {
    let Some(cascades) = world.get_resource_mut::<Cascades>() else {
        return;
    };
    cascades.step += 1;
    cascades.steamed.clear();
    let step = cascades.step;
    if !cascades.entries.iter().any(|entry| entry.due <= step) {
        return;
    }
    cascades.index();
    let (mut due, waiting): (Vec<Entry>, Vec<Entry>) = std::mem::take(&mut cascades.entries)
        .into_iter()
        .partition(|entry| entry.due <= step);
    due.sort_by(order);
    let later = due.split_off(CASCADE_CAP.min(due.len()));
    cascades.entries = waiting;
    cascades.entries.extend(later);
    for entry in &due {
        if let Some(target) = entry.target.filter(|_| entry.kind != CascadeKind::Jump) {
            cascades.keys.remove(&(entry.kind, target));
        }
    }
    let mut wanted: Vec<u64> = due.iter().filter_map(|entry| entry.target).collect();
    wanted.sort_unstable();
    let bin = Bin::build(world, &wanted);
    for entry in &due {
        execute(world, &bin, entry);
    }
}

/// What a binned thing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum What {
    SmallBall,
    BowlingBall,
    Iron,
    Scrap,
    Target,
}

/// One binned thing: its entity, its body's centre and shape, its save ID
/// and what it is.
#[derive(Debug, Clone, Copy)]
struct Item {
    entity: Entity,
    centre: Vec2,
    shape: Shape,
    id: u64,
    what: What,
}

/// A run's bodies and targets, binned once by their centres' cells, and
/// the due entries' targets by save ID.
struct Bin {
    items: Vec<Item>,
    cells: HashMap<(i32, i32), Vec<usize>>,
    ids: HashMap<u64, Entity>,
}

fn cell_of(at: Vec2) -> (i32, i32) {
    ((at.x / CELL).floor() as i32, (at.y / CELL).floor() as i32)
}

/// How far `at` lies from a body of `shape` about `centre`: 0 inside it.
fn surface_distance(at: Vec2, centre: Vec2, shape: Shape) -> f32 {
    let offset = at - centre;
    match shape {
        Shape::Aabb { half_extents } => (offset.abs() - half_extents).max(Vec2::ZERO).length(),
        Shape::Circle { radius } => (offset.length() - radius).max(0.0),
    }
}

impl Bin {
    /// The step's balls, iron, scraps and live targets, and the entities of
    /// the save IDs `wanted` (sorted), the due entries' targets (P6).
    fn build(world: &World, wanted: &[u64]) -> Self {
        let mut items = Vec::new();
        for (entity, at, collider, id, small, ball, iron, scrap) in world.query::<(
            Entity,
            &Position,
            &Collider,
            &SaveId,
            Option<&SmallBall>,
            Option<&Ball>,
            Option<&IronBlock>,
            Option<&IronScrap>,
        )>() {
            let what = if small.is_some() {
                What::SmallBall
            } else if ball.is_some() {
                What::BowlingBall
            } else if iron.is_some() {
                What::Iron
            } else if scrap.is_some() {
                What::Scrap
            } else if world
                .get::<Health>(entity)
                .is_some_and(|health| !health.is_dead())
            {
                What::Target
            } else {
                continue;
            };
            items.push(Item {
                entity,
                centre: at.0 + collider.offset,
                shape: collider.shape,
                id: id.0,
                what,
            });
        }
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        let mut ids = HashMap::with_capacity(wanted.len());
        for (index, item) in items.iter().enumerate() {
            cells.entry(cell_of(item.centre)).or_default().push(index);
            if wanted.binary_search(&item.id).is_ok() {
                ids.insert(item.id, item.entity);
            }
        }
        Self { items, cells, ids }
    }

    /// The live things within `reach` of `at`, by surface distance, in
    /// save-ID order, each with its distance.
    fn near(&self, world: &World, at: Vec2, reach: f32) -> Vec<(Item, f32)> {
        let span = Vec2::splat(reach + LARGEST_HALF);
        let (min, max) = (cell_of(at - span), cell_of(at + span));
        let mut found = Vec::new();
        for y in min.1..=max.1 {
            for x in min.0..=max.0 {
                for &index in self.cells.get(&(x, y)).into_iter().flatten() {
                    let item = self.items[index];
                    if !world.is_alive(item.entity) {
                        continue;
                    }
                    let distance = surface_distance(at, item.centre, item.shape);
                    if distance <= reach {
                        found.push((item, distance));
                    }
                }
            }
        }
        found.sort_by_key(|(item, _)| item.id);
        found
    }
}

/// A body's centre now.
fn centre(world: &World, entity: Entity) -> Option<Vec2> {
    let at = world.get::<Position>(entity)?.0;
    Some(
        at + world
            .get::<Collider>(entity)
            .map_or(Vec2::ZERO, |c| c.offset),
    )
}

/// The entity of the save ID `id` the run credits: the run's player.
fn credited(world: &World, id: Option<u64>) -> Option<Entity> {
    let player = world.get_resource::<Run>()?.player;
    (world.get::<SaveId>(player).map(|own| own.0) == id).then_some(player)
}

/// Runs one entry.
fn execute(world: &mut World, bin: &Bin, entry: &Entry) {
    let target = entry
        .target
        .and_then(|id| bin.ids.get(&id).copied())
        .filter(|&entity| world.is_alive(entity));
    let at = target
        .and_then(|entity| centre(world, entity))
        .unwrap_or_else(|| Vec2::from(entry.at));
    let origin = Vec2::from(entry.origin);
    let source = credited(world, entry.source);
    match entry.kind {
        CascadeKind::Explosion => explode(world, bin, target, at, origin, source, entry.strength),
        CascadeKind::Shatter => shatter(world, target, at),
        CascadeKind::Steam => steam(world, bin, target, at, origin, source, entry.strength),
        CascadeKind::Jump => {
            crate::features::lightning::conduct(world, target, at, origin, source, entry);
        }
    }
}

/// Ends the frozen `thing`: a target's `frozen` ends; a frozen ball is
/// destroyed and a frozen scrap chips, quietly; frozen iron becomes its
/// sixteen scraps with their burst, thrown away from `from`, the blast that
/// began the chain, when it lies off the iron. A body no longer frozen is
/// left as it is.
fn end_frozen(world: &mut World, thing: Entity, from: Option<Vec2>) {
    if world.has::<Health>(thing) {
        end_effect(world, thing, &DataRef::by_id("frozen"));
    } else if world.has::<Frozen>(thing) {
        if world.has::<IronBlock>(thing) {
            shatter_iron(world, thing, from);
        } else {
            despawn_tree(world, thing);
        }
    }
}

/// Pushes the dynamic body `entity` from `at` by `speed` at the centre,
/// falling to 0 at `reach`, over its mass.
fn shove(world: &mut World, entity: Entity, at: Vec2, reach: f32, speed: f32, distance: f32) {
    let Some(body) = world.get::<RigidBody>(entity).copied() else {
        return;
    };
    if body.kind != BodyKind::Dynamic {
        return;
    }
    let Some(centre) = centre(world, entity) else {
        return;
    };
    let away = (centre - at).try_normalize().unwrap_or(-Vec2::Y);
    let falloff = 1.0 - distance / reach.max(1.0);
    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
        velocity.0 += away * speed * falloff * body.inv_mass;
    }
    wake(world, entity);
}

/// A hit from the blast at `at` of `parts` fire and physical on `target`,
/// pushed by `speed` falling to 0 at `reach`, half on the player.
fn blast_hit(
    world: &mut World,
    target: Entity,
    at: Vec2,
    parts: f32,
    push: (f32, f32, f32),
    source: Option<Entity>,
) {
    let (speed, reach, distance) = push;
    let away = centre(world, target)
        .and_then(|centre| (centre - at).try_normalize())
        .unwrap_or(-Vec2::Y);
    let falloff = 1.0 - distance / reach.max(1.0);
    let mut hit = combat::Hit::new(target, [(Kind::Fire, parts), (Kind::Physical, parts)])
        .at(at)
        .push(away * speed * falloff)
        .blast();
    if let Some(source) = source {
        hit = hit.from(source);
    }
    hit.queue(world);
}

/// Thermal shock at `at`, of `reach`: the frozen thing ends; the targets in
/// reach take fire and physical parts of an eighth of the reach each with
/// its push; small balls in reach light; frozen bodies in reach queue their
/// own explosions; the bodies are pushed.
fn explode(
    world: &mut World,
    bin: &Bin,
    thing: Option<Entity>,
    at: Vec2,
    origin: Vec2,
    source: Option<Entity>,
    reach: f32,
) {
    if let Some(thing) = thing {
        let from = (origin.distance(at) > 1e-3).then_some(origin);
        end_frozen(world, thing, from);
    }
    let parts = reach / 8.0;
    for (item, distance) in bin.near(world, at, reach) {
        if !world.is_alive(item.entity) {
            continue;
        }
        match item.what {
            What::Target => {
                blast_hit(
                    world,
                    item.entity,
                    at,
                    parts,
                    (BLAST_PUSH, reach, distance),
                    source,
                );
            }
            _ => {
                if world.has::<Frozen>(item.entity) {
                    thermal_shock(world, item.entity, item.centre, origin);
                } else if item.what == What::SmallBall {
                    super::systems::ignite(world, item.entity);
                }
                shove(world, item.entity, at, reach, BLAST_PUSH, distance);
            }
        }
    }
    spawn_burst(world, "ex10_frost_blast", at);
    spawn_burst(world, "ex10_fireball_blast", at);
    audio::play(world, Sound::FrostBlasted, at);
}

/// A hard hit's shards at `at`: the frozen thing ends; frozen iron's
/// scraps bring their own burst.
fn shatter(world: &mut World, thing: Option<Entity>, at: Vec2) {
    let iron =
        thing.is_some_and(|thing| world.has::<IronBlock>(thing) && world.has::<Frozen>(thing));
    if let Some(thing) = thing {
        end_frozen(world, thing, None);
    }
    if !iron {
        spawn_burst(world, "ex10_ice_shatter", at);
        ice_pulse(world, at, 32.0, true);
    }
}

/// A steam burst at `at`, of `reach`: a small ball that bursts is gone; the
/// targets in reach are put out and doused at once, so its own fire cannot
/// light them again; the burning small balls in reach queue their own
/// bursts; then the targets take its fire and physical parts with its
/// push, and the bodies are pushed.
fn steam(
    world: &mut World,
    bin: &Bin,
    thing: Option<Entity>,
    at: Vec2,
    origin: Vec2,
    source: Option<Entity>,
    reach: f32,
) {
    if let Some(thing) = thing.filter(|&thing| world.has::<SmallBall>(thing)) {
        despawn_tree(world, thing);
    }
    let near = bin.near(world, at, reach);
    for &(item, _) in &near {
        if item.what != What::Target || !world.is_alive(item.entity) {
            continue;
        }
        if has_status(world, item.entity, BURNING) {
            end_effect(world, item.entity, &DataRef::by_id("burning"));
        }
        EffectRequest::new(item.entity, DataRef::by_id("doused")).apply(world);
        if let Some(cascades) = world.get_resource_mut::<Cascades>() {
            cascades.steamed.push(item.entity);
        }
    }
    for &(item, distance) in &near {
        if !world.is_alive(item.entity) {
            continue;
        }
        match item.what {
            What::Target => {
                blast_hit(
                    world,
                    item.entity,
                    at,
                    STEAM_PART,
                    (STEAM_PUSH, reach, distance),
                    source,
                );
            }
            What::SmallBall => {
                let burning = world
                    .get::<BallBurn>(item.entity)
                    .is_some_and(|burn| burn.remaining > 0.0);
                if burning {
                    queue(
                        world,
                        CascadeKind::Steam,
                        Some(item.entity),
                        item.centre,
                        origin,
                        SMALL_STEAM_REACH,
                    );
                }
                shove(world, item.entity, at, reach, STEAM_PUSH, distance);
            }
            _ => shove(world, item.entity, at, reach, STEAM_PUSH, distance),
        }
    }
    spawn_burst(world, "ex10_steam_burst", at);
    audio::play(world, Sound::Steamed, at);
    combat::react(world, Reaction::Steam);
}
