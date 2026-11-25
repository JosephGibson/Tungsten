//! The cascades: a saved resource of the run counter, the lightning
//! strikes' counter and the heated bodies' points, which `cascade_run`
//! counts and clears in the overlap phase, after 01's contact systems and
//! before the kit's resolve. The lightning's conduction is the kit's
//! (`kit::conduct_from`, 7.13d), its chains saved by the kit.
//!
//! The element endings and the kegs are chemistry's (7.13a): thermal shock
//! (`heat` on a frozen body, `damage.fire` on a frozen target), shatter
//! (`crack` on a frozen thing) and steam (`cold` on a burning body,
//! `damage.cold` on a burning target) are `interactions` rules whose
//! handlers here run 01's blast, douse and end; `heat` on what is not
//! frozen, and `kindle`, run `light`; a keg's or a bomb's fuse running out
//! (its `lit` status's end) and a `jolt` run `detonate`. A
//! handler's stimuli go to chemistry's next run, so a chain still rolls a
//! ring a step, through the kit's queue, its cap 01's
//! [`CASCADE_CAP`](super::CASCADE_CAP) and its bound
//! [`CASCADE_BOUND`](super::CASCADE_BOUND).
//!
//! A handler credits its stimulus's live credit: the player, or a live
//! target, so a goblin's bomb that shocks a frozen body still credits the
//! goblin; a gone lighter credits none.

use std::collections::HashMap;
use std::collections::hash_map::Entry as Slot;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten::core::save::RefCodec;
use tungsten::core::{
    BodyKind, Collider, DataRef, Entity, Position, RigidBody, Shape, Velocity, World,
};
use tungsten::kit::{Blast, Chemistry, EffectRequest, Health, Reaction, end_effect, stimulate};
use tungsten::physics::wake;

use super::systems::{ice_pulse, ignite, shatter_iron};
use super::{
    BALL_BLAST, BLAST_PUSH, COLD, CRACK, HEAT, IRON_BLAST, SCRAP_BLAST, SMALL_STEAM_REACH,
    STEAM_PART, STEAM_PUSH, STEAM_REACH, TARGET_BLAST,
};
use super::{is_burning, is_frozen};
use crate::features::audio::{self, components::Sound};
use crate::features::combat::{self, BURNING, DOUSED, Kind, has_status};
use crate::features::enemies::armour;
use crate::features::props::components::{Bomb, Crate, Keg, Rock, Stone};
use crate::features::props::{self, BOMB_BLAST, KEG_BLAST, PROP_BLAST};
use crate::features::run::Run;
use crate::features::toys::components::{Ball, IronBlock, IronScrap, SmallBall};
use crate::features::toys::systems::flare;
use crate::shared::{
    Saved, body_centre, despawn_tree, spawn_burst, spawn_key_burst, surface_distance,
};

/// The cells a run bins its bodies and targets into, in world pixels.
const CELL: f32 = 64.0;
/// The largest body's half side, an iron block's, which a bin query adds.
const LARGEST_HALF: f32 = 60.0;

/// The cascades, a World resource a run's start inserts and a save holds:
/// the run counter and the lightning strikes' counter, which picks a
/// bolt's pieces.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cascades {
    pub step: u64,
    pub strike: u64,
    /// The targets this step's steam bursts hit, which a dummy's end reads:
    /// the step's, never saved.
    #[serde(skip)]
    pub steamed: Vec<Entity>,
    /// The kegs and bombs this step's `detonate` handlers have blown, in
    /// turn: the step's, never saved.
    #[serde(skip)]
    pub detonated: Vec<Entity>,
    /// The things this step's thermal shocks and steam bursts happened to,
    /// which a dummy's end reads: the step's, never saved.
    #[serde(skip)]
    pub reacted: Vec<Entity>,
    /// Where each body's `heat` came from, with the order it came in,
    /// which its thermal shock throws iron's scraps away from: kept until
    /// its handler reads it, or the kit's queue empties. Saved in that
    /// order, each body by reference ([`cascades_refs`]).
    #[serde(skip)]
    pub heat_from: HashMap<Entity, (u64, Vec2)>,
    /// The order the next body's `heat` takes.
    #[serde(skip)]
    pub heat_next: u64,
    /// A load's heat points, in order, waiting for their bodies.
    #[serde(skip)]
    pub heat_loaded: Vec<Vec2>,
    /// The thermal shocks the run's chains have set off
    /// ([`thermal_shock_handler`]), which the `pit_shock` probe reads: the
    /// run's, never saved.
    #[serde(skip)]
    pub shocks: u64,
    /// The steam bursts the run's chains have set off ([`steam_at`]): the
    /// run's, never saved.
    #[serde(skip)]
    pub steams: u64,
}

/// [`Cascades`]' saved form: its counters and each heated body's point, in
/// the order its heat came.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CascadesForm {
    pub step: u64,
    pub strike: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heat: Vec<[f32; 2]>,
}

/// The heated bodies in the order their heat came, with their points.
fn heat_order(cascades: &Cascades) -> Vec<(Entity, Vec2)> {
    let mut heat: Vec<(u64, Entity, Vec2)> = cascades
        .heat_from
        .iter()
        .map(|(&body, &(order, from))| (order, body, from))
        .collect();
    heat.sort_unstable_by_key(|&(order, ..)| order);
    heat.into_iter()
        .map(|(_, body, from)| (body, from))
        .collect()
}

impl From<&Cascades> for CascadesForm {
    fn from(cascades: &Cascades) -> Self {
        Self {
            step: cascades.step,
            strike: cascades.strike,
            heat: heat_order(cascades)
                .into_iter()
                .map(|(_, from)| from.to_array())
                .collect(),
        }
    }
}

impl From<CascadesForm> for Cascades {
    fn from(form: CascadesForm) -> Self {
        Self {
            step: form.step,
            strike: form.strike,
            heat_loaded: form.heat.into_iter().map(Vec2::from).collect(),
            ..Self::default()
        }
    }
}

/// [`Cascades`] as a save holds it: its heat on bodies with a save ID
/// alone. A body the chain despawned has no queue entry left that reads
/// its point (the kit keeps such an entry dead, `D-274`), and its reference
/// would drop, so a capture leaves it out; `None` when every heated body
/// has an ID.
#[must_use]
pub fn cascades_saved(world: &World, _: Option<Entity>, cascades: &Cascades) -> Option<Cascades> {
    let saved = |body: &Entity| world.has::<Saved>(*body);
    if cascades.heat_from.keys().all(saved) {
        return None;
    }
    Some(Cascades {
        step: cascades.step,
        strike: cascades.strike,
        heat_from: cascades
            .heat_from
            .iter()
            .filter(|(body, _)| saved(body))
            .map(|(&body, &heat)| (body, heat))
            .collect(),
        heat_next: cascades.heat_next,
        ..Cascades::default()
    })
}

/// [`Cascades`]' references: each heated body under its place in the
/// order; a body that did not load drops its point.
#[must_use]
pub fn cascades_refs() -> RefCodec<Cascades> {
    RefCodec {
        save: |cascades, out| {
            for (i, (body, _)) in heat_order(cascades).into_iter().enumerate() {
                out.put(&i.to_string(), body);
            }
        },
        load: |cascades, refs| {
            for (i, from) in std::mem::take(&mut cascades.heat_loaded)
                .into_iter()
                .enumerate()
            {
                if let Some(body) = refs.first(&i.to_string()) {
                    cascades.note_heat(body, from);
                }
            }
        },
        keys: |cascades| {
            (0..cascades.heat_from.len())
                .map(|i| i.to_string())
                .collect()
        },
    }
}

impl Cascades {
    /// Keeps where `body`'s heat came from, unless its earlier heat waits.
    fn note_heat(&mut self, body: Entity, from: Vec2) {
        if let Slot::Vacant(slot) = self.heat_from.entry(body) {
            slot.insert((self.heat_next, from));
            self.heat_next += 1;
        }
    }
}

/// The run's player's save ID, which the spells, the gun and a fire nobody
/// lit credit.
#[must_use]
pub fn player_credit(world: &World) -> Option<u64> {
    world
        .get_resource::<Run>()
        .and_then(|run| crate::shared::save_id(world, run.player))
        .map(|id| id.0)
}

/// Sends `heat` to `body` from `from`, credited to `credit`: thermal shock
/// on a frozen body, a light on the rest ([`light_handler`]), at chemistry's
/// next run; `from` is kept for a frozen iron's scraps.
pub fn heat(world: &mut World, body: Entity, from: Vec2, credit: Option<Entity>) {
    if let Some(cascades) = world.get_resource_mut::<Cascades>() {
        cascades.note_heat(body, from);
    }
    stimulate(world, body, HEAT, 1.0, credit);
}

/// Sends `crack` to `thing`, credited to the run's player: a hard hit's
/// shards on a frozen thing ([`shatter_handler`]), at chemistry's next run.
pub fn shatter_at(world: &mut World, thing: Entity) {
    let credit = world.get_resource::<Run>().map(|run| run.player);
    stimulate(world, thing, CRACK, 1.0, credit);
}

/// The run's player, or the live entity `credit` names: what a handler's
/// hits and endings credit, none for a gone one.
fn live_credit(world: &World, credit: Option<Entity>) -> Option<Entity> {
    let credit = credit?;
    let player = world.get_resource::<Run>().map(|run| run.player);
    let live = world.is_alive(credit)
        && world
            .get::<Health>(credit)
            .is_some_and(|health| !health.is_dead());
    (player == Some(credit) || live).then_some(credit)
}

/// The handlers' bin of the step's bodies and targets, built at its first
/// handler a step: a World resource, never saved.
#[derive(Default)]
struct HandlerBin(Option<(u64, Bin)>);

/// Runs `then` with the step's bin, built at the step's first handler.
fn with_bin(world: &mut World, then: impl FnOnce(&mut World, &Bin)) {
    let step = world.get_resource::<Cascades>().map_or(0, |c| c.step);
    if !world.has_resource::<HandlerBin>() {
        world.insert_resource(HandlerBin::default());
    }
    let cached = world
        .resource_mut::<HandlerBin>()
        .0
        .take()
        .filter(|(built, _)| *built == step);
    let bin = cached.map_or_else(|| Bin::build(world), |(_, bin)| bin);
    then(world, &bin);
    world.resource_mut::<HandlerBin>().0 = Some((step, bin));
}

/// Notes `thing` among the step's reactions, which a dummy's end reads.
fn react_on(world: &mut World, thing: Entity) {
    if let Some(cascades) = world.get_resource_mut::<Cascades>() {
        cascades.reacted.push(thing);
    }
}

/// Chemistry's `thermal_shock` handler: fire on a frozen thing explodes
/// it, ended (a target's `frozen`; iron into its scraps, thrown away from
/// where its heat came from; a ball or a scrap gone), then its [`blast`]
/// of its kind's reach, crediting the stimulus's live credit, its bursts
/// and its sound.
pub fn thermal_shock_handler(world: &mut World, reaction: &Reaction) {
    let Some(thing) = reaction.receiver() else {
        return;
    };
    let Some(at) = body_centre(world, thing) else {
        return;
    };
    let reach = blast_reach(world, thing);
    let credit = live_credit(world, reaction.credit());
    let from = world
        .get_resource_mut::<Cascades>()
        .and_then(|cascades| cascades.heat_from.remove(&thing))
        .map(|(_, from)| from)
        .filter(|from| from.distance(at) > 1e-3);
    react_on(world, thing);
    if let Some(cascades) = world.get_resource_mut::<Cascades>() {
        cascades.shocks += 1;
    }
    end_frozen(world, thing, from);
    with_bin(world, |world, bin| {
        blast(world, bin, at, credit, reach, Some(thing));
    });
    spawn_burst(world, "ex10_frost_blast", at);
    spawn_burst(world, "ex10_fireball_blast", at);
    audio::play(world, Sound::FrostBlasted, at);
}

/// Chemistry's `shatter` handler: a hard hit's shards on a frozen thing.
pub fn shatter_handler(world: &mut World, reaction: &Reaction) {
    let Some(thing) = reaction.receiver() else {
        return;
    };
    let at = body_centre(world, thing).unwrap_or(Vec2::ZERO);
    shatter(world, Some(thing), at);
}

/// Chemistry's `steam` handler: cold on a burning thing bursts into steam
/// of its reach (a small ball's [`SMALL_STEAM_REACH`], else
/// [`STEAM_REACH`]), crediting the stimulus's live credit ([`steam`]).
pub fn steam_handler(world: &mut World, reaction: &Reaction) {
    let Some(thing) = reaction.receiver() else {
        return;
    };
    let reach = if world.has::<SmallBall>(thing) {
        SMALL_STEAM_REACH
    } else {
        STEAM_REACH
    };
    let credit = live_credit(world, reaction.credit());
    steam_at(world, thing, reach, credit);
}

/// A steam burst from the burning `thing` at once, of `reach`, crediting
/// `credit` ([`steam`]): a rule's, or the cold spray's on a burning prop,
/// which it puts out at once.
pub fn steam_at(world: &mut World, thing: Entity, reach: f32, credit: Option<Entity>) {
    let Some(at) = body_centre(world, thing) else {
        return;
    };
    react_on(world, thing);
    if let Some(cascades) = world.get_resource_mut::<Cascades>() {
        cascades.steams += 1;
    }
    with_bin(world, |world, bin| {
        steam(world, bin, Some(thing), at, credit, reach)
    });
}

/// Chemistry's `light` handler, `heat` on what is not frozen and `kindle`:
/// a small ball that never burned lights; a crate or a plank catches and a
/// keg's fuse starts ([`props::light_by`]), its lighter the stimulus's live
/// credit; the rest take nothing.
pub fn light_handler(world: &mut World, reaction: &Reaction) {
    let Some(body) = reaction.receiver() else {
        return;
    };
    if world.has::<SmallBall>(body) {
        ignite(world, body);
    } else if props::is_prop(world, body) {
        let lighter = live_credit(world, reaction.credit());
        props::light_by(world, body, lighter);
    }
}

/// Chemistry's `fuse_out` handler, a keg's or a bomb's `lit` status ending:
/// a fuse run out sets it off ([`detonate_handler`]) in the step it ran
/// out, as its clock did; one put out leaves it whole, the douse that put
/// it out already on it (`props::put_out`).
pub fn fuse_out_handler(world: &mut World, reaction: &Reaction) {
    let Some(keg) = reaction.receiver() else {
        return;
    };
    if !has_status(world, keg, DOUSED) {
        detonate_handler(world, reaction);
    }
}

/// Chemistry's `detonate` handler: a keg's or a bomb's fuse ran out
/// ([`fuse_out_handler`]) or a `jolt` set it off: its [`detonate`] of its
/// kind's reach, [`KEG_BLAST`] or [`BOMB_BLAST`], crediting the stimulus's
/// live credit, whoever lit it.
pub fn detonate_handler(world: &mut World, reaction: &Reaction) {
    let Some(keg) = reaction.receiver() else {
        return;
    };
    let reach = if world.has::<Keg>(keg) {
        KEG_BLAST
    } else if world.has::<Bomb>(keg) {
        BOMB_BLAST
    } else {
        return;
    };
    let Some(at) = body_centre(world, keg) else {
        return;
    };
    let credit = live_credit(world, reaction.credit());
    with_bin(world, |world, bin| {
        detonate(world, bin, keg, at, credit, reach);
    });
}

/// A frozen body's explosion's reach: an ice ball's, iron's, a scrap's or a
/// prop's.
#[must_use]
pub fn blast_reach(world: &World, body: Entity) -> f32 {
    if world.has::<IronBlock>(body) {
        IRON_BLAST
    } else if world.has::<IronScrap>(body) {
        SCRAP_BLAST
    } else if world.has::<Ball>(body) {
        BALL_BLAST
    } else if props::is_prop(world, body) {
        PROP_BLAST
    } else {
        TARGET_BLAST
    }
}

/// Sends thermal shock's `heat` to the frozen `thing` from `from`,
/// crediting the run's player.
pub fn thermal_shock(world: &mut World, thing: Entity, from: Vec2) {
    let credit = world.get_resource::<Run>().map(|run| run.player);
    heat(world, thing, from, credit);
}

/// `cascade_run`: counts the run and clears the step's lists; with
/// chemistry's queue empty, the heated bodies' points go.
pub fn run(world: &mut World) {
    let Some(cascades) = world.get_resource_mut::<Cascades>() else {
        return;
    };
    cascades.step += 1;
    cascades.steamed.clear();
    cascades.detonated.clear();
    cascades.reacted.clear();
    let idle = world
        .get_resource::<Chemistry>()
        .is_none_or(|chemistry| chemistry.pending() == 0);
    let Some(cascades) = world.get_resource_mut::<Cascades>() else {
        return;
    };
    if idle && !cascades.heat_from.is_empty() {
        cascades.heat_from.clear();
        cascades.heat_next = 0;
    }
}

/// What a binned thing is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum What {
    SmallBall,
    BowlingBall,
    Iron,
    Scrap,
    Crate,
    Keg,
    Bomb,
    Rock,
    Stone,
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

/// A step's bodies and targets, binned once by their centres' cells.
struct Bin {
    items: Vec<Item>,
    cells: BinCells,
}

/// A directory may cover this many cells an item, and 64 more; past it the
/// bin maps its cells as it always did.
const DIRECTORY_CELLS: u64 = 4;

/// The bin's items by their cells. While the occupied cells' box is small,
/// a directory over it, row by row and then column by column, each cell's
/// items in the order the query found them, which is the order the map
/// gives a query; else that map.
enum BinCells {
    Directory {
        /// The box's first column and row, and its columns and rows.
        column: i32,
        row: i32,
        columns: i32,
        rows: i32,
        /// By row, then column, where each cell's items end among the items
        /// that follow: so a cell's items start where the cell before it
        /// ends. Then the items, cell by cell. One list, so the cells take no
        /// more room than the map.
        cells: Vec<u32>,
    },
    Map(HashMap<(i32, i32), Vec<usize>>),
}

impl BinCells {
    /// The cells of `items`: one pass to count each cell's items, one to
    /// lay them out in item order.
    fn of(items: &[Item]) -> Self {
        // The box from a first pass, each cell taken again where it is
        // needed, so the map's path allocates what the map alone did.
        let mut keys = items.iter().map(|item| cell_of(item.centre));
        let Some(first) = keys.next() else {
            return Self::Map(HashMap::new());
        };
        let (mut low, mut high) = (first, first);
        for (x, y) in keys {
            low = (low.0.min(x), low.1.min(y));
            high = (high.0.max(x), high.1.max(y));
        }
        let columns = (i64::from(high.0) - i64::from(low.0) + 1).unsigned_abs();
        let rows = (i64::from(high.1) - i64::from(low.1) + 1).unsigned_abs();
        let slots = columns.saturating_mul(rows);
        if slots > DIRECTORY_CELLS * items.len() as u64 + 64 {
            let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
            for (index, item) in items.iter().enumerate() {
                cells.entry(cell_of(item.centre)).or_default().push(index);
            }
            return Self::Map(cells);
        }
        // Within the box every offset fits: it holds few cells.
        let (columns, rows) = (columns as i32, rows as i32);
        let slot = |item: &Item| {
            let (x, y) = cell_of(item.centre);
            ((y - low.1) * columns + (x - low.0)) as usize
        };
        let slots = slots as usize;
        let mut cells = vec![0_u32; slots + items.len()];
        for item in items {
            cells[slot(item)] += 1;
        }
        // Each cell's count becomes its start, then its end as its items go in.
        let mut start = 0;
        for at in &mut cells[..slots] {
            let count = *at;
            *at = start;
            start += count;
        }
        for (index, item) in items.iter().enumerate() {
            let at = slot(item);
            let place = cells[at];
            cells[slots + place as usize] = index as u32;
            cells[at] = place + 1;
        }
        Self::Directory {
            column: low.0,
            row: low.1,
            columns,
            rows,
            cells,
        }
    }

    /// Visits the items of every cell from `min` to `max`, rows outer and
    /// columns inner, each cell's in item order.
    fn visit(&self, min: (i32, i32), max: (i32, i32), mut each: impl FnMut(usize)) {
        match self {
            Self::Map(cells) => {
                for y in min.1..=max.1 {
                    for x in min.0..=max.0 {
                        for &index in cells.get(&(x, y)).into_iter().flatten() {
                            each(index);
                        }
                    }
                }
            }
            Self::Directory {
                column,
                row,
                columns,
                rows,
                cells,
            } => {
                // A cell outside the box holds nothing.
                let (x0, x1) = (min.0.max(*column), max.0.min(column + columns - 1));
                let (y0, y1) = (min.1.max(*row), max.1.min(row + rows - 1));
                if x0 > x1 {
                    return;
                }
                let slots = (columns * rows) as usize;
                for y in y0..=y1 {
                    let base = ((y - row) * columns) as usize;
                    let (first, last) =
                        (base + (x0 - column) as usize, base + (x1 - column) as usize);
                    let from = if first == 0 { 0 } else { cells[first - 1] };
                    let to = cells[last];
                    for &index in &cells[slots + from as usize..slots + to as usize] {
                        each(index as usize);
                    }
                }
            }
        }
    }
}

fn cell_of(at: Vec2) -> (i32, i32) {
    ((at.x / CELL).floor() as i32, (at.y / CELL).floor() as i32)
}

impl Bin {
    /// The step's balls, iron, scraps, crates, kegs and live targets (P6).
    /// The balls and iron come from the query's columns; the few other rows
    /// are looked up.
    fn build(world: &World) -> Self {
        let mut items = Vec::new();
        for (entity, at, collider, id, small, ball, iron, scrap) in world.query::<(
            Entity,
            &Position,
            &Collider,
            &Saved,
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
            } else if world.has::<Crate>(entity) {
                What::Crate
            } else if world.has::<Keg>(entity) {
                What::Keg
            } else if world.has::<Bomb>(entity) {
                What::Bomb
            } else if world.has::<Rock>(entity) {
                What::Rock
            } else if world.has::<Stone>(entity) {
                What::Stone
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
                id: id.id().0,
                what,
            });
        }
        Self::of(items)
    }

    /// The bin of `items`, binned by their cells.
    fn of(items: Vec<Item>) -> Self {
        let cells = BinCells::of(&items);
        Self { items, cells }
    }

    /// The live things within `reach` of `at`, by surface distance, in
    /// save-ID order, each with its distance.
    fn near(&self, world: &World, at: Vec2, reach: f32) -> Vec<(Item, f32)> {
        let span = Vec2::splat(reach + LARGEST_HALF);
        let (min, max) = (cell_of(at - span), cell_of(at + span));
        let mut found = Vec::new();
        self.cells.visit(min, max, |index| {
            let item = self.items[index];
            if !world.is_alive(item.entity) {
                return;
            }
            let distance = surface_distance(at, item.centre, item.shape);
            if distance <= reach {
                found.push((item, distance));
            }
        });
        found.sort_by_key(|(item, _)| item.id);
        found
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
    } else if is_frozen(world, thing) {
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
    let Some(centre) = body_centre(world, entity) else {
        return;
    };
    let away = (centre - at).try_normalize().unwrap_or(-Vec2::Y);
    let falloff = 1.0 - distance / reach.max(1.0);
    if let Some(velocity) = world.get_mut::<Velocity>(entity) {
        velocity.0 += away * speed * falloff * body.inv_mass;
    }
    wake(world, entity);
}

/// The kit's blast of `reach` with `part` fire and `part` physical, flat to
/// its edge, pushing [`push`](Blast::with_push) away from its centre: a
/// blast's or a steam burst's in the [`BLAST`](combat::BLAST) context,
/// which the player takes half of, at its centre through 01's armour,
/// landed at once, with no stimulus and no impulse (01 shoves its bodies),
/// hurting its receiver when `receiver`.
fn blast_of(reach: f32, part: f32, push: f32, receiver: bool) -> Blast {
    Blast::new(reach)
        .with_part(Kind::Fire.tag(), part)
        .with_part(Kind::Physical.tag(), part)
        .with_far(1.0)
        .with_push(push)
        .with_context([combat::BLAST])
        .with_receiver(receiver)
        .at_centre()
        .landing_now()
        .with_stimulus(false)
        .with_prepare(armour::PREPARE)
}

/// A blast at `at` of `reach`, a thermal shock's from `thing` or a
/// detonation's: the kit's [`blast`](tungsten::kit::blast) hurts the
/// targets in reach, `thing` among them, with fire and physical parts of an
/// eighth of the reach each, [`BLAST_PUSH`] away, from `credit`, at once;
/// then its frozen bodies, small balls and crates are sent `heat` (each its
/// rule: thermal shock, or a light), its kegs and bombs `jolt`, crediting
/// `credit`, and its bodies are pushed.
fn blast(
    world: &mut World,
    bin: &Bin,
    at: Vec2,
    credit: Option<Entity>,
    reach: f32,
    thing: Option<Entity>,
) {
    let def = blast_of(reach, reach / 8.0, BLAST_PUSH, thing.is_some());
    tungsten::kit::blast(world, at, &def, credit, thing);
    for (item, distance) in bin.near(world, at, reach) {
        if !world.is_alive(item.entity) || item.what == What::Target {
            continue;
        }
        if is_frozen(world, item.entity) {
            heat(world, item.entity, at, credit);
        } else {
            match item.what {
                What::SmallBall | What::Crate => heat(world, item.entity, at, credit),
                What::Keg | What::Bomb => {
                    props::jolt(world, item.entity, credit);
                }
                _ => {}
            }
        }
        shove(world, item.entity, at, reach, BLAST_PUSH, distance);
    }
}

/// The bursts of a keg's detonation: the missile's four and its staves.
const DETONATION_BURSTS: [&str; 5] = [
    "ex10_fireball_blast",
    "ex10_blast_embers",
    "ex10_blast_dust",
    "ex10_blast_smoke",
    "ex10_keg_staves",
];

/// A keg's or a bomb's detonation at `at`, of `reach`, crediting
/// `credit`: once a keg a step, and never on one gone. A frozen keg
/// shatters instead; else it goes and its [`blast`] runs, its bursts key
/// ones for the step's first detonation, the missile's flare for the
/// step's first two, and its boom, whose rule shakes the camera.
fn detonate(
    world: &mut World,
    bin: &Bin,
    keg: Entity,
    at: Vec2,
    credit: Option<Entity>,
    reach: f32,
) {
    if !world.is_alive(keg) || !(world.has::<Keg>(keg) || world.has::<Bomb>(keg)) {
        return;
    }
    let Some(cascades) = world.get_resource_mut::<Cascades>() else {
        return;
    };
    if cascades.detonated.contains(&keg) {
        return;
    }
    cascades.detonated.push(keg);
    let count = cascades.detonated.len();
    if is_frozen(world, keg) {
        shatter(world, Some(keg), at);
        return;
    }
    despawn_tree(world, keg);
    blast(world, bin, at, credit, reach, None);
    for burst in DETONATION_BURSTS {
        if count == 1 {
            spawn_key_burst(world, burst, at);
        } else {
            spawn_burst(world, burst, at);
        }
    }
    if count <= 2 {
        flare(world, at, true);
    }
    audio::play(world, Sound::KegBlasted, at);
}

/// A hard hit's shards at `at`: the frozen thing ends; frozen iron's
/// scraps bring their own burst.
fn shatter(world: &mut World, thing: Option<Entity>, at: Vec2) {
    let iron = thing.is_some_and(|thing| world.has::<IronBlock>(thing) && is_frozen(world, thing));
    if let Some(thing) = thing {
        end_frozen(world, thing, None);
    }
    if !iron {
        spawn_key_burst(world, "ex10_ice_shatter", at);
        ice_pulse(world, at, 32.0, true);
    }
}

/// A steam burst at `at`, of `reach`: a small ball that bursts is gone; the
/// targets in reach are put out and doused at once, so its own fire cannot
/// light them again; the burning small balls in reach are sent `cold`,
/// their own bursts at chemistry's next run, crediting it; the crates and
/// kegs in reach are put out and doused; then the kit's blast gives the
/// targets, `thing` among them, its fire and physical parts with its push,
/// at once, and the bodies are pushed.
fn steam(
    world: &mut World,
    bin: &Bin,
    thing: Option<Entity>,
    at: Vec2,
    credit: Option<Entity>,
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
    let def = blast_of(reach, STEAM_PART, STEAM_PUSH, true);
    tungsten::kit::blast(world, at, &def, credit, thing);
    for &(item, distance) in &near {
        if !world.is_alive(item.entity) {
            continue;
        }
        match item.what {
            What::Target => {}
            What::SmallBall => {
                if is_burning(world, item.entity) {
                    stimulate(world, item.entity, COLD, 1.0, credit);
                }
                shove(world, item.entity, at, reach, STEAM_PUSH, distance);
            }
            What::Crate | What::Keg | What::Bomb => {
                props::put_out(world, item.entity);
                if world.is_alive(item.entity) {
                    shove(world, item.entity, at, reach, STEAM_PUSH, distance);
                }
            }
            _ => shove(world, item.entity, at, reach, STEAM_PUSH, distance),
        }
    }
    spawn_burst(world, "ex10_steam_burst", at);
    audio::play(world, Sound::Steamed, at);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A seeded draw.
    fn draw(stream: &mut u64) -> u64 {
        *stream = stream.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *stream;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A draw in `[0, 1)`.
    fn unit(stream: &mut u64) -> f32 {
        (draw(stream) >> 40) as f32 / (1_u64 << 24) as f32
    }

    /// `near` as the bin answered it before its directory: through a map of
    /// its cells, rows outer and columns inner.
    fn near_by_map(bin: &Bin, world: &World, at: Vec2, reach: f32) -> Vec<(Item, f32)> {
        let mut cells: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (index, item) in bin.items.iter().enumerate() {
            cells.entry(cell_of(item.centre)).or_default().push(index);
        }
        let span = Vec2::splat(reach + LARGEST_HALF);
        let (min, max) = (cell_of(at - span), cell_of(at + span));
        let mut found = Vec::new();
        for y in min.1..=max.1 {
            for x in min.0..=max.0 {
                for &index in cells.get(&(x, y)).into_iter().flatten() {
                    let item = bin.items[index];
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

    /// What a find is, to compare: its entity, save ID and distance's bits.
    fn keyed(found: &[(Item, f32)]) -> Vec<(Entity, u64, u32)> {
        found
            .iter()
            .map(|(item, distance)| (item.entity, item.id, distance.to_bits()))
            .collect()
    }

    #[test]
    fn the_bins_directory_finds_what_its_map_did() {
        // Seeds without and with far outliers: a directory, then the map.
        for (seed, outliers) in [(7_u64, false), (99, false), (4242, true)] {
            let mut world = World::new();
            let mut stream = seed;
            let mut items = Vec::new();
            for at in 0..900_u32 {
                let entity = world.spawn();
                let mut centre = Vec2::new(
                    3000.0 + 1600.0 * unit(&mut stream),
                    2300.0 + 2100.0 * unit(&mut stream),
                );
                if outliers && at % 150 == 0 {
                    centre = Vec2::new(-90_000.0, 70_000.0 * unit(&mut stream));
                }
                let shape = if at % 3 == 0 {
                    Shape::Aabb {
                        half_extents: Vec2::new(5.0 + 55.0 * unit(&mut stream), 8.0),
                    }
                } else {
                    Shape::Circle { radius: 7.5 }
                };
                // Save IDs with ties, which the stable sort leaves in visit order.
                let id = draw(&mut stream) % 400;
                items.push(Item {
                    entity,
                    centre,
                    shape,
                    id,
                    what: What::SmallBall,
                });
            }
            for item in items.iter().step_by(41) {
                world.despawn(item.entity);
            }
            let bin = Bin::of(items);
            assert_eq!(
                matches!(bin.cells, BinCells::Map(_)),
                outliers,
                "seed {seed}: the cells' form"
            );
            let mut hits = 0;
            for _ in 0..300 {
                let at = Vec2::new(
                    2900.0 + 1800.0 * unit(&mut stream),
                    2200.0 + 2300.0 * unit(&mut stream),
                );
                let reach = 10.0 + 200.0 * unit(&mut stream);
                let found = bin.near(&world, at, reach);
                hits += found.len();
                assert_eq!(
                    keyed(&found),
                    keyed(&near_by_map(&bin, &world, at, reach)),
                    "seed {seed} at {at} reach {reach}"
                );
            }
            assert!(
                hits > 1000,
                "seed {seed}: the queries found things ({hits})"
            );
        }
    }
}
