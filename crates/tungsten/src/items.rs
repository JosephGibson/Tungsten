//! The item screens (redo 8.1's second plan, `D-31[6]`): an inventory and a
//! shop over the kit's items, built from a game's `ui_screens` files
//! `inventory` and `shop` on the dialogue box's pattern (`D-315`), with
//! the item tooltip and the cell menu as screen files of their own.

use tungsten_core::ui::{
    InputDevice, ScreenAction, UiAction, UiBindings, UiEvent, UiRouter, UiRow, UiScreenRegistry,
    UiTree, WidgetId, screen_build,
};
use tungsten_core::{DataRegistry, Entity, EventQueue, Plugin, Schedule, Stage, World, system};

use crate::kit::{
    Counters, Dialogue, Inventory, Item, ItemBase, ItemPlace, ItemRefusal, Pays, Shop, ShopRefusal,
    ShopVisit, WeaponBlock, built, buy, drop_item, end_shop, inventory, move_item, price_of, sell,
    shop_stock, use_item, visit,
};
use crate::screens::{self, ModalScreen, popup, toast};
use crate::state::{GameState, StateContext, StateId, StateStack};

/// The inventory's screen ID and the `ui_screens` file it builds.
pub const INVENTORY_SCREEN: StateId = "inventory";
/// The shop's screen ID and the `ui_screens` file it builds.
pub const SHOP_SCREEN: StateId = "shop";
/// [`ItemsUiPlugin`]'s system, in `update`: opens the shop over a visit.
pub const SHOP_OPEN: &str = "shop_open";

/// Opts a game into the item screens (redo 8.1's second plan): registers
/// the bindings the files name, the screens `inventory` and `shop`
/// ([`screens::register`]) and [`SHOP_OPEN`] in `update`, which opens the
/// shop while a visit runs ([`visit`]), no talk runs and the screens are
/// idle. A screen whose file is missing warns once and opens nothing.
///
/// Its bindings: rows `inventory.cells` (a row a bag cell, keyed by its
/// index: `icon`, `name`, `count`, empty below 2, and `style`,
/// `rarity.<rarity>`, empty for an empty cell) and `inventory.worn` (a row
/// a worn slot, keyed by its name: `slot`, `icon`, `name`, `style`), text
/// `inventory.coins` and visible `inventory.holding`; for a tooltip or a
/// menu, text `item.name`, style `item.style`, text `item.slot`, rows
/// `item.lines` and `item.block` (`text`), text `item.requires` (empty when
/// the holder meets them), text `item.price` and visible `item.priced`;
/// for a shop, text `shop.name` and `shop.coins` and rows `shop.stock`
/// (keyed by base: `icon`, `name`, `price`, `count`, empty when unlimited,
/// and `style`). The `item.*` bindings read the tooltip's widget, else the
/// open popup's control, else the held cell, by key: `cells/<i>`,
/// `worn/<slot>` or `stock/<base>`. Its actions, on that same item:
/// `inventory.use`, `inventory.wear` (onto its slot, or off into a free
/// cell), `inventory.drop` (onto its base's `ground`), `shop.buy` (a stock
/// row's, or the item's), `shop.sell` (the whole stack) and `shop.leave`;
/// each closes the popup it runs in.
///
/// On either screen a drag from `cells/<i>` or `worn/<slot>` onto another
/// moves the item ([`move_item`]), onto the node keyed `ground` drops a
/// cell's stack, onto `sell` sells it, and from `stock/<base>` onto a cell
/// buys one; a refusal shows as a toast naming why. Accept from a key or a
/// pad on a cell or slot holds it (`inventory.holding`), Accept on another
/// moves it there, on `ground` or `sell` drops or sells a held cell's
/// stack, Accept on it again lets go and opens its menu (its node's `on`),
/// and Cancel lets go first and closes after; a pointer's
/// click runs the node's `on`. A screen's other controls run their `on` as
/// a data screen's do, a `setting` aside. The shop's bag is its customer's;
/// leaving it, or a visit ended from Rust, closes it, and closing it ends
/// the visit. Whether play stops under either is the covered state's
/// `holds_clock`.
#[derive(Debug, Default, Clone, Copy)]
pub struct ItemsUiPlugin;

impl Plugin for ItemsUiPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        if !world.has_resource::<ItemsView>() {
            world.insert_resource(ItemsView::default());
        }
        UiBindings::of(world)
            .rows("inventory.cells", cell_rows)
            .rows("inventory.worn", worn_rows)
            .text("inventory.coins", coins_text)
            .visible("inventory.holding", holding)
            .text("item.name", item_name)
            .style("item.style", item_style)
            .text("item.slot", item_slot)
            .rows("item.lines", item_lines)
            .rows("item.block", item_block)
            .text("item.requires", item_requires)
            .text("item.price", item_price)
            .visible("item.priced", item_priced)
            .text("shop.name", shop_name)
            .text("shop.coins", coins_text)
            .rows("shop.stock", stock_rows)
            .action("inventory.use", use_action)
            .action("inventory.wear", wear_action)
            .action("inventory.drop", drop_action)
            .action("shop.buy", buy_action)
            .action("shop.sell", sell_action)
            .action("shop.leave", leave_action);
        screens::register(world, INVENTORY_SCREEN, make_inventory);
        screens::register(world, SHOP_SCREEN, make_shop);
        schedule.add(Stage::Update, system(SHOP_OPEN, shop_open));
    }
}

/// What the item screens show, a World resource [`ItemsUiPlugin`] inserts:
/// whose bag, the place Accept holds, whether a shop is up and the files
/// warned missing.
#[derive(Debug, Default)]
pub struct ItemsView {
    holder: Option<Entity>,
    held: Option<ItemPlace>,
    shop_up: bool,
    warned: Vec<StateId>,
}

impl ItemsView {
    /// Whose bag the screens show: [`open_inventory`]'s holder, or the
    /// shop's customer.
    #[must_use]
    pub fn holder(&self) -> Option<Entity> {
        self.holder
    }

    /// The cell or slot Accept holds.
    #[must_use]
    pub fn held(&self) -> Option<&ItemPlace> {
        self.held.as_ref()
    }
}

/// Opens the inventory screen over `holder`'s bag: `false`, opening
/// nothing, without [`ItemsUiPlugin`], without the file `inventory`
/// (warned once), for a holder with no [`Inventory`], or while the screens
/// are not idle.
pub fn open_inventory(world: &mut World, holder: Entity) -> bool {
    if !world.has_resource::<ItemsView>() {
        return false;
    }
    if !has_file(world, INVENTORY_SCREEN) || inventory(world, holder).is_none() {
        return false;
    }
    if !screens::idle(world) {
        return false;
    }
    view_mut(world).holder = Some(holder);
    screens::open(world, INVENTORY_SCREEN)
}

fn view_mut(world: &mut World) -> &mut ItemsView {
    if !world.has_resource::<ItemsView>() {
        world.insert_resource(ItemsView::default());
    }
    world.resource_mut::<ItemsView>()
}

/// Whether the `ui_screens` file `id` is loaded; warns once when not.
fn has_file(world: &mut World, id: StateId) -> bool {
    let loaded = world
        .get_resource::<UiScreenRegistry>()
        .is_some_and(|registry| registry.contains(id));
    if !loaded {
        let view = view_mut(world);
        if !view.warned.contains(&id) {
            view.warned.push(id);
            log::warn!("items: no `ui_screens` file `{id}` is loaded; its screen does not open");
        }
    }
    loaded
}

fn holder(world: &World) -> Option<Entity> {
    world.get_resource::<ItemsView>()?.holder
}

fn bag(world: &World) -> Option<&Inventory> {
    inventory(world, holder(world)?)
}

fn base_of<'w>(world: &'w World, base: &str) -> Option<&'w ItemBase> {
    world.get_resource::<DataRegistry>()?.get::<ItemBase>(base)
}

fn shop_of(world: &World) -> Option<&Shop> {
    let visit = visit(world)?;
    world
        .get_resource::<DataRegistry>()?
        .get::<Shop>(visit.shop())
}

// ---- keys ----

/// What a key names: a bag cell or worn slot, a stock entry, or a target.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Spot {
    Place(ItemPlace),
    Stock(String),
    Ground,
    Sell,
}

fn spot(name: &str) -> Option<Spot> {
    if let Some(cell) = name.strip_prefix("cells/") {
        return cell
            .parse()
            .ok()
            .map(|cell| Spot::Place(ItemPlace::Cell(cell)));
    }
    if let Some(slot) = name.strip_prefix("worn/") {
        return Some(Spot::Place(ItemPlace::worn(slot)));
    }
    if let Some(base) = name.strip_prefix("stock/") {
        return Some(Spot::Stock(base.to_owned()));
    }
    match name {
        "ground" => Some(Spot::Ground),
        "sell" => Some(Spot::Sell),
        _ => None,
    }
}

fn spot_of(world: &World, widget: WidgetId) -> Option<Spot> {
    spot(world.get_resource::<UiTree>()?.key_name(widget)?)
}

/// The item a place in the bag holds.
fn item_at<'w>(world: &'w World, place: &ItemPlace) -> Option<&'w Item> {
    let bag = bag(world)?;
    match place {
        ItemPlace::Cell(cell) => bag.cell(*cell),
        ItemPlace::Worn(slot) => bag.worn(slot),
    }
}

/// What the `item.*` bindings and the actions read: the tooltip's widget,
/// else the open popup's control, else the held place.
fn shown_spot(world: &World) -> Option<Spot> {
    let anchored = |anchor: Option<(WidgetId, _)>| anchor.and_then(|(at, _)| spot_of(world, at));
    anchored(popup::tooltip_anchor(world))
        .or_else(|| anchored(popup::popup_anchor(world)))
        .or_else(|| {
            Some(Spot::Place(
                world.get_resource::<ItemsView>()?.held.clone()?,
            ))
        })
}

/// The item the `item.*` bindings show: a bag item, or one of a stock
/// entry's base.
fn shown_item(world: &World) -> Option<Item> {
    match shown_spot(world)? {
        Spot::Place(place) => item_at(world, &place).cloned(),
        Spot::Stock(base) => Some(Item::new(base)),
        Spot::Ground | Spot::Sell => None,
    }
}

// ---- bindings ----

fn name_of(world: &World, item: &Item) -> String {
    built(world, item).map_or_else(|| item.base().to_owned(), |made| made.name().to_owned())
}

fn icon_of(world: &World, base: &str) -> String {
    base_of(world, base).map_or_else(String::new, |base| base.icon().to_owned())
}

fn style_of(item: &Item) -> String {
    format!("rarity.{}", item.rarity())
}

fn item_row(world: &World, key: String, item: Option<&Item>) -> UiRow {
    let row = UiRow::new(key);
    let Some(item) = item else {
        return row
            .with("icon", "")
            .with("name", "")
            .with("count", "")
            .with("style", "");
    };
    let count = if item.count() < 2 {
        String::new()
    } else {
        item.count().to_string()
    };
    row.with("icon", icon_of(world, item.base()))
        .with("name", name_of(world, item))
        .with("count", count)
        .with("style", style_of(item))
}

fn cell_rows(world: &World) -> Vec<UiRow> {
    bag(world).map_or_else(Vec::new, |bag| {
        bag.cells()
            .enumerate()
            .map(|(cell, item)| item_row(world, cell.to_string(), item))
            .collect()
    })
}

fn worn_rows(world: &World) -> Vec<UiRow> {
    bag(world).map_or_else(Vec::new, |bag| {
        bag.slots()
            .map(|(name, slot)| item_row(world, name.to_owned(), slot.item()).with("slot", name))
            .collect()
    })
}

/// The coins the screens show: the visit's purse in the shop's currency,
/// else the holder's `coins` counter, else the game's.
fn purse(world: &World) -> i64 {
    if let (Some(visit), Some(shop)) = (visit(world), shop_of(world)) {
        let counters = match shop.pays() {
            Pays::Game => world.get_resource::<Counters>(),
            Pays::Other => world.get::<Counters>(visit.customer()),
        };
        return counters.map_or(0, |counters| counters.get(shop.currency()));
    }
    let own = holder(world).and_then(|holder| world.get::<Counters>(holder));
    own.or_else(|| world.get_resource::<Counters>())
        .map_or(0, |counters| counters.get("coins"))
}

fn coins_text(world: &World) -> String {
    purse(world).to_string()
}

fn holding(world: &World) -> bool {
    world
        .get_resource::<ItemsView>()
        .is_some_and(|view| view.held.is_some())
}

fn item_name(world: &World) -> String {
    shown_item(world).map_or_else(String::new, |item| name_of(world, &item))
}

fn item_style(world: &World) -> String {
    shown_item(world).map_or_else(String::new, |item| style_of(&item))
}

fn item_slot(world: &World) -> String {
    shown_item(world)
        .and_then(|item| base_of(world, item.base())?.slot().map(str::to_owned))
        .unwrap_or_default()
}

fn text_rows(lines: impl IntoIterator<Item = String>) -> Vec<UiRow> {
    lines
        .into_iter()
        .enumerate()
        .map(|(at, text)| UiRow::new(at.to_string()).with("text", text))
        .collect()
}

fn item_lines(world: &World) -> Vec<UiRow> {
    let made = shown_item(world).and_then(|item| built(world, &item));
    text_rows(
        made.iter()
            .flat_map(|made| made.lines().iter().map(|line| line.text().to_owned())),
    )
}

fn item_block(world: &World) -> Vec<UiRow> {
    let made = shown_item(world).and_then(|item| built(world, &item));
    text_rows(
        made.and_then(|made| made.block().map(WeaponBlock::text))
            .unwrap_or_default(),
    )
}

fn item_requires(world: &World) -> String {
    let Some(item) = shown_item(world) else {
        return String::new();
    };
    let requires = base_of(world, item.base()).and_then(ItemBase::requires);
    match requires {
        Some(requires) if !requires.holds(world, holder(world), None) => {
            "Its requirements are not met".to_owned()
        }
        _ => String::new(),
    }
}

/// A stock entry's price now, while a visit runs.
fn stock_price(world: &World) -> Option<u32> {
    match shown_spot(world)? {
        Spot::Stock(base) => price_of(world, &base),
        _ => None,
    }
}

fn item_price(world: &World) -> String {
    match (stock_price(world), shop_of(world)) {
        (Some(price), Some(shop)) => format!("{price} {}", shop.currency()),
        _ => String::new(),
    }
}

fn item_priced(world: &World) -> bool {
    stock_price(world).is_some()
}

fn shop_name(world: &World) -> String {
    shop_of(world).map_or_else(String::new, |shop| shop.name().to_owned())
}

fn stock_rows(world: &World) -> Vec<UiRow> {
    shop_stock(world)
        .into_iter()
        .map(|(base, price, left)| {
            let item = Item::new(base.clone());
            UiRow::new(base.clone())
                .with("icon", icon_of(world, &base))
                .with("name", name_of(world, &item))
                .with("price", price.to_string())
                .with(
                    "count",
                    left.map_or_else(String::new, |left| left.to_string()),
                )
                .with("style", style_of(&item))
        })
        .collect()
}

// ---- actions ----

fn refused(why: ItemRefusal) -> &'static str {
    match why {
        ItemRefusal::NoItem => "There is nothing there",
        ItemRefusal::WrongSlot => "It is not worn there",
        ItemRefusal::Requires => "Its requirements are not met",
        ItemRefusal::Full => "The bag is full",
        ItemRefusal::Stacked => "A stack cannot be worn",
        _ => "It does not go there",
    }
}

fn shop_refused(world: &World, why: ShopRefusal) -> String {
    let currency = shop_of(world).map_or("coins", Shop::currency);
    match why {
        ShopRefusal::Closed => "The shop is closed".to_owned(),
        ShopRefusal::Hidden => "It is not for sale".to_owned(),
        ShopRefusal::SoldOut => "It is sold out".to_owned(),
        ShopRefusal::Poor => format!("Not enough {currency}"),
        ShopRefusal::Full => "The bag is full".to_owned(),
        ShopRefusal::NotBought => "The shop does not buy that".to_owned(),
        _ => "It cannot be traded".to_owned(),
    }
}

/// Moves the item at `from` to `to`, a toast naming a refusal.
fn moved(world: &mut World, from: ItemPlace, to: ItemPlace) {
    let Some(holder) = holder(world) else {
        return;
    };
    if from != to
        && let Err(why) = move_item(world, holder, from, to)
    {
        toast(world, refused(why));
    }
}

/// The first empty cell of the bag.
fn free_cell(world: &World) -> Option<usize> {
    bag(world)?.cells().position(|item| item.is_none())
}

/// Takes the item worn in `slot` off into a free cell; that cell.
fn taken_off(world: &mut World, slot: &str) -> Result<usize, ItemRefusal> {
    let holder = holder(world).ok_or(ItemRefusal::NoItem)?;
    let cell = free_cell(world).ok_or(ItemRefusal::Full)?;
    move_item(world, holder, ItemPlace::worn(slot), ItemPlace::Cell(cell))?;
    Ok(cell)
}

/// Drops a cell's whole stack onto its base's `ground`.
fn dropped_cell(world: &mut World, cell: usize) {
    let Some(holder) = holder(world) else {
        return;
    };
    let count = item_at(world, &ItemPlace::Cell(cell)).map_or(0, Item::count);
    if drop_item(world, holder, cell, count).is_none() {
        toast(world, "It cannot be dropped");
    }
}

fn bought(world: &mut World, base: &str) {
    if let Err(why) = buy(world, base) {
        let text = shop_refused(world, why);
        toast(world, text);
    }
}

fn sold(world: &mut World, cell: usize) {
    let count = item_at(world, &ItemPlace::Cell(cell)).map_or(0, Item::count);
    let text = match sell(world, cell, count) {
        Ok(paid) => {
            let currency = shop_of(world).map_or("coins", Shop::currency);
            format!("Sold for {paid} {currency}")
        }
        Err(why) => shop_refused(world, why),
    };
    toast(world, text);
}

/// The spot an action acts on: the open popup's control, else the held
/// place; never a tooltip's widget.
fn acted_spot(world: &World) -> Option<Spot> {
    popup::popup_anchor(world)
        .and_then(|(at, _)| spot_of(world, at))
        .or_else(|| {
            Some(Spot::Place(
                world.get_resource::<ItemsView>()?.held.clone()?,
            ))
        })
}

/// The place an action acts on.
fn acted_on(world: &World) -> Option<ItemPlace> {
    match acted_spot(world)? {
        Spot::Place(place) => Some(place),
        _ => None,
    }
}

/// Closes the popup an action ran in, letting go of a held place.
fn done(world: &mut World) {
    view_mut(world).held = None;
    if popup::popup_anchor(world).is_some() {
        screens::pop(world);
    }
}

fn use_action(world: &mut World, _: UiAction) {
    if let (Some(ItemPlace::Cell(cell)), Some(holder)) = (acted_on(world), holder(world))
        && !use_item(world, holder, cell)
    {
        toast(world, "It cannot be used");
    }
    done(world);
}

fn wear_action(world: &mut World, _: UiAction) {
    match acted_on(world) {
        Some(ItemPlace::Cell(cell)) => {
            let kind = item_at(world, &ItemPlace::Cell(cell))
                .and_then(|item| base_of(world, item.base())?.slot().map(str::to_owned));
            let slot = kind.and_then(|kind| {
                let slots = || bag(world).into_iter().flat_map(Inventory::slots);
                let fits = |(_, slot): &(&str, &crate::kit::WornSlot)| slot.kind() == kind;
                let empty = slots().filter(fits).find(|(_, slot)| slot.item().is_none());
                empty
                    .or_else(|| slots().find(fits))
                    .map(|(name, _)| name.to_owned())
            });
            match slot {
                Some(slot) => moved(world, ItemPlace::Cell(cell), ItemPlace::Worn(slot)),
                None => toast(world, refused(ItemRefusal::WrongSlot)),
            }
        }
        Some(ItemPlace::Worn(slot)) => {
            if let Err(why) = taken_off(world, &slot) {
                toast(world, refused(why));
            }
        }
        None => {}
    }
    done(world);
}

fn drop_action(world: &mut World, _: UiAction) {
    match acted_on(world) {
        Some(ItemPlace::Cell(cell)) => dropped_cell(world, cell),
        Some(ItemPlace::Worn(slot)) => match taken_off(world, &slot) {
            Ok(cell) => dropped_cell(world, cell),
            Err(why) => toast(world, refused(why)),
        },
        None => {}
    }
    done(world);
}

fn buy_action(world: &mut World, call: UiAction) {
    let base = call.row.or_else(|| match acted_spot(world)? {
        Spot::Stock(base) => Some(base),
        _ => None,
    });
    if let Some(base) = base {
        bought(world, &base);
    }
    done(world);
}

fn sell_action(world: &mut World, _: UiAction) {
    if let Some(ItemPlace::Cell(cell)) = acted_on(world) {
        sold(world, cell);
    }
    done(world);
}

fn leave_action(world: &mut World, _: UiAction) {
    end_shop(world);
}

// ---- the screens ----

fn make_inventory() -> Box<dyn GameState> {
    Box::new(ItemsScreen::new(INVENTORY_SCREEN))
}

fn make_shop() -> Box<dyn GameState> {
    Box::new(ItemsScreen::new(SHOP_SCREEN))
}

/// Opens the shop over a visit while none is up, no talk runs and the
/// screens are idle, its bag the customer's.
fn shop_open(world: &mut World) {
    let Some(customer) = visit(world).map(ShopVisit::customer) else {
        return;
    };
    if world
        .get_resource::<ItemsView>()
        .is_none_or(|view| view.shop_up)
    {
        return;
    }
    let talking = world
        .get_resource::<Dialogue>()
        .is_some_and(|dialogue| dialogue.current().is_some());
    if talking || !screens::idle(world) || !has_file(world, SHOP_SCREEN) {
        return;
    }
    view_mut(world).holder = Some(customer);
    screens::open(world, SHOP_SCREEN);
}

/// The inventory or the shop: its file built in `on_enter` and adopted as
/// a [`ModalScreen`], its drags, holds and actions run, its bindings
/// refreshed each frame.
struct ItemsScreen {
    id: StateId,
    screen: Option<ModalScreen>,
    closing: bool,
}

/// What a frame's routing did on the screen.
enum Used {
    Activated(WidgetId),
    Dropped(WidgetId, WidgetId),
}

impl ItemsScreen {
    fn new(id: StateId) -> Self {
        Self {
            id,
            screen: None,
            closing: false,
        }
    }

    /// Runs this routing's activations and drops on `root`.
    fn react(world: &mut World, root: WidgetId) {
        let used: Vec<Used> = {
            let tree = world.resource::<UiTree>();
            let events = world.resource::<EventQueue<UiEvent>>();
            let ours = |widget: WidgetId| tree.root_of(widget) == Some(root);
            events
                .iter_current()
                .filter_map(|event| match *event {
                    UiEvent::Activated { widget, .. } if ours(widget) => {
                        Some(Used::Activated(widget))
                    }
                    UiEvent::Dropped { widget, target, .. } if ours(widget) => {
                        Some(Used::Dropped(widget, target))
                    }
                    _ => None,
                })
                .collect()
        };
        for used in used {
            match used {
                Used::Activated(widget) => match spot_of(world, widget) {
                    Some(Spot::Place(place)) => Self::accepted(world, root, widget, place),
                    Some(target @ (Spot::Ground | Spot::Sell)) => {
                        match view_mut(world).held.take() {
                            Some(ItemPlace::Cell(cell)) if target == Spot::Ground => {
                                dropped_cell(world, cell);
                            }
                            Some(ItemPlace::Cell(cell)) => sold(world, cell),
                            _ => run(world, root, widget),
                        }
                    }
                    _ => run(world, root, widget),
                },
                Used::Dropped(from, to) => Self::dropped(world, from, to),
            }
        }
    }

    /// A cell or slot activated: held from a key or a pad, placed when
    /// another is held, its `on` run from the pointer or when held again.
    fn accepted(world: &mut World, root: WidgetId, widget: WidgetId, place: ItemPlace) {
        let keyed = world
            .get_resource::<UiRouter>()
            .is_some_and(|router| router.last_device() != InputDevice::Pointer);
        match view_mut(world).held.take() {
            Some(held) if held == place => {
                if keyed {
                    run(world, root, widget);
                }
            }
            Some(held) => moved(world, held, place),
            None if keyed => {
                if item_at(world, &place).is_some() {
                    view_mut(world).held = Some(place);
                }
            }
            None => run(world, root, widget),
        }
    }

    /// A drag dropped `from` onto `to`.
    fn dropped(world: &mut World, from: WidgetId, to: WidgetId) {
        let (Some(from), Some(to)) = (spot_of(world, from), spot_of(world, to)) else {
            return;
        };
        match (from, to) {
            (Spot::Place(from), Spot::Place(to)) => moved(world, from, to),
            (Spot::Place(ItemPlace::Cell(cell)), Spot::Ground) => dropped_cell(world, cell),
            (Spot::Place(ItemPlace::Cell(cell)), Spot::Sell) => sold(world, cell),
            (Spot::Stock(base), Spot::Place(ItemPlace::Cell(_))) => bought(world, &base),
            _ => {}
        }
    }
}

/// Runs `widget`'s `on`, as a data screen does; a `setting` writes nothing
/// here.
fn run(world: &mut World, root: WidgetId, widget: WidgetId) {
    let Some(keyed) = screen_build::action(world, root, widget) else {
        return;
    };
    match &keyed.action {
        ScreenAction::Pop => {
            screens::pop(world);
        }
        ScreenAction::Push(id) => {
            screens::open(world, id);
        }
        ScreenAction::Popup(id) => {
            popup::open(world, id, widget);
        }
        ScreenAction::Named(name) => {
            let action = world
                .get_resource::<UiBindings>()
                .and_then(|bindings| bindings.action_named(name));
            if let Some(action) = action {
                let call = UiAction {
                    key: keyed.key.clone(),
                    row: keyed.row.clone(),
                    value: None,
                };
                action(world, call);
            }
        }
        ScreenAction::Setting(_) => {}
    }
}

/// Whether this routing cancelled on `root`.
fn cancelled(world: &World, root: WidgetId) -> bool {
    world
        .get_resource::<EventQueue<UiEvent>>()
        .is_some_and(|events| {
            events
                .iter_current()
                .any(|event| matches!(event, UiEvent::Cancel { scope, .. } if *scope == root))
        })
}

impl GameState for ItemsScreen {
    fn id(&self) -> StateId {
        self.id
    }

    fn on_enter(&mut self, ctx: &mut StateContext) {
        let world = &mut *ctx.world;
        let Some(built) = screen_build::build(world, self.id) else {
            world.resource_mut::<StateStack>().request_pop();
            return;
        };
        let screen = ModalScreen::adopt(world, built.root, built.card);
        let customer = visit(world).map(ShopVisit::customer);
        let view = view_mut(world);
        view.held = None;
        if self.id == SHOP_SCREEN {
            view.shop_up = true;
            view.holder = customer.or(view.holder);
        }
        screen.resume(world);
        self.screen = Some(screen);
    }

    fn on_exit(&mut self, ctx: &mut StateContext) {
        let view = view_mut(ctx.world);
        view.held = None;
        if self.id == SHOP_SCREEN {
            view.shop_up = false;
            end_shop(ctx.world);
        }
    }

    fn on_resume(&mut self, ctx: &mut StateContext) {
        if let Some(screen) = self.screen {
            screen.resume(ctx.world);
        }
    }

    fn update(&mut self, world: &mut World) {
        let Some(mut screen) = self.screen else {
            return;
        };
        let root = screen.root;
        if self.id == SHOP_SCREEN && visit(world).is_none() && !self.closing {
            self.closing = screens::pop(world);
        }
        let held = world
            .get_resource::<ItemsView>()
            .is_some_and(|view| view.held.is_some());
        if held && screens::idle(world) && cancelled(world, root) {
            // Cancel lets go first; the modal screen's own turn would pop.
            view_mut(world).held = None;
            popup::tick(world, root);
        } else {
            screen.update(world);
            self.screen = Some(screen);
            if screens::idle(world) {
                Self::react(world, root);
            }
        }
        screen_build::refresh(world, root);
    }
}

#[cfg(test)]
#[path = "tests/items.rs"]
mod tests;
