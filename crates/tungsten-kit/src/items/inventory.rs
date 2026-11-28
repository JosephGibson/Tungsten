//! The inventory (redo 8.1's second plan, P8-3, Q17): a holder's bag of
//! stacked cells and its worn slots, moved by calls at once; a worn item
//! as one equipment status, apart from every other, its `added` and `pen`
//! lines on the holder's skills; the game's item ledger; and the restore
//! hook that rebuilds what worn items apply from the data of the day.

use std::collections::BTreeMap;

use glam::Vec2;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use tungsten_core::save;
use tungsten_core::{DataRef, DataRegistry, Entity, MissingAsset, SpawnContext, World};

use crate::damage::effects::{
    Effect, EffectRequest, apply_apart_quietly, end_effect_serial, end_effect_serial_quietly,
};
use crate::foundation::describe::{
    ARPG, ComponentDescription, FieldDescription, FieldType, RPG, describe, describe_shape,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::stats::valid_stat;
use crate::foundation::trace::{TraceKind, trace};
use crate::intent::ability::Abilities;
use crate::items::base::{BuiltKind, ItemBase, MAX_STACK};
use crate::items::item::{Item, built, check_item};
use crate::motion::drive::centre_of;
use crate::react::cues::{Target, run_list};

/// The scene component and its saved name.
pub(crate) const INVENTORY: &str = "inventory";
/// The ledger's saved name.
const ITEM_LEDGER: &str = "item_ledger";
/// The restore hook's name.
const RESTORE_HOOK: &str = "kit.items";
/// The most cells a bag has.
const MAX_CELLS: u32 = 256;
/// The most slots an inventory wears.
const MAX_SLOTS: usize = 32;
/// Why a worn item's skill lines went nowhere.
const NO_ABILITIES: &str = "the holder has no Abilities for the item's skill lines";

/// The source a worn item's skill lines are placed under.
pub(crate) fn item_source(uid: u32) -> String {
    format!("item:{uid}")
}

/// A place in an inventory: a bag cell by index, or a worn slot by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemPlace {
    /// The bag's cell, from 0.
    Cell(usize),
    /// The worn slot of this name.
    Worn(String),
}

impl ItemPlace {
    /// The worn slot `name`.
    #[must_use]
    pub fn worn(name: impl Into<String>) -> Self {
        Self::Worn(name.into())
    }
}

/// Why a move was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ItemRefusal {
    /// The holder has no inventory, or the place it moves from holds no
    /// item.
    NoItem,
    /// The place it moves to is no cell or slot of the inventory.
    NoPlace,
    /// The item is not worn in that slot's kind, or a swap would put one
    /// in a slot of another kind.
    WrongSlot,
    /// The holder fails the base's `requires`.
    Requires,
    /// No cell is free.
    Full,
    /// A stack of more than one cannot be worn.
    Stacked,
}

/// One worn slot: the slot kind it takes, the item worn there and its
/// equipment status's serial.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WornSlot {
    kind: String,
    item: Option<Item>,
    status: Option<u64>,
}

impl WornSlot {
    /// The slot kind it takes.
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The item worn there.
    #[must_use]
    pub fn item(&self) -> Option<&Item> {
        self.item.as_ref()
    }

    /// The serial of the worn item's equipment status.
    #[must_use]
    pub fn status(&self) -> Option<u64> {
        self.status
    }
}

/// A holder's bag and worn slots: a scene component (`inventory`) that
/// saves with its declarations, every cell and every slot, empty ones too,
/// since a restore inserts it whole. Moved by [`give`], [`give_item`],
/// [`take`], [`move_item`], [`use_item`] and [`use_item_on`], all at once.
///
/// Scene name `inventory`: `{ "cells": 24, "slots": { "lantern": "lantern",
/// "charm_1": "charm" }, "items": ["oil_flask", { "base": "oilskin_hood" }] }`,
/// 1 to 256 cells, at most 32 slots each a name and its slot kind, and the
/// items the bag's first cells hold at spawn, a base ID standing for one
/// of it; the load checks each.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Inventory, Item};
///
/// let bag = Inventory::new(4).with_slot("hood", "hood").with_item(Item::new("oil_flask").with_count(3));
/// assert_eq!(bag.len(), 4);
/// assert_eq!(bag.cell(0).map(Item::count), Some(3));
/// assert_eq!(bag.slot("hood").map(|slot| slot.kind()), Some("hood"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    cells: Vec<Option<Item>>,
    slots: BTreeMap<String, WornSlot>,
}

impl Inventory {
    /// A bag of `cells` empty cells and no slots.
    ///
    /// # Panics
    ///
    /// When `cells` is outside 1 to 256.
    #[must_use]
    #[track_caller]
    pub fn new(cells: u32) -> Self {
        assert!(
            (1..=MAX_CELLS).contains(&cells),
            "Inventory::new: 1 to {MAX_CELLS} cells (got {cells})"
        );
        Self {
            cells: vec![None; cells as usize],
            slots: BTreeMap::new(),
        }
    }

    /// With the empty slot `name`, which takes items of slot kind `kind`.
    ///
    /// # Panics
    ///
    /// When it has 32 slots, or a name is not `[a-z][a-z0-9_]*`.
    #[must_use]
    #[track_caller]
    pub fn with_slot(mut self, name: &str, kind: &str) -> Self {
        assert!(
            valid_stat(name) && valid_stat(kind),
            "Inventory::with_slot: `{name}` and `{kind}` are names (`[a-z][a-z0-9_]*`)"
        );
        assert!(
            self.slots.len() < MAX_SLOTS,
            "Inventory::with_slot: at most {MAX_SLOTS} slots"
        );
        self.slots.insert(
            name.to_owned(),
            WornSlot {
                kind: kind.to_owned(),
                item: None,
                status: None,
            },
        );
        self
    }

    /// With `item` in its first empty cell.
    ///
    /// # Panics
    ///
    /// When no cell is empty.
    #[must_use]
    #[track_caller]
    pub fn with_item(mut self, item: Item) -> Self {
        let Some(cell) = self.cells.iter_mut().find(|cell| cell.is_none()) else {
            panic!("Inventory::with_item: no empty cell");
        };
        *cell = Some(item);
        self
    }

    /// How many cells the bag has.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Whether every cell is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.iter().all(Option::is_none)
    }

    /// The bag's cells, each its item or none.
    pub fn cells(&self) -> impl Iterator<Item = Option<&Item>> {
        self.cells.iter().map(Option::as_ref)
    }

    /// The item in cell `cell`.
    #[must_use]
    pub fn cell(&self, cell: usize) -> Option<&Item> {
        self.cells.get(cell)?.as_ref()
    }

    /// The worn slots, by name.
    pub fn slots(&self) -> impl Iterator<Item = (&str, &WornSlot)> {
        self.slots.iter().map(|(name, slot)| (name.as_str(), slot))
    }

    /// The worn slot `name`.
    #[must_use]
    pub fn slot(&self, name: &str) -> Option<&WornSlot> {
        self.slots.get(name)
    }

    /// The item worn in slot `name`.
    #[must_use]
    pub fn worn(&self, name: &str) -> Option<&Item> {
        self.slots.get(name)?.item.as_ref()
    }

    /// Takes `count` off cell `cell`'s item, emptying it at its last.
    pub(crate) fn spend(&mut self, cell: usize, count: u32) {
        let Some(slot) = self.cells.get_mut(cell) else {
            return;
        };
        if let Some(item) = slot {
            if item.count() > count {
                item.set_count(item.count() - count);
            } else {
                *slot = None;
            }
        }
    }

    /// How many of base `base` the bag and the worn slots hold.
    #[must_use]
    pub fn count_of(&self, base: &str) -> u32 {
        self.cells
            .iter()
            .flatten()
            .chain(self.slots.values().filter_map(|slot| slot.item.as_ref()))
            .filter(|item| item.base() == base)
            .fold(0_u32, |sum, item| sum.saturating_add(item.count()))
    }
}

/// The game's item ledger, a World resource saved at version 1: the next
/// uid a bag gives an item, and how many of each base each shop has sold
/// (`<shop>/<base>`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemLedger {
    next_uid: u32,
    #[serde(default)]
    sold: BTreeMap<String, u32>,
}

impl Default for ItemLedger {
    fn default() -> Self {
        Self {
            next_uid: 1,
            sold: BTreeMap::new(),
        }
    }
}

impl ItemLedger {
    /// The uid the next item a bag first takes is given.
    #[must_use]
    pub fn next_uid(&self) -> u32 {
        self.next_uid
    }

    /// How many of `base` shop `shop` has sold.
    #[must_use]
    pub fn sold(&self, shop: &str, base: &str) -> u32 {
        self.sold
            .get(&format!("{shop}/{base}"))
            .copied()
            .unwrap_or(0)
    }
}

/// The next uid, from the World's ledger, inserted when it has none.
fn next_uid(world: &mut World) -> u32 {
    if !world.has_resource::<ItemLedger>() {
        world.insert_resource(ItemLedger::default());
    }
    let ledger = world.resource_mut::<ItemLedger>();
    let uid = ledger.next_uid;
    ledger.next_uid = ledger.next_uid.saturating_add(1).max(1);
    uid
}

/// `holder`'s inventory, to read its cells and slots.
#[must_use]
pub fn inventory(world: &World, holder: Entity) -> Option<&Inventory> {
    world.get::<Inventory>(holder)
}

/// How many of base `base` `holder` holds, bag and worn.
#[must_use]
pub fn count_of(world: &World, holder: Entity, base: &str) -> u32 {
    inventory(world, holder).map_or(0, |inventory| inventory.count_of(base))
}

/// Base `id`'s entry.
fn base_of<'w>(world: &'w World, id: &str) -> Option<&'w ItemBase> {
    world.get_resource::<DataRegistry>()?.get::<ItemBase>(id)
}

/// Gives `holder` `count` of base `base` at once: onto its bag's stacks of
/// that base with no affixes and the same level and rarity, up to the
/// base's `stack`, then into its first empty cells, each new item given a
/// uid. How many fit; 0 for a holder with no inventory or an unknown base.
pub fn give(world: &mut World, holder: Entity, base: &str, count: u32) -> u32 {
    let Some(stack) = base_of(world, base).map(ItemBase::stack) else {
        return 0;
    };
    if count == 0 || !world.has::<Inventory>(holder) {
        return 0;
    }
    let template = Item::new(base);
    let mut left = count;
    let bag = world.get_mut::<Inventory>(holder).expect("checked");
    for item in bag.cells.iter_mut().flatten() {
        if left == 0 {
            break;
        }
        if item.stacks_with(&template) && item.count() < stack {
            let added = left.min(stack - item.count());
            item.set_count(item.count() + added);
            left -= added;
        }
    }
    while left > 0 {
        let Some(at) = world
            .get::<Inventory>(holder)
            .and_then(|bag| bag.cells.iter().position(Option::is_none))
        else {
            break;
        };
        let added = left.min(stack);
        let mut item = template.clone().with_count(added);
        item.set_uid(next_uid(world));
        world.get_mut::<Inventory>(holder).expect("checked").cells[at] = Some(item);
        left -= added;
    }
    count - left
}

/// How many of `item` `holder`'s bag has room for, its base's `stack` to
/// a cell (the item's own count for a base the data lacks), and that stack;
/// `None` without a bag.
fn room_for(world: &World, holder: Entity, item: &Item) -> Option<(u64, u32)> {
    let stack = base_of(world, item.base()).map_or(item.count(), ItemBase::stack);
    let bag = world.get::<Inventory>(holder)?;
    let stackable = item.affixes().is_empty() && item.name_words().next().is_none();
    let room = bag
        .cells
        .iter()
        .map(|cell| match cell {
            None => u64::from(stack),
            Some(held) if stackable && held.stacks_with(item) => {
                u64::from(stack.saturating_sub(held.count()))
            }
            Some(_) => 0,
        })
        .sum();
    Some((room, stack))
}

/// Whether `holder`'s bag takes `item` whole, as [`give_item`] would.
pub(crate) fn takes_whole(world: &World, holder: Entity, item: &Item) -> bool {
    room_for(world, holder, item).is_some_and(|(room, _)| room >= u64::from(item.count()))
}

/// Gives `holder` `item` whole, at once: onto the bag's stacks it stacks
/// with, then into empty cells, given a uid when it has none; or hands it
/// back, the bag unchanged, when it does not all fit.
pub fn give_item(world: &mut World, holder: Entity, item: Item) -> Result<(), Item> {
    let Some((room, stack)) = room_for(world, holder, &item) else {
        return Err(item);
    };
    if room < u64::from(item.count()) {
        return Err(item);
    }
    let stackable = item.affixes().is_empty() && item.name_words().next().is_none();
    let mut left = item.count();
    if stackable {
        let bag = world.get_mut::<Inventory>(holder).expect("checked");
        for held in bag.cells.iter_mut().flatten() {
            if left == 0 {
                break;
            }
            if held.stacks_with(&item) && held.count() < stack {
                let added = left.min(stack - held.count());
                held.set_count(held.count() + added);
                left -= added;
            }
        }
    }
    let mut first = true;
    while left > 0 {
        let at = world
            .get::<Inventory>(holder)
            .and_then(|bag| bag.cells.iter().position(Option::is_none))
            .expect("the room was counted");
        let added = left.min(stack);
        let mut placed = item.clone().with_count(added);
        if !first || placed.uid() == 0 {
            placed.set_uid(next_uid(world));
        }
        first = false;
        world.get_mut::<Inventory>(holder).expect("checked").cells[at] = Some(placed);
        left -= added;
    }
    Ok(())
}

/// Takes `count` of base `base` from `holder`'s bag at once, from its
/// cells in order, never a worn one; how many it took.
pub fn take(world: &mut World, holder: Entity, base: &str, count: u32) -> u32 {
    let Some(bag) = world.get_mut::<Inventory>(holder) else {
        return 0;
    };
    let mut left = count;
    for cell in &mut bag.cells {
        if left == 0 {
            break;
        }
        if let Some(item) = cell
            && item.base() == base
        {
            let taken = left.min(item.count());
            left -= taken;
            if taken == item.count() {
                *cell = None;
            } else {
                item.set_count(item.count() - taken);
            }
        }
    }
    count - left
}

/// Whether `holder` may wear `item` in a slot of kind `kind`: its base's
/// slot is that kind, it is one, and the holder meets its `requires`.
fn may_wear(world: &World, holder: Entity, item: &Item, kind: &str) -> Result<(), ItemRefusal> {
    let Some(base) = base_of(world, item.base()) else {
        return Err(ItemRefusal::WrongSlot);
    };
    if base.slot() != Some(kind) {
        return Err(ItemRefusal::WrongSlot);
    }
    if item.count() > 1 {
        return Err(ItemRefusal::Stacked);
    }
    if base
        .requires()
        .is_some_and(|requires| !requires.holds(world, Some(holder), None))
    {
        return Err(ItemRefusal::Requires);
    }
    Ok(())
}

/// Moves an item in `holder`'s inventory at once: cell to cell merges a
/// stack up to its base's `stack`, else swaps; cell to slot wears it when
/// its base's slot is the slot's kind and the holder meets its `requires`,
/// the item worn there swapping back into the cell; slot to cell takes it
/// off into an empty cell, or swaps with an item there that fits the slot;
/// slot to slot swaps within one kind. Wearing applies the item's
/// equipment status and taking off ends it.
///
/// # Examples
///
/// ```
/// use tungsten_core::{DataRegistry, Plugin, Schedule, World};
/// use tungsten_kit::{Inventory, Item, ItemBase, ItemPlace, KitPlugins, Tags, move_item};
///
/// let mut world = World::new();
/// let mut schedule = Schedule::new();
/// KitPlugins::set().build(&mut schedule, &mut world);
/// let hood: ItemBase =
///     serde_json::from_str(r#"{ "name": "Hood", "icon": "hood", "slot": "hood" }"#).unwrap();
/// world.resource_mut::<DataRegistry>().insert("hood", hood);
/// let warden = world.spawn_with((Inventory::new(4).with_slot("head", "hood").with_item(Item::new("hood")),));
/// move_item(&mut world, warden, ItemPlace::Cell(0), ItemPlace::worn("head")).unwrap();
/// let bag = world.get::<Inventory>(warden).unwrap();
/// assert!(bag.cell(0).is_none() && bag.worn("head").is_some());
/// assert!(bag.slot("head").unwrap().status().is_some(), "worn: its status applied");
/// ```
pub fn move_item(
    world: &mut World,
    holder: Entity,
    from: ItemPlace,
    to: ItemPlace,
) -> Result<(), ItemRefusal> {
    let Some(bag) = world.get::<Inventory>(holder) else {
        return Err(ItemRefusal::NoItem);
    };
    match (from, to) {
        (ItemPlace::Cell(a), ItemPlace::Cell(b)) => {
            let Some(item) = bag.cell(a) else {
                return Err(ItemRefusal::NoItem);
            };
            if b >= bag.len() {
                return Err(ItemRefusal::NoPlace);
            }
            if a == b {
                return Ok(());
            }
            let stack = base_of(world, item.base()).map_or(1, ItemBase::stack);
            let bag = world.get_mut::<Inventory>(holder).expect("checked");
            let moving = bag.cells[a].take().expect("checked");
            match bag.cells[b].take() {
                None => bag.cells[b] = Some(moving),
                Some(mut held) if held.stacks_with(&moving) && held.count() < stack => {
                    let added = moving.count().min(stack - held.count());
                    held.set_count(held.count() + added);
                    bag.cells[b] = Some(held);
                    if added < moving.count() {
                        let mut rest = moving;
                        rest.set_count(rest.count() - added);
                        bag.cells[a] = Some(rest);
                    }
                }
                Some(held) => {
                    bag.cells[b] = Some(moving);
                    bag.cells[a] = Some(held);
                }
            }
            Ok(())
        }
        (ItemPlace::Cell(a), ItemPlace::Worn(name)) => {
            let Some(item) = bag.cell(a) else {
                return Err(ItemRefusal::NoItem);
            };
            let Some(slot) = bag.slot(&name) else {
                return Err(ItemRefusal::NoPlace);
            };
            may_wear(world, holder, item, &slot.kind)?;
            let bag = world.get_mut::<Inventory>(holder).expect("checked");
            let mut wearing = bag.cells[a].take().expect("checked");
            let slot = bag.slots.get_mut(&name).expect("checked");
            let old = slot.item.take();
            let old_status = slot.status.take();
            bag.cells[a].clone_from(&old);
            if let Some(old) = old {
                take_off(world, holder, &old, old_status, false);
            }
            if wearing.uid() == 0 {
                wearing.set_uid(next_uid(world));
            }
            let status = put_on(world, holder, &wearing, false);
            let slot = world
                .get_mut::<Inventory>(holder)
                .and_then(|bag| bag.slots.get_mut(&name))
                .expect("checked");
            slot.item = Some(wearing);
            slot.status = status;
            Ok(())
        }
        (ItemPlace::Worn(name), ItemPlace::Cell(b)) => {
            let Some(slot) = bag.slot(&name) else {
                return Err(ItemRefusal::NoItem);
            };
            if slot.item.is_none() {
                return Err(ItemRefusal::NoItem);
            }
            if b >= bag.len() {
                return Err(ItemRefusal::NoPlace);
            }
            if let Some(other) = bag.cell(b) {
                may_wear(world, holder, other, &slot.kind)?;
            }
            let bag = world.get_mut::<Inventory>(holder).expect("checked");
            let swapping = bag.cells[b].take();
            let slot = bag.slots.get_mut(&name).expect("checked");
            let worn = slot.item.take().expect("checked");
            let status = slot.status.take();
            bag.cells[b] = Some(worn.clone());
            take_off(world, holder, &worn, status, false);
            if let Some(mut wearing) = swapping {
                if wearing.uid() == 0 {
                    wearing.set_uid(next_uid(world));
                }
                let status = put_on(world, holder, &wearing, false);
                let slot = world
                    .get_mut::<Inventory>(holder)
                    .and_then(|bag| bag.slots.get_mut(&name))
                    .expect("checked");
                slot.item = Some(wearing);
                slot.status = status;
            }
            Ok(())
        }
        (ItemPlace::Worn(a), ItemPlace::Worn(b)) => {
            let (Some(from), Some(to)) = (bag.slot(&a), bag.slot(&b)) else {
                return Err(if bag.slot(&a).is_none() {
                    ItemRefusal::NoItem
                } else {
                    ItemRefusal::NoPlace
                });
            };
            if from.item.is_none() {
                return Err(ItemRefusal::NoItem);
            }
            if from.kind != to.kind {
                return Err(ItemRefusal::WrongSlot);
            }
            if a == b {
                return Ok(());
            }
            let bag = world.get_mut::<Inventory>(holder).expect("checked");
            let first = bag.slots.get_mut(&a).expect("checked");
            let (item, status) = (first.item.take(), first.status.take());
            let second = bag.slots.get_mut(&b).expect("checked");
            let held = (second.item.take(), second.status.take());
            second.item = item;
            second.status = status;
            let first = bag.slots.get_mut(&a).expect("checked");
            first.item = held.0;
            first.status = held.1;
            Ok(())
        }
    }
}

/// Uses the item in `holder`'s cell `cell` on the holder at once: one is
/// spent first when its base consumes, so its list cannot reach the unit
/// it spends, then the base's `use` list runs through the story lists'
/// path, the holder its subject, with no other, at the holder's centre.
/// False, doing nothing, for an empty cell or a base with no `use`.
pub fn use_item(world: &mut World, holder: Entity, cell: usize) -> bool {
    use_on(world, holder, cell, holder)
}

/// Uses the item in `holder`'s cell `cell` on `target` at once (PS-13): as
/// [`use_item`], the target the list's subject, at its centre, so a potion
/// used on an ally acts on the ally. False, spending nothing, for a target
/// that is not alive.
pub fn use_item_on(world: &mut World, holder: Entity, cell: usize, target: Entity) -> bool {
    if !world.is_alive(target) {
        return false;
    }
    use_on(world, holder, cell, target)
}

fn use_on(world: &mut World, holder: Entity, cell: usize, subject: Entity) -> bool {
    let Some(item) = inventory(world, holder).and_then(|bag| bag.cell(cell)) else {
        return false;
    };
    let Some(base) = base_of(world, item.base()) else {
        return false;
    };
    if !base.usable() {
        return false;
    }
    let list = base.use_list().to_vec();
    if base.consumes() {
        world
            .get_mut::<Inventory>(holder)
            .expect("read above")
            .spend(cell, 1);
    }
    let at = centre_of(world, subject).unwrap_or(Vec2::ZERO);
    run_list(world, &list, Some(subject), None, at);
    true
}

/// Applies worn `item`'s equipment status to `holder`: one infinite
/// effect instance of its global stat and resistance lines and its base's
/// grants, apart from every other, and its `added` and `pen` lines on the
/// holder's skills under `item:<uid>`; quietly at a restore. The
/// status's serial; none for an item whose base the data lacks.
fn put_on(world: &mut World, holder: Entity, item: &Item, quiet: bool) -> Option<u64> {
    let made = built(world, item)?;
    if !made.known() {
        return None;
    }
    let grant: Vec<String> = base_of(world, item.base())
        .map(|base| base.grant().map(str::to_owned).collect())
        .unwrap_or_default();
    let mut effect = Effect::new().with_grant(grant);
    let mut skill = Vec::new();
    for line in made.lines() {
        match &line.kind {
            BuiltKind::Stat { stat, line } => effect = effect.with_line(stat, line.clone()),
            BuiltKind::Skill(line) => skill.push(line.clone()),
            BuiltKind::Resist { tag, line, floor } => {
                if let Some(line) = line {
                    effect = effect.with_resist_line(tag, line.clone());
                }
                if let Some(floor) = floor {
                    effect = effect.with_resist_floor(tag, *floor);
                }
            }
        }
    }
    let request = EffectRequest::new(holder, DataRef::inline(effect));
    let serial = if quiet {
        apply_apart_quietly(world, &request)
    } else {
        request.apply_apart(world).1
    };
    if !skill.is_empty() {
        match world.get_mut::<Abilities>(holder) {
            Some(abilities) => abilities.add_lines(&item_source(item.uid()), skill),
            None if !quiet => trace(world, holder, || TraceKind::Skipped {
                event: "wear",
                action: "skill_lines",
                why: NO_ABILITIES,
            }),
            None => {}
        }
    }
    serial
}

/// Takes worn `item`'s status off `holder`: its instance ended by serial
/// and its skill lines removed; quietly at a restore.
fn take_off(world: &mut World, holder: Entity, item: &Item, status: Option<u64>, quiet: bool) {
    if let Some(serial) = status {
        if quiet {
            end_effect_serial_quietly(world, holder, serial);
        } else {
            end_effect_serial(world, holder, serial);
        }
    }
    if let Some(abilities) = world.get_mut::<Abilities>(holder) {
        abilities.remove_lines(&item_source(item.uid()));
    }
}

/// The restore hook: each worn item's saved status ends and its skill
/// lines leave, quietly, and it is worn again from the data of the day,
/// quietly, so what it applies is what its tooltip shows; a worn item
/// whose base is gone stays worn and applies nothing.
fn rebuild_worn(world: &mut World) {
    if !world.has_any::<Inventory>() {
        return;
    }
    let holders: Vec<Entity> = world
        .query::<(Entity, &Inventory)>()
        .map(|(entity, _)| entity)
        .collect();
    for holder in holders {
        let worn: Vec<(String, Item, Option<u64>)> = world
            .get::<Inventory>(holder)
            .map(|bag| {
                bag.slots
                    .iter()
                    .filter_map(|(name, slot)| {
                        Some((name.clone(), slot.item.clone()?, slot.status))
                    })
                    .collect()
            })
            .unwrap_or_default();
        for (name, item, status) in worn {
            take_off(world, holder, &item, status, true);
            let status = put_on(world, holder, &item, true);
            if let Some(slot) = world
                .get_mut::<Inventory>(holder)
                .and_then(|bag| bag.slots.get_mut(&name))
            {
                slot.status = status;
            }
        }
    }
}

/// An item a scene names: its base's ID for one of it, or the item.
#[derive(Debug, Clone, PartialEq)]
enum ItemRef {
    Base(String),
    Item(Item),
}

impl ItemRef {
    fn into_item(self) -> Item {
        match self {
            ItemRef::Base(base) => Item::new(base),
            ItemRef::Item(item) => item,
        }
    }
}

impl Serialize for ItemRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ItemRef::Base(base) => base.serialize(serializer),
            ItemRef::Item(item) => item.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for ItemRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::String(base) => Ok(ItemRef::Base(base)),
            object @ Value::Object(_) => serde_json::from_value(object)
                .map(ItemRef::Item)
                .map_err(D::Error::custom),
            other => Err(D::Error::custom(format!(
                "an item, or its base's ID (got {other})"
            ))),
        }
    }
}

/// `components.inventory` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "InventoryFields")]
struct InventoryDef {
    cells: u32,
    slots: BTreeMap<String, String>,
    items: Vec<ItemRef>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InventoryFields {
    cells: u32,
    #[serde(default)]
    slots: BTreeMap<String, String>,
    #[serde(default)]
    items: Vec<ItemRef>,
}

impl TryFrom<InventoryFields> for InventoryDef {
    type Error = String;

    fn try_from(fields: InventoryFields) -> Result<Self, Self::Error> {
        if !(1..=MAX_CELLS).contains(&fields.cells) {
            return Err(format!("cells: 1 to {MAX_CELLS} (got {})", fields.cells));
        }
        if fields.slots.len() > MAX_SLOTS {
            return Err(format!("slots: at most {MAX_SLOTS}"));
        }
        for (name, kind) in &fields.slots {
            if !valid_stat(name) || !valid_stat(kind) {
                return Err(format!(
                    "slots.{name}: a slot's name and its kind `{kind}` are names (`[a-z][a-z0-9_]*`)"
                ));
            }
        }
        if fields.items.len() > fields.cells as usize {
            return Err(format!(
                "items: {} items for {} cells",
                fields.items.len(),
                fields.cells
            ));
        }
        Ok(Self {
            cells: fields.cells,
            slots: fields.slots,
            items: fields.items,
        })
    }
}

impl From<InventoryDef> for Inventory {
    fn from(def: InventoryDef) -> Self {
        let mut cells: Vec<Option<Item>> = def
            .items
            .into_iter()
            .map(|item| Some(item.into_item()))
            .collect();
        cells.resize(def.cells as usize, None);
        Self {
            cells,
            slots: def
                .slots
                .into_iter()
                .map(|(name, kind)| {
                    (
                        name,
                        WornSlot {
                            kind,
                            item: None,
                            status: None,
                        },
                    )
                })
                .collect(),
        }
    }
}

/// The `inventory` component's check: each item's base and affixes.
fn check_inventory(def: &InventoryDef, world: &World) -> Result<(), MissingAsset> {
    for (i, item) in def.items.iter().enumerate() {
        let field = format!("items.{i}");
        match item {
            ItemRef::Base(base) => {
                if base_of(world, base).is_none() && world.get_resource::<DataRegistry>().is_some()
                {
                    return Err(MissingAsset::new(field, "item base", base.as_str()));
                }
            }
            ItemRef::Item(item) => check_item(world, &field, item)?,
        }
    }
    Ok(())
}

/// A `give` or `take` action as data writes it: a base, how many and whose
/// bag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ItemActionFields", into = "ItemActionFields")]
pub(crate) struct ItemActionDef {
    pub(crate) item: String,
    pub(crate) count: u32,
    pub(crate) target: Target,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemActionFields {
    item: String,
    #[serde(default = "one")]
    count: u32,
    #[serde(default)]
    target: Target,
}

fn one() -> u32 {
    1
}

impl TryFrom<ItemActionFields> for ItemActionDef {
    type Error = String;

    fn try_from(fields: ItemActionFields) -> Result<Self, Self::Error> {
        if fields.item.is_empty() {
            return Err("item: an `item_bases` ID".into());
        }
        if !(1..=MAX_STACK).contains(&fields.count) {
            return Err(format!("count: 1 to {MAX_STACK} (got {})", fields.count));
        }
        if fields.target == Target::Game {
            return Err("target: a bag is an entity's, `self` or `other`".into());
        }
        Ok(Self {
            item: fields.item,
            count: fields.count,
            target: fields.target,
        })
    }
}

impl From<ItemActionDef> for ItemActionFields {
    fn from(def: ItemActionDef) -> Self {
        Self {
            item: def.item,
            count: def.count,
            target: def.target,
        }
    }
}

/// The load's check of a base an action or a condition names.
pub(crate) fn check_base_ref(world: &World, field: &str, id: &str) -> Result<(), MissingAsset> {
    match world.get_resource::<DataRegistry>() {
        Some(_) if base_of(world, id).is_none() => Err(MissingAsset::new(field, "item base", id)),
        _ => Ok(()),
    }
}

/// Registers the `inventory` component and [`ItemLedger`], both saved at
/// version 1, the restore hook and their descriptions.
pub(crate) fn build_inventory(world: &mut World) {
    register_rows::<Inventory>(world, "Inventory");
    world.register_saved::<Inventory>(INVENTORY);
    world.register_saved_resource::<ItemLedger>(ITEM_LEDGER);
    world.register_component_with_assets(
        INVENTORY,
        check_inventory,
        |def: InventoryDef, _: &SpawnContext<'_>| (Inventory::from(def),),
    );
    save::on_restore(world, RESTORE_HOOK, rebuild_worn);
    describe_inventory(world);
}

impl tungsten_core::Inspectable for Inventory {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let held = self.cells.iter().flatten().count();
        let worn: Vec<String> = self
            .slots
            .iter()
            .filter_map(|(name, slot)| Some(format!("{name} {}", slot.item.as_ref()?.base())))
            .collect();
        vec![
            ("cells", format!("{held} of {} held", self.cells.len())),
            (
                "worn",
                if worn.is_empty() {
                    "none".to_owned()
                } else {
                    worn.join(", ")
                },
            ),
        ]
    }
}

/// The `inventory` round trip: through the scene form.
fn inventory_round_trip(value: Value) -> Result<Value, String> {
    let def: InventoryDef = serde_json::from_value(value).map_err(|e| e.to_string())?;
    serde_json::to_value(def).map_err(|e| e.to_string())
}

/// The component and its shapes.
fn describe_inventory(world: &mut World) {
    describe(
        world,
        ComponentDescription::new(
            INVENTORY,
            "A bag of stacked cells and worn slots: a worn item's lines on its wearer through one equipment status.",
        )
        .field(FieldDescription::new(
            "cells",
            FieldType::U32 {
                min: Some(1),
                max: Some(MAX_CELLS),
            },
            "how many cells the bag has",
        ))
        .field(
            FieldDescription::new(
                "slots",
                FieldType::Object("slot name → slot kind"),
                "at most 32 worn slots, each `[a-z][a-z0-9_]*` and the slot kind an item's base names to be worn there",
            )
            .default("{}"),
        )
        .field(
            FieldDescription::new(
                "items",
                FieldType::List(Box::new(FieldType::Shape("item"))),
                "the bag's first cells at spawn: an item, or its base's ID for one of it; the load checks each",
            )
            .default("[]"),
        )
        .genre(RPG)
        .genre(ARPG)
        .sample(r#"{ "cells": 24 }"#)
        .roundtrip(inventory_round_trip),
    );
    let shapes = [
        ComponentDescription::new(
            "item_action",
            "Some of a base given to, or taken from, the subject's or the other's bag.",
        )
        .field(FieldDescription::new(
            "item",
            FieldType::Text,
            "the `item_bases` entry; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "count",
                FieldType::U32 {
                    min: Some(1),
                    max: Some(MAX_STACK),
                },
                "how many: a give stacks then fills empty cells, a take counts from the bag's cells in order, never a worn one",
            )
            .default("1"),
        )
        .field(
            FieldDescription::new(
                "target",
                FieldType::Enum(&["self", "other"]),
                "the subject's bag or the other entity's",
            )
            .default(r#""self""#),
        )
        .sample(r#"{ "item": "oil_flask" }"#)
        .roundtrip(crate::foundation::describe::round_trip::<ItemActionDef>),
        ComponentDescription::new(
            "item_condition",
            "How many of a base the target holds, bag and worn, within inclusive bounds; with none, at least 1.",
        )
        .field(FieldDescription::new(
            "id",
            FieldType::Text,
            "the `item_bases` entry; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "min",
                FieldType::I64 {
                    min: None,
                    max: None,
                },
                "the least count that holds",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "max",
                FieldType::I64 {
                    min: None,
                    max: None,
                },
                "the greatest count that holds",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "target",
                FieldType::Enum(&["self", "other"]),
                "the subject's bag or the other entity's",
            )
            .default(r#""self""#),
        )
        .sample(r#"{ "id": "net" }"#)
        .roundtrip(crate::foundation::describe::round_trip::<crate::story::condition::ItemConditionDef>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
}

#[cfg(test)]
#[path = "../tests/inventory.rs"]
mod tests;
