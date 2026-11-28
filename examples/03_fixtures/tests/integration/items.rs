//! The `items` scene on the headless harness (redo 8.1's second plan): the
//! fixtures' manifest loads beside the kit, the scene's inline entries
//! build, its screen files check with the item screens' names, and the
//! demo's path runs: the flasks taken, the hood bought from the chandler,
//! its tooltip read and the hood worn.

use std::path::PathBuf;

use example_03_fixtures::items::{self, ItemsScene};
use tungsten::App;
use tungsten::core::ui::{UiBindings, UiKey, UiScreenRegistry, UiTree, WidgetId};
use tungsten::core::{Config, DataRegistry, Entity, KeyCode};
use tungsten::items::{INVENTORY_SCREEN, SHOP_SCREEN};
use tungsten::kit::{
    Affix, Counters, GroundItem, Interactable, Interactor, Inventory, Item, ItemBase, Shop,
    ShopVisit, Tags, built, visit,
};
use tungsten::screens::popup;
use tungsten::testing::UiHarness;

/// The scene on the harness: configured as `run` does, the fixtures'
/// manifest loaded, the room set up, its first frames settled.
fn rig() -> UiHarness {
    let mut app = App::new(Config::default()).expect("App::new failed");
    items::configure(&mut app);
    let mut harness = UiHarness::new(app);
    harness
        .harness_mut()
        .load_manifest(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/manifest.json"))
        .expect("the fixtures' manifest loads beside the kit");
    items::setup(harness.world_mut());
    harness.step(12);
    harness
}

fn top(harness: &UiHarness) -> Option<&'static str> {
    harness
        .world()
        .resource::<tungsten::StateStack>()
        .active_id()
}

fn scene(harness: &UiHarness) -> ItemsScene {
    *harness.world().resource::<ItemsScene>()
}

fn bag(harness: &UiHarness) -> &Inventory {
    harness
        .world()
        .get::<Inventory>(scene(harness).warden)
        .expect("the warden's bag")
}

fn coins(harness: &UiHarness) -> i64 {
    harness.world().resource::<Counters>().get("coins")
}

fn keyed(harness: &UiHarness, key: &str) -> WidgetId {
    harness
        .find_key(UiKey::new(key))
        .unwrap_or_else(|| panic!("no widget keyed {key}"))
}

fn tap(harness: &mut UiHarness, key: KeyCode) {
    harness.key_down(key);
    harness.step(1);
    harness.key_up(key);
    harness.step(1);
}

/// Holds `action` until the warden's focus is `target`, at most 600 frames.
fn walk_until(harness: &mut UiHarness, action: &str, target: Entity) {
    let focus = |harness: &UiHarness| {
        harness
            .world()
            .get::<Interactor>(scene(harness).warden)
            .and_then(Interactor::focus)
    };
    harness.harness_mut().press_action(action);
    for _ in 0..600 {
        if focus(harness) == Some(target) {
            break;
        }
        harness.step(1);
    }
    harness.harness_mut().release_action(action);
    harness.step(2);
    assert_eq!(focus(harness), Some(target), "{action} reached its target");
}

/// Drags `from`'s widget onto `to`'s and lets the drop route.
fn drag(harness: &mut UiHarness, from: &str, to: &str) {
    let (from, to) = (keyed(harness, from), keyed(harness, to));
    harness.drag(from, to);
    harness.step(2);
}

#[test]
fn the_items_scene_loads_and_its_screens_check() {
    let harness = rig();
    assert_eq!(top(&harness), Some(items::PLAY));
    let world = harness.world();
    let data = world.resource::<DataRegistry>();
    for id in ["oil_flask", "oilskin_hood", "brass_charm", "ashwood_wand"] {
        assert!(data.get::<ItemBase>(id).is_some(), "base {id}");
    }
    for id in ["fine", "searing", "quickened"] {
        assert!(data.get::<Affix>(id).is_some(), "affix {id}");
    }
    assert!(data.get::<Shop>(items::SHOP).is_some());
    let scene = scene(&harness);
    assert!(world.get::<Interactor>(scene.warden).is_some());
    assert!(world.get::<Interactable>(scene.chandler).is_some());
    let slots: Vec<&str> = bag(&harness).slots().map(|(name, _)| name).collect();
    assert_eq!(slots.len(), 5, "{slots:?}");
    assert_eq!(bag(&harness).len(), 24);

    // Each ground item builds from the inline entries, in its rarity.
    for (entity, rarity, affixes) in [
        (scene.flasks, "normal", 0),
        (scene.charm, "magic", 1),
        (scene.wand, "rare", 2),
    ] {
        let item = world
            .get::<GroundItem>(entity)
            .expect("a ground item")
            .item()
            .clone();
        let made = built(world, &item).expect("it builds");
        assert!(made.known(), "{}: its base and affixes known", item.base());
        assert_eq!(made.rarity(), rarity);
        assert_eq!(item.affixes().len(), affixes);
    }
    assert_eq!(
        world
            .get::<GroundItem>(scene.flasks)
            .unwrap()
            .item()
            .count(),
        3,
        "a stack"
    );

    // The screen files check with the item screens' names and the
    // theme's text styles, as `tungsten check` holds them.
    let mut names = world.resource::<UiBindings>().names();
    let styles = world.resource::<UiTree>().theme().names.text_styles.keys();
    names.styles.extend(styles.cloned());
    let files = world.resource::<UiScreenRegistry>();
    for id in files.ids() {
        names.screens.insert(id.to_owned());
        let file = files.get(id).expect("listed");
        if file.popup_refusal().is_none() {
            names.popups.insert(id.to_owned());
        }
        if file.tooltip_refusal().is_none() {
            names.tooltips.insert(id.to_owned());
        }
    }
    for id in [INVENTORY_SCREEN, SHOP_SCREEN, "item_tooltip", "cell_menu"] {
        let file = files
            .get(id)
            .unwrap_or_else(|| panic!("the manifest lists `{id}`"));
        assert!(
            file.check(&names).is_empty(),
            "{id}: {:?}",
            file.check(&names)
        );
    }
}

#[test]
fn the_warden_takes_the_flasks_buys_and_wears_the_hood() {
    let mut harness = rig();
    let ItemsScene {
        warden, chandler, ..
    } = scene(&harness);

    // The walk to the chandler takes the flasks on the way.
    walk_until(&mut harness, "move_right", chandler);
    let flasks = bag(&harness)
        .cell(0)
        .map(|item| (item.base(), item.count()));
    assert_eq!(flasks, Some(("oil_flask", 3)), "the stack taken");

    // Trade: the shop opens over the visit; the hood bought by a drag.
    harness.tap_action(items::INTERACT);
    harness.step(12);
    assert_eq!(
        visit(harness.world()).map(ShopVisit::shop),
        Some(items::SHOP)
    );
    assert_eq!(top(&harness), Some(SHOP_SCREEN));
    drag(&mut harness, "stock/oilskin_hood", "cells/1");
    let hood = bag(&harness).cell(1).map(Item::base);
    assert_eq!(hood, Some("oilskin_hood"), "bought");
    assert_eq!(coins(&harness), items::COINS - 12, "at its price");
    tap(&mut harness, KeyCode::Escape);
    harness.step(12);
    assert_eq!(top(&harness), Some(items::PLAY), "the shop closed");
    assert!(visit(harness.world()).is_none(), "and its visit ended");

    // The bag: the hood's tooltip, then the hood dragged onto the lantern.
    harness.tap_action(items::INVENTORY);
    harness.step(12);
    assert_eq!(top(&harness), Some(INVENTORY_SCREEN));
    let cell = keyed(&harness, "cells/1");
    harness.hover(cell);
    harness.step(1);
    harness.wait(popup::TooltipSettings::default().delay);
    harness.step(2);
    let tooltip = popup::shown_tooltip(harness.world()).expect("the hood's tooltip");
    let name = harness
        .tree()
        .find_key(tooltip, UiKey::new("name"))
        .expect("its name");
    assert_eq!(harness.tree().plain_text(name), "Oilskin Hood");
    drag(&mut harness, "cells/1", "worn/lantern");
    let worn = bag(&harness).worn("lantern").map(Item::base);
    assert_eq!(worn, Some("oilskin_hood"), "worn");
    let tags = harness
        .world()
        .get::<Tags>(warden)
        .expect("the warden's tags");
    assert!(tags.has("status.hooded"), "the hood's grant");
    tap(&mut harness, KeyCode::Escape);
    harness.step(12);
    assert_eq!(top(&harness), Some(items::PLAY), "the bag closed");
}
