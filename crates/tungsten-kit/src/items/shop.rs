//! Shops (redo 8.1's second plan, P8-3, LW9): the `shops` kind's stock,
//! prices scaled by a counter, buying and selling at once during the one
//! visit a `shop` cue action or a game's call opens.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use tungsten_core::save;
use tungsten_core::{DataRegistry, Entity, EventQueue, MissingAsset, Plugin, Schedule, World};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_kind, describe_shape,
    round_trip,
};
use crate::foundation::tags::{TagFilter, Tags, valid_tag};
use crate::items::base::{ItemBase, MAX_STACK};
use crate::items::inventory::{
    ItemLedger, check_base_ref, give, inventory, ledger_mut, takes_whole,
};
use crate::items::item::Item;
use crate::react::counters::Counters;
use crate::react::cues::Target;
use crate::story::condition::Condition;

/// The data kind.
pub(crate) const SHOPS: &str = "shops";
/// The restore hook's name.
const RESTORE_HOOK: &str = "kit.shops";
/// The most entries a shop stocks.
const MAX_STOCK: usize = 256;
/// The greatest bound of a price scale.
const MAX_FACTOR: f32 = 10.0;

/// Whose counters hold a shop's currency.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pays {
    /// The game's own counters.
    #[default]
    Game,
    /// The customer's.
    Other,
}

/// One entry of a shop's stock: a base, how many (none: unlimited), its
/// price (none: the base's) and when it shows, a condition with the keeper
/// as `self` and the customer as `other`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ShopEntryFields", into = "ShopEntryFields")]
pub struct ShopEntry {
    item: String,
    count: Option<u32>,
    price: Option<u32>,
    when: Option<Condition>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShopEntryFields {
    item: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    price: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    when: Option<Condition>,
}

impl TryFrom<ShopEntryFields> for ShopEntry {
    type Error = String;

    fn try_from(fields: ShopEntryFields) -> Result<Self, Self::Error> {
        if let Some(count) = fields.count
            && !(1..=MAX_STACK).contains(&count)
        {
            return Err(format!("count: 1 to {MAX_STACK} (got {count})"));
        }
        Ok(Self {
            item: fields.item,
            count: fields.count,
            price: fields.price,
            when: fields.when,
        })
    }
}

impl From<ShopEntry> for ShopEntryFields {
    fn from(entry: ShopEntry) -> Self {
        Self {
            item: entry.item,
            count: entry.count,
            price: entry.price,
            when: entry.when,
        }
    }
}

impl ShopEntry {
    /// The base it sells.
    #[must_use]
    pub fn item(&self) -> &str {
        &self.item
    }

    /// How many it holds; `None` for unlimited.
    #[must_use]
    pub fn count(&self) -> Option<u32> {
        self.count
    }

    /// Its price before the scale; `None` for the base's.
    #[must_use]
    pub fn price(&self) -> Option<u32> {
        self.price
    }

    /// When it shows.
    #[must_use]
    pub fn when(&self) -> Option<&Condition> {
        self.when.as_ref()
    }
}

/// A shop's price scale: the factor `clamp(1 + per × count, min, max)`,
/// `count` a counter of the keeper's (`self`), the customer's (`other`) or
/// the game's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PriceScaleFields", into = "PriceScaleFields")]
pub struct PriceScale {
    counter: String,
    target: Target,
    per: f32,
    min: f32,
    max: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PriceScaleFields {
    counter: String,
    #[serde(default)]
    target: Target,
    #[serde(deserialize_with = "crate::foundation::finite")]
    per: f32,
    min: f32,
    max: f32,
}

impl TryFrom<PriceScaleFields> for PriceScale {
    type Error = String;

    fn try_from(fields: PriceScaleFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.counter) {
            return Err(format!(
                "counter: `{}` is not a counter name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.counter
            ));
        }
        for (field, bound) in [("min", fields.min), ("max", fields.max)] {
            if !(bound.is_finite() && bound > 0.0 && bound <= MAX_FACTOR) {
                return Err(format!("{field}: in (0, {MAX_FACTOR}] (got {bound})"));
            }
        }
        if fields.min > fields.max {
            return Err(format!("min: {} exceeds `max` {}", fields.min, fields.max));
        }
        Ok(Self {
            counter: fields.counter,
            target: fields.target,
            per: fields.per,
            min: fields.min,
            max: fields.max,
        })
    }
}

impl From<PriceScale> for PriceScaleFields {
    fn from(scale: PriceScale) -> Self {
        Self {
            counter: scale.counter,
            target: scale.target,
            per: scale.per,
            min: scale.min,
            max: scale.max,
        }
    }
}

/// A shop: a `shops` entry, one a file. Its name, the counter it is paid
/// in and whose counters hold it, its stock, the bases it buys by their
/// tags and the share of its price a sale pays, and the counter that
/// scales its prices.
///
/// ```json
/// { "name": "Tamsin's Chandlery", "currency": "coins",
///   "stock": [{ "item": "oil_flask" },
///             { "item": "oilskin_hood", "count": 2, "price": 12,
///               "when": { "counter": { "name": "day", "min": 4, "target": "game" } } }],
///   "buys": { "any": ["item.flask"] }, "sell": 0.5,
///   "scale": { "counter": "trust", "per": -0.1, "min": 0.5, "max": 1 } }
/// ```
///
/// A buy costs `max(1, round(p × factor))` for an entry's price `p` above
/// 0, nothing at 0; a sale pays `floor(q × sell)` a unit, `q` what the
/// shop would charge for the base now, so no buy-and-sell loop gains coin
/// while the factor holds. The load checks the conditions, the tag filter
/// and each base.
///
/// # Examples
///
/// ```
/// use tungsten_kit::Shop;
///
/// let chandlery: Shop = serde_json::from_str(
///     r#"{ "name": "Chandlery", "stock": [{ "item": "oil_flask", "price": 3 }] }"#,
/// )
/// .unwrap();
/// assert_eq!((chandlery.name(), chandlery.currency(), chandlery.sell()), ("Chandlery", "coins", 0.5));
/// assert_eq!(chandlery.stock().next().unwrap().price(), Some(3));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ShopFields", into = "ShopFields")]
pub struct Shop {
    name: String,
    currency: String,
    pays: Pays,
    stock: Vec<ShopEntry>,
    buys: Option<TagFilter>,
    sell: f32,
    scale: Option<PriceScale>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ShopFields {
    name: String,
    #[serde(default = "coins")]
    currency: String,
    #[serde(default)]
    pays: Pays,
    #[serde(default)]
    stock: Vec<ShopEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    buys: Option<TagFilter>,
    #[serde(default = "half", deserialize_with = "crate::foundation::fraction")]
    sell: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scale: Option<PriceScale>,
}

fn coins() -> String {
    "coins".to_owned()
}

fn half() -> f32 {
    0.5
}

impl TryFrom<ShopFields> for Shop {
    type Error = String;

    fn try_from(fields: ShopFields) -> Result<Self, Self::Error> {
        if fields.name.is_empty() {
            return Err("name: the shop's name".into());
        }
        if !valid_tag(&fields.currency) {
            return Err(format!(
                "currency: `{}` is not a counter name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.currency
            ));
        }
        if fields.stock.len() > MAX_STOCK {
            return Err(format!("stock: at most {MAX_STOCK} entries"));
        }
        let mut listed = BTreeSet::new();
        for (i, entry) in fields.stock.iter().enumerate() {
            if !listed.insert(entry.item.as_str()) {
                return Err(format!("stock.{i}: `{}` listed twice", entry.item));
            }
        }
        Ok(Self {
            name: fields.name,
            currency: fields.currency,
            pays: fields.pays,
            stock: fields.stock,
            buys: fields.buys,
            sell: fields.sell,
            scale: fields.scale,
        })
    }
}

impl From<Shop> for ShopFields {
    fn from(shop: Shop) -> Self {
        Self {
            name: shop.name,
            currency: shop.currency,
            pays: shop.pays,
            stock: shop.stock,
            buys: shop.buys,
            sell: shop.sell,
            scale: shop.scale,
        }
    }
}

impl Shop {
    /// Its name, shown.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The counter its prices are paid in.
    #[must_use]
    pub fn currency(&self) -> &str {
        &self.currency
    }

    /// Whose counters hold its currency.
    #[must_use]
    pub fn pays(&self) -> Pays {
        self.pays
    }

    /// Its stock, in order.
    pub fn stock(&self) -> impl Iterator<Item = &ShopEntry> {
        self.stock.iter()
    }

    /// The share of its price a sale pays.
    #[must_use]
    pub fn sell(&self) -> f32 {
        self.sell
    }

    fn entry(&self, base: &str) -> Option<&ShopEntry> {
        self.stock.iter().find(|entry| entry.item == base)
    }
}

/// The `shops` kind's check: each entry's base and condition, the tag
/// filter's tags.
fn check_shop(shop: &Shop, world: &World) -> Result<(), MissingAsset> {
    for (i, entry) in shop.stock.iter().enumerate() {
        check_base_ref(world, &format!("stock.{i}.item"), &entry.item)?;
        if let Some(when) = &entry.when {
            when.check(world, &format!("stock.{i}.when"))?;
        }
    }
    if let Some(buys) = &shop.buys {
        buys.check(world, "buys")?;
    }
    Ok(())
}

/// Why a buy or a sale was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShopRefusal {
    /// No visit is open.
    Closed,
    /// The shop does not list the base, or its entry's `when` fails.
    Hidden,
    /// None of it is left.
    SoldOut,
    /// The payer's counter holds less than the price.
    Poor,
    /// The customer's bag does not take it.
    Full,
    /// The shop buys no item of that base's tags, or the cell is empty.
    NotBought,
}

/// The one shop visit open: a World resource, unsaved, which
/// [`open_shop`] inserts and [`end_shop`] removes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShopVisit {
    shop: String,
    keeper: Entity,
    customer: Entity,
}

impl ShopVisit {
    /// The shop's `shops` ID.
    #[must_use]
    pub fn shop(&self) -> &str {
        &self.shop
    }

    /// Who keeps it.
    #[must_use]
    pub fn keeper(&self) -> Entity {
        self.keeper
    }

    /// Who visits it.
    #[must_use]
    pub fn customer(&self) -> Entity {
        self.customer
    }
}

/// A visit opened.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShopOpened {
    /// The shop's `shops` ID.
    pub shop: String,
    /// Who keeps it.
    pub keeper: Entity,
    /// Who visits it.
    pub customer: Entity,
}

/// A visit ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ShopClosed {
    /// The shop's `shops` ID.
    pub shop: String,
}

fn shop_data<'w>(world: &'w World, id: &str) -> Option<&'w Shop> {
    world.get_resource::<DataRegistry>()?.get::<Shop>(id)
}

/// Opens a visit of shop `id`, kept by `keeper`, for `customer`: false,
/// opening nothing, while one is open, for an unknown shop or an entity
/// that is not alive.
pub fn open_shop(world: &mut World, id: &str, keeper: Entity, customer: Entity) -> bool {
    if world.has_resource::<ShopVisit>()
        || shop_data(world, id).is_none()
        || !world.is_alive(keeper)
        || !world.is_alive(customer)
    {
        return false;
    }
    world.insert_resource(ShopVisit {
        shop: id.to_owned(),
        keeper,
        customer,
    });
    if let Some(queue) = world.get_resource_mut::<EventQueue<ShopOpened>>() {
        queue.send(ShopOpened {
            shop: id.to_owned(),
            keeper,
            customer,
        });
    }
    true
}

/// The visit open, when one is.
#[must_use]
pub fn visit(world: &World) -> Option<&ShopVisit> {
    world.get_resource::<ShopVisit>()
}

/// Ends the open visit; whether one was open.
pub fn end_shop(world: &mut World) -> bool {
    let Some(visit) = world.remove_resource::<ShopVisit>() else {
        return false;
    };
    if let Some(queue) = world.get_resource_mut::<EventQueue<ShopClosed>>() {
        queue.send(ShopClosed { shop: visit.shop });
    }
    true
}

/// The count a scale reads, 0 when its holder holds none.
fn scale_count(world: &World, visit: &ShopVisit, scale: &PriceScale) -> i64 {
    let counters = match scale.target {
        Target::Game => world.get_resource::<Counters>(),
        Target::Subject => world.get::<Counters>(visit.keeper),
        Target::Other => world.get::<Counters>(visit.customer),
    };
    counters.map_or(0, |counters| counters.get(&scale.counter))
}

/// The shop's price factor now.
fn factor(world: &World, visit: &ShopVisit, shop: &Shop) -> f64 {
    shop.scale.as_ref().map_or(1.0, |scale| {
        let count = scale_count(world, visit, scale) as f64;
        (1.0 + f64::from(scale.per) * count).clamp(f64::from(scale.min), f64::from(scale.max))
    })
}

/// What the shop charges for `base` now: its entry's price, else the
/// base's, scaled; `max(1, round(p × factor))` above 0.
fn charge(world: &World, visit: &ShopVisit, shop: &Shop, base: &str) -> Option<u32> {
    let listed = shop.entry(base).and_then(|entry| entry.price);
    let price = match listed {
        Some(price) => price,
        None => world
            .get_resource::<DataRegistry>()?
            .get::<ItemBase>(base)?
            .price(),
    };
    if price == 0 {
        return Some(0);
    }
    let scaled = (f64::from(price) * factor(world, visit, shop)).round();
    Some(scaled.clamp(1.0, f64::from(u32::MAX)) as u32)
}

/// Whether an entry shows now, its `when` read with the keeper as `self`
/// and the customer as `other`.
fn shown(world: &World, visit: &ShopVisit, entry: &ShopEntry) -> bool {
    entry
        .when
        .as_ref()
        .is_none_or(|when| when.holds(world, Some(visit.keeper), Some(visit.customer)))
}

/// How many of an entry are left: its count less what the ledger has sold;
/// `None` for unlimited.
fn left(world: &World, visit: &ShopVisit, entry: &ShopEntry) -> Option<u32> {
    let sold = world
        .get_resource::<ItemLedger>()
        .map_or(0, |ledger| ledger.sold(&visit.shop, &entry.item));
    entry.count.map(|count| count.saturating_sub(sold))
}

/// The open visit's buy price of `base`: shown, scaled; `None` without a
/// visit or for a base the shop does not show.
#[must_use]
pub fn price_of(world: &World, base: &str) -> Option<u32> {
    let visit = visit(world)?;
    let shop = shop_data(world, &visit.shop)?;
    let entry = shop.entry(base)?;
    if !shown(world, visit, entry) {
        return None;
    }
    charge(world, visit, shop, base)
}

/// The open visit's shown stock, in order: each base, its price now and
/// how many are left (`None` for unlimited).
#[must_use]
pub fn shop_stock(world: &World) -> Vec<(String, u32, Option<u32>)> {
    let Some(visit) = visit(world) else {
        return Vec::new();
    };
    let Some(shop) = shop_data(world, &visit.shop) else {
        return Vec::new();
    };
    shop.stock
        .iter()
        .filter(|entry| shown(world, visit, entry))
        .filter_map(|entry| {
            let price = charge(world, visit, shop, &entry.item)?;
            Some((entry.item.clone(), price, left(world, visit, entry)))
        })
        .collect()
}

/// The payer's count of the currency.
fn purse(world: &World, visit: &ShopVisit, shop: &Shop) -> i64 {
    let counters = match shop.pays {
        Pays::Game => world.get_resource::<Counters>(),
        Pays::Other => world.get::<Counters>(visit.customer),
    };
    counters.map_or(0, |counters| counters.get(&shop.currency))
}

/// Adds `amount` to the payer's count of the currency.
fn pay(world: &mut World, visit: &ShopVisit, shop: &Shop, amount: i64) {
    let counters = match shop.pays {
        Pays::Game => {
            if !world.has_resource::<Counters>() {
                world.insert_resource(Counters::new());
            }
            world.resource_mut::<Counters>()
        }
        Pays::Other => {
            if !world.has::<Counters>(visit.customer) {
                world.insert(visit.customer, Counters::new());
            }
            world.get_mut::<Counters>(visit.customer).expect("inserted")
        }
    };
    let now = counters.get(&shop.currency);
    counters.set(&shop.currency, now.saturating_add(amount));
}

/// Buys one of `base` at once for the visit's customer: every check first
/// (shown, left, affordable, room), then the payment, the item into the
/// customer's bag and the sale counted in the [`ItemLedger`].
pub fn buy(world: &mut World, base: &str) -> Result<(), ShopRefusal> {
    let Some(visit) = visit(world).cloned() else {
        return Err(ShopRefusal::Closed);
    };
    let Some(shop) = shop_data(world, &visit.shop).cloned() else {
        return Err(ShopRefusal::Closed);
    };
    let Some(entry) = shop.entry(base) else {
        return Err(ShopRefusal::Hidden);
    };
    if !shown(world, &visit, entry) {
        return Err(ShopRefusal::Hidden);
    }
    if left(world, &visit, entry) == Some(0) {
        return Err(ShopRefusal::SoldOut);
    }
    let Some(price) = charge(world, &visit, &shop, base) else {
        return Err(ShopRefusal::Hidden);
    };
    if purse(world, &visit, &shop) < i64::from(price) {
        return Err(ShopRefusal::Poor);
    }
    if !takes_whole(world, visit.customer, &Item::new(base)) {
        return Err(ShopRefusal::Full);
    }
    pay(world, &visit, &shop, -i64::from(price));
    give(world, visit.customer, base, 1);
    ledger_mut(world).add_sold(&visit.shop, base, 1);
    Ok(())
}

/// Sells up to `count` of the item in the visit's customer's cell `cell`
/// at once: the shop's `buys` must pass its base's tags; it pays
/// `floor(q × sell)` a unit, `q` its buy price now. What it paid.
pub fn sell(world: &mut World, cell: usize, count: u32) -> Result<u32, ShopRefusal> {
    let Some(visit) = visit(world).cloned() else {
        return Err(ShopRefusal::Closed);
    };
    let Some(shop) = shop_data(world, &visit.shop).cloned() else {
        return Err(ShopRefusal::Closed);
    };
    let Some(item) = inventory(world, visit.customer).and_then(|bag| bag.cell(cell)) else {
        return Err(ShopRefusal::NotBought);
    };
    let base = item.base().to_owned();
    let units = count.min(item.count());
    let Some(data) = world
        .get_resource::<DataRegistry>()
        .and_then(|data| data.get::<ItemBase>(&base))
    else {
        return Err(ShopRefusal::NotBought);
    };
    let tags = Tags::new(data.tags().map(str::to_owned));
    if units == 0
        || !shop
            .buys
            .as_ref()
            .is_some_and(|buys| buys.matches(Some(&tags)))
    {
        return Err(ShopRefusal::NotBought);
    }
    let q = charge(world, &visit, &shop, &base).unwrap_or(0);
    let each = (f64::from(q) * f64::from(shop.sell)).floor() as u32;
    let paid = each.saturating_mul(units);
    world
        .get_mut::<crate::Inventory>(visit.customer)
        .expect("read above")
        .spend(cell, units);
    pay(world, &visit, &shop, i64::from(paid));
    Ok(paid)
}

/// The restore hook: a visit open at the load ends.
fn end_visit(world: &mut World) {
    end_shop(world);
}

/// Registers the `shops` kind with its check, [`ShopOpened`] and
/// [`ShopClosed`], and a restore hook that ends a visit, since a visit is
/// not saved. It adds no system.
#[derive(Debug, Default, Clone, Copy)]
pub struct ShopsPlugin;

impl Plugin for ShopsPlugin {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        world.register_data_kind_with_assets(SHOPS, check_shop);
        world.register_event::<ShopOpened>();
        world.register_event::<ShopClosed>();
        save::on_restore(world, RESTORE_HOOK, end_visit);
        describe_shops(world);
    }
}

/// The kind and its shapes.
fn describe_shops(world: &mut World) {
    describe_kind(
        world,
        SHOPS,
        "A `shop`: its stock, scaled prices, what it buys and the share of its price a sale pays.",
    );
    let shapes = [
        ComponentDescription::new(
            "shop",
            "A `shops` entry: stock, a currency counter, prices scaled by a counter, and what it buys back.",
        )
        .field(FieldDescription::new("name", FieldType::Text, "its name, shown"))
        .field(FieldDescription::new("currency", FieldType::Text, "the counter its prices are paid in, dotted, undeclared").default(r#""coins""#))
        .field(FieldDescription::new("pays", FieldType::Enum(&["game", "other"]), "whose counters hold the currency: the game's or the customer's").default(r#""game""#))
        .field(FieldDescription::new("stock", FieldType::List(Box::new(FieldType::Shape("shop_entry"))), "at most 256 entries, a base listed once; the load checks each").default("[]"))
        .field(FieldDescription::new("buys", FieldType::Filter, "a filter over a base's tags: what it buys; none buys nothing; the load checks its tags").default("null"))
        .field(
            FieldDescription::new("sell", FieldType::F32 { min: Bound::Inclusive(0.0), max: Bound::Inclusive(1.0) }, "the share of its buy price now a sale pays a unit, floored")
                .default("0.5"),
        )
        .field(FieldDescription::new("scale", FieldType::Shape("price_scale"), "a counter that scales every price; none: 1").default("null"))
        .sample(r#"{ "name": "Chandlery" }"#)
        .roundtrip(round_trip::<Shop>),
        ComponentDescription::new(
            "shop_entry",
            "A base a shop sells: how many, its price, and when it shows, the keeper `self` and the customer `other`.",
        )
        .field(FieldDescription::new("item", FieldType::Text, "the `item_bases` entry; the load checks it"))
        .field(
            FieldDescription::new("count", FieldType::U32 { min: Some(1), max: Some(MAX_STACK) }, "how many it holds, less what the ledger has sold; none: unlimited")
                .default("null"),
        )
        .field(FieldDescription::new("price", FieldType::U32 { min: None, max: None }, "whole coins before the scale; none: the base's; 0 is free").default("null"))
        .field(FieldDescription::new("when", FieldType::Shape("condition"), "it shows, and sells, only while this holds").default("null"))
        .sample(r#"{ "item": "oil_flask" }"#)
        .roundtrip(round_trip::<ShopEntry>),
        ComponentDescription::new(
            "price_scale",
            "A shop's price factor: clamp(1 + per × count, min, max), the count a counter of the keeper's, the customer's or the game's.",
        )
        .field(FieldDescription::new("counter", FieldType::Text, "the counter read, 0 when its holder holds none"))
        .field(FieldDescription::new("target", FieldType::Enum(&["self", "other", "game"]), "the keeper's, the customer's or the game's").default(r#""self""#))
        .field(FieldDescription::new("per", FieldType::F32 { min: Bound::Open, max: Bound::Open }, "the factor's change for each count"))
        .field(FieldDescription::new("min", FieldType::F32 { min: Bound::Exclusive(0.0), max: Bound::Inclusive(10.0) }, "the least factor, at most `max`"))
        .field(FieldDescription::new("max", FieldType::F32 { min: Bound::Exclusive(0.0), max: Bound::Inclusive(10.0) }, "the greatest factor"))
        .sample(r#"{ "counter": "trust", "per": -0.05, "min": 0.5, "max": 10 }"#)
        .roundtrip(round_trip::<PriceScale>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
}

/// The load's check of a shop ID a `shop` action names.
pub(crate) fn check_shop_ref(world: &World, field: &str, id: &str) -> Result<(), MissingAsset> {
    match world.get_resource::<DataRegistry>() {
        Some(_) if shop_data(world, id).is_none() => Err(MissingAsset::new(field, "shop", id)),
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "../tests/shop.rs"]
mod tests;
