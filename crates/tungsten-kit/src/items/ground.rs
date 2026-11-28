//! Ground items (redo 8.1's second plan, PA-13): an item lying in the
//! world on a pickup, collected whole into a collector's bag, dropped from
//! a bag onto its base's `ground` prefab, and labelled each frame in its
//! rarity's theme text style.

use glam::Vec2;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use tungsten_core::text::{TextOutline, WorldLabel, WorldLabels};
use tungsten_core::ui::{Theme, UiTree};
use tungsten_core::{
    DataRegistry, Entity, Inspectable, MissingAsset, Schedule, SpawnContext, World, system,
};

use crate::foundation::describe::{
    ARPG, ComponentDescription, FieldDescription, FieldType, RPG, describe,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::Phase;
use crate::items::base::{ItemBase, MAX_STACK};
use crate::items::inventory::{Inventory, takes_whole};
use crate::items::item::{Item, built, check_item};
use crate::motion::drive::centre_of;
use crate::motion::slide::step_dt;

/// [`ItemsPlugin`](crate::ItemsPlugin)'s system that counts down a drop's
/// refusal of its dropper, in the tick phase.
pub const GROUND_ITEM_TICK: &str = "ground_item_tick";
/// [`ItemsPlugin`](crate::ItemsPlugin)'s system that labels ground items,
/// in the present phase.
pub const GROUND_ITEM_LABELS: &str = "ground_item_labels";

/// The scene component and its saved name.
pub(crate) const GROUND_ITEM: &str = "ground_item";
/// Game seconds a drop refuses its dropper.
const REFUSAL: f32 = 1.0;
/// The outline a label takes without its rarity's style.
const OUTLINE: TextOutline = TextOutline::new([12, 12, 16, 255], 1.0);

/// An item lying in the world: a [`Pickup`](crate::Pickup)'s entity that
/// holds one goes only to a collector whose [`Inventory`] takes it whole,
/// never its dropper within its first second, and lands in that bag at
/// once; a labelled one shows its name, `×n` past one, at its centre in
/// the theme's text style `rarity.<rarity>`.
///
/// Scene name `ground_item`: `{ "item": "oil_flask", "count": 2, "label": true,
/// "offset": [0, -12] }`, `item` a base's ID or an item, `count` beside an
/// ID alone; the load checks the item. Saved with the item whole.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{GroundItem, Item};
///
/// let flasks = GroundItem::new(Item::new("oil_flask").with_count(2));
/// assert_eq!(flasks.item().count(), 2);
/// assert!(flasks.labelled());
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct GroundItem {
    item: Item,
    label: bool,
    offset: Vec2,
    /// Not saved: an entity is not data.
    dropper: Option<Entity>,
    /// Game seconds it still refuses its dropper; not saved.
    refusing: f32,
    /// Its label's text and style name, built once.
    shown: Option<(String, String)>,
}

impl GroundItem {
    /// `item` lying labelled, its label at its centre.
    #[must_use]
    pub fn new(item: Item) -> Self {
        Self {
            item,
            label: true,
            offset: Vec2::ZERO,
            dropper: None,
            refusing: 0.0,
            shown: None,
        }
    }

    /// Unlabelled.
    #[must_use]
    pub fn unlabelled(mut self) -> Self {
        self.label = false;
        self
    }

    /// Its label shifted by `offset` UI units.
    #[must_use]
    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// The item it holds.
    #[must_use]
    pub fn item(&self) -> &Item {
        &self.item
    }

    /// Whether it shows a label.
    #[must_use]
    pub fn labelled(&self) -> bool {
        self.label
    }

    /// The entity it refuses for now, its dropper, while it refuses one.
    #[must_use]
    pub fn refused(&self) -> Option<Entity> {
        self.dropper.filter(|_| self.refusing > 0.0)
    }

    /// Whether `collector` may take it: not its dropper while it refuses
    /// one, and with a bag that takes it whole.
    pub(crate) fn accepts(&self, world: &World, collector: Entity) -> bool {
        self.refused() != Some(collector) && takes_whole(world, collector, &self.item)
    }
}

/// A ground item as a save writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GroundItemSave {
    item: Item,
    #[serde(default = "yes")]
    label: bool,
    #[serde(default, with = "crate::foundation::vec2")]
    offset: Vec2,
}

fn yes() -> bool {
    true
}

impl Serialize for GroundItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        GroundItemSave {
            item: self.item.clone(),
            label: self.label,
            offset: self.offset,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for GroundItem {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let save = GroundItemSave::deserialize(deserializer)?;
        Ok(Self {
            label: save.label,
            offset: save.offset,
            ..Self::new(save.item)
        })
    }
}

/// `components.ground_item` as a scene writes it.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "GroundItemFields")]
struct GroundItemDef {
    item: GroundRef,
    count: u32,
    label: bool,
    #[serde(with = "crate::foundation::vec2")]
    offset: Vec2,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GroundItemFields {
    item: GroundRef,
    #[serde(default)]
    count: Option<u32>,
    #[serde(default = "yes")]
    label: bool,
    #[serde(default, with = "crate::foundation::vec2")]
    offset: Vec2,
}

impl TryFrom<GroundItemFields> for GroundItemDef {
    type Error = String;

    fn try_from(fields: GroundItemFields) -> Result<Self, Self::Error> {
        let count = match (&fields.item, fields.count) {
            (GroundRef::Item(_), Some(_)) => {
                return Err(
                    "count: an item holds its own count; `count` goes beside a base's ID".into(),
                );
            }
            (_, Some(count)) if !(1..=MAX_STACK).contains(&count) => {
                return Err(format!("count: 1 to {MAX_STACK} (got {count})"));
            }
            (_, count) => count.unwrap_or(1),
        };
        if !fields.offset.is_finite() {
            return Err("offset: must be finite".into());
        }
        Ok(Self {
            item: fields.item,
            count,
            label: fields.label,
            offset: fields.offset,
        })
    }
}

/// A ground item's `item`: a base's ID or the item.
enum GroundRef {
    Base(String),
    Item(Item),
}

impl Serialize for GroundRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            GroundRef::Base(base) => base.serialize(serializer),
            GroundRef::Item(item) => item.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for GroundRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::String(base) => Ok(GroundRef::Base(base)),
            object @ Value::Object(_) => serde_json::from_value(object)
                .map(GroundRef::Item)
                .map_err(D::Error::custom),
            other => Err(D::Error::custom(format!(
                "an item, or its base's ID (got {other})"
            ))),
        }
    }
}

impl GroundItemDef {
    fn item(&self) -> Item {
        match &self.item {
            GroundRef::Base(base) => Item::new(base.clone()).with_count(self.count),
            GroundRef::Item(item) => item.clone(),
        }
    }
}

/// The `ground_item` component's check: the item's base and affixes.
fn check_ground(def: &GroundItemDef, world: &World) -> Result<(), MissingAsset> {
    check_item(world, "item", &def.item())
}

/// Drops `count` of the item in `holder`'s cell `cell` at once, onto its
/// base's `ground` prefab spawned at the holder's centre, refusing the
/// holder for a second of game time; the new entity, or `None` for an
/// empty cell, a count of 0, or a base with no `ground`.
pub fn drop_item(world: &mut World, holder: Entity, cell: usize, count: u32) -> Option<Entity> {
    let item = world.get::<Inventory>(holder)?.cell(cell)?.clone();
    let ground = world
        .get_resource::<DataRegistry>()?
        .get::<ItemBase>(item.base())?
        .ground()?
        .to_owned();
    let count = count.min(item.count());
    if count == 0 {
        return None;
    }
    let at = centre_of(world, holder).unwrap_or(Vec2::ZERO);
    let entity = match world.spawn_prefab(&ground, at) {
        Ok(entity) => entity,
        Err(error) => {
            log::warn!("drop_item: `{ground}` did not spawn: {error}");
            return None;
        }
    };
    let mut dropped = item.clone();
    if count < item.count() {
        dropped.set_count(count);
        dropped.set_uid(0);
    }
    let lying = world.get::<GroundItem>(entity).map_or_else(
        || GroundItem::new(dropped.clone()),
        |prefab| GroundItem {
            item: dropped.clone(),
            shown: None,
            ..prefab.clone()
        },
    );
    world.insert(
        entity,
        GroundItem {
            dropper: Some(holder),
            refusing: REFUSAL,
            ..lying
        },
    );
    let bag = world.get_mut::<Inventory>(holder).expect("read above");
    bag.spend(cell, count);
    Some(entity)
}

/// Counts each drop's refusal of its dropper down by the step.
fn ground_item_tick(world: &mut World) {
    if !world.has_any::<GroundItem>() {
        return;
    }
    let dt = step_dt(world);
    for (_, ground) in world.query_mut::<(Entity, &mut GroundItem)>() {
        if ground.refusing > 0.0 {
            ground.refusing = (ground.refusing - dt).max(0.0);
        }
    }
}

/// [`GROUND_ITEM_LABELS`]' state between frames: the labelled entities,
/// and the theme used without a UI tree.
#[derive(Default)]
struct Labeller {
    held: Vec<Entity>,
    fallback: Option<Theme>,
}

impl Labeller {
    /// Pushes one world label a labelled ground item: its built name, `×n`
    /// past one, at its centre plus its offset, in the theme's text style
    /// `rarity.<rarity>`, else the theme's base text with a dark outline.
    fn run(&mut self, world: &mut World) {
        if !world.has_any::<GroundItem>() || !world.has_resource::<WorldLabels>() {
            return;
        }
        self.held.clear();
        self.held.extend(
            world
                .query::<(Entity, &GroundItem)>()
                .filter(|(_, ground)| ground.label)
                .map(|(entity, _)| entity),
        );
        for i in 0..self.held.len() {
            let entity = self.held[i];
            if world
                .get::<GroundItem>(entity)
                .is_some_and(|ground| ground.shown.is_none())
            {
                let item = world.get::<GroundItem>(entity).expect("held").item.clone();
                let name = built(world, &item)
                    .map_or_else(|| item.base().to_owned(), |made| made.name().to_owned());
                let text = if item.count() > 1 {
                    format!("{name} \u{d7}{}", item.count())
                } else {
                    name
                };
                let style = format!("rarity.{}", item.rarity());
                world.get_mut::<GroundItem>(entity).expect("held").shown = Some((text, style));
            }
            let Some(anchor) = centre_of(world, entity) else {
                continue;
            };
            let ground = world.get::<GroundItem>(entity).expect("held");
            let (text, style) = ground.shown.as_ref().expect("built above");
            let theme = match world.get_resource::<UiTree>() {
                Some(tree) => tree.theme(),
                None => self.fallback.get_or_insert_with(Theme::default),
            };
            let base = theme.base_text();
            let label = match theme.text_style(style) {
                Some(named) => {
                    let mut label = WorldLabel::new(
                        text.clone(),
                        anchor,
                        named.text.family.clone().unwrap_or(base.family),
                        named.text.size.unwrap_or(base.size),
                    )
                    .with_color(named.text.color.unwrap_or(base.color));
                    if let Some(outline) = named.outline {
                        label = label.with_outline(outline);
                    }
                    if let Some(shadow) = named.shadow {
                        label = label.with_shadow(shadow);
                    }
                    label
                }
                None => WorldLabel::new(text.clone(), anchor, base.family, base.size)
                    .with_color(base.color)
                    .with_outline(OUTLINE),
            }
            .with_offset(ground.offset);
            world.resource_mut::<WorldLabels>().push(label);
        }
    }
}

impl Inspectable for GroundItem {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "item",
                format!("{} ×{}", self.item.base(), self.item.count()),
            ),
            ("label", self.label.to_string()),
        ]
    }
}

/// The `ground_item` round trip: through the scene form.
fn ground_round_trip(value: Value) -> Result<Value, String> {
    let def: GroundItemDef = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let mut out = serde_json::to_value(def).map_err(|e| e.to_string())?;
    if let Some(object) = out.as_object_mut()
        && object.get("item").is_some_and(Value::is_object)
    {
        object.remove("count");
    }
    Ok(out)
}

/// Registers the `ground_item` component, saved at version 1, its systems
/// and its description.
pub(crate) fn build_ground(schedule: &mut Schedule, world: &mut World) {
    register_rows::<GroundItem>(world, "GroundItem");
    world.register_saved::<GroundItem>(GROUND_ITEM);
    world.register_component_with_assets(
        GROUND_ITEM,
        check_ground,
        |def: GroundItemDef, _: &SpawnContext<'_>| {
            let lying = GroundItem::new(def.item()).with_offset(def.offset);
            (if def.label { lying } else { lying.unlabelled() },)
        },
    );
    schedule.add(
        Phase::Tick.stage(),
        Phase::Tick.join(system(GROUND_ITEM_TICK, ground_item_tick)),
    );
    let mut labeller = Labeller::default();
    schedule.add(
        Phase::Present.stage(),
        Phase::Present.join(system(GROUND_ITEM_LABELS, move |world: &mut World| {
            labeller.run(world);
        })),
    );
    describe(
        world,
        ComponentDescription::new(
            GROUND_ITEM,
            "An item lying on a pickup, collected whole into a bag and labelled in its rarity's text style.",
        )
        .field(FieldDescription::new(
            "item",
            FieldType::Shape("item"),
            "the item, or its base's ID for some of it; the load checks it",
        ))
        .field(
            FieldDescription::new(
                "count",
                FieldType::U32 {
                    min: Some(1),
                    max: Some(MAX_STACK),
                },
                "beside a base's ID: how many lie there",
            )
            .default("1"),
        )
        .field(
            FieldDescription::new(
                "label",
                FieldType::Bool,
                "whether its name shows over it, `×n` past one, in the theme's text style `rarity.<rarity>`",
            )
            .default("true"),
        )
        .field(
            FieldDescription::new(
                "offset",
                FieldType::Vec2,
                "its label's shift in UI units, y down",
            )
            .default("[0.0, 0.0]"),
        )
        .event("PickedUp")
        .genre(RPG)
        .genre(ARPG)
        .sample(r#"{ "item": "oil_flask" }"#)
        .roundtrip(ground_round_trip),
    );
}

#[cfg(test)]
#[path = "../tests/ground.rs"]
mod tests;
