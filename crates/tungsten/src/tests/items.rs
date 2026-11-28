use std::path::PathBuf;

use glam::Vec2;
use tungsten_core::assets::UvRect;
use tungsten_core::assets::manifest::FilterMode;
use tungsten_core::ui::{UiKey, UiScreenRegistry, WidgetId, WidgetKind};
use tungsten_core::{AssetRegistry, Config, KeyCode, TextureHandle, Transform, World};

use super::*;
use crate::App;
use crate::kit::{
    Counters, Dialogue, GroundItem, Inventory, Item, KitPlugins, RolledAffix, built, count_of,
    give, give_item, inventory, move_item, open_shop, talk, visit,
};
use crate::screens::{ScreensPlugin, Toasts, popup};
use crate::state::StateStack;
use crate::testing::UiHarness;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/items")
}

/// Registers the 16 × 16 sprites the bases' icons name, so their images
/// show and the load's icon check passes.
fn sprites(world: &mut World) {
    let assets = world.resource_mut::<AssetRegistry>();
    for name in ["flask", "hood", "charm"] {
        assets.register_sprite(
            name.to_owned(),
            FilterMode::Nearest,
            16,
            16,
            PathBuf::from(format!("{name}.png")),
            TextureHandle(1),
            UvRect {
                min: [0.0, 0.0],
                max: [1.0, 1.0],
            },
            None,
            None,
            None,
        );
    }
}

/// A harness with the kit, the screens and the item screens, the items
/// fixture loaded and 50 of the game's coins; a warden with a bag of 8 and
/// a hood and a charm slot, and a keeper.
fn rig() -> (UiHarness, Entity, Entity) {
    let mut app = App::new(Config::default()).expect("App::new failed");
    app.add_plugins(KitPlugins::set());
    app.add_plugin(ScreensPlugin);
    app.add_plugin(ItemsUiPlugin);
    sprites(app.world_mut());
    let mut ui = UiHarness::new(app);
    ui.harness_mut()
        .load_manifest(fixture().join("manifest.json"))
        .expect("the items fixture loads");
    let world = ui.world_mut();
    world.insert_resource(Counters::new().with("coins", 50));
    let bag = Inventory::new(8)
        .with_slot("hood", "hood")
        .with_slot("charm", "charm");
    let warden = world.spawn_with((Transform::from_position(Vec2::new(64.0, 64.0)), bag));
    let keeper = world.spawn_with((Transform::from_position(Vec2::new(96.0, 64.0)),));
    (ui, warden, keeper)
}

/// The inventory opened over the warden's bag, its slide in finished.
fn opened(ui: &mut UiHarness, warden: Entity) {
    assert!(
        open_inventory(ui.world_mut(), warden),
        "the inventory opens"
    );
    ui.step(12);
    assert_eq!(top(ui), Some(INVENTORY_SCREEN));
}

fn top(ui: &UiHarness) -> Option<StateId> {
    ui.world().resource::<StateStack>().active_id()
}

fn keyed(ui: &UiHarness, key: &str) -> WidgetId {
    ui.find_key(UiKey::new(key))
        .unwrap_or_else(|| panic!("no widget keyed {key}"))
}

/// Every label's text in `widget`'s subtree, depth first.
fn texts(ui: &UiHarness, widget: WidgetId) -> Vec<String> {
    let tree = ui.tree();
    let mut stack = vec![widget];
    let mut found = Vec::new();
    while let Some(id) = stack.pop() {
        if tree.kind(id) == Some(WidgetKind::Label) {
            found.push(tree.plain_text(id));
        }
        stack.extend(tree.children(id).iter().rev());
    }
    found
}

fn toasts(ui: &UiHarness) -> Vec<String> {
    ui.world().resource::<Toasts>().texts(ui.tree())
}

fn coins(ui: &UiHarness) -> i64 {
    ui.world().resource::<Counters>().get("coins")
}

fn bag(ui: &UiHarness, warden: Entity) -> &Inventory {
    inventory(ui.world(), warden).expect("the warden's bag")
}

fn tap(ui: &mut UiHarness, key: KeyCode) {
    ui.key_down(key);
    ui.step(1);
    ui.key_up(key);
    ui.step(1);
}

/// Accept on `key`'s widget from the keyboard.
fn accept(ui: &mut UiHarness, key: &str) {
    let widget = keyed(ui, key);
    assert!(ui.focus(widget), "{key} takes focus");
    tap(ui, KeyCode::Enter);
}

/// Drags `from`'s widget onto `to`'s and lets the drop route.
fn drag(ui: &mut UiHarness, from: &str, to: &str) {
    let (from, to) = (keyed(ui, from), keyed(ui, to));
    ui.drag(from, to);
    ui.step(2);
}

/// Clicks `key`'s widget and lets its action and any popup settle.
fn click(ui: &mut UiHarness, key: &str) {
    let widget = keyed(ui, key);
    ui.click(widget);
    ui.step(12);
}

/// A magic brass charm with the `fine` prefix.
fn fine_charm() -> Item {
    Item::new("brass_charm")
        .with_rarity("magic")
        .with_affix(RolledAffix::new("fine", 1, [8.0]))
}

#[test]
fn the_inventory_binds_cells_worn_and_coins() {
    let (mut ui, warden, _) = rig();
    let world = ui.world_mut();
    give(world, warden, "oil_flask", 3);
    give(world, warden, "oilskin_hood", 1);
    give_item(world, warden, fine_charm()).expect("room");
    move_item(world, warden, ItemPlace::Cell(2), ItemPlace::worn("charm")).expect("worn");
    opened(&mut ui, warden);

    let cells: Vec<WidgetId> = (0..8)
        .map(|cell| keyed(&ui, &format!("cells/{cell}")))
        .collect();
    assert_eq!(
        texts(&ui, cells[0]),
        ["Oil Flask", "3"],
        "a stack shows its count"
    );
    assert_eq!(texts(&ui, cells[1]), ["Oilskin Hood", ""], "one shows none");
    assert_eq!(texts(&ui, cells[2]), ["", ""], "an empty cell");
    let tree = ui.tree();
    assert_eq!(tree.text_style(cells[0]), Some("rarity.normal"));
    let image = |cell: WidgetId| {
        let children = tree.children(cell).iter().copied();
        let mut images = children.filter(|id| tree.kind(*id) == Some(WidgetKind::Image));
        images.next().expect("the cell's image")
    };
    let icon = image(cells[0]);
    let flask = ui.world().resource::<AssetRegistry>().sprite_id("flask");
    assert_eq!(tree.image_sprite(icon), flask, "the base's icon");
    let empty_icon = image(cells[2]);
    assert_eq!(
        tree.visibility(empty_icon),
        Some(tungsten_core::ui::Visibility::Collapsed)
    );

    let charm = keyed(&ui, "worn/charm");
    let name = built(ui.world(), &fine_charm())
        .expect("built")
        .name()
        .to_owned();
    assert_eq!(texts(&ui, charm), [name.as_str(), "charm"]);
    assert_eq!(ui.tree().text_style(charm), Some("rarity.magic"));
    assert_eq!(texts(&ui, keyed(&ui, "worn/hood")), ["", "hood"]);
    assert_eq!(texts(&ui, keyed(&ui, "coins")), ["50"]);

    // They follow the world.
    let world = ui.world_mut();
    world.resource_mut::<Counters>().set("coins", 45);
    give(world, warden, "oil_flask", 1);
    ui.step(1);
    assert_eq!(texts(&ui, keyed(&ui, "coins")), ["45"]);
    assert_eq!(texts(&ui, keyed(&ui, "cells/0")), ["Oil Flask", "4"]);
}

#[test]
fn dragging_a_cell_onto_a_slot_wears_it_and_back_takes_it_off() {
    let (mut ui, warden, _) = rig();
    give(ui.world_mut(), warden, "oilskin_hood", 1);
    opened(&mut ui, warden);
    drag(&mut ui, "cells/0", "worn/hood");
    let worn = bag(&ui, warden).worn("hood").map(Item::base);
    assert_eq!(worn, Some("oilskin_hood"));
    assert!(bag(&ui, warden).cell(0).is_none());
    assert_eq!(
        texts(&ui, keyed(&ui, "worn/hood")),
        ["Oilskin Hood", "hood"]
    );

    drag(&mut ui, "worn/hood", "cells/3");
    assert!(bag(&ui, warden).worn("hood").is_none(), "taken off");
    assert_eq!(
        bag(&ui, warden).cell(3).map(Item::base),
        Some("oilskin_hood")
    );
    assert!(toasts(&ui).is_empty());
}

#[test]
fn a_refused_drop_leaves_both_and_toasts_why() {
    let (mut ui, warden, _) = rig();
    let world = ui.world_mut();
    give(world, warden, "oil_flask", 1);
    give(world, warden, "oilskin_hood", 1);
    opened(&mut ui, warden);
    drag(&mut ui, "cells/0", "worn/hood");
    assert_eq!(bag(&ui, warden).cell(0).map(Item::base), Some("oil_flask"));
    assert!(bag(&ui, warden).worn("hood").is_none());
    assert_eq!(toasts(&ui), ["It is not worn there"]);
}

#[test]
fn accept_holds_a_cell_and_accept_puts_it_down() {
    let (mut ui, warden, _) = rig();
    give(ui.world_mut(), warden, "oil_flask", 2);
    opened(&mut ui, warden);
    let holding = |ui: &UiHarness| {
        let hint = ui
            .tree()
            .find_key(ui.front_modal().unwrap(), UiKey::new("holding"));
        hint.and_then(|hint| ui.tree().visibility(hint))
            == Some(tungsten_core::ui::Visibility::Visible)
    };
    assert!(!holding(&ui));
    accept(&mut ui, "cells/0");
    let view = ui.world().resource::<ItemsView>();
    assert_eq!(view.held(), Some(&ItemPlace::Cell(0)));
    assert!(holding(&ui), "the hint shows");
    accept(&mut ui, "cells/5");
    assert_eq!(
        bag(&ui, warden).cell(5).map(Item::count),
        Some(2),
        "put down"
    );
    assert!(bag(&ui, warden).cell(0).is_none());
    assert_eq!(ui.world().resource::<ItemsView>().held(), None);
    assert!(!holding(&ui));

    // An empty cell holds nothing; Cancel lets go first, then closes.
    accept(&mut ui, "cells/1");
    assert_eq!(ui.world().resource::<ItemsView>().held(), None);
    accept(&mut ui, "cells/5");
    tap(&mut ui, KeyCode::Escape);
    assert_eq!(ui.world().resource::<ItemsView>().held(), None, "let go");
    assert_eq!(top(&ui), Some(INVENTORY_SCREEN), "still open");

    // Accept twice on a cell opens its menu.
    accept(&mut ui, "cells/5");
    accept(&mut ui, "cells/5");
    ui.step(4);
    assert_eq!(top(&ui), Some("cell_menu"));
    tap(&mut ui, KeyCode::Escape);
    ui.step(12);
    tap(&mut ui, KeyCode::Escape);
    ui.step(12);
    assert_eq!(top(&ui), None, "closed");
}

#[test]
fn a_cells_tooltip_shows_its_name_in_its_rarity_style_and_its_lines() {
    let (mut ui, warden, _) = rig();
    give_item(ui.world_mut(), warden, fine_charm()).expect("room");
    opened(&mut ui, warden);
    let cell = keyed(&ui, "cells/0");
    ui.hover(cell);
    ui.step(1);
    ui.wait(popup::TooltipSettings::default().delay);
    ui.step(2);
    let root = popup::shown_tooltip(ui.world()).expect("the cell's tooltip");
    let made = built(ui.world(), &fine_charm()).expect("built");
    let tree = ui.tree();
    let find = |key: &str| tree.find_key(root, UiKey::new(key)).expect(key);
    assert_eq!(tree.plain_text(find("name")), made.name());
    assert_eq!(tree.text_style(find("name")), Some("rarity.magic"));
    assert_eq!(tree.plain_text(find("slot")), "charm");
    let lines: Vec<String> = made
        .lines()
        .iter()
        .map(|line| line.text().to_owned())
        .collect();
    assert_eq!(lines.len(), 2, "the implicit line and the affix's");
    assert_eq!(texts(&ui, find("lines")), lines);
    assert!(texts(&ui, find("block")).is_empty(), "no weapon block");
    assert_eq!(tree.plain_text(find("requires")), "");
    assert_ne!(
        tree.visibility(find("price")),
        Some(tungsten_core::ui::Visibility::Visible),
        "no visit prices it"
    );
}

#[test]
fn the_cell_menu_uses_wears_and_drops() {
    let (mut ui, warden, _) = rig();
    let world = ui.world_mut();
    give(world, warden, "oil_flask", 2);
    give(world, warden, "oilskin_hood", 1);
    opened(&mut ui, warden);

    click(&mut ui, "cells/0");
    assert_eq!(top(&ui), Some("cell_menu"), "a click opens the menu");
    let menu = ui.front_modal().expect("the menu");
    let tree = ui.tree();
    let name = tree.find_key(menu, UiKey::new("name")).expect("its name");
    assert_eq!(tree.plain_text(name), "Oil Flask", "it reads its control");
    click(&mut ui, "use");
    assert_eq!(top(&ui), Some(INVENTORY_SCREEN), "the action closes it");
    assert_eq!(count_of(ui.world(), warden, "oil_flask"), 1, "one used");
    assert_eq!(
        ui.world().get::<Counters>(warden).map(|c| c.get("oil")),
        Some(10)
    );

    click(&mut ui, "cells/1");
    click(&mut ui, "wear");
    assert_eq!(
        bag(&ui, warden).worn("hood").map(Item::base),
        Some("oilskin_hood")
    );
    click(&mut ui, "worn/hood");
    click(&mut ui, "wear");
    assert!(bag(&ui, warden).worn("hood").is_none(), "taken off");

    click(&mut ui, "cells/0");
    click(&mut ui, "drop");
    assert!(bag(&ui, warden).cell(0).is_none(), "dropped");
    let lying = ui.world().query::<&GroundItem>().count();
    assert_eq!(lying, 1, "onto the ground");
    assert!(toasts(&ui).is_empty(), "{:?}", toasts(&ui));
}

/// The warden visiting the chandlery, its screen opened.
fn shopping(ui: &mut UiHarness, warden: Entity, keeper: Entity) {
    assert!(open_shop(ui.world_mut(), "chandlery", keeper, warden));
    ui.step(12);
    assert_eq!(top(ui), Some(SHOP_SCREEN), "a visit opens the shop");
}

#[test]
fn a_visit_opens_the_shop_and_a_drag_from_the_stock_buys() {
    let (mut ui, warden, keeper) = rig();
    shopping(&mut ui, warden, keeper);
    assert_eq!(texts(&ui, keyed(&ui, "name")), ["Tamsin's Chandlery"]);
    assert_eq!(texts(&ui, keyed(&ui, "coins")), ["50"]);
    assert_eq!(
        texts(&ui, keyed(&ui, "stock/oilskin_hood")),
        ["Oilskin Hood", "12", "2"]
    );
    assert_eq!(
        texts(&ui, keyed(&ui, "stock/oil_flask")),
        ["Oil Flask", "3", ""]
    );

    drag(&mut ui, "stock/oil_flask", "cells/0");
    assert_eq!(count_of(ui.world(), warden, "oil_flask"), 1, "bought");
    assert_eq!(coins(&ui), 47);
    click(&mut ui, "stock/oilskin_hood");
    assert_eq!(
        count_of(ui.world(), warden, "oilskin_hood"),
        1,
        "its action buys"
    );
    assert_eq!(coins(&ui), 35);
    assert_eq!(
        texts(&ui, keyed(&ui, "stock/oilskin_hood"))[2],
        "1",
        "one left"
    );

    click(&mut ui, "leave");
    assert!(visit(ui.world()).is_none(), "the visit ends");
    assert_eq!(top(&ui), None, "and its screen closes");
}

#[test]
fn a_drag_onto_sell_sells() {
    let (mut ui, warden, keeper) = rig();
    give(ui.world_mut(), warden, "oil_flask", 2);
    shopping(&mut ui, warden, keeper);
    drag(&mut ui, "cells/0", "sell");
    assert_eq!(
        count_of(ui.world(), warden, "oil_flask"),
        0,
        "the stack sold"
    );
    assert_eq!(coins(&ui), 52, "floor(3 × 0.5) a flask");
    assert_eq!(toasts(&ui), ["Sold for 2 coins"]);

    give_item(ui.world_mut(), warden, fine_charm()).expect("room");
    ui.step(1);
    drag(&mut ui, "cells/0", "sell");
    assert_eq!(count_of(ui.world(), warden, "brass_charm"), 1, "kept");
    assert_eq!(coins(&ui), 52);
    assert!(toasts(&ui).contains(&"The shop does not buy that".to_owned()));

    // Closing the screen ends the visit.
    tap(&mut ui, KeyCode::Escape);
    ui.step(12);
    assert_eq!(top(&ui), None);
    assert!(visit(ui.world()).is_none());
}

#[test]
fn a_shop_a_talk_opens_waits_for_the_talk_to_end() {
    let (mut ui, warden, keeper) = rig();
    let world = ui.world_mut();
    assert!(talk(world, "chandler.greeting", keeper, warden));
    Dialogue::advance(world);
    Dialogue::choose(world, 0);
    assert!(visit(ui.world()).is_some(), "the choice opens a visit");
    ui.step(12);
    assert_ne!(top(&ui), Some(SHOP_SCREEN), "not while the talk runs");
    Dialogue::advance(ui.world_mut());
    assert!(
        ui.world().resource::<Dialogue>().current().is_none(),
        "the talk ended"
    );
    ui.step(12);
    assert_eq!(top(&ui), Some(SHOP_SCREEN));
}

#[test]
fn the_screens_check_against_the_bindings() {
    let (ui, ..) = rig();
    let world = ui.world();
    let registry = world.resource::<UiScreenRegistry>();
    let names = crate::asset_loader::check::screen_names(world, registry);
    for id in [INVENTORY_SCREEN, SHOP_SCREEN, "item_tooltip", "cell_menu"] {
        let file = registry.get(id).expect(id);
        assert!(
            file.check(&names).is_empty(),
            "{id}: {:?}",
            file.check(&names)
        );
    }
    let file = |id: &str| registry.get(id).expect(id);
    assert!(file(INVENTORY_SCREEN).data_screen_refusal().is_none());
    assert!(file("cell_menu").popup_refusal().is_none());
    assert!(file("item_tooltip").tooltip_refusal().is_none());
}

#[test]
fn without_its_file_a_screen_warns_once_and_opens_nothing() {
    let (mut ui, warden, keeper) = rig();
    let world = ui.world_mut();
    let mut registry = UiScreenRegistry::new();
    std::mem::swap(world.resource_mut::<UiScreenRegistry>(), &mut registry);
    assert!(!open_inventory(world, warden));
    assert!(!open_inventory(world, warden));
    assert!(open_shop(world, "chandlery", keeper, warden));
    ui.step(12);
    assert_eq!(top(&ui), None, "nothing opens");
    assert!(visit(ui.world()).is_some(), "the visit stays for Rust");
    let warned = &ui.world().resource::<ItemsView>().warned;
    assert_eq!(warned, &[INVENTORY_SCREEN, SHOP_SCREEN], "each once");
}
