//! Stats: named values with ordered modifiers, which the kit's movers,
//! health and cooldowns read when an entity carries them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tungsten_core::{
    DataRegistry, Inspectable, MissingAsset, Plugin, Schedule, SpawnContext, World, system,
};

use crate::damage::health::Health;
use crate::foundation::describe::{
    Bound, ComponentDescription, FieldDescription, FieldType, PLATFORMER, TOP_DOWN, describe,
    describe_kind, describe_shape, round_trip,
};
use crate::foundation::inspect::register_rows;
use crate::foundation::phases::{Phase, ensure_phases};
use crate::lifecycle::cooldown::Cooldown;
use crate::motion::platformer::PlatformerController;
use crate::motion::steering::Steering;
use crate::motion::top_down::TopDownMover;

/// [`StatsPlugin`]'s system, in the tick phase.
pub const STATS_APPLY: &str = "stats_apply";

/// The scene component the plugin registers, and the data kind.
const STATS: &str = "stats";
/// The shape of a `stats` entry's value.
const STAT_BOUNDS: &str = "stat_bounds";

/// Whether `name` is a stat name: `[a-z][a-z0-9_]*`.
fn valid_stat(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The bounds of a well-known stat: `max_health` from 1, the rest from 0,
/// none above; `None` for any other name.
fn well_known_bounds(name: &str) -> Option<(f32, f32)> {
    if name == Stats::MAX_HEALTH {
        Some((1.0, f32::INFINITY))
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

    /// `((base + Σadd) × (1 + Σpct) × Πmul)`, replaced by the last-added
    /// override, then clamped to the bounds and to `±f32::MAX`. The sums
    /// and products run in `f64`, and a zero factor makes the product 0, so
    /// no `0 × ∞` arises and the value is always finite.
    fn recompute(&mut self) {
        let mut sum = f64::from(self.base);
        let mut pct = 1.0_f64;
        let mut product = 1.0_f64;
        let mut zero = false;
        let mut over = None;
        for (_, modifier) in &self.modifiers {
            match *modifier {
                Modifier::Add(v) => sum += f64::from(v),
                Modifier::Pct(v) => pct += f64::from(v),
                Modifier::Mul(0.0) => zero = true,
                Modifier::Mul(v) => product *= f64::from(v),
                Modifier::Override(v) => over = Some(f64::from(v)),
            }
        }
        let raw = match over {
            Some(value) => value,
            None if zero || sum == 0.0 || pct == 0.0 => 0.0,
            None => sum * pct * product,
        };
        let limit = f64::from(f32::MAX);
        let value = raw
            .clamp(f64::from(self.min), f64::from(self.max))
            .clamp(-limit, limit);
        self.value = value as f32;
    }
}

/// An entity's stats: named values such as `move_speed` or `armor`, each a
/// base changed by modifiers. A stat's value is `((base + Σadd) × (1 +
/// Σpct) × Πmul)`, replaced by the last-added [`Modifier::Override`], then
/// clamped to its bounds, and it is recomputed when a base or a modifier
/// changes, so [`get`](Self::get) never computes. Every value is finite:
/// the arithmetic runs in `f64`, a zero factor makes the product 0, and
/// the result is clamped to `±f32::MAX` too.
///
/// The well-known stats ([`WELL_KNOWN`](Self::WELL_KNOWN)) are bounded to
/// `[0, ∞)`, [`MAX_HEALTH`](Self::MAX_HEALTH) to `[1, ∞)`. [`StatsPlugin`]'s
/// [`STATS_APPLY`] writes each one an entity has into its item, when they
/// differ: [`MOVE_SPEED`](Self::MOVE_SPEED) into a [`TopDownMover`]'s speed,
/// a [`PlatformerController`]'s run speed or a [`Steering`]'s top speed (0
/// stops it),
/// [`MAX_HEALTH`](Self::MAX_HEALTH) into a [`Health`]'s max, its current
/// health kept as the same fraction of it, and
/// [`COOLDOWN_RATE`](Self::COOLDOWN_RATE) into a [`Cooldown`]'s rate. An
/// item without that stat runs on its own field. A game's stat is
/// unbounded unless [`with_bounds`](Self::with_bounds),
/// [`StatsPlugin::with_stat`] or the `stats` data kind bound it.
///
/// Modifiers are keyed by a source, such as an effect's ID, so
/// [`remove_modifiers`](Self::remove_modifiers) takes back everything one
/// source gave.
///
/// Scene name `stats`: `{ "move_speed": 160, "armor": 2 }`, the base
/// values by stat name; the load checks each name is well-known or
/// declared and each base lies in its bounds. It is data: a game's save
/// holds the bases, bounds and modifiers.
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
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(
    try_from = "BTreeMap<String, StatFields>",
    into = "BTreeMap<String, StatFields>"
)]
pub struct Stats {
    /// Sorted by name.
    stats: Vec<Stat>,
}

/// A stat as a save writes it: unbounded ends as `null`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StatFields {
    base: f32,
    #[serde(default)]
    min: Option<f32>,
    #[serde(default)]
    max: Option<f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    modifiers: Vec<(String, Modifier)>,
}

impl TryFrom<BTreeMap<String, StatFields>> for Stats {
    type Error = String;

    fn try_from(fields: BTreeMap<String, StatFields>) -> Result<Self, Self::Error> {
        let mut stats = Vec::with_capacity(fields.len());
        for (name, fields) in fields {
            if !valid_stat(&name) {
                return Err(format!("`{name}` is not a stat name (`[a-z][a-z0-9_]*`)"));
            }
            if !fields.base.is_finite() {
                return Err(format!("{name}.base: must be finite (got {})", fields.base));
            }
            let min = fields.min.unwrap_or(f32::NEG_INFINITY);
            let max = fields.max.unwrap_or(f32::INFINITY);
            check_bounds(min, max).map_err(|e| format!("{name}: {e}"))?;
            if let Some((_, modifier)) = fields.modifiers.iter().find(|(_, m)| !m.is_finite()) {
                return Err(format!(
                    "{name}: a modifier must be finite (got {modifier:?})"
                ));
            }
            let mut stat = Stat::new(&name, fields.base);
            stat.min = min;
            stat.max = max;
            stat.modifiers = fields.modifiers;
            stat.recompute();
            stats.push(stat);
        }
        Ok(Self { stats })
    }
}

impl From<Stats> for BTreeMap<String, StatFields> {
    fn from(stats: Stats) -> Self {
        stats
            .stats
            .into_iter()
            .map(|stat| {
                let fields = StatFields {
                    base: stat.base,
                    min: stat.min.is_finite().then_some(stat.min),
                    max: stat.max.is_finite().then_some(stat.max),
                    modifiers: stat.modifiers,
                };
                (stat.name, fields)
            })
            .collect()
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
    /// A hit's damage, read from the damage pipeline on.
    pub const DAMAGE: &'static str = "damage";
    /// How fast a [`Cooldown`] counts down: 1 is the game clock, 2 twice
    /// as fast, 0 holds it.
    pub const COOLDOWN_RATE: &'static str = "cooldown_rate";
    /// An area's size, read from the effects on.
    pub const AREA: &'static str = "area";
    /// How near a pickup is collected, read from the pickups on.
    pub const PICKUP_RADIUS: &'static str = "pickup_radius";
    /// A projectile's speed, read from the projectiles on.
    pub const PROJECTILE_SPEED: &'static str = "projectile_speed";
    /// The stats every game knows without declaring them.
    pub const WELL_KNOWN: [&'static str; 7] = [
        Self::MOVE_SPEED,
        Self::MAX_HEALTH,
        Self::DAMAGE,
        Self::COOLDOWN_RATE,
        Self::AREA,
        Self::PICKUP_RADIUS,
        Self::PROJECTILE_SPEED,
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
    /// [`with`](Self::with)'s bounds.
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
            Err(at) => self.stats.insert(at, Stat::new(stat, base)),
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

    /// Removes every modifier `source` gave, on every stat, and recomputes
    /// those stats; how many it removed.
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
        removed
    }

    /// Each stat's name and value, by name.
    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)> {
        self.stats
            .iter()
            .map(|stat| (stat.name.as_str(), stat.value))
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
    fn is_finite(self) -> bool {
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

/// `components.stats` as a scene writes it: base values by name, each
/// name and value checked.
#[derive(Serialize, Deserialize)]
#[serde(try_from = "BTreeMap<String, f32>")]
struct StatsDef(BTreeMap<String, f32>);

impl TryFrom<BTreeMap<String, f32>> for StatsDef {
    type Error = String;

    fn try_from(bases: BTreeMap<String, f32>) -> Result<Self, Self::Error> {
        for (name, base) in &bases {
            if !valid_stat(name) {
                return Err(format!("`{name}` is not a stat name (`[a-z][a-z0-9_]*`)"));
            }
            if !base.is_finite() {
                return Err(format!("{name}: must be finite (got {base})"));
            }
        }
        Ok(Self(bases))
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
        describe(
            world,
            ComponentDescription::new(
                STATS,
                "Named values with ordered modifiers, which the movers, health and cooldowns read.",
            )
            .field(FieldDescription::new(
                "bases",
                FieldType::Object("stat name → finite base"),
                "the value itself: each stat well-known or declared, its base within its bounds",
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
                for (name, &base) in &def.0 {
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
                }
                Ok(())
            },
            |def: StatsDef, ctx: &SpawnContext<'_>| {
                let mut stats = Stats::new();
                for (name, base) in def.0 {
                    stats.set_base(&name, base);
                    if well_known_bounds(&name).is_none()
                        && let Some((min, max)) = bounds_of(ctx.world(), &name)
                    {
                        stats = stats.with_bounds(&name, min, max);
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

/// Writes each well-known stat an entity has into its item, when they
/// differ.
fn stats_apply(world: &mut World) {
    for (stats, mover, controller, steering, health, cooldown) in world.query_mut::<(
        &Stats,
        Option<&mut TopDownMover>,
        Option<&mut PlatformerController>,
        Option<&mut Steering>,
        Option<&mut Health>,
        Option<&mut Cooldown>,
    )>() {
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
    }
}

impl Inspectable for Stats {
    fn inspect_rows(&self) -> Vec<(&'static str, String)> {
        self.stats
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
            .collect()
    }
}

#[cfg(test)]
#[path = "../tests/stats.rs"]
mod tests;
