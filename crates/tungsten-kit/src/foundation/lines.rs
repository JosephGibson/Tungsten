//! Lines: a [`Modifier`] a source puts on a stat, scoped by tag filters on
//! a read's context, its holder's tags and the other party's at a hit, and
//! the reads that take a context ([`Read`], [`TagSet`], [`Sums`]).

use serde::{Deserialize, Serialize};
use tungsten_core::{MissingAsset, World};

use crate::foundation::stats::Modifier;
use crate::foundation::tags::{STATE_TAGS, TagFilter, Tags, covers};

/// A line: a [`Modifier`] that a source puts on a stat, read only where its
/// filters pass. `with` filters the read's context (a skill's tags, a hit's
/// track, a part's damage type); `while` the holder's own tags at the read;
/// `against` the other party's at a hit (the target's for an attacker's
/// line, the attacker's for a defender's). A line with none is unscoped and
/// reaches every read, [`Stats::get`](crate::Stats::get) included; a scoped
/// one reaches only the reads that take a context,
/// [`Stats::read`](crate::Stats::read) and [`Stats::sums`](crate::Stats::sums).
/// A scoped line may not override: the last-added override would follow
/// apply order in every context, so `mul 0` says "never" instead.
///
/// Data form: `{ "modifier": { "pct": 0.5 }, "with": { "any":
/// ["damage.fire"] }, "while": { .. }, "against": { .. } }`, each filter
/// optional; `stat_modifier` adds its `stat`.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Line, Modifier, Read, Stats, TagFilter, TagSet, Tags};
///
/// let mut stats = Stats::new().with(Stats::DAMAGE, 10.0);
/// let fire = TagFilter::new().any(["damage.fire"]);
/// stats.add_line(Stats::DAMAGE, "passive:searing", Line::new(Modifier::Pct(0.5)).with(fire));
/// assert_eq!(stats.get(Stats::DAMAGE), Some(10.0), "an entity-level read");
///
/// let fire_part = [TagSet::Name("damage.fire")];
/// assert_eq!(stats.read(Stats::DAMAGE, &Read::new(&fire_part)), Some(15.0));
/// let cold_part = [TagSet::Name("damage.cold")];
/// assert_eq!(stats.read(Stats::DAMAGE, &Read::new(&cold_part)), Some(10.0));
///
/// // An `against` line reads the other party, at a hit only.
/// let chilled = TagFilter::new().all(["status.chilled"]);
/// stats.add_line(Stats::DAMAGE, "passive:cold_snap", Line::new(Modifier::Add(2.0)).against(chilled));
/// let brute = Tags::new(["status.chilled"]);
/// assert_eq!(stats.read(Stats::DAMAGE, &Read::new(&cold_part).against(Some(&brute))), Some(12.0));
/// assert_eq!(stats.read(Stats::DAMAGE, &Read::new(&cold_part)), Some(10.0), "no hit");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "LineFields", into = "LineFields")]
pub struct Line {
    modifier: Modifier,
    with: TagFilter,
    holder: TagFilter,
    against: TagFilter,
}

/// A line as data writes it, each filter left out when it passes
/// everything.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LineFields {
    modifier: Modifier,
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

impl TryFrom<LineFields> for Line {
    type Error = String;

    fn try_from(fields: LineFields) -> Result<Self, Self::Error> {
        Line::from_parts(fields.modifier, fields.with, fields.holder, fields.against)
    }
}

impl From<Line> for LineFields {
    fn from(line: Line) -> Self {
        Self {
            modifier: line.modifier,
            with: line.with,
            holder: line.holder,
            against: line.against,
        }
    }
}

impl Line {
    /// An unscoped line of `modifier`: it reaches every read.
    ///
    /// # Panics
    ///
    /// When the modifier's value is not finite.
    #[must_use]
    #[track_caller]
    pub fn new(modifier: Modifier) -> Self {
        assert!(
            modifier.is_finite(),
            "Line::new: the modifier must be finite (got {modifier:?})"
        );
        Self {
            modifier,
            with: TagFilter::new(),
            holder: TagFilter::new(),
            against: TagFilter::new(),
        }
    }

    /// A line of its parts, checked as data is: finite, and no scoped
    /// override.
    pub(crate) fn from_parts(
        modifier: Modifier,
        with: TagFilter,
        holder: TagFilter,
        against: TagFilter,
    ) -> Result<Self, String> {
        if !modifier.is_finite() {
            return Err(format!("modifier: must be finite (got {modifier:?})"));
        }
        let line = Self {
            modifier,
            with,
            holder,
            against,
        };
        line.check_override()?;
        Ok(line)
    }

    /// No scoped override (§17, T9 of the ARPG design).
    fn check_override(&self) -> Result<(), String> {
        if self.is_scoped() && matches!(self.modifier, Modifier::Override(_)) {
            Err("a scoped line may not override (`with`, `while` or `against` beside `override`); `mul 0` says never".to_string())
        } else {
            Ok(())
        }
    }

    #[track_caller]
    fn checked(self, method: &str) -> Self {
        if let Err(error) = self.check_override() {
            panic!("Line::{method}: {error}");
        }
        self
    }

    /// The line, read only where `filter` passes the read's context.
    ///
    /// # Panics
    ///
    /// When the line overrides and `filter` names a tag.
    #[must_use]
    #[track_caller]
    pub fn with(mut self, filter: TagFilter) -> Self {
        self.with = filter;
        self.checked("with")
    }

    /// The line, read only while `filter` passes its holder's tags at the
    /// read (`while` in data).
    ///
    /// # Panics
    ///
    /// When the line overrides and `filter` names a tag.
    #[must_use]
    #[track_caller]
    pub fn while_holder(mut self, filter: TagFilter) -> Self {
        self.holder = filter;
        self.checked("while_holder")
    }

    /// The line, read only at a hit whose other party's tags pass `filter`:
    /// the target's for an attacker's line, the attacker's for a
    /// defender's.
    ///
    /// # Panics
    ///
    /// When the line overrides and `filter` names a tag.
    #[must_use]
    #[track_caller]
    pub fn against(mut self, filter: TagFilter) -> Self {
        self.against = filter;
        self.checked("against")
    }

    /// Its modifier.
    #[must_use]
    pub fn modifier(&self) -> Modifier {
        self.modifier
    }

    /// Its filter over the read's context (`with`).
    #[must_use]
    pub fn context_filter(&self) -> &TagFilter {
        &self.with
    }

    /// Its filter over the holder's tags (`while`).
    #[must_use]
    pub fn holder_filter(&self) -> &TagFilter {
        &self.holder
    }

    /// Its filter over the other party's tags at a hit (`against`).
    #[must_use]
    pub fn against_filter(&self) -> &TagFilter {
        &self.against
    }

    /// Whether a filter names a tag, so only the reads that take a context
    /// take it.
    #[must_use]
    pub fn is_scoped(&self) -> bool {
        !(self.with.is_everything() && self.holder.is_everything() && self.against.is_everything())
    }

    /// Whether a read at use leaves it for the hit (§5.4 of the ARPG
    /// design): it has an `against` filter, or its `with` names a state tag
    /// (`delivery.pierced`, `delivery.forked`, `delivery.chained`,
    /// `delivery.repeated`), which only the hit knows.
    #[must_use]
    pub fn waits_for_hit(&self) -> bool {
        !self.against.is_everything()
            || self
                .with
                .names()
                .any(|name| STATE_TAGS.iter().any(|state| covers(state, name)))
    }

    /// The line with `modifier` in place of its own, as an effect's scale
    /// makes it.
    pub(crate) fn with_modifier(mut self, modifier: Modifier) -> Self {
        self.modifier = modifier;
        self
    }

    /// The load's check: every tag its filters name declared.
    pub(crate) fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset> {
        self.with.check(world, &format!("{field}.with"))?;
        self.holder.check(world, &format!("{field}.while"))?;
        self.against.check(world, &format!("{field}.against"))
    }
}

/// One set of names in a read's context, each carrying its dotted
/// ancestors as [`Tags::has`] reads them: a read matches a line's `with`
/// against its sets together, without merging them.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum TagSet<'a> {
    /// An entity's or a request's tags.
    Tags(&'a Tags),
    /// Names, such as a skill's tags or a part's history.
    Names(&'a [String]),
    /// One name, such as a hit's track (`delivery.hit`) or a part's damage
    /// type.
    Name(&'a str),
}

impl TagSet<'_> {
    /// Whether the set carries `tag`, exactly or as an ancestor.
    fn has(&self, tag: &str) -> bool {
        match self {
            TagSet::Tags(tags) => tags.has(tag),
            TagSet::Names(names) => names.iter().any(|name| covers(name, tag)),
            TagSet::Name(name) => covers(name, tag),
        }
    }
}

/// A read of a stat in a context (§5.1 of the ARPG design): the context's
/// tag sets, the holder's tags for `while` lines, and at a hit the other
/// party's for `against` lines. A read at use ([`at_use`](Self::at_use))
/// leaves the lines that wait for the hit in its [`Sums`] instead.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Line, Modifier, Read, Stats, TagFilter, TagSet, Tags};
///
/// let mut stats = Stats::new().with("crit_chance", 0.1);
/// let focused = TagFilter::new().all(["status.focused"]);
/// stats.add_line("crit_chance", "passive:poise", Line::new(Modifier::Pct(1.0)).while_holder(focused));
/// let skill = vec!["skill.spell".to_string()];
/// let context = [TagSet::Names(&skill)];
/// let calm = Tags::new(["team.player"]);
/// let focus = Tags::new(["team.player", "status.focused"]);
/// assert_eq!(stats.read("crit_chance", &Read::new(&context).holder(Some(&calm))), Some(0.1));
/// assert_eq!(stats.read("crit_chance", &Read::new(&context).holder(Some(&focus))), Some(0.2));
/// assert_eq!(stats.get("crit_chance"), Some(0.1), "`while` reaches context reads only");
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Read<'a> {
    context: &'a [TagSet<'a>],
    holder: Option<&'a Tags>,
    other: Option<&'a Tags>,
    at_hit: bool,
    at_use: bool,
}

/// Why a line's filter kept it from a read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skip {
    /// `with` failed the context.
    Context,
    /// `while` failed the holder's tags.
    Holder,
    /// `against` failed the other party's tags.
    Against,
    /// `against` at a read that is no hit.
    NoHit,
}

/// What a read does with a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reach {
    /// The read takes it.
    Applies,
    /// A filter kept it out.
    Skipped(Skip),
    /// A read at use left it for the hit.
    Deferred,
}

impl<'a> Read<'a> {
    /// A read in `context`, its sets matched together.
    #[must_use]
    pub fn new(context: &'a [TagSet<'a>]) -> Self {
        Self {
            context,
            ..Self::default()
        }
    }

    /// The read, with the holder's tags for `while` lines; none without.
    #[must_use]
    pub fn holder(mut self, tags: Option<&'a Tags>) -> Self {
        self.holder = tags;
        self
    }

    /// The read at a hit, with the other party's tags for `against` lines;
    /// a read that is no hit takes no `against` line.
    #[must_use]
    pub fn against(mut self, tags: Option<&'a Tags>) -> Self {
        self.other = tags;
        self.at_hit = true;
        self
    }

    /// The read at a use: a line that [waits for the
    /// hit](Line::waits_for_hit) goes into the [`Sums`]' deferred lines.
    #[must_use]
    pub fn at_use(mut self) -> Self {
        self.at_use = true;
        self
    }

    /// Whether the context carries `tag`.
    fn has(&self, tag: &str) -> bool {
        self.context.iter().any(|set| set.has(tag))
    }

    /// What the read does with `line`.
    pub(crate) fn reach(&self, line: &Line) -> Reach {
        if self.at_use && line.waits_for_hit() {
            return Reach::Deferred;
        }
        if !line.with.matches_by(|tag| self.has(tag)) {
            return Reach::Skipped(Skip::Context);
        }
        if !line.holder.matches(self.holder) {
            return Reach::Skipped(Skip::Holder);
        }
        if !line.against.is_everything() {
            if !self.at_hit {
                return Reach::Skipped(Skip::NoHit);
            }
            if !line.against.matches(self.other) {
                return Reach::Skipped(Skip::Against);
            }
        }
        Reach::Applies
    }
}

/// What a read's lines sum to, as `((base + Σadd) × (1 + Σpct) × Πmul)`
/// keeps them: the sum with the base, the percentage factor, the product,
/// the last-added override and the lines a read at use left for the hit.
/// [`value`](Self::value) finishes the formula as
/// [`Stats::get`](crate::Stats::get) does, in `f64`, a zero factor making
/// the product 0; [`fold`](Self::fold) adds a term, so a hit finishes what
/// a use began.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Modifier, Sums};
///
/// let mut sums = Sums::new(16.0);
/// sums.fold(Modifier::Add(4.0));
/// sums.fold(Modifier::Pct(0.5));
/// sums.fold(Modifier::Mul(0.75));
/// assert_eq!((sums.add(), sums.pct(), sums.mul()), (20.0, 0.5, 0.75));
/// assert_eq!(sums.value(0.0, f32::INFINITY), 22.5, "20 × 1.5 × 0.75");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Sums {
    add: f64,
    factor: f64,
    product: f64,
    zero: bool,
    over: Option<f64>,
    deferred: Vec<(String, Line)>,
}

impl Sums {
    /// Sums of `base` alone.
    #[must_use]
    pub fn new(base: f32) -> Self {
        Self {
            add: f64::from(base),
            factor: 1.0,
            product: 1.0,
            zero: false,
            over: None,
            deferred: Vec::new(),
        }
    }

    /// Adds `modifier`'s term: to the sum, the percentage factor or the
    /// product; an override replaces the last.
    pub fn fold(&mut self, modifier: Modifier) {
        match modifier {
            Modifier::Add(v) => self.add += f64::from(v),
            Modifier::Pct(v) => self.factor += f64::from(v),
            Modifier::Mul(0.0) => self.zero = true,
            Modifier::Mul(v) => self.product *= f64::from(v),
            Modifier::Override(v) => self.over = Some(f64::from(v)),
        }
    }

    /// `base + Σadd`.
    #[must_use]
    pub fn add(&self) -> f64 {
        self.add
    }

    /// `Σpct`: the percentage factor less its 1.
    #[must_use]
    pub fn pct(&self) -> f64 {
        self.factor - 1.0
    }

    /// `Πmul`, 0 once a factor of 0 joined it.
    #[must_use]
    pub fn mul(&self) -> f64 {
        if self.zero { 0.0 } else { self.product }
    }

    /// The last-added override, which wins over the rest.
    #[must_use]
    pub fn override_value(&self) -> Option<f64> {
        self.over
    }

    /// The lines a read at use left for the hit, each with its source, in
    /// add order.
    pub fn deferred(&self) -> impl Iterator<Item = (&str, &Line)> {
        self.deferred
            .iter()
            .map(|(source, line)| (source.as_str(), line))
    }

    /// Leaves `line` from `source` for the hit.
    pub(crate) fn defer(&mut self, source: &str, line: &Line) {
        self.deferred.push((source.to_owned(), line.clone()));
    }

    /// The value: the override, else the product of the three (0 when
    /// any is 0), clamped to `[min, max]` and to `±f32::MAX`.
    #[must_use]
    pub fn value(&self, min: f32, max: f32) -> f32 {
        let raw = match self.over {
            Some(value) => value,
            None if self.zero || self.add == 0.0 || self.factor == 0.0 => 0.0,
            None => self.add * self.factor * self.product,
        };
        let limit = f64::from(f32::MAX);
        let value = raw
            .clamp(f64::from(min), f64::from(max))
            .clamp(-limit, limit);
        value as f32
    }
}

#[cfg(test)]
#[path = "../tests/lines.rs"]
mod tests;
