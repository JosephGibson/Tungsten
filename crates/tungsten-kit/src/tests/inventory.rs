use serde_json::{Value, json};
use tungsten_core::save::{self as saves, capture_world, restore_world};
use tungsten_core::{CommandBuffer, EventQueue, PluginSet, PrefabRegistry, Schedule, Stage, Time};

use super::*;
use crate::damage::defense::Resist;
use crate::damage::effects::{EffectApplied, EffectExpired, Effects};
use crate::foundation::stats::Stats;
use crate::foundation::tags::Tags;
use crate::foundation::trace::KitTrace;
use crate::items::affix::Affix;
use crate::items::item::RolledAffix;
use crate::react::counters::Counters;
use crate::{KitPlugins, TagsPlugin};

/// What a restore's spawns take.
#[derive(Debug, Clone, Copy)]
struct Restored;

const TAGS: [&str; 8] = [
    "status.ringed",
    "status.hooded",
    "item.ring",
    "item.hood",
    "skill.spell",
    "damage.fire",
    "team.player",
    "team.ally",
];

fn rig() -> (World, Schedule) {
    rig_without(&[])
}

/// [`rig`] with the bases `skip` names left out of the data.
fn rig_without(skip: &[&str]) -> (World, Schedule) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    let plugins: PluginSet = KitPlugins::set()
        .without::<TagsPlugin>()
        .with(TagsPlugin::new().with_tags(TAGS));
    plugins.build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    let types: crate::damage::kinds::DamageTypes =
        serde_json::from_value(json!({ "damage.fire": {} })).unwrap();
    world.resource_mut::<DataRegistry>().insert("types", types);
    for (id, base) in [
        (
            "oil_flask",
            json!({ "name": "Oil Flask", "icon": "flask", "stack": 5,
            "use": [{ "counter": { "name": "oil", "add": 10 } }] }),
        ),
        (
            "ring",
            json!({ "name": "Ring", "icon": "ring", "tags": ["item.ring"], "slot": "ring",
            "grant": ["status.ringed"],
            "implicit": [{ "stat": "damage", "modifier": { "add": 2 } }] }),
        ),
        (
            "hood",
            json!({ "name": "Hood", "icon": "hood", "tags": ["item.hood"], "slot": "hood",
            "requires": { "counter": { "name": "trust", "min": 3 } },
            "grant": ["status.hooded"],
            "implicit": [{ "resist": "damage.fire", "modifier": { "add": -0.1 } }] }),
        ),
        (
            "wand",
            json!({ "name": "Wand", "icon": "wand", "slot": "hand",
            "implicit": [
                { "added": { "damage.fire": [1, 3] }, "with": { "all": ["skill.spell"] } },
                { "stat": "damage", "modifier": { "pct": 0.5 }, "with": { "all": ["skill.spell"] } }
            ],
            "weapon": { "damage": { "damage.fire": [2, 4] }, "use_time": 1 } }),
        ),
        (
            "potion",
            json!({ "name": "Potion", "icon": "potion", "stack": 5,
            "use": [{ "counter": { "name": "healed", "add": 1 } }] }),
        ),
        (
            "shard",
            json!({ "name": "Shard", "icon": "shard", "stack": 10 }),
        ),
        (
            "gift",
            json!({ "name": "Gift", "icon": "gift",
            "use": [{ "give": { "item": "shard" } }] }),
        ),
    ] {
        if skip.contains(&id) {
            continue;
        }
        let base: ItemBase = serde_json::from_value(base).unwrap_or_else(|e| panic!("{id}: {e}"));
        world.resource_mut::<DataRegistry>().insert(id, base);
    }
    let bright: Affix = serde_json::from_value(json!({
        "kind": "prefix", "group": "bright",
        "tiers": [{ "level": 1, "weight": 1, "values": [[1, 10]] }],
        "lines": [{ "stat": "damage", "modifier": { "add": { "value": 0 } } }]
    }))
    .unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("bright", bright);
    (world, schedule)
}

fn step(world: &mut World, schedule: &mut Schedule) {
    world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
    schedule.run_stage(Stage::FixedUpdate, world, |_, _| {});
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world.flush_events();
}

/// A bag of `cells` with two ring slots, a head and a hand, on a holder
/// with stats, resistances, tags, counters and abilities.
fn bag(cells: u32) -> Inventory {
    Inventory::new(cells)
        .with_slot("ring_1", "ring")
        .with_slot("ring_2", "ring")
        .with_slot("head", "hood")
        .with_slot("hand", "hand")
}

fn holder(world: &mut World, inventory: Inventory) -> Entity {
    world.spawn_with((
        inventory,
        Stats::new().with(Stats::DAMAGE, 10.0),
        Resist::new(),
        Tags::new(["team.player"]),
        Counters::new(),
        crate::Abilities::new(),
    ))
}

/// Each cell as (base, count), `-` for an empty one.
fn cells(world: &World, holder: Entity) -> Vec<(String, u32)> {
    inventory(world, holder)
        .unwrap()
        .cells()
        .map(|cell| {
            cell.map_or(("-".to_owned(), 0), |item| {
                (item.base().to_owned(), item.count())
            })
        })
        .collect()
}

fn row(base: &str, count: u32) -> (String, u32) {
    (base.to_owned(), count)
}

fn empty() -> (String, u32) {
    row("-", 0)
}

fn damage_of(world: &World, holder: Entity) -> f32 {
    world
        .get::<Stats>(holder)
        .unwrap()
        .get(Stats::DAMAGE)
        .unwrap()
}

fn statuses(world: &World, holder: Entity) -> usize {
    world
        .get::<Effects>(holder)
        .map_or(0, |effects| effects.instances().count())
}

#[test]
fn give_stacks_then_fills_the_first_empty_cells() {
    let (mut world, _) = rig();
    let warden = holder(
        &mut world,
        bag(4).with_item(Item::new("oil_flask").with_count(3)),
    );
    assert_eq!(give(&mut world, warden, "oil_flask", 4), 4);
    assert_eq!(
        cells(&world, warden),
        [row("oil_flask", 5), row("oil_flask", 2), empty(), empty()]
    );
    // A stack of another level or rarity is no stack of a plain one.
    let magic = Item::new("oil_flask").with_rarity("magic");
    give_item(&mut world, warden, magic).unwrap();
    assert_eq!(give(&mut world, warden, "oil_flask", 3), 3);
    assert_eq!(
        cells(&world, warden),
        [
            row("oil_flask", 5),
            row("oil_flask", 5),
            row("oil_flask", 1),
            empty()
        ],
        "the plain stack took three, the magic flask none"
    );
    let uids: Vec<u32> = inventory(&world, warden)
        .unwrap()
        .cells()
        .flatten()
        .map(Item::uid)
        .collect();
    assert_eq!(
        uids,
        [0, 1, 2],
        "a spawn's item has none; each new one the next"
    );
    assert_eq!(world.resource::<ItemLedger>().next_uid(), 3);
    assert_eq!(give(&mut world, warden, "no_such_base", 1), 0);
}

#[test]
fn a_full_bag_gives_what_fits_and_give_item_hands_it_back() {
    let (mut world, _) = rig();
    let warden = holder(&mut world, bag(2).with_item(Item::new("ring")));
    assert_eq!(
        give(&mut world, warden, "oil_flask", 7),
        5,
        "one empty cell, a stack of 5"
    );
    assert_eq!(cells(&world, warden), [row("ring", 1), row("oil_flask", 5)]);
    let charmed = Item::new("ring").with_affix(RolledAffix::new("bright", 1, [4.0]));
    let back = give_item(&mut world, warden, charmed.clone()).unwrap_err();
    assert_eq!(back, charmed, "handed back whole");
    assert_eq!(
        cells(&world, warden),
        [row("ring", 1), row("oil_flask", 5)],
        "unchanged"
    );
    // Three flasks onto a full stack do not fit either.
    let flasks = Item::new("oil_flask").with_count(3);
    assert!(give_item(&mut world, warden, flasks).is_err());
    take(&mut world, warden, "oil_flask", 4);
    give_item(&mut world, warden, Item::new("oil_flask").with_count(3)).unwrap();
    assert_eq!(cells(&world, warden), [row("ring", 1), row("oil_flask", 4)]);
}

#[test]
fn take_counts_from_the_bag_alone() {
    let (mut world, _) = rig();
    let warden = holder(
        &mut world,
        bag(4)
            .with_item(Item::new("ring"))
            .with_item(Item::new("oil_flask").with_count(2))
            .with_item(Item::new("ring"))
            .with_item(Item::new("oil_flask").with_count(4)),
    );
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(0),
        ItemPlace::worn("ring_1"),
    )
    .unwrap();
    assert_eq!(count_of(&world, warden, "ring"), 2, "bag and worn");
    assert_eq!(take(&mut world, warden, "oil_flask", 3), 3);
    assert_eq!(
        cells(&world, warden),
        [empty(), empty(), row("ring", 1), row("oil_flask", 3)]
    );
    assert_eq!(take(&mut world, warden, "ring", 2), 1, "never a worn one");
    assert_eq!(count_of(&world, warden, "ring"), 1);
    assert!(inventory(&world, warden).unwrap().worn("ring_1").is_some());
}

#[test]
fn moving_merges_swaps_and_wears_by_slot_kind() {
    let (mut world, _) = rig();
    let warden = holder(
        &mut world,
        bag(4)
            .with_item(Item::new("oil_flask").with_count(4))
            .with_item(Item::new("oil_flask").with_count(3))
            .with_item(Item::new("ring"))
            .with_item(Item::new("wand")),
    );
    move_item(&mut world, warden, ItemPlace::Cell(1), ItemPlace::Cell(0)).unwrap();
    assert_eq!(
        cells(&world, warden),
        [
            row("oil_flask", 5),
            row("oil_flask", 2),
            row("ring", 1),
            row("wand", 1)
        ],
        "merged up to the stack, the rest left"
    );
    move_item(&mut world, warden, ItemPlace::Cell(2), ItemPlace::Cell(0)).unwrap();
    assert_eq!(
        cells(&world, warden)[..3],
        [row("ring", 1), row("oil_flask", 2), row("oil_flask", 5)],
        "swapped"
    );
    assert_eq!(
        move_item(
            &mut world,
            warden,
            ItemPlace::Cell(0),
            ItemPlace::worn("head")
        ),
        Err(ItemRefusal::WrongSlot)
    );
    assert_eq!(
        move_item(
            &mut world,
            warden,
            ItemPlace::Cell(1),
            ItemPlace::worn("ring_1")
        ),
        Err(ItemRefusal::WrongSlot),
        "a flask is worn nowhere"
    );
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(0),
        ItemPlace::worn("ring_1"),
    )
    .unwrap();
    move_item(
        &mut world,
        warden,
        ItemPlace::worn("ring_1"),
        ItemPlace::worn("ring_2"),
    )
    .unwrap();
    let bag = inventory(&world, warden).unwrap();
    assert!(
        bag.worn("ring_1").is_none() && bag.worn("ring_2").is_some(),
        "slot to slot of one kind"
    );
    assert_eq!(
        move_item(
            &mut world,
            warden,
            ItemPlace::worn("ring_2"),
            ItemPlace::worn("hand")
        ),
        Err(ItemRefusal::WrongSlot)
    );
    // Slot to an occupied cell swaps with an item that fits, else refuses.
    assert_eq!(
        move_item(
            &mut world,
            warden,
            ItemPlace::worn("ring_2"),
            ItemPlace::Cell(1)
        ),
        Err(ItemRefusal::WrongSlot)
    );
    move_item(
        &mut world,
        warden,
        ItemPlace::worn("ring_2"),
        ItemPlace::Cell(0),
    )
    .unwrap();
    assert!(inventory(&world, warden).unwrap().worn("ring_2").is_none());
    assert_eq!(cells(&world, warden)[0], row("ring", 1));
    assert_eq!(
        move_item(&mut world, warden, ItemPlace::Cell(5), ItemPlace::Cell(0)),
        Err(ItemRefusal::NoItem)
    );
    assert_eq!(
        move_item(&mut world, warden, ItemPlace::Cell(0), ItemPlace::Cell(9)),
        Err(ItemRefusal::NoPlace)
    );
    assert_eq!(
        move_item(
            &mut world,
            warden,
            ItemPlace::Cell(0),
            ItemPlace::worn("tail")
        ),
        Err(ItemRefusal::NoPlace)
    );
    // A saved stack of a wearable base, as data that changed leaves one.
    let stacked = holder(
        &mut world,
        Inventory::new(2)
            .with_slot("a", "ring")
            .with_item(Item::new("ring").with_count(2)),
    );
    assert_eq!(
        move_item(
            &mut world,
            stacked,
            ItemPlace::Cell(0),
            ItemPlace::worn("a")
        ),
        Err(ItemRefusal::Stacked)
    );
}

#[test]
fn requires_refuses_a_wear_the_holder_fails() {
    let (mut world, _) = rig();
    let warden = holder(&mut world, bag(2).with_item(Item::new("hood")));
    assert_eq!(
        move_item(
            &mut world,
            warden,
            ItemPlace::Cell(0),
            ItemPlace::worn("head")
        ),
        Err(ItemRefusal::Requires)
    );
    assert!(inventory(&world, warden).unwrap().worn("head").is_none());
    world.get_mut::<Counters>(warden).unwrap().set("trust", 3);
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(0),
        ItemPlace::worn("head"),
    )
    .unwrap();
    assert!(world.get::<Tags>(warden).unwrap().has("status.hooded"));
}

#[test]
fn wearing_applies_one_status_per_item_and_taking_off_ends_it() {
    let (mut world, _) = rig();
    let warden = holder(
        &mut world,
        bag(3)
            .with_item(Item::new("ring"))
            .with_item(Item::new("ring")),
    );
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(0),
        ItemPlace::worn("ring_1"),
    )
    .unwrap();
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(1),
        ItemPlace::worn("ring_2"),
    )
    .unwrap();
    assert_eq!(
        statuses(&world, warden),
        2,
        "two equal rings, two instances"
    );
    assert_eq!(damage_of(&world, warden), 14.0, "10 + 2 + 2");
    let bag = inventory(&world, warden).unwrap();
    let (one, two) = (
        bag.slot("ring_1").unwrap().status(),
        bag.slot("ring_2").unwrap().status(),
    );
    assert!(one.is_some() && two.is_some() && one != two);
    move_item(
        &mut world,
        warden,
        ItemPlace::worn("ring_1"),
        ItemPlace::Cell(2),
    )
    .unwrap();
    assert_eq!(statuses(&world, warden), 1);
    assert_eq!(damage_of(&world, warden), 12.0, "the other's line stays");
    assert!(
        world.get::<Tags>(warden).unwrap().has("status.ringed"),
        "the other still grants it"
    );
    move_item(
        &mut world,
        warden,
        ItemPlace::worn("ring_2"),
        ItemPlace::Cell(0),
    )
    .unwrap();
    assert_eq!(
        (statuses(&world, warden), damage_of(&world, warden)),
        (0, 10.0)
    );
    assert!(!world.get::<Tags>(warden).unwrap().has("status.ringed"));
}

#[test]
fn a_worn_items_lines_reach_the_holder_and_leave_with_it() {
    let (mut world, _) = rig();
    let warden = holder(
        &mut world,
        bag(2)
            .with_item(Item::new("wand"))
            .with_item(Item::new("hood")),
    );
    world.get_mut::<Counters>(warden).unwrap().set("trust", 5);
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(0),
        ItemPlace::worn("hand"),
    )
    .unwrap();
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(1),
        ItemPlace::worn("head"),
    )
    .unwrap();
    let uid = inventory(&world, warden)
        .unwrap()
        .worn("hand")
        .unwrap()
        .uid();
    assert_ne!(uid, 0, "worn: given a uid");
    let source = item_source(uid);
    type Held = (String, Option<(String, [f32; 2])>);
    let held: Vec<Held> = world
        .get::<crate::Abilities>(warden)
        .unwrap()
        .holder_lines()
        .map(|(from, line)| {
            (
                from.to_owned(),
                line.as_added()
                    .map(|(kind, range)| (kind.to_owned(), range)),
            )
        })
        .collect();
    assert_eq!(
        held,
        [(source.clone(), Some(("damage.fire".to_owned(), [1.0, 3.0])))]
    );
    let spell = [crate::foundation::lines::TagSet::Name("skill.spell")];
    let read = crate::foundation::lines::Read::new(&spell);
    let stats = world.get::<Stats>(warden).unwrap();
    assert_eq!(
        stats.read(Stats::DAMAGE, &read),
        Some(15.0),
        "the scoped line in a spell's context"
    );
    assert_eq!(stats.get(Stats::DAMAGE), Some(10.0), "and not outside it");
    let fire = Tags::new(["damage.fire"]);
    assert!((world.get::<Resist>(warden).unwrap().factor(&fire) - 0.9).abs() < 1e-6);
    move_item(
        &mut world,
        warden,
        ItemPlace::worn("hand"),
        ItemPlace::Cell(0),
    )
    .unwrap();
    move_item(
        &mut world,
        warden,
        ItemPlace::worn("head"),
        ItemPlace::Cell(1),
    )
    .unwrap();
    assert_eq!(
        world
            .get::<crate::Abilities>(warden)
            .unwrap()
            .holder_lines()
            .count(),
        0
    );
    let stats = world.get::<Stats>(warden).unwrap();
    assert_eq!(stats.read(Stats::DAMAGE, &read), Some(10.0));
    assert!((world.get::<Resist>(warden).unwrap().factor(&fire) - 1.0).abs() < 1e-6);
    assert!(!world.get::<Tags>(warden).unwrap().has("status.hooded"));
}

#[test]
fn using_spends_one_then_runs_the_bases_list_at_the_call() {
    let (mut world, _) = rig();
    let warden = holder(
        &mut world,
        bag(3)
            .with_item(Item::new("oil_flask").with_count(2))
            .with_item(Item::new("gift")),
    );
    assert!(use_item(&mut world, warden, 0));
    assert_eq!(
        world.get::<Counters>(warden).unwrap().get("oil"),
        10,
        "the list ran at the call"
    );
    assert_eq!(cells(&world, warden)[0], row("oil_flask", 1));
    // The gift's list gives a shard into the cell the spend emptied: the
    // spend came first.
    assert!(use_item(&mut world, warden, 1));
    assert_eq!(
        cells(&world, warden),
        [row("oil_flask", 1), row("shard", 1), empty()]
    );
    assert!(!use_item(&mut world, warden, 1), "a shard has no `use`");
    assert!(!use_item(&mut world, warden, 2), "an empty cell");
    assert!(use_item(&mut world, warden, 0));
    assert_eq!(cells(&world, warden)[0], empty(), "the last flask spent");
    assert_eq!(world.get::<Counters>(warden).unwrap().get("oil"), 20);
}

#[test]
fn a_potion_used_on_an_ally_runs_its_list_with_the_ally_as_the_subject() {
    let (mut world, _) = rig();
    let warden = holder(
        &mut world,
        bag(2).with_item(Item::new("potion").with_count(2)),
    );
    let ally = world.spawn_with((Tags::new(["team.ally"]), Counters::new()));
    assert!(use_item_on(&mut world, warden, 0, ally));
    assert_eq!(
        world.get::<Counters>(ally).unwrap().get("healed"),
        1,
        "the ally is the subject"
    );
    assert_eq!(
        world.get::<Counters>(warden).unwrap().get("healed"),
        0,
        "not its holder"
    );
    assert_eq!(cells(&world, warden)[0], row("potion", 1), "one spent");
    world.despawn(ally);
    assert!(
        !use_item_on(&mut world, warden, 0, ally),
        "a target not alive"
    );
    assert_eq!(cells(&world, warden)[0], row("potion", 1), "none spent");
    // Used on its holder, it is `use_item`.
    assert!(use_item_on(&mut world, warden, 0, warden));
    assert_eq!(world.get::<Counters>(warden).unwrap().get("healed"), 1);
}

/// A rig with the prefab `holder`, its inventory from `inventory`, and a
/// named source to spawn it from, so a save holds the holder.
fn saved_rig(inventory: &Value) -> (World, Schedule, Entity) {
    saved_rig_without(inventory, &[])
}

/// [`saved_rig`] with the bases `skip` names left out of the data.
fn saved_rig_without(inventory: &Value, skip: &[&str]) -> (World, Schedule, Entity) {
    let (mut world, schedule) = rig_without(skip);
    let mut prefabs = PrefabRegistry::new();
    prefabs.insert(
        "holder".to_owned(),
        serde_json::from_value(json!({ "components": {
            "inventory": inventory,
            "stats": { "damage": 10 },
            "resist": {},
            "tags": ["team.player"],
            "counters": { "trust": 5 }
        } }))
        .unwrap(),
    );
    world.insert_resource(prefabs);
    let source = world.spawn_with((tungsten_core::Transform::default(),));
    saves::name(&mut world, source, None, "source").unwrap();
    (world, schedule, source)
}

fn spawn_holder(world: &mut World, source: Entity) -> Entity {
    let prepared = world.prepare_prefab("holder", [8.0, 8.0]).unwrap();
    world
        .resource_mut::<CommandBuffer>()
        .spawn_prefab_from(source, prepared);
    let mut commands = world.remove_resource::<CommandBuffer>().unwrap();
    world.flush_reusing(&mut commands);
    world.insert_resource(commands);
    world
        .query::<(Entity, &Inventory)>()
        .map(|(entity, _)| entity)
        .next()
        .unwrap()
}

fn restored_holder(world: &World) -> Entity {
    let held: Vec<Entity> = world
        .query::<(Entity, &Inventory)>()
        .map(|(entity, _)| entity)
        .collect();
    assert_eq!(held.len(), 1, "the holder spawned again");
    held[0]
}

const BAG: &str = r#"{ "cells": 4, "slots": { "ring_1": "ring", "ring_2": "ring", "head": "hood" },
                       "items": ["ring", { "base": "ring", "affixes": [{ "id": "bright", "tier": 1, "values": [3] }] }, "oil_flask"] }"#;

#[test]
fn the_inventory_saves_its_slots_empty_or_worn_and_restores() {
    let bag: Value = serde_json::from_str(BAG).unwrap();
    let (mut world, _, source) = saved_rig(&bag);
    let warden = spawn_holder(&mut world, source);
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(1),
        ItemPlace::worn("ring_2"),
    )
    .unwrap();
    give(&mut world, warden, "oil_flask", 2);
    let saved = capture_world(&world).unwrap();
    let before = inventory(&world, warden).unwrap().clone();
    let form = serde_json::to_value(&before).unwrap();
    assert_eq!(form["cells"].as_array().unwrap().len(), 4);
    assert_eq!(
        form["slots"]["ring_1"],
        json!({ "kind": "ring", "item": null, "status": null }),
        "an empty slot kept"
    );
    assert_eq!(form["slots"]["head"]["kind"], "hood");
    assert!(form["slots"]["ring_2"]["status"].is_u64());

    let (mut fresh, ..) = saved_rig(&bag);
    restore_world(&mut fresh, &saved, &(Restored,)).unwrap();
    let warden = restored_holder(&fresh);
    let after = inventory(&fresh, warden).unwrap();
    assert_eq!(
        after.cells().collect::<Vec<_>>(),
        before.cells().collect::<Vec<_>>()
    );
    assert_eq!(after.worn("ring_2"), before.worn("ring_2"));
    assert_eq!(after.slot("ring_1").unwrap().kind(), "ring");
    assert_eq!(
        damage_of(&fresh, warden),
        15.0,
        "10 + the ring's 2 + bright's 3"
    );
    assert_eq!(
        fresh.resource::<ItemLedger>(),
        world.resource::<ItemLedger>(),
        "the ledger saved"
    );
}

#[test]
fn a_restore_rebuilds_worn_statuses_quietly_from_the_data_of_the_day() {
    let bag: Value = serde_json::from_str(BAG).unwrap();
    let (mut world, mut schedule, source) = saved_rig(&bag);
    let warden = spawn_holder(&mut world, source);
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(1),
        ItemPlace::worn("ring_2"),
    )
    .unwrap();
    assert_eq!(damage_of(&world, warden), 15.0);
    let saved = capture_world(&world).unwrap();

    let (mut fresh, ..) = saved_rig(&bag);
    // The data of the day doubles bright's line.
    let doubled: Affix = serde_json::from_value(json!({
        "kind": "prefix", "group": "bright",
        "tiers": [{ "level": 1, "weight": 1, "values": [[1, 10]] }],
        "lines": [{ "stat": "damage", "modifier": { "add": { "value": 0, "times": 2 } } }]
    }))
    .unwrap();
    fresh
        .resource_mut::<DataRegistry>()
        .insert("bright", doubled);
    step(&mut fresh, &mut schedule);
    restore_world(&mut fresh, &saved, &(Restored,)).unwrap();
    let warden = restored_holder(&fresh);
    assert_eq!(
        damage_of(&fresh, warden),
        18.0,
        "10 + 2 + bright's 3 doubled: applied as shown"
    );
    let worn = inventory(&fresh, warden)
        .unwrap()
        .worn("ring_2")
        .unwrap()
        .clone();
    let texts: Vec<String> = built(&fresh, &worn)
        .unwrap()
        .lines()
        .iter()
        .map(|line| line.text().to_owned())
        .collect();
    assert_eq!(texts, ["+2 damage", "+6 damage"], "shown doubled");
    assert_eq!(
        statuses(&fresh, warden),
        1,
        "the saved instance ended, one rebuilt"
    );
    assert_eq!(
        fresh.resource::<EventQueue<EffectApplied>>().iter().count(),
        0,
        "no cue hears the rebuild"
    );
    assert_eq!(
        fresh.resource::<EventQueue<EffectExpired>>().iter().count(),
        0
    );
    let status = inventory(&fresh, warden)
        .unwrap()
        .slot("ring_2")
        .unwrap()
        .status();
    let serials: Vec<u64> = fresh
        .get::<Effects>(warden)
        .unwrap()
        .instances()
        .map(crate::damage::effects::EffectInstance::serial)
        .collect();
    assert_eq!(
        serials,
        [status.unwrap()],
        "the slot names the rebuilt instance"
    );
}

#[test]
fn a_worn_item_whose_base_is_gone_stays_worn_and_applies_nothing() {
    let bag: Value = serde_json::from_str(BAG).unwrap();
    let (mut world, _, source) = saved_rig(&bag);
    let warden = spawn_holder(&mut world, source);
    move_item(
        &mut world,
        warden,
        ItemPlace::Cell(0),
        ItemPlace::worn("ring_1"),
    )
    .unwrap();
    assert!(world.get::<Tags>(warden).unwrap().has("status.ringed"));
    let saved = capture_world(&world).unwrap();

    // The data of the day has no `ring`; the prefab the restore spawns again
    // names none, as a load that checks it would hold.
    let plain = json!({ "cells": 4, "slots": { "ring_1": "ring" } });
    let (mut fresh, ..) = saved_rig_without(&plain, &["ring"]);
    restore_world(&mut fresh, &saved, &(Restored,)).unwrap();
    let warden = restored_holder(&fresh);
    let bag = inventory(&fresh, warden).unwrap();
    assert_eq!(
        bag.worn("ring_1").map(Item::base),
        Some("ring"),
        "still worn"
    );
    assert_eq!(
        bag.slot("ring_1").unwrap().status(),
        None,
        "applying nothing"
    );
    assert_eq!(statuses(&fresh, warden), 0);
    assert_eq!(damage_of(&fresh, warden), 10.0);
    assert!(!fresh.get::<Tags>(warden).unwrap().has("status.ringed"));
}

#[test]
fn a_world_without_an_inventory_queries_nothing() {
    let (mut world, mut schedule) = rig();
    let walker = world.spawn_with((Stats::new(), Counters::new()));
    for _ in 0..3 {
        step(&mut world, &mut schedule);
    }
    let saved = capture_world(&world).unwrap();
    restore_world(&mut world, &saved, &(Restored,)).unwrap();
    assert!(!world.has_any::<Inventory>());
    assert!(!world.has_resource::<ItemLedger>(), "nothing took a uid");
    assert_eq!(give(&mut world, walker, "oil_flask", 1), 0, "no bag");
    assert!(!use_item(&mut world, walker, 0));
    assert_eq!(
        move_item(&mut world, walker, ItemPlace::Cell(0), ItemPlace::Cell(1)),
        Err(ItemRefusal::NoItem)
    );
}
