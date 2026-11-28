//! Item instances (redo 8.1's second plan, PA-12, §4.6 of the ARPG design):
//! a base with its level, rarity, seed, rolled affixes, name and count;
//! the load's check of one a prefab holds; and what one builds into, its
//! name, its block after its local lines and its global lines with their
//! text.

use std::cell::RefCell;
use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tungsten_core::{DataRegistry, MissingAsset, World};

use crate::foundation::stats::valid_stat;
use crate::items::affix::{Affix, MAX_LEVEL, MAX_TIERS};
use crate::items::base::{BuiltLine, ItemBase, LocalTerm, MAX_STACK, MAX_VALUES, WeaponBlock};

/// The most affixes an instance holds.
const MAX_AFFIXES: usize = 12;
/// The most words of an instance's own name.
const MAX_WORDS: usize = 4;
/// The most characters of a name's word.
const MAX_WORD: usize = 32;

/// One rolled affix of an instance: its `affixes` ID, its tier (1 the
/// lowest) and its rolled values, one a range of the tier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RolledAffixFields", into = "RolledAffixFields")]
pub struct RolledAffix {
    id: String,
    tier: u32,
    values: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RolledAffixFields {
    id: String,
    tier: u32,
    values: Vec<f32>,
}

impl TryFrom<RolledAffixFields> for RolledAffix {
    type Error = String;

    fn try_from(fields: RolledAffixFields) -> Result<Self, Self::Error> {
        let tiers = u32::try_from(MAX_TIERS).unwrap_or(u32::MAX);
        if !(1..=tiers).contains(&fields.tier) {
            return Err(format!("tier: 1 to {tiers} (got {})", fields.tier));
        }
        if fields.values.is_empty() || fields.values.len() > MAX_VALUES {
            return Err(format!("values: 1 to {MAX_VALUES}"));
        }
        if let Some((i, value)) = fields
            .values
            .iter()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(format!("values.{i}: must be finite (got {value})"));
        }
        Ok(Self {
            id: fields.id,
            tier: fields.tier,
            values: fields.values,
        })
    }
}

impl From<RolledAffix> for RolledAffixFields {
    fn from(affix: RolledAffix) -> Self {
        Self {
            id: affix.id,
            tier: affix.tier,
            values: affix.values,
        }
    }
}

impl RolledAffix {
    /// The affix `id` at tier `tier` (1 the lowest) with `values`.
    ///
    /// # Panics
    ///
    /// When the tier is outside 1 to 16, or the values are not 1 to 4
    /// finite numbers.
    #[must_use]
    #[track_caller]
    pub fn new(id: impl Into<String>, tier: u32, values: impl IntoIterator<Item = f32>) -> Self {
        let fields = RolledAffixFields {
            id: id.into(),
            tier,
            values: values.into_iter().collect(),
        };
        Self::try_from(fields).unwrap_or_else(|e| panic!("RolledAffix::new: {e}"))
    }

    /// Its `affixes` ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Its tier, 1 the lowest.
    #[must_use]
    pub fn tier(&self) -> u32 {
        self.tier
    }

    /// Its rolled values.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

/// An item instance (§4.6 of the ARPG design): its uid (0 until a bag
/// first takes it), its `item_bases` ID, its level, rarity and seed, its
/// rolled affixes, the words of its own name and how many it stands for.
///
/// ```json
/// { "uid": 9137, "base": "ashwood_wand", "level": 42, "rarity": "rare", "seed": "0x9e3779b97f4a7c15",
///   "affixes": [{ "id": "searing", "tier": 2, "values": [22] }],
///   "name": ["Ember", "Coil"] }
/// ```
///
/// The parse holds the level to 1–100, the seed to a hex `u64`, at most 12
/// affixes and 4 words, and the count to 1–9,999; a data item a prefix
/// holds is checked at load against its base and affixes
/// ([`ItemsPlugin`](crate::ItemsPlugin)). A restored item is kept as
/// saved: what it names that the data lacks builds to nothing, warned once.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Item, RolledAffix};
///
/// let wand = Item::new("ashwood_wand")
///     .with_level(42)
///     .with_rarity("rare")
///     .with_affix(RolledAffix::new("searing", 2, [22.0]));
/// let data = serde_json::to_value(&wand).unwrap();
/// assert_eq!(data["affixes"][0]["values"], serde_json::json!([22.0]));
/// assert_eq!(serde_json::from_value::<Item>(data).unwrap(), wand);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ItemFields", into = "ItemFields")]
pub struct Item {
    uid: u32,
    base: String,
    level: u32,
    rarity: String,
    seed: u64,
    affixes: Vec<RolledAffix>,
    name: Vec<String>,
    count: u32,
}

/// An item as data writes it; `count` written past 1.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemFields {
    #[serde(default)]
    uid: u32,
    base: String,
    #[serde(default = "first_level")]
    level: u32,
    #[serde(default = "normal")]
    rarity: String,
    #[serde(default = "zero_seed")]
    seed: String,
    #[serde(default)]
    affixes: Vec<RolledAffix>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    name: Vec<String>,
    #[serde(default = "first_level", skip_serializing_if = "is_one")]
    count: u32,
    /// 8.7's unique form, refused by name.
    #[serde(default, skip_serializing)]
    unique: Option<Value>,
}

fn first_level() -> u32 {
    1
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_one(count: &u32) -> bool {
    *count == 1
}

fn normal() -> String {
    "normal".to_owned()
}

fn zero_seed() -> String {
    "0x0".to_owned()
}

/// `0x`-led hex digits as a `u64`.
fn parse_seed(text: &str) -> Option<u64> {
    let digits = text.strip_prefix("0x")?;
    if digits.is_empty() || digits.len() > 16 {
        return None;
    }
    u64::from_str_radix(digits, 16).ok()
}

impl TryFrom<ItemFields> for Item {
    type Error = String;

    fn try_from(fields: ItemFields) -> Result<Self, Self::Error> {
        if fields.unique.is_some() {
            return Err("unique: a unique item is 8.7's; an instance names its `base`".into());
        }
        if fields.base.is_empty() {
            return Err("base: an `item_bases` ID".into());
        }
        if !(1..=MAX_LEVEL).contains(&fields.level) {
            return Err(format!("level: 1 to {MAX_LEVEL} (got {})", fields.level));
        }
        if !valid_stat(&fields.rarity) {
            return Err(format!(
                "rarity: `{}` is not a rarity name (`[a-z][a-z0-9_]*`)",
                fields.rarity
            ));
        }
        let Some(seed) = parse_seed(&fields.seed) else {
            return Err(format!(
                "seed: `{}` is not a hex u64 (`0x` and 1 to 16 digits)",
                fields.seed
            ));
        };
        if fields.affixes.len() > MAX_AFFIXES {
            return Err(format!("affixes: at most {MAX_AFFIXES}"));
        }
        if fields.name.len() > MAX_WORDS {
            return Err(format!("name: at most {MAX_WORDS} words"));
        }
        for (i, word) in fields.name.iter().enumerate() {
            if word.is_empty() || word.chars().count() > MAX_WORD || word.contains(' ') {
                return Err(format!(
                    "name.{i}: a word of 1 to {MAX_WORD} characters, no space"
                ));
            }
        }
        if !(1..=MAX_STACK).contains(&fields.count) {
            return Err(format!("count: 1 to {MAX_STACK} (got {})", fields.count));
        }
        Ok(Self {
            uid: fields.uid,
            base: fields.base,
            level: fields.level,
            rarity: fields.rarity,
            seed,
            affixes: fields.affixes,
            name: fields.name,
            count: fields.count,
        })
    }
}

impl From<Item> for ItemFields {
    fn from(item: Item) -> Self {
        Self {
            uid: item.uid,
            base: item.base,
            level: item.level,
            rarity: item.rarity,
            seed: format!("{:#x}", item.seed),
            affixes: item.affixes,
            name: item.name,
            count: item.count,
            unique: None,
        }
    }
}

impl Item {
    /// One of base `base`: level 1, `normal`, seed 0, no affixes, its
    /// base's name, uid 0.
    #[must_use]
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            uid: 0,
            base: base.into(),
            level: 1,
            rarity: normal(),
            seed: 0,
            affixes: Vec::new(),
            name: Vec::new(),
            count: 1,
        }
    }

    /// At item level `level`.
    ///
    /// # Panics
    ///
    /// When `level` is outside 1 to 100.
    #[must_use]
    #[track_caller]
    pub fn with_level(mut self, level: u32) -> Self {
        assert!(
            (1..=MAX_LEVEL).contains(&level),
            "Item::with_level: 1 to {MAX_LEVEL} (got {level})"
        );
        self.level = level;
        self
    }

    /// Of rarity `rarity`, a name.
    ///
    /// # Panics
    ///
    /// When `rarity` is not `[a-z][a-z0-9_]*`.
    #[must_use]
    #[track_caller]
    pub fn with_rarity(mut self, rarity: impl Into<String>) -> Self {
        let rarity = rarity.into();
        assert!(
            valid_stat(&rarity),
            "Item::with_rarity: `{rarity}` is not a rarity name"
        );
        self.rarity = rarity;
        self
    }

    /// With seed `seed`, kept for 8.7's reroll.
    #[must_use]
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// With `affix` after its others.
    ///
    /// # Panics
    ///
    /// When the item holds 12 affixes.
    #[must_use]
    #[track_caller]
    pub fn with_affix(mut self, affix: RolledAffix) -> Self {
        assert!(
            self.affixes.len() < MAX_AFFIXES,
            "Item::with_affix: at most {MAX_AFFIXES} affixes"
        );
        self.affixes.push(affix);
        self
    }

    /// Named by `words`, in place of its base's name.
    ///
    /// # Panics
    ///
    /// When there are more than 4 words, or a word is empty, past 32
    /// characters or holds a space.
    #[must_use]
    #[track_caller]
    pub fn with_name<S: Into<String>>(mut self, words: impl IntoIterator<Item = S>) -> Self {
        let words: Vec<String> = words.into_iter().map(Into::into).collect();
        assert!(
            words.len() <= MAX_WORDS
                && words.iter().all(|word| {
                    !word.is_empty() && word.chars().count() <= MAX_WORD && !word.contains(' ')
                }),
            "Item::with_name: at most {MAX_WORDS} words of 1 to {MAX_WORD} characters"
        );
        self.name = words;
        self
    }

    /// Standing for `count` of its base, stacked in one cell.
    ///
    /// # Panics
    ///
    /// When `count` is outside 1 to 9,999.
    #[must_use]
    #[track_caller]
    pub fn with_count(mut self, count: u32) -> Self {
        assert!(
            (1..=MAX_STACK).contains(&count),
            "Item::with_count: 1 to {MAX_STACK} (got {count})"
        );
        self.count = count;
        self
    }

    /// Its uid: 0 until a bag first takes it.
    #[must_use]
    pub fn uid(&self) -> u32 {
        self.uid
    }

    /// Its `item_bases` ID.
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Its item level.
    #[must_use]
    pub fn level(&self) -> u32 {
        self.level
    }

    /// Its rarity's name.
    #[must_use]
    pub fn rarity(&self) -> &str {
        &self.rarity
    }

    /// Its seed.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Its rolled affixes, in order.
    #[must_use]
    pub fn affixes(&self) -> &[RolledAffix] {
        &self.affixes
    }

    /// The words of its own name; none: its base's name.
    pub fn name_words(&self) -> impl Iterator<Item = &str> {
        self.name.iter().map(String::as_str)
    }

    /// How many it stands for.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Whether it stacks with `other`: the same base, level and rarity, and
    /// neither has affixes or a name of its own.
    #[must_use]
    pub fn stacks_with(&self, other: &Item) -> bool {
        self.base == other.base
            && self.level == other.level
            && self.rarity == other.rarity
            && self.affixes.is_empty()
            && other.affixes.is_empty()
            && self.name.is_empty()
            && other.name.is_empty()
    }
}

/// The load's check of an item data holds, at `field`: its base and each
/// affix known, each tier there at or below the item's level, one value a
/// range, each inside its range on the tier's step, no group twice, a
/// local line only on a base with a block, and the count within the base's
/// stack.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn check_item(world: &World, field: &str, item: &Item) -> Result<(), MissingAsset> {
    let Some(data) = world.get_resource::<DataRegistry>() else {
        return Ok(());
    };
    let Some(base) = data.get::<ItemBase>(&item.base) else {
        return Err(MissingAsset::new(
            format!("{field}.base"),
            "item base",
            item.base.as_str(),
        ));
    };
    if item.count > base.stack() {
        return Err(MissingAsset::new(
            format!("{field}.count"),
            "count within its base's stack",
            item.count.to_string(),
        ));
    }
    let mut groups = BTreeSet::new();
    for (i, rolled) in item.affixes.iter().enumerate() {
        let at = format!("{field}.affixes.{i}");
        let Some(affix) = data.get::<Affix>(&rolled.id) else {
            return Err(MissingAsset::new(
                format!("{at}.id"),
                "affix",
                rolled.id.as_str(),
            ));
        };
        let tier = usize::try_from(rolled.tier)
            .ok()
            .and_then(|tier| tier.checked_sub(1))
            .and_then(|i| affix.tiers().get(i));
        let Some(tier) = tier.filter(|tier| tier.level() <= item.level) else {
            return Err(MissingAsset::new(
                format!("{at}.tier"),
                "affix tier at or below the item's level",
                rolled.tier.to_string(),
            ));
        };
        if rolled.values.len() != tier.values().len() {
            return Err(MissingAsset::new(
                format!("{at}.values"),
                "one value a range of the tier",
                rolled.values.len().to_string(),
            ));
        }
        for (j, value) in rolled.values.iter().enumerate() {
            if !tier.holds(j, *value) {
                return Err(MissingAsset::new(
                    format!("{at}.values.{j}"),
                    "value inside its range, on its step",
                    value.to_string(),
                ));
            }
        }
        if !groups.insert(affix.group()) {
            return Err(MissingAsset::new(
                format!("{at}.id"),
                "affix of a group the item has not",
                rolled.id.as_str(),
            ));
        }
        if affix.has_local() && base.weapon().is_none() {
            return Err(MissingAsset::new(
                format!("{at}.id"),
                "affix whose local lines fold into a weapon block",
                rolled.id.as_str(),
            ));
        }
    }
    Ok(())
}

/// The warnings a build gave, by what it could not read: a World resource
/// [`ItemsPlugin`](crate::ItemsPlugin) inserts, so each warns once.
#[derive(Debug, Default)]
pub(crate) struct ItemWarnings(RefCell<BTreeSet<String>>);

impl ItemWarnings {
    /// How many distinct warnings were given.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.0.borrow().len()
    }
}

/// Warns `message` once a `key`, else every time in a World without
/// [`ItemWarnings`].
fn warn_once(world: &World, key: String, message: impl FnOnce() -> String) {
    let fresh = world
        .get_resource::<ItemWarnings>()
        .is_none_or(|warnings| warnings.0.borrow_mut().insert(key));
    if fresh {
        log::warn!("{}", message());
    }
}

/// What an item builds into from the data of the day: its name, its
/// rarity, its block after its local lines, and its global lines in order,
/// the base's implicit lines and then each affix's at its values, each
/// with its text.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltItem {
    name: String,
    rarity: String,
    known: bool,
    block: Option<WeaponBlock>,
    lines: Vec<BuiltLine>,
}

impl BuiltItem {
    /// Its name: its own words, else its base's name, else its base's ID
    /// when the data lacks the base.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its rarity's name.
    #[must_use]
    pub fn rarity(&self) -> &str {
        &self.rarity
    }

    /// Whether the data holds its base.
    #[must_use]
    pub fn known(&self) -> bool {
        self.known
    }

    /// Its weapon block after its local lines, when its base has one.
    #[must_use]
    pub fn block(&self) -> Option<&WeaponBlock> {
        self.block.as_ref()
    }

    /// Its global lines, in order.
    #[must_use]
    pub fn lines(&self) -> &[BuiltLine] {
        &self.lines
    }
}

/// What `item` builds into from `world`'s data (§2.3 of the plan): `None`
/// in a World with no data registry. A base the data lacks leaves the item
/// shown by its base ID with no block or lines; an affix, a tier or a line
/// the data or the saved values cannot fill leaves those lines out; each
/// warned once.
///
/// # Examples
///
/// ```
/// use tungsten_core::{DataRegistry, World};
/// use tungsten_kit::{Item, ItemBase, ItemsPlugin, built};
///
/// let mut world = World::new();
/// let mut schedule = tungsten_core::Schedule::new();
/// tungsten_core::Plugin::build(&ItemsPlugin, &mut schedule, &mut world);
/// let hood: ItemBase = serde_json::from_str(
///     r#"{ "name": "Oilskin Hood", "icon": "hood", "slot": "hood",
///          "implicit": [{ "stat": "move_speed", "modifier": { "pct": -0.05 } }] }"#,
/// )
/// .unwrap();
/// world.resource_mut::<DataRegistry>().insert("oilskin_hood", hood);
/// let built = built(&world, &Item::new("oilskin_hood")).unwrap();
/// assert_eq!(built.name(), "Oilskin Hood");
/// assert_eq!(built.lines()[0].text(), "-5% move_speed");
/// ```
#[must_use]
pub fn built(world: &World, item: &Item) -> Option<BuiltItem> {
    let data = world.get_resource::<DataRegistry>()?;
    let own_name = (!item.name.is_empty()).then(|| item.name.join(" "));
    let Some(base) = data.get::<ItemBase>(&item.base) else {
        warn_once(world, format!("base:{}", item.base), || {
            format!(
                "item `{}`: its base is gone from the data; it shows by its base ID with no lines",
                item.base
            )
        });
        return Some(BuiltItem {
            name: own_name.unwrap_or_else(|| item.base.clone()),
            rarity: item.rarity.clone(),
            known: false,
            block: None,
            lines: Vec::new(),
        });
    };
    let mut terms: Vec<LocalTerm> = Vec::new();
    let mut lines = Vec::new();
    let mut take = |line: &crate::items::base::ItemLine, values: &[f32], key: String| {
        if line.is_local() {
            if base.weapon().is_none() {
                warn_once(world, key.clone(), || {
                    format!(
                        "item line {key}: a local line on a base with no weapon block; left out"
                    )
                });
            } else if let Some(term) = line.local_term(values) {
                terms.push(term);
            } else {
                warn_once(world, key.clone(), || {
                    format!("item line {key}: its saved values do not fill it; left out")
                });
            }
        } else if let Some(built) = line.build(values) {
            lines.push(built);
        } else {
            warn_once(world, key.clone(), || {
                format!("item line {key}: its saved values do not fill it; left out")
            });
        }
    };
    for (i, line) in base.implicit().iter().enumerate() {
        take(line, &[], format!("{}/implicit.{i}", item.base));
    }
    for rolled in &item.affixes {
        let Some(affix) = data.get::<Affix>(&rolled.id) else {
            warn_once(world, format!("affix:{}", rolled.id), || {
                format!(
                    "affix `{}`: gone from the data; its lines are left out",
                    rolled.id
                )
            });
            continue;
        };
        let tiers = affix.tiers().len();
        if usize::try_from(rolled.tier).map_or(true, |tier| tier == 0 || tier > tiers) {
            warn_once(world, format!("tier:{}/{}", rolled.id, rolled.tier), || {
                format!(
                    "affix `{}`: tier {} is gone from the data; its lines are left out",
                    rolled.id, rolled.tier
                )
            });
            continue;
        }
        for (j, line) in affix.lines().iter().enumerate() {
            take(line, &rolled.values, format!("{}/lines.{j}", rolled.id));
        }
    }
    let block = base.weapon().map(|block| block.folded(&terms));
    Some(BuiltItem {
        name: own_name.unwrap_or_else(|| base.name().to_owned()),
        rarity: item.rarity.clone(),
        known: true,
        block,
        lines,
    })
}

#[cfg(test)]
#[path = "../tests/item.rs"]
mod tests;
