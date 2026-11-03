//! Stats: named values with ordered modifiers and scoped lines, which the
//! kit's movers, health, cooldowns and hit items read when an entity
//! carries them.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tungsten_core::{
    DataRegistry, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext, World, system,
};

use crate::damage::health::Health;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, TOP_DOWN, describe,
    describe_kind, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::lines::{Line, Reach, Read, Sums};
use crate::foundation::phases::{Phase, ensure_phases};
use crate::foundation::world_sources::{WorldSources, bring_stale};
use crate::lifecycle::cooldown::Cooldown;
use crate::motion::platformer::PlatformerController;
use crate::motion::steering::Steering;
use crate::motion::top_down::TopDownMover;
use crate::overlap::contact::ContactDamage;
use crate::overlap::hit::Hitbox;

/// [`StatsPlugin`]'s system, in the tick phase.
pub const STATS_APPLY: &str = "stats_apply";

/// The scene component the plugin registers, and the data kind.
const STATS: &str = "stats";
/// The shape of a `stats` entry's value.
const STAT_BOUNDS: &str = "stat_bounds";

/// Whether `name` is a stat name: `[a-z][a-z0-9_]*`.
pub(crate) fn valid_stat(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The bounds of a well-known stat: `max_health` from 1, `crit_chance` in
/// `[0, 1]`, `crit_multiplier` from 1, the rest from 0, none above; `None`
/// for any other name.
pub(crate) fn well_known_bounds(name: &str) -> Option<(f32, f32)> {
    if name == Stats::MAX_HEALTH || name == Stats::CRIT_MULTIPLIER {
        Some((1.0, f32::INFINITY))
    } else if name == Stats::CRIT_CHANCE {
        Some((0.0, 1.0))
    } else if Stats::WELL_KNOWN.contains(&name) {
        Some((0.0, f32::INFINITY))
    } else {
        None
    }
}

/// One stat: its base, its clamp, its modifiers in add order and the value
/// they make.
#[derive(Debug, Clone, PartialEq)]
struct Stat {
    name: String,
    base: f32,
    min: f32,
    max: f32,
    modifiers: Vec<(String, Modifier)>,
    value: f32,
}

impl Stat {
    fn new(name: &str, base: f32) -> Self {
        let (min, max) = well_known_bounds(name).unwrap_or((f32::NEG_INFINITY, f32::INFINITY));
        let mut stat = Self {
            name: name.to_owned(),
            base,
            min,
            max,
            modifiers: Vec::new(),
            value: 0.0,
        };
        stat.recompute();
        stat
    }

    /// The value [`modified`] makes of the base, the modifiers and the
    /// bounds.
    fn recompute(&mut self) {
        self.value = modified(
            self.base,
            self.modifiers.iter().map(|&(_, modifier)| modifier),
            self.min,
            self.max,
        );
    }
}

/// `((base + Σadd) × (1 + Σpct) × Πmul)`, replaced by the last-added
/// override, then clamped to `[min, max]` and to `±f32::MAX`: a stat's value,
/// and a [`Resist`](crate::Resist) factor's under its modifiers. The sums
/// and products run in `f64`, and a zero factor makes the product 0, so no
/// `0 × ∞` arises and the value is always finite.
pub(crate) fn modified(
    base: f32,
    modifiers: impl IntoIterator<Item = Modifier>,
    min: f32,
    max: f32,
) -> f32 {
    let mut sums = Sums::new(base);
    for modifier in modifiers {
        sums.fold(modifier);
    }
    sums.value(min, max)
}

/// A line [`Stats`] keeps for the reads that take a context: a scoped one,
/// or one on a stat the entity lacks.
#[derive(Debug, Clone, PartialEq)]
struct KeptLine {
    stat: String,
    source: String,
    line: Line,
}

/// The lines [`Stats`] keeps beside its stats, boxed so stats without one
/// stay small.
#[derive(Debug, Clone, Default, PartialEq)]
struct Kept {
    /// Sorted by stat, in add order within one.
    lines: Vec<KeptLine>,
}

/// An entity's stats: named values such as `move_speed` or `armor`, each a
/// base changed by modifiers. A stat's value is `((base + Σadd) × (1 +
/// Σpct) × Πmul)`, replaced by the last-added [`Modifier::Override`], then
/// clamped to its bounds, and it is recomputed when a base or a modifier
/// changes, so [`get`](Self::get) never computes. Every value is finite:
/// the arithmetic runs in `f64`, a zero factor makes the product 0, and
/// the result is clamped to `±f32::MAX` too.
///
/// A [`Line`] scoped by tag filters ([`add_line`](Self::add_line)) reaches
/// only the reads that take a context: [`read`](Self::read), and
/// [`sums`](Self::sums), whose skill sheets bring their own base, so a line
/// on a stat the entity lacks is kept for them. [`get`](Self::get) and every
/// item's read stay the base and the unscoped modifiers (§5.1 of the ARPG
/// design's empty context).
///
/// The well-known stats ([`WELL_KNOWN`](Self::WELL_KNOWN)) are bounded to
/// `[0, ∞)`, [`MAX_HEALTH`](Self::MAX_HEALTH) and
/// [`CRIT_MULTIPLIER`](Self::CRIT_MULTIPLIER) to `[1, ∞)`,
/// [`CRIT_CHANCE`](Self::CRIT_CHANCE) to `[0, 1]`. [`StatsPlugin`]'s
/// [`STATS_APPLY`] writes each one an entity has into its item, when they
/// differ: [`MOVE_SPEED`](Self::MOVE_SPEED) into a [`TopDownMover`]'s speed,
/// a [`PlatformerController`]'s run speed or a [`Steering`]'s top speed (0
/// stops it),
/// [`MAX_HEALTH`](Self::MAX_HEALTH) into a [`Health`]'s max, its current
/// health kept as the same fraction of it,
/// [`COOLDOWN_RATE`](Self::COOLDOWN_RATE) into a [`Cooldown`]'s rate and
/// [`DAMAGE`](Self::DAMAGE) into a [`Hitbox`]'s and a [`ContactDamage`]'s
/// damage. An item without that stat runs on its own field; an item reads
/// no stat of another entity's, but for a pickup's magnet, which reads its
/// collector's [`PICKUP_RADIUS`](Self::PICKUP_RADIUS). A game's stat is
/// unbounded unless [`with_bounds`](Self::with_bounds),
/// [`StatsPlugin::with_stat`] or the `stats` data kind bound it.
///
/// Modifiers and lines are keyed by a source, such as an effect's ID, so
/// [`remove_modifiers`](Self::remove_modifiers) takes back everything one
/// source gave.
///
/// Scene name `stats`: `{ "move_speed": 160, "armor": 2 }`, the base
/// values by stat name, or a stat as `{ "base": 1, "lines": [{ "modifier":
/// { "mul": 0.5 }, "with": { "all": ["hit.blast"] } }] }`, its lines the
/// entity's own under the source `base`; the load checks each name is
/// well-known or declared, each base lies in its bounds and each line's
/// tags are declared. It is data: a game's save holds the bases, bounds,
/// modifiers and kept lines.
///
/// # Examples
///
/// ```
/// use tungsten_kit::{Modifier, Stats};
///
/// let mut stats = Stats::new().with(Stats::MOVE_SPEED, 100.0).with("armor", 2.0);
/// stats.add_modifier(Stats::MOVE_SPEED, "haste", Modifier::Pct(0.5));
/// stats.add_modifier(Stats::MOVE_SPEED, "boots", Modifier::Add(20.0));
/// assert_eq!(stats.get(Stats::MOVE_SPEED), Some(180.0), "(100 + 20) × 1.5");
///
/// // A slow below zero clamps to the well-known stat's floor.
/// stats.add_modifier(Stats::MOVE_SPEED, "glue", Modifier::Add(-500.0));
/// assert_eq!(stats.get(Stats::MOVE_SPEED), Some(0.0));
/// assert_eq!(stats.remove_modifiers("glue"), 1);
/// assert_eq!(stats.get(Stats::MOVE_SPEED), Some(180.0));
///
/// // An override wins over the rest; a game stat takes its own clamp.
/// stats.add_modifier("armor", "stone", Modifier::Override(50.0));
/// let stats = stats.with_bounds("armor", 0.0, 10.0);
/// assert_eq!(stats.get("armor"), Some(10.0));
/// assert_eq!(stats.base("armor"), Some(2.0));
/// assert_eq!(stats.get("luck"), None);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(
    try_from = "BTreeMap<String, StatFields>",
    into = "BTreeMap<String, StatFields>"
)]
#[allow(clippy::struct_field_names)] // `stats` holds the stats themselves, beside the kept lines.
pub struct Stats {
    /// Sorted by name.
    stats: Vec<Stat>,
    /// The kept lines, when there are any.
    kept: Option<Box<Kept>>,
    /// The [`WorldSources`] revision the stats were brought to; neither
    /// saved nor compared.
    revision: u64,
}

impl PartialEq for Stats {
    /// The stats and their kept lines, not the world-source revision.
    fn eq(&self, other: &Self) -> bool {
        self.stats == other.stats && self.kept == other.kept
    }
}

/// A stat as a save writes it: unbounded ends as `null`; a stat the entity
/// lacks, holding only kept lines, without a base. Its bounds sit in an
/// `f32` each and its kept lines behind one pointer, so it is as small as a
/// save of stats without lines has always made it: a capture builds one
/// per stat.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    base: Option<f32>,
    #[serde(default)]
    min: SavedBound,
    #[serde(default)]
    max: SavedBound,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    modifiers: Vec<(String, Modifier)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lines: Option<Box<SavedLines>>,
}

/// A stat's kept lines as a save writes them: source and line pairs.
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct SavedLines(Vec<(String, Line)>);

/// A bound as a save writes it, `null` for none, held as NaN.
#[derive(Clone, Copy)]
struct SavedBound(f32);

impl Default for SavedBound {
    fn default() -> Self {
        Self(f32::NAN)
    }
}

impl SavedBound {
    /// The bound of a finite value, else none.
    fn of(value: f32) -> Self {
        if value.is_finite() {
            Self(value)
        } else {
            Self::default()
        }
    }

    /// Its value, `none` when it holds none.
    fn or(self, none: f32) -> f32 {
        if self.0.is_nan() { none } else { self.0 }
    }

    fn is_some(self) -> bool {
        !self.0.is_nan()
    }
}

impl Serialize for SavedBound {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0.is_nan() {
            serializer.serialize_none()
        } else {
            serializer.serialize_some(&self.0)
        }
    }
}

impl<'de> Deserialize<'de> for SavedBound {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Option::<f32>::deserialize(deserializer)?.map_or_else(Self::default, Self))
    }
}

impl TryFrom<BTreeMap<String, StatFields>> for Stats {
    type Error = String;

    fn try_from(fields: BTreeMap<String, StatFields>) -> Result<Self, Self::Error> {
        let mut stats = Self {
            stats: Vec::with_capacity(fields.len()),
            ..Self::default()
        };
        for (name, fields) in fields {
            if !valid_stat(&name) {
                return Err(format!("`{name}` is not a stat name (`[a-z][a-z0-9_]*`)"));
            }
            let lines = fields.lines.map(|lines| lines.0).unwrap_or_default();
            match fields.base {
                Some(base) => {
                    if !base.is_finite() {
                        return Err(format!("{name}.base: must be finite (got {base})"));
                    }
                    let min = fields.min.or(f32::NEG_INFINITY);
                    let max = fields.max.or(f32::INFINITY);
                    check_bounds(min, max).map_err(|e| format!("{name}: {e}"))?;
                    if let Some((_, modifier)) =
                        fields.modifiers.iter().find(|(_, m)| !m.is_finite())
                    {
                        return Err(format!(
                            "{name}: a modifier must be finite (got {modifier:?})"
                        ));
                    }
                    let mut stat = Stat::new(&name, base);
                    stat.min = min;
                    stat.max = max;
                    stat.modifiers = fields.modifiers;
                    stat.recompute();
                    stats.stats.push(stat);
                }
                None if lines.is_empty() => {
                    return Err(format!("{name}: a stat needs a base or kept lines"));
                }
                None if fields.min.is_some()
                    || fields.max.is_some()
                    || !fields.modifiers.is_empty() =>
                {
                    return Err(format!(
                        "{name}: a stat without a base holds only kept lines"
                    ));
                }
                None => {}
            }
            for (source, line) in lines {
                stats.add_line(&name, &source, line);
            }
        }
        Ok(stats)
    }
}

impl From<Stats> for BTreeMap<String, StatFields> {
    fn from(stats: Stats) -> Self {
        let mut out: BTreeMap<String, StatFields> = stats
            .stats
            .into_iter()
            .map(|stat| {
                let fields = StatFields {
                    base: Some(stat.base),
                    min: SavedBound::of(stat.min),
                    max: SavedBound::of(stat.max),
                    modifiers: stat.modifiers,
                    lines: None,
                };
                (stat.name, fields)
            })
            .collect();
        for KeptLine { stat, source, line } in stats.kept.map(|kept| kept.lines).unwrap_or_default()
        {
            let fields = out.entry(stat).or_insert_with(|| StatFields {
                base: None,
                min: SavedBound::default(),
                max: SavedBound::default(),
                modifiers: Vec::new(),
                lines: None,
            });
            fields
                .lines
                .get_or_insert_with(|| Box::new(SavedLines(Vec::new())))
                .0
                .push((source, line));
        }
        out
    }
}

/// `min <= max`, neither NaN.
fn check_bounds(min: f32, max: f32) -> Result<(), String> {
    if min.is_nan() || max.is_nan() || min > max {
        Err(format!("the bounds {min}..{max} are not ordered"))
    } else {
        Ok(())
    }
}

impl Stats {
    /// The speed a mover runs at, in world pixels per second.
    pub const MOVE_SPEED: &'static str = "move_speed";
    /// A [`Health`]'s full health.
    pub const MAX_HEALTH: &'static str = "max_health";
    /// The damage of the entity's [`Hitbox`] and [`ContactDamage`], which
    /// `stats_apply` writes: one number, or a typed item's total, which its
    /// parts split by their proportions.
    pub const DAMAGE: &'static str = "damage";
    /// How fast a [`Cooldown`] counts down: 1 is the game clock, 2 twice
    /// as fast, 0 holds it.
    pub const COOLDOWN_RATE: &'static str = "cooldown_rate";
    /// An area's size, which the kit's effects will read (W13k); no item
    /// reads it yet.
    pub const AREA: &'static str = "area";
    /// How far from a collector its pickups' magnets reach, in world
    /// pixels: a [`Pickup`](crate::Pickup) with a magnet moves toward the
    /// nearest collector whose radius reaches it.
    pub const PICKUP_RADIUS: &'static str = "pickup_radius";
    /// The speed of the projectiles an entity fires, which the kit's
    /// abilities will read when they spawn one (W13k); no item reads it
    /// yet.
    pub const PROJECTILE_SPEED: &'static str = "projectile_speed";
    /// The chance in `[0, 1]` that a source's hit crits, so every genre's
    /// source can crit without a declaration.
    pub const CRIT_CHANCE: &'static str = "crit_chance";
    /// What a crit multiplies a hit by, from 1.
    pub const CRIT_MULTIPLIER: &'static str = "crit_multiplier";
    /// What a target's hits are multiplied by, from 0, read in the hit's
    /// context: `damage_taken` lines scoped by a hit's tags shape what it
    /// takes.
    pub const DAMAGE_TAKEN: &'static str = "damage_taken";
    /// The stats every game knows without declaring them.
    pub const WELL_KNOWN: [&'static str; 10] = [
        Self::MOVE_SPEED,
        Self::MAX_HEALTH,
        Self::DAMAGE,
        Self::COOLDOWN_RATE,
        Self::AREA,
        Self::PICKUP_RADIUS,
        Self::PROJECTILE_SPEED,
        Self::CRIT_CHANCE,
        Self::CRIT_MULTIPLIER,
        Self::DAMAGE_TAKEN,
    ];

    /// No stats.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The stats with `stat` at base `base`, added or replaced; a new
    /// well-known stat takes its bounds, a new game stat none.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*` or `base` is not finite.
    #[must_use]
    #[track_caller]
    pub fn with(mut self, stat: &str, base: f32) -> Self {
        self.set_base(stat, base);
        self
    }

    /// The stats with `stat` clamped to `[min, max]`, either end infinite
    /// for none: a game stat's clamp.
    ///
    /// # Panics
    ///
    /// When the stats lack `stat`, or `min` is above `max` or either is
    /// NaN.
    #[must_use]
    #[track_caller]
    pub fn with_bounds(mut self, stat: &str, min: f32, max: f32) -> Self {
        if let Err(error) = check_bounds(min, max) {
            panic!("Stats::with_bounds: {stat}: {error}");
        }
        let Some(entry) = self.find_mut(stat) else {
            panic!("Stats::with_bounds: no stat `{stat}`; add it with `with` first");
        };
        entry.min = min;
        entry.max = max;
        entry.recompute();
        self
    }

    /// Sets `stat`'s base, adding the stat when it is not there, with
    /// [`with`](Self::with)'s bounds; a new stat takes the unscoped lines
    /// kept for it as its modifiers, in add order.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*` or `base` is not finite.
    #[track_caller]
    pub fn set_base(&mut self, stat: &str, base: f32) {
        assert!(
            valid_stat(stat),
            "Stats: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
        );
        assert!(
            base.is_finite(),
            "Stats: {stat}'s base must be finite (got {base})"
        );
        match self.search(stat) {
            Ok(at) => {
                self.stats[at].base = base;
                self.stats[at].recompute();
            }
            Err(at) => {
                let mut entry = Stat::new(stat, base);
                if let Some(kept) = &mut self.kept {
                    let mut i = 0;
                    while i < kept.lines.len() {
                        if kept.lines[i].stat == stat && !kept.lines[i].line.is_scoped() {
                            let moved = kept.lines.remove(i);
                            entry.modifiers.push((moved.source, moved.line.modifier()));
                        } else {
                            i += 1;
                        }
                    }
                    entry.recompute();
                }
                self.drop_empty_kept();
                self.stats.insert(at, entry);
            }
        }
    }

    /// `stat`'s base, when it has the stat.
    #[must_use]
    pub fn base(&self, stat: &str) -> Option<f32> {
        self.find(stat).map(|stat| stat.base)
    }

    /// `stat`'s current value, when it has the stat: kept up to date, never
    /// computed on read.
    #[must_use]
    pub fn get(&self, stat: &str) -> Option<f32> {
        self.find(stat).map(|stat| stat.value)
    }

    /// Adds `modifier` to `stat` from `source`, after its earlier ones, and
    /// recomputes it; whether the stats have `stat`. A stat the entity lacks
    /// takes nothing, since there is no base to change.
    ///
    /// # Panics
    ///
    /// When the modifier's value is not finite.
    #[track_caller]
    pub fn add_modifier(&mut self, stat: &str, source: &str, modifier: Modifier) -> bool {
        assert!(
            modifier.is_finite(),
            "Stats::add_modifier: {stat}: the modifier must be finite (got {modifier:?})"
        );
        let Some(entry) = self.find_mut(stat) else {
            return false;
        };
        entry.modifiers.push((source.to_owned(), modifier));
        entry.recompute();
        true
    }

    /// Adds `line` to `stat` from `source`, after its earlier ones. An
    /// unscoped line on a stat the entity has is
    /// [`add_modifier`](Self::add_modifier)'s, so [`get`](Self::get) takes
    /// it; a scoped one, or any on a stat the entity lacks, is kept for the
    /// reads that take a context, [`read`](Self::read) and
    /// [`sums`](Self::sums), whose skill sheets bring their own base. A
    /// [`Line`] cannot be a scoped override, which its builders and the
    /// loads refuse.
    ///
    /// # Panics
    ///
    /// When `stat` is not `[a-z][a-z0-9_]*`.
    #[track_caller]
    pub fn add_line(&mut self, stat: &str, source: &str, line: Line) {
        assert!(
            valid_stat(stat),
            "Stats::add_line: `{stat}` is not a stat name (`[a-z][a-z0-9_]*`)"
        );
        if !line.is_scoped()
            && let Some(entry) = self.find_mut(stat)
        {
            entry.modifiers.push((source.to_owned(), line.modifier()));
            entry.recompute();
            return;
        }
        let kept = self.kept.get_or_insert_with(Box::default);
        let at = kept
            .lines
            .partition_point(|kept| kept.stat.as_str() <= stat);
        kept.lines.insert(
            at,
            KeptLine {
                stat: stat.to_owned(),
                source: source.to_owned(),
                line,
            },
        );
    }

    /// `stat` read in `read`'s context, when the entity has the stat: its
    /// base, its modifiers and the kept lines the read takes, clamped to its
    /// bounds. A read without kept lines is [`get`](Self::get)'s value; a
    /// read at use skips the lines it defers.
    #[must_use]
    pub fn read(&self, stat: &str, read: &Read<'_>) -> Option<f32> {
        let entry = self.find(stat)?;
        if self.kept.is_none() {
            return Some(entry.value);
        }
        Some(self.sums(stat, read).value(entry.min, entry.max))
    }

    /// `stat`'s sums in `read`'s context, for a skill sheet or a hit that
    /// brings its own base: the entity's base (0 without the stat), its
    /// modifiers and the kept lines the read takes, with the lines a read
    /// at use leaves for the hit.
    #[must_use]
    pub fn sums(&self, stat: &str, read: &Read<'_>) -> Sums {
        let entry = self.find(stat);
        let mut sums = Sums::new(entry.map_or(0.0, |entry| entry.base));
        if let Some(entry) = entry {
            for &(_, modifier) in &entry.modifiers {
                sums.fold(modifier);
            }
        }
        for kept in self.kept_on(stat) {
            match read.reach(&kept.line) {
                Reach::Applies => sums.fold(kept.line.modifier()),
                Reach::Deferred => sums.defer(&kept.source, &kept.line),
                Reach::Skipped(_) => {}
            }
        }
        sums
    }

    /// Each kept line: its stat, its source and the line, by stat and then
    /// in add order.
    pub fn lines(&self) -> impl Iterator<Item = (&str, &str, &Line)> {
        self.kept.iter().flat_map(|kept| {
            kept.lines
                .iter()
                .map(|kept| (kept.stat.as_str(), kept.source.as_str(), &kept.line))
        })
    }

    /// Removes every modifier and kept line `source` gave, on every stat,
    /// and recomputes those stats; how many it removed.
    pub fn remove_modifiers(&mut self, source: &str) -> usize {
        let mut removed = 0;
        for stat in &mut self.stats {
            let before = stat.modifiers.len();
            stat.modifiers.retain(|(from, _)| from != source);
            if stat.modifiers.len() != before {
                removed += before - stat.modifiers.len();
                stat.recompute();
            }
        }
        if let Some(kept) = &mut self.kept {
            let before = kept.lines.len();
            kept.lines.retain(|kept| kept.source != source);
            removed += before - kept.lines.len();
        }
        self.drop_empty_kept();
        removed
    }

    /// Each stat's name and value, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)> {
        self.stats
            .iter()
            .map(|stat| (stat.name.as_str(), stat.value))
    }

    /// The [`WorldSources`] revision the stats were brought to.
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn set_revision(&mut self, revision: u64) {
        self.revision = revision;
    }

    /// Whether what `source` gave is exactly what `wanted` would give,
    /// `wanted` sorted by stat: each stat's modifiers from it, then its kept
    /// lines, in order.
    pub(crate) fn holds_exactly(&self, source: &str, wanted: &[(String, Line)]) -> bool {
        for entry in &self.stats {
            let have = entry
                .modifiers
                .iter()
                .filter(|(from, _)| from == source)
                .map(|&(_, modifier)| modifier);
            let want = wanted
                .iter()
                .filter(|(stat, line)| *stat == entry.name && !line.is_scoped())
                .map(|(_, line)| line.modifier());
            if !have.eq(want) {
                return false;
            }
        }
        let have = self
            .kept
            .iter()
            .flat_map(|kept| &kept.lines)
            .filter(|kept| kept.source == source)
            .map(|kept| (kept.stat.as_str(), &kept.line));
        let want = wanted
            .iter()
            .filter(|(stat, line)| line.is_scoped() || self.find(stat).is_none())
            .map(|(stat, line)| (stat.as_str(), line));
        have.eq(want)
    }

    /// The kept lines on `stat`, in add order.
    fn kept_on<'s>(&'s self, stat: &'s str) -> impl Iterator<Item = &'s KeptLine> {
        let lines = self.kept.as_deref().map_or(&[][..], |kept| &kept.lines);
        let start = lines.partition_point(|kept| kept.stat.as_str() < stat);
        lines[start..]
            .iter()
            .take_while(move |kept| kept.stat == stat)
    }

    /// Drops the box once it keeps no line.
    fn drop_empty_kept(&mut self) {
        if self.kept.as_ref().is_some_and(|kept| kept.lines.is_empty()) {
            self.kept = None;
        }
    }

    fn search(&self, stat: &str) -> Result<usize, usize> {
        self.stats
            .binary_search_by(|entry| entry.name.as_str().cmp(stat))
    }

    fn find(&self, stat: &str) -> Option<&Stat> {
        self.search(stat).ok().map(|at| &self.stats[at])
    }

    fn find_mut(&mut self, stat: &str) -> Option<&mut Stat> {
        self.search(stat).ok().map(|at| &mut self.stats[at])
    }
}

/// A change to a stat, from a source. A match outside this crate needs a
/// wildcard arm, since later forms are additions.
///
/// Data form, snake case: `{ "add": 20 }`, `{ "pct": 0.5 }`,
/// `{ "mul": 2 }`, `{ "override": 0 }`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Modifier {
    /// Added to the base.
    Add(f32),
    /// Added to the percentage factor, which starts at 1: 0.5 is +50 %.
    Pct(f32),
    /// Multiplies the result.
    Mul(f32),
    /// Replaces the result; the last-added override wins.
    Override(f32),
}

impl Modifier {
    /// Whether its value is finite.
    pub(crate) fn is_finite(self) -> bool {
        match self {
            Modifier::Add(v) | Modifier::Pct(v) | Modifier::Mul(v) | Modifier::Override(v) => {
                v.is_finite()
            }
        }
    }
}

/// The game stats declared at build, a World resource: name, min, max.
#[derive(Debug, Default)]
struct StatDeclarations(Vec<(String, f32, f32)>);

/// A `stats` data entry: stat bounds by name, each declaring a game stat.
#[derive(Deserialize)]
#[serde(try_from = "BTreeMap<String, StatBoundsDef>")]
struct StatBoundsList(BTreeMap<String, (f32, f32)>);

impl TryFrom<BTreeMap<String, StatBoundsDef>> for StatBoundsList {
    type Error = String;

    fn try_from(entries: BTreeMap<String, StatBoundsDef>) -> Result<Self, Self::Error> {
        let mut out = BTreeMap::new();
        for (name, bounds) in entries {
            if !valid_stat(&name) {
                return Err(format!("`{name}` is not a stat name (`[a-z][a-z0-9_]*`)"));
            }
            if well_known_bounds(&name).is_some() {
                return Err(format!(
                    "`{name}` is a well-known stat; its bounds are the kit's"
                ));
            }
            out.insert(
                name,
                (
                    bounds.min.unwrap_or(f32::NEG_INFINITY),
                    bounds.max.unwrap_or(f32::INFINITY),
                ),
            );
        }
        Ok(Self(out))
    }
}

/// A game stat's bounds as data writes them: `{ "min": 0, "max": 10 }`,
/// each optional.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "StatBoundsFields")]
struct StatBoundsDef {
    #[serde(default)]
    min: Option<f32>,
    #[serde(default)]
    max: Option<f32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatBoundsFields {
    #[serde(default)]
    min: Option<f32>,
    #[serde(default)]
    max: Option<f32>,
}

impl TryFrom<StatBoundsFields> for StatBoundsDef {
    type Error = String;

    fn try_from(fields: StatBoundsFields) -> Result<Self, Self::Error> {
        for (field, value) in [("min", fields.min), ("max", fields.max)] {
            if let Some(value) = value
                && !value.is_finite()
            {
                return Err(format!("{field}: must be finite (got {value})"));
            }
        }
        check_bounds(
            fields.min.unwrap_or(f32::NEG_INFINITY),
            fields.max.unwrap_or(f32::INFINITY),
        )?;
        Ok(Self {
            min: fields.min,
            max: fields.max,
        })
    }
}

/// `components.stats` as a scene writes it: by stat name, a base or a base
/// with the entity's own lines, each name and value checked.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, StatEntryDef>")]
struct StatsDef(BTreeMap<String, StatEntryDef>);

impl TryFrom<BTreeMap<String, StatEntryDef>> for StatsDef {
    type Error = String;

    fn try_from(entries: BTreeMap<String, StatEntryDef>) -> Result<Self, Self::Error> {
        for (name, entry) in &entries {
            if !valid_stat(name) {
                return Err(format!("`{name}` is not a stat name (`[a-z][a-z0-9_]*`)"));
            }
            if !entry.base.is_finite() {
                return Err(format!("{name}: must be finite (got {})", entry.base));
            }
        }
        Ok(Self(entries))
    }
}

/// One stat as a scene writes it: its base, or `{ "base": 1, "lines": [..]
/// }`.
struct StatEntryDef {
    base: f32,
    lines: Vec<Line>,
}

/// The object form of a [`StatEntryDef`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StatEntryFields {
    base: f32,
    #[serde(default)]
    lines: Vec<Line>,
}

impl Serialize for StatEntryDef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct as _;
        if self.lines.is_empty() {
            return serializer.serialize_f32(self.base);
        }
        let mut entry = serializer.serialize_struct("StatEntry", 2)?;
        entry.serialize_field("base", &self.base)?;
        entry.serialize_field("lines", &self.lines)?;
        entry.end()
    }
}

impl<'de> Deserialize<'de> for StatEntryDef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StatEntryVisitor)
    }
}

/// Reads either form of a stat.
struct StatEntryVisitor;

impl<'de> Visitor<'de> for StatEntryVisitor {
    type Value = StatEntryDef;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a base, or an object of `base` and `lines`")
    }

    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<StatEntryDef, E> {
        Ok(StatEntryDef {
            base: value as f32,
            lines: Vec::new(),
        })
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<StatEntryDef, E> {
        self.visit_f64(value as f64)
    }

    fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<StatEntryDef, E> {
        self.visit_f64(value as f64)
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<StatEntryDef, A::Error> {
        let fields =
            StatEntryFields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
        Ok(StatEntryDef {
            base: fields.base,
            lines: fields.lines,
        })
    }
}

/// `stat`'s bounds: a well-known stat's, else the first declaration found,
/// the build's and then the `stats` entries' in ID order; `None` for an
/// undeclared stat.
fn bounds_of(world: &World, stat: &str) -> Option<(f32, f32)> {
    if let Some(bounds) = well_known_bounds(stat) {
        return Some(bounds);
    }
    let by_build = world
        .get_resource::<StatDeclarations>()
        .and_then(|declared| {
            declared
                .0
                .iter()
                .find(|(name, ..)| name == stat)
                .map(|&(_, min, max)| (min, max))
        });
    by_build.or_else(|| {
        let data = world.get_resource::<DataRegistry>()?;
        data.ids(STATS).find_map(|id| {
            data.get::<StatBoundsList>(id)
                .and_then(|list| list.0.get(stat).copied())
        })
    })
}

/// Registers the `stats` scene component and data kind, declares the game
/// stats a load may name, and adds [`STATS_APPLY`] to the tick phase, after
/// the health tick by registration, so a spawned entity moves at its stats
/// from its first step. A game stat is declared by
/// [`with_stat`](Self::with_stat) at build or by an entry of the `stats`
/// kind, `{ "armor": { "min": 0, "max": 10 }, "luck": {} }` under the
/// manifest's `data.stats`; the first declaration found gives its bounds.
/// A scene's stat that is neither well-known nor declared, or whose base
/// lies outside its bounds, fails the load.
///
/// # Examples
///
/// ```
/// use tungsten_core::{Plugin, Schedule, Stage, Time, World};
/// use tungsten_kit::{Health, Stats, StatsPlugin, TopDownMover};
///
/// let mut world = World::new();
/// world.insert_resource(Time::new());
/// let mut schedule = Schedule::new();
/// StatsPlugin::new().with_stat("armor", 0.0, 10.0).build(&mut schedule, &mut world);
/// schedule.resolve().unwrap();
///
/// let knight = world.spawn_with((
///     TopDownMover::new(100.0),
///     Health::new(5.0),
///     Stats::new().with(Stats::MOVE_SPEED, 60.0).with(Stats::MAX_HEALTH, 8.0),
/// ));
/// world.resource_mut::<Time>().advance_frame(1.0 / 60.0);
/// schedule.run_stage(Stage::FixedUpdate, &mut world, |_, _| {});
/// let health = world.get::<Health>(knight).unwrap();
/// assert_eq!((health.current(), health.max()), (8.0, 8.0), "5/5 becomes 8/8");
/// ```
#[derive(Debug, Default, Clone)]
pub struct StatsPlugin {
    declared: Vec<(String, f32, f32)>,
}

impl StatsPlugin {
    /// A plugin declaring no game stat at build; the `stats` data kind
    /// declares the rest.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The plugin, declaring the game stat `name` clamped to `[min, max]`,
    /// either end infinite for none.
    ///
    /// # Panics
    ///
    /// When `name` is not `[a-z][a-z0-9_]*` or is a well-known stat, or
    /// `min` is above `max` or either is NaN.
    #[must_use]
    #[track_caller]
    pub fn with_stat(mut self, name: &str, min: f32, max: f32) -> Self {
        assert!(
            valid_stat(name),
            "StatsPlugin::with_stat: `{name}` is not a stat name (`[a-z][a-z0-9_]*`)"
        );
        assert!(
            well_known_bounds(name).is_none(),
            "StatsPlugin::with_stat: `{name}` is a well-known stat; its bounds are the kit's"
        );
        if let Err(error) = check_bounds(min, max) {
            panic!("StatsPlugin::with_stat: {name}: {error}");
        }
        self.declared.push((name.to_owned(), min, max));
        self
    }
}

impl Plugin for StatsPlugin {
    fn build(&self, schedule: &mut Schedule, world: &mut World) {
        ensure_phases(schedule);
        register_rows::<Stats>(world, "Stats");
        world.register_saved::<Stats>(STATS);
        describe(
            world,
            ComponentDescription::new(
                STATS,
                "Named values with ordered modifiers, which the movers, health and cooldowns read.",
            )
            .field(FieldDescription::new(
                "bases",
                FieldType::Object("stat name → finite base, or { base, lines: [line] }"),
                "the value itself: each stat well-known or declared, its base within its bounds; a line is `{ modifier, with, while, against }`, the entity's own under the source `base`, its tags declared",
            ))
            .genre(PLATFORMER)
            .genre(TOP_DOWN)
            .sample(r#"{ "move_speed": 160 }"#)
            .roundtrip(round_trip::<StatsDef>),
        );
        describe_shape(
            world,
            ComponentDescription::new(STAT_BOUNDS, "A game stat's clamp.")
                .field(
                    FieldDescription::new(
                        "min",
                        FieldType::F32 {
                            min: Bound::Open,
                            max: Bound::Open,
                        },
                        "the least value; none when left out",
                    )
                    .default("null"),
                )
                .field(
                    FieldDescription::new(
                        "max",
                        FieldType::F32 {
                            min: Bound::Open,
                            max: Bound::Open,
                        },
                        "the greatest value, at least `min`; none when left out",
                    )
                    .default("null"),
                )
                .sample("{}")
                .roundtrip(round_trip::<StatBoundsDef>),
        );
        describe_kind(
            world,
            STATS,
            "An object of `stat_bounds` by stat name: each declares a game stat and its clamp.",
        );
        if let Some(declared) = world.get_resource_mut::<StatDeclarations>() {
            declared.0.extend(self.declared.iter().cloned());
        } else {
            world.insert_resource(StatDeclarations(self.declared.clone()));
        }
        world.register_data_kind::<StatBoundsList>(STATS);
        world.register_component_with_assets(
            STATS,
            |def: &StatsDef, world: &World| {
                for (name, entry) in &def.0 {
                    let base = entry.base;
                    let Some((min, max)) = bounds_of(world, name) else {
                        return Err(MissingAsset::new(name.as_str(), "stat", name.as_str()));
                    };
                    if !(min..=max).contains(&base) {
                        return Err(MissingAsset::new(
                            name.as_str(),
                            "stat whose bounds hold its base",
                            format!("{name} = {base}"),
                        ));
                    }
                    for (i, line) in entry.lines.iter().enumerate() {
                        line.check(world, &format!("{name}.lines.{i}"))?;
                    }
                }
                Ok(())
            },
            |def: StatsDef, ctx: &SpawnContext<'_>| {
                let mut stats = Stats::new();
                for (name, entry) in def.0 {
                    stats.set_base(&name, entry.base);
                    if well_known_bounds(&name).is_none()
                        && let Some((min, max)) = bounds_of(ctx.world(), &name)
                    {
                        stats = stats.with_bounds(&name, min, max);
                    }
                    for line in entry.lines {
                        stats.add_line(&name, BASE_SOURCE, line);
                    }
                }
                (stats,)
            },
        );
        schedule.add(
            Phase::Tick.stage(),
            Phase::Tick.join(system(STATS_APPLY, stats_apply)),
        );
    }
}

/// The source of an entity's own lines, written in its `stats`.
const BASE_SOURCE: &str = "base";

/// Writes each well-known stat an entity has into its item, when they
/// differ; on a step that met stats behind the [`WorldSources`] revision,
/// brings them up to date and writes again, so a spawn's items take the
/// world's sources in the step its stats first apply. A world without a
/// source never meets one, so it runs one query.
fn stats_apply(world: &mut World) {
    let revision = world
        .get_resource::<WorldSources>()
        .map_or(0, WorldSources::revision);
    if write_each(world, revision) {
        bring_stale(world, revision);
        write_each(world, revision);
    }
}

/// Writes each well-known stat an entity has into its item, when they
/// differ; whether any entity's stats lag `revision`.
fn write_each(world: &mut World, revision: u64) -> bool {
    let mut stale = false;
    for (stats, mover, controller, steering, health, cooldown, hitbox, contact) in world
        .query_mut::<(
            &Stats,
            Option<&mut TopDownMover>,
            Option<&mut PlatformerController>,
            Option<&mut Steering>,
            Option<&mut Health>,
            Option<&mut Cooldown>,
            Option<&mut Hitbox>,
            Option<&mut ContactDamage>,
        )>()
    {
        stale |= stats.revision != revision;
        if let Some(speed) = stats.get(Stats::MOVE_SPEED) {
            if let Some(mover) = mover {
                mover.set_speed(speed);
            }
            if let Some(controller) = controller {
                controller.set_run_speed(speed);
            }
            if let Some(steering) = steering {
                steering.set_max_speed(speed);
            }
        }
        if let (Some(max), Some(health)) = (stats.get(Stats::MAX_HEALTH), health) {
            health.set_max(max);
        }
        if let (Some(rate), Some(cooldown)) = (stats.get(Stats::COOLDOWN_RATE), cooldown) {
            cooldown.set_rate(rate);
        }
        if let Some(damage) = stats.get(Stats::DAMAGE) {
            if let Some(hitbox) = hitbox {
                hitbox.set_damage(damage);
            }
            if let Some(contact) = contact {
                contact.set_damage(damage);
            }
        }
    }
    stale
}

impl Inspectable for Stats {
    /// A row per stat, then one for the kept lines when there are any.
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        let mut rows: Vec<(&'static str, String)> = self
            .stats
            .iter()
            .map(|stat| {
                (
                    "stat",
                    format!(
                        "{} = {} (base {}, {} mods)",
                        stat.name,
                        stat.value,
                        stat.base,
                        stat.modifiers.len()
                    ),
                )
            })
            .collect();
        if let Some(kept) = &self.kept {
            let lines: Vec<String> = kept
                .lines
                .iter()
                .map(|kept| format!("{} from {}", kept.stat, kept.source))
                .collect();
            rows.push(("lines", lines.join(", ")));
        }
        rows
    }
}

#[cfg(test)]
#[path = "../tests/stats.rs"]
mod tests;
