//! Affixes (redo 8.1's second plan, PA-12): the `affixes` kind, each a
//! prefix or suffix of a group with its weights by base tag, its tiers of
//! value ranges by item level and the lines its rolled values fill. 8.7's
//! roll reads the weights; an instance names its affixes by ID, tier and
//! values.

use serde::{Deserialize, Serialize};
use tungsten_core::{MissingAsset, World};

use crate::foundation::stats::valid_stat;
use crate::foundation::tags::{check_declared, valid_tag};
use crate::items::base::{ItemLine, MAX_LINES, MAX_VALUES};

/// The data kind.
pub(crate) const AFFIXES: &str = "affixes";
/// The most weights and tiers an affix holds.
const MAX_WEIGHTS: usize = 16;
/// The most tiers an affix holds.
pub(crate) const MAX_TIERS: usize = 16;
/// The highest item level.
pub(crate) const MAX_LEVEL: u32 = 100;
/// How far a value may sit off its step and still be on it, as a share of
/// the step.
#[cfg_attr(not(test), allow(dead_code))]
const STEP_SLACK: f32 = 1e-3;

/// Where an affix goes on an item's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AffixKind {
    /// Before the base's name.
    Prefix,
    /// After it.
    Suffix,
}

/// An affix's weight for bases carrying a tag: 8.7's roll takes the first
/// entry whose tag a base carries; none, or 0, leaves the affix out.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AffixWeightFields", into = "AffixWeightFields")]
pub struct AffixWeight {
    tag: String,
    weight: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AffixWeightFields {
    tag: String,
    #[serde(deserialize_with = "crate::foundation::non_negative")]
    weight: f32,
}

impl TryFrom<AffixWeightFields> for AffixWeight {
    type Error = String;

    fn try_from(fields: AffixWeightFields) -> Result<Self, Self::Error> {
        if !valid_tag(&fields.tag) {
            return Err(format!(
                "tag: `{}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)",
                fields.tag
            ));
        }
        Ok(Self {
            tag: fields.tag,
            weight: fields.weight,
        })
    }
}

impl From<AffixWeight> for AffixWeightFields {
    fn from(weight: AffixWeight) -> Self {
        Self {
            tag: weight.tag,
            weight: weight.weight,
        }
    }
}

impl AffixWeight {
    /// The base tag it weighs.
    #[must_use]
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Its weight, at least 0.
    #[must_use]
    pub fn weight(&self) -> f32 {
        self.weight
    }
}

/// One tier of an affix: the least item level that rolls it, its weight
/// among the affix's tiers, its value ranges and the step a value lands
/// on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AffixTierFields", into = "AffixTierFields")]
pub struct AffixTier {
    level: u32,
    weight: f32,
    values: Vec<[f32; 2]>,
    step: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AffixTierFields {
    level: u32,
    #[serde(deserialize_with = "crate::foundation::positive")]
    weight: f32,
    values: Vec<[f32; 2]>,
    #[serde(
        default = "unit_step",
        deserialize_with = "crate::foundation::positive"
    )]
    step: f32,
}

fn unit_step() -> f32 {
    1.0
}

impl TryFrom<AffixTierFields> for AffixTier {
    type Error = String;

    fn try_from(fields: AffixTierFields) -> Result<Self, Self::Error> {
        if !(1..=MAX_LEVEL).contains(&fields.level) {
            return Err(format!("level: 1 to {MAX_LEVEL} (got {})", fields.level));
        }
        if fields.values.is_empty() || fields.values.len() > MAX_VALUES {
            return Err(format!("values: 1 to {MAX_VALUES} ranges"));
        }
        for (i, [min, max]) in fields.values.iter().enumerate() {
            if !(min.is_finite() && max.is_finite() && min <= max) {
                return Err(format!(
                    "values.{i}: [{min}, {max}] is not a range with min <= max"
                ));
            }
        }
        Ok(Self {
            level: fields.level,
            weight: fields.weight,
            values: fields.values,
            step: fields.step,
        })
    }
}

impl From<AffixTier> for AffixTierFields {
    fn from(tier: AffixTier) -> Self {
        Self {
            level: tier.level,
            weight: tier.weight,
            values: tier.values,
            step: tier.step,
        }
    }
}

impl AffixTier {
    /// The least item level that rolls it.
    #[must_use]
    pub fn level(&self) -> u32 {
        self.level
    }

    /// Its weight among the affix's tiers.
    #[must_use]
    pub fn weight(&self) -> f32 {
        self.weight
    }

    /// Its value ranges, each `[min, max]`.
    #[must_use]
    pub fn values(&self) -> &[[f32; 2]] {
        &self.values
    }

    /// The step a rolled value lands on, from each range's least.
    #[must_use]
    pub fn step(&self) -> f32 {
        self.step
    }

    /// Whether `value` lies in range `i` on its step.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn holds(&self, i: usize, value: f32) -> bool {
        let Some([min, max]) = self.values.get(i) else {
            return false;
        };
        if !(value.is_finite() && *min <= value && value <= *max) {
            return false;
        }
        let steps = (value - min) / self.step;
        (steps - steps.round()).abs() <= STEP_SLACK
    }
}

/// An affix: an `affixes` entry, one a file. A prefix or suffix of a group
/// (an item takes one affix of a group), its weights by base tag, its
/// tiers, ascending by item level, and its lines, whose numbers may read
/// the tier's rolled values (`{ "value": i, "times": k }`).
///
/// ```json
/// { "kind": "prefix", "group": "fire_damage_pct",
///   "weights": [{ "tag": "item.wand", "weight": 1000 }],
///   "tiers": [{ "level": 1, "weight": 1000, "values": [[10, 16]] },
///             { "level": 20, "weight": 600, "values": [[17, 24]] }],
///   "lines": [{ "stat": "damage", "modifier": { "pct": { "value": 0, "times": 0.01 } },
///               "with": { "any": ["damage.fire"] } }] }
/// ```
///
/// The parse holds the tiers' levels ascending and every rolled value a
/// line reads to a range every tier has; the load checks the weights' and
/// lines' tags, the lines' stats and damage types.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Affix, AffixKind};
///
/// let searing: Affix = serde_json::from_str(
///     r#"{ "kind": "prefix", "group": "fire_damage_pct",
///          "tiers": [{ "level": 1, "weight": 1000, "values": [[10, 16]] }],
///          "lines": [{ "stat": "damage", "modifier": { "pct": { "value": 0, "times": 0.01 } } }] }"#,
/// )
/// .unwrap();
/// assert_eq!((searing.kind(), searing.group()), (AffixKind::Prefix, "fire_damage_pct"));
/// assert_eq!(searing.tiers()[0].values(), [[10.0, 16.0]]);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "AffixFields", into = "AffixFields")]
pub struct Affix {
    kind: AffixKind,
    group: String,
    weights: Vec<AffixWeight>,
    tiers: Vec<AffixTier>,
    lines: Vec<ItemLine>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AffixFields {
    kind: AffixKind,
    group: String,
    #[serde(default)]
    weights: Vec<AffixWeight>,
    tiers: Vec<AffixTier>,
    lines: Vec<ItemLine>,
}

impl TryFrom<AffixFields> for Affix {
    type Error = String;

    fn try_from(fields: AffixFields) -> Result<Self, Self::Error> {
        if !valid_stat(&fields.group) {
            return Err(format!(
                "group: `{}` is not a group name (`[a-z][a-z0-9_]*`)",
                fields.group
            ));
        }
        if fields.weights.len() > MAX_WEIGHTS {
            return Err(format!("weights: at most {MAX_WEIGHTS}"));
        }
        if fields.tiers.is_empty() || fields.tiers.len() > MAX_TIERS {
            return Err(format!("tiers: 1 to {MAX_TIERS}"));
        }
        for (i, pair) in fields.tiers.windows(2).enumerate() {
            if pair[1].level <= pair[0].level {
                return Err(format!(
                    "tiers.{}.level: {} after {}; tiers ascend by level",
                    i + 1,
                    pair[1].level,
                    pair[0].level
                ));
            }
        }
        if fields.lines.is_empty() || fields.lines.len() > MAX_LINES {
            return Err(format!("lines: 1 to {MAX_LINES}"));
        }
        let ranges = fields
            .tiers
            .iter()
            .map(|tier| tier.values.len())
            .min()
            .unwrap_or(0);
        for (i, line) in fields.lines.iter().enumerate() {
            if let Some(value) = line.rolled_values().find(|&v| usize::from(v) >= ranges) {
                return Err(format!(
                    "lines.{i}: reads rolled value {value}, past the {ranges} every tier rolls"
                ));
            }
        }
        Ok(Self {
            kind: fields.kind,
            group: fields.group,
            weights: fields.weights,
            tiers: fields.tiers,
            lines: fields.lines,
        })
    }
}

impl From<Affix> for AffixFields {
    fn from(affix: Affix) -> Self {
        Self {
            kind: affix.kind,
            group: affix.group,
            weights: affix.weights,
            tiers: affix.tiers,
            lines: affix.lines,
        }
    }
}

impl Affix {
    /// A prefix or a suffix.
    #[must_use]
    pub fn kind(&self) -> AffixKind {
        self.kind
    }

    /// Its group: an item takes one affix of a group.
    #[must_use]
    pub fn group(&self) -> &str {
        &self.group
    }

    /// Its weights by base tag, in order.
    #[must_use]
    pub fn weights(&self) -> &[AffixWeight] {
        &self.weights
    }

    /// Its tiers, ascending by level; tier 1 is the first.
    #[must_use]
    pub fn tiers(&self) -> &[AffixTier] {
        &self.tiers
    }

    /// Whether a line folds into the base's block.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn has_local(&self) -> bool {
        self.lines.iter().any(ItemLine::is_local)
    }

    /// Its lines.
    pub(crate) fn lines(&self) -> &[ItemLine] {
        &self.lines
    }
}

/// The `affixes` kind's check: the weights' tags and the lines' names.
pub(crate) fn check_affix(affix: &Affix, world: &World) -> Result<(), MissingAsset> {
    for (i, weight) in affix.weights.iter().enumerate() {
        check_declared(world, &format!("weights.{i}.tag"), &weight.tag)?;
    }
    for (i, line) in affix.lines.iter().enumerate() {
        line.check_in(world, &format!("lines.{i}"))?;
    }
    Ok(())
}
