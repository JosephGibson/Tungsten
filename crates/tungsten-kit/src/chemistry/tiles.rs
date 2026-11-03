//! Chemistry on tiles (redo 7.10b): each placement's cells whose tile has a
//! `material` receive stimuli, their tags the material read as a tag and
//! their statuses' grants and reached bands; the statuses are held per cell
//! in a saved resource; and two outputs write a cell: `damage` through
//! `damage_tile`, and `set_tile` with a revert kept per map cell.

use std::collections::BTreeMap;
use std::rc::Rc;

use glam::Vec2;
use serde::{Deserialize, Serialize};
use tungsten_core::physics::TileHit;
use tungsten_core::save::SaveOptions;
use tungsten_core::{
    DataRef, EMPTY_TILE, Entity, TileIndex, TilemapData, TilemapInstance, TilemapRegistry, World,
};

use crate::chemistry::outputs::TileCell;
use crate::chemistry::queue::{Receiver, StatusList};
use crate::chemistry::queue_status_list;
use crate::chemistry::stimuli::{Emission, Named};
use crate::damage::effects::{Effect, Refresh, built, definition, factor_of};
use crate::foundation::tags::{Tags, valid_tag};
use crate::foundation::trace::{TraceKind, trace};
use crate::lifecycle::cooldown::Cooldown;
use crate::{TIME_EPSILON, count_down};

/// The name a save holds the tile statuses under.
pub(crate) const TILE_STATUSES: &str = "tile_statuses";

/// Why a cell's output did not run, for the trace.
pub(crate) const NO_MAP: &str = "the cell's map or cell is gone";
pub(crate) const NO_SPRITE: &str = "the map's tileset lacks the sprite";
pub(crate) const NOT_A_TILE: &str = "the receiver is no tile cell";
const UNBREAKABLE: &str = "the cell's tile has no health";
const REVERT_DROPPED: &str = "a revert's cell was written since";

/// A placement's cell: its map's ID, its placement's top-left by the bits
/// of its coordinates (as the physics keys a placement), its layer's
/// index, row and column. Ordered by map ID, origin, layer, row and column.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Cell {
    pub(crate) map: Rc<str>,
    pub(crate) origin: [u32; 2],
    pub(crate) layer: u32,
    pub(crate) row: u32,
    pub(crate) col: u32,
}

/// A map's cell, which every placement of its ID shows: its map's ID, layer,
/// row and column.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct MapCell {
    map: Rc<str>,
    layer: u32,
    row: u32,
    col: u32,
}

impl Cell {
    /// The cell at `(col, row)` of `layer` of the placement of `map` at
    /// `origin`.
    pub(crate) fn new(map: Rc<str>, origin: Vec2, layer: usize, col: u32, row: u32) -> Self {
        Self {
            map,
            origin: [origin.x.to_bits(), origin.y.to_bits()],
            layer: u32::try_from(layer).unwrap_or(u32::MAX),
            row,
            col,
        }
    }

    /// Its placement's top-left.
    pub(crate) fn origin(&self) -> Vec2 {
        Vec2::new(
            f32::from_bits(self.origin[0]),
            f32::from_bits(self.origin[1]),
        )
    }

    /// Its centre in the world, by its map's tile size.
    pub(crate) fn centre(&self, data: &TilemapData) -> Vec2 {
        self.origin()
            + Vec2::new(
                (self.col as f32 + 0.5) * data.tile_width as f32,
                (self.row as f32 + 0.5) * data.tile_height as f32,
            )
    }

    fn map_cell(&self) -> MapCell {
        MapCell {
            map: Rc::clone(&self.map),
            layer: self.layer,
            row: self.row,
            col: self.col,
        }
    }

    /// Whether `instance` is its placement: its map's ID at its origin.
    fn placed_by(&self, instance: &TilemapInstance) -> bool {
        *instance.id == *self.map && bits(instance.origin) == self.origin
    }

    /// The entity whose `TilemapInstance` places it, when one does.
    pub(crate) fn placement(&self, world: &World) -> Option<Entity> {
        world
            .query::<(Entity, &TilemapInstance)>()
            .find(|(_, instance)| self.placed_by(instance))
            .map(|(entity, _)| entity)
    }

    /// Its map's data, when the World's registry holds it.
    pub(crate) fn data<'w>(&self, world: &'w World) -> Option<&'w TilemapData> {
        world.get_resource::<TilemapRegistry>()?.get(&self.map)
    }

    /// Its tile, when its map is registered and it lies inside it.
    fn tile(&self, world: &World) -> Option<TileIndex> {
        self.data(world)?
            .tile_at(self.layer as usize, self.col, self.row)
    }

    /// Its centre, when its map is registered.
    pub(crate) fn centre_in(&self, world: &World) -> Option<Vec2> {
        self.data(world).map(|data| self.centre(data))
    }

    /// The saved form.
    pub(crate) fn save(&self) -> CellSave {
        let origin = self.origin();
        CellSave {
            map: self.map.to_string(),
            origin: [origin.x, origin.y],
            layer: self.layer,
            col: self.col,
            row: self.row,
        }
    }
}

/// A coordinate pair's bits.
fn bits(at: Vec2) -> [u32; 2] {
    [at.x.to_bits(), at.y.to_bits()]
}

/// A cell as a save writes it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CellSave {
    map: String,
    origin: [f32; 2],
    layer: u32,
    col: u32,
    row: u32,
}

impl CellSave {
    /// The cell it names.
    pub(crate) fn cell(&self) -> Cell {
        Cell::new(
            Rc::from(self.map.as_str()),
            Vec2::new(self.origin[0], self.origin[1]),
            self.layer as usize,
            self.col,
            self.row,
        )
    }
}

/// The map IDs a run names, each allocated once.
#[derive(Debug, Default)]
pub(crate) struct MapNames(Vec<Rc<str>>);

impl MapNames {
    /// The shared ID `id`.
    pub(crate) fn of(&mut self, id: &str) -> Rc<str> {
        if let Some(known) = self.0.iter().find(|known| ***known == *id) {
            return Rc::clone(known);
        }
        let name: Rc<str> = Rc::from(id);
        self.0.push(Rc::clone(&name));
        name
    }
}

/// The material of a tile, when it has one: its tile is a receiver.
fn material(data: &TilemapData, tile: TileIndex) -> Option<&str> {
    (tile >= 0)
        .then(|| data.tile_props(tile).material.as_deref())
        .flatten()
}

/// Whether `cell` receives: its tile has a material.
pub(crate) fn receives(world: &World, cell: &Cell) -> bool {
    let Some(data) = cell.data(world) else {
        return false;
    };
    data.tile_at(cell.layer as usize, cell.col, cell.row)
        .is_some_and(|tile| material(data, tile).is_some())
}

/// Whether `cell`'s placement still stands: a `TilemapInstance` places its
/// map at its origin.
pub(crate) fn placed(world: &World, cell: &Cell) -> bool {
    world
        .query::<&TilemapInstance>()
        .any(|instance| cell.placed_by(instance))
}

/// `cell`'s tags: its tile's material read as a tag (none when the name is
/// no tag) and its statuses' grants and reached bands' grants.
pub(crate) fn tags_of(world: &World, cell: &Cell) -> Tags {
    let mut names: Vec<String> = Vec::new();
    if let (Some(data), Some(tile)) = (cell.data(world), cell.tile(world))
        && let Some(material) = material(data, tile)
        && valid_tag(material)
    {
        names.push(material.to_owned());
    }
    if let Some(instances) = world
        .get_resource::<TileStatuses>()
        .and_then(|statuses| statuses.cells.get(cell))
    {
        for instance in instances {
            let Some(effect) = definition(world, &instance.effect) else {
                continue;
            };
            names.extend(effect.grant().iter().cloned());
            for (_, grant) in effect.bands().take(instance.bands as usize) {
                names.extend(grant.iter().cloned());
            }
        }
    }
    Tags::new(names)
}

/// Into `out`, in their order, every cell of every placement, on every
/// layer, whose centre lies within `radius` of `point` and whose tile has a
/// material, but `skip`.
pub(crate) fn cells_within(
    world: &World,
    point: Vec2,
    radius: f32,
    skip: Option<&Cell>,
    names: &mut MapNames,
    out: &mut Vec<Cell>,
) {
    out.clear();
    let Some(registry) = world.get_resource::<TilemapRegistry>() else {
        return;
    };
    if !(point.is_finite() && radius.is_finite() && radius >= 0.0) {
        return;
    }
    for instance in world.query::<&TilemapInstance>() {
        let Some(data) = registry.get(&instance.id) else {
            continue;
        };
        let (w, h) = (data.tile_width as f32, data.tile_height as f32);
        let local = point - instance.origin;
        // The columns and rows whose centres can lie within the radius.
        let span = |at: f32, size: f32, count: u32| -> Option<(u32, u32)> {
            let low = ((at - radius) / size - 0.5).ceil().max(0.0);
            let high = ((at + radius) / size - 0.5).floor().min(count as f32 - 1.0);
            (low <= high).then_some((low as u32, high as u32))
        };
        let (Some((c0, c1)), Some((r0, r1))) =
            (span(local.x, w, data.width), span(local.y, h, data.height))
        else {
            continue;
        };
        let name = names.of(&instance.id);
        for (layer, tiles) in data.layers.iter().enumerate() {
            for row in r0..=r1 {
                for col in c0..=c1 {
                    let tile = tiles.tiles[(row as usize) * (data.width as usize) + col as usize];
                    if material(data, tile).is_none() {
                        continue;
                    }
                    let cell = Cell::new(Rc::clone(&name), instance.origin, layer, col, row);
                    if (cell.centre(data) - point).length() > radius || Some(&cell) == skip {
                        continue;
                    }
                    out.push(cell);
                }
            }
        }
    }
    out.sort_unstable();
}

/// The cell a body's impact on a tile struck, when its tile has a material.
pub(crate) fn cell_of_hit(world: &World, hit: &TileHit, names: &mut MapNames) -> Option<Cell> {
    let instance = world.get::<TilemapInstance>(hit.map)?;
    let cell = Cell::new(
        names.of(&instance.id),
        instance.origin,
        hit.layer,
        hit.col,
        hit.row,
    );
    receives(world, &cell).then_some(cell)
}

/// One status on a cell: an effect's intensity, decay, bands, duration,
/// emission and `on_end` apply there; its lines, resistances, heal,
/// restore and periodic payload do nothing on a cell.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TileInstance {
    effect: DataRef<Effect>,
    /// Not saved: an entity is not data.
    source: Option<Entity>,
    left: Option<f32>,
    intensity: Option<f32>,
    factor: f32,
    /// How many of its bands its intensity reaches.
    bands: u32,
    emit_left: Option<f32>,
}

impl TileInstance {
    /// Its effect.
    #[cfg(test)]
    pub(crate) fn effect(&self) -> &DataRef<Effect> {
        &self.effect
    }
}

/// A revert waiting on a map cell: the tile the cell held before a
/// `set_tile` output wrote it, by sprite ID (`None` for an empty cell) and
/// whether it was an auto cell, the sprite the output wrote, and the game
/// seconds left.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Revert {
    old: Option<String>,
    auto: bool,
    wrote: String,
    left: f32,
}

/// The statuses on tile cells, sparse by cell (a placement's map and
/// top-left, the layer, column and row), and the `set_tile` reverts waiting
/// per map cell: a World resource [`ChemistryPlugin`](crate::ChemistryPlugin)
/// inserts and a save holds as `tile_statuses`. A cell whose tile has a
/// `material` takes statuses from chemistry's outputs; their intensity,
/// decay, bands, duration, emission and `on_end` apply there.
///
/// # Examples
///
/// ```
/// use glam::Vec2;
/// use tungsten_core::{Plugin, Schedule, World};
/// use tungsten_kit::{ChemistryPlugin, TileCell, TileStatuses};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// ChemistryPlugin::new().build(&mut schedule, &mut world);
/// let cell = TileCell { map: "meadow", origin: Vec2::ZERO, layer: 0, col: 3, row: 0 };
/// let statuses = world.resource::<TileStatuses>();
/// assert!(!statuses.carries(cell, "burning"));
/// assert_eq!(statuses.intensity(cell, "burning"), None);
/// ```
#[derive(Debug, Default)]
pub struct TileStatuses {
    pub(crate) cells: BTreeMap<Cell, Vec<TileInstance>>,
    reverts: BTreeMap<MapCell, Revert>,
}

impl TileStatuses {
    /// Whether `cell` carries the status `status`, an `effects` entry ID.
    #[must_use]
    pub fn carries(&self, cell: TileCell<'_>, status: &str) -> bool {
        self.instance(cell, status).is_some()
    }

    /// The intensity of `cell`'s status `status`, when it carries one that
    /// has an intensity.
    #[must_use]
    pub fn intensity(&self, cell: TileCell<'_>, status: &str) -> Option<f32> {
        self.instance(cell, status)?.intensity
    }

    fn instance(&self, cell: TileCell<'_>, status: &str) -> Option<&TileInstance> {
        let key = Cell::new(
            Rc::from(cell.map),
            cell.origin,
            cell.layer,
            cell.col,
            cell.row,
        );
        self.on(&key)
            .iter()
            .find(|instance| instance.effect.id() == Some(status))
    }

    /// Whether it holds no status and no revert.
    pub(crate) fn is_empty(&self) -> bool {
        self.cells.is_empty() && self.reverts.is_empty()
    }

    /// The statuses on `cell`.
    pub(crate) fn on(&self, cell: &Cell) -> &[TileInstance] {
        self.cells.get(cell).map_or(&[], Vec::as_slice)
    }
}

/// The World's tile statuses, inserted when missing.
fn statuses_mut(world: &mut World) -> &mut TileStatuses {
    if !world.has_resource::<TileStatuses>() {
        world.insert_resource(TileStatuses::default());
    }
    world.resource_mut::<TileStatuses>()
}

/// Queues `lists` on `cell`, each `(list, depth, credit)`.
fn queue_lists(world: &mut World, cell: &Cell, lists: Vec<(StatusList, u8, Option<Entity>)>) {
    for (list, depth, credit) in lists {
        queue_status_list(world, Receiver::Cell(cell.clone()), list, depth, credit);
    }
}

fn on_end_of(effect: &DataRef<Effect>) -> StatusList {
    StatusList {
        effect: effect.clone(),
        band: None,
    }
}

/// The band entries from `from` bands to `to`, those with outputs.
fn entered(
    effect: &Effect,
    effect_ref: &DataRef<Effect>,
    from: usize,
    to: usize,
    depth: u8,
    credit: Option<Entity>,
    lists: &mut Vec<(StatusList, u8, Option<Entity>)>,
) {
    for band in from..to {
        if effect
            .band_enter(band)
            .is_some_and(|enter| !enter.is_empty())
        {
            let list = StatusList {
                effect: effect_ref.clone(),
                band: Some(band as u32),
            };
            lists.push((list, depth, credit));
        }
    }
}

/// Applies `effect` to `cell` as an entity takes a request of `amount` at
/// `status_effect`: a status built by it to its max, an infinite or timed
/// one placed or refreshed by its stacking; an instant one ends as it
/// lands. Its band entries and an end queue `depth` deep, credited to
/// `credit`. Whether it took.
pub(crate) fn apply(
    world: &mut World,
    cell: &Cell,
    effect_ref: &DataRef<Effect>,
    amount: f32,
    status_effect: f32,
    credit: Option<Entity>,
    depth: u8,
) -> bool {
    let Some(effect) = definition(world, effect_ref).cloned() else {
        return false;
    };
    let existing = world.get_resource::<TileStatuses>().and_then(|statuses| {
        statuses
            .on(cell)
            .iter()
            .position(|instance| instance.effect == *effect_ref)
    });
    let prior = existing.and_then(|at| world.resource::<TileStatuses>().on(cell)[at].intensity);
    let intensity = effect
        .intensity()
        .map(|(max, _)| built(prior.unwrap_or(0.0), amount, status_effect, max));
    if existing.is_none() && intensity.is_some_and(|intensity| intensity <= 0.0) {
        return false;
    }
    let factor = match intensity {
        Some(intensity) => factor_of(world, &effect, intensity),
        None => factor_of(world, &effect, status_effect),
    };
    let mut lists = Vec::new();
    if effect.scaled_duration(factor) == Some(0.0) {
        if !effect.on_end().is_empty() {
            lists.push((on_end_of(effect_ref), depth, credit));
        }
        queue_lists(world, cell, lists);
        return true;
    }
    let reached = effect.bands_reached(intensity);
    let statuses = statuses_mut(world);
    let instances = statuses.cells.entry(cell.clone()).or_default();
    let was = match existing {
        None => {
            instances.push(TileInstance {
                effect: effect_ref.clone(),
                source: credit,
                left: effect.scaled_duration(factor),
                intensity,
                factor,
                bands: 0,
                emit_left: effect.emission().map(|emit| emit.period),
            });
            0
        }
        Some(at) => {
            let instance = &mut instances[at];
            let duration = effect.scaled_duration(factor);
            instance.left = match effect.refresh() {
                Refresh::Keep => instance.left,
                Refresh::Extend => match (instance.left, duration) {
                    (Some(left), Some(duration)) => Some(
                        (f64::from(left) + f64::from(duration)).min(f64::from(f32::MAX)) as f32,
                    ),
                    _ => None,
                },
                _ => duration,
            };
            instance.intensity = intensity;
            instance.factor = factor;
            if credit.is_some() {
                instance.source = credit;
            }
            instance.bands as usize
        }
    };
    let at = existing.unwrap_or(instances.len() - 1);
    instances[at].bands = reached as u32;
    if reached > was {
        entered(&effect, effect_ref, was, reached, depth, credit, &mut lists);
    }
    queue_lists(world, cell, lists);
    true
}

/// Ends `effect`'s status on `cell`, its `on_end` queued `depth` deep.
/// Whether there was one.
pub(crate) fn remove(
    world: &mut World,
    cell: &Cell,
    effect_ref: &DataRef<Effect>,
    depth: u8,
) -> bool {
    let Some(statuses) = world.get_resource_mut::<TileStatuses>() else {
        return false;
    };
    let Some(instances) = statuses.cells.get_mut(cell) else {
        return false;
    };
    let Some(at) = instances
        .iter()
        .position(|instance| instance.effect == *effect_ref)
    else {
        return false;
    };
    let instance = instances.remove(at);
    if instances.is_empty() {
        statuses.cells.remove(cell);
    }
    end(world, cell, &instance, depth);
    true
}

/// Queues `instance`'s `on_end` on `cell`, `depth` deep, when it has one.
fn end(world: &mut World, cell: &Cell, instance: &TileInstance, depth: u8) {
    let ends =
        definition(world, &instance.effect).is_some_and(|effect| !effect.on_end().is_empty());
    if ends {
        queue_lists(
            world,
            cell,
            vec![(on_end_of(&instance.effect), depth, instance.source)],
        );
    }
}

/// Takes `amount` off the intensity of `effect`'s status on `cell`: at 0
/// it has run out, its `on_end` queued `depth` deep. False without such a
/// status.
pub(crate) fn consume(
    world: &mut World,
    cell: &Cell,
    effect_ref: &DataRef<Effect>,
    amount: f32,
    depth: u8,
) -> bool {
    let Some(effect) = definition(world, effect_ref).cloned() else {
        return false;
    };
    let Some(at) = world.get_resource::<TileStatuses>().and_then(|statuses| {
        statuses
            .on(cell)
            .iter()
            .position(|instance| instance.effect == *effect_ref && instance.intensity.is_some())
    }) else {
        return false;
    };
    let was = world.resource::<TileStatuses>().on(cell)[at]
        .intensity
        .unwrap_or_default();
    let rest = count_down(was, amount);
    if rest <= 0.0 {
        return remove(world, cell, effect_ref, depth);
    }
    let factor = factor_of(world, &effect, rest);
    let instance = &mut statuses_mut(world)
        .cells
        .get_mut(cell)
        .expect("found above")[at];
    instance.intensity = Some(rest);
    instance.factor = factor;
    instance.bands = effect.bands_reached(Some(rest)) as u32;
    true
}

/// [`chemistry_tick`](crate::CHEMISTRY_TICK)'s work on tiles: the statuses
/// of a placement no `TilemapInstance` holds, gone or moved, and the
/// reverts of a map no placement shows drop; each status counts its time,
/// its intensity's decay and its emission's clock by `dt`, an end queueing
/// its `on_end` at depth 0, an emission from its cell's centre; each revert
/// counts down and writes its cell's old tile back while the registry's
/// edit record still holds its write.
pub(crate) fn tick(world: &mut World, dt: f32) {
    if world
        .get_resource::<TileStatuses>()
        .is_none_or(TileStatuses::is_empty)
    {
        return;
    }
    let placements: Vec<(String, [u32; 2])> = world
        .query::<&TilemapInstance>()
        .map(|instance| (instance.id.clone(), bits(instance.origin)))
        .collect();
    let mut statuses = world
        .remove_resource::<TileStatuses>()
        .expect("checked above");
    statuses.cells.retain(|cell, _| {
        placements
            .iter()
            .any(|(id, origin)| **id == *cell.map && *origin == cell.origin)
    });
    statuses
        .reverts
        .retain(|map_cell, _| placements.iter().any(|(id, _)| **id == *map_cell.map));
    let mut ends: Vec<(Cell, TileInstance)> = Vec::new();
    let mut emissions: Vec<(Cell, Named, f32, f32, Option<Entity>)> = Vec::new();
    let mut lists: Vec<(Cell, StatusList, u8, Option<Entity>)> = Vec::new();
    if dt > 0.0 {
        for (cell, instances) in &mut statuses.cells {
            let mut i = 0;
            while i < instances.len() {
                let instance = &mut instances[i];
                let Some(effect) = definition(world, &instance.effect) else {
                    i += 1;
                    continue;
                };
                let elapsed = instance.left.map_or(dt, |left| dt.min(left));
                // The emission's clock, as an entity's instance counts it.
                if let Some(emit) = effect.emission() {
                    let mut next = instance.emit_left.unwrap_or(emit.period) - elapsed;
                    let mut elapses = 0_u32;
                    while next <= TIME_EPSILON && elapses < Cooldown::MAX_ELAPSES_PER_STEP {
                        elapses += 1;
                        next += emit.period;
                    }
                    if next <= TIME_EPSILON {
                        next = emit.period;
                    }
                    instance.emit_left = Some(next);
                    if elapses > 0 {
                        let amount = (f64::from(emit.amount)
                            * f64::from(elapses)
                            * f64::from(instance.factor))
                        .min(f64::from(f32::MAX)) as f32;
                        emissions.push((
                            cell.clone(),
                            Named::New(emit.stimulus.clone()),
                            amount,
                            emit.radius,
                            instance.source,
                        ));
                    }
                } else {
                    instance.emit_left = None;
                }
                let mut over = false;
                if let (Some((_, decay)), Some(was)) = (effect.intensity(), instance.intensity) {
                    let rest = count_down(was, decay * elapsed);
                    if rest > 0.0 {
                        instance.intensity = Some(rest);
                        instance.factor = factor_of(world, effect, rest);
                        instance.bands = effect.bands_reached(Some(rest)) as u32;
                    } else {
                        over = true;
                    }
                }
                if let Some(was) = instance.left {
                    let rest = count_down(was, dt);
                    instance.left = Some(rest);
                    if rest <= 0.0 {
                        over = true;
                    }
                }
                if over {
                    ends.push((cell.clone(), instances.remove(i)));
                } else {
                    i += 1;
                }
            }
        }
        statuses.cells.retain(|_, instances| !instances.is_empty());
    }
    let mut due: Vec<(MapCell, Revert)> = Vec::new();
    if dt > 0.0 {
        for (map_cell, revert) in &mut statuses.reverts {
            revert.left = count_down(revert.left, dt);
            if revert.left <= 0.0 {
                due.push((map_cell.clone(), revert.clone()));
            }
        }
        statuses.reverts.retain(|_, revert| revert.left > 0.0);
    }
    world.insert_resource(statuses);
    for (cell, instance) in ends {
        if definition(world, &instance.effect).is_some_and(|effect| !effect.on_end().is_empty()) {
            lists.push((cell, on_end_of(&instance.effect), 0, instance.source));
        }
    }
    for (cell, list, depth, credit) in lists {
        queue_status_list(world, Receiver::Cell(cell), list, depth, credit);
    }
    for (cell, stimulus, amount, radius, credit) in emissions {
        emit_from_cell(world, &cell, stimulus, amount, radius, credit, 0);
    }
    for (map_cell, revert) in due {
        write_back(world, &map_cell, &revert);
    }
}

/// Queues an emission from `cell`'s centre for chemistry's next run,
/// `depth` reactions deep; nothing without its map or chemistry.
pub(crate) fn emit_from_cell(
    world: &mut World,
    cell: &Cell,
    stimulus: Named,
    amount: f32,
    radius: f32,
    credit: Option<Entity>,
    depth: u8,
) {
    let Some(point) = cell.centre_in(world) else {
        return;
    };
    let Some(chemistry) = world.get_resource_mut::<crate::Chemistry>() else {
        return;
    };
    let stimulus = match stimulus {
        Named::Known(id) => id,
        Named::New(name) => chemistry.intern(&name),
    };
    chemistry.emitted.push(Emission {
        stimulus,
        amount,
        radius,
        point,
        from: Some(Receiver::Cell(cell.clone())),
        credit,
        depth,
    });
}

/// The first tileset index holding `sprite`.
fn index_of(data: &TilemapData, sprite: &str) -> Option<TileIndex> {
    data.tileset
        .iter()
        .position(|known| known == sprite)
        .and_then(|at| TileIndex::try_from(at).ok())
}

/// Writes `sprite` at `cell` through the registry, which every placement of
/// its map shows. With `revert`, the cell's old tile comes back after that
/// many seconds: a revert already waiting on the map cell keeps its old
/// tile, so overlapping reverts end on the original; without, a waiting
/// revert is dropped.
pub(crate) fn set_tile(
    world: &mut World,
    cell: &Cell,
    sprite: &str,
    revert: Option<f32>,
) -> Result<(), &'static str> {
    let (layer, col, row) = (cell.layer as usize, cell.col, cell.row);
    let Some(data) = cell.data(world) else {
        return Err(NO_MAP);
    };
    let Some(old_tile) = data.tile_at(layer, col, row) else {
        return Err(NO_MAP);
    };
    let Some(index) = index_of(data, sprite) else {
        return Err(NO_SPRITE);
    };
    let old = usize::try_from(old_tile)
        .ok()
        .and_then(|at| data.tileset.get(at).cloned());
    let auto = data.is_auto(layer, col, row);
    world
        .resource_mut::<TilemapRegistry>()
        .set_tile(&cell.map, layer, col, row, index);
    let key = cell.map_cell();
    let statuses = statuses_mut(world);
    match revert {
        Some(left) => {
            let (old, auto) = match statuses.reverts.get(&key) {
                Some(waiting) => (waiting.old.clone(), waiting.auto),
                None => (old, auto),
            };
            statuses.reverts.insert(
                key,
                Revert {
                    old,
                    auto,
                    wrote: sprite.to_owned(),
                    left,
                },
            );
        }
        None => {
            statuses.reverts.remove(&key);
        }
    }
    Ok(())
}

/// A revert run out: the old tile written back while the registry's edit
/// record still holds the revert's write for the cell; an auto cell's
/// through its style's brush, so the auto-tiler resolves and keeps it.
fn write_back(world: &mut World, map_cell: &MapCell, revert: &Revert) {
    let (layer, col, row) = (map_cell.layer as usize, map_cell.col, map_cell.row);
    let Some(data) = world
        .get_resource::<TilemapRegistry>()
        .and_then(|registry| registry.get(&map_cell.map))
    else {
        return;
    };
    let registry = world.resource::<TilemapRegistry>();
    let stands = registry
        .edited_cells(&map_cell.map)
        .find(|(cell, _)| (cell.layer, cell.col, cell.row) == (layer, col, row))
        .and_then(|(_, tile)| usize::try_from(tile).ok())
        .and_then(|at| data.tileset.get(at))
        .is_some_and(|sprite| *sprite == revert.wrote);
    if !stands {
        let placement = world
            .query::<(Entity, &TilemapInstance)>()
            .find(|(_, instance)| *instance.id == *map_cell.map)
            .map(|(entity, _)| entity);
        if let Some(placement) = placement {
            trace(world, placement, || TraceKind::Skipped {
                event: "interaction",
                action: "set_tile",
                why: REVERT_DROPPED,
            });
        }
        return;
    }
    let old = revert
        .old
        .as_deref()
        .and_then(|sprite| index_of(data, sprite));
    let tile = match (old, revert.auto) {
        (Some(old), true) => {
            let style = data.tile_props(old).style.clone();
            (0..data.tileset.len())
                .filter_map(|at| TileIndex::try_from(at).ok())
                .find(|&at| {
                    let props = data.tile_props(at);
                    props.is_brush() && props.style == style
                })
                .unwrap_or(old)
        }
        (Some(old), false) => old,
        (None, _) => EMPTY_TILE,
    };
    world
        .resource_mut::<TilemapRegistry>()
        .set_tile(&map_cell.map, layer, col, row, tile);
}

/// Damages `cell` by `amount` through `damage_tile`: a cell of a tile with
/// health keeps what is left and breaks at 0.
pub(crate) fn damage(world: &mut World, cell: &Cell, amount: f32) -> Result<(), &'static str> {
    if cell.tile(world).is_none() {
        return Err(NO_MAP);
    }
    let outcome = world.resource_mut::<TilemapRegistry>().damage_tile(
        &cell.map,
        cell.layer as usize,
        cell.col,
        cell.row,
        amount,
    );
    match outcome {
        tungsten_core::TileDamage::Unbreakable => Err(UNBREAKABLE),
        _ => Ok(()),
    }
}

/// A cell's statuses as a save writes them.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CellStatusesSave {
    cell: CellSave,
    statuses: Vec<TileInstanceSave>,
}

/// A tile status as a save writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TileInstanceSave {
    effect: DataRef<Effect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    left: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    intensity: Option<f32>,
    #[serde(default = "one")]
    factor: f32,
    #[serde(default)]
    bands: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    emit_left: Option<f32>,
}

fn one() -> f32 {
    1.0
}

/// A revert as a save writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RevertSave {
    map: String,
    layer: u32,
    col: u32,
    row: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    old: Option<String>,
    #[serde(default)]
    auto: bool,
    wrote: String,
    left: f32,
}

/// The tile statuses as a save writes them.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct TileStatusesSave {
    cells: Vec<CellStatusesSave>,
    reverts: Vec<RevertSave>,
}

/// A saved number as a clock: finite, at least 0.
fn clock(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

impl From<&TileStatuses> for TileStatusesSave {
    fn from(statuses: &TileStatuses) -> Self {
        Self {
            cells: statuses
                .cells
                .iter()
                .map(|(cell, instances)| CellStatusesSave {
                    cell: cell.save(),
                    statuses: instances
                        .iter()
                        .map(|instance| TileInstanceSave {
                            effect: instance.effect.clone(),
                            left: instance.left,
                            intensity: instance.intensity,
                            factor: instance.factor,
                            bands: instance.bands,
                            emit_left: instance.emit_left,
                        })
                        .collect(),
                })
                .collect(),
            reverts: statuses
                .reverts
                .iter()
                .map(|(key, revert)| RevertSave {
                    map: key.map.to_string(),
                    layer: key.layer,
                    col: key.col,
                    row: key.row,
                    old: revert.old.clone(),
                    auto: revert.auto,
                    wrote: revert.wrote.clone(),
                    left: revert.left,
                })
                .collect(),
        }
    }
}

impl From<TileStatusesSave> for TileStatuses {
    fn from(saved: TileStatusesSave) -> Self {
        let mut names = MapNames::default();
        let mut statuses = TileStatuses::default();
        for entry in saved.cells {
            let mut cell = entry.cell.cell();
            cell.map = names.of(&cell.map);
            let instances: Vec<TileInstance> = entry
                .statuses
                .into_iter()
                .map(|saved| TileInstance {
                    effect: saved.effect,
                    source: None,
                    left: saved.left.map(clock),
                    intensity: saved
                        .intensity
                        .filter(|intensity| intensity.is_finite() && *intensity > 0.0),
                    factor: clock(saved.factor),
                    bands: saved.bands,
                    emit_left: saved.emit_left.map(clock),
                })
                .collect();
            if !instances.is_empty() {
                statuses.cells.insert(cell, instances);
            }
        }
        for revert in saved.reverts {
            statuses.reverts.insert(
                MapCell {
                    map: names.of(&revert.map),
                    layer: revert.layer,
                    row: revert.row,
                    col: revert.col,
                },
                Revert {
                    old: revert.old,
                    auto: revert.auto,
                    wrote: revert.wrote,
                    left: clock(revert.left),
                },
            );
        }
        statuses
    }
}

/// The tile statuses' save options: a plain form, no references.
pub(crate) fn tile_saved() -> SaveOptions<TileStatuses> {
    SaveOptions::<TileStatuses>::form::<TileStatusesSave>()
}
