use glam::Vec2;
use serde_json::{Value, json};
use tungsten_core::save::{capture_world, restore_world};
use tungsten_core::{CommandBuffer, Schedule, Time};

use super::*;
use crate::foundation::trace::{KitTrace, TraceKind};
use crate::items::inventory::{Inventory, count_of};
use crate::react::cues::{CueAction, run_list};
use crate::{KitPlugins, TagsPlugin};

/// What a restore's spawns take.
#[derive(Debug, Clone, Copy)]
struct Restored;

fn chandlery() -> Value {
    json!({
        "name": "Chandlery",
        "stock": [
            { "item": "oil_flask" },
            { "item": "hood", "count": 2,
              "when": { "counter": { "name": "day", "min": 4, "target": "game" } } },
            { "item": "wand", "price": 0 }
        ],
        "buys": { "any": ["item.flask", "item.hood"] },
        "sell": 0.5,
        "scale": { "counter": "trust", "per": -0.1, "min": 0.5, "max": 1.5 }
    })
}

/// A World with the kit, the bases and the chandlery, the game's 50 coins
/// and day 3, a keeper with no trust and a customer with a bag of `cells`.
fn rig(cells: u32) -> (World, Entity, Entity) {
    let mut world = World::new();
    world.insert_resource(Time::new());
    world.insert_resource(CommandBuffer::new());
    world.insert_resource(KitTrace::default());
    let mut schedule = Schedule::new();
    KitPlugins::set()
        .without::<TagsPlugin>()
        .with(TagsPlugin::new().with_tags(["item.flask", "item.hood", "item.weapon"]))
        .build(&mut schedule, &mut world);
    schedule.resolve().unwrap();
    for (id, base) in [
        (
            "oil_flask",
            json!({ "name": "Oil Flask", "icon": "flask", "tags": ["item.flask"], "stack": 5, "price": 3 }),
        ),
        (
            "hood",
            json!({ "name": "Hood", "icon": "hood", "tags": ["item.hood"], "slot": "hood", "price": 12 }),
        ),
        (
            "wand",
            json!({ "name": "Wand", "icon": "wand", "tags": ["item.weapon"], "price": 40 }),
        ),
    ] {
        let base: ItemBase = serde_json::from_value(base).unwrap();
        world.resource_mut::<DataRegistry>().insert(id, base);
    }
    let shop: Shop = serde_json::from_value(chandlery()).unwrap();
    world
        .resource_mut::<DataRegistry>()
        .insert("chandlery", shop);
    world.insert_resource(Counters::new().with("coins", 50).with("day", 3));
    let keeper = world.spawn_with((Counters::new(),));
    let customer = world.spawn_with((Inventory::new(cells), Counters::new()));
    (world, keeper, customer)
}

fn coins(world: &World) -> i64 {
    world.resource::<Counters>().get("coins")
}

fn open(world: &mut World, keeper: Entity, customer: Entity) {
    assert!(open_shop(world, "chandlery", keeper, customer));
}

#[test]
fn an_entry_shows_while_its_condition_holds() {
    let (mut world, keeper, customer) = rig(4);
    open(&mut world, keeper, customer);
    assert_eq!(price_of(&world, "hood"), None, "day 3: hidden");
    let bases = |world: &World| -> Vec<String> {
        shop_stock(world)
            .into_iter()
            .map(|(base, _, _)| base)
            .collect()
    };
    assert_eq!(bases(&world), ["oil_flask", "wand"]);
    world.resource_mut::<Counters>().set("day", 4);
    assert_eq!(price_of(&world, "hood"), Some(12));
    assert_eq!(bases(&world), ["oil_flask", "hood", "wand"]);
    assert_eq!(
        shop_stock(&world)[1],
        ("hood".to_owned(), 12, Some(2)),
        "its price now and how many are left"
    );
    assert_eq!(price_of(&world, "lantern"), None, "not stocked");
}

#[test]
fn buying_pays_the_scaled_price_and_takes_stock() {
    let (mut world, keeper, customer) = rig(4);
    world.resource_mut::<Counters>().set("day", 4);
    world.get_mut::<Counters>(keeper).unwrap().set("trust", 2);
    open(&mut world, keeper, customer);
    assert_eq!(price_of(&world, "hood"), Some(10), "12 × 0.8, rounded");
    buy(&mut world, "hood").unwrap();
    assert_eq!(coins(&world), 40);
    assert_eq!(count_of(&world, customer, "hood"), 1);
    assert_eq!(world.resource::<ItemLedger>().sold("chandlery", "hood"), 1);
    assert_eq!(shop_stock(&world)[1].2, Some(1), "one left");
    assert_eq!(
        price_of(&world, "oil_flask"),
        Some(2),
        "3 × 0.8 = 2.4, rounded"
    );
    buy(&mut world, "oil_flask").unwrap();
    buy(&mut world, "oil_flask").unwrap();
    assert_eq!(
        (coins(&world), count_of(&world, customer, "oil_flask")),
        (36, 2)
    );
    assert_eq!(price_of(&world, "wand"), Some(0), "a price of 0 is free");
    buy(&mut world, "wand").unwrap();
    assert_eq!(coins(&world), 36);
}

#[test]
fn buying_is_refused_when_poor_full_sold_out_or_hidden() {
    let (mut world, keeper, customer) = rig(2);
    assert_eq!(buy(&mut world, "oil_flask"), Err(ShopRefusal::Closed));
    open(&mut world, keeper, customer);
    assert_eq!(buy(&mut world, "hood"), Err(ShopRefusal::Hidden), "day 3");
    assert_eq!(
        buy(&mut world, "lantern"),
        Err(ShopRefusal::Hidden),
        "not stocked"
    );
    world.resource_mut::<Counters>().set("day", 4);
    buy(&mut world, "hood").unwrap();
    buy(&mut world, "hood").unwrap();
    assert_eq!(
        buy(&mut world, "hood"),
        Err(ShopRefusal::SoldOut),
        "two stocked"
    );
    assert_eq!(
        buy(&mut world, "wand"),
        Err(ShopRefusal::Full),
        "both cells hold a hood"
    );
    let before = coins(&world);
    assert_eq!(before, 26);
    world.resource_mut::<Counters>().set("coins", 2);
    crate::take(&mut world, customer, "hood", 1);
    assert_eq!(
        buy(&mut world, "oil_flask"),
        Err(ShopRefusal::Poor),
        "2 coins for a price of 3"
    );
    assert_eq!(coins(&world), 2, "a refused buy pays nothing");
    assert_eq!(
        count_of(&world, customer, "oil_flask"),
        0,
        "and gives nothing"
    );
}

#[test]
fn a_sale_pays_its_share_of_the_price_now_and_no_loop_gains_coin() {
    let (mut world, keeper, customer) = rig(4);
    world.resource_mut::<Counters>().set("day", 4);
    open(&mut world, keeper, customer);
    buy(&mut world, "hood").unwrap();
    assert_eq!(coins(&world), 38);
    assert_eq!(sell(&mut world, 0, 1), Ok(6), "floor(12 × 0.5)");
    assert_eq!((coins(&world), count_of(&world, customer, "hood")), (44, 0));
    // The factor at its least: still no loop gains.
    world.get_mut::<Counters>(keeper).unwrap().set("trust", 9);
    for _ in 0..5 {
        let before = coins(&world);
        buy(&mut world, "oil_flask").unwrap();
        sell(&mut world, 0, 1).unwrap();
        assert!(coins(&world) < before, "{} after {before}", coins(&world));
    }
    buy(&mut world, "wand").unwrap();
    assert_eq!(
        sell(&mut world, 0, 1),
        Err(ShopRefusal::NotBought),
        "it buys no weapon"
    );
    assert_eq!(
        sell(&mut world, 3, 1),
        Err(ShopRefusal::NotBought),
        "an empty cell"
    );
    // A stack sells what it holds, at most.
    for _ in 0..3 {
        buy(&mut world, "oil_flask").unwrap();
    }
    let at = inventory(&world, customer)
        .unwrap()
        .cells()
        .position(|cell| cell.is_some_and(|item| item.base() == "oil_flask"))
        .unwrap();
    assert_eq!(
        sell(&mut world, at, 9),
        Ok(3),
        "three at floor(2 × 0.5) = 1"
    );
}

#[test]
fn the_price_scale_follows_its_counter_within_its_bounds() {
    let (mut world, keeper, customer) = rig(4);
    world.resource_mut::<Counters>().set("day", 4);
    open(&mut world, keeper, customer);
    for (trust, price) in [(0, 12), (2, 10), (9, 6), (-10, 18), (-3, 16)] {
        world
            .get_mut::<Counters>(keeper)
            .unwrap()
            .set("trust", trust);
        assert_eq!(price_of(&world, "hood"), Some(price), "trust {trust}");
    }
    // A scale on the customer's counter, then the game's.
    let mut shop: Value = chandlery();
    shop["scale"] = json!({ "counter": "fame", "target": "other", "per": 0.5, "min": 1, "max": 2 });
    world.resource_mut::<DataRegistry>().insert(
        "chandlery",
        serde_json::from_value::<Shop>(shop.clone()).unwrap(),
    );
    world.get_mut::<Counters>(customer).unwrap().set("fame", 1);
    assert_eq!(price_of(&world, "hood"), Some(18));
    shop["scale"]["target"] = "game".into();
    world.resource_mut::<DataRegistry>().insert(
        "chandlery",
        serde_json::from_value::<Shop>(shop.clone()).unwrap(),
    );
    assert_eq!(price_of(&world, "hood"), Some(12), "the game holds no fame");
    for (scale, why) in [
        (
            json!({ "counter": "trust", "per": 1, "min": 0, "max": 1 }),
            "min: in (0, 10]",
        ),
        (
            json!({ "counter": "trust", "per": 1, "min": 2, "max": 1 }),
            "exceeds `max`",
        ),
        (
            json!({ "counter": "trust", "per": 1, "min": 1, "max": 11 }),
            "max: in (0, 10]",
        ),
    ] {
        shop["scale"] = scale.clone();
        let error = serde_json::from_value::<Shop>(shop.clone())
            .unwrap_err()
            .to_string();
        assert!(error.contains(why), "{scale}: {error}");
    }
}

#[test]
fn sold_counts_save_with_the_ledger() {
    let (mut world, keeper, customer) = rig(4);
    world.resource_mut::<Counters>().set("day", 4);
    open(&mut world, keeper, customer);
    buy(&mut world, "hood").unwrap();
    let saved = capture_world(&world).unwrap();
    let (mut fresh, keeper, customer) = rig(4);
    restore_world(&mut fresh, &saved, &(Restored,)).unwrap();
    assert_eq!(fresh.resource::<ItemLedger>().sold("chandlery", "hood"), 1);
    fresh.resource_mut::<Counters>().set("day", 4);
    open(&mut fresh, keeper, customer);
    assert_eq!(shop_stock(&fresh)[1].2, Some(1), "one left after the load");
}

#[test]
fn the_shop_action_opens_one_visit_and_a_restore_ends_it() {
    let (mut world, keeper, customer) = rig(4);
    run_list(
        &mut world,
        &[CueAction::shop("chandlery")],
        Some(keeper),
        Some(customer),
        Vec2::ZERO,
    );
    let open_now = visit(&world).unwrap();
    assert_eq!(
        (open_now.shop(), open_now.keeper(), open_now.customer()),
        ("chandlery", keeper, customer)
    );
    assert_eq!(world.resource::<EventQueue<ShopOpened>>().iter().count(), 1);
    run_list(
        &mut world,
        &[CueAction::shop("chandlery")],
        Some(keeper),
        Some(customer),
        Vec2::ZERO,
    );
    let skipped = world
        .resource::<KitTrace>()
        .iter()
        .filter(|entry| matches!(entry.kind, TraceKind::Skipped { action: "shop", .. }))
        .count();
    assert_eq!(skipped, 1, "one visit at a time");
    assert_eq!(world.resource::<EventQueue<ShopOpened>>().iter().count(), 1);
    let saved = capture_world(&world).unwrap();
    restore_world(&mut world, &saved, &(Restored,)).unwrap();
    assert!(
        visit(&world).is_none(),
        "a visit is not saved; the restore ended it"
    );
    assert_eq!(world.resource::<EventQueue<ShopClosed>>().iter().count(), 1);
    assert!(!end_shop(&mut world), "none open");
    // The load checks the shop and an other entity, the customer.
    let missing = crate::react::cues::check_list(&world, "do", &[CueAction::shop("market")], true)
        .unwrap_err();
    assert_eq!(missing.field, "do.0.shop");
    let missing =
        crate::react::cues::check_list(&world, "do", &[CueAction::shop("chandlery")], false)
            .unwrap_err();
    assert_eq!(
        (missing.field.as_str(), missing.kind),
        ("do.0.shop", "cue event with an other entity, the customer")
    );
}
