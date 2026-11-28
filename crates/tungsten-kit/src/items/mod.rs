//! Items (redo 8.1's second plan, P8-3, PA-12): item bases and affixes as
//! data, rolled instances and what they build into, from §4.6 of the ARPG
//! design.

pub(crate) mod affix;
pub(crate) mod base;
pub(crate) mod item;

use tungsten_core::{Plugin, Schedule, World};

use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_kind, describe_shape,
    round_trip,
};
use crate::items::affix::{AFFIXES, Affix, AffixTier, AffixWeight, check_affix};
use crate::items::base::{ITEM_BASES, ItemBase, ItemLine, RolledValueDef, WeaponBlock, check_base};
use crate::items::item::{Item, ItemWarnings, RolledAffix};

/// Registers the `item_bases` and `affixes` data kinds with their checks
/// and descriptions, and the record that warns once of each part of a
/// saved item the data no longer holds. It adds no system.
#[derive(Debug, Default, Clone, Copy)]
pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, _schedule: &mut Schedule, world: &mut World) {
        world.register_data_kind_with_assets(ITEM_BASES, check_base);
        world.register_data_kind_with_assets(AFFIXES, check_affix);
        if !world.has_resource::<ItemWarnings>() {
            world.insert_resource(ItemWarnings::default());
        }
        describe_items(world);
    }
}

/// A line's description: the forms a skill line and a resistance modifier
/// write, `local` and `text`.
fn item_line_description() -> ComponentDescription {
    ComponentDescription::new(
        "item_line",
        "A line of an item: a stat's modifier, an `added` range, a `pen` or a `resist` modifier, on its wearer or, `local`, folded into its base's block.",
    )
    .field(
        FieldDescription::new(
            "stat",
            FieldType::Text,
            "the stat its `modifier` changes; the load checks it is declared; a local line's is the block's `damage`, `use_time` or `crit_chance`",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "modifier",
            FieldType::Shape("modifier"),
            "the change, with `stat` or `resist`; in an affix's line its number may be a rolled_value; a local or scoped line may not override",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "added",
            FieldType::Object("one damage type → [min, max]"),
            "flat damage of that type on the wearer's skills, or, local, added to the block's part of it; may not wait for the hit",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "pen",
            FieldType::Object("one damage type → points"),
            "penetration on the wearer's skills for that type's parts",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "resist",
            FieldType::Tag,
            "the damage tag whose resistance its `modifier` (an `add` of -0.19 is 19 % more resistance) or `floor` changes; the load checks it",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "floor",
            FieldType::Shape("modifier"),
            "with `resist`: a change to the tag's floor",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "with",
            FieldType::Filter,
            "read only where it passes the read's context; a local `damage` line folds into the parts it passes",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "while",
            FieldType::Filter,
            "read only while it passes the wearer's tags; refused on a local or `resist` line",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "against",
            FieldType::Filter,
            "read only at a hit whose other party passes it; refused on a local line",
        )
        .default("null"),
    )
    .field(
        FieldDescription::new(
            "local",
            FieldType::Bool,
            "folded into the base's weapon block when the item is built, in the kit's modifier order; refused on a base without a block",
        )
        .default("false"),
    )
    .field(
        FieldDescription::new(
            "text",
            FieldType::Text,
            "shown in place of the generic text, `{0}` to `{3}` the affix's rolled values; at most 200 characters",
        )
        .default("null"),
    )
    .sample(r#"{ "stat": "damage", "modifier": { "pct": 0.1 } }"#)
    .roundtrip(round_trip::<ItemLine>)
}

/// The `item` round trip: a name and a count left out, which the
/// description writes as their defaults.
fn item_round_trip(value: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut out = round_trip::<Item>(value)?;
    if let Some(object) = out.as_object_mut() {
        object
            .entry("name")
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        object.entry("count").or_insert_with(|| 1.into());
    }
    Ok(out)
}

/// The two kinds and their shapes.
fn describe_items(world: &mut World) {
    describe_kind(
        world,
        ITEM_BASES,
        "An `item_base`: what every item of it shares, its slot, stack, price, lines, block and use.",
    );
    describe_kind(
        world,
        AFFIXES,
        "An `affix`: a prefix or suffix, its weights by base tag, its tiers of ranges and its lines.",
    );
    let lines = || FieldType::List(Box::new(FieldType::Shape("item_line")));
    let shapes = [
        ComponentDescription::new(
            "item_base",
            "An `item_bases` entry: its name, icon, slot, stack, price, requirement, grants, implicit lines, weapon block, use and drop.",
        )
        .field(FieldDescription::new("name", FieldType::Text, "shown for an item with no words of its own; 1 to 200 characters"))
        .field(FieldDescription::new("icon", FieldType::Text, "a manifest sprite ID; the load checks it"))
        .field(FieldDescription::new("text", FieldType::Text, "a line under its name; 1 to 200 characters").default("null"))
        .field(FieldDescription::new("tags", FieldType::List(Box::new(FieldType::Tag)), "its tags, which affix weights read; the load checks each").default("[]"))
        .field(FieldDescription::new("slot", FieldType::Text, "the slot kind it is worn in, `[a-z][a-z0-9_]*`; none: never worn").default("null"))
        .field(
            FieldDescription::new("stack", FieldType::U32 { min: Some(1), max: Some(9_999) }, "how many stack in a cell; past 1 refuses `slot` and `weapon`")
                .default("1"),
        )
        .field(FieldDescription::new("price", FieldType::U32 { min: None, max: None }, "whole coins a shop asks by default").default("0"))
        .field(FieldDescription::new("requires", FieldType::Shape("condition"), "what its wearer meets to wear it, the wearer the subject with no other").default("null"))
        .field(FieldDescription::new("grant", FieldType::List(Box::new(FieldType::Tag)), "at most 8 tags its wearer carries while it is worn; the load checks each").default("[]"))
        .field(FieldDescription::new("implicit", lines(), "at most 8 lines every item of it gives, none reading a rolled value").default("[]"))
        .field(FieldDescription::new("weapon", FieldType::Shape("weapon_block"), "its block, which local lines fold into; read by no attack yet").default("null"))
        .field(FieldDescription::new("use", FieldType::List(Box::new(FieldType::Shape("cue_action"))), "at most 16 actions run at a use, a story list on whoever it is used on, with no other; no `ability`, `loop`, `stop`, `label` or `target: other`").default("[]"))
        .field(FieldDescription::new("consume", FieldType::Bool, "whether a use spends one; true when it has a `use`").default("null"))
        .field(FieldDescription::new("ground", FieldType::Prefab, "the prefab a drop of it spawns, carrying `ground_item` and `pickup`; none: it drops nothing").default("null"))
        .sample(r#"{ "name": "Oil flask", "icon": "flask" }"#)
        .roundtrip(round_trip::<ItemBase>),
        ComponentDescription::new("weapon_block", "A base's weapon block: its damage parts, use time and crit chance.")
            .field(FieldDescription::new("damage", FieldType::Object("damage type → [min, max]"), "1 to 8 parts, each a known damage type's range with 0 <= min <= max"))
            .field(FieldDescription::new("use_time", FieldType::F32 { min: Bound::Exclusive(0.0), max: Bound::Open }, "seconds a use takes"))
            .field(
                FieldDescription::new("crit_chance", FieldType::F32 { min: Bound::Inclusive(0.0), max: Bound::Inclusive(1.0) }, "its chance to crit")
                    .default("0.0"),
            )
            .sample(r#"{ "damage": { "damage.fire": [4, 8] }, "use_time": 0.8 }"#)
            .roundtrip(round_trip::<WeaponBlock>),
        item_line_description(),
        ComponentDescription::new("rolled_value", "A number an affix's line reads from its rolled values: the `value`-th times `times`.")
            .field(FieldDescription::new("value", FieldType::U32 { min: Some(0), max: Some(3) }, "which rolled value, 0 the first; every tier rolls it"))
            .field(
                FieldDescription::new("times", FieldType::F32 { min: Bound::Open, max: Bound::Open }, "what the value is multiplied by: 0.01 turns 22 into 22 %")
                    .default("1.0"),
            )
            .sample(r#"{ "value": 0, "times": 0.01 }"#)
            .roundtrip(round_trip::<RolledValueDef>),
        ComponentDescription::new("affix", "An `affixes` entry: a prefix or suffix of a group, its weights, tiers and lines.")
            .field(FieldDescription::new("kind", FieldType::Enum(&["prefix", "suffix"]), "where it goes on an item's name"))
            .field(FieldDescription::new("group", FieldType::Text, "an item takes one affix of a group, `[a-z][a-z0-9_]*`"))
            .field(
                FieldDescription::new("weights", FieldType::List(Box::new(FieldType::Shape("affix_weight"))), "at most 16; 8.7's roll takes the first whose tag a base carries, none or 0 leaving it out")
                    .default("[]"),
            )
            .field(FieldDescription::new("tiers", FieldType::List(Box::new(FieldType::Shape("affix_tier"))), "1 to 16, ascending by level; tier 1 the first"))
            .field(FieldDescription::new("lines", lines(), "1 to 8 lines, each rolled value one every tier rolls; the load checks their names"))
            .sample(r#"{ "kind": "prefix", "group": "fire", "tiers": [{ "level": 1, "weight": 1000, "values": [[10, 16]] }], "lines": [{ "stat": "damage", "modifier": { "pct": { "value": 0, "times": 0.01 } } }] }"#)
            .roundtrip(round_trip::<Affix>),
        ComponentDescription::new("affix_tier", "An affix's tier: the least item level that rolls it, its weight, its ranges and step.")
            .field(FieldDescription::new("level", FieldType::U32 { min: Some(1), max: Some(100) }, "the least item level that rolls it"))
            .field(FieldDescription::new("weight", FieldType::F32 { min: Bound::Exclusive(0.0), max: Bound::Open }, "its share among the affix's tiers at a roll"))
            .field(FieldDescription::new("values", FieldType::List(Box::new(FieldType::Object("[min, max]"))), "1 to 4 ranges, min <= max, one rolled value each"))
            .field(
                FieldDescription::new("step", FieldType::F32 { min: Bound::Exclusive(0.0), max: Bound::Open }, "a rolled value lands on a range's least plus whole steps")
                    .default("1.0"),
            )
            .sample(r#"{ "level": 1, "weight": 1000, "values": [[10, 16]] }"#)
            .roundtrip(round_trip::<AffixTier>),
        ComponentDescription::new("affix_weight", "An affix's weight for bases carrying a tag.")
            .field(FieldDescription::new("tag", FieldType::Tag, "the base tag; the load checks it"))
            .field(FieldDescription::new("weight", FieldType::F32 { min: Bound::Inclusive(0.0), max: Bound::Open }, "its weight; 0 leaves the affix out of such bases"))
            .sample(r#"{ "tag": "item.wand", "weight": 1000 }"#)
            .roundtrip(round_trip::<AffixWeight>),
        ComponentDescription::new("item", "An item instance: its base, level, rarity, seed, rolled affixes, name and count.")
            .field(FieldDescription::new("uid", FieldType::U32 { min: None, max: None }, "its uid, 0 until a bag first takes it").default("0"))
            .field(FieldDescription::new("base", FieldType::Text, "its `item_bases` ID; the load checks it"))
            .field(FieldDescription::new("level", FieldType::U32 { min: Some(1), max: Some(100) }, "its item level, at or above each affix tier's").default("1"))
            .field(FieldDescription::new("rarity", FieldType::Text, "its rarity's name, which the theme's `rarity.<name>` text style shows").default(r#""normal""#))
            .field(FieldDescription::new("seed", FieldType::Text, "a hex u64 kept for 8.7's reroll").default(r#""0x0""#))
            .field(
                FieldDescription::new("affixes", FieldType::List(Box::new(FieldType::Shape("rolled_affix"))), "at most 12, no group twice; the load checks each against its affix")
                    .default("[]"),
            )
            .field(FieldDescription::new("name", FieldType::List(Box::new(FieldType::Text)), "at most 4 words shown in place of its base's name").default("[]"))
            .field(FieldDescription::new("count", FieldType::U32 { min: Some(1), max: Some(9_999) }, "how many it stands for, within its base's stack").default("1"))
            .sample(r#"{ "base": "ashwood_wand" }"#)
            .roundtrip(item_round_trip),
        ComponentDescription::new("rolled_affix", "One affix of an item: its ID, tier and rolled values.")
            .field(FieldDescription::new("id", FieldType::Text, "the `affixes` entry; the load checks it"))
            .field(FieldDescription::new("tier", FieldType::U32 { min: Some(1), max: Some(16) }, "its tier, 1 the lowest, at or below the item's level"))
            .field(FieldDescription::new("values", FieldType::List(Box::new(FieldType::F32 { min: Bound::Open, max: Bound::Open })), "one a range of the tier, each inside it on its step"))
            .sample(r#"{ "id": "searing", "tier": 2, "values": [22] }"#)
            .roundtrip(round_trip::<RolledAffix>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
}
