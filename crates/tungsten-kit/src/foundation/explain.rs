//! Why-records (§10 of the ARPG design): [`KitExplain`], a ring of the
//! latest sheets a use resolved and hits the pipeline resolved, each with
//! every line it read and what the read did with it, kept only while a game
//! or a test inserts the resource.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt;

use tungsten_core::Entity;

use crate::foundation::lines::{Reach, Read, Scope, Skip, Sums};
use crate::foundation::stats::Modifier;
use crate::foundation::tags::{STATE_TAGS, TagFilter, Tags};

/// How many records [`KitExplain::new`] keeps.
pub const EXPLAIN_RECORDS: usize = 64;

/// Why-records of sheets and hits (§10 of the ARPG design): a ring of the
/// latest [`ExplainRecord`]s, the oldest dropped past its capacity. The kit
/// records only while the resource is present, so a game inserts it to
/// look and removes it after; no trace kind is added. Each sheet a use
/// resolves is recorded with every line it read, a use its cost or target
/// then fails included, and rides its [`Delivery`](crate::Delivery) to its
/// hits' records. The ring sits behind
/// a `RefCell` so a sheet's resolve, which reads `&World`, can record
/// (`D-086`'s pattern). Each record's `Display` writes the §9.1 listing, a
/// line each.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Time, World};
/// use tungsten_kit::{DamageRequest, Health, HealthPlugin, KitExplain, Resist, Tags};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// HealthPlugin.build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
/// world.insert_resource(KitExplain::new());
///
/// let golem = world.spawn_with((Health::new(10.0), Resist::new().with_factor("damage.fire", 0.5)));
/// DamageRequest::new(golem, 4.0).with_tags(Tags::new(["damage.fire"])).apply(&mut world);
/// let hit = world.resource::<KitExplain>().last_hit().unwrap();
/// assert_eq!((hit.amount, hit.parts[0].after), (2.0, 2.0));
/// assert_eq!(hit.parts[0].resist.as_ref().map(|resist| resist.factor), Some(0.5));
/// assert!(hit.to_string().contains("resist  damage.fire 0.50 (no floor; no penetration) → 2.00"));
/// ```
#[derive(Debug)]
pub struct KitExplain {
    ring: RefCell<VecDeque<ExplainRecord>>,
    capacity: usize,
}

impl Default for KitExplain {
    fn default() -> Self {
        Self::new()
    }
}

impl KitExplain {
    /// A ring of [`EXPLAIN_RECORDS`] records.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(EXPLAIN_RECORDS)
    }

    /// A ring of `capacity` records, at least one.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ring: RefCell::new(VecDeque::new()),
            capacity: capacity.max(1),
        }
    }

    /// The records it holds, oldest first.
    #[must_use]
    pub fn records(&self) -> Vec<ExplainRecord> {
        self.ring.borrow().iter().cloned().collect()
    }

    /// The latest hit's record.
    #[must_use]
    pub fn last_hit(&self) -> Option<HitRecord> {
        self.ring
            .borrow()
            .iter()
            .rev()
            .find_map(|record| match record {
                ExplainRecord::Hit(hit) => Some(hit.clone()),
                ExplainRecord::Sheet(_) => None,
            })
    }

    /// The latest sheet's record.
    #[must_use]
    pub fn last_sheet(&self) -> Option<SheetRecord> {
        self.ring
            .borrow()
            .iter()
            .rev()
            .find_map(|record| match record {
                ExplainRecord::Sheet(sheet) => Some(sheet.clone()),
                ExplainRecord::Hit(_) => None,
            })
    }

    /// How many records it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.ring.borrow().len()
    }

    /// Whether it holds no record.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ring.borrow().is_empty()
    }

    /// Drops every record.
    pub fn clear(&mut self) {
        self.ring.get_mut().clear();
    }

    /// Keeps `record`, dropping the oldest past the capacity.
    pub(crate) fn push(&self, record: ExplainRecord) {
        let mut ring = self.ring.borrow_mut();
        ring.push_back(record);
        while ring.len() > self.capacity {
            ring.pop_front();
        }
    }
}

/// A why-record: a sheet's or a hit's.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ExplainRecord {
    /// A sheet a use resolved.
    Sheet(SheetRecord),
    /// A hit the pipeline resolved past its target's checks.
    Hit(HitRecord),
}

/// Where a line was read: into a sheet at its use, or at its hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadAt {
    /// At the use, into the sheet.
    Use,
    /// At the hit, a line the use left for it.
    Hit,
}

/// What a read did with a line.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LineOutcome {
    /// The read took it.
    Applied,
    /// The read took it through the part's history alone.
    ThroughHistory,
    /// A read at use left it for the hit.
    Deferred,
    /// A filter kept it out, and why (`skill.melee not in context`).
    Skipped(String),
}

/// A line as a read met it: its source, its stat (`pen` for penetration
/// points), its modifier (a `pen` line's points as `add`) and filters,
/// where it was read and what the read did.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct LineRecord {
    /// The source key that placed it (`passive:spellcraft`).
    pub source: String,
    /// The stat it is on, or `pen`.
    pub stat: String,
    /// Its kind and value.
    pub modifier: Modifier,
    /// Its filters.
    pub scope: Scope,
    /// Where it was read.
    pub read: ReadAt,
    /// What the read did with it.
    pub outcome: LineOutcome,
}

/// A sheet stat as its use read it: the base, each line, the sums of those
/// it took, the clamp and the value.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct StatRecord {
    /// The stat (`crit_chance`).
    pub stat: String,
    /// Its base before the lines.
    pub base: f32,
    /// Each line on it.
    pub lines: Vec<LineRecord>,
    /// The sums of the lines the read took.
    pub sums: Sums,
    /// The bounds the value is clamped to.
    pub clamp: [f32; 2],
    /// The value the sheet took.
    pub value: f32,
}

/// One step on a part's way from its base at a use.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum OriginStep {
    /// The share conversion left of the type.
    Kept(f32),
    /// A share converted to `to` by a `convert` line of `source`.
    Converted {
        /// The share.
        share: f32,
        /// The type it became.
        to: String,
        /// The line's source.
        source: String,
    },
    /// A share added as `to` by an `extra` line of `source`.
    Extra {
        /// The share.
        share: f32,
        /// The type it became.
        to: String,
        /// The line's source.
        source: String,
    },
}

/// Where a part came from at its use: a base range of a type, the skill's
/// own or an `added` line's, then each conversion or extra on the way.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PartOrigin {
    /// The base's damage type.
    pub kind: String,
    /// The base's range.
    pub range: [f32; 2],
    /// The `added` line's source, or none for the skill's own base.
    pub added: Option<String>,
    /// The conversions and extras on the way, in order.
    pub steps: Vec<OriginStep>,
}

impl PartOrigin {
    pub(crate) fn new(kind: &str, range: [f32; 2], added: Option<&str>) -> Self {
        Self {
            kind: kind.to_owned(),
            range,
            added: added.map(str::to_owned),
            steps: Vec::new(),
        }
    }

    /// `origins`, each with `step` after its own.
    pub(crate) fn stepped(origins: &[PartOrigin], step: &OriginStep) -> Vec<PartOrigin> {
        origins
            .iter()
            .map(|origin| {
                let mut origin = origin.clone();
                origin.steps.push(step.clone());
                origin
            })
            .collect()
    }

    /// Its base at the range draw `u`.
    fn at(&self, u: f32) -> f64 {
        let [min, max] = self.range.map(f64::from);
        min + f64::from(u) * (max - min)
    }
}

/// A part a sheet snapshots: its type, history and range, where it came
/// from, and the lines its use read on the hit track.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PartRecord {
    /// Its damage type.
    pub kind: String,
    /// The types it came from, sorted.
    pub history: Vec<String>,
    /// Its range.
    pub range: [f32; 2],
    /// Where it came from.
    pub origins: Vec<PartOrigin>,
    /// The `damage` and `pen` lines its use read.
    pub lines: Vec<LineRecord>,
}

/// A sheet a use resolved: the holder, the ability, the skill's level,
/// each sheet stat and each part.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct SheetRecord {
    /// The entity whose use it was.
    pub holder: Entity,
    /// The ability's entry name.
    pub ability: String,
    /// The skill's level at the use.
    pub level: f32,
    /// `skill_level`, `crit_chance`, `crit_multiplier`, `cost` (when the
    /// skill has one) and `cooldown`.
    pub stats: Vec<StatRecord>,
    /// Its parts, in order.
    pub parts: Vec<PartRecord>,
}

/// A hit's crit as the resolve met it.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum CritRecord {
    /// None could happen: a periodic hit, or a source without a chance.
    None,
    /// The request's builder decided it before the resolve.
    Decided(bool),
    /// The resolve drew it: it crit when `roll` fell under `chance`, every
    /// part times `multiplier`.
    Drawn {
        /// The chance.
        chance: f32,
        /// The draw.
        roll: f32,
        /// The crit's factor.
        multiplier: f32,
    },
}

/// A resistance as one part met it: the factor after its floors, the floor
/// of the part's type, the penetration points and the product they make.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ResistRecord {
    /// The factor in the hit's context, raised to its floor.
    pub factor: f32,
    /// The part's type's floor, when it lists one.
    pub floor: Option<f32>,
    /// The penetration points, capped at 1.
    pub pen: f32,
    /// What the part was multiplied by: 0 for an immunity.
    pub product: f64,
}

/// One part of a hit: its types and history, where it came from, its base,
/// the lines on it with their sums, the crit's factor, its amount before
/// the defences, the resistance, `damage_taken` and what landed.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HitPartRecord {
    /// Its damage types.
    pub tags: Vec<String>,
    /// The types it came from, sorted.
    pub history: Vec<String>,
    /// Where it came from, for a delivery's part.
    pub origins: Vec<PartOrigin>,
    /// Its amount before the lines: its range at the draw, or the
    /// request's.
    pub base: f64,
    /// The lines on it, those of its use then those of its hit.
    pub lines: Vec<LineRecord>,
    /// The `pct` sum of the lines taken.
    pub pct: f64,
    /// The `mul` product of the lines taken.
    pub mul: f64,
    /// The factor the lines made: `(1 + pct) × mul`, or an override.
    pub factor: f64,
    /// The crit's factor, 1 without.
    pub crit: f64,
    /// Its amount before the defences.
    pub before: f64,
    /// The target's resistance, when it has one.
    pub resist: Option<ResistRecord>,
    /// The target's `damage_taken` in the hit's context, 1 without.
    pub taken: f64,
    /// What landed: 0 when it dropped or met an immunity.
    pub after: f32,
    /// An immunity dropped it.
    pub immune: bool,
}

/// A hit the pipeline resolved: its target and source, the range draw for
/// a delivery's, its crit, each part and the amount that landed, and the
/// sheet its delivery carried.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct HitRecord {
    /// The entity hit.
    pub target: Entity,
    /// The entity it came from, when known.
    pub source: Option<Entity>,
    /// A periodic request's hit.
    pub periodic: bool,
    /// The range draw every part shared, for a delivery's hit.
    pub draw: Option<f32>,
    /// Its crit.
    pub crit: CritRecord,
    /// Its parts, in order.
    pub parts: Vec<HitPartRecord>,
    /// The amount that landed.
    pub amount: f32,
    /// It killed the target.
    pub killed: bool,
    /// The sheet its delivery carried, recorded at the use.
    pub sheet: Option<Box<SheetRecord>>,
}

impl HitRecord {
    pub(crate) fn new(target: Entity, source: Option<Entity>, periodic: bool) -> Self {
        Self {
            target,
            source,
            periodic,
            draw: None,
            crit: CritRecord::None,
            parts: Vec::new(),
            amount: 0.0,
            killed: false,
            sheet: None,
        }
    }
}

/// What a read at the hit or the use did with a line it reached through
/// `reach`: taken, through the history alone when `bare` (the read without
/// the history) would not take it, left for the hit, or kept out and why.
pub(crate) fn outcome(
    reach: Reach,
    bare: impl FnOnce() -> Reach,
    why: impl FnOnce(Skip) -> String,
) -> LineOutcome {
    match reach {
        Reach::Applies if bare() == Reach::Applies => LineOutcome::Applied,
        Reach::Applies => LineOutcome::ThroughHistory,
        Reach::Deferred => LineOutcome::Deferred,
        Reach::Skipped(skip) => LineOutcome::Skipped(why(skip)),
    }
}

/// Why `skip` kept a line of `scope` out of `read`: the names its failing
/// filter lacked or held, against the read's context, the `holder`'s tags
/// or the `other` party's (`skill.melee not in context`,
/// `delivery.chained not set`, `status.chilled not on the target`).
pub(crate) fn why(
    skip: Skip,
    scope: &Scope,
    read: &Read<'_>,
    holder: Option<&Tags>,
    other: Option<&Tags>,
) -> String {
    match skip {
        Skip::Context => reasons(
            scope.context_filter(),
            |tag| carries(read, tag),
            "in context",
        ),
        Skip::Holder => reasons(
            scope.holder_filter(),
            |tag| holder.is_some_and(|tags| tags.has(tag)),
            "on the holder",
        ),
        Skip::Against => reasons(
            scope.against_filter(),
            |tag| other.is_some_and(|tags| tags.has(tag)),
            "on the target",
        ),
        Skip::NoHit => "against reads only at a hit".to_string(),
    }
}

/// Whether `read`'s context carries `tag`, by a probe line.
fn carries(read: &Read<'_>, tag: &str) -> bool {
    let probe = Scope::new().with(TagFilter::new().all([tag]));
    read.reach_scope(&probe) == Reach::Applies
}

/// The names `filter` failed on, `has` telling which a read holds: each
/// `all` name missing, an `any` list none of whose names is there, each
/// `none` name there.
fn reasons(filter: &TagFilter, has: impl Fn(&str) -> bool, place: &str) -> String {
    let [all, any, none] = lists(filter);
    let missing = |tag: &str| {
        if place == "in context" && STATE_TAGS.contains(&tag) {
            format!("{tag} not set")
        } else {
            format!("{tag} not {place}")
        }
    };
    let mut out: Vec<String> = all
        .iter()
        .filter(|tag| !has(tag))
        .map(|tag| missing(tag))
        .collect();
    match any.as_slice() {
        [] => {}
        [tag] if !has(tag) => out.push(missing(tag)),
        names if !names.iter().any(|tag| has(tag)) => {
            out.push(format!("none of {} {place}", names.join(", ")));
        }
        _ => {}
    }
    out.extend(
        none.iter()
            .filter(|tag| has(tag))
            .map(|tag| format!("{tag} {place}")),
    );
    out.join("; ")
}

/// `filter`'s `all`, `any` and `none` lists, from its data form.
fn lists(filter: &TagFilter) -> [Vec<String>; 3] {
    let data = serde_json::to_value(filter).unwrap_or_default();
    ["all", "any", "none"].map(|list| {
        data.get(list)
            .and_then(serde_json::Value::as_array)
            .map_or_else(Vec::new, |names| {
                names
                    .iter()
                    .filter_map(|name| name.as_str().map(str::to_owned))
                    .collect()
            })
    })
}

/// A scope's filters as the listing writes them: `with skill.spell`,
/// `while status.focused`, `against status.chilled`.
fn filter_text(scope: &Scope) -> String {
    let mut out = Vec::new();
    for (word, filter) in [
        ("with", scope.context_filter()),
        ("while", scope.holder_filter()),
        ("against", scope.against_filter()),
    ] {
        if filter.is_everything() {
            continue;
        }
        let [mut names, any, none] = lists(filter);
        if !any.is_empty() {
            names.push(any.join(" or "));
        }
        names.extend(none.iter().map(|tag| format!("not {tag}")));
        out.push(format!("{word} {}", names.join(", ")));
    }
    out.join(" ")
}

/// A type's last segment: `cold` of `damage.cold`.
fn short(kind: &str) -> &str {
    kind.rsplit('.').next().unwrap_or(kind)
}

/// One line of a listing: kind, value, source, filters and the read.
fn write_line(f: &mut fmt::Formatter<'_>, indent: &str, line: &LineRecord) -> fmt::Result {
    let taken = !matches!(line.outcome, LineOutcome::Skipped(_));
    let (kind, value) = match line.modifier {
        _ if line.stat == "pen" => ("pen", format!("{:+.2}", modifier_value(line.modifier))),
        Modifier::Add(v) => ("add", format!("{v:+.2}")),
        Modifier::Pct(v) if taken => ("pct", format!("{v:+.2}")),
        Modifier::Pct(v) => ("pct", format!("{v:.2}")),
        Modifier::Mul(v) => ("mul", format!("×{v:.2}")),
        Modifier::Override(v) => ("set", format!("={v:.2}")),
    };
    let read = match line.read {
        ReadAt::Use => "use",
        ReadAt::Hit => "hit",
    };
    let read = match &line.outcome {
        LineOutcome::Applied => read.to_string(),
        LineOutcome::ThroughHistory => format!("{read} (history)"),
        LineOutcome::Deferred => "left for the hit".to_string(),
        LineOutcome::Skipped(why) => format!("skipped: {why}"),
    };
    writeln!(
        f,
        "{indent}{kind:<4} {value:>5} {:<20} {:<29} {read}",
        line.source,
        filter_text(&line.scope)
    )
}

fn modifier_value(modifier: Modifier) -> f32 {
    match modifier {
        Modifier::Add(v) | Modifier::Pct(v) | Modifier::Mul(v) | Modifier::Override(v) => v,
    }
}

/// Where a part came from, as the listing writes it: `from fire 16–24 at
/// u 0.50 → 20.00, converted 0.5 to cold (support:rimecraft)`.
fn write_origins(
    f: &mut fmt::Formatter<'_>,
    origins: &[PartOrigin],
    u: Option<f32>,
) -> fmt::Result {
    for (i, origin) in origins.iter().enumerate() {
        let [min, max] = origin.range;
        let sep = if i == 0 { "   " } else { "; " };
        write!(f, "{sep}from ")?;
        if let Some(source) = &origin.added {
            write!(f, "added ")?;
            write!(f, "{} {min}–{max} ({source})", short(&origin.kind))?;
        } else {
            write!(f, "{} {min}–{max}", short(&origin.kind))?;
        }
        if let Some(u) = u {
            write!(f, " at u {u:.2} → {:.2}", origin.at(u))?;
        }
        for step in &origin.steps {
            match step {
                OriginStep::Kept(share) => write!(f, ", kept {share}")?,
                OriginStep::Converted { share, to, source } => {
                    write!(f, ", converted {share} to {} ({source})", short(to))?;
                }
                OriginStep::Extra { share, to, source } => {
                    write!(f, ", extra {share} as {} ({source})", short(to))?;
                }
            }
        }
    }
    Ok(())
}

impl fmt::Display for SheetRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "sheet {} of {} at level {}",
            self.ability, self.holder, self.level
        )?;
        for stat in &self.stats {
            let [min, max] = stat.clamp;
            writeln!(
                f,
                "  {} {:.2} in [{min}, {max}]: {:.2} × {:.2} × {:.2}",
                stat.stat,
                stat.value,
                stat.sums.add(),
                1.0 + stat.sums.pct(),
                stat.sums.mul()
            )?;
            for line in &stat.lines {
                write_line(f, "    ", line)?;
            }
        }
        for part in &self.parts {
            let [min, max] = part.range;
            write!(f, "{} part  {min}–{max}", short(&part.kind))?;
            write_origins(f, &part.origins, None)?;
            writeln!(f)?;
            for line in &part.lines {
                write_line(f, "  ", line)?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for HitRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "hit on {}", self.target)?;
        if let Some(source) = self.source {
            write!(f, " from {source}")?;
        }
        let killed = if self.killed { ", killed" } else { "" };
        writeln!(f, ": {:.2}{killed}", self.amount)?;
        let crit = match self.crit {
            CritRecord::None => "no crit".to_string(),
            CritRecord::Decided(true) => "crit (decided)".to_string(),
            CritRecord::Decided(false) => "no crit (decided)".to_string(),
            CritRecord::Drawn {
                chance,
                roll,
                multiplier,
            } if roll < chance => format!("crit ×{multiplier:.2} (draw {roll:.2} < {chance:.2})"),
            CritRecord::Drawn { chance, roll, .. } => {
                format!("no crit (draw {roll:.2} ≥ {chance:.2})")
            }
        };
        for part in &self.parts {
            let kind = part.tags.first().map_or("untyped", String::as_str);
            write!(f, "{} part  {:.2}", short(kind), part.base)?;
            write_origins(f, &part.origins, self.draw)?;
            writeln!(f)?;
            for line in &part.lines {
                write_line(f, "  ", line)?;
            }
            let lined = part.base * part.factor;
            write!(
                f,
                "  = {:.2} × {:.2} × {:.2} = {lined:.2}; {crit}",
                part.base,
                1.0 + part.pct,
                part.mul
            )?;
            if part.crit == 1.0 {
                writeln!(f)?;
            } else {
                writeln!(f, " → {:.2}", part.before)?;
            }
            match part.resist {
                None => writeln!(f, "  resist  none → {:.2}", part.before)?,
                Some(_) if part.immune => writeln!(f, "  resist  {kind} immune")?,
                Some(resist) => {
                    write!(f, "  resist  {kind} {:.2}", resist.factor)?;
                    if resist.pen > 0.0 {
                        write!(f, " + {:.2} pen = {:.2}", resist.pen, resist.product)?;
                    }
                    let floor = resist.floor.map_or_else(
                        || "no floor".to_string(),
                        |floor| format!("floor {floor:.2}"),
                    );
                    let pen = if resist.pen > 0.0 {
                        ""
                    } else {
                        "; no penetration"
                    };
                    writeln!(f, " ({floor}{pen}) → {:.2}", part.before * resist.product)?;
                }
            }
            writeln!(f, "  damage_taken {:.2} → {:.2}", part.taken, part.after)?;
        }
        Ok(())
    }
}

impl fmt::Display for ExplainRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExplainRecord::Sheet(sheet) => sheet.fmt(f),
            ExplainRecord::Hit(hit) => hit.fmt(f),
        }
    }
}

#[cfg(test)]
#[path = "../tests/explain.rs"]
mod tests;
