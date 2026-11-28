//! Item bases (redo 8.1's second plan, PA-12): the `item_bases` kind, the
//! weapon block a base may carry, and the item lines bases and affixes
//! share, with the numbers an affix's lines take from its rolled values.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use tungsten_core::{AssetRegistry, MissingAsset, PrefabRegistry, World};

use crate::damage::kinds::check_type;
use crate::foundation::lines::{Line, Scope, Sums};
use crate::foundation::stats::{Modifier, Stats, check_stat, valid_stat};
use crate::foundation::tags::{TagFilter, check_declared, covers, valid_tag};
use crate::intent::sheet::SkillLine;
use crate::react::cues::{CueAction, check_list};
use crate::story::condition::Condition;

/// The data kind.
pub(crate) const ITEM_BASES: &str = "item_bases";
/// The most a stack holds.
pub(crate) const MAX_STACK: u32 = 9_999;
/// The most tags a base grants while worn.
const MAX_GRANT: usize = 8;
/// The most implicit lines a base holds, and lines an affix holds.
pub(crate) const MAX_LINES: usize = 8;
/// The most actions a base's `use` list holds.
const MAX_USE: usize = 16;
/// The most characters of a line's own text, a base's name or its text.
const MAX_TEXT: usize = 200;
/// The most rolled values an affix's tier rolls.
pub(crate) const MAX_VALUES: usize = 4;
/// The block's stats a local line may change.
const LOCAL_STATS: [&str; 3] = [Stats::DAMAGE, "use_time", Stats::CRIT_CHANCE];
/// The least use time a fold leaves.
const MIN_USE_TIME: f32 = 0.01;

/// A number in an item line: written, or one of an affix's rolled values
/// times a factor (`{ "value": i, "times": k }`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Amount {
    /// The number itself.
    Fixed(f32),
    /// The `value`-th rolled value times `times`.
    Rolled { value: u8, times: f32 },
}

impl Amount {
    /// The number for these rolled values; `None` past them.
    pub(crate) fn resolve(self, values: &[f32]) -> Option<f32> {
        match self {
            Amount::Fixed(v) => Some(v),
            Amount::Rolled { value, times } => {
                values.get(usize::from(value)).map(|rolled| rolled * times)
            }
        }
    }

    /// The rolled value it reads, when it reads one.
    fn rolled(self) -> Option<u8> {
        match self {
            Amount::Fixed(_) => None,
            Amount::Rolled { value, .. } => Some(value),
        }
    }
}

/// A rolled value as data writes it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RolledValueFields")]
pub(crate) struct RolledValueDef {
    value: u8,
    times: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RolledValueFields {
    value: u32,
    #[serde(default = "one", deserialize_with = "crate::foundation::finite")]
    times: f32,
}

fn one() -> f32 {
    1.0
}

impl TryFrom<RolledValueFields> for RolledValueDef {
    type Error = String;

    fn try_from(fields: RolledValueFields) -> Result<Self, Self::Error> {
        match u8::try_from(fields.value) {
            Ok(value) if usize::from(value) < MAX_VALUES => Ok(Self {
                value,
                times: fields.times,
            }),
            _ => Err(format!(
                "value: {} names no rolled value (0 to {})",
                fields.value,
                MAX_VALUES - 1
            )),
        }
    }
}

impl Serialize for Amount {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match *self {
            Amount::Fixed(v) => v.serialize(serializer),
            Amount::Rolled { value, times } => {
                RolledValueDef { value, times }.serialize(serializer)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Amount {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            Value::Number(number) => {
                let v = number.as_f64().unwrap_or(f64::NAN) as f32;
                if v.is_finite() {
                    Ok(Amount::Fixed(v))
                } else {
                    Err(D::Error::custom(format!("must be finite (got {number})")))
                }
            }
            object @ Value::Object(_) => {
                let def: RolledValueDef =
                    serde_json::from_value(object).map_err(D::Error::custom)?;
                Ok(Amount::Rolled {
                    value: def.value,
                    times: def.times,
                })
            }
            other => Err(D::Error::custom(format!(
                "a number, or a rolled value `{{ \"value\": i, \"times\": k }}` (got {other})"
            ))),
        }
    }
}

/// What a line's modifier does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Add,
    Pct,
    Mul,
    Override,
}

/// A modifier whose number may be a rolled value.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ItemModifierFields", into = "ItemModifierFields")]
pub(crate) struct ItemModifier {
    op: Op,
    amount: Amount,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemModifierFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    add: Option<Amount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pct: Option<Amount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mul: Option<Amount>,
    #[serde(default, rename = "override", skip_serializing_if = "Option::is_none")]
    over: Option<Amount>,
}

impl TryFrom<ItemModifierFields> for ItemModifier {
    type Error = String;

    fn try_from(fields: ItemModifierFields) -> Result<Self, Self::Error> {
        let given = [
            (Op::Add, fields.add),
            (Op::Pct, fields.pct),
            (Op::Mul, fields.mul),
            (Op::Override, fields.over),
        ];
        let mut set = given
            .into_iter()
            .filter_map(|(op, amount)| amount.map(|amount| Self { op, amount }));
        match (set.next(), set.next()) {
            (Some(modifier), None) => Ok(modifier),
            _ => Err("a modifier is one of `add`, `pct`, `mul` or `override`".into()),
        }
    }
}

impl From<ItemModifier> for ItemModifierFields {
    fn from(modifier: ItemModifier) -> Self {
        let mut fields = Self {
            add: None,
            pct: None,
            mul: None,
            over: None,
        };
        let slot = match modifier.op {
            Op::Add => &mut fields.add,
            Op::Pct => &mut fields.pct,
            Op::Mul => &mut fields.mul,
            Op::Override => &mut fields.over,
        };
        *slot = Some(modifier.amount);
        fields
    }
}

impl ItemModifier {
    /// The kit's modifier at these rolled values.
    fn resolve(self, values: &[f32]) -> Option<Modifier> {
        let v = self.amount.resolve(values)?;
        v.is_finite().then_some(match self.op {
            Op::Add => Modifier::Add(v),
            Op::Pct => Modifier::Pct(v),
            Op::Mul => Modifier::Mul(v),
            Op::Override => Modifier::Override(v),
        })
    }
}

/// What an item line changes.
#[derive(Debug, Clone, PartialEq)]
enum LineForm {
    /// A stat, by a modifier.
    Stat {
        stat: String,
        modifier: ItemModifier,
    },
    /// Flat damage of a type, a range.
    Added { kind: String, range: [Amount; 2] },
    /// Penetration for a type's parts.
    Pen { kind: String, points: Amount },
    /// A resistance's factor, its floor or both.
    Resist {
        tag: String,
        modifier: Option<ItemModifier>,
        floor: Option<ItemModifier>,
    },
}

/// One line of an item (§4.6 of the ARPG design): a stat's modifier, an
/// `added` range or a `pen` as a skill line writes them, or a `resist`
/// modifier, with its filters; `local` folds it into the base's weapon
/// block instead of the wearer, and `text`, shown in place of the generic
/// text, writes `{0}` to `{3}` for the affix's rolled values. A number in
/// an affix's line may be `{ "value": i, "times": k }`, the `i`-th rolled
/// value times `k`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ItemLineFields", into = "ItemLineFields")]
pub(crate) struct ItemLine {
    form: LineForm,
    with: TagFilter,
    holder: TagFilter,
    against: TagFilter,
    local: bool,
    text: Option<String>,
}

/// An item line as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemLineFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    modifier: Option<ItemModifier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    added: Option<BTreeMap<String, [Amount; 2]>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pen: Option<BTreeMap<String, Amount>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    resist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    floor: Option<ItemModifier>,
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
    #[serde(default)]
    local: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

/// The one entry of a one-type map.
fn only<T>(map: BTreeMap<String, T>, field: &str) -> Result<(String, T), String> {
    let mut entries = map.into_iter();
    match (entries.next(), entries.next()) {
        (Some(entry), None) => Ok(entry),
        _ => Err(format!("{field}: exactly one damage type")),
    }
}

fn check_tag_name(field: &str, name: &str) -> Result<(), String> {
    if valid_tag(name) {
        Ok(())
    } else {
        Err(format!(
            "{field}: `{name}` is not a tag name (`[a-z][a-z0-9_]*` segments joined by dots)"
        ))
    }
}

impl TryFrom<ItemLineFields> for ItemLine {
    type Error = String;

    fn try_from(fields: ItemLineFields) -> Result<Self, Self::Error> {
        let forms = usize::from(fields.stat.is_some())
            + usize::from(fields.added.is_some())
            + usize::from(fields.pen.is_some())
            + usize::from(fields.resist.is_some());
        if forms != 1 {
            return Err(
                "a line is one of `stat` with `modifier`, `added`, `pen` or `resist`".into(),
            );
        }
        if fields.floor.is_some() && fields.resist.is_none() {
            return Err("`floor` changes a `resist` line's floor".into());
        }
        let form = if let Some(tag) = fields.resist {
            check_tag_name("resist", &tag)?;
            if !fields.holder.is_everything() {
                return Err("a `resist` line takes `with` and `against`, not `while`".into());
            }
            if fields.modifier.is_none() {
                if fields.floor.is_none() {
                    return Err("a `resist` line names its `modifier`, its `floor` or both".into());
                }
                if !(fields.with.is_everything() && fields.against.is_everything()) {
                    return Err("`with` and `against` scope a `modifier`, which is missing".into());
                }
            }
            LineForm::Resist {
                tag,
                modifier: fields.modifier,
                floor: fields.floor,
            }
        } else if let Some(stat) = fields.stat {
            if !valid_stat(&stat) {
                return Err(format!(
                    "stat: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
                ));
            }
            let Some(modifier) = fields.modifier else {
                return Err("a stat line names its `stat` and its `modifier`".into());
            };
            LineForm::Stat { stat, modifier }
        } else if fields.modifier.is_some() {
            return Err("`modifier` goes with `stat` or `resist`".into());
        } else if let Some(added) = fields.added {
            let (kind, range) = only(added, "added")?;
            check_tag_name("added", &kind)?;
            if let [Amount::Fixed(min), Amount::Fixed(max)] = range
                && !(0.0 <= min && min <= max)
            {
                return Err(format!(
                    "added: {kind}: [{min}, {max}] is not a range with 0 <= min <= max"
                ));
            }
            LineForm::Added { kind, range }
        } else if let Some(pen) = fields.pen {
            let (kind, points) = only(pen, "pen")?;
            check_tag_name("pen", &kind)?;
            LineForm::Pen { kind, points }
        } else {
            unreachable!("one form was counted");
        };
        if let Some(text) = &fields.text
            && text.chars().count() > MAX_TEXT
        {
            return Err(format!("text: past {MAX_TEXT} characters"));
        }
        let line = Self {
            form,
            with: fields.with,
            holder: fields.holder,
            against: fields.against,
            local: fields.local,
            text: fields.text,
        };
        line.check_scope()?;
        Ok(line)
    }
}

impl From<ItemLine> for ItemLineFields {
    fn from(line: ItemLine) -> Self {
        let mut fields = Self {
            stat: None,
            modifier: None,
            added: None,
            pen: None,
            resist: None,
            floor: None,
            with: line.with,
            holder: line.holder,
            against: line.against,
            local: line.local,
            text: line.text,
        };
        match line.form {
            LineForm::Stat { stat, modifier } => {
                fields.stat = Some(stat);
                fields.modifier = Some(modifier);
            }
            LineForm::Added { kind, range } => {
                fields.added = Some(BTreeMap::from([(kind, range)]));
            }
            LineForm::Pen { kind, points } => fields.pen = Some(BTreeMap::from([(kind, points)])),
            LineForm::Resist {
                tag,
                modifier,
                floor,
            } => {
                fields.resist = Some(tag);
                fields.modifier = modifier;
                fields.floor = floor;
            }
        }
        fields
    }
}

impl ItemLine {
    /// Its filters, as a scope.
    fn scope(&self) -> Scope {
        Scope::new()
            .with(self.with.clone())
            .while_holder(self.holder.clone())
            .against(self.against.clone())
    }

    /// Whether it folds into the base's block.
    pub(crate) fn is_local(&self) -> bool {
        self.local
    }

    /// The scope rules: no scoped override, an `added` line that does not
    /// wait for the hit, and a local line on the block's stats or an
    /// `added`, taking `with` alone and only for `damage`.
    fn check_scope(&self) -> Result<(), String> {
        let scoped = !self.scope().is_everything();
        match &self.form {
            LineForm::Stat { modifier, .. } if scoped && modifier.op == Op::Override => {
                return Err("a scoped line may not override (`with`, `while` or `against` beside `override`); `mul 0` says never".into());
            }
            LineForm::Resist {
                modifier: Some(modifier),
                ..
            } if scoped && modifier.op == Op::Override => {
                return Err("a scoped line may not override (`with` or `against` beside `override`); `mul 0` says never".into());
            }
            LineForm::Added { .. } if self.scope().waits_for_hit() => {
                return Err("an `added` line, spent at a use, may not wait for the hit (`against`, or a state tag in `with`)".into());
            }
            _ => {}
        }
        if !self.local {
            return Ok(());
        }
        if !(self.holder.is_everything() && self.against.is_everything()) {
            return Err("a local line takes `with` alone: the block it folds into has no holder and no hit (`while`, `against`)".into());
        }
        match &self.form {
            LineForm::Stat { stat, modifier } => {
                if !LOCAL_STATS.contains(&stat.as_str()) {
                    return Err(format!(
                        "stat: a local line changes the block's `damage`, `use_time` or `crit_chance`, not `{stat}`"
                    ));
                }
                if modifier.op == Op::Override {
                    return Err("a local line may not override".into());
                }
                if stat != Stats::DAMAGE && !self.with.is_everything() {
                    return Err(format!(
                        "with: a local `{stat}` line folds into the block whole; `with` names damage parts"
                    ));
                }
                Ok(())
            }
            LineForm::Added { .. } if self.with.is_everything() => Ok(()),
            LineForm::Added { .. } => {
                Err("with: a local `added` line names its part by its type".into())
            }
            LineForm::Pen { .. } | LineForm::Resist { .. } => Err(
                "a local line is a `stat` line on the block or an `added`, not a `pen` or `resist`"
                    .into(),
            ),
        }
    }

    /// The rolled values its numbers read.
    pub(crate) fn rolled_values(&self) -> impl Iterator<Item = u8> {
        let amounts: Vec<Amount> = match &self.form {
            LineForm::Stat { modifier, .. } => vec![modifier.amount],
            LineForm::Added { range, .. } => range.to_vec(),
            LineForm::Pen { points, .. } => vec![*points],
            LineForm::Resist {
                modifier, floor, ..
            } => modifier.iter().chain(floor).map(|m| m.amount).collect(),
        };
        amounts.into_iter().filter_map(Amount::rolled)
    }

    /// The load's check: its filters' tags declared, its stat declared, its
    /// damage types and its resistance's tag known.
    pub(crate) fn check_in(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        self.scope().check(world, field)?;
        match &self.form {
            LineForm::Stat { stat, .. } => {
                if self.local {
                    Ok(())
                } else {
                    check_stat(world, &format!("{field}.stat"), stat)
                }
            }
            LineForm::Added { kind, .. } => {
                check_type(world, &format!("{field}.added"), kind, true)
            }
            LineForm::Pen { kind, .. } => check_type(world, &format!("{field}.pen"), kind, true),
            LineForm::Resist { tag, .. } => check_declared(world, &format!("{field}.resist"), tag),
        }
    }

    /// The line at these rolled values, as the wearer takes it, with its
    /// text; `None` when a number cannot be read or does not stand.
    pub(crate) fn build(&self, values: &[f32]) -> Option<BuiltLine> {
        let kind = match &self.form {
            LineForm::Stat { stat, modifier } => {
                let modifier = modifier.resolve(values)?;
                BuiltKind::Stat {
                    stat: stat.clone(),
                    line: Line::scoped(modifier, self.scope()).ok()?,
                }
            }
            LineForm::Added { kind, range } => {
                let (min, max) = (range[0].resolve(values)?, range[1].resolve(values)?);
                if !(min.is_finite() && max.is_finite() && 0.0 <= min && min <= max) {
                    return None;
                }
                BuiltKind::Skill(self.skill_line("added", kind, serde_json::json!([min, max]))?)
            }
            LineForm::Pen { kind, points } => {
                let points = points.resolve(values)?;
                if !points.is_finite() {
                    return None;
                }
                BuiltKind::Skill(self.skill_line("pen", kind, serde_json::json!(points))?)
            }
            LineForm::Resist {
                tag,
                modifier,
                floor,
            } => {
                let line = match modifier {
                    Some(modifier) => Some(
                        Line::from_parts(
                            modifier.resolve(values)?,
                            self.with.clone(),
                            TagFilter::new(),
                            self.against.clone(),
                        )
                        .ok()?,
                    ),
                    None => None,
                };
                let floor = match floor {
                    Some(floor) => Some(floor.resolve(values)?),
                    None => None,
                };
                BuiltKind::Resist {
                    tag: tag.clone(),
                    line,
                    floor,
                }
            }
        };
        let text = match &self.text {
            Some(own) => own_text(own, values),
            None => generic_text(&kind),
        };
        Some(BuiltLine { kind, text })
    }

    /// A skill line, `{ form: { kind: value } }` in this line's scope,
    /// through its data form, so a value that does not stand is `None`
    /// rather than a panic.
    fn skill_line(&self, form: &str, kind: &str, value: Value) -> Option<SkillLine> {
        let mut object = serde_json::Map::new();
        let mut part = serde_json::Map::new();
        part.insert(kind.to_owned(), value);
        object.insert(form.to_owned(), Value::Object(part));
        for (name, filter) in [
            ("with", &self.with),
            ("while", &self.holder),
            ("against", &self.against),
        ] {
            if !filter.is_everything() {
                object.insert(name.into(), serde_json::to_value(filter).ok()?);
            }
        }
        serde_json::from_value(Value::Object(object)).ok()
    }

    /// The local fold term at these rolled values: `None` when it cannot
    /// be read.
    pub(crate) fn local_term(&self, values: &[f32]) -> Option<LocalTerm> {
        match &self.form {
            LineForm::Stat { stat, modifier } => Some(LocalTerm::Stat {
                stat: stat.clone(),
                modifier: modifier.resolve(values)?,
                with: self.with.clone(),
            }),
            LineForm::Added { kind, range } => {
                let (min, max) = (range[0].resolve(values)?, range[1].resolve(values)?);
                (min.is_finite() && max.is_finite() && 0.0 <= min && min <= max).then(|| {
                    LocalTerm::Added {
                        kind: kind.clone(),
                        range: [min, max],
                    }
                })
            }
            LineForm::Pen { .. } | LineForm::Resist { .. } => None,
        }
    }
}

/// A local line read at its values, for the block's fold.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LocalTerm {
    /// A modifier on `damage` (the parts `with` passes), `use_time` or
    /// `crit_chance`.
    Stat {
        stat: String,
        modifier: Modifier,
        with: TagFilter,
    },
    /// Flat damage added to a part of its type, or a new part.
    Added { kind: String, range: [f32; 2] },
}

/// What a built line gives the wearer.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum BuiltKind {
    /// A stat line, on the wearer's stats through the equipment status.
    Stat { stat: String, line: Line },
    /// An `added` or `pen` line, on the wearer's skills.
    Skill(SkillLine),
    /// A resistance modifier, through the equipment status.
    Resist {
        tag: String,
        line: Option<Line>,
        floor: Option<Modifier>,
    },
}

/// One of a built item's global lines: what it gives the wearer, at its
/// rolled values, and its text.
#[derive(Debug, Clone, PartialEq)]
pub struct BuiltLine {
    pub(crate) kind: BuiltKind,
    text: String,
}

impl BuiltLine {
    /// Its text: the line's own, its `{0}` to `{3}` the rolled values, else
    /// a generic one (`+22% damage with damage.fire`, `adds 4–6
    /// damage.fire`, `damage.fire resist +19%`).
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Its stat and line, for a stat line.
    #[must_use]
    pub fn as_stat(&self) -> Option<(&str, &Line)> {
        match &self.kind {
            BuiltKind::Stat { stat, line } => Some((stat, line)),
            _ => None,
        }
    }

    /// Its skill line, for an `added` or `pen` line.
    #[must_use]
    pub fn as_skill_line(&self) -> Option<&SkillLine> {
        match &self.kind {
            BuiltKind::Skill(line) => Some(line),
            _ => None,
        }
    }

    /// Its tag, its factor's line and its floor's modifier, for a
    /// resistance line.
    #[must_use]
    pub fn as_resist(&self) -> Option<(&str, Option<&Line>, Option<Modifier>)> {
        match &self.kind {
            BuiltKind::Resist { tag, line, floor } => Some((tag, line.as_ref(), *floor)),
            _ => None,
        }
    }
}

/// A number as item text shows it: whole when it is, else to two places.
pub(crate) fn shown(v: f32) -> String {
    let rounded = (f64::from(v) * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 1e-9 {
        format!("{}", rounded.round() as i64)
    } else {
        let text = format!("{rounded:.2}");
        text.trim_end_matches('0').to_owned()
    }
}

/// A signed number as item text shows it.
fn signed(v: f32) -> String {
    if v < 0.0 {
        format!("-{}", shown(-v))
    } else {
        format!("+{}", shown(v))
    }
}

/// A line's own text, `{0}` to `{3}` the rolled values.
fn own_text(text: &str, values: &[f32]) -> String {
    let mut out = text.to_owned();
    for (i, value) in values.iter().enumerate().take(MAX_VALUES) {
        out = out.replace(&format!("{{{i}}}"), &shown(*value));
    }
    out
}

/// A filter as item text shows it: its `all` names joined by "and", its
/// `any` by "or", its `none` after "not".
fn filter_text(filter: &TagFilter) -> String {
    let value = serde_json::to_value(filter).unwrap_or(Value::Null);
    let names = |key: &str| -> Vec<String> {
        value
            .get(key)
            .and_then(Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut parts = Vec::new();
    let all = names("all");
    if !all.is_empty() {
        parts.push(all.join(" and "));
    }
    let any = names("any");
    if !any.is_empty() {
        parts.push(any.join(" or "));
    }
    let none = names("none");
    if !none.is_empty() {
        parts.push(format!("not {}", none.join(" or ")));
    }
    parts.join(", ")
}

/// The filters' words after a line's text.
fn scope_text(scope: &Scope) -> String {
    let mut out = String::new();
    for (word, filter) in [
        ("with", scope.context_filter()),
        ("while", scope.holder_filter()),
        ("against", scope.against_filter()),
    ] {
        if !filter.is_everything() {
            let _ = write!(out, " {word} {}", filter_text(filter));
        }
    }
    out
}

/// A stat modifier's words: `+22% damage`.
fn modifier_text(modifier: Modifier, name: &str) -> String {
    match modifier {
        Modifier::Add(v) => format!("{} {name}", signed(v)),
        Modifier::Pct(v) => format!("{}% {name}", signed(v * 100.0)),
        Modifier::Mul(v) => format!("\u{d7}{} {name}", shown(v)),
        Modifier::Override(v) => format!("{name} = {}", shown(v)),
    }
}

/// A resistance modifier's words: `damage.fire resist +19%` for an `add`,
/// whose lower factor is more resistance.
fn resist_text(modifier: Modifier, tag: &str) -> String {
    match modifier {
        Modifier::Add(v) => format!("{tag} resist {}%", signed(-v * 100.0)),
        Modifier::Pct(v) => format!("{tag} resist factor {}%", signed(v * 100.0)),
        Modifier::Mul(v) => format!("{tag} resist factor \u{d7}{}", shown(v)),
        Modifier::Override(v) => format!("{tag} resist factor = {}", shown(v)),
    }
}

/// A built line's generic text.
fn generic_text(kind: &BuiltKind) -> String {
    match kind {
        BuiltKind::Stat { stat, line } => {
            format!(
                "{}{}",
                modifier_text(line.modifier(), stat),
                scope_text(line.scope())
            )
        }
        BuiltKind::Skill(line) => {
            let body = if let Some((kind, [min, max])) = line.as_added() {
                format!("adds {}\u{2013}{} {kind}", shown(min), shown(max))
            } else if let Some((kind, points)) = line.as_pen() {
                format!("penetrates {}% {kind}", shown(points * 100.0))
            } else {
                String::new()
            };
            format!("{body}{}", scope_text(line.scope()))
        }
        BuiltKind::Resist { tag, line, floor } => {
            let mut parts = Vec::new();
            if let Some(line) = line {
                parts.push(format!(
                    "{}{}",
                    resist_text(line.modifier(), tag),
                    scope_text(line.scope())
                ));
            }
            if let Some(floor) = floor {
                parts.push(match *floor {
                    Modifier::Add(v) => format!("{tag} floor {}%", signed(v * 100.0)),
                    other => format!("{tag} floor {}", modifier_text(other, "").trim_end()),
                });
            }
            parts.join(", ")
        }
    }
}

/// A weapon block: its damage parts, each a declared damage type's range,
/// its use time and its crit chance. A base's block is what its local
/// lines fold into when an item is built; no attack reads it yet (8.7).
///
/// Data form: `{ "damage": { "damage.fire": [4, 8] }, "use_time": 0.8,
/// "crit_chance": 0.07 }`.
///
/// # Examples
///
/// ```
/// use tungsten_kit::WeaponBlock;
///
/// let block: WeaponBlock = serde_json::from_str(
///     r#"{ "damage": { "damage.fire": [4, 8] }, "use_time": 0.8 }"#,
/// )
/// .unwrap();
/// assert_eq!(block.damage().collect::<Vec<_>>(), [("damage.fire", [4.0, 8.0])]);
/// assert_eq!((block.use_time(), block.crit_chance()), (0.8, 0.0));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "WeaponBlockFields", into = "WeaponBlockFields")]
pub struct WeaponBlock {
    damage: Vec<(String, [f32; 2])>,
    use_time: f32,
    crit_chance: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WeaponBlockFields {
    damage: BTreeMap<String, [f32; 2]>,
    #[serde(deserialize_with = "crate::foundation::positive")]
    use_time: f32,
    #[serde(default, deserialize_with = "crate::foundation::fraction")]
    crit_chance: f32,
}

impl TryFrom<WeaponBlockFields> for WeaponBlock {
    type Error = String;

    fn try_from(fields: WeaponBlockFields) -> Result<Self, Self::Error> {
        if fields.damage.is_empty() || fields.damage.len() > MAX_LINES {
            return Err(format!(
                "damage: 1 to {MAX_LINES} parts, each a damage type's [min, max]"
            ));
        }
        for (kind, [min, max]) in &fields.damage {
            check_tag_name(&format!("damage.{kind}"), kind)?;
            if !(min.is_finite() && max.is_finite() && 0.0 <= *min && min <= max) {
                return Err(format!(
                    "damage.{kind}: [{min}, {max}] is not a range with 0 <= min <= max"
                ));
            }
        }
        Ok(Self {
            damage: fields.damage.into_iter().collect(),
            use_time: fields.use_time,
            crit_chance: fields.crit_chance,
        })
    }
}

impl From<WeaponBlock> for WeaponBlockFields {
    fn from(block: WeaponBlock) -> Self {
        Self {
            damage: block.damage.into_iter().collect(),
            use_time: block.use_time,
            crit_chance: block.crit_chance,
        }
    }
}

impl WeaponBlock {
    /// Its damage parts, each its type and range, by type.
    pub fn damage(&self) -> impl Iterator<Item = (&str, [f32; 2])> {
        self.damage
            .iter()
            .map(|(kind, range)| (kind.as_str(), *range))
    }

    /// Seconds a use takes.
    #[must_use]
    pub fn use_time(&self) -> f32 {
        self.use_time
    }

    /// Its crit chance, 0 to 1.
    #[must_use]
    pub fn crit_chance(&self) -> f32 {
        self.crit_chance
    }

    /// Its lines as item text shows them: each part's range, then its use
    /// time and crit chance.
    #[must_use]
    pub fn text(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .damage
            .iter()
            .map(|(kind, [min, max])| format!("{}\u{2013}{} {kind}", shown(*min), shown(*max)))
            .collect();
        lines.push(format!("use time {} s", shown(self.use_time)));
        lines.push(format!("crit chance {}%", shown(self.crit_chance * 100.0)));
        lines
    }

    /// The load's check: each part a known damage type.
    fn check_in(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        for (kind, _) in &self.damage {
            check_type(world, &format!("{field}.damage.{kind}"), kind, true)?;
        }
        Ok(())
    }

    /// The block with `terms` folded in, in the kit's modifier order over
    /// each base value: the adds summed, then `× (1 + Σ pct)`, then the
    /// product of the `mul`s. An `added` term adds to its type's part, or
    /// adds the part; a `damage` term reaches each part its `with` passes.
    pub(crate) fn folded(&self, terms: &[LocalTerm]) -> WeaponBlock {
        let mut parts = self.damage.clone();
        for term in terms {
            if let LocalTerm::Added { kind, .. } = term
                && !parts.iter().any(|(part, _)| part == kind)
            {
                parts.push((kind.clone(), [0.0, 0.0]));
            }
        }
        let damage = parts
            .into_iter()
            .map(|(kind, [lo, hi])| {
                let (mut min, mut max) = (Sums::new(lo), Sums::new(hi));
                for term in terms {
                    match term {
                        LocalTerm::Added { kind: added, range } if *added == kind => {
                            min.fold(Modifier::Add(range[0]));
                            max.fold(Modifier::Add(range[1]));
                        }
                        LocalTerm::Stat {
                            stat,
                            modifier,
                            with,
                        } if stat == Stats::DAMAGE && with.matches_by(|tag| covers(tag, &kind)) => {
                            min.fold(*modifier);
                            max.fold(*modifier);
                        }
                        _ => {}
                    }
                }
                let lo = min.value(0.0, f32::INFINITY);
                (kind, [lo, max.value(0.0, f32::INFINITY).max(lo)])
            })
            .collect();
        let whole = |name: &str, base: f32| {
            let mut sums = Sums::new(base);
            for term in terms {
                if let LocalTerm::Stat { stat, modifier, .. } = term
                    && stat == name
                {
                    sums.fold(*modifier);
                }
            }
            sums
        };
        WeaponBlock {
            damage,
            use_time: whole("use_time", self.use_time).value(MIN_USE_TIME, f32::INFINITY),
            crit_chance: whole(Stats::CRIT_CHANCE, self.crit_chance).value(0.0, 1.0),
        }
    }
}

/// An item base: an `item_bases` entry, one a file. Its name, icon, text
/// and tags; the slot kind it is worn in (none: never worn); how many
/// stack in a cell; its price in whole coins; what its wearer must meet
/// (`requires`, a condition on the wearer); the tags it grants and the
/// implicit lines it gives while worn; its weapon block; what using it
/// does (`use`, a story list run at the use) and whether a use spends one;
/// and the prefab a drop of it spawns (`ground`).
///
/// ```json
/// { "name": "Ashwood Wand", "icon": "wand", "tags": ["item.wand"], "slot": "main_hand",
///   "implicit": [{ "stat": "damage", "modifier": { "pct": 0.08 }, "with": { "all": ["skill.spell"] } }],
///   "weapon": { "damage": { "damage.fire": [4, 8] }, "use_time": 0.8 } }
/// ```
///
/// The parse refuses a stack past 1 beside a slot or a block, a local line
/// on a base with no block, a rolled value in an implicit line and a
/// `requires` that reads `other`; the load checks the icon, tags, stats,
/// damage types, the condition, the `use` list (no `target: other`) and
/// the `ground` prefab, which must carry `ground_item` and `pickup`.
///
/// # Examples
///
/// ```
/// use tungsten_kit::ItemBase;
///
/// let flask: ItemBase = serde_json::from_str(
///     r#"{ "name": "Oil flask", "icon": "flask", "stack": 5, "price": 3 }"#,
/// )
/// .unwrap();
/// assert_eq!((flask.name(), flask.stack(), flask.price()), ("Oil flask", 5, 3));
/// assert!(flask.slot().is_none() && flask.weapon().is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ItemBaseFields", into = "ItemBaseFields")]
pub struct ItemBase {
    name: String,
    icon: String,
    text: Option<String>,
    tags: Vec<String>,
    slot: Option<String>,
    stack: u32,
    price: u32,
    requires: Option<Condition>,
    grant: Vec<String>,
    implicit: Vec<ItemLine>,
    weapon: Option<WeaponBlock>,
    use_list: Vec<CueAction>,
    consume: Option<bool>,
    ground: Option<String>,
}

/// An item base as data writes it.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ItemBaseFields {
    name: String,
    icon: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    slot: Option<String>,
    #[serde(default = "one_unit")]
    stack: u32,
    #[serde(default)]
    price: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    requires: Option<Condition>,
    #[serde(default)]
    grant: Vec<String>,
    #[serde(default)]
    implicit: Vec<ItemLine>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    weapon: Option<WeaponBlock>,
    #[serde(default, rename = "use")]
    use_list: Vec<CueAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consume: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ground: Option<String>,
}

fn one_unit() -> u32 {
    1
}

impl TryFrom<ItemBaseFields> for ItemBase {
    type Error = String;

    fn try_from(fields: ItemBaseFields) -> Result<Self, Self::Error> {
        for (field, text) in [("name", Some(&fields.name)), ("text", fields.text.as_ref())] {
            if let Some(text) = text
                && (text.is_empty() || text.chars().count() > MAX_TEXT)
            {
                return Err(format!("{field}: 1 to {MAX_TEXT} characters"));
            }
        }
        if fields.icon.is_empty() {
            return Err("icon: a manifest sprite ID".into());
        }
        for (i, tag) in fields.tags.iter().enumerate() {
            check_tag_name(&format!("tags.{i}"), tag)?;
        }
        if let Some(slot) = &fields.slot
            && !valid_stat(slot)
        {
            return Err(format!(
                "slot: `{slot}` is not a slot kind (`[a-z][a-z0-9_]*`)"
            ));
        }
        if !(1..=MAX_STACK).contains(&fields.stack) {
            return Err(format!("stack: 1 to {MAX_STACK} (got {})", fields.stack));
        }
        if fields.stack > 1 && (fields.slot.is_some() || fields.weapon.is_some()) {
            return Err("stack: an item worn or with a weapon block stacks to 1".into());
        }
        if fields.requires.as_ref().is_some_and(Condition::reads_other) {
            return Err("requires: the wearer is its subject, with no other entity".into());
        }
        if fields.grant.len() > MAX_GRANT {
            return Err(format!("grant: at most {MAX_GRANT} tags"));
        }
        for (i, tag) in fields.grant.iter().enumerate() {
            check_tag_name(&format!("grant.{i}"), tag)?;
        }
        if fields.implicit.len() > MAX_LINES {
            return Err(format!("implicit: at most {MAX_LINES} lines"));
        }
        for (i, line) in fields.implicit.iter().enumerate() {
            if line.rolled_values().next().is_some() {
                return Err(format!(
                    "implicit.{i}: a base rolls no values; a rolled value is an affix's"
                ));
            }
            if line.local && fields.weapon.is_none() {
                return Err(format!(
                    "implicit.{i}: a local line folds into a `weapon` block, which the base lacks"
                ));
            }
        }
        if fields.use_list.len() > MAX_USE {
            return Err(format!("use: at most {MAX_USE} actions"));
        }
        Ok(Self {
            name: fields.name,
            icon: fields.icon,
            text: fields.text,
            tags: fields.tags,
            slot: fields.slot,
            stack: fields.stack,
            price: fields.price,
            requires: fields.requires,
            grant: fields.grant,
            implicit: fields.implicit,
            weapon: fields.weapon,
            use_list: fields.use_list,
            consume: fields.consume,
            ground: fields.ground,
        })
    }
}

impl From<ItemBase> for ItemBaseFields {
    fn from(base: ItemBase) -> Self {
        Self {
            name: base.name,
            icon: base.icon,
            text: base.text,
            tags: base.tags,
            slot: base.slot,
            stack: base.stack,
            price: base.price,
            requires: base.requires,
            grant: base.grant,
            implicit: base.implicit,
            weapon: base.weapon,
            use_list: base.use_list,
            consume: base.consume,
            ground: base.ground,
        }
    }
}

impl ItemBase {
    /// Its name, which an item with no words of its own shows.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Its icon, a manifest sprite ID.
    #[must_use]
    pub fn icon(&self) -> &str {
        &self.icon
    }

    /// Its text, when it has some.
    #[must_use]
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// Its tags.
    pub fn tags(&self) -> impl Iterator<Item = &str> {
        self.tags.iter().map(String::as_str)
    }

    /// The slot kind it is worn in; `None` when it is never worn.
    #[must_use]
    pub fn slot(&self) -> Option<&str> {
        self.slot.as_deref()
    }

    /// How many stack in a cell.
    #[must_use]
    pub fn stack(&self) -> u32 {
        self.stack
    }

    /// Its price in whole coins.
    #[must_use]
    pub fn price(&self) -> u32 {
        self.price
    }

    /// What its wearer must meet, a condition on the wearer.
    #[must_use]
    pub fn requires(&self) -> Option<&Condition> {
        self.requires.as_ref()
    }

    /// The tags it grants its wearer.
    pub fn grant(&self) -> impl Iterator<Item = &str> {
        self.grant.iter().map(String::as_str)
    }

    /// Its weapon block, before any item's local lines.
    #[must_use]
    pub fn weapon(&self) -> Option<&WeaponBlock> {
        self.weapon.as_ref()
    }

    /// Whether it can be used: it has a `use` list.
    #[must_use]
    pub fn usable(&self) -> bool {
        !self.use_list.is_empty()
    }

    /// Whether a use spends one: its `consume`, else whether it can be
    /// used.
    #[must_use]
    pub fn consumes(&self) -> bool {
        self.consume.unwrap_or(self.usable())
    }

    /// The prefab a drop of it spawns, when it has one.
    #[must_use]
    pub fn ground(&self) -> Option<&str> {
        self.ground.as_deref()
    }

    /// Its implicit lines.
    pub(crate) fn implicit(&self) -> &[ItemLine] {
        &self.implicit
    }
}

/// The `item_bases` kind's check: the icon a manifest sprite, the tags and
/// grants declared, the condition's names, each implicit line's names, the
/// block's damage types, the `use` list as a story list with no other, and
/// the `ground` prefab carrying `ground_item` and `pickup`.
pub(crate) fn check_base(base: &ItemBase, world: &World) -> Result<(), MissingAsset> {
    let interned = world
        .get_resource::<AssetRegistry>()
        .is_none_or(|assets| assets.sprite_id(&base.icon).is_some());
    if !interned {
        return Err(MissingAsset::new("icon", "sprite", base.icon.as_str()));
    }
    for (i, tag) in base.tags.iter().enumerate() {
        check_declared(world, &format!("tags.{i}"), tag)?;
    }
    for (i, tag) in base.grant.iter().enumerate() {
        check_declared(world, &format!("grant.{i}"), tag)?;
    }
    if let Some(requires) = &base.requires {
        requires.check(world, "requires")?;
    }
    for (i, line) in base.implicit.iter().enumerate() {
        line.check_in(world, &format!("implicit.{i}"))?;
    }
    if let Some(block) = &base.weapon {
        block.check_in(world, "weapon")?;
    }
    check_list(world, "use", &base.use_list, false)?;
    if let Some(ground) = &base.ground {
        world.check_prefab_id("ground", ground)?;
        let carries = world
            .get_resource::<PrefabRegistry>()
            .and_then(|prefabs| prefabs.get(ground))
            .is_some_and(|prefab| {
                ["ground_item", "pickup"]
                    .iter()
                    .all(|name| prefab.components.contains_key(*name))
            });
        if !carries {
            return Err(MissingAsset::new(
                "ground",
                "prefab carrying `ground_item` and `pickup`",
                ground.as_str(),
            ));
        }
    }
    Ok(())
}
