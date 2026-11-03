//! Skill sheets (§4.2, §4.3, §6.2 and §6.4 of the ARPG design): an
//! ability's tags, bases, level curves and skill-local lines, resolved at a
//! use into a [`Delivery`], the snapshot its spawns and armed hitboxes carry
//! to their hits.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tungsten_core::save::{RefCodec, RefReader, RefWriter, SaveOptions};
use tungsten_core::{DataRef, DataRegistry, Entity, MissingAsset, World};

use crate::damage::effects::MODIFIER;
use crate::damage::kinds::{CONVERSION_RULE, ConversionRuleDef, check_rule, check_type};
use crate::foundation::curve::Curve;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, describe_shape, round_trip,
};
use crate::foundation::explain::{
    CritRecord, ExplainRecord, HitPartRecord, HitRecord, KitExplain, LineOutcome, LineRecord,
    OriginStep, PartOrigin, PartRecord, ReadAt, SheetRecord, StatRecord, outcome, why,
};
use crate::foundation::lines::{Line, Reach, Read, Scope, Sums, TagSet};
use crate::foundation::optional_non_negative;
use crate::foundation::stats::{Modifier, StatLine, Stats, check_at_use, valid_stat};
use crate::foundation::tags::{TRACK_DOT, TRACK_HIT, TagFilter, Tags, covers, valid_tag};
use crate::intent::ability::{Abilities, Ability};

/// The lowest and the highest skill level.
const LEVELS: (f32, f32) = (1.0, 40.0);
/// A crit's multiplier for a holder without `crit_multiplier`.
const CRIT_MULTIPLIER: f32 = 1.5;
/// The bases a skill's `levels` may name.
const LEVEL_BASES: [&str; 4] = ["damage", "crit_chance", "cost", "cooldown"];
/// The name a save holds a [`Delivery`] under.
pub(crate) const DELIVERY: &str = "delivery";

/// What a skill's line changes.
#[derive(Debug, Clone, PartialEq)]
enum LineKind {
    /// A stat the sheet reads, by a modifier.
    Stat { stat: String, modifier: Modifier },
    /// Flat damage of a type, a range, added to that type's part.
    Added { kind: String, range: [f32; 2] },
    /// Penetration points for the parts of a type.
    Pen { kind: String, points: f32 },
    /// A share of each part of a type converted to another.
    Convert { from: String, to: String, pct: f32 },
    /// A share of each part carrying a type added as another.
    Extra { from: String, to: String, pct: f32 },
}

/// A line a skill reads (§4.1 of the ARPG design): a stat line, flat damage
/// of a damage type (`added`), penetration points for a type's parts
/// (`pen`), or a share of a type converted to another (`convert`) or added
/// as another (`extra`), each with a [`Scope`]. Only the lines on `damage`,
/// `crit_chance` and `crit_multiplier`, and `pen` lines, may wait for the
/// hit (an `against` filter, or a state tag in `with`): one on
/// `skill_level`, `cost` or `cooldown`, or an `added`, `convert` or `extra`
/// line, is refused, since a use spends them.
///
/// Data form, one kind with its filters beside it:
/// `{ "stat": "damage", "modifier": { "pct": 0.4 }, "with": { "all": ["skill.spell"] } }`,
/// `{ "added": { "damage.fire": [4, 6] } }`,
/// `{ "pen": { "damage.fire": 0.1 }, "against": { "all": ["status.chilled"] } }`,
/// `{ "convert": { "from": "damage.fire", "to": "damage.cold", "pct": 0.5 } }` or
/// `{ "extra": { "from": "damage.cold", "to": "damage.fire", "pct": 0.18 } }`.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Line, Modifier, SkillLine, TagFilter};
///
/// let spells = TagFilter::new().all(["skill.spell"]);
/// let line = SkillLine::stat("damage", Line::new(Modifier::Pct(0.4)).with(spells));
/// assert_eq!(line.as_stat(), Some(("damage", Modifier::Pct(0.4))));
/// let added = SkillLine::added("damage.fire", 4.0, 6.0);
/// assert_eq!(added.as_added(), Some(("damage.fire", [4.0, 6.0])));
/// let data = serde_json::to_value(&added).unwrap();
/// assert_eq!(data, serde_json::json!({ "added": { "damage.fire": [4.0, 6.0] } }));
/// let half: SkillLine = serde_json::from_value(serde_json::json!({
///     "convert": { "from": "damage.fire", "to": "damage.cold", "pct": 0.5 }
/// }))
/// .unwrap();
/// assert_eq!(half, SkillLine::convert("damage.fire", "damage.cold", 0.5));
/// assert_eq!(half.as_convert(), Some(("damage.fire", "damage.cold", 0.5)));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SkillLineFields", into = "SkillLineFields")]
pub struct SkillLine {
    kind: LineKind,
    scope: Scope,
}

impl SkillLine {
    /// A line on `stat`, `line`'s modifier in its scope.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*`, or the line waits for the hit
    /// on a stat a use spends.
    #[must_use]
    #[track_caller]
    pub fn stat(stat: &str, line: Line) -> Self {
        let (modifier, scope) = line.into_parts();
        let made = Self {
            kind: LineKind::Stat {
                stat: stat.to_owned(),
                modifier,
            },
            scope,
        };
        made.checked("stat")
    }

    /// Flat damage of `kind`, from `min` to `max`, added to that type's
    /// part in the skill's context.
    ///
    /// # Panics
    ///
    /// When `kind` is not a tag name, or `min` and `max` are not finite with
    /// `0 <= min <= max`.
    #[must_use]
    #[track_caller]
    pub fn added(kind: &str, min: f32, max: f32) -> Self {
        Self {
            kind: LineKind::Added {
                kind: kind.to_owned(),
                range: [min, max],
            },
            scope: Scope::new(),
        }
        .checked("added")
    }

    /// `points` of penetration for the parts of `kind` and its dotted
    /// descendants, by type: a part's history reaches only its filters, so
    /// a cold `pen` line `with` `damage.fire` reaches cold converted from
    /// fire and not native cold.
    ///
    /// # Panics
    ///
    /// When `kind` is not a tag name or `points` is not finite.
    #[must_use]
    #[track_caller]
    pub fn pen(kind: &str, points: f32) -> Self {
        Self {
            kind: LineKind::Pen {
                kind: kind.to_owned(),
                points,
            },
            scope: Scope::new(),
        }
        .checked("pen")
    }

    /// `pct` of each part whose type is `from` or a dotted descendant
    /// converted to `to` at a use (§6.2 step 6), the share's history the
    /// part's plus its type; one part's shares past 1 normalize, and no
    /// share converts again.
    ///
    /// # Panics
    ///
    /// When `from` or `to` is not a tag name, they are equal, or `pct` is
    /// outside `[0, 1]`.
    #[must_use]
    #[track_caller]
    pub fn convert(from: &str, to: &str, pct: f32) -> Self {
        Self {
            kind: LineKind::Convert {
                from: from.to_owned(),
                to: to.to_owned(),
                pct,
            },
            scope: Scope::new(),
        }
        .checked("convert")
    }

    /// `pct` of each part whose type or history carries `from` added as a
    /// new part of `to` at a use, after the conversion (§6.2 step 7): its
    /// history the part's plus its type; an extra part makes none.
    ///
    /// # Panics
    ///
    /// When `from` or `to` is not a tag name, they are equal, or `pct` is
    /// outside `[0, 1]`.
    #[must_use]
    #[track_caller]
    pub fn extra(from: &str, to: &str, pct: f32) -> Self {
        Self {
            kind: LineKind::Extra {
                from: from.to_owned(),
                to: to.to_owned(),
                pct,
            },
            scope: Scope::new(),
        }
        .checked("extra")
    }

    /// The line, read only where `filter` passes the read's context.
    ///
    /// # Panics
    ///
    /// When the line may then not stand: see [`SkillLine`].
    #[must_use]
    #[track_caller]
    pub fn with(mut self, filter: TagFilter) -> Self {
        self.scope = self.scope.with(filter);
        self.checked("with")
    }

    /// The line, read only while `filter` passes the holder's tags.
    ///
    /// # Panics
    ///
    /// When the line may then not stand: see [`SkillLine`].
    #[must_use]
    #[track_caller]
    pub fn while_holder(mut self, filter: TagFilter) -> Self {
        self.scope = self.scope.while_holder(filter);
        self.checked("while_holder")
    }

    /// The line, read only at a hit whose other party's tags pass `filter`.
    ///
    /// # Panics
    ///
    /// When the line may then not stand: see [`SkillLine`].
    #[must_use]
    #[track_caller]
    pub fn against(mut self, filter: TagFilter) -> Self {
        self.scope = self.scope.against(filter);
        self.checked("against")
    }

    /// Its stat and modifier, for a stat line.
    #[must_use]
    pub fn as_stat(&self) -> Option<(&str, Modifier)> {
        match &self.kind {
            LineKind::Stat { stat, modifier } => Some((stat, *modifier)),
            _ => None,
        }
    }

    /// Its damage type and range, for an `added` line.
    #[must_use]
    pub fn as_added(&self) -> Option<(&str, [f32; 2])> {
        match &self.kind {
            LineKind::Added { kind, range } => Some((kind, *range)),
            _ => None,
        }
    }

    /// Its damage type and points, for a `pen` line.
    #[must_use]
    pub fn as_pen(&self) -> Option<(&str, f32)> {
        match &self.kind {
            LineKind::Pen { kind, points } => Some((kind, *points)),
            _ => None,
        }
    }

    /// Its source type, target type and share, for a `convert` line.
    #[must_use]
    pub fn as_convert(&self) -> Option<(&str, &str, f32)> {
        match &self.kind {
            LineKind::Convert { from, to, pct } => Some((from, to, *pct)),
            _ => None,
        }
    }

    /// Its source type, target type and share, for an `extra` line.
    #[must_use]
    pub fn as_extra(&self) -> Option<(&str, &str, f32)> {
        match &self.kind {
            LineKind::Extra { from, to, pct } => Some((from, to, *pct)),
            _ => None,
        }
    }

    /// Its filters.
    #[must_use]
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// Whether a sheet keeps it for the hit: a line on `damage`,
    /// `crit_chance` or `crit_multiplier`, or a `pen` line, that waits for
    /// the hit.
    pub(crate) fn deferred(&self) -> bool {
        self.scope.waits_for_hit()
            && match &self.kind {
                LineKind::Stat { stat, .. } => {
                    [Stats::DAMAGE, Stats::CRIT_CHANCE, Stats::CRIT_MULTIPLIER]
                        .contains(&stat.as_str())
                }
                LineKind::Pen { .. } => true,
                LineKind::Added { .. } | LineKind::Convert { .. } | LineKind::Extra { .. } => false,
            }
    }

    /// Whether it may stand: names and numbers checked, no scoped override,
    /// and no wait for the hit on what a use spends.
    fn check(&self) -> Result<(), String> {
        match &self.kind {
            LineKind::Stat { stat, modifier } => {
                if !valid_stat(stat) {
                    return Err(format!(
                        "stat: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
                    ));
                }
                Line::scoped(*modifier, self.scope.clone())?;
                check_at_use(stat, &self.scope)
            }
            LineKind::Added { kind, range } => {
                check_kind(kind)?;
                if !(range.iter().all(|v| v.is_finite()) && 0.0 <= range[0] && range[0] <= range[1])
                {
                    return Err(format!(
                        "added: {kind}: [{}, {}] is not a range with 0 <= min <= max",
                        range[0], range[1]
                    ));
                }
                if self.scope.waits_for_hit() {
                    return Err("an `added` line, spent at a use, may not wait for the hit (`against`, or a state tag in `with`)".to_string());
                }
                Ok(())
            }
            LineKind::Pen { kind, points } => {
                check_kind(kind)?;
                if points.is_finite() {
                    Ok(())
                } else {
                    Err(format!("pen: {kind}: must be finite (got {points})"))
                }
            }
            LineKind::Convert { from, to, pct } | LineKind::Extra { from, to, pct } => {
                check_rule(from, to, *pct)?;
                if self.scope.waits_for_hit() {
                    return Err("a `convert` or `extra` line, spent at a use, may not wait for the hit (`against`, or a state tag in `with`)".to_string());
                }
                Ok(())
            }
        }
    }

    #[track_caller]
    fn checked(self, method: &str) -> Self {
        if let Err(error) = self.check() {
            panic!("SkillLine::{method}: {error}");
        }
        self
    }

    /// The load's check: its filters' tags and its damage type.
    pub(crate) fn check_in(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        self.scope.check(world, field)?;
        match &self.kind {
            LineKind::Added { kind, .. } => {
                check_type(world, &format!("{field}.added"), kind, true)
            }
            LineKind::Pen { kind, .. } => check_type(world, &format!("{field}.pen"), kind, true),
            LineKind::Convert { from, to, .. } | LineKind::Extra { from, to, .. } => {
                let form = if matches!(self.kind, LineKind::Convert { .. }) {
                    "convert"
                } else {
                    "extra"
                };
                check_type(world, &format!("{field}.{form}.from"), from, true)?;
                check_type(world, &format!("{field}.{form}.to"), to, false)
            }
            LineKind::Stat { .. } => Ok(()),
        }
    }
}

/// `kind` a tag name.
fn check_kind(kind: &str) -> Result<(), String> {
    if valid_tag(kind) {
        Ok(())
    } else {
        Err(format!(
            "`{kind}` is not a damage type (`[a-z][a-z0-9_]*` segments joined by dots)"
        ))
    }
}

/// A skill's line as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SkillLineFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    modifier: Option<Modifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added: Option<BTreeMap<String, [f32; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pen: Option<BTreeMap<String, f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    convert: Option<ConversionRuleDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    extra: Option<ConversionRuleDef>,
    #[serde(default, skip_serializing_if = "TagFilter::is_everything")]
    with: TagFilter,
    #[serde(
        default,
        rename = "while",
        skip_serializing_if = "TagFilter::is_everything"
    )]
    holder: TagFilter,
    #[serde(default, skip_serializing_if = "TagFilter::is_everything")]
    against: TagFilter,
}

/// The one entry of a one-type map.
fn only<T>(map: BTreeMap<String, T>, field: &str) -> Result<(String, T), String> {
    let mut entries = map.into_iter();
    match (entries.next(), entries.next()) {
        (Some(entry), None) => Ok(entry),
        _ => Err(format!("{field}: exactly one damage type")),
    }
}

impl TryFrom<SkillLineFields> for SkillLine {
    type Error = String;

    fn try_from(fields: SkillLineFields) -> Result<Self, Self::Error> {
        let kinds = usize::from(fields.stat.is_some() || fields.modifier.is_some())
            + usize::from(fields.added.is_some())
            + usize::from(fields.pen.is_some())
            + usize::from(fields.convert.is_some())
            + usize::from(fields.extra.is_some());
        if kinds != 1 {
            return Err(
                "a line is one of `stat` with `modifier`, `added`, `pen`, `convert` or `extra`"
                    .to_string(),
            );
        }
        let kind = if let Some(added) = fields.added {
            let (kind, range) = only(added, "added")?;
            LineKind::Added { kind, range }
        } else if let Some(pen) = fields.pen {
            let (kind, points) = only(pen, "pen")?;
            LineKind::Pen { kind, points }
        } else if let Some(rule) = fields.convert {
            let (from, to, pct) = rule.into_parts();
            LineKind::Convert { from, to, pct }
        } else if let Some(rule) = fields.extra {
            let (from, to, pct) = rule.into_parts();
            LineKind::Extra { from, to, pct }
        } else if let (Some(stat), Some(modifier)) = (fields.stat, fields.modifier) {
            LineKind::Stat { stat, modifier }
        } else {
            return Err("a stat line names its `stat` and its `modifier`".to_string());
        };
        let line = Self {
            kind,
            scope: Scope::new()
                .with(fields.with)
                .while_holder(fields.holder)
                .against(fields.against),
        };
        line.check()?;
        Ok(line)
    }
}

impl From<SkillLine> for SkillLineFields {
    fn from(line: SkillLine) -> Self {
        let rule = |from: &str, to: &str, pct: f32| {
            ConversionRuleDef::new(from, to, pct).expect("a line's rule was checked when made")
        };
        let mut fields = Self {
            stat: None,
            modifier: None,
            added: None,
            pen: None,
            convert: None,
            extra: None,
            with: line.scope.context_filter().clone(),
            holder: line.scope.holder_filter().clone(),
            against: line.scope.against_filter().clone(),
        };
        match line.kind {
            LineKind::Stat { stat, modifier } => {
                fields.stat = Some(stat);
                fields.modifier = Some(modifier);
            }
            LineKind::Added { kind, range } => fields.added = Some(BTreeMap::from([(kind, range)])),
            LineKind::Pen { kind, points } => fields.pen = Some(BTreeMap::from([(kind, points)])),
            LineKind::Convert { from, to, pct } => fields.convert = Some(rule(&from, &to, pct)),
            LineKind::Extra { from, to, pct } => fields.extra = Some(rule(&from, &to, pct)),
        }
        fields
    }
}

/// An ability's skill sheet (§4.2 of the ARPG design): its bases (damage
/// ranges by type, a crit chance, a cost, a cooldown), the curves its level
/// reads them through, and its skill-local lines, which reach this skill
/// alone under the source `skill:<entry>`. A use resolves it in the
/// ability's tags, its context: the sheet's cost and cooldown replace the
/// ability's, and every payload carries the [`Delivery`] it makes.
///
/// A level curve maps the skill's level (its owner's entry's, plus `add`
/// lines on `skill_level`, from 1 to 40) to a base: a scalar's value, or a
/// damage range's midpoint, the range scaled to it about its midpoint, so
/// it keeps its spread.
///
/// Data form, the `skill` of an `abilities` entry:
///
/// ```json
/// {
///   "bases": { "damage": { "damage.fire": [16, 24] }, "crit_chance": 0.06, "cost": 12 },
///   "levels": { "damage": "cinder_damage_by_level" },
///   "lines": [{ "stat": "damage", "modifier": { "mul": 0.8 } }]
/// }
/// ```
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Line, Modifier, Skill, SkillLine};
///
/// let cinder = Skill::new()
///     .with_damage("damage.fire", 16.0, 24.0)
///     .with_crit_chance(0.06)
///     .with_cost(12.0)
///     .with_line(SkillLine::stat("cost", Line::new(Modifier::Mul(1.3))));
/// assert_eq!(cinder.damage().collect::<Vec<_>>(), [("damage.fire", [16.0, 24.0])]);
/// assert_eq!((cinder.crit_chance(), cinder.cost(), cinder.cooldown()), (Some(0.06), Some(12.0), None));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SkillDef", into = "SkillDef")]
pub struct Skill {
    /// Sorted by type.
    damage: Vec<(String, [f32; 2])>,
    crit_chance: Option<f32>,
    cost: Option<f32>,
    cooldown: Option<f32>,
    /// Sorted by base.
    levels: Vec<(String, DataRef<Curve>)>,
    lines: Vec<SkillLine>,
}

impl Skill {
    /// A skill with no base, curve or line.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The skill, with `min` to `max` of `kind` among its damage bases.
    ///
    /// # Panics
    ///
    /// When `kind` is not a tag name, or the range is not finite with
    /// `0 <= min <= max`.
    #[must_use]
    #[track_caller]
    pub fn with_damage(mut self, kind: &str, min: f32, max: f32) -> Self {
        if let Err(error) = SkillLine::added(kind, min, max).check() {
            panic!("Skill::with_damage: {error}");
        }
        let at = self
            .damage
            .partition_point(|(name, _)| name.as_str() < kind);
        if self.damage.get(at).is_some_and(|(name, _)| name == kind) {
            self.damage[at].1 = [min, max];
        } else {
            self.damage.insert(at, (kind.to_owned(), [min, max]));
        }
        self
    }

    /// The skill, with a base crit chance in `[0, 1]`.
    ///
    /// # Panics
    ///
    /// When `chance` is outside `[0, 1]`.
    #[must_use]
    #[track_caller]
    pub fn with_crit_chance(mut self, chance: f32) -> Self {
        assert!(
            (0.0..=1.0).contains(&chance),
            "Skill::with_crit_chance: {chance} is not in [0, 1]"
        );
        self.crit_chance = Some(chance);
        self
    }

    /// The skill, with a base cost of at least 0, which replaces its
    /// ability's amount.
    ///
    /// # Panics
    ///
    /// When `cost` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_cost(mut self, cost: f32) -> Self {
        assert!(
            cost.is_finite() && cost >= 0.0,
            "Skill::with_cost: must be finite and >= 0 (got {cost})"
        );
        self.cost = Some(cost);
        self
    }

    /// The skill, with a base cooldown in seconds, which replaces its
    /// ability's.
    ///
    /// # Panics
    ///
    /// When `seconds` is not finite and at least 0.
    #[must_use]
    #[track_caller]
    pub fn with_cooldown(mut self, seconds: f32) -> Self {
        assert!(
            seconds.is_finite() && seconds >= 0.0,
            "Skill::with_cooldown: must be finite and >= 0 (got {seconds})"
        );
        self.cooldown = Some(seconds);
        self
    }

    /// The skill, reading `base` (`damage`, `crit_chance`, `cost` or
    /// `cooldown`) at its level through `curve`.
    ///
    /// # Panics
    ///
    /// When `base` is none of those.
    #[must_use]
    #[track_caller]
    pub fn with_level_curve(mut self, base: &str, curve: DataRef<Curve>) -> Self {
        assert!(
            LEVEL_BASES.contains(&base),
            "Skill::with_level_curve: `{base}` is not one of {LEVEL_BASES:?}"
        );
        self.levels.retain(|(name, _)| name != base);
        let at = self
            .levels
            .partition_point(|(name, _)| name.as_str() < base);
        self.levels.insert(at, (base.to_owned(), curve));
        self
    }

    /// The skill, with `line` among its skill-local lines.
    #[must_use]
    pub fn with_line(mut self, line: SkillLine) -> Self {
        self.lines.push(line);
        self
    }

    /// Its damage bases, by type.
    pub fn damage(&self) -> impl Iterator<Item = (&str, [f32; 2])> {
        self.damage
            .iter()
            .map(|(kind, range)| (kind.as_str(), *range))
    }

    /// Its base crit chance.
    #[must_use]
    pub fn crit_chance(&self) -> Option<f32> {
        self.crit_chance
    }

    /// Its base cost.
    #[must_use]
    pub fn cost(&self) -> Option<f32> {
        self.cost
    }

    /// Its base cooldown.
    #[must_use]
    pub fn cooldown(&self) -> Option<f32> {
        self.cooldown
    }

    /// Its skill-local lines, in order.
    #[must_use]
    pub fn lines(&self) -> &[SkillLine] {
        &self.lines
    }

    /// The load's check: its damage types, curves and lines' tags.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        for (kind, _) in &self.damage {
            check_type(world, &format!("{field}.bases.damage.{kind}"), kind, true)?;
        }
        for (base, curve) in &self.levels {
            curve.check(world, &format!("{field}.levels.{base}"))?;
        }
        for (i, line) in self.lines.iter().enumerate() {
            line.check_in(world, &format!("{field}.lines.{i}"))?;
        }
        Ok(())
    }
}

/// A skill as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SkillDef {
    #[serde(default)]
    bases: BasesDef,
    #[serde(default)]
    levels: BTreeMap<String, DataRef<Curve>>,
    #[serde(default)]
    lines: Vec<SkillLine>,
}

/// A skill's bases as data writes them.
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BasesDef {
    #[serde(default)]
    damage: BTreeMap<String, [f32; 2]>,
    #[serde(default, deserialize_with = "optional_unit")]
    crit_chance: Option<f32>,
    #[serde(default, deserialize_with = "optional_non_negative")]
    cost: Option<f32>,
    #[serde(default, deserialize_with = "optional_non_negative")]
    cooldown: Option<f32>,
}

/// An optional number in `[0, 1]`, for `deserialize_with`.
fn optional_unit<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f32>, D::Error> {
    use serde::de::Error as _;
    match Option::<f32>::deserialize(deserializer)? {
        Some(value) if !(0.0..=1.0).contains(&value) => {
            Err(D::Error::custom(format!("must be in [0, 1] (got {value})")))
        }
        value => Ok(value),
    }
}

impl TryFrom<SkillDef> for Skill {
    type Error = String;

    fn try_from(def: SkillDef) -> Result<Self, Self::Error> {
        let mut skill = Skill::new();
        for (kind, [min, max]) in def.bases.damage {
            SkillLine {
                kind: LineKind::Added {
                    kind: kind.clone(),
                    range: [min, max],
                },
                scope: Scope::new(),
            }
            .check()
            .map_err(|e| format!("bases.damage: {e}"))?;
            skill.damage.push((kind, [min, max]));
        }
        if let Some(chance) = def.bases.crit_chance
            && !(0.0..=1.0).contains(&chance)
        {
            return Err(format!("bases.crit_chance: {chance} is not in [0, 1]"));
        }
        for (field, value) in [("cost", def.bases.cost), ("cooldown", def.bases.cooldown)] {
            if let Some(value) = value
                && !(value.is_finite() && value >= 0.0)
            {
                return Err(format!(
                    "bases.{field}: must be finite and >= 0 (got {value})"
                ));
            }
        }
        skill.crit_chance = def.bases.crit_chance;
        skill.cost = def.bases.cost;
        skill.cooldown = def.bases.cooldown;
        for (base, curve) in def.levels {
            if !LEVEL_BASES.contains(&base.as_str()) {
                return Err(format!("levels: `{base}` is not one of {LEVEL_BASES:?}"));
            }
            skill.levels.push((base, curve));
        }
        skill.lines = def.lines;
        Ok(skill)
    }
}

impl From<Skill> for SkillDef {
    fn from(skill: Skill) -> Self {
        Self {
            bases: BasesDef {
                damage: skill.damage.into_iter().collect(),
                crit_chance: skill.crit_chance,
                cost: skill.cost,
                cooldown: skill.cooldown,
            },
            levels: skill.levels.into_iter().collect(),
            lines: skill.lines,
        }
    }
}

/// A read's sums as a snapshot keeps them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Partial {
    add: f64,
    factor: f64,
    product: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    zero: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    over: Option<f64>,
}

impl Partial {
    fn of(sums: &Sums) -> Self {
        let (add, factor, product, zero, over) = sums.numbers();
        Self {
            add,
            factor,
            product,
            zero,
            over,
        }
    }

    fn sums(self) -> Sums {
        Sums::from_numbers(self.add, self.factor, self.product, self.zero, self.over)
    }
}

/// One damage part a delivery carries: its type and history, its range,
/// its use-time sums on both tracks and its penetration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeliveryPart {
    pub(crate) kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) history: Vec<String>,
    pub(crate) range: [f32; 2],
    hit: Partial,
    dot: Partial,
    #[serde(default)]
    pen: f32,
}

/// What a skill's use snapshots for its hits (§3.5 of the ARPG design): per
/// damage part its type, history, range, use-time sums on the hit and the
/// damage-over-time tracks and penetration; the crit chance and
/// multiplier sums; the skill's context, its holder's tags at the use and
/// the lines left for the hit; the hit's state tags; and the holder, for
/// credit while it lives. A `spawn` payload adds a clone to what it spawns,
/// a `hitbox` payload arms its hitbox with one for that activation; a hit
/// from either resolves from it ([`DamageRequest::from_delivery`]), never
/// reading its user again, so a user who died, swapped gear or lost a buff
/// changes nothing in flight. It is saved, the holder by save ID, and a hit
/// after its holder is gone has no source. While [`KitExplain`] is kept it
/// carries its sheet's record to its hits' records, never saved.
///
/// [`DamageRequest::from_delivery`]: crate::DamageRequest::from_delivery
#[derive(Debug, Clone, PartialEq)]
pub struct Delivery {
    holder: Option<Entity>,
    /// The skill's level at the use.
    level: f32,
    holder_tags: Tags,
    context: Vec<String>,
    state: Tags,
    pub(crate) parts: Vec<DeliveryPart>,
    crit_chance: Partial,
    crit_multiplier: Partial,
    deferred: Vec<(String, SkillLine)>,
    /// The hitbox activation it arms, for one a `hitbox` payload placed.
    activation: Option<u32>,
    /// Its sheet's why-record, while one is kept.
    explain: Option<Box<SheetRecord>>,
}

impl Delivery {
    /// The entity whose use made it, while it is known.
    #[must_use]
    pub fn holder(&self) -> Option<Entity> {
        self.holder
    }

    /// Its skill's level at the use, from 1 to 40.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Its parts' damage types and ranges, in order.
    pub fn ranges(&self) -> impl Iterator<Item = (&str, [f32; 2])> {
        self.parts
            .iter()
            .map(|part| (part.kind.as_str(), part.range))
    }

    /// Its skill's context at the use.
    #[must_use]
    pub fn context(&self) -> &[String] {
        &self.context
    }

    /// Its holder's tags at the use, which the hit reads for every
    /// `against`, a defender's lines included.
    #[must_use]
    pub fn holder_tags(&self) -> &Tags {
        &self.holder_tags
    }

    /// The state tags its hits carry.
    #[must_use]
    pub fn state(&self) -> &Tags {
        &self.state
    }

    /// It, with the state tags `tags` (`delivery.chained`) on its hits.
    #[must_use]
    pub fn with_state(mut self, tags: Tags) -> Self {
        self.state = tags;
        self
    }

    /// It, arming the hitbox activation `activation`.
    pub(crate) fn armed(mut self, activation: u32) -> Self {
        self.activation = Some(activation);
        self
    }

    /// Whether a hit by an entity whose hitbox is at `activation` carries
    /// it: one a spawn carries always, one an armed hitbox carries for its
    /// activation alone.
    pub(crate) fn reaches(&self, activation: Option<u32>) -> bool {
        self.activation.is_none() || self.activation == activation
    }

    /// The holder while it lives in `world`.
    pub(crate) fn credit(&self, world: &World) -> Option<Entity> {
        self.holder.filter(|&holder| world.is_alive(holder))
    }

    /// The parts' amounts at a hit on a target with `target` tags, in the
    /// hit's `context` (its track and the state tags among it): the
    /// deferred lines folded in, one range draw `u` shared by every part,
    /// the factors, then the crit draw `crit` under the chance; each part's
    /// amount in `f64` and its penetration, and whether it crit. With a
    /// `record`, each part's lines, sums and origins go in it with the
    /// draws and the sheet's record.
    pub(crate) fn amounts(
        &self,
        target: Option<&Tags>,
        u: f32,
        crit: f32,
        out: &mut Vec<(f64, f32)>,
        mut record: Option<&mut HitRecord>,
    ) -> bool {
        out.clear();
        let mut chance = self.crit_chance.sums();
        let mut multiplier = self.crit_multiplier.sums();
        let at_hit = [
            TagSet::Names(&self.context),
            TagSet::Name(TRACK_HIT),
            TagSet::Tags(&self.state),
        ];
        let read = Read::new(&at_hit)
            .holder(Some(&self.holder_tags))
            .against(target);
        for (_, line) in &self.deferred {
            if let LineKind::Stat { stat, modifier } = &line.kind
                && read.reach_scope(&line.scope) == Reach::Applies
            {
                match stat.as_str() {
                    Stats::CRIT_CHANCE => chance.fold(*modifier),
                    Stats::CRIT_MULTIPLIER => multiplier.fold(*modifier),
                    _ => {}
                }
            }
        }
        for (at, part) in self.parts.iter().enumerate() {
            let sets = [
                TagSet::Names(&self.context),
                TagSet::Name(TRACK_HIT),
                TagSet::Tags(&self.state),
                TagSet::Name(&part.kind),
                TagSet::Names(&part.history),
            ];
            let read = Read::new(&sets)
                .holder(Some(&self.holder_tags))
                .against(target);
            let mut hit = part.hit.sums();
            let mut pen = part.pen;
            for (_, line) in &self.deferred {
                if read.reach_scope(&line.scope) != Reach::Applies {
                    continue;
                }
                match &line.kind {
                    LineKind::Stat { stat, modifier } if stat == Stats::DAMAGE => {
                        hit.fold(*modifier);
                    }
                    LineKind::Pen { kind, points } if covers(&part.kind, kind) => pen += points,
                    _ => {}
                }
            }
            let [min, max] = part.range;
            let rolled = f64::from(min) + f64::from(u) * (f64::from(max) - f64::from(min));
            out.push((rolled * factor_of(&hit), pen.clamp(0.0, 1.0)));
            if let Some(record) = record.as_deref_mut() {
                let noted = self.noted(at, target, &read, &hit, rolled);
                record.parts.push(noted);
            }
        }
        let chance = chance.value(0.0, 1.0);
        let multiplier = multiplier.value(1.0, f32::INFINITY);
        let roll = crit;
        let crit = roll < chance;
        if crit {
            for (amount, _) in out.iter_mut() {
                *amount *= f64::from(multiplier);
            }
        }
        if let Some(record) = record {
            record.draw = Some(u);
            record.crit = CritRecord::Drawn {
                chance,
                roll,
                multiplier,
            };
            record.sheet.clone_from(&self.explain);
            if crit {
                for part in &mut record.parts {
                    part.crit = f64::from(multiplier);
                    part.before *= part.crit;
                }
            }
        }
        crit
    }

    /// Part `at`'s record at a hit `read` met: the lines of its use from the
    /// sheet's record, those the use left for the hit as the read met them,
    /// its origins, its sums `hit` and its amount `rolled` at the draw.
    fn noted(
        &self,
        at: usize,
        target: Option<&Tags>,
        read: &Read<'_>,
        hit: &Sums,
        rolled: f64,
    ) -> HitPartRecord {
        let part = &self.parts[at];
        let used = self.explain.as_ref().and_then(|sheet| sheet.parts.get(at));
        let mut lines: Vec<LineRecord> = used.map_or_else(Vec::new, |used| {
            used.lines
                .iter()
                .filter(|line| line.outcome != LineOutcome::Deferred)
                .cloned()
                .collect()
        });
        let bare_sets = [
            TagSet::Names(&self.context),
            TagSet::Name(TRACK_HIT),
            TagSet::Tags(&self.state),
            TagSet::Name(&part.kind),
        ];
        let bare = Read::new(&bare_sets)
            .holder(Some(&self.holder_tags))
            .against(target);
        for (source, line) in &self.deferred {
            let (stat, modifier) = match &line.kind {
                LineKind::Stat { stat, modifier } if stat == Stats::DAMAGE => {
                    (Stats::DAMAGE, *modifier)
                }
                LineKind::Pen { kind, points } if covers(&part.kind, kind) => {
                    ("pen", Modifier::Add(*points))
                }
                _ => continue,
            };
            let outcome = outcome(
                read.reach_scope(&line.scope),
                || bare.reach_scope(&line.scope),
                |skip| why(skip, &line.scope, read, Some(&self.holder_tags), target),
            );
            lines.push(LineRecord {
                source: source.clone(),
                stat: stat.to_owned(),
                modifier,
                scope: line.scope.clone(),
                read: ReadAt::Hit,
                outcome,
            });
        }
        let factor = factor_of(hit);
        HitPartRecord {
            tags: vec![part.kind.clone()],
            history: part.history.clone(),
            origins: used.map_or_else(Vec::new, |used| used.origins.clone()),
            base: rolled,
            lines,
            pct: hit.pct(),
            mul: hit.mul(),
            factor,
            crit: 1.0,
            before: rolled * factor,
            resist: None,
            taken: 1.0,
            after: 0.0,
            immune: false,
        }
    }
}

/// The damage factor `(1 + Σpct) × Πmul` of a part's sums, 0 below 0; an
/// override wins.
fn factor_of(sums: &Sums) -> f64 {
    if let Some(over) = sums.override_value() {
        return over.max(0.0);
    }
    ((1.0 + sums.pct()) * sums.mul()).max(0.0)
}

/// A delivery as a save writes it, its holder by reference.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliverySave {
    level: f32,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    holder_tags: Tags,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    context: Vec<String>,
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    state: Tags,
    parts: Vec<DeliveryPart>,
    crit_chance: Partial,
    crit_multiplier: Partial,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    deferred: Vec<(String, SkillLine)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    activation: Option<u32>,
}

impl From<&Delivery> for DeliverySave {
    fn from(delivery: &Delivery) -> Self {
        Self {
            level: delivery.level,
            holder_tags: delivery.holder_tags.clone(),
            context: delivery.context.clone(),
            state: delivery.state.clone(),
            parts: delivery.parts.clone(),
            crit_chance: delivery.crit_chance,
            crit_multiplier: delivery.crit_multiplier,
            deferred: delivery.deferred.clone(),
            activation: delivery.activation,
        }
    }
}

impl From<DeliverySave> for Delivery {
    fn from(save: DeliverySave) -> Self {
        Self {
            holder: None,
            level: save.level.clamp(LEVELS.0, LEVELS.1),
            holder_tags: save.holder_tags,
            context: save.context,
            state: save.state,
            parts: save.parts,
            crit_chance: save.crit_chance,
            crit_multiplier: save.crit_multiplier,
            deferred: save.deferred,
            activation: save.activation,
            explain: None,
        }
    }
}

impl Serialize for Delivery {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        DeliverySave::from(self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Delivery {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        DeliverySave::deserialize(deserializer).map(Delivery::from)
    }
}

/// The descriptions of a skill's shapes: `skill`, `skill_bases` and
/// `skill_line`.
pub(crate) fn describe_skills(
    world: &mut World,
    skill: &'static str,
    bases: &'static str,
    line: &'static str,
) {
    let filter = |name: &'static str, doc: &'static str| {
        FieldDescription::new(name, FieldType::Filter, doc).default("null")
    };
    let shapes = [
        ComponentDescription::new(
            skill,
            "An ability's skill sheet: its bases, the curves its level reads them through, and its skill-local lines.",
        )
        .field(
            FieldDescription::new(
                "bases",
                FieldType::Shape(bases),
                "the sheet's own values before any line",
            )
            .default(r#"{ "damage": {}, "crit_chance": null, "cost": null, "cooldown": null }"#),
        )
        .field(
            FieldDescription::new(
                "levels",
                FieldType::Object("base (`damage`, `crit_chance`, `cost` or `cooldown`) → curves ID"),
                "each read at the skill's level, its owner's entry's plus `add` lines on `skill_level`, 1 to 40; a damage range scaled about its midpoint to the curve's value",
            )
            .default("{}"),
        )
        .field(
            FieldDescription::new(
                "lines",
                FieldType::List(Box::new(FieldType::Shape(line))),
                "skill-local lines, under the source `skill:<entry>`",
            )
            .default("[]"),
        )
        .sample(r#"{ "bases": { "damage": { "damage.fire": [16, 24] }, "cost": 12 } }"#)
        .roundtrip(round_trip::<SkillDef>),
        ComponentDescription::new(bases, "A skill sheet's own values.")
            .field(
                FieldDescription::new(
                    "damage",
                    FieldType::Object("damage type → [min, max]"),
                    "a part's range by type, 0 <= min <= max; the load checks each type",
                )
                .default("{}"),
            )
            .field(
                FieldDescription::new(
                    "crit_chance",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Inclusive(1.0),
                    },
                    "the chance its hits crit, before lines",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "cost",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "replaces the ability's cost amount, its stat the ability's",
                )
                .default("null"),
            )
            .field(
                FieldDescription::new(
                    "cooldown",
                    FieldType::F32 {
                        min: Bound::Inclusive(0.0),
                        max: Bound::Open,
                    },
                    "replaces the ability's cooldown, in seconds",
                )
                .default("null"),
            )
            .sample("{}")
            .roundtrip(round_trip::<BasesDef>),
        ComponentDescription::new(
            line,
            "A line a skill reads: a stat's modifier, flat damage of a type, or penetration for a type's parts, with its filters.",
        )
        .field(
            FieldDescription::new(
                "stat",
                FieldType::Text,
                "the stat its `modifier` changes; beside `modifier` alone",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new("modifier", FieldType::Shape(MODIFIER), "the change, with `stat`")
                .default("null"),
        )
        .field(
            FieldDescription::new(
                "added",
                FieldType::Object("one damage type → [min, max]"),
                "flat damage added to that type's part; may not wait for the hit",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "pen",
                FieldType::Object("one damage type → points"),
                "penetration for the parts of that type and its descendants, by type or history",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "convert",
                FieldType::Shape(CONVERSION_RULE),
                "a share of each part of `from` (or a descendant) converted to `to` at a use, its history the part's and its type; may not wait for the hit",
            )
            .default("null"),
        )
        .field(
            FieldDescription::new(
                "extra",
                FieldType::Shape(CONVERSION_RULE),
                "a share of each part whose type or history carries `from` added as `to` after the conversion; may not wait for the hit",
            )
            .default("null"),
        )
        .field(filter("with", "read only where it passes the context: the skill's tags, a part's track, type and history"))
        .field(filter("while", "read only while it passes the holder's tags at the use"))
        .field(filter(
            "against",
            "read at the hit, against the target's tags; only `damage`, `crit_chance`, `crit_multiplier` and `pen` may wait for the hit",
        ))
        .sample(r#"{ "stat": "damage", "modifier": { "mul": 0.8 } }"#)
        .roundtrip(round_trip::<SkillLine>),
    ];
    for shape in shapes {
        describe_shape(world, shape);
    }
}

/// The key a save holds a delivery's holder under.
const HOLDER: &str = "holder";

/// [`Delivery`] as a save holds it, its holder by save ID: one gone before
/// the save, or without a save ID, comes back as none.
pub(crate) fn delivery_saved() -> SaveOptions<Delivery> {
    SaveOptions::<Delivery>::form::<DeliverySave>().refs(RefCodec {
        save: |delivery, out: &mut RefWriter<'_>| {
            if let Some(holder) = delivery.holder {
                out.put(HOLDER, holder);
            }
        },
        load: |delivery, refs: &RefReader<'_>| {
            delivery.holder = refs.first(HOLDER);
        },
        keys: |delivery| {
            delivery
                .holder
                .map(|_| HOLDER.to_owned())
                .into_iter()
                .collect()
        },
    })
}

/// What a use of a skilled ability resolved: its cost and cooldown,
/// which replace the ability's, and the delivery its payloads carry.
#[derive(Debug, Clone)]
pub(crate) struct Sheet {
    pub(crate) cost: Option<f32>,
    pub(crate) cooldown: f32,
    pub(crate) delivery: Delivery,
}

/// A line a sheet reads, with its source: one of the holder's stat lines,
/// or a skill line.
#[derive(Clone, Copy)]
enum Source<'a> {
    Stat(&'a str, StatLine<'a>),
    Skill(&'a str, &'a SkillLine),
}

impl<'a> Source<'a> {
    /// The source key that placed the line.
    fn source(self) -> &'a str {
        match self {
            Source::Stat(_, stat_line) => stat_line.source(),
            Source::Skill(source, _) => source,
        }
    }
}

/// `curve`'s definition.
fn curve_of<'a>(world: &'a World, curve: &'a DataRef<Curve>) -> Option<&'a Curve> {
    match world.get_resource::<DataRegistry>() {
        Some(data) => curve.resolve(data),
        None => curve.as_inline(),
    }
}

/// A part between its base and its snapshot: its type, its history (the
/// types it came from, sorted), its range and, while a record is kept,
/// where it came from.
struct Working {
    kind: String,
    history: Vec<String>,
    range: [f64; 2],
    origins: Vec<PartOrigin>,
}

impl Working {
    /// A base part of `kind` over `range`, the skill's own or an `added`
    /// line's, its origin kept while `explaining`.
    fn base(kind: &str, range: [f64; 2], added: Option<&str>, explaining: bool) -> Self {
        let origins = if explaining {
            vec![PartOrigin::new(kind, range.map(|v| v as f32), added)]
        } else {
            Vec::new()
        };
        Self {
            kind: kind.to_owned(),
            history: Vec::new(),
            range,
            origins,
        }
    }

    /// Its origins, each with `step` after its own: none while none is kept.
    fn stepped(&self, step: impl FnOnce() -> OriginStep) -> Vec<PartOrigin> {
        if self.origins.is_empty() {
            Vec::new()
        } else {
            PartOrigin::stepped(&self.origins, &step())
        }
    }
}

/// A conversion or extra line's rule: its types and share, its scope and
/// its source.
struct Rule<'a> {
    from: &'a str,
    to: &'a str,
    pct: f32,
    scope: &'a Scope,
    source: &'a str,
}

/// A line's rule of one kind: [`SkillLine::as_convert`] or
/// [`SkillLine::as_extra`].
type RuleOf = fn(&SkillLine) -> Option<(&str, &str, f32)>;

/// Whether `rule` reaches a part of `kind` and `history` at the use: its
/// filters read in the skill's context and the part's type and history.
fn rule_reaches(
    scope: &Scope,
    context: &[String],
    holder: Option<&Tags>,
    kind: &str,
    history: &[String],
) -> bool {
    let sets = [
        TagSet::Names(context),
        TagSet::Name(kind),
        TagSet::Names(history),
    ];
    Read::new(&sets).holder(holder).reach_scope(scope) == Reach::Applies
}

/// `history` with `kind` added, sorted and deduplicated.
fn grown(history: &[String], kind: &str) -> Vec<String> {
    let mut grown = history.to_vec();
    grown.push(kind.to_owned());
    grown.sort();
    grown.dedup();
    grown
}

/// §6.2 step 6: in one pass, each part's matching `convert` lines (its type
/// `from` or a descendant) take their shares, normalized past 1, each a part
/// of `to` whose history is the part's plus its type; the remainder stays,
/// first.
fn convert(
    parts: Vec<Working>,
    rules: &[Rule<'_>],
    context: &[String],
    holder: Option<&Tags>,
) -> Vec<Working> {
    let mut out = Vec::with_capacity(parts.len());
    for part in parts {
        let matching: Vec<&Rule<'_>> = rules
            .iter()
            .filter(|rule| {
                covers(&part.kind, rule.from)
                    && rule_reaches(rule.scope, context, holder, &part.kind, &part.history)
            })
            .collect();
        if matching.is_empty() {
            out.push(part);
            continue;
        }
        let sum: f64 = matching.iter().map(|rule| f64::from(rule.pct)).sum();
        if sum < 1.0 {
            let keep = 1.0 - sum;
            out.push(Working {
                kind: part.kind.clone(),
                history: part.history.clone(),
                range: part.range.map(|v| v * keep),
                origins: part.stepped(|| OriginStep::Kept(keep as f32)),
            });
        }
        for rule in matching {
            let share = if sum > 1.0 {
                f64::from(rule.pct) / sum
            } else {
                f64::from(rule.pct)
            };
            out.push(Working {
                kind: rule.to.to_owned(),
                history: grown(&part.history, &part.kind),
                range: part.range.map(|v| v * share),
                origins: part.stepped(|| OriginStep::Converted {
                    share: share as f32,
                    to: rule.to.to_owned(),
                    source: rule.source.to_owned(),
                }),
            });
        }
    }
    out
}

/// §6.2 step 7: in one pass over the converted parts, each `extra` line adds
/// `pct` of every part whose type or history carries `from` as a part of
/// `to`, its history that part's plus its type; an extra part makes none.
fn extra(
    parts: Vec<Working>,
    rules: &[Rule<'_>],
    context: &[String],
    holder: Option<&Tags>,
) -> Vec<Working> {
    let mut added = Vec::new();
    for part in &parts {
        for rule in rules {
            let carries = covers(&part.kind, rule.from)
                || part.history.iter().any(|kind| covers(kind, rule.from));
            if carries && rule_reaches(rule.scope, context, holder, &part.kind, &part.history) {
                let pct = f64::from(rule.pct);
                added.push(Working {
                    kind: rule.to.to_owned(),
                    history: grown(&part.history, &part.kind),
                    range: part.range.map(|v| v * pct),
                    origins: part.stepped(|| OriginStep::Extra {
                        share: rule.pct,
                        to: rule.to.to_owned(),
                        source: rule.source.to_owned(),
                    }),
                });
            }
        }
    }
    let mut parts = parts;
    parts.extend(added);
    parts
}

/// §6.2 step 8: parts of equal type and history merged, in first-appearance
/// order, their origins together; a part of nothing drops.
fn merge(parts: Vec<Working>) -> Vec<Working> {
    let mut out: Vec<Working> = Vec::with_capacity(parts.len());
    for part in parts {
        if part.range[0] <= 0.0 && part.range[1] <= 0.0 {
            continue;
        }
        match out
            .iter_mut()
            .find(|known| known.kind == part.kind && known.history == part.history)
        {
            Some(known) => {
                known.range[0] += part.range[0];
                known.range[1] += part.range[1];
                known.origins.extend(part.origins);
            }
            None => out.push(part),
        }
    }
    out
}

/// Resolves `entry`'s sheet for `holder`'s use of `ability` with `skill`
/// (§6.2 steps 1–3, 5 and 9 of the ARPG design), from the holder's stat
/// lines, its holder-wide skill lines and the skill's own and placed
/// lines, read in the ability's tags.
pub(crate) fn resolve(
    world: &World,
    holder: Entity,
    entry: &str,
    ability: &Ability,
    skill: &Skill,
) -> Sheet {
    let stats = world.get::<Stats>(holder);
    let holder_tags = world.get::<Tags>(holder);
    let abilities = world.get::<Abilities>(holder);
    let context = ability.tags();
    let own_source = format!("skill:{entry}");
    let mut local: Vec<(&str, &SkillLine)> = skill
        .lines
        .iter()
        .map(|line| (own_source.as_str(), line))
        .collect();
    if let Some(abilities) = abilities {
        local.extend(abilities.skill_lines(entry));
        local.extend(abilities.holder_lines());
    }
    let lines_on = |stat: &'static str| {
        stats
            .into_iter()
            .flat_map(move |stats| {
                stats
                    .lines_on(stat)
                    .map(move |line| Source::Stat(stat, line))
            })
            .chain(
                local
                    .iter()
                    .filter(move |(_, line)| line.as_stat().is_some_and(|(name, _)| name == stat))
                    .map(|&(source, line)| Source::Skill(source, line)),
            )
    };
    let reach = |read: &Read<'_>, line: Source<'_>| match line {
        Source::Stat(_, StatLine::Modifier(..)) => Reach::Applies,
        Source::Stat(_, stat_line) => stat_line
            .scope()
            .map_or(Reach::Applies, |scope| read.reach_scope(scope)),
        Source::Skill(_, line) => read.reach_scope(&line.scope),
    };
    let modifier_of = |line: Source<'_>| match line {
        Source::Stat(_, stat_line) => stat_line.modifier(),
        Source::Skill(_, line) => line
            .as_stat()
            .map_or(Modifier::Add(0.0), |(_, modifier)| modifier),
    };
    let scope_of = |line: Source<'_>| match line {
        Source::Stat(_, stat_line) => stat_line.scope().cloned().unwrap_or_default(),
        Source::Skill(_, line) => line.scope.clone(),
    };
    let in_context = [TagSet::Names(context)];
    let at_use = Read::new(&in_context).holder(holder_tags).at_use();
    let explain = world.get_resource::<KitExplain>();

    // The level: the entry's, plus `add` lines in the context.
    let entry_level = abilities.and_then(|a| a.level(entry)).unwrap_or(1) as f32;
    let mut level = entry_level;
    for line in lines_on(Stats::SKILL_LEVEL) {
        if let (Reach::Applies, Modifier::Add(v)) = (reach(&at_use, line), modifier_of(line)) {
            level += v;
        }
    }
    let level = level.clamp(LEVELS.0, LEVELS.1);
    let curve_at = |base: &str| {
        skill
            .levels
            .iter()
            .find(|(name, _)| name == base)
            .and_then(|(_, curve)| curve_of(world, curve))
            .map(|curve| curve.at(level))
    };

    // A sheet stat: its base, then the lines that reach it at the use.
    let sums_of = |stat: &'static str, base: f32| {
        let mut sums = Sums::new(base);
        for line in lines_on(stat) {
            if reach(&at_use, line) == Reach::Applies {
                sums.fold(modifier_of(line));
            }
        }
        sums
    };
    let crit_base = curve_at("crit_chance").or(skill.crit_chance).unwrap_or(0.0)
        + stats
            .and_then(|s| s.base(Stats::CRIT_CHANCE))
            .unwrap_or(0.0);
    let crit_chance = sums_of(Stats::CRIT_CHANCE, crit_base);
    let multiplier_base = stats
        .and_then(|s| s.base(Stats::CRIT_MULTIPLIER))
        .unwrap_or(CRIT_MULTIPLIER);
    let crit_multiplier = sums_of(Stats::CRIT_MULTIPLIER, multiplier_base);
    let cost_base = curve_at("cost")
        .or(skill.cost)
        .or(ability.cost().map(|(_, amount)| amount));
    let cost = cost_base.map(|base| {
        let scale = ability
            .cost_scale()
            .and_then(|scale| stats?.get(scale))
            .unwrap_or(1.0);
        (sums_of(Stats::COST, base).value(0.0, f32::INFINITY) * scale).max(0.0)
    });
    let cooldown_base = curve_at("cooldown")
        .or(skill.cooldown)
        .unwrap_or(ability.cooldown());
    let cooldown = sums_of(Stats::COOLDOWN, cooldown_base).value(0.0, f32::INFINITY);

    // The lines kept for the hit.
    let mut deferred: Vec<(String, SkillLine)> = Vec::new();
    for stat in [Stats::DAMAGE, Stats::CRIT_CHANCE, Stats::CRIT_MULTIPLIER] {
        for line in lines_on(stat) {
            if reach(&at_use, line) == Reach::Deferred {
                let (source, kept) = match line {
                    Source::Stat(stat, stat_line) => {
                        let Some(line) = stat_line.line() else {
                            continue;
                        };
                        (stat_line.source(), SkillLine::stat(stat, line.clone()))
                    }
                    Source::Skill(source, line) => (source, line.clone()),
                };
                deferred.push((source.to_owned(), kept));
            }
        }
    }
    for &(source, line) in &local {
        if line.as_pen().is_some() && line.deferred() {
            deferred.push((source.to_owned(), line.clone()));
        }
    }

    // The parts: each damage type's base, at the level, plus `added` lines,
    // converted, with their extras, merged.
    let to_f32 = |v: f64| v.clamp(0.0, f64::from(f32::MAX)) as f32;
    let mut ranges: Vec<Working> = Vec::new();
    let damage_curve = curve_at("damage");
    for (kind, [min, max]) in &skill.damage {
        let (mut min, mut max) = (f64::from(*min), f64::from(*max));
        let mid = f64::midpoint(min, max);
        if let Some(at) = damage_curve
            && mid > 0.0
        {
            let k = f64::from(at) / mid;
            (min, max) = (min * k, max * k);
        }
        ranges.push(Working::base(kind, [min, max], None, explain.is_some()));
    }
    for &(source, line) in &local {
        if let Some((kind, [min, max])) = line.as_added()
            && at_use.reach_scope(&line.scope) == Reach::Applies
        {
            let range = [f64::from(min), f64::from(max)];
            match ranges.iter_mut().find(|part| part.kind == kind) {
                Some(part) => {
                    part.range[0] += range[0];
                    part.range[1] += range[1];
                    if explain.is_some() {
                        part.origins
                            .push(PartOrigin::new(kind, [min, max], Some(source)));
                    }
                }
                None => ranges.push(Working::base(kind, range, Some(source), explain.is_some())),
            }
        }
    }
    ranges.retain(|part| part.range[0] > 0.0 || part.range[1] > 0.0);
    let rules = |kind: RuleOf| {
        local
            .iter()
            .filter_map(|&(source, line)| {
                kind(line).map(|(from, to, pct)| Rule {
                    from,
                    to,
                    pct,
                    scope: &line.scope,
                    source,
                })
            })
            .collect::<Vec<_>>()
    };
    let converted = convert(ranges, &rules(SkillLine::as_convert), context, holder_tags);
    let merged = merge(extra(
        converted,
        &rules(SkillLine::as_extra),
        context,
        holder_tags,
    ));

    // The why-record, while one is kept: each sheet stat's lines and each
    // part's on the hit track, read as the use read them.
    let explained = explain.map(|explain| {
        let stat_record = |stat: &'static str, base: f32, clamp: [f32; 2], value: f32| {
            let mut sums = Sums::new(base);
            let mut lines = Vec::new();
            for line in lines_on(stat) {
                let modifier = modifier_of(line);
                let scope = scope_of(line);
                let mut outcome = outcome(
                    reach(&at_use, line),
                    || Reach::Applies,
                    |skip| why(skip, &scope, &at_use, holder_tags, None),
                );
                if stat == Stats::SKILL_LEVEL
                    && outcome == LineOutcome::Applied
                    && !matches!(modifier, Modifier::Add(_))
                {
                    outcome = LineOutcome::Skipped("a level takes add lines only".to_owned());
                }
                if outcome == LineOutcome::Applied {
                    sums.fold(modifier);
                }
                lines.push(LineRecord {
                    source: line.source().to_owned(),
                    stat: stat.to_owned(),
                    modifier,
                    scope,
                    read: ReadAt::Use,
                    outcome,
                });
            }
            StatRecord {
                stat: stat.to_owned(),
                base,
                lines,
                sums,
                clamp,
                value,
            }
        };
        let part_lines = |part: &Working| {
            let sets = [
                TagSet::Names(context),
                TagSet::Name(TRACK_HIT),
                TagSet::Name(&part.kind),
                TagSet::Names(&part.history),
            ];
            let bare_sets = [
                TagSet::Names(context),
                TagSet::Name(TRACK_HIT),
                TagSet::Name(&part.kind),
            ];
            let read = Read::new(&sets).holder(holder_tags).at_use();
            let bare = Read::new(&bare_sets).holder(holder_tags).at_use();
            let mut lines = Vec::new();
            for line in lines_on(Stats::DAMAGE) {
                let modifier = modifier_of(line);
                if !matches!(modifier, Modifier::Pct(_) | Modifier::Mul(_)) {
                    continue;
                }
                let scope = scope_of(line);
                let outcome = outcome(
                    reach(&read, line),
                    || reach(&bare, line),
                    |skip| why(skip, &scope, &read, holder_tags, None),
                );
                lines.push(LineRecord {
                    source: line.source().to_owned(),
                    stat: Stats::DAMAGE.to_owned(),
                    modifier,
                    scope,
                    read: ReadAt::Use,
                    outcome,
                });
            }
            for &(source, line) in &local {
                if let Some((pen_kind, points)) = line.as_pen()
                    && covers(&part.kind, pen_kind)
                {
                    let outcome = outcome(
                        read.reach_scope(&line.scope),
                        || bare.reach_scope(&line.scope),
                        |skip| why(skip, &line.scope, &read, holder_tags, None),
                    );
                    lines.push(LineRecord {
                        source: source.to_owned(),
                        stat: "pen".to_owned(),
                        modifier: Modifier::Add(points),
                        scope: line.scope.clone(),
                        read: ReadAt::Use,
                        outcome,
                    });
                }
            }
            lines
        };
        let mut sheet_stats = vec![
            stat_record(Stats::SKILL_LEVEL, entry_level, [LEVELS.0, LEVELS.1], level),
            stat_record(
                Stats::CRIT_CHANCE,
                crit_base,
                [0.0, 1.0],
                crit_chance.value(0.0, 1.0),
            ),
            stat_record(
                Stats::CRIT_MULTIPLIER,
                multiplier_base,
                [1.0, f32::INFINITY],
                crit_multiplier.value(1.0, f32::INFINITY),
            ),
        ];
        if let (Some(base), Some(cost)) = (cost_base, cost) {
            sheet_stats.push(stat_record(Stats::COST, base, [0.0, f32::INFINITY], cost));
        }
        sheet_stats.push(stat_record(
            Stats::COOLDOWN,
            cooldown_base,
            [0.0, f32::INFINITY],
            cooldown,
        ));
        let record = SheetRecord {
            holder,
            ability: entry.to_owned(),
            level,
            stats: sheet_stats,
            parts: merged
                .iter()
                .map(|part| PartRecord {
                    kind: part.kind.clone(),
                    history: part.history.clone(),
                    range: part.range.map(to_f32),
                    origins: part.origins.clone(),
                    lines: part_lines(part),
                })
                .collect(),
        };
        explain.push(ExplainRecord::Sheet(record.clone()));
        Box::new(record)
    });

    let parts = merged
        .into_iter()
        .map(
            |Working {
                 kind,
                 history,
                 range: [min, max],
                 ..
             }| {
                let track = |track: &str| {
                    let sets = [
                        TagSet::Names(context),
                        TagSet::Name(track),
                        TagSet::Name(&kind),
                        TagSet::Names(&history),
                    ];
                    let read = Read::new(&sets).holder(holder_tags).at_use();
                    let mut sums = Sums::new(0.0);
                    for line in lines_on(Stats::DAMAGE) {
                        if reach(&read, line) == Reach::Applies
                            && let modifier @ (Modifier::Pct(_) | Modifier::Mul(_)) =
                                modifier_of(line)
                        {
                            sums.fold(modifier);
                        }
                    }
                    let mut pen = 0.0_f32;
                    for &(_, line) in &local {
                        if let Some((pen_kind, points)) = line.as_pen()
                            && covers(&kind, pen_kind)
                            && read.reach_scope(&line.scope) == Reach::Applies
                        {
                            pen += points;
                        }
                    }
                    (Partial::of(&sums), pen)
                };
                let (hit, pen) = track(TRACK_HIT);
                let (dot, _) = track(TRACK_DOT);
                DeliveryPart {
                    range: [to_f32(min), to_f32(max)],
                    kind,
                    history,
                    hit,
                    dot,
                    pen: pen.clamp(0.0, 1.0),
                }
            },
        )
        .collect();

    Sheet {
        cost,
        cooldown,
        delivery: Delivery {
            holder: Some(holder),
            level,
            holder_tags: holder_tags.cloned().unwrap_or_default(),
            context: context.to_vec(),
            state: Tags::default(),
            parts,
            crit_chance: Partial::of(&crit_chance),
            crit_multiplier: Partial::of(&crit_multiplier),
            deferred,
            activation: None,
            explain: explained,
        },
    }
}

#[cfg(test)]
#[path = "../tests/sheet.rs"]
mod tests;
